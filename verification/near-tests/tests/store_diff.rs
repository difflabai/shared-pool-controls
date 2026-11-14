//! SH-8: differential test of the shell against the pool-core reference,
//! with the contract state reloaded (Borsh) on every call.
//!
//! Host path: STORE_DIFF_SEQS (default 2,000) sequences of STORE_DIFF_STEPS
//! (default 40) random calls, from the six fixtures, the F-1 wedge config,
//! `open_config` and random small configurations (the conformance generator's
//! distribution). Every call runs on a contract freshly `state_read` from
//! storage and is compared with `pool_core::Pool` (the same pinned crate):
//! return value, panic message, events, outgoing receipts, and the stored
//! bytes (= Borsh of the core's pool); views (get_pool, get_commitments,
//! preview) are compared after every call.
//!
//! Wasm path: WASM_DIFF_SEQS (default 300) sequences through the real wasm of
//! the documented build (build.sh, `--cfg near`; JSON in and out, near-sdk
//! wrappers), same comparisons, JSON views every few steps.
//!
//! Window (v1.1.0, MAX_RECORDS = 256): `sh8_store_diff_window` runs fewer,
//! longer sequences from a submit-heavy mix, so records are pruned and calls
//! name pruned ids, ids are `first_id + k`, and pages are by id.
//!
//! Every step also checks LIVE-ALL at the shell (`check_live_all`): no
//! `cancel`, `settle` or `on_result` fails `Bounds` or `Arithmetic`.
//!
//! Coverage: every error the generator can reach is required to occur, so a
//! generator that stopped exercising a guard fails the test.
//!
//! The reference (`core_step`) fixes, on every call, the caller as the
//! predecessor with 15% relayed calls (SH-1), the on_result mapping (SH-3),
//! the submit decision (SH-4), errors as panics that change nothing (SH-5),
//! and the dispatch and reconcile receipts (SH-6, SH-7). The views include
//! `preview`, compared with the core's (shell half of RD-PREVIEW).
//!
// PROP: SH-8 SH-1 SH-3 SH-4 SH-5 SH-6 SH-7 SH-9 RD-PREVIEW LIVE-ALL

mod common;
use common::*;
use near_sdk::serde_json::{self, json, Value};
use pool_core::{Config, Pool, Request, Status};
use std::collections::BTreeMap;

type Coverage = BTreeMap<String, u64>;

fn config_for(seq: u64, rng: &mut Rng) -> (String, Config) {
    match seq % 10 {
        0 => {
            let names = pool_core::fixtures::NAMES;
            let name = names[(seq / 10 % names.len() as u64) as usize];
            (name.to_string(), pool_core::fixtures::config(name).expect("fixture"))
        }
        1 => ("wedge".into(), wedge_config()),
        2 => ("open".into(), open_config()),
        _ => ("random".into(), random_config(rng)),
    }
}

/// What a call did, for coverage: method and result (error, submit reason,
/// on_result outcome/error).
fn label(op: &Op, r: &Result<Value, Fail>, events: &[Value]) -> String {
    let m = op.method();
    match (op, r) {
        (_, Err(f)) => format!("{m}:Err({})", f.msg()),
        (Op::Submit { .. }, Ok(v)) => format!("{m}:{}", v["decision"]["reason"].as_str().unwrap()),
        (Op::OnResult { results, .. }, Ok(_)) => {
            let s;
            let class = match results.as_slice() {
                [Res::Failed] => "failed",
                [Res::Ok(b)] if b.len() > 64 => "long",
                [Res::Ok(b)] => match serde_json::from_slice::<String>(b) {
                    Ok(x) => {
                        s = x;
                        match s.as_str() {
                            "completed" | "cancelled" | "active" | "unknown" => s.as_str(),
                            _ => "other",
                        }
                    }
                    Err(_) => "nonstring",
                },
                _ => "count",
            };
            let err = &events[0]["data"][0]["error"];
            format!("{m}:{class}:{}", err.as_str().unwrap_or("ok"))
        }
        (_, Ok(_)) => format!("{m}:Ok"),
    }
}

fn run_host(seq: u64, steps: u64, seed: u64, cov: &mut Coverage) {
    let mut rng = Rng::new(seed.wrapping_mul(1_000_003).wrapping_add(seq));
    let (name, cfg) = config_for(seq, &mut rng);
    let mut now = 100 + rng.below(1_000);
    let mut storage = deploy(cfg.clone(), now);
    let mut pool = Pool::new(cfg, now).unwrap();
    assert_eq!(storage, storage_of(&pool), "seq {seq}: new");
    for i in 0..steps {
        now += rng.below(4);
        let ts = now * 1_000_000_000 + rng.below(1_000_000_000);
        let (ctx, op) = random_step(&mut rng, &pool, ts);
        let what = format!("seed {seed} seq {seq} ({name}) step {i}: {op:?} by {} (signer {}) at {ts}", ctx.predecessor, ctx.signer);
        let out = step(storage, &mut pool, &ctx, &op, &what);
        storage = out.storage;
        // The stored bytes decode to the core's pool (Borsh round trip).
        assert_eq!(pool_of(&storage), pool, "{what}: reload");
        let events: Vec<Value> = out.logs.iter().map(|l| parse_event(l)).collect();
        *cov.entry(label(&op, &out.result, &events)).or_default() += 1;
        check_views(&storage, &pool, &mut rng, now, &what);
    }
}

/// Errors and outcomes the generator reaches; each must occur at least once
/// over the default run (a vacuity guard on the generator).
const REQUIRED_HOST: &[&str] = &[
    "submit:Allowed",
    "submit:Paused",
    "submit:Stale",
    "submit:Expired",
    "submit:SnapshotExpired",
    "submit:InvalidRecipe",
    "submit:Unauthorized",
    "submit:InvalidUnits",
    "submit:Prerequisite",
    "submit:Bounds",
    "submit:Energy",
    "dispatch:Ok",
    "dispatch:Err(NotFound)",
    "dispatch:Err(Unauthorized)",
    "dispatch:Err(Paused)",
    "dispatch:Err(InvalidState)",
    "dispatch:Err(Expired)",
    "dispatch:Err(SnapshotExpired)",
    "reconcile:Ok",
    "reconcile:Err(NotFound)",
    "reconcile:Err(InvalidState)",
    "reconcile:Err(Method reconcile doesn't accept deposit)",
    "on_result:active:ok",
    "on_result:completed:ok",
    "on_result:cancelled:ok",
    "on_result:unknown:ok",
    "on_result:other:ok",
    "on_result:failed:ok",
    "on_result:long:ok",
    "on_result:nonstring:ok",
    "on_result:count:ok",
    "on_result:completed:InvalidState",
    "on_result:cancelled:InvalidState",
    // v1.1.0: a result for an id with no record is logged, not a panic.
    "on_result:completed:NotFound",
    "on_result:failed:NotFound",
    "settle:Ok",
    "settle:Err(NotFound)",
    "settle:Err(Unauthorized)",
    "settle:Err(InvalidState)",
    "cancel:Ok",
    "cancel:Err(NotFound)",
    "cancel:Err(Unauthorized)",
    "cancel:Err(Pending)",
    // v1.1.0: cancelling a Completed record is refused (F-2b).
    "cancel:Err(InvalidState)",
    "set_paused:Ok",
    "set_paused:Err(Unauthorized)",
    "observe:Ok",
    "observe:Err(Unauthorized)",
    "observe:Err(Pending)",
    "observe:Err(Bounds)",
];

fn print_coverage(title: &str, cov: &Coverage) {
    let total: u64 = cov.values().sum();
    println!("{title}: {total} calls");
    for (k, v) in cov {
        println!("  {k:<40} {v}");
    }
}

// PROP: SH-8 SH-1 SH-3 SH-4 SH-5 SH-6 SH-7 SH-9 RD-PREVIEW
#[test]
fn sh8_store_diff_host() {
    let seqs = env_u64("STORE_DIFF_SEQS", 2_000);
    let steps = env_u64("STORE_DIFF_STEPS", 40);
    let seed = env_u64("STORE_DIFF_SEED", 1);
    let shards = 4;
    let covs: Vec<Coverage> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..shards)
            .map(|k| {
                s.spawn(move || {
                    quiet_expected_panics();
                    let mut cov = Coverage::new();
                    for seq in (k..seqs).step_by(shards as usize) {
                        run_host(seq, steps, seed, &mut cov);
                    }
                    cov
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p))).collect()
    });
    let mut cov = Coverage::new();
    for c in covs {
        for (k, v) in c {
            *cov.entry(k).or_default() += v;
        }
    }
    print_coverage(&format!("SH-8 host: {seqs} x {steps}, seed {seed}"), &cov);
    assert_eq!(cov.values().sum::<u64>(), seqs * steps);
    if seqs >= 2_000 && steps >= 40 {
        let missing: Vec<_> = REQUIRED_HOST.iter().filter(|k| !cov.contains_key(**k)).collect();
        assert!(missing.is_empty(), "never reached: {missing:?}");
    }
}

/// SH-8 across the v1.1.0 window: long sequences whose submits, dispatches
/// and settlements mostly succeed, so the pool retains 256 records and
/// prunes. Every pruned id is then NotFound (dispatch, reconcile, settle,
/// cancel) or a logged NotFound (`on_result`), and pages are by id. On the
/// host path or the documented wasm build.
fn run_window(path: Path, seq: u64, steps: u64, seed: u64, cov: &mut Coverage) -> u32 {
    let mut rng = Rng::new(seed.wrapping_mul(9_000_011).wrapping_add(seq) ^ 0x3a11);
    let cfg = open_config();
    let mut now = 100;
    let mut storage = match path {
        Path::Host => deploy(cfg.clone(), now),
        Path::Wasm(b) => wasm_deploy(b, &cfg, now),
    };
    let mut pool = Pool::new(cfg, now).unwrap();
    for i in 0..steps {
        now += rng.below(2);
        let ts = now * 1_000_000_000 + rng.below(1_000_000_000);
        // A third of the steps submit a fresh request (current version, not
        // expired) and half the rest drive the oldest open record to a
        // terminal state by its own roles, so the pool fills and prunes.
        let fresh = rng.chance(33);
        let (ctx, op) = match pool.records.iter().find(|c| !matches!(c.status, Status::Completed | Status::Cancelled)) {
            _ if fresh => {
                let k = rng.below(pool.config.recipes.len() as u64) as u32;
                let actor = pool.config.recipes[k as usize].actor.clone();
                let req = Request { recipe: k, units: 1 + rng.below(3) as u32, version: pool.version, expires_at: now + 100 };
                (Ctx::new(&actor, &actor, ts), Op::Submit { req })
            }
            Some(c) if rng.chance(50) => {
                let r = &pool.config.recipes[c.recipe as usize];
                match c.status {
                    Status::Reserved if rng.chance(60) => (Ctx::new(&r.actor, &r.actor, ts), Op::Dispatch { id: c.id }),
                    Status::Reserved => (Ctx::new(&r.actor, &r.actor, ts), Op::Cancel { id: c.id }),
                    _ => (Ctx::new(&r.executor, &r.executor, ts), Op::Settle { id: c.id, completed: rng.chance(60) }),
                }
            }
            _ => random_step(&mut rng, &pool, ts),
        };
        let what = format!("{path:?} window seed {seed} seq {seq} step {i}: {op:?} by {} at {ts}", ctx.predecessor);
        let (result, logs) = match path {
            Path::Host => {
                let out = step(storage, &mut pool, &ctx, &op, &what);
                storage = out.storage;
                (out.result, out.logs)
            }
            Path::Wasm(b) => {
                let exp = core_step(&mut pool.clone(), &ctx, &op);
                let out = wasm_diff_step(b, storage, &mut pool, &ctx, &op, &what);
                storage = out.storage;
                (exp.result, out.logs)
            }
        };
        let events: Vec<Value> = logs.iter().map(|l| parse_event(l)).collect();
        let pruned = match &op {
            Op::Dispatch { id } | Op::Reconcile { id } | Op::Settle { id, .. } | Op::Cancel { id } | Op::OnResult { id, .. } => *id < pool.first_id,
            _ => false,
        };
        let l = label(&op, &result, &events);
        *cov.entry(if pruned { format!("pruned:{l}") } else { l }).or_default() += 1;
        if i % 16 == 15 {
            match path {
                Path::Host => check_views(&storage, &pool, &mut rng, now, &what),
                Path::Wasm(b) => wasm_views(b, &storage, &pool, &mut rng, now, &what),
            }
        }
        // The window: ids are first_id + k, at most MAX_RECORDS retained.
        assert!(pool.records.len() <= pool_core::MAX_RECORDS, "{what}");
        for (k, c) in pool.records.iter().enumerate() {
            assert_eq!(c.id, pool.first_id + k as u32, "{what}: id of record {k}");
        }
    }
    pool.first_id
}

// PROP: SH-8 SH-5 SH-9 SH-3 LIVE-ALL
#[test]
fn sh8_store_diff_window() {
    let seqs = env_u64("STORE_DIFF_WINDOW_SEQS", 4);
    let steps = env_u64("STORE_DIFF_WINDOW_STEPS", 1_500);
    let seed = env_u64("STORE_DIFF_SEED", 1);
    // `seqs` host sequences and one on the documented build (256 MiB stacks:
    // wasmi in a debug build).
    let runs: Vec<(Path, u64)> = (0..seqs).map(|seq| (Path::Host, seq)).chain([(Path::Wasm(Build::Near), seqs)]).collect();
    let firsts: Vec<(u32, Coverage)> = std::thread::scope(|s| {
        let hs: Vec<_> = runs
            .iter()
            .map(|&(path, seq)| {
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn_scoped(s, move || {
                        quiet_expected_panics();
                        let mut cov = Coverage::new();
                        let f = run_window(path, seq, steps, seed, &mut cov);
                        (f, cov)
                    })
                    .unwrap()
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p))).collect()
    });
    let mut cov = Coverage::new();
    for (_, c) in &firsts {
        for (k, v) in c {
            *cov.entry(k.clone()).or_default() += v;
        }
    }
    print_coverage(
        &format!("SH-8 window: host {seqs} x {steps} + Near 1 x {steps}, seed {seed}, first_id {:?}", firsts.iter().map(|f| f.0).collect::<Vec<_>>()),
        &cov,
    );
    if seqs >= 4 && steps >= 1_500 {
        assert!(firsts.iter().all(|f| f.0 > 0), "a sequence never pruned");
        for k in ["pruned:dispatch:Err(NotFound)", "pruned:settle:Err(NotFound)", "pruned:cancel:Err(NotFound)", "pruned:reconcile:Err(NotFound)"] {
            assert!(cov.contains_key(k), "never reached {k}");
        }
        assert!(cov.keys().any(|k| k.starts_with("pruned:on_result:") && k.ends_with(":NotFound")), "never reached a pruned on_result");
    }
}

/// The wasm's JSON views equal the core's reads.
fn wasm_views(b: Build, storage: &Storage, pool: &Pool, rng: &mut Rng, now: u64, what: &str) {
    let w = wasm(b, "pool_contract");
    let call = |method: &str, args: Value| {
        let out = wasm_call(w, &Ctx::by(Z, now), storage.clone(), vec![], method, &serde_json::to_vec(&args).unwrap());
        assert!(out.logs.is_empty() && out.receipts.is_empty(), "{what}: view {method}");
        assert_eq!(&out.storage, storage, "{what}: view {method} changed state");
        out.json()
    };
    assert_eq!(call("get_pool", json!({})), serde_json::to_value(pool).unwrap(), "{what}: get_pool");
    let n = pool.records.len() as u64;
    let lo = pool.first_id.saturating_sub(2);
    let (from, limit) = (lo + rng.below(n + 5) as u32, rng.below(60) as u32);
    let want: Vec<Value> = expected_page(pool, from, limit).iter().map(commitment_json).collect();
    assert_eq!(call("get_commitments", json!({ "from": from, "limit": limit })), json!(want), "{what}: page");
    let accts = accounts_of(&pool.config);
    let actor = rng.pick(&accts).clone();
    let req = random_request(rng, pool, now);
    let d = pool.preview(&actor, &req, now);
    assert_eq!(
        call("preview", json!({ "actor": actor, "request": request_json(&req) })),
        decision_json(&d),
        "{what}: preview"
    );
}

fn run_wasm(b: Build, seq: u64, steps: u64, seed: u64, cov: &mut Coverage) {
    let mut rng = Rng::new(seed.wrapping_mul(7_000_003).wrapping_add(seq) ^ 0x5eed);
    let (name, cfg) = config_for(seq, &mut rng);
    let mut now = 100 + rng.below(1_000);
    let mut storage = wasm_deploy(b, &cfg, now);
    let mut pool = Pool::new(cfg, now).unwrap();
    assert_eq!(storage, storage_of(&pool), "seq {seq}: new");
    for i in 0..steps {
        now += rng.below(4);
        let ts = now * 1_000_000_000 + rng.below(1_000_000_000);
        let (ctx, op) = random_step(&mut rng, &pool, ts);
        let what = format!("{b:?} seed {seed} seq {seq} ({name}) step {i}: {op:?} by {} (signer {}) at {ts}", ctx.predecessor, ctx.signer);
        let exp = core_step(&mut pool.clone(), &ctx, &op);
        let out = wasm_diff_step(b, storage, &mut pool, &ctx, &op, &what);
        storage = out.storage.clone();
        assert_eq!(pool_of(&storage), pool, "{what}: reload");
        let events: Vec<Value> = out.logs.iter().map(|l| parse_event(l)).collect();
        *cov.entry(label(&op, &exp.result, &events)).or_default() += 1;
        if i % 4 == 3 {
            wasm_views(b, &storage, &pool, &mut rng, now, &what);
        }
    }
}

// PROP: SH-8 SH-1 SH-3 SH-4 SH-5 SH-6 SH-7 SH-9 RD-PREVIEW
#[test]
fn sh8_store_diff_wasm() {
    let seqs = env_u64("WASM_DIFF_SEQS", 300);
    let steps = env_u64("STORE_DIFF_STEPS", 40);
    let seed = env_u64("STORE_DIFF_SEED", 1);
    // One thread per build (256 MiB stacks: wasmi in a debug build).
    let hs: Vec<_> = Build::all()
        .into_iter()
        .map(|b| {
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || {
                    quiet_expected_panics();
                    let mut cov = Coverage::new();
                    for seq in 0..seqs {
                        run_wasm(b, seq, steps, seed, &mut cov);
                    }
                    (b, cov)
                })
                .unwrap()
        })
        .collect();
    for h in hs {
        let (b, cov) = h.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
        print_coverage(&format!("SH-8 wasm {b:?}: {seqs} x {steps}, seed {seed}"), &cov);
        assert_eq!(cov.values().sum::<u64>(), seqs * steps);
        if seqs >= 150 {
            for k in ["submit:Allowed", "dispatch:Ok", "reconcile:Ok", "on_result:failed:ok", "settle:Ok", "settle:Err(Unauthorized)", "observe:Ok", "cancel:Err(Pending)"] {
                assert!(cov.contains_key(k), "{b:?}: never reached {k}");
            }
        }
    }
}
