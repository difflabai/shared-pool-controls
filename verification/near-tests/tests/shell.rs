//! Exact shell tests of pool-contract v1.1.0 (SH-1..SH-7, SH-9, SH-10) and
//! the regression tests of this layer's four v1.0.0 findings, all fixed in
//! v1.1.0 (NT-F1, NT-F2, NT-F3's import half is SURF-2/SURF-3, NT-F4).
//!
//! Every scenario runs on two paths: the host path (the `#[near]` methods as
//! Rust functions, reload around every call) and the real wasm of the
//! documented build (`Near`: upstream scripts/build.sh, `--cfg near`). Each
//! call is compared with the core (`Sim`, through `step` / `wasm_diff_step`:
//! result, panic, events, receipts, Borsh state, and LIVE-ALL on reachable
//! states), and the tests below add explicit expectations on top. The
//! no-cfg build (`Plain`) appears only in `nt_f1_*`, as the negative
//! control.
//!
//! Each test carries its own tag line. The v1.0.0 witnesses are inverted,
//! not deleted: `nt_f1_*` under SH-5, `nt_f2_*` under SH-10, NT-F4's
//! `sh3_non_string_*` under SH-3. RD-PREVIEW here is the shell half only:
//! the `preview` view equals the core's `preview`.
//!
// PROP: SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-9 SH-10 RD-PREVIEW LIVE-ALL

mod common;
use common::*;
use near_sdk::serde_json::{self, json, Value};
use pool_core::{Commitment, Config, Error as E, Metric, Pool, Recipe, Request, Shape, Status, Term, VERSION_CAP};

fn guest(e: E) -> Fail {
    Fail::Guest(e.to_string())
}

/// open_config with one record of recipe 1 (a2, executor x2, complete -1 per
/// unit) for 3 units, dispatched at t=101.
fn dispatched(path: Path) -> Sim {
    let mut s = Sim::new(path, open_config(), 100);
    let r = s.ok(&Ctx::by(A2, 100), submit(1, 3, 0, 150));
    assert_eq!(r.value()["id"], json!(0));
    s.ok(&Ctx::by(A2, 101), Op::Dispatch { id: 0 });
    assert_eq!(s.status(0), Status::Dispatched);
    s
}

/// open_config after 257 submit and cancel cycles by a1 (the upstream
/// `pruned()` fixture): record 0 is pruned, records 1..=256 are retained
/// and Cancelled.
fn pruned(path: Path) -> Sim {
    let mut s = Sim::new(path, open_config(), 100);
    for id in 0..=pool_core::MAX_RECORDS as u32 {
        let r = s.ok(&Ctx::by(A1, 101), submit(0, 1, s.pool.version, 150));
        assert_eq!(r.value()["id"], json!(id));
        s.ok(&Ctx::by(A1, 101), Op::Cancel { id });
    }
    let p = pool_of(&s.storage);
    assert_eq!((p.first_id, p.records.len()), (1, pool_core::MAX_RECORDS));
    s
}

// ---------------------------------------------------------------------------
// SH-1: caller() is the predecessor; now() is the block timestamp in seconds
// ---------------------------------------------------------------------------

// PROP: SH-1
#[test]
fn sh1_caller_is_the_predecessor_not_the_signer() {
    big_stack(|| {
        for path in paths() {
            let t = |s: u64| s * 1_000_000_000 + 5;
            let mut s = Sim::new(path, open_config(), 100);
            // A signature relayed by another account is not the actor's call.
            let r = s.ok(&Ctx::new(RELAY, A1, t(101)), submit(0, 1, 0, 150));
            assert_eq!(r.value()["decision"]["reason"], json!("Unauthorized"), "{path:?}");
            assert_eq!(r.value()["id"], Value::Null);
            assert_eq!(r.event()["actor"], json!(RELAY));
            // The actor's own call counts, whoever signed the transaction.
            let r = s.ok(&Ctx::new(A1, Z, t(101)), submit(0, 1, 0, 150));
            assert_eq!(r.value()["id"], json!(0));
            assert_eq!(r.event()["actor"], json!(A1));
            s.err(&Ctx::new(RELAY, A1, t(102)), Op::Dispatch { id: 0 }, guest(E::Unauthorized));
            s.ok(&Ctx::new(A1, RELAY, t(102)), Op::Dispatch { id: 0 });
            s.err(
                &Ctx::new(A1, X1, t(103)),
                Op::Settle { id: 0, completed: true },
                guest(E::Unauthorized),
            );
            s.ok(&Ctx::new(X1, A1, t(103)), Op::Settle { id: 0, completed: true });
            assert_eq!(s.status(0), Status::Completed);
            s.ok(&Ctx::new(A1, A1, t(104)), submit(0, 1, s.pool.version, 150));
            s.err(&Ctx::new(RELAY, OWNER, t(104)), Op::Cancel { id: 1 }, guest(E::Unauthorized));
            s.ok(&Ctx::new(OWNER, RELAY, t(104)), Op::Cancel { id: 1 });
            s.err(&Ctx::new(RELAY, OWNER, t(105)), Op::Pause { paused: true }, guest(E::Unauthorized));
            s.ok(&Ctx::new(OWNER, RELAY, t(105)), Op::Pause { paused: true });
            s.ok(&Ctx::new(OWNER, Z, t(105)), Op::Pause { paused: false });
            s.err(
                &Ctx::new(RELAY, OWNER, t(106)),
                Op::Observe { values: vec![1] },
                guest(E::Unauthorized),
            );
            s.ok(&Ctx::new(OWNER, RELAY, t(106)), Op::Observe { values: vec![1] });
            // preview's actor is an argument, not the caller.
            let req = Request { recipe: 0, units: 1, version: s.pool.version, expires_at: 150 };
            let d = view(&s.storage, 106, |c| c.preview(acct(A1), req.clone()));
            assert!(d.allowed);
        }
    });
}

// PROP: SH-1
#[test]
fn sh1_now_is_the_block_timestamp_in_whole_seconds() {
    big_stack(|| {
        for path in paths() {
            let mut s = Sim::new(path, open_config(), 100);
            // 149.999999999 s is second 149: a request expiring at 150 is live.
            let r = s.ok(&Ctx::new(A1, A1, 149_999_999_999), submit(0, 1, 0, 150));
            assert_eq!(r.value()["id"], json!(0), "{path:?}");
            // At exactly 150 s it has expired (expires_at <= now), for submit
            // and for dispatch (now >= expires_at).
            let r = s.ok(&Ctx::new(A1, A1, 150_000_000_000), submit(0, 1, 1, 150));
            assert_eq!(r.value()["decision"]["reason"], json!("Expired"));
            s.err(&Ctx::new(A1, A1, 150_000_000_000), Op::Dispatch { id: 0 }, guest(E::Expired));
            // The 24 h horizon is measured from the same second.
            let r = s.ok(&Ctx::new(A1, A1, 160_999_999_999), submit(0, 1, 1, 160 + 86_400));
            assert_eq!(r.value()["decision"]["reason"], json!("Allowed"));
            let r = s.ok(&Ctx::new(A1, A1, 161_000_000_000), submit(0, 1, 2, 161 + 86_401));
            assert_eq!(r.value()["decision"]["reason"], json!("Expired"));
            s.ok(&Ctx::by(OWNER, 162), Op::Cancel { id: 0 });
            s.ok(&Ctx::by(OWNER, 162), Op::Cancel { id: 1 });
            s.ok(&Ctx::new(OWNER, OWNER, 300_999_999_999), Op::Observe { values: vec![3] });
            assert_eq!(pool_of(&s.storage).observed_at, 300);
        }
    });
}

// ---------------------------------------------------------------------------
// SH-2: `new` is private and runs once (wasm wrappers)
// ---------------------------------------------------------------------------

// PROP: SH-2
#[test]
fn sh2_new_is_private_and_initialises_once() {
    big_stack(|| {
        let cfg = open_config();
        let input = serde_json::to_vec(&json!({ "config": cfg })).unwrap();
        let empty = Storage::new();
        // Host path: new() stores exactly Borsh(Pool::new(config, now)).
        assert_eq!(deploy(cfg.clone(), 100), storage_of(&Pool::new(cfg.clone(), 100).unwrap()));
        for b in Build::all() {
            let w = wasm(b, "pool_contract");
            for who in [OWNER, A1, X1, Z] {
                // Any other predecessor, even when the pool's key signed.
                let ctx = Ctx::new(who, POOL, 100_000_000_000);
                let out = wasm_call(w, &ctx, empty.clone(), vec![], "new", &input);
                assert_wasm_fail(b, &out, &Fail::Guest("Method new is private".into()), &empty, "new by other");
                assert!(out.called("predecessor_account_id") && !out.called("input"), "{b:?}: {:?}", out.host_calls);
                assert!(!out.called("storage_write"));
            }
            let out = wasm_call(w, &Ctx::by(POOL, 100), empty.clone(), vec![], "new", &input);
            assert!(out.result.is_ok(), "{b:?}: {:?}", out.result);
            assert_eq!(out.storage, storage_of(&Pool::new(cfg.clone(), 100).unwrap()));
            assert!(out.logs.is_empty() && out.receipts.is_empty());
            // Init once: a second `new` (even a different config) fails.
            let other = serde_json::to_vec(&json!({ "config": wedge_config() })).unwrap();
            let again = wasm_call(w, &Ctx::by(POOL, 200), out.storage.clone(), vec![], "new", &other);
            assert_wasm_fail(
                b,
                &again,
                &Fail::Guest("The contract has already been initialized".into()),
                &out.storage,
                "second new",
            );
            assert!(again.called("storage_has_key") && !again.called("storage_write"));
            // Before init every method fails (PanicOnDefault).
            let get = wasm_call(w, &Ctx::by(Z, 100), empty.clone(), vec![], "get_pool", b"{}");
            assert_wasm_fail(b, &get, &Fail::Guest("The contract is not initialized".into()), &empty, "uninit");
            let new_with = |bad: &Config| {
                wasm_call(
                    w,
                    &Ctx::by(POOL, 100),
                    empty.clone(),
                    vec![],
                    "new",
                    &serde_json::to_vec(&json!({ "config": bad })).unwrap(),
                )
            };
            // An invalid config fails with the core's error...
            let mut bad = cfg.clone();
            bad.metrics.clear();
            assert_wasm_fail(b, &new_with(&bad), &guest(E::InvalidConfig), &empty, "invalid config");
            // ...including v1.1.0's rule that an actor is never its own
            // recipe's executor (docs/api.md:2) ...
            let mut bad = cfg.clone();
            bad.recipes[1].executor = bad.recipes[1].actor.clone();
            assert_wasm_fail(b, &new_with(&bad), &guest(E::InvalidConfig), &empty, "actor == executor");
            // ...which does not stop the owner from being an executor.
            let mut owner_exec = cfg.clone();
            owner_exec.recipes[0].executor = OWNER.into();
            let out = new_with(&owner_exec);
            assert!(out.result.is_ok(), "owner as executor: {:?}", out.result);
            assert_eq!(out.storage, storage_of(&Pool::new(owner_exec, 100).unwrap()));
            // ...and an invalid account name with the shell's own message.
            for (field, msg) in [("owner", "Invalid owner account"), ("actor", "Invalid actor account"), ("executor", "Invalid executor account")] {
                let mut bad = cfg.clone();
                match field {
                    "owner" => bad.owner = "Not An Account".into(),
                    "actor" => bad.recipes[1].actor = "UPPER.near".into(),
                    _ => bad.recipes[0].executor = "a..b".into(),
                }
                assert!(bad.validate().is_ok(), "the core accepts {field}");
                let out = new_with(&bad);
                let f = out.fail();
                assert_eq!(out.storage, empty);
                match b {
                    Build::Plain => assert_eq!(f.panic_msg, None),
                    Build::Near => assert!(
                        f.panic_msg.as_deref().unwrap_or("").contains(msg),
                        "{field}: {:?}",
                        f.panic_msg
                    ),
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// SH-3: on_result is private, and maps the executor's raw result
// ---------------------------------------------------------------------------

// PROP: SH-3
#[test]
fn sh3_on_result_is_private() {
    big_stack(|| {
        for b in Build::all() {
            let s = dispatched(Path::Wasm(b));
            let w = wasm(b, "pool_contract");
            for (pred, signer) in [(Z, Z), (X2, X2), (A2, A2), (OWNER, OWNER), (Z, POOL)] {
                for result in ["completed", "cancelled", "active"] {
                    let out = wasm_call(
                        w,
                        &Ctx::new(pred, signer, 102_000_000_000),
                        s.storage.clone(),
                        prs(&[Res::ok_str(result)]),
                        "on_result",
                        br#"{"id":0}"#,
                    );
                    assert_wasm_fail(
                        b,
                        &out,
                        &Fail::Guest("Method on_result is private".into()),
                        &s.storage,
                        &format!("on_result by {pred}"),
                    );
                    assert!(!out.called("input") && !out.called("promise_result"));
                }
            }
            // The pool itself (its own callback) may.
            let out = wasm_call(
                w,
                &Ctx::by(POOL, 102),
                s.storage.clone(),
                prs(&[Res::ok_str("active")]),
                "on_result",
                br#"{"id":0}"#,
            );
            assert!(out.result.is_ok());
            assert_eq!(pool_of(&out.storage).records[0].status, Status::Active);
        }
    });
}

// PROP: SH-3
#[test]
fn sh3_on_result_maps_the_status_string() {
    big_stack(|| {
        // (promise result, status after, outcome in the event)
        let table: &[(Option<&str>, Status)] = &[
            (Some("completed"), Status::Completed),
            (Some("cancelled"), Status::Cancelled),
            (Some("active"), Status::Active),
            (Some("unknown"), Status::Unknown),
            (Some("garbage"), Status::Unknown),
            (Some(""), Status::Unknown),
            (Some("Completed"), Status::Unknown),
            (Some(" completed"), Status::Unknown),
            (None, Status::Unknown), // a failed call (execute panicked)
        ];
        for path in paths() {
            for (result, after) in table {
                let mut s = dispatched(path);
                let v0 = s.pool.version;
                let r = s.ok(&Ctx::by(POOL, 102), Op::cb(0, *result));
                assert_eq!(s.status(0), *after, "{path:?} {result:?}");
                assert_eq!(
                    *r.event(),
                    json!({ "id": 0, "outcome": result.unwrap_or("unknown"), "error": null })
                );
                let p = pool_of(&s.storage);
                assert_eq!(p.version, v0 + 1);
                match after {
                    Status::Completed => assert_eq!(p.values, vec![-3]),
                    _ => assert_eq!(p.values, vec![0]),
                }
                assert!(r.calls.is_empty());
            }
            // Later results: idempotent and conflicting outcomes.
            let mut s = dispatched(path);
            s.ok(&Ctx::by(POOL, 102), Op::cb(0, Some("active")));
            let v = s.pool.version;
            // Active ignores a later unknown/active (no version bump: reconcile is
            // permissionless).
            for later in [Some("unknown"), Some("active"), None, Some("garbage")] {
                let r = s.ok(&Ctx::by(POOL, 103), Op::cb(0, later));
                assert_eq!(r.event()["error"], Value::Null);
                assert_eq!(s.status(0), Status::Active);
                assert_eq!(pool_of(&s.storage).version, v);
            }
            s.ok(&Ctx::by(POOL, 104), Op::cb(0, Some("completed")));
            assert_eq!(s.status(0), Status::Completed);
            let v = s.pool.version;
            s.ok(&Ctx::by(POOL, 104), Op::cb(0, Some("completed")));
            // The opposite terminal outcome is refused inside the callback: the
            // error goes to the event only, the call itself succeeds.
            let r = s.ok(&Ctx::by(POOL, 105), Op::cb(0, Some("cancelled")));
            assert_eq!(r.event()["error"], json!("InvalidState"));
            assert_eq!((s.status(0), pool_of(&s.storage).version), (Status::Completed, v));
            // Unknown -> Active -> settled through the callback.
            let mut s = dispatched(path);
            s.ok(&Ctx::by(POOL, 102), Op::cb(0, None));
            assert_eq!(s.status(0), Status::Unknown);
            s.ok(&Ctx::by(POOL, 103), Op::cb(0, None));
            s.ok(&Ctx::by(POOL, 103), Op::cb(0, Some("active")));
            assert_eq!(s.status(0), Status::Active);
            s.ok(&Ctx::by(POOL, 104), Op::cb(0, Some("cancelled")));
            assert_eq!(s.status(0), Status::Cancelled);
            // v1.1.0 (docs/api.md:20): a result for an id with no record
            // changes nothing and is logged with error NotFound; the callback
            // does not panic.
            for id in [1, 7, u32::MAX] {
                let before = s.storage.clone();
                let r = s.ok(&Ctx::by(POOL, 104), Op::cb(id, Some("completed")));
                assert_eq!(*r.event(), json!({ "id": id, "outcome": "completed", "error": "NotFound" }), "{path:?}");
                assert_eq!(s.storage, before);
            }
        }
    });
}

/// NT-F4 (fixed in v1.1.0): `on_result` reads its promise result itself. A
/// failed call, no result, two results, more than 64 bytes, or anything but
/// one JSON string is "unknown" (docs/api.md:40); a string other than the
/// three statuses is logged as itself and also acknowledges as Unknown. The
/// v1.0.0 witness (`sh3_non_string_promise_results_fail_the_callback`: the
/// wrapper could not deserialize the result and the callback failed) is
/// inverted here; the 16 cases are upstream's `nt_f4_non_string_result_
/// becomes_unknown`, on both paths.
// PROP: SH-3
#[test]
fn sh3_non_string_promise_results_become_unknown() {
    big_stack(|| {
        use Status::{Active, Cancelled, Completed, Unknown};
        let ok = |b: &[u8]| vec![Res::raw(b)];
        // "completed" as a JSON string padded with trailing whitespace to n bytes.
        let padded = |n: usize| {
            let mut s = b"\"completed\"".to_vec();
            s.resize(n, b' ');
            s
        };
        let long = format!("\"{}\"", "a".repeat(10_000));
        let cases: Vec<(&str, Vec<Res>, Status, &str)> = vec![
            ("number", ok(b"5"), Unknown, "unknown"),
            ("object", ok(br#"{"status":"active"}"#), Unknown, "unknown"),
            ("array", ok(br#"["active"]"#), Unknown, "unknown"),
            ("null", ok(b"null"), Unknown, "unknown"),
            ("empty", ok(b""), Unknown, "unknown"),
            ("unquoted", ok(b"completed"), Unknown, "unknown"),
            ("invalid UTF-8", ok(b"\"\xff\xfe\""), Unknown, "unknown"),
            ("10 kB string", ok(long.as_bytes()), Unknown, "unknown"),
            ("65 bytes", ok(&padded(65)), Unknown, "unknown"),
            ("failed", vec![Res::failed()], Unknown, "unknown"),
            ("no result", vec![], Unknown, "unknown"),
            ("two results", vec![Res::ok_str("active"), Res::ok_str("active")], Unknown, "unknown"),
            ("other string", ok(b"\"bogus\""), Unknown, "bogus"),
            ("active", ok(b"\"active\""), Active, "active"),
            ("64 bytes", ok(&padded(64)), Completed, "completed"),
            ("cancelled", ok(b"\"cancelled\""), Cancelled, "cancelled"),
        ];
        for path in paths() {
            for (name, results, status, outcome) in &cases {
                let mut s = dispatched(path);
                let r = s.ok(&Ctx::by(POOL, 102), Op::OnResult { id: 0, results: results.clone() });
                assert_eq!(s.status(0), *status, "{path:?} {name}");
                assert_eq!(*r.event(), json!({ "id": 0, "outcome": outcome, "error": null }), "{path:?} {name}");
                if let Path::Wasm(_) = path {
                    // The callback asks how many results there are first, and
                    // reads one only when there is exactly one.
                    assert!(r.host_calls.contains(&"promise_results_count"), "{name}");
                    assert_eq!(r.host_calls.contains(&"promise_result"), results.len() == 1, "{name}: {:?}", r.host_calls);
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// SH-4: submit returns the core's decision and id; a rejection writes nothing
// ---------------------------------------------------------------------------

fn cfg_with(metrics: Vec<Metric>, terms: Vec<Term>, recipes: Vec<Recipe>, max_age: u32) -> Config {
    Config {
        name: "case".into(),
        owner: OWNER.into(),
        metrics,
        terms,
        recipes,
        max_age_seconds: max_age,
    }
}
fn m(min: i64, max: i64, initial: i64) -> Metric {
    Metric { name: "m".into(), unit: "u".into(), initial, min, max }
}
fn t(metric: u32, target: i64, shape: Shape) -> Term {
    Term { metric, target, weight: 1, shape }
}

/// One rejection case: a configuration, an edit of the state, a request (by
/// `who` at `now`) and the reason the core gives.
struct Case {
    name: &'static str,
    cfg: Config,
    edit: fn(&mut Pool),
    who: &'static str,
    now: u64,
    req: fn(&Pool) -> Request,
    reason: &'static str,
}

fn rq(recipe: u32, units: u32, version: u32, expires_at: u64) -> Request {
    Request { recipe, units, version, expires_at }
}

/// 256 retained records (ids 0..256) of recipe 0, Completed except the
/// oldest, which has status `front`.
fn window(p: &mut Pool, front: Status) {
    p.records = (0..256)
        .map(|id| Commitment { id, recipe: 0, units: 1, expires_at: 150, status: Status::Completed })
        .collect();
    p.records[0].status = front;
}

fn rejection_cases() -> Vec<Case> {
    let mut prereq = open_config();
    prereq.recipes[1].requires = vec![(0, 2)];
    vec![
        Case { name: "paused", cfg: open_config(), edit: |p| p.paused = true, who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Paused" },
        Case { name: "stale", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 1, p.version + 1, 150), reason: "Stale" },
        Case { name: "expired-now", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 1, p.version, 101), reason: "Expired" },
        Case { name: "expired-horizon", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 1, p.version, 101 + 86_401), reason: "Expired" },
        Case { name: "snapshot", cfg: cfg_with(vec![m(-10, 10, 0)], vec![t(0, 0, Shape::Excess)], vec![recipe(A1, X1, vec![0], vec![0], 5)], 10), edit: |_| {}, who: A1, now: 111, req: |p| rq(0, 1, p.version, 150), reason: "SnapshotExpired" },
        // v1.1.0 window: a full window is refused only while its oldest
        // record is open (it cannot be pruned).
        Case { name: "full", cfg: open_config(), edit: |p| window(p, Status::Reserved), who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Full" },
        Case { name: "full-unknown-front", cfg: open_config(), edit: |p| window(p, Status::Unknown), who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Full" },
        Case { name: "recipe", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(2, 1, p.version, 150), reason: "InvalidRecipe" },
        Case { name: "unauthorized", cfg: open_config(), edit: |_| {}, who: A2, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Unauthorized" },
        Case { name: "units-0", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 0, p.version, 150), reason: "InvalidUnits" },
        Case { name: "units-max", cfg: open_config(), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 101, p.version, 150), reason: "InvalidUnits" },
        Case { name: "prerequisite", cfg: prereq, edit: |_| {}, who: A2, now: 101, req: |p| rq(1, 1, p.version, 150), reason: "Prerequisite" },
        Case { name: "arithmetic", cfg: cfg_with(vec![m(-1_000_000, 1_000_000, 999_999)], vec![t(0, 1_000_000, Shape::Excess)], vec![recipe(A1, X1, vec![1], vec![0], 5)], 0), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 2, p.version, 150), reason: "Arithmetic" },
        // v1.1.0: no new work from VERSION_CAP on (the last check).
        Case { name: "version-cap", cfg: open_config(), edit: |p| p.version = VERSION_CAP, who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Arithmetic" },
        Case { name: "bounds", cfg: cfg_with(vec![m(0, 10, 5)], vec![t(0, 0, Shape::Deficit)], vec![recipe(A1, X1, vec![6], vec![0], 5)], 0), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Bounds" },
        Case { name: "energy", cfg: cfg_with(vec![m(0, 10, 5)], vec![t(0, 5, Shape::Target)], vec![recipe(A1, X1, vec![1], vec![0], 5)], 0), edit: |_| {}, who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Energy" },
        // Disturbed state ([12,5] outside [0,10]): v1.1.0's envelope counts
        // the hedge's completion (+2 -> 12), which is no better than now
        // (F-1 fixed: refused at admission with Bounds; v1.0.0 said Energy).
        Case { name: "energy-hedge", cfg: cfg_with(vec![m(0, 10, 5), m(-100, 100, 5)], vec![t(1, 0, Shape::Target)], vec![recipe(A1, X1, vec![-2, 0], vec![2, 5], 1)], 0), edit: |p| p.values = vec![12, 5], who: A1, now: 101, req: |p| rq(0, 1, p.version, 150), reason: "Bounds" },
    ]
}

// PROP: SH-4
#[test]
fn sh4_rejected_submit_returns_the_decision_and_writes_nothing() {
    big_stack(|| {
        for path in paths() {
            for c in rejection_cases() {
                let mut s = Sim::new(path, c.cfg.clone(), 100);
                s.edit(c.edit);
                let req = (c.req)(&s.pool);
                let decision = s.pool.preview(c.who, &req, c.now);
                assert_eq!(decision.reason, c.reason, "case {} is not the intended rejection", c.name);
                let before = s.storage.clone();
                let r = s.ok(&Ctx::by(c.who, c.now), Op::Submit { req: req.clone() });
                // The exact core decision, no id, an event, and the same bytes.
                assert_eq!(r.value(), &json!({ "decision": decision_json(&decision), "id": null }), "{}", c.name);
                assert_eq!(r.event()["allowed"], json!(false));
                assert_eq!(r.event()["reason"], json!(c.reason));
                assert_eq!(r.event()["version"], json!(s.pool.version));
                assert_eq!(s.storage, before, "{path:?} {}: rejected submit changed state", c.name);
                if let Path::Wasm(_) = path {
                    // The &mut self wrapper still rewrites STATE (same bytes).
                    assert!(r.host_calls.contains(&"storage_write"));
                }
            }
            // An admitted submit returns the core's id and decision.
            let mut s = Sim::new(path, open_config(), 100);
            for i in 0..3u32 {
                let req = rq(i % 2, 1 + i, s.pool.version, 150);
                let who = if i % 2 == 0 { A1 } else { A2 };
                let d = s.pool.preview(who, &req, 101);
                let r = s.ok(&Ctx::by(who, 101), Op::Submit { req });
                assert_eq!(r.value(), &json!({ "decision": decision_json(&d), "id": i }));
            }
            // A full window whose oldest record is terminal prunes it: the
            // new record gets id 256 and the window starts at id 1.
            for front in [Status::Completed, Status::Cancelled] {
                let mut s = Sim::new(path, open_config(), 100);
                s.edit(|p| window(p, front.clone()));
                let r = s.ok(&Ctx::by(A1, 101), submit(0, 1, s.pool.version, 150));
                assert_eq!(r.value()["id"], json!(256), "{path:?}");
                let p = pool_of(&s.storage);
                assert_eq!((p.first_id, p.records.len(), p.records[0].id, p.records[255].id), (1, 256, 1, 256));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// SH-5: a core Err is a panic with the error string, and changes nothing
// ---------------------------------------------------------------------------

/// Two metrics; recipe 0 lowers metric 0 by 4 on reservation; recipe 1 would
/// raise it by 8 on completion. Releasing 0 while 1 is open would leave
/// [0, 10] (5 + 8 = 13): v1.0.0 admitted 1 and then refused the release
/// (F-1); v1.1.0 refuses 1 at admission.
fn dependent_config() -> Config {
    cfg_with(
        vec![m(0, 10, 5), m(-100, 100, 0)],
        vec![t(1, 0, Shape::Target)],
        vec![
            recipe(A1, X1, vec![-4, 0], vec![0, 0], 1),
            recipe(A2, X2, vec![0, 0], vec![8, 0], 1),
        ],
        10,
    )
}

// PROP: SH-5 LIVE-ALL
#[test]
fn sh5_core_errors_are_panics_with_the_error_string() {
    big_stack(|| {
        for path in paths() {
            // F-1 fixed: the record whose completion a release would push out
            // of bounds is refused up front, so the release never fails.
            let mut s = Sim::new(path, dependent_config(), 100);
            s.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 105));
            let r = s.ok(&Ctx::by(A2, 101), submit(1, 1, 1, 150));
            assert_eq!(r.value()["decision"]["reason"], json!("Bounds"), "{path:?}");
            s.ok(&Ctx::by(A1, 102), Op::Cancel { id: 0 });
            assert_eq!(pool_of(&s.storage).values, vec![5, 0]);

            // Every guard, as a Guest panic with the core's error string.
            let mut cfg = open_config();
            cfg.max_age_seconds = 10;
            let mut s = Sim::new(path, cfg, 100);
            s.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 105));
            s.ok(&Ctx::by(A2, 101), submit(1, 1, 1, 150));
            // dispatch
            s.err(&Ctx::by(A1, 101), Op::Dispatch { id: 9 }, guest(E::NotFound));
            s.err(&Ctx::by(A2, 101), Op::Dispatch { id: 0 }, guest(E::Unauthorized));
            s.ok(&Ctx::by(OWNER, 101), Op::Pause { paused: true });
            s.err(&Ctx::by(A1, 101), Op::Dispatch { id: 0 }, guest(E::Paused));
            s.ok(&Ctx::by(OWNER, 101), Op::Pause { paused: false });
            s.err(&Ctx::by(A1, 105), Op::Dispatch { id: 0 }, guest(E::Expired));
            s.err(&Ctx::by(A2, 111), Op::Dispatch { id: 1 }, guest(E::SnapshotExpired));
            // cancel
            s.err(&Ctx::by(A1, 102), Op::Cancel { id: 9 }, guest(E::NotFound));
            s.err(&Ctx::by(A2, 102), Op::Cancel { id: 0 }, guest(E::Unauthorized));
            s.ok(&Ctx::by(A2, 102), Op::Dispatch { id: 1 });
            s.err(&Ctx::by(A2, 102), Op::Dispatch { id: 1 }, guest(E::InvalidState));
            s.err(&Ctx::by(A2, 102), Op::Cancel { id: 1 }, guest(E::Pending));
            s.err(&Ctx::by(OWNER, 102), Op::Cancel { id: 1 }, guest(E::Pending));
            // settle
            s.err(&Ctx::by(X2, 102), Op::Settle { id: 9, completed: true }, guest(E::NotFound));
            s.err(&Ctx::by(A2, 102), Op::Settle { id: 1, completed: true }, guest(E::Unauthorized));
            s.err(&Ctx::by(X1, 102), Op::Settle { id: 1, completed: true }, guest(E::Unauthorized));
            s.err(&Ctx::by(X1, 102), Op::Settle { id: 0, completed: true }, guest(E::InvalidState));
            // pause / observe
            s.err(&Ctx::by(A1, 102), Op::Pause { paused: true }, guest(E::Unauthorized));
            s.err(&Ctx::by(Z, 102), Op::Observe { values: vec![5] }, guest(E::Unauthorized));
            s.err(&Ctx::by(OWNER, 102), Op::Observe { values: vec![5] }, guest(E::Pending));
            // v1.1.0: restating the current values is a refresh, allowed
            // while records are open; it renews the snapshot only.
            let before = pool_of(&s.storage);
            let r = s.ok(&Ctx::by(OWNER, 103), Op::Observe { values: before.values.clone() });
            assert_eq!(*r.event(), json!({ "version": before.version + 1, "values": before.values }));
            let after = pool_of(&s.storage);
            assert_eq!((after.observed_at, &after.records, &after.values), (103, &before.records, &before.values));
            s.ok(&Ctx::by(X2, 103), Op::Settle { id: 1, completed: true });
            // F-2b: cancelling a Completed record is conflicting evidence.
            s.err(&Ctx::by(A2, 103), Op::Cancel { id: 1 }, guest(E::InvalidState));
            s.err(&Ctx::by(OWNER, 103), Op::Cancel { id: 1 }, guest(E::InvalidState));
            s.ok(&Ctx::by(A1, 103), Op::Cancel { id: 0 });
            s.ok(&Ctx::by(OWNER, 103), Op::Cancel { id: 0 }); // idempotent
            s.err(&Ctx::by(OWNER, 104), Op::Observe { values: vec![5, 1] }, guest(E::Bounds));
            s.err(&Ctx::by(OWNER, 104), Op::Observe { values: vec![1_000_001] }, guest(E::Bounds));
            s.ok(&Ctx::by(OWNER, 104), Op::Observe { values: vec![5] });

            // VERSION_CAP: submit, dispatch, set_paused and observe stop
            // (Arithmetic); acknowledgment, settlement and cancellation use
            // the 3 * 256 reserve and go through.
            let mut s = Sim::new(path, open_config(), 100);
            s.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 150));
            s.ok(&Ctx::by(A1, 101), submit(0, 1, 1, 150));
            s.ok(&Ctx::by(A1, 101), Op::Dispatch { id: 1 });
            s.ok(&Ctx::by(A2, 101), submit(1, 1, 3, 150));
            s.ok(&Ctx::by(A2, 101), Op::Dispatch { id: 2 });
            s.edit(|p| p.version = VERSION_CAP);
            let r = s.ok(&Ctx::by(A1, 101), submit(0, 1, VERSION_CAP, 150));
            assert_eq!((&r.value()["decision"]["reason"], &r.value()["id"]), (&json!("Arithmetic"), &Value::Null));
            s.err(&Ctx::by(A1, 101), Op::Dispatch { id: 0 }, guest(E::Arithmetic));
            s.err(&Ctx::by(OWNER, 101), Op::Pause { paused: true }, guest(E::Arithmetic));
            s.err(&Ctx::by(OWNER, 101), Op::Observe { values: vec![0] }, guest(E::Arithmetic));
            let r = s.ok(&Ctx::by(POOL, 101), Op::cb(1, Some("active")));
            assert_eq!(r.event()["error"], Value::Null);
            s.ok(&Ctx::by(X1, 101), Op::Settle { id: 1, completed: true });
            s.ok(&Ctx::by(POOL, 101), Op::cb(2, None));
            s.ok(&Ctx::by(POOL, 101), Op::cb(2, Some("active")));
            s.ok(&Ctx::by(X2, 101), Op::Settle { id: 2, completed: false });
            s.ok(&Ctx::by(A1, 101), Op::Cancel { id: 0 });
            let p = pool_of(&s.storage);
            assert_eq!(p.version, VERSION_CAP + 6);
            assert!(p.records.iter().all(|c| matches!(c.status, Status::Completed | Status::Cancelled)));
            // set_paused toggles up to the cap, then stops; the open record
            // can still be cancelled.
            let mut q = Sim::new(path, open_config(), 100);
            q.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 150));
            q.ok(&Ctx::by(A1, 101), submit(0, 1, 1, 150));
            q.ok(&Ctx::by(A1, 101), Op::Dispatch { id: 1 });
            q.edit(|p| p.version = VERSION_CAP - 2);
            q.ok(&Ctx::by(OWNER, 101), Op::Pause { paused: true });
            q.ok(&Ctx::by(OWNER, 101), Op::Pause { paused: false });
            q.err(&Ctx::by(OWNER, 101), Op::Pause { paused: true }, guest(E::Arithmetic));
            q.ok(&Ctx::by(A1, 101), Op::Cancel { id: 0 });
            assert_eq!(pool_of(&q.storage).version, VERSION_CAP + 1);
            // Beyond the reserve (a crafted, unreachable version): the plain
            // increment fails too, as a panic for settle and in the event for
            // on_result, which never panics on a core error.
            q.edit(|p| p.version = u32::MAX);
            q.err(&Ctx::by(X1, 101), Op::Settle { id: 1, completed: true }, guest(E::Arithmetic));
            let r = q.ok(&Ctx::by(POOL, 101), Op::cb(1, Some("active")));
            assert_eq!(r.event()["error"], json!("Arithmetic"));
            assert_eq!(q.status(1), Status::Dispatched);

            // A submit the preview allows but the core's submit refuses: the
            // shell panics with the core's error and writes nothing, rather
            // than swallowing it. The state is crafted and unreachable: ids
            // are issued one per submit, below VERSION_CAP, so `first_id +
            // len` never overflows, and an allowed preview is otherwise
            // always a successful submit.
            let mut w = Sim::new(path, open_config(), 100);
            w.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 150));
            w.ok(&Ctx::by(A1, 101), Op::Cancel { id: 0 });
            w.edit(|p| {
                p.first_id = u32::MAX;
                p.records[0].id = u32::MAX;
            });
            let v = w.pool.version;
            assert!(w.pool.preview(A1, &rq(0, 1, v, 150), 101).allowed, "{path:?}");
            w.err(&Ctx::by(A1, 101), submit(0, 1, v, 150), guest(E::Arithmetic));
        }
    });
}

// ---------------------------------------------------------------------------
// SH-6: dispatch sends execute{id,recipe,units} to the recipe's executor,
// then on_result{id} to itself
// ---------------------------------------------------------------------------

// PROP: SH-6
#[test]
fn sh6_dispatch_promise_shape() {
    big_stack(|| {
        for path in paths() {
            let mut s = Sim::new(path, open_config(), 100);
            s.ok(&Ctx::by(A1, 100), submit(0, 7, 0, 150));
            s.ok(&Ctx::by(A2, 100), submit(1, 3, 1, 150));
            for (id, who, exec, recipe, units) in [(1u32, A2, X2, 1u32, 3u32), (0, A1, X1, 0, 7)] {
                let before = pool_of(&s.storage);
                let r = s.ok(&Ctx::by(who, 101), Op::Dispatch { id });
                assert_eq!(
                    r.calls,
                    vec![
                        fcall(exec, "execute", json!({ "id": id, "recipe": recipe, "units": units }), 30, vec![]),
                        fcall(POOL, "on_result", json!({ "id": id }), 30, vec![0]),
                    ],
                    "{path:?}"
                );
                assert!(r.events.is_empty(), "dispatch logs nothing");
                if let Path::Wasm(_) = path {
                    // NT-F3 fixed: env::promise_create then promise_then, and
                    // the callback (index 1) is the call's result; no batch
                    // actions and no value_return.
                    assert_eq!(r.promise_return, Some(1));
                    for h in ["promise_create", "promise_then", "promise_return"] {
                        assert!(r.host_calls.contains(&h), "{h}: {:?}", r.host_calls);
                    }
                    assert!(!r.host_calls.iter().any(|h| h.starts_with("promise_batch") || *h == "value_return"), "{:?}", r.host_calls);
                }
                let after = pool_of(&s.storage);
                assert_eq!(after.version, before.version + 1);
                assert_eq!(after.values, before.values);
                assert_eq!(after.record(id).unwrap().status, Status::Dispatched);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// SH-7: reconcile: anyone, no deposit, only for in-flight records, status then
// on_result
// ---------------------------------------------------------------------------

// PROP: SH-7 SH-10
#[test]
fn sh7_reconcile_guard_and_promise_shape() {
    big_stack(|| {
        let deposit = || Fail::Guest("Method reconcile doesn't accept deposit".into());
        for path in paths() {
            let mut s = Sim::new(path, open_config(), 100);
            // v1.1.0: a missing id is the core's NotFound through `must`.
            s.err(&Ctx::by(Z, 100), Op::Reconcile { id: 0 }, guest(E::NotFound));
            // The deposit check comes first, whatever the id.
            s.err(&Ctx::by(Z, 100).with_deposit(1), Op::Reconcile { id: 0 }, deposit());
            s.ok(&Ctx::by(A2, 100), submit(1, 2, 0, 150));
            // Reserved: not in flight.
            s.err(&Ctx::by(Z, 100), Op::Reconcile { id: 0 }, Fail::Rust("InvalidState".into()));
            s.err(&Ctx::by(Z, 100).with_deposit(1), Op::Reconcile { id: 0 }, deposit());
            s.ok(&Ctx::by(A2, 101), Op::Dispatch { id: 0 });
            let check = |s: &mut Sim, who: &str| {
                let before = s.storage.clone();
                let r = s.ok(&Ctx::new(who, Z, 102_000_000_000), Op::Reconcile { id: 0 });
                assert_eq!(
                    r.calls,
                    vec![
                        fcall(X2, "status", json!({ "id": 0 }), 10, vec![]),
                        fcall(POOL, "on_result", json!({ "id": 0 }), 30, vec![0]),
                    ]
                );
                assert!(r.events.is_empty());
                assert_eq!(s.storage, before, "reconcile changed state");
                if let Path::Wasm(_) = path {
                    assert!(!r.host_calls.contains(&"storage_write"), "reconcile (&self) wrote");
                    assert!(r.host_calls.contains(&"attached_deposit"), "no deposit check");
                    assert_eq!(r.promise_return, Some(1));
                }
                // With a deposit the same call is refused and sends nothing.
                s.err(&Ctx::new(who, Z, 102_000_000_000).with_deposit(10u128.pow(24)), Op::Reconcile { id: 0 }, deposit());
            };
            for who in [Z, A1, X2, OWNER, RELAY] {
                check(&mut s, who); // Dispatched
            }
            s.ok(&Ctx::by(POOL, 102), Op::cb(0, None));
            check(&mut s, Z); // Unknown
            s.ok(&Ctx::by(POOL, 102), Op::cb(0, Some("active")));
            check(&mut s, Z); // Active
            s.ok(&Ctx::by(X2, 103), Op::Settle { id: 0, completed: false });
            s.err(&Ctx::by(Z, 103), Op::Reconcile { id: 0 }, Fail::Rust("InvalidState".into()));
            s.ok(&Ctx::by(A2, 103), submit(1, 2, s.pool.version, 150));
            s.ok(&Ctx::by(A2, 103), Op::Dispatch { id: 1 });
            s.ok(&Ctx::by(X2, 103), Op::Settle { id: 1, completed: true });
            s.err(&Ctx::by(Z, 103), Op::Reconcile { id: 1 }, Fail::Rust("InvalidState".into()));
            s.err(&Ctx::by(Z, 103), Op::Reconcile { id: 2 }, guest(E::NotFound));
        }
    });
}

// ---------------------------------------------------------------------------
// SH-9: views
// ---------------------------------------------------------------------------

/// Every `get_commitments(from, limit)` in `froms x limits`, on `path`, equals
/// the core's page by id (docs/api.md:11), and `get_pool` and `preview` equal
/// the core's reads.
fn check_pages(s: &Sim, froms: &[u32], limits: &[u32], what: &str) {
    match s.path {
        Path::Host => {
            let mut rng = Rng::new(9);
            check_views(&s.storage, &s.pool, &mut rng, 101, what);
            for &from in froms {
                for &limit in limits {
                    let got = view(&s.storage, 101, |c| c.get_commitments(from, limit));
                    assert_eq!(got, expected_page(&s.pool, from, limit), "{what} ({from}, {limit})");
                    assert!(got.len() <= 50);
                }
            }
        }
        Path::Wasm(b) => {
            let w = wasm(b, "pool_contract");
            let v = |method: &str, args: Value| {
                let out = wasm_call(w, &Ctx::by(Z, 101), s.storage.clone(), vec![], method, &serde_json::to_vec(&args).unwrap());
                assert!(out.logs.is_empty() && out.receipts.is_empty());
                assert!(!out.called("storage_write"), "view {method} wrote");
                assert_eq!(out.storage, s.storage);
                out.json()
            };
            assert_eq!(v("get_pool", json!({})), serde_json::to_value(&s.pool).unwrap());
            for &from in froms {
                for &limit in limits {
                    let want: Vec<Value> = expected_page(&s.pool, from, limit).iter().map(commitment_json).collect();
                    assert_eq!(v("get_commitments", json!({ "from": from, "limit": limit })), json!(want), "{what} ({from}, {limit})");
                }
            }
            for (actor, req) in [(A1, rq(0, 1, s.pool.version, 150)), (A2, rq(1, 100, 3, 150)), (Z, rq(5, 1, 0, 0))] {
                let d = s.pool.preview(actor, &req, 101);
                assert_eq!(v("preview", json!({ "actor": actor, "request": request_json(&req) })), decision_json(&d));
            }
        }
    }
}

/// 300 submits through a full window: the 44 oldest records are terminal
/// and pruned (first_id 44); the oldest retained one is open, so the next
/// submit is refused Full. Pages are by id, around and past the window.
// PROP: SH-9 RD-PREVIEW
#[test]
fn sh9_views_and_paging_by_id_through_the_window() {
    big_stack(|| {
        for path in paths() {
            let mut s = Sim::new(path, open_config(), 100);
            for i in 0..300u32 {
                // Recipe 1 completes at -1 per unit: one unit keeps the
                // envelope inside [-1000, 1000].
                let (who, exec, recipe, units) = if i % 3 == 0 { (A2, X2, 1, 1) } else { (A1, X1, 0, 1 + i % 100) };
                let r = s.ok(&Ctx::by(who, 101), submit(recipe, units, s.pool.version, 150));
                assert_eq!(r.value()["id"], json!(i));
                if i < 44 || (i > 44 && i % 7 == 0) {
                    // Terminal: completed, cancelled before or after dispatch.
                    match i % 3 {
                        0 => {
                            s.ok(&Ctx::by(who, 101), Op::Dispatch { id: i });
                            s.ok(&Ctx::by(exec, 101), Op::Settle { id: i, completed: true });
                        }
                        1 => {
                            s.ok(&Ctx::by(who, 101), Op::Cancel { id: i });
                        }
                        _ => {
                            s.ok(&Ctx::by(who, 101), Op::Dispatch { id: i });
                            s.ok(&Ctx::by(exec, 101), Op::Settle { id: i, completed: false });
                        }
                    }
                } else if i % 5 == 0 {
                    s.ok(&Ctx::by(who, 101), Op::Dispatch { id: i });
                }
            }
            let p = pool_of(&s.storage);
            assert_eq!((p.first_id, p.records.len()), (44, 256), "{path:?}");
            let r = s.ok(&Ctx::by(A1, 101), submit(0, 1, s.pool.version, 150));
            assert_eq!(r.value()["decision"]["reason"], json!("Full"));
            let froms = [0u32, 1, 43, 44, 45, 49, 50, 51, 93, 94, 95, 100, 200, 249, 250, 251, 299, 300, 301, u32::MAX];
            let limits = [0u32, 1, 49, 50, 51, 100, u32::MAX];
            check_pages(&s, &froms, &limits, &format!("{path:?} 300 records"));
        }
    });
}

/// F-3 (fixed in v1.1.0): ids stay stable across a prune. Upstream's
/// `f3_commitments_page_by_id_after_a_prune` and `f3_reconcile_of_a_pruned_
/// id_is_not_found` through both paths, plus every other id-taking method:
/// a pruned id is NotFound, and a result for it is logged.
// PROP: SH-9 SH-3 SH-5 SH-6 SH-7
#[test]
fn sh9_pruned_ids_are_not_found_and_pages_are_by_id() {
    big_stack(|| {
        for path in paths() {
            let mut s = pruned(path);
            let ids = |s: &Sim, from: u32, limit: u32| -> Vec<u32> { expected_page(&s.pool, from, limit).iter().map(|c| c.id).collect() };
            assert_eq!(ids(&s, 0, 3), vec![1, 2, 3]);
            assert_eq!(ids(&s, 1, 3), vec![1, 2, 3]);
            assert_eq!(ids(&s, 255, 50), vec![255, 256]);
            assert_eq!(ids(&s, 257, 5), Vec::<u32>::new());
            assert_eq!(ids(&s, 100, 100), (100..150).collect::<Vec<u32>>());
            check_pages(&s, &[0, 1, 2, 255, 256, 257, 100, u32::MAX], &[0, 3, 5, 50, 100], &format!("{path:?} pruned"));
            let before = s.storage.clone();
            let r = s.ok(&Ctx::by(POOL, 102), Op::cb(0, Some("completed")));
            assert_eq!(*r.event(), json!({ "id": 0, "outcome": "completed", "error": "NotFound" }));
            assert_eq!(s.storage, before);
            s.err(&Ctx::by(Z, 102), Op::Reconcile { id: 0 }, guest(E::NotFound));
            s.err(&Ctx::by(A1, 102), Op::Dispatch { id: 0 }, guest(E::NotFound));
            s.err(&Ctx::by(X1, 102), Op::Settle { id: 0, completed: false }, guest(E::NotFound));
            s.err(&Ctx::by(A1, 102), Op::Cancel { id: 0 }, guest(E::NotFound));
            // The retained ids still work: 1 is the oldest.
            s.ok(&Ctx::by(A1, 102), Op::Cancel { id: 1 });
        }
    });
}

// ---------------------------------------------------------------------------
// SH-10: nonpayable methods reject an attached deposit (wasm wrappers)
// ---------------------------------------------------------------------------

// PROP: SH-10
#[test]
fn sh10_mutating_methods_reject_attached_deposits() {
    big_stack(|| {
        for b in Build::all() {
            // Record 0 dispatched, record 1 reserved: every call below would
            // succeed without the deposit.
            let mut s = dispatched(Path::Wasm(b));
            s.ok(&Ctx::by(A2, 101), submit(1, 1, s.pool.version, 150));
            let fresh = Sim::new(Path::Wasm(b), open_config(), 100).storage;
            let w = wasm(b, "pool_contract");
            // (method, caller, args, promise results, storage, check in the wrapper)
            let cases: Vec<(&str, &str, Value, Vec<Res>, Storage, bool)> = vec![
                ("new", POOL, json!({ "config": open_config() }), vec![], Storage::new(), true),
                ("submit", A1, json!({ "request": request_json(&rq(0, 1, s.pool.version, 150)) }), vec![], s.storage.clone(), true),
                ("dispatch", A2, json!({ "id": 1 }), vec![], s.storage.clone(), true),
                ("settle", X2, json!({ "id": 0, "completed": true }), vec![], s.storage.clone(), true),
                ("cancel", OWNER, json!({ "id": 1 }), vec![], s.storage.clone(), true),
                ("set_paused", OWNER, json!({ "paused": true }), vec![], s.storage.clone(), true),
                ("observe", OWNER, json!({ "values": [0] }), vec![], fresh, true),
                ("on_result", POOL, json!({ "id": 0 }), vec![Res::ok_str("active")], s.storage.clone(), true),
                // NT-F2 fixed: reconcile is &self, so near-sdk generates no
                // check; the shell's own `require!` is the first statement of
                // its body (after the arguments are read).
                ("reconcile", Z, json!({ "id": 0 }), vec![], s.storage.clone(), false),
            ];
            for (method, who, args, results, storage, wrapper) in cases {
                for yocto in [1u128, 10u128.pow(24)] {
                    let out = wasm_call(
                        w,
                        &Ctx::by(who, 102).with_deposit(yocto),
                        storage.clone(),
                        prs(&results),
                        method,
                        &serde_json::to_vec(&args).unwrap(),
                    );
                    assert_wasm_fail(
                        b,
                        &out,
                        &Fail::Guest(format!("Method {method} doesn't accept deposit")),
                        &storage,
                        method,
                    );
                    assert!(out.called("attached_deposit"), "{method}: {:?}", out.host_calls);
                    assert_eq!(out.called("input"), !wrapper, "{method}: {:?}", out.host_calls);
                }
                // Without a deposit the same call goes through.
                let out = wasm_call(w, &Ctx::by(who, 102), storage.clone(), prs(&results), method, &serde_json::to_vec(&args).unwrap());
                assert!(out.result.is_ok(), "{method} without deposit: {:?}", out.result);
            }
        }
    });
}

/// NT-F2 (fixed in v1.1.0): v1.0.0's `reconcile` was a `&self` method with no
/// deposit check, so it accepted and kept any attached NEAR although
/// docs/api.md says "All mutating methods are nonpayable". v1.1.0 adds the
/// check (docs/api.md:5). The v1.0.0 witness (`nt_f2_reconcile_accepts_and_
/// keeps_an_attached_deposit`) is inverted: on both paths, a deposit is
/// refused with near-sdk's message and nothing is sent. Views stay payable-
/// agnostic (near-sdk checks no deposit on them).
// PROP: SH-10
#[test]
fn nt_f2_reconcile_rejects_an_attached_deposit() {
    big_stack(|| {
        for path in paths() {
            let mut s = dispatched(path);
            for yocto in [1u128, 10u128.pow(24)] {
                s.err(
                    &Ctx::by(Z, 102).with_deposit(yocto),
                    Op::Reconcile { id: 0 },
                    Fail::Guest("Method reconcile doesn't accept deposit".into()),
                );
            }
            let r = s.ok(&Ctx::by(Z, 102), Op::Reconcile { id: 0 });
            assert_eq!(r.calls.len(), 2);
            assert!(r.calls.iter().all(|c| c.deposit == 0), "nothing is forwarded");
            if let Path::Wasm(b) = path {
                let w = wasm(b, "pool_contract");
                for (method, args) in [("get_pool", json!({})), ("get_commitments", json!({"from":0,"limit":1}))] {
                    let out = wasm_call(w, &Ctx::by(Z, 102).with_deposit(1), s.storage.clone(), vec![], method, &serde_json::to_vec(&args).unwrap());
                    assert!(out.result.is_ok(), "{method}");
                }
            }
        }
    });
}

/// NT-F1 (fixed in v1.1.0): v1.0.0's documented build was plain `cargo
/// build`, near-sdk 5.29.1's "pure-Rust fallback, not for on-chain use":
/// `panic_str` was `eprintln!` + `panic!()` instead of the `panic_utf8` host
/// call, so every failure was a bare `unreachable` trap with no message.
/// v1.1.0's build.sh sets `--cfg near` and refuses a wasm that does not
/// import `panic_utf8` (SURF-3(b)(i)). The v1.0.0 witness
/// (`nt_f1_upstream_build_drops_every_panic_message`) is inverted: the
/// documented build reports the error string for a core error, a Rust
/// panic, `#[private]`, the deposit and init checks and a bad argument. The
/// no-cfg build stays the negative control: it still drops them.
// PROP: SH-5
#[test]
fn nt_f1_documented_build_reports_every_panic_message() {
    big_stack(|| {
        for name in ["pool_contract", "pool_executor"] {
            assert!(wasm(Build::Near, name).imports.iter().any(|i| i == "env.panic_utf8"), "{name}");
            assert!(!wasm(Build::Plain, name).imports.iter().any(|i| i == "env.panic_utf8"), "{name}: control");
        }
        let s = dispatched(Path::Wasm(Build::Near));
        let w = wasm(Build::Near, "pool_contract");
        let call = |ctx: &Ctx, method: &str, args: &[u8], results: &[Res]| wasm_call(w, ctx, s.storage.clone(), prs(results), method, args);
        let cases: Vec<(&str, WasmOutcome)> = vec![
            ("Unauthorized", call(&Ctx::by(Z, 102), "observe", br#"{"values":[1]}"#, &[])),
            ("NotFound", call(&Ctx::by(Z, 102), "reconcile", br#"{"id":9}"#, &[])),
            ("Method on_result is private", call(&Ctx::by(Z, 102), "on_result", br#"{"id":0}"#, &[Res::ok_str("active")])),
            ("Method settle doesn't accept deposit", call(&Ctx::by(X2, 102).with_deposit(1), "settle", br#"{"id":0,"completed":true}"#, &[])),
            ("The contract has already been initialized", call(&Ctx::by(POOL, 102), "new", &serde_json::to_vec(&json!({ "config": open_config() })).unwrap(), &[])),
        ];
        for (msg, out) in &cases {
            assert_eq!(out.fail().panic_msg.as_deref(), Some(*msg));
            assert!(out.called("panic_utf8"));
        }
        // A Rust panic (reconcile's status assert) goes through the panic hook.
        let mut s2 = Sim::new(Path::Wasm(Build::Near), open_config(), 100);
        s2.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 150));
        let out = wasm_call(w, &Ctx::by(Z, 102), s2.storage.clone(), vec![], "reconcile", br#"{"id":0}"#);
        assert_wasm_fail(Build::Near, &out, &Fail::Rust("InvalidState".into()), &s2.storage, "reconcile Reserved");
        // A bad argument is a deserialization error with its message.
        let out = call(&Ctx::by(A2, 102), "dispatch", br#"{"id":"zero"}"#, &[]);
        assert!(out.fail().panic_msg.as_deref().unwrap_or("").starts_with("Failed to deserialize input from JSON"), "{:?}", out.fail());
        // The negative control: the no-cfg build of the same source traps
        // with no message.
        let s = dispatched(Path::Wasm(Build::Plain));
        let out = wasm_step(Build::Plain, &Ctx::by(Z, 101), s.storage.clone(), &Op::Observe { values: vec![1] });
        let f = out.fail();
        assert_eq!(f.panic_msg, None);
        assert_eq!(f.trap, "wasm `unreachable` instruction executed");
        assert!(!out.called("panic_utf8"));
    });
}
