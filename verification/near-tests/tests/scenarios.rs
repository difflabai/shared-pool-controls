//! The published scenarios are what the pinned core does (SCN-1).
//!
//! `../source-mirror/scenarios/<name>/{config.json,trace.json}` are exported
//! by `crates/demo` (`pool-demo export`, `crates/demo/src/main.rs:36-56`).
//! This test re-derives each one from `pool_core` with the same steps, at the
//! same fixed time, and compares the JSON values: per fixture request
//! `preview`, then `submit` when allowed, then `dispatch` (if Reserved),
//! `acknowledge(true)` and `settle(true)` by the recipe's executor. The pools
//! carry `first_id` and `completed_units`, so a stale export fails here.
// PROP: SCN-1
use near_sdk::serde_json::{self, json, Value};
use pool_core::{fixtures, Pool, Request, Status};
use std::path::Path;

/// The fixed fixture epoch of `crates/demo/src/main.rs:36`.
const TIME: u64 = 1_794_441_600;

fn request(p: &Pool, recipe: u32, units: u32) -> Request {
    Request {
        recipe,
        units,
        version: p.version,
        expires_at: TIME + 3600,
    }
}

fn finish(p: &mut Pool, id: u32) {
    let c = p.record(id).expect("a submitted record").clone();
    let r = p.config.recipes[c.recipe as usize].clone();
    if c.status == Status::Reserved {
        p.dispatch(&r.actor, id, TIME).expect("dispatch");
    }
    p.acknowledge(id, true).expect("acknowledge");
    p.settle(&r.executor, id, true).expect("settle");
}

fn derive(name: &str) -> Value {
    let mut p = Pool::new(fixtures::config(name).expect("a fixture"), TIME).expect("a valid fixture");
    let initial = p.clone();
    let mut steps = vec![];
    for (r, u) in fixtures::trace(name) {
        let req = request(&p, r, u);
        let actor = p.config.recipes[r as usize].actor.clone();
        let d = p.preview(&actor, &req, TIME);
        let id = if d.allowed {
            Some(p.submit(&actor, req.clone(), TIME).expect("an allowed submit"))
        } else {
            None
        };
        if let Some(id) = id {
            finish(&mut p, id);
        }
        steps.push(json!({"request":req,"actor":actor,"decision":d,"id":id,"pool":p}));
    }
    json!({"scenario":name,"mode":"Rust core simulation; mocked execution","initial":initial,"steps":steps})
}

fn read(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// PROP: SCN-1
#[test]
fn scn1_published_scenarios_are_the_core() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../source-mirror/scenarios");
    let mut dirs: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    dirs.sort();
    let mut names: Vec<String> = fixtures::NAMES.iter().map(|s| s.to_string()).collect();
    names.sort();
    assert_eq!(dirs, names, "scenarios/ must hold exactly the fixtures");
    let mut admitted = 0;
    for name in fixtures::NAMES {
        let dir = root.join(name);
        let config = read(&dir.join("config.json"));
        assert_eq!(
            config,
            serde_json::to_value(fixtures::config(name).unwrap()).unwrap(),
            "{name}: config.json is not fixtures::config"
        );
        let published = read(&dir.join("trace.json"));
        let keys: Vec<&String> = published.as_object().expect("a trace object").keys().collect();
        assert_eq!(keys.len(), 4, "{name}: trace keys {keys:?}");
        let derived = derive(name);
        let steps = derived["steps"].as_array().unwrap();
        assert!(!steps.is_empty(), "{name}: no steps");
        for (k, s) in steps.iter().enumerate() {
            assert_eq!(
                published["steps"][k], *s,
                "{name}: step {k} differs from the core"
            );
            if !s["id"].is_null() {
                admitted += 1;
            }
        }
        assert_eq!(published, derived, "{name}: trace.json differs from the core");
        let last = &steps[steps.len() - 1]["pool"];
        assert!(last.get("first_id").is_some() && last.get("completed_units").is_some());
    }
    assert!(admitted > 0, "no scenario admits a request");
}
