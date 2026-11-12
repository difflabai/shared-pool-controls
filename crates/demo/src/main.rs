//! Local-only Rust demo server and reproducible JSON trace exporter.
//! No private keys, external execution, or production authentication.
use pool_core::{fixtures, Pool, Request, Status};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn request(p: &Pool, r: u32, u: u32, time: u64) -> Request {
    Request {
        recipe: r,
        units: u,
        version: p.version,
        expires_at: time + 3600,
    }
}
fn finish_at(p: &mut Pool, id: u32, time: u64) -> Result<(), pool_core::Error> {
    let c = p.record(id)?.clone();
    let r = p.config.recipes[c.recipe as usize].clone();
    if c.status == Status::Reserved {
        p.dispatch(&r.actor, id, time)?;
    }
    p.acknowledge(id, true)?;
    p.settle(&r.executor, id, true)
}
fn trace(name: &str) -> Value {
    let time = 1_794_441_600; // Fixed fixture epoch: reproducible exports.
    let mut p = Pool::new(fixtures::config(name).unwrap(), time).unwrap();
    let initial = p.clone();
    let mut steps = vec![];
    for (r, u) in fixtures::trace(name) {
        let req = request(&p, r, u, time);
        let actor = p.config.recipes[r as usize].actor.clone();
        let d = p.preview(&actor, &req, time);
        let id = if d.allowed {
            Some(p.submit(&actor, req.clone(), time).unwrap())
        } else {
            None
        };
        if let Some(id) = id {
            finish_at(&mut p, id, time).unwrap();
        }
        steps.push(json!({"request":req,"actor":actor,"decision":d,"id":id,"pool":p}));
    }
    json!({"scenario":name,"mode":"Rust core simulation; mocked execution", "initial":initial,"steps":steps})
}
fn api(path: &str, body: &[u8], pools: &mut BTreeMap<String, Pool>) -> Result<Value, String> {
    let input: Value = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(body).map_err(|e| e.to_string())?
    };
    if path == "/api/scenarios" {
        return Ok(json!(fixtures::NAMES));
    }
    let name = input
        .get("scenario")
        .and_then(Value::as_str)
        .unwrap_or("gpu");
    if !fixtures::NAMES.contains(&name) {
        return Err("Unknown scenario".into());
    }
    if path == "/api/reset" {
        pools.insert(
            name.into(),
            Pool::new(fixtures::config(name).unwrap(), now()).unwrap(),
        );
    }
    let p = pools
        .entry(name.into())
        .or_insert_with(|| Pool::new(fixtures::config(name).unwrap(), now()).unwrap());
    if path == "/api/state" || path == "/api/reset" {
        return Ok(json!({"pool":p}));
    }
    if path == "/api/trace" {
        return Ok(trace(name));
    }
    if path == "/api/observe" {
        let values = serde_json::from_value(input["values"].clone()).map_err(|e| e.to_string())?;
        p.observe(&p.config.owner.clone(), values, now())
            .map_err(|e| e.to_string())?;
        return Ok(json!({"pool":p}));
    }
    if path == "/api/lifecycle" {
        let id = input["id"]
            .as_u64()
            .filter(|v| *v <= u32::MAX as u64)
            .ok_or("Missing id")? as u32;
        let c = p.record(id).map_err(|e| e.to_string())?.clone();
        let r = p.config.recipes[c.recipe as usize].clone();
        match input["action"].as_str().unwrap_or("") {
            "complete" => finish_at(p, id, now()),
            "unknown" => {
                if c.status == Status::Reserved {
                    p.dispatch(&r.actor, id, now()).map_err(|e| e.to_string())?;
                }
                p.acknowledge(id, false)
            }
            "cancel" => p.cancel(&r.actor, id, now()),
            "not_executed" => p.settle(&r.executor, id, false),
            _ => return Err("Unknown lifecycle action".into()),
        }
        .map_err(|e| e.to_string())?;
        return Ok(json!({"pool":p}));
    }
    if !["/api/preview", "/api/submit", "/api/alternative"].contains(&path) {
        return Err("Unknown endpoint".into());
    }
    let req: Request =
        serde_json::from_value(input["request"].clone()).map_err(|e| e.to_string())?;
    // Identity selection is intentionally only a local simulator feature.
    let actor = input["actor"].as_str().ok_or("Missing actor")?;
    if path == "/api/alternative" {
        let alternative = p.best_quantity(actor, &req, now());
        return Ok(json!({"request":alternative,"pool":p}));
    }
    let decision = p.preview(actor, &req, now());
    let id = if path == "/api/submit" && decision.allowed {
        Some(p.submit(actor, req, now()).map_err(|e| e.to_string())?)
    } else {
        None
    };
    Ok(json!({"decision":decision,"id":id,"pool":p}))
}
fn response(s: &mut TcpStream, status: &str, mime: &str, body: &[u8]) {
    let h=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",body.len());
    let _ = s.write_all(h.as_bytes());
    let _ = s.write_all(body);
}
fn handle(s: &mut TcpStream, pools: &mut BTreeMap<String, Pool>, port: u16) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
    let mut data = vec![];
    let mut buf = [0; 4096];
    let end;
    loop {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        };
        if data.len() > 65536 {
            return;
        }
        if let Some(i) = data.windows(4).position(|x| x == b"\r\n\r\n") {
            end = i + 4;
            break;
        }
    }
    let headers = String::from_utf8_lossy(&data[..end]).to_string();
    let first = headers.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/").split('?').next().unwrap_or("/");
    let field = |name: &str| {
        headers
            .lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim().to_string())
    };
    let allowed_hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !field("Host")
        .map(|h| allowed_hosts.contains(&h))
        .unwrap_or(false)
    {
        response(s, "403 Forbidden", "text/plain", b"Local Host required");
        return;
    }
    if let Some(origin) = field("Origin") {
        if !allowed_hosts
            .iter()
            .any(|h| origin == format!("http://{h}"))
        {
            response(s, "403 Forbidden", "text/plain", b"Same origin required");
            return;
        }
    }
    let len = field("Content-Length")
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(0);
    if len > 32768 {
        return;
    }
    while data.len() < end + len {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    if path.starts_with("/api/") {
        if method != "POST" || field("Content-Type").as_deref() != Some("application/json") {
            response(
                s,
                "405 Method Not Allowed",
                "text/plain",
                b"POST application/json required",
            );
            return;
        }
        let result = api(path, &data[end..end + len], pools);
        let status = if result.is_ok() {
            "200 OK"
        } else {
            "400 Bad Request"
        };
        let body = serde_json::to_vec(&result.unwrap_or_else(|e| json!({"error":e}))).unwrap();
        response(s, status, "application/json", &body);
        return;
    }
    if method != "GET" {
        response(s, "405 Method Not Allowed", "text/plain", b"GET required");
        return;
    }
    // Static allowlist avoids filesystem traversal or exposing repository files.
    let (file, mime) = match path {
        "/" | "/index.html" => ("index.html", "text/html; charset=utf-8"),
        "/app.js" => ("app.js", "text/javascript"),
        "/base.css" => ("base.css", "text/css"),
        "/live.html" => ("live.html", "text/html; charset=utf-8"),
        "/live.js" => ("live.js", "text/javascript"),
        _ => {
            response(s, "404 Not Found", "text/plain", b"Not found");
            return;
        }
    };
    match std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../ui")
            .join(file),
    ) {
        Ok(b) => response(s, "200 OK", mime, &b),
        Err(_) => response(s, "404 Not Found", "text/plain", b"Not found"),
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str).unwrap_or("serve") {
        "export" => {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
            for name in fixtures::NAMES {
                let dir = root.join(name);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(
                    dir.join("config.json"),
                    serde_json::to_string_pretty(&fixtures::config(name).unwrap()).unwrap(),
                )
                .unwrap();
                std::fs::write(
                    dir.join("trace.json"),
                    serde_json::to_string_pretty(&trace(name)).unwrap(),
                )
                .unwrap();
            }
            println!("Exported all Rust fixture traces.");
        }
        "trace" => {
            let name = args.get(2).map(String::as_str).unwrap_or("gpu");
            assert!(fixtures::NAMES.contains(&name), "unknown scenario");
            println!("{}", serde_json::to_string_pretty(&trace(name)).unwrap());
        }
        "serve" => {
            let port = args
                .get(2)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(8080);
            let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind local demo port");
            println!("Shared Pool Controls: http://127.0.0.1:{port} (local Rust simulation; no keys or real funds)");
            let mut pools = BTreeMap::new();
            for mut stream in listener.incoming().flatten() {
                handle(&mut stream, &mut pools, port);
            }
        }
        _ => panic!("Usage: pool-demo [serve [port] | trace [scenario] | export]"),
    }
}
