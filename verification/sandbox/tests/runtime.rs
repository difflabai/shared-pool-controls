//! Runtime tests on the real wasm (pool_contract.wasm and pool_executor.wasm
//! built from the pin by upstream scripts/build.sh, via
//! scripts/build-wasm.sh) in a local near-sandbox: RT-PRED, RT-PRIVATE,
//! RT-CALLBACK, RT-REVERT, RT-RACE, RT-WEDGE, RT-WINDOW, RT-D1, and SB-1
//! (inverted at v1.1.0: the documented build reports every panic message).
//!
//! Every test keeps a host-side `pool_core::Pool` (the real core) in step
//! with the chain and compares the whole `get_pool` state after each step,
//! so a divergence between the wasm and the core fails the test. Every
//! failure must carry `Smart contract panicked: <msg>` (`expect_panic` in
//! common/). RT-WINDOW and RT-D1 port upstream
//! crates/sandbox/tests/integration.rs's window and D-1 tests and also check
//! EX-7 (`Ex7` in common/) after every transaction.
// `ok()` asserts success itself; most callers drop the outcome.
#![allow(unused_must_use)]
mod common;

use common::*;
use pool_core::{Error, Pool, Request, Status, VERSION_CAP};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

async fn same(env: &Env, reference: &Pool, what: &str) {
    assert_eq!(&env.pool().await, reference, "{what}: chain state differs from the core");
}

fn forward(env: &Env, method: &str, args: Value) -> Value {
    json!({ "receiver": env.lock.id(), "method": method, "args": args.to_string() })
}

// ------------------------------------------------------------------ RT-PRED

/// PROP: RT-PRED. A call forwarded by a third-party contract is attributed to
/// that contract (the predecessor), never to the account that signed it.
#[tokio::test]
async fn rt_pred_relay_is_the_caller() {
    let env = Env::bare().await;
    let relay = deploy(&env.worker, "relay", 10, &relay_wasm()).await;
    let cfg = basic_config(
        env.owner.id(),
        &[(env.actor.id(), env.exec.id()), (relay.id(), env.exec.id())],
    );
    let env = env.init(&cfg).await;
    let mut reference = env.pool().await;
    let mut rows = vec![];

    // 1. The actor signs a forward of its own, otherwise valid, submit.
    let req = env.request(0, 4).await;
    let raw = env.raw().await;
    let r = ok(&env.actor, relay.id(), "forward", forward(&env, "submit", json!({ "request": req }))).await;
    let v: Value = r.clone().json().unwrap();
    let expected = reference.preview(relay.id().as_str(), &req, env.now().await);
    assert_eq!(v["decision"], serde_json::to_value(&expected).unwrap());
    assert_eq!(v["decision"]["allowed"], false);
    assert_eq!(v["decision"]["reason"], "Unauthorized");
    assert_eq!(v["id"], Value::Null);
    let ev = event(&r, env.lock.id(), "decision");
    assert_eq!(ev["actor"], relay.id().as_str(), "the event names the relay as actor");
    assert_eq!(env.raw().await, raw, "a rejected submit writes nothing");
    rows.push(json!({"case": "actor signs relay.forward(submit recipe 0)", "attributed_to": ev["actor"], "allowed": false, "reason": v["decision"]["reason"], "storage_unchanged": true}));
    // Control: the same request, sent by the actor itself, is accepted.
    let (_, v) = env.submit(&env.actor, &req).await;
    assert_eq!(v["decision"]["allowed"], true);
    let id0 = reference.submit(env.actor.id().as_str(), req, env.now().await).unwrap();
    same(&env, &reference, "direct submit").await;
    rows.push(json!({"case": "actor submits the same request directly", "allowed": true, "id": id0}));

    // 2. Recipe 1's actor is the relay: a forward signed by an unrelated
    // account is the relay's submit and is accepted; the signer plays no part.
    let req = env.request(1, 2).await;
    let r = ok(&env.mallory, relay.id(), "forward", forward(&env, "submit", json!({ "request": req }))).await;
    let v: Value = r.clone().json().unwrap();
    assert_eq!(v["decision"]["allowed"], true, "{v}");
    assert_eq!(event(&r, env.lock.id(), "decision")["actor"], relay.id().as_str());
    let id1 = reference.submit(relay.id().as_str(), req, env.now().await).unwrap();
    assert_eq!(v["id"], id1);
    same(&env, &reference, "relay submit").await;
    rows.push(json!({"case": "mallory signs relay.forward(submit recipe 1); the relay is recipe 1's actor", "attributed_to": relay.id(), "allowed": true}));
    let req = env.request(1, 2).await;
    let (_, v) = env.submit(&env.mallory, &req).await;
    assert_eq!(v["decision"]["reason"], "Unauthorized");
    same(&env, &reference, "mallory direct").await;
    rows.push(json!({"case": "mallory submits recipe 1 directly", "allowed": false, "reason": "Unauthorized"}));

    // 3. Mutating calls forwarded by the relay are the relay's, whoever signs.
    let raw = env.raw().await;
    for (signer, method, args) in [
        (&env.actor, "dispatch", json!({ "id": id0 })),
        (&env.actor, "cancel", json!({ "id": id0 })),
        (env.exec.as_account(), "settle", json!({ "id": id1, "completed": true })),
        (&env.owner, "set_paused", json!({ "paused": true })),
        (&env.owner, "observe", json!({ "values": [50, 0] })),
    ] {
        let mut p = reference.clone();
        let core = match method {
            "dispatch" => p.dispatch(relay.id().as_str(), id0, env.now().await),
            "cancel" => p.cancel(relay.id().as_str(), id0, env.now().await),
            "settle" => p.settle(relay.id().as_str(), id1, true),
            "set_paused" => p.set_paused(relay.id().as_str(), true),
            _ => p.observe(relay.id().as_str(), vec![50, 0], env.now().await),
        };
        assert_eq!(core, Err(Error::Unauthorized));
        let observed = fails(signer, relay.id(), "forward", forward(&env, method, args), "Unauthorized").await;
        assert_eq!(env.raw().await, raw, "{method} via relay changed state");
        rows.push(json!({"case": format!("{} signs relay.forward({method})", signer.id()), "core": "Unauthorized", "observed_panic": observed, "storage_unchanged": true}));
    }
    // The relay may dispatch its own record, whoever asks it to.
    ok(&env.mallory, relay.id(), "forward", forward(&env, "dispatch", json!({ "id": id1 }))).await;
    reference.dispatch(relay.id().as_str(), id1, env.now().await).unwrap();
    reference.acknowledge(id1, true).unwrap();
    same(&env, &reference, "relay dispatch").await;
    rows.push(json!({"case": "mallory signs relay.forward(dispatch id1)", "result": "Ok, Active"}));
    record("RT-PRED", json!({ "property": "RT-PRED", "assumption": "ASM-PRED", "result": "pass", "cases": rows }));
}

// --------------------------------------------------------------- RT-PRIVATE

/// PROP: RT-PRIVATE EX-8. `new` (pool and executor), the pool's `on_result`
/// and the executor's `on_settled` reject every predecessor other than the
/// contract's own account; `new` runs once. v1.1.0 (NT-F4): `on_result` by
/// the pool's own key is Ok and reads "unknown" (a transaction carries no
/// promise result), which leaves an Unknown or Active record as it is and
/// logs NotFound for a missing id. `on_settled` by the executor's own key
/// fails "not a settle callback" and changes nothing.
#[tokio::test]
async fn rt_private_new_and_on_result() {
    let env = Env::bare().await;
    let relay = deploy(&env.worker, "relay", 10, &relay_wasm()).await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let new_args = json!({ "config": cfg });
    let mut rows = vec![];
    for a in [&env.owner, &env.actor, &env.mallory, env.exec.as_account()] {
        let o = fails(a, env.lock.id(), "new", new_args.clone(), "Method new is private").await;
        rows.push(json!({"call": "pool.new", "predecessor": a.id(), "expected": "Method new is private", "observed_panic": o}));
    }
    // Signed by the pool's own key but forwarded: the predecessor is the relay.
    let o = fails(
        env.lock.as_account(),
        relay.id(),
        "forward",
        json!({ "receiver": env.lock.id(), "method": "new", "args": new_args.to_string() }),
        "Method new is private",
    )
    .await;
    rows.push(json!({"call": "pool.new via relay, signed by the pool's key", "predecessor": relay.id(), "expected": "Method new is private", "observed_panic": o}));
    assert!(env.raw().await.is_empty(), "a rejected new left state");
    // A view's failure is a query error, not an execution outcome.
    let e = format!("{:?}", env.lock.view("get_pool").await.unwrap_err());
    assert!(e.contains("The contract is not initialized"), "{e}");
    rows.push(json!({"call": "pool.get_pool before new (view)", "observed_error": "The contract is not initialized"}));
    let o = fails(
        &env.owner,
        env.exec.id(),
        "new",
        json!({ "lock": env.lock.id(), "operator": env.mallory.id() }),
        "Method new is private",
    )
    .await;
    assert!(raw_state(&env.worker, env.exec.id()).await.is_empty());
    rows.push(json!({"call": "exec.new", "predecessor": env.owner.id(), "expected": "Method new is private", "observed_panic": o}));

    let env = env.init(&cfg).await;
    let raw = env.raw().await;
    let raw_exec = raw_state(&env.worker, env.exec.id()).await;
    let mut cfg2 = cfg.clone();
    cfg2.owner = env.mallory.id().to_string();
    let o = fails(env.lock.as_account(), env.lock.id(), "new", json!({ "config": cfg2 }), "The contract has already been initialized").await;
    fails(
        env.exec.as_account(),
        env.exec.id(),
        "new",
        json!({ "lock": env.lock.id(), "operator": env.mallory.id() }),
        "The contract has already been initialized",
    )
    .await;
    assert_eq!(env.raw().await, raw);
    assert_eq!(raw_state(&env.worker, env.exec.id()).await, raw_exec);
    rows.push(json!({"call": "pool.new and exec.new again, by their own accounts", "expected": "The contract has already been initialized", "observed_panic": o, "storage_unchanged": true}));

    // An Unknown record: a forged callback would move it (to Active, or to
    // Cancelled with the reservation released) if it got through.
    let mut reference = env.pool().await;
    env.mode("unknown").await;
    let id = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
    reference.dispatch(env.actor.id().as_str(), id, env.now().await).unwrap();
    reference.acknowledge(id, false).unwrap();
    same(&env, &reference, "dispatch (unknown)").await;
    let raw = env.raw().await;
    for a in [&env.owner, &env.actor, &env.mallory, env.exec.as_account()] {
        let o = fails(a, env.lock.id(), "on_result", json!({ "id": id }), "Method on_result is private").await;
        assert_eq!(env.raw().await, raw);
        rows.push(json!({"call": "pool.on_result", "predecessor": a.id(), "expected": "Method on_result is private", "observed_panic": o, "storage_unchanged": true}));
    }
    let o = fails(&env.actor, relay.id(), "forward", forward(&env, "on_result", json!({ "id": id })), "Method on_result is private").await;
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "pool.on_result via relay", "predecessor": relay.id(), "expected": "Method on_result is private", "observed_panic": o, "storage_unchanged": true}));
    // The pool's own key passes the private check. A transaction carries no
    // promise result, so v1.1.0's on_result reads "unknown" (NT-F4) and runs
    // acknowledge(id, false): a no-op on this Unknown record, version
    // included. Only the deployment account's key can do this (ASM-PRIVATE).
    let r = ok(env.lock.as_account(), env.lock.id(), "on_result", json!({ "id": id })).await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": id, "outcome": "unknown", "error": null }));
    let mut p = reference.clone();
    p.acknowledge(id, false).unwrap();
    assert_eq!(p, reference, "core: acknowledge(false) on Unknown is a no-op");
    assert_eq!(env.raw().await, raw, "own-key on_result on an Unknown record changed storage");
    same(&env, &reference, "after forged callbacks").await;
    rows.push(json!({"call": "pool.on_result signed by the pool's own key (no promise result), Unknown record", "predecessor": env.lock.id(), "result": "Ok", "event": ev, "storage_unchanged": true}));
    // An Active record: acknowledge(false) ignores it, so the same call is a
    // no-op there too.
    env.mode("normal").await;
    let active = env.submit_ok(&mut reference, &env.actor, 0, 1).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": active })).await;
    reference.dispatch(env.actor.id().as_str(), active, env.now().await).unwrap();
    reference.acknowledge(active, true).unwrap();
    same(&env, &reference, "dispatch (normal)").await;
    let raw = env.raw().await;
    let r = ok(env.lock.as_account(), env.lock.id(), "on_result", json!({ "id": active })).await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": active, "outcome": "unknown", "error": null }));
    assert_eq!(env.raw().await, raw, "own-key on_result on an Active record changed storage");
    assert_eq!(env.status(active).await, Status::Active);
    rows.push(json!({"call": "pool.on_result signed by the pool's own key, Active record", "predecessor": env.lock.id(), "result": "Ok", "event": ev, "status": "Active", "storage_unchanged": true}));
    // A missing id: logged with error NotFound, nothing written.
    let r = ok(env.lock.as_account(), env.lock.id(), "on_result", json!({ "id": 999 })).await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": 999, "outcome": "unknown", "error": "NotFound" }));
    assert_eq!(env.raw().await, raw);
    same(&env, &reference, "after own-key callbacks").await;
    rows.push(json!({"call": "pool.on_result signed by the pool's own key, id 999 (not issued)", "result": "Ok", "event": ev, "storage_unchanged": true}));

    // The executor's on_settled (EX-8): private, and it needs the settle's
    // promise result. An id of 600 would forget every job if it went through.
    let raw_exec = env.raw_exec().await;
    let jobs_before = env.jobs().await;
    assert_eq!(jobs_before.len(), 2, "{jobs_before:?}");
    for a in [&env.owner, &env.actor, &env.mallory, env.lock.as_account()] {
        let o = fails(a, env.exec.id(), "on_settled", json!({ "id": 600 }), "Method on_settled is private").await;
        assert_eq!(env.raw_exec().await, raw_exec);
        rows.push(json!({"call": "exec.on_settled({id: 600})", "predecessor": a.id(), "expected": "Method on_settled is private", "observed_panic": o, "executor_storage_unchanged": true}));
    }
    let o = fails(&env.actor, relay.id(), "forward", json!({ "receiver": env.exec.id(), "method": "on_settled", "args": json!({ "id": 600 }).to_string() }), "Method on_settled is private").await;
    assert_eq!(env.raw_exec().await, raw_exec);
    rows.push(json!({"call": "exec.on_settled via relay", "predecessor": relay.id(), "expected": "Method on_settled is private", "observed_panic": o, "executor_storage_unchanged": true}));
    let o = fails_containing(env.exec.as_account(), env.exec.id(), "on_settled", json!({ "id": 600 }), "not a settle callback").await;
    assert_eq!(env.raw_exec().await, raw_exec);
    assert_eq!(env.jobs().await, jobs_before);
    rows.push(json!({"call": "exec.on_settled({id: 600}) signed by the executor's own key (no promise result)", "predecessor": env.exec.id(), "expected": "not a settle callback", "observed_panic": o, "executor_storage_unchanged": true}));
    record("RT-PRIVATE", json!({ "property": "RT-PRIVATE", "also": "EX-8 (on_settled privacy and result count on the real runtime)", "assumption": "ASM-PRIVATE", "result": "pass", "cases": rows }));
}

// -------------------------------------------------------------- RT-CALLBACK

/// PROP: RT-CALLBACK EX-5. The dispatch callback's mapping on the real
/// receipt path, one row per executor mode, compared with the core after
/// each step; the executor's jobs and `newest` after each step; and the
/// receipt shape of `finish` / `confirm_not_executed` (v1.1.0: the call, the
/// lock's `settle`, then the executor's `on_settled`).
#[tokio::test]
async fn rt_callback_status_mapping() {
    let env = Env::bare().await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let env = env.init(&cfg).await;
    let a = env.actor.id().as_str();
    let x = env.exec.id().as_str().to_string();
    let mut reference = env.pool().await;
    let mut rows = vec![];

    // normal: execute returns "active" -> acknowledge(true) -> Active, the
    // reservation stays (ASYNC-1).
    let id_normal = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    let held = reference.values.clone();
    let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id_normal })).await;
    reference.dispatch(a, id_normal, env.now().await).unwrap();
    reference.acknowledge(id_normal, true).unwrap();
    same(&env, &reference, "normal").await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": id_normal, "outcome": "active", "error": null }));
    assert_eq!(env.status(id_normal).await, Status::Active);
    assert_eq!(env.pool().await.values, held);
    let job = |id: u32, status: &str| ExecJob { id, recipe: 0, units: 4, status: status.into() };
    assert_eq!(env.jobs().await, vec![job(id_normal, "active")]);
    assert_eq!(env.newest().await, id_normal);
    rows.push(json!({"mode": "normal", "execute_returns": "active", "status": "Active", "values": held, "event": ev, "executor_job": "active"}));

    // reject: execute returns "cancelled" -> settle(executor, false) ->
    // Cancelled, the reservation released.
    env.mode("reject").await;
    let id_reject = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    let before = reference.values.clone();
    let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id_reject })).await;
    reference.dispatch(a, id_reject, env.now().await).unwrap();
    reference.settle(&x, id_reject, false).unwrap();
    same(&env, &reference, "reject").await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": id_reject, "outcome": "cancelled", "error": null }));
    assert_eq!(env.status(id_reject).await, Status::Cancelled);
    assert_eq!(reference.values, held, "release undoes exactly the reservation");
    assert_eq!(env.jobs().await, vec![job(id_normal, "active"), job(id_reject, "cancelled")]);
    assert_eq!(env.newest().await, id_reject);
    rows.push(json!({"mode": "reject", "execute_returns": "cancelled", "status": "Cancelled", "values_before": before, "values_after": reference.values, "event": ev}));

    // unknown: execute stores an active job but returns "unknown" ->
    // acknowledge(false) -> Unknown; reconcile (anyone) reads "active".
    env.mode("unknown").await;
    let id_unknown = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id_unknown })).await;
    reference.dispatch(a, id_unknown, env.now().await).unwrap();
    reference.acknowledge(id_unknown, false).unwrap();
    same(&env, &reference, "unknown").await;
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": id_unknown, "outcome": "unknown", "error": null }));
    // The executor stored an active job; only its answer was "unknown".
    assert_eq!(env.jobs().await.last(), Some(&job(id_unknown, "active")));
    fails(&env.actor, env.lock.id(), "cancel", json!({ "id": id_unknown }), "Pending").await;
    let r = ok(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id_unknown })).await;
    reference.acknowledge(id_unknown, true).unwrap();
    same(&env, &reference, "reconcile unknown").await;
    let ev2 = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev2["outcome"], "active");
    // A repeated reconcile is a no-op, version included.
    let v = env.pool().await.version;
    ok(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id_unknown })).await;
    assert_eq!(env.pool().await.version, v);
    rows.push(json!({"mode": "unknown", "execute_returns": "unknown", "status": "Unknown", "event": ev, "cancel": "Pending", "reconcile": {"by": "mallory", "status_returns": "active", "status": "Active", "repeat_is_noop": true}}));

    // panic: execute fails -> the callback sees a PromiseError -> Unknown, the
    // reservation stays. The dispatch's own state change is not reverted.
    env.mode("panic").await;
    let id_panic = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    let held_panic = reference.values.clone();
    let r = tx(&env.actor, env.lock.id(), "dispatch", json!({ "id": id_panic })).await;
    let on_exec = receipts_on(&r, env.exec.id());
    let failed = on_exec.iter().find(|o| o.is_failure()).expect("execute should fail");
    let exec_failure = format!("{:?}", (*failed).clone().into_result().unwrap_err());
    let exec_panic = expect_panic(&exec_failure, "injected failure", false);
    assert!(r.is_success(), "the callback itself succeeds: {:?}", r.failures());
    reference.dispatch(a, id_panic, env.now().await).unwrap();
    reference.acknowledge(id_panic, false).unwrap();
    same(&env, &reference, "panic").await;
    // The failed execute stored nothing and did not move `newest`.
    assert_eq!(env.jobs().await.len(), 3);
    assert_eq!(env.newest().await, id_unknown);
    let ev = event(&r, env.lock.id(), "reconciled");
    assert_eq!(ev, json!({ "id": id_panic, "outcome": "unknown", "error": null }));
    assert_eq!(env.pool().await.values, held_panic);
    // reconcile: no job, status "unknown" -> acknowledge(false), a no-op.
    let v = env.pool().await.version;
    ok(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id_panic })).await;
    assert_eq!(env.pool().await.version, v);
    // Definitive nonexecution from the operator releases it: the call, the
    // lock's settle(false), then on_settled, which raises `newest`.
    let r = ok(&env.owner, env.exec.id(), "confirm_not_executed", json!({ "id": id_panic, "recipe": 0, "units": 4 })).await;
    let confirm_shape = shape(&r, env.exec.id(), env.lock.id());
    assert_eq!(confirm_shape, (vec![true, true], vec![true]));
    reference.settle(&x, id_panic, false).unwrap();
    same(&env, &reference, "confirm_not_executed").await;
    assert_eq!(env.jobs().await.last(), Some(&job(id_panic, "cancelled")));
    assert_eq!(env.newest().await, id_panic);
    rows.push(json!({"mode": "panic", "execute": "fails", "execute_panic": exec_panic, "dispatch_tx_success": true, "status": "Unknown", "values": held_panic, "event": ev,
        "reconcile": "status unknown, no-op", "confirm_not_executed": {"status": "Cancelled", "values": reference.values, "receipts_ok": {"executor": confirm_shape.0, "lock": confirm_shape.1}}}));

    // Close the rest through the operator: Completed and Cancelled.
    let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": id_normal, "completed": true })).await;
    assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
    reference.settle(&x, id_normal, true).unwrap();
    let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": id_unknown, "completed": false })).await;
    assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
    reference.settle(&x, id_unknown, false).unwrap();
    same(&env, &reference, "finish").await;
    // Same terminal again: idempotent (TERM-1 through the executor).
    let v = env.pool().await.version;
    ok(&env.owner, env.exec.id(), "finish", json!({ "id": id_normal, "completed": true })).await;
    assert_eq!(env.pool().await.version, v);
    let statuses: Vec<String> = env.pool().await.records.iter().map(|c| format!("{:?}", c.status)).collect();
    assert_eq!(statuses, ["Completed", "Cancelled", "Cancelled", "Cancelled"]);
    let held_jobs: Vec<(u32, String)> = env.jobs().await.into_iter().map(|j| (j.id, j.status)).collect();
    assert_eq!(
        held_jobs,
        vec![(id_normal, "completed".into()), (id_reject, "cancelled".into()), (id_unknown, "cancelled".into()), (id_panic, "cancelled".into())]
    );
    assert_eq!(env.newest().await, id_panic);
    record(
        "RT-CALLBACK",
        json!({ "property": "RT-CALLBACK", "assumption": "ASM-CALLBACK", "result": "pass", "modes": rows,
            "finish_receipts_ok": {"executor": [true, true], "lock": [true], "order": "finish, then the lock's settle, then on_settled"},
            "final": summary(&env.pool().await), "executor_jobs": held_jobs, "executor_newest": id_panic }),
    );
}

/// Upstream crates/sandbox/tests/integration.rs `non_string_executor` at the
/// pin (MIT, Copyright (c) 2026 Differential AI Lab), ported unchanged: a
/// Wasm executor whose `execute` returns the JSON number `5` and whose
/// `status` returns a JSON string of 70 bytes. Neither is one JSON string of
/// at most 64 bytes. Hand-assembled: one import (`env.value_return`), one
/// page of memory, two exports, two data segments.
fn non_string_executor() -> Vec<u8> {
    fn uleb(mut n: usize, out: &mut Vec<u8>) {
        loop {
            let byte = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    fn bytes(b: &[u8], out: &mut Vec<u8>) {
        uleb(b.len(), out);
        out.extend_from_slice(b);
    }
    fn section(id: u8, items: &[Vec<u8>], out: &mut Vec<u8>) {
        let mut body = vec![];
        uleb(items.len(), &mut body);
        items.iter().for_each(|i| body.extend_from_slice(i));
        out.push(id);
        bytes(&body, out);
    }
    let long = format!("\"{}\"", "a".repeat(68));
    // (offset, data) of each value; offsets and lengths are below 64, so each
    // i32.const and i64.const operand is one signed LEB128 byte.
    let values: [(u8, &[u8]); 2] = [(0, b"5"), (8, long.as_bytes())];
    assert!(values.iter().all(|(o, d)| *o < 64 && d.len() < 128));
    let mut m = b"\0asm\x01\0\0\0".to_vec();
    // Types: 0 = (i64, i64) -> (), 1 = () -> ().
    section(1, &[vec![0x60, 2, 0x7e, 0x7e, 0], vec![0x60, 0, 0]], &mut m);
    let mut import = vec![];
    bytes(b"env", &mut import);
    bytes(b"value_return", &mut import);
    import.extend([0, 0]);
    section(2, &[import], &mut m);
    section(3, &[vec![1], vec![1]], &mut m);
    section(5, &[vec![0, 1]], &mut m);
    let export = |name: &[u8], func: u8| {
        let mut e = vec![];
        bytes(name, &mut e);
        e.extend([0, func]);
        e
    };
    section(7, &[export(b"execute", 1), export(b"status", 2)], &mut m);
    // value_return(len, ptr) with each value, then end.
    let body = |(offset, data): (u8, &[u8])| {
        let len = data.len() as u8;
        // A value of 64 or more needs a second byte as signed LEB128.
        let mut code = vec![0, 0x42];
        if len < 64 {
            code.push(len);
        } else {
            code.extend([len, 0]);
            code[2] |= 0x80;
        }
        code.extend([0x42, offset, 0x10, 0, 0x0b]);
        let mut b = vec![];
        bytes(&code, &mut b);
        b
    };
    section(10, &[body(values[0]), body(values[1])], &mut m);
    let data = |(offset, d): (u8, &[u8])| {
        let mut s = vec![0, 0x41, offset, 0x0b];
        bytes(d, &mut s);
        s
    };
    section(11, &[data(values[0]), data(values[1])], &mut m);
    m
}

/// PROP: RT-CALLBACK. NT-F4 on the real host (upstream integration.rs's
/// non-string test, with the core compared after every step and raw storage
/// checked): an executor result that is not one JSON string of at most 64
/// bytes leaves the record Unknown, and the callback does not fail. `execute`
/// returns `5`; `status` returns a 70-byte string; then the executor
/// account's own settle(false) releases the reservation.
#[tokio::test]
async fn rt_callback_non_string_executor_results() {
    let worker = sandbox().await;
    let owner = subaccount(&worker, "owner", 20).await;
    let actor = subaccount(&worker, "actor", 20).await;
    let lock = deploy(&worker, "pool", 50, &contract_wasm()).await;
    let ns = deploy(&worker, "nsexec", 10, &non_string_executor()).await;
    // The executor returns these exact bytes; it does not fail.
    let long = format!("\"{}\"", "a".repeat(68));
    assert_eq!(long.len(), 70);
    for (method, value) in [("execute", "5"), ("status", long.as_str())] {
        let r = ns.view(method).await.unwrap();
        assert_eq!(r.result, value.as_bytes(), "{method}");
    }
    let mut cfg = pool_core::fixtures::config("gpu").unwrap();
    cfg.owner = owner.id().to_string();
    for r in &mut cfg.recipes {
        r.actor = actor.id().to_string();
        r.executor = ns.id().to_string();
    }
    ok(lock.as_account(), lock.id(), "new", json!({ "config": cfg })).await;
    let mut reference = pool(&lock).await;
    let now = block_now(&worker).await;
    let req = Request { recipe: 0, units: 4, version: 0, expires_at: now + 3600 };
    let r = ok(&actor, lock.id(), "submit", json!({ "request": req })).await;
    assert_eq!(r.json::<Value>().unwrap()["id"], 0);
    reference.submit(actor.id().as_str(), req, block_now(&worker).await).unwrap();
    assert_eq!(pool(&lock).await, reference);
    let expected = json!({ "id": 0, "outcome": "unknown", "error": null });
    let mut rows = vec![];
    // execute returns 5: the record goes Unknown, the transaction succeeds.
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 0 })).await;
    assert_eq!(event(&r, lock.id(), "reconciled"), expected);
    reference.dispatch(actor.id().as_str(), 0, block_now(&worker).await).unwrap();
    reference.acknowledge(0, false).unwrap();
    assert_eq!(pool(&lock).await, reference);
    assert_eq!(reference.records[0].status, Status::Unknown);
    rows.push(json!({"call": "dispatch", "executor_returns": "5 (a JSON number)", "status": "Unknown", "event": expected, "tx": "success"}));
    // status returns a 70-byte string: still Unknown, nothing written.
    let raw = raw_state(&worker, lock.id()).await;
    let r = ok(&actor, lock.id(), "reconcile", json!({ "id": 0 })).await;
    assert_eq!(event(&r, lock.id(), "reconciled"), expected);
    assert_eq!(raw_state(&worker, lock.id()).await, raw, "reconcile of an unreadable status wrote state");
    assert_eq!(pool(&lock).await, reference);
    rows.push(json!({"call": "reconcile", "executor_returns": "a 70-byte JSON string", "status": "Unknown", "event": expected, "storage_unchanged": true}));
    // The reservation is held until the executor account reports.
    ok(ns.as_account(), lock.id(), "settle", json!({ "id": 0, "completed": false })).await;
    reference.settle(ns.id().as_str(), 0, false).unwrap();
    assert_eq!(pool(&lock).await, reference);
    assert_eq!(reference.records[0].status, Status::Cancelled);
    rows.push(json!({"call": "settle(0, false) signed by the executor account", "status": "Cancelled", "values": reference.values}));
    record(
        "RT-CALLBACK-nonstring",
        json!({ "property": "RT-CALLBACK", "finding": "NT-F4 (fixed in v1.1.0)", "result": "pass",
            "executor": "upstream integration.rs non_string_executor (hand-assembled wasm)", "executor_sha256": sha256_hex(&non_string_executor()), "cases": rows }),
    );
}

// ---------------------------------------------------------------- RT-REVERT

/// PROP: RT-REVERT. A call whose core step fails panics and leaves the pool's
/// storage byte-for-byte unchanged (ASM-ATOMIC), with no event; a rejected
/// submit returns a decision instead and also writes nothing. v1.1.0: a
/// deposit on `reconcile` is refused (NT-F2); cancel of a Completed record is
/// InvalidState; at VERSION_CAP (patched in) submit, dispatch, set_paused and
/// observe stop with Arithmetic (submit as a rejected decision, which preview
/// predicts), while cancel and settle still resolve the open records (F-5).
#[tokio::test]
async fn rt_revert_core_errors_write_nothing() {
    let env = Env::bare().await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let env = env.init(&cfg).await;
    let (o, a, m) = (env.owner.id().as_str(), env.actor.id().as_str(), env.mallory.id().as_str());
    let x = env.exec.id().as_str().to_string();
    let mut reference = env.pool().await;
    let reserved = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    let active = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": active })).await;
    reference.dispatch(a, active, env.now().await).unwrap();
    reference.acknowledge(active, true).unwrap();
    same(&env, &reference, "setup").await;
    let now = env.now().await;
    let mut rows = vec![];

    type Core<'a> = Box<dyn Fn(&mut Pool) -> Result<(), Error> + 'a>;
    // (signer, method, args, error, the same call on the core)
    let xs = x.clone();
    let cases: Vec<(&near_workspaces::Account, &str, Value, Error, Core)> = vec![
        (&env.mallory, "dispatch", json!({"id": reserved}), Error::Unauthorized, Box::new(move |p: &mut Pool| p.dispatch(m, reserved, now))),
        (&env.actor, "dispatch", json!({"id": 99}), Error::NotFound, Box::new(move |p: &mut Pool| p.dispatch(a, 99, now))),
        (&env.actor, "dispatch", json!({"id": active}), Error::InvalidState, Box::new(move |p: &mut Pool| p.dispatch(a, active, now))),
        (&env.actor, "settle", json!({"id": active, "completed": true}), Error::Unauthorized, Box::new(move |p: &mut Pool| p.settle(a, active, true))),
        (env.exec.as_account(), "settle", json!({"id": reserved, "completed": true}), Error::InvalidState, Box::new(|p: &mut Pool| p.settle(&xs, reserved, true))),
        (env.exec.as_account(), "settle", json!({"id": 99, "completed": false}), Error::NotFound, Box::new(|p: &mut Pool| p.settle(&xs, 99, false))),
        (&env.mallory, "cancel", json!({"id": reserved}), Error::Unauthorized, Box::new(move |p: &mut Pool| p.cancel(m, reserved, now))),
        (&env.actor, "cancel", json!({"id": active}), Error::Pending, Box::new(move |p: &mut Pool| p.cancel(a, active, now))),
        (&env.actor, "set_paused", json!({"paused": true}), Error::Unauthorized, Box::new(move |p: &mut Pool| p.set_paused(a, true))),
        (&env.owner, "observe", json!({"values": [50, 0]}), Error::Pending, Box::new(move |p: &mut Pool| p.observe(o, vec![50, 0], now))),
    ];
    for (signer, method, args, err, core) in cases {
        let raw = env.raw().await;
        let mut p = reference.clone();
        assert_eq!(core(&mut p), Err(err.clone()), "core disagrees on {method}");
        assert_eq!(p, reference, "core error changed the pool (TX-0)");
        let (observed, r) = fails_r(signer, env.lock.id(), method, args.clone(), &err.to_string()).await;
        assert!(events_on(&r, env.lock.id()).is_empty(), "{method}: event from a failed call");
        assert_eq!(env.raw().await, raw, "{method}: storage changed");
        rows.push(json!({"call": method, "signer": signer.id(), "args": args, "core_error": err.to_string(), "observed_panic": observed, "storage_unchanged": true, "events": 0}));
    }
    same(&env, &reference, "after errors").await;

    // Paused: dispatch panics, submit is rejected without a panic.
    ok(&env.owner, env.lock.id(), "set_paused", json!({ "paused": true })).await;
    reference.set_paused(o, true).unwrap();
    let raw = env.raw().await;
    let observed = fails(&env.actor, env.lock.id(), "dispatch", json!({ "id": reserved }), "Paused").await;
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "dispatch", "state": "paused", "core_error": "Paused", "observed_panic": observed, "storage_unchanged": true}));
    let req = env.request(0, 1).await;
    let (r, v) = env.submit(&env.actor, &req).await;
    assert_eq!(v["decision"]["reason"], "Paused");
    assert_eq!(event(&r, env.lock.id(), "decision")["allowed"], false);
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "submit", "state": "paused", "rejected": "Paused", "panic": false, "storage_unchanged": true}));
    ok(&env.owner, env.lock.id(), "set_paused", json!({ "paused": false })).await;
    reference.set_paused(o, false).unwrap();

    // Rejected submits (Stale, InvalidUnits, Unauthorized): no panic, one
    // decision event, nothing written.
    let raw = env.raw().await;
    let mut stale = env.request(0, 1).await;
    stale.version -= 1;
    for (signer, req, reason) in [
        (&env.actor, stale, "Stale"),
        (&env.actor, env.request(0, 11).await, "InvalidUnits"),
        (&env.mallory, env.request(0, 1).await, "Unauthorized"),
    ] {
        let (r, v) = env.submit(signer, &req).await;
        assert_eq!(v["decision"]["reason"], reason);
        assert_eq!(v["id"], Value::Null);
        assert_eq!(event(&r, env.lock.id(), "decision")["allowed"], false);
        assert_eq!(env.raw().await, raw, "rejected submit ({reason}) wrote state");
        rows.push(json!({"call": "submit", "rejected": reason, "panic": false, "storage_unchanged": true}));
    }
    same(&env, &reference, "after rejections").await;

    // Close everything, then observe with bad input: Bounds.
    ok(&env.actor, env.lock.id(), "cancel", json!({ "id": reserved })).await;
    reference.cancel(a, reserved, env.now().await).unwrap();
    ok(&env.owner, env.exec.id(), "finish", json!({ "id": active, "completed": true })).await;
    reference.settle(&x, active, true).unwrap();
    same(&env, &reference, "closed").await;
    for values in [vec![50], vec![50, 1_000_001], vec![-1_000_001, 0]] {
        let raw = env.raw().await;
        assert_eq!(reference.clone().observe(o, values.clone(), now), Err(Error::Bounds));
        let observed = fails(&env.owner, env.lock.id(), "observe", json!({ "values": values }), "Bounds").await;
        assert_eq!(env.raw().await, raw);
        rows.push(json!({"call": "observe", "values": values, "core_error": "Bounds", "observed_panic": observed, "storage_unchanged": true}));
    }
    // v1.1.0: cancel of a Completed record is conflicting terminal evidence.
    let raw = env.raw().await;
    assert_eq!(reference.clone().cancel(a, active, now), Err(Error::InvalidState));
    let (observed, r) = fails_r(&env.actor, env.lock.id(), "cancel", json!({ "id": active }), "InvalidState").await;
    assert!(events_on(&r, env.lock.id()).is_empty());
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "cancel", "state": "record Completed", "core_error": "InvalidState", "observed_panic": observed, "storage_unchanged": true}));

    // Open records for the deposit and version-cap checks: one Reserved, one
    // Active.
    let reserved = env.submit_ok(&mut reference, &env.actor, 0, 2).await;
    let active = env.submit_ok(&mut reference, &env.actor, 0, 3).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": active })).await;
    reference.dispatch(a, active, env.now().await).unwrap();
    reference.acknowledge(active, true).unwrap();
    same(&env, &reference, "open records").await;

    // NT-F2: reconcile is a &self method, so near-sdk generates no
    // nonpayable check; v1.1.0 adds one. A 1-yocto reconcile fails with the
    // SDK's text and changes nothing (the deposit is refunded).
    let raw = env.raw().await;
    let r = tx_deposit(&env.mallory, env.lock.id(), "reconcile", json!({ "id": active }), 1).await;
    let observed = expect_panic(&failure_text(&r), "Method reconcile doesn't accept deposit", true);
    assert!(receipts_on(&r, env.exec.id()).is_empty(), "a refused reconcile called the executor");
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "reconcile", "deposit_yocto": 1, "finding": "NT-F2 (fixed)", "observed_panic": observed, "executor_called": false, "storage_unchanged": true}));

    // F-5: the version cap. Reaching it takes about 2^32 effective
    // transitions, so the state is patched in.
    reference.version = VERSION_CAP;
    env.worker.patch_state(env.lock.id(), b"STATE", &borsh::to_vec(&reference).unwrap()).await.unwrap();
    same(&env, &reference, "patched to VERSION_CAP").await;
    let state = format!("version = VERSION_CAP = {VERSION_CAP} (patched); record {reserved} Reserved, record {active} Active, the rest terminal");
    let now = env.now().await;
    let raw = env.raw().await;
    let values = reference.values.clone();
    let xs = x.clone();
    let capped: Vec<(&near_workspaces::Account, &str, Value, Core)> = vec![
        (&env.owner, "set_paused", json!({"paused": true}), Box::new(move |p: &mut Pool| p.set_paused(o, true))),
        (&env.owner, "observe", json!({"values": values.clone()}), Box::new(move |p: &mut Pool| p.observe(o, values.clone(), now))),
        (&env.actor, "dispatch", json!({"id": reserved}), Box::new(move |p: &mut Pool| p.dispatch(a, reserved, now))),
    ];
    for (signer, method, args, core) in capped {
        let mut p = reference.clone();
        assert_eq!(core(&mut p), Err(Error::Arithmetic), "core at the cap: {method}");
        assert_eq!(p, reference);
        let (observed, r) = fails_r(signer, env.lock.id(), method, args.clone(), "Arithmetic").await;
        assert!(events_on(&r, env.lock.id()).is_empty());
        assert!(receipts_on(&r, env.exec.id()).is_empty(), "{method} at the cap called the executor");
        assert_eq!(env.raw().await, raw, "{method} at the cap changed storage");
        let what = if method == "observe" { "observe (restating the current values)".to_string() } else { method.to_string() };
        rows.push(json!({"call": what, "state": state, "core_error": "Arithmetic", "observed_panic": observed, "storage_unchanged": true}));
    }
    // submit: preview predicts the refusal (v1.0.0's preview said allowed
    // and submit panicked); submit returns the rejected decision.
    let req = Request { recipe: 0, units: 1, version: VERSION_CAP, expires_at: env.now().await + 3600 };
    let preview: pool_core::Decision = env.lock.view("preview").args_json(json!({ "actor": env.actor.id(), "request": req })).await.unwrap().json().unwrap();
    assert_eq!((preview.allowed, preview.reason.as_str()), (false, "Arithmetic"), "{preview:?}");
    assert_eq!(preview, reference.preview(a, &req, env.now().await));
    let (r, v) = env.submit(&env.actor, &req).await;
    assert_eq!((&v["decision"]["allowed"], &v["decision"]["reason"], &v["id"]), (&json!(false), &json!("Arithmetic"), &Value::Null));
    assert_eq!(event(&r, env.lock.id(), "decision")["allowed"], false);
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "submit", "state": state, "preview": {"allowed": false, "reason": "Arithmetic"}, "rejected": "Arithmetic", "panic": false, "storage_unchanged": true}));
    // Resolution still works above the cap (plain increment): cancel the
    // Reserved record, finish the Active one through the executor.
    ok(&env.actor, env.lock.id(), "cancel", json!({ "id": reserved })).await;
    reference.cancel(a, reserved, env.now().await).unwrap();
    same(&env, &reference, "cancel at the cap").await;
    let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": active, "completed": true })).await;
    assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
    reference.settle(&xs, active, true).unwrap();
    same(&env, &reference, "finish at the cap").await;
    assert_eq!(reference.version, VERSION_CAP + 2);
    rows.push(json!({"call": "cancel (Reserved), then operator finish(true) (Active)", "state": state, "result": "Ok, Ok", "version_after": reference.version, "status": ["Cancelled", "Completed"]}));
    // Retired: nothing new starts, also with every record terminal.
    let raw = env.raw().await;
    let values = reference.values.clone();
    assert_eq!(reference.clone().observe(o, values.clone(), now), Err(Error::Arithmetic));
    let observed = fails(&env.owner, env.lock.id(), "observe", json!({ "values": values }), "Arithmetic").await;
    assert_eq!(env.raw().await, raw);
    rows.push(json!({"call": "observe (restate), every record terminal", "state": format!("version = VERSION_CAP + 2"), "core_error": "Arithmetic", "observed_panic": observed, "storage_unchanged": true}));
    record("RT-REVERT", json!({ "property": "RT-REVERT", "assumption": "ASM-ATOMIC", "result": "pass", "version_cap": VERSION_CAP, "cases": rows }));
}

// ------------------------------------------------------------------ RT-RACE

/// PROP: RT-RACE. The executor's settle lands before the dispatch callback
/// (deterministic on one shard with the helper executor, see relay/): the
/// late "active" ack is ignored, version included. Also with settle(false),
/// and for a stale reconcile read.
#[tokio::test]
async fn rt_race_settle_before_callback() {
    let env = Env::bare().await;
    let racer = deploy(&env.worker, "racer", 10, &relay_wasm()).await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), racer.id())]);
    let env = env.init(&cfg).await;
    let a = env.actor.id().as_str();
    let x = racer.id().as_str();
    let mut reference = env.pool().await;
    let mut rows = vec![];

    for completed in [true, false] {
        ok(&env.owner, racer.id(), "set_settle", json!({ "settle": completed })).await;
        let id = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
        let v0 = reference.version;
        let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(a, id, env.now().await).unwrap();
        // The other order, for contrast: callback first, then settle.
        let mut callback_first = reference.clone();
        callback_first.acknowledge(id, true).unwrap();
        callback_first.settle(x, id, completed).unwrap();
        // The order NEAR produces here: settle, then the late ack (a no-op).
        reference.settle(x, id, completed).unwrap();
        let before_ack = reference.clone();
        reference.acknowledge(id, true).unwrap();
        assert_eq!(reference, before_ack, "late ack must be a no-op");
        same(&env, &reference, "race").await;
        assert_eq!(env.pool().await.version, v0 + 2, "dispatch + settle only");
        assert_eq!(callback_first.version, v0 + 3);
        let settled = event(&r, env.lock.id(), "settled");
        let ack = event(&r, env.lock.id(), "reconciled");
        assert_eq!(settled, json!({ "id": id, "completed": completed }));
        assert_eq!(ack, json!({ "id": id, "outcome": "active", "error": null }));
        let h_settle = block_height(&env.worker, receipts_on(&r, env.lock.id()).into_iter().find(|o| o.logs.iter().any(|l| l.contains("\"settled\""))).unwrap()).await;
        let h_ack = block_height(&env.worker, receipts_on(&r, env.lock.id()).into_iter().find(|o| o.logs.iter().any(|l| l.contains("\"reconciled\""))).unwrap()).await;
        assert!(h_settle <= h_ack);
        rows.push(json!({"path": "dispatch: execute settles, then returns active", "completed": completed,
            "status": format!("{:?}", env.status(id).await), "version_delta": 2, "callback_first_would_give": 3,
            "settle_block": h_settle, "callback_block": h_ack, "late_ack_event": ack}));
    }

    // Stale reconcile read: an Active record; reconcile's status call
    // settles first and answers "active".
    ok(&env.owner, racer.id(), "set_settle", json!({ "settle": null })).await;
    let id = env.submit_ok(&mut reference, &env.actor, 0, 4).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
    reference.dispatch(a, id, env.now().await).unwrap();
    reference.acknowledge(id, true).unwrap();
    same(&env, &reference, "active").await;
    ok(&env.owner, racer.id(), "set_settle", json!({ "settle": true })).await;
    let v0 = reference.version;
    let r = ok(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id })).await;
    reference.settle(x, id, true).unwrap();
    reference.acknowledge(id, true).unwrap();
    same(&env, &reference, "stale reconcile").await;
    assert_eq!(env.pool().await.version, v0 + 1);
    assert_eq!(event(&r, env.lock.id(), "reconciled")["outcome"], "active");
    // A terminal record cannot be reconciled at all.
    let observed = fails_containing(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id }), "InvalidState").await;
    rows.push(json!({"path": "reconcile: status settles, then returns active", "completed": true,
        "status": format!("{:?}", env.status(id).await), "version_delta": 1, "reconcile_after_terminal": observed}));
    record("RT-RACE", json!({ "property": "RT-RACE", "assumption": "ASM-INTERLEAVE", "result": "pass", "executor": "helper (relay/ deployed as racer)", "cases": rows }));
}

/// PROP: RT-RACE (upstream executor). The upstream executor contract cannot
/// overtake its own callback on one shard: `finish` needs the job that
/// `execute` creates in the receipt that also emits the callback's data, so
/// its notify lands after the callback. The executor account's own key can:
/// a settle sent while the dispatch is in flight. Timed race, repeated over a
/// range of delays; every outcome must be one of the serial orders the core
/// allows, and the late-ack order must occur at least once.
#[tokio::test]
async fn rt_race_executor_key_settles_while_callback_in_flight() {
    const DELAYS_MS: [u64; 6] = [0, 100, 200, 300, 400, 150];
    const ATTEMPTS: usize = 16;
    let env = Env::bare().await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let env = env.init(&cfg).await;
    let a = env.actor.id().as_str();
    let x = env.exec.id().as_str();
    let mut reference = env.pool().await;
    let mut rows = vec![];
    let mut late = 0;
    for attempt in 0..ATTEMPTS {
        let delay = DELAYS_MS[attempt % DELAYS_MS.len()];
        let id = env.submit_ok(&mut reference, &env.actor, 0, 1).await;
        let v0 = reference.version;
        let t1 = env.actor.call(env.lock.id(), "dispatch").args_json(json!({ "id": id })).max_gas().transact_async().await.unwrap();
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let t2 = env
            .exec
            .as_account()
            .call(env.lock.id(), "settle")
            .args_json(json!({ "id": id, "completed": true }))
            .max_gas()
            .transact_async()
            .await
            .unwrap();
        let start = Instant::now();
        let r1 = t1.await.unwrap();
        let r2 = t2.await.unwrap();
        assert!(start.elapsed() < Duration::from_secs(60));
        assert!(r1.is_success(), "{:?}", r1.failures());
        let on_pool = receipts_on(&r1, env.lock.id());
        let ack_receipt = on_pool.iter().find(|o| o.logs.iter().any(|l| l.contains("\"reconciled\""))).unwrap();
        let dispatch_receipt = on_pool.iter().find(|o| !o.logs.iter().any(|l| l.contains("\"reconciled\""))).unwrap();
        let settle_receipt = receipts_on(&r2, env.lock.id())[0];
        let heights = json!({
            "dispatch": block_height(&env.worker, dispatch_receipt).await,
            "execute": block_height(&env.worker, receipts_on(&r1, env.exec.id())[0]).await,
            "callback": block_height(&env.worker, ack_receipt).await,
            "settle": block_height(&env.worker, settle_receipt).await,
        });
        // The three serial orders the core allows.
        let mut settle_first = reference.clone();
        assert_eq!(settle_first.clone().settle(x, id, true), Err(Error::InvalidState));
        settle_first.dispatch(a, id, 0).unwrap();
        settle_first.acknowledge(id, true).unwrap();
        let mut late_ack = reference.clone();
        late_ack.dispatch(a, id, 0).unwrap();
        late_ack.settle(x, id, true).unwrap();
        late_ack.acknowledge(id, true).unwrap();
        let mut serial = reference.clone();
        serial.dispatch(a, id, 0).unwrap();
        serial.acknowledge(id, true).unwrap();
        serial.settle(x, id, true).unwrap();
        let p = env.pool().await;
        let order = if r2.is_failure() {
            assert_eq!(p, settle_first, "attempt {attempt}");
            let observed = expect_panic(&failure_text(&r2), "InvalidState", true);
            json!({ "order": "settle before dispatch: rejected", "observed_panic": observed })
        } else if p == late_ack {
            late += 1;
            json!({ "order": "dispatch, settle, callback: late ack ignored" })
        } else if p == serial {
            json!({ "order": "dispatch, callback, settle" })
        } else {
            panic!("attempt {attempt}: no serial order gives {p:?}");
        };
        assert_eq!(event(&r1, env.lock.id(), "reconciled")["error"], Value::Null);
        let mut row = json!({ "attempt": attempt, "delay_ms": delay, "version_delta": p.version - v0, "blocks": heights });
        row.as_object_mut().unwrap().extend(order.as_object().unwrap().clone());
        rows.push(row);
        reference = p;
        if late >= 3 {
            break;
        }
    }
    record(
        "RT-RACE-timed",
        json!({ "property": "RT-RACE", "executor": "upstream pool_executor, its account key signing settle", "late_ack_hits": late, "attempts": rows }),
    );
    assert!(late > 0, "the late-ack interleaving never occurred in {ATTEMPTS} attempts");
}

/// PROP: RT-RACE EX-5 (upstream executor, operator `finish`): the same timed
/// race with the operator's `finish(id, true)`, the path the property names.
/// `finish` needs the job `execute` creates; NEAR emits `execute`'s data
/// receipt (the callback's input) before any receipt `finish` can create
/// later, so on one shard the notify cannot overtake the callback. Every
/// outcome must be a serial order the core allows; the counts are recorded,
/// and a late-ack hit here would only strengthen RT-RACE. v1.1.0: an
/// accepted finish has three receipts (finish, the lock's settle, then
/// `on_settled`, which raises the executor's `newest` to the id); a refused
/// one ("unknown job") has only its own. `finish` returns `settle`'s value,
/// so the async outcome is re-polled (`complete`) until `on_settled` has run.
#[tokio::test]
async fn rt_race_operator_finish_vs_callback() {
    // Finer around the 200-300 ms boundary seen in the first runs, where
    // finish lands in execute's block or the next one.
    const DELAYS_MS: [u64; 8] = [0, 150, 200, 225, 250, 275, 300, 400];
    const ATTEMPTS: usize = 16;
    let env = Env::bare().await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let env = env.init(&cfg).await;
    let a = env.actor.id().as_str();
    let x = env.exec.id().as_str();
    let mut reference = env.pool().await;
    let mut rows = vec![];
    let mut counts = std::collections::BTreeMap::<&str, u32>::new();
    for attempt in 0..ATTEMPTS {
        let delay = DELAYS_MS[attempt % DELAYS_MS.len()];
        let id = env.submit_ok(&mut reference, &env.actor, 0, 1).await;
        let v0 = reference.version;
        let t1 = env.actor.call(env.lock.id(), "dispatch").args_json(json!({ "id": id })).max_gas().transact_async().await.unwrap();
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let t2 = env.owner.call(env.exec.id(), "finish").args_json(json!({ "id": id, "completed": true })).max_gas().transact_async().await.unwrap();
        let r1 = t1.await.unwrap();
        // An accepted finish is complete once on_settled (its second receipt
        // on the executor) has run; a refused one fails in its only receipt.
        let r2 = complete(&t2, |r| r.is_failure() || receipts_on(r, env.exec.id()).len() >= 2).await;
        assert!(r1.is_success(), "{:?}", r1.failures());
        let mut late_ack = reference.clone();
        late_ack.dispatch(a, id, 0).unwrap();
        late_ack.settle(x, id, true).unwrap();
        late_ack.acknowledge(id, true).unwrap();
        let mut serial = reference.clone();
        serial.dispatch(a, id, 0).unwrap();
        serial.acknowledge(id, true).unwrap();
        let mut p = env.pool().await;
        let (order, finish_block) = if r2.is_failure() {
            // finish ran before execute created the job: nothing notified.
            assert_eq!(p, serial, "attempt {attempt}");
            let observed = expect_panic(&failure_text(&r2), "unknown job", false);
            assert_eq!(shape(&r2, env.exec.id(), env.lock.id()), (vec![false], vec![]));
            let r3 = ok(&env.owner, env.exec.id(), "finish", json!({ "id": id, "completed": true })).await;
            assert_eq!(shape(&r3, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
            serial.settle(x, id, true).unwrap();
            p = env.pool().await;
            assert_eq!(p, serial);
            (format!("finish before execute: rejected ({observed}); finished afterwards"), Value::Null)
        } else {
            assert_eq!(shape(&r2, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]), "attempt {attempt}");
            let h = json!(block_height(&env.worker, receipts_on(&r2, env.lock.id())[0]).await);
            serial.settle(x, id, true).unwrap();
            if p == late_ack {
                ("dispatch, finish->settle, callback: late ack ignored".to_string(), h)
            } else {
                assert_eq!(p, serial, "attempt {attempt}: no serial order");
                ("dispatch, callback, finish->settle".to_string(), h)
            }
        };
        // The executor: the job is completed and on_settled raised `newest`.
        let st = exec_state(&env.worker, env.exec.id()).await;
        assert_eq!(st.newest, id, "attempt {attempt}");
        assert_eq!(st.jobs.iter().find(|j| j.id == id).map(|j| j.status.as_str()), Some("completed"));
        let key = if order.starts_with("finish before") { "finish_before_execute" } else if order.contains("late ack") { "late_ack" } else { "callback_first" };
        *counts.entry(key).or_default() += 1;
        let ack = receipts_on(&r1, env.lock.id()).into_iter().find(|o| o.logs.iter().any(|l| l.contains("\"reconciled\""))).unwrap();
        rows.push(json!({
            "attempt": attempt, "delay_ms": delay, "order": order, "version_delta_dispatch_to_terminal": p.version - v0,
            "blocks": { "execute": block_height(&env.worker, receipts_on(&r1, env.exec.id())[0]).await, "callback": block_height(&env.worker, ack).await, "settle": finish_block },
        }));
        reference = p;
    }
    record(
        "RT-RACE-finish",
        json!({ "property": "RT-RACE", "executor": "upstream pool_executor, operator finish", "counts": counts, "attempts": rows,
            "finish_receipts_ok": {"executor": [true, true], "lock": [true]},
            "note": "single shard: finish's notify is created after execute's data receipt, so the callback runs first" }),
    );
}

// ---------------------------------------------------------------- RT-WEDGE

/// A `preview` view of `req` by `actor`.
async fn preview(env: &Env, actor: &near_workspaces::AccountId, req: &Request) -> pool_core::Decision {
    env.lock.view("preview").args_json(json!({ "actor": actor, "request": req })).await.unwrap().json().unwrap()
}

/// `submit` that must be rejected with `reason` (no panic, nothing written,
/// the same refusal from the core and from `preview`).
async fn refused(env: &Env, reference: &Pool, actor: &near_workspaces::Account, recipe: u32, units: u32, reason: &str) -> Value {
    let req = env.request(recipe, units).await;
    let now = env.now().await;
    let expected = reference.preview(actor.id().as_str(), &req, now);
    assert_eq!((expected.allowed, expected.reason.as_str()), (false, reason), "core preview");
    assert_eq!(preview(env, actor.id(), &req).await, expected);
    let raw = env.raw().await;
    let (r, v) = env.submit(actor, &req).await;
    assert_eq!(v["decision"], serde_json::to_value(&expected).unwrap());
    assert_eq!(v["id"], Value::Null);
    assert_eq!(event(&r, env.lock.id(), "decision")["allowed"], false);
    assert_eq!(env.raw().await, raw, "a refused submit wrote state");
    json!({ "recipe": recipe, "units": units, "allowed": false, "reason": reason, "storage_unchanged": true })
}

/// `observe` by the owner that must succeed, applied to `reference`.
async fn observe_ok(env: &Env, reference: &mut Pool, values: Vec<i64>) {
    ok(&env.owner, env.lock.id(), "observe", json!({ "values": values })).await;
    reference.observe(env.owner.id().as_str(), values, env.now().await).unwrap();
    reference.observed_at = env.pool().await.observed_at;
    same(env, reference, "observe").await;
}

/// PROP: RT-WEDGE LIVE-2 EX-5. F-1, fixed in v1.1.0, end to end on the wasm
/// with the upstream executor. The v1.0.0 wedge (F-1 brief: observe [12, 5],
/// then the hedge r(a1, x1, [-2,0], [2,-5], 1, [])) is refused at submit
/// with Bounds and nothing is written, so nothing can wedge and observe is
/// Ok after. The hedge shape that cures by completion (upstream
/// crates/core/src/findings.rs:175: reserve [0,-1], complete [-3,0]) is
/// admitted in the same disturbed state and settles through the operator's
/// `finish(true)` (the call, the lock's settle, then on_settled).
#[tokio::test]
async fn rt_wedge_f1_refused_and_cure_settles() {
    let env = Env::bare().await;
    let cfg = wedge_config(env.owner.id(), env.actor.id(), env.exec.id());
    let base = env.init(&cfg).await;
    let env = &base;
    let a = env.actor.id().as_str();
    let mut reference = env.pool().await;
    let mut steps = vec![];

    observe_ok(env, &mut reference, vec![12, 5]).await;
    steps.push(json!({"step": "owner observe [12, 5] (metric 0 above its max 10)", "result": "Ok"}));
    let req = env.request(0, 1).await;
    assert_eq!(reference.best_quantity(a, &req, env.now().await), None);
    assert_eq!(reference.clone().submit(a, req, env.now().await), Err(Error::Bounds));
    let row = refused(env, &reference, &env.actor, 0, 1, "Bounds").await;
    steps.push(json!({"step": "actor submit the F-1 hedge x 1", "result": row}));
    let raw = env.raw().await;
    let o = fails(&env.actor, env.lock.id(), "dispatch", json!({ "id": 0 }), "NotFound").await;
    assert_eq!(env.raw().await, raw);
    assert!(env.jobs().await.is_empty());
    steps.push(json!({"step": "actor dispatch(0)", "observed_panic": o, "executor_jobs": 0}));
    observe_ok(env, &mut reference, vec![5, 5]).await;
    assert!(reference.records.is_empty());
    steps.push(json!({"step": "owner observe [5, 5]: nothing is open", "result": "Ok"}));

    // The cure-by-completion hedge, in a second pool in the same sandbox.
    let env = base.fresh("cure").await;
    let cfg = cure_config(env.owner.id(), env.actor.id(), env.exec.id());
    let env = env.init(&cfg).await;
    let x = env.exec.id().as_str().to_string();
    let mut reference = env.pool().await;
    let mut cure = vec![];
    observe_ok(&env, &mut reference, vec![12, 5]).await;
    let id = env.submit_ok(&mut reference, &env.actor, 0, 1).await;
    same(&env, &reference, "cure submit").await;
    assert_eq!(reference.values, vec![12, 4]);
    cure.push(json!({"step": "observe [12, 5]; actor submit the cure hedge x 1", "result": "Ok (allowed)", "id": id, "values": reference.values}));
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
    reference.dispatch(a, id, env.now().await).unwrap();
    reference.acknowledge(id, true).unwrap();
    same(&env, &reference, "cure dispatch").await;
    assert_eq!(env.jobs().await, vec![ExecJob { id, recipe: 0, units: 1, status: "active".into() }]);
    cure.push(json!({"step": "actor dispatch", "result": "execute -> active -> Active"}));
    let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": id, "completed": true })).await;
    let sh = shape(&r, env.exec.id(), env.lock.id());
    assert_eq!(sh, (vec![true, true], vec![true]));
    assert_eq!(event(&r, env.lock.id(), "settled"), json!({ "id": id, "completed": true }));
    reference.settle(&x, id, true).unwrap();
    same(&env, &reference, "cure finish").await;
    assert_eq!((env.status(id).await, reference.values.clone()), (Status::Completed, vec![9, 4]));
    assert_eq!(env.jobs().await, vec![ExecJob { id, recipe: 0, units: 1, status: "completed".into() }]);
    assert_eq!(env.newest().await, id);
    cure.push(json!({"step": "operator finish(id, true)", "receipts_ok": {"executor": sh.0, "lock": sh.1}, "status": "Completed", "values": [9, 4], "executor_job": "completed"}));
    // The operator cannot report the other outcome instead.
    let (raw, raw_exec) = (env.raw().await, env.raw_exec().await);
    let o = fails_containing(&env.owner, env.exec.id(), "finish", json!({ "id": id, "completed": false }), "terminal outcome conflict").await;
    assert_eq!((env.raw().await, env.raw_exec().await), (raw, raw_exec));
    cure.push(json!({"step": "operator finish(id, false)", "observed_panic": o, "storage_unchanged": true}));
    observe_ok(&env, &mut reference, vec![5, 5]).await;
    cure.push(json!({"step": "owner observe [5, 5]", "result": "Ok"}));
    record(
        "RT-WEDGE",
        json!({
            "property": "RT-WEDGE", "finding": "F-1 (fixed in v1.1.0)", "result": "pass: the wedge hedge is refused (Bounds), the cure hedge settles; LIVE-2 holds on the wasm",
            "f1_config": "m(0,10,5), m(-100,100,5); t(1,0,1,target); r(actor, exec, [-2,0], [2,-5], 1, [])",
            "f1_steps": steps,
            "cure_config": "m(0,10,5), m(-100,100,5); t(1,0,1,target); r(actor, exec, [0,-1], [-3,0], 1, []) (upstream findings.rs:175)",
            "cure_steps": cure,
            "final": summary(&env.pool().await),
        }),
    );
}

/// PROP: RT-WEDGE LIVE-2 EX-5. F-2, fixed in v1.1.0, on the wasm with the
/// `risk` fixture (upstream findings.rs:209 and :241; actors and executor
/// mapped to sandbox accounts): the selloff hedge may not rely on the credit
/// position's reservation (8 and 7 units refused Bounds, 6 admitted); after
/// the hedge completes (selloff loss 20), the position's honest release is
/// Ok: `cancel` while it is Reserved, or, once dispatched (its execute
/// failed, so it is Unknown), the operator's `confirm_not_executed`, whose
/// settle(false) the lock accepts. observe is Ok after either.
#[tokio::test]
async fn rt_wedge_f2_honest_release_after_the_hedge() {
    let base = Env::bare().await;
    let mut actors = std::collections::BTreeMap::new();
    for name in ["credit", "equity", "macro"] {
        actors.insert(format!("{name}.test.near"), subaccount(&base.worker, name, 20).await);
    }
    let mut forms = vec![];
    for (tag, reserved) in [("reserved", true), ("dispatched", false)] {
        let env = base.fresh(tag).await;
        let mut cfg = pool_core::fixtures::config("risk").unwrap();
        cfg.owner = env.owner.id().to_string();
        for r in &mut cfg.recipes {
            r.actor = actors[&r.actor].id().to_string();
            r.executor = env.exec.id().to_string();
        }
        let env = env.init(&cfg).await;
        let credit = &actors["credit.test.near"];
        let macro_ = &actors["macro.test.near"];
        let x = env.exec.id().as_str().to_string();
        let mut reference = env.pool().await;
        let mut steps = vec![];
        let position = env.submit_ok(&mut reference, credit, 0, 1).await;
        steps.push(json!({"step": "credit submit position (recipe 0) x 1", "id": position, "values": reference.values}));
        for units in [8, 7] {
            let row = refused(&env, &reference, macro_, 3, units, "Bounds").await;
            steps.push(json!({"step": format!("macro submit selloff hedge (recipe 3) x {units}"), "result": row}));
        }
        let req = env.request(3, 8).await;
        let best = reference.best_quantity(macro_.id().as_str(), &req, env.now().await).map(|q| q.units);
        assert_eq!(best, Some(6));
        let hedge = env.submit_ok(&mut reference, macro_, 3, 6).await;
        same(&env, &reference, "hedge x 6").await;
        steps.push(json!({"step": "macro submit selloff hedge x 6 (core best_quantity 6)", "id": hedge, "values": reference.values}));
        if !reserved {
            // The position is dispatched; its execute fails, so it is Unknown
            // until the operator's evidence.
            env.mode("panic").await;
            let r = ok(credit, env.lock.id(), "dispatch", json!({ "id": position })).await;
            assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![false], vec![true, true]));
            reference.dispatch(credit.id().as_str(), position, env.now().await).unwrap();
            reference.acknowledge(position, false).unwrap();
            same(&env, &reference, "position dispatched").await;
            env.mode("normal").await;
            steps.push(json!({"step": "credit dispatch position (executor in panic mode)", "status": "Unknown"}));
        }
        ok(macro_, env.lock.id(), "dispatch", json!({ "id": hedge })).await;
        reference.dispatch(macro_.id().as_str(), hedge, env.now().await).unwrap();
        reference.acknowledge(hedge, true).unwrap();
        let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": hedge, "completed": true })).await;
        assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
        reference.settle(&x, hedge, true).unwrap();
        same(&env, &reference, "hedge completed").await;
        assert_eq!(reference.values[0], 20);
        steps.push(json!({"step": "macro dispatch hedge; operator finish(hedge, true)", "status": "Completed", "values": reference.values}));
        // The honest release of the position.
        if reserved {
            ok(credit, env.lock.id(), "cancel", json!({ "id": position })).await;
            reference.cancel(credit.id().as_str(), position, env.now().await).unwrap();
            steps.push(json!({"step": "credit cancel(position)", "result": "Ok"}));
        } else {
            let r = ok(&env.owner, env.exec.id(), "confirm_not_executed", json!({ "id": position, "recipe": 0, "units": 1 })).await;
            let sh = shape(&r, env.exec.id(), env.lock.id());
            assert_eq!(sh, (vec![true, true], vec![true]), "F-2: the honest settle(false) is Ok");
            assert_eq!(event(&r, env.lock.id(), "settled"), json!({ "id": position, "completed": false }));
            reference.settle(&x, position, false).unwrap();
            steps.push(json!({"step": "operator confirm_not_executed(position)", "receipts_ok": {"executor": sh.0, "lock": sh.1}, "result": "Ok"}));
        }
        same(&env, &reference, "position released").await;
        assert_eq!(reference.values, vec![0, 40, 30, 28]);
        assert_eq!(env.status(position).await, Status::Cancelled);
        observe_ok(&env, &mut reference, vec![60, 40, 30, 40]).await;
        steps.push(json!({"step": "owner observe [60, 40, 30, 40]", "result": "Ok"}));
        forms.push(json!({ "form": tag, "steps": steps, "final": summary(&reference) }));
    }
    record(
        "RT-WEDGE-F2",
        json!({ "property": "RT-WEDGE", "finding": "F-2 (fixed in v1.1.0)", "fixture": "risk (upstream crates/core/src/fixtures.rs)", "result": "pass: the hedge is sized as if the position were released, and the position's release is Ok in both forms", "forms": forms }),
    );
}

// --------------------------------------------------------------- RT-WINDOW

/// `get_commitments(from, limit)`'s ids.
async fn page(env: &Env, from: u32, limit: u32) -> Vec<u32> {
    let v: Vec<Value> = env.lock.view("get_commitments").args_json(json!({ "from": from, "limit": limit })).await.unwrap().json().unwrap();
    v.iter().map(|c| c["id"].as_u64().unwrap() as u32).collect()
}

const CHECKPOINTS: [u32; 10] = [0, 50, 100, 150, 200, 250, 255, 256, 257, 299];

/// PROP: RT-WINDOW EX-7. F-3, fixed in v1.1.0, end to end (upstream
/// integration.rs `window_of_300_records_through_the_executor`, ported with
/// the core compared after every transaction): 300 records dispatched and
/// settled. Ids 0-9 and 290-299 run in normal mode and the operator
/// finishes them; the rest run in reject mode, where execute's "cancelled"
/// settles them through on_result. The lock keeps the 256 most recent
/// (first_id 44), the executor at most 256 jobs, and EX-7 holds after every
/// transaction. A pruned id is NotFound for dispatch, reconcile, cancel and
/// settle (storage byte-identical), and forgotten by the executor.
#[tokio::test]
async fn rt_window_300_records_through_the_executor() {
    let env = Env::bare().await;
    let cfg = window_config("window", env.owner.id(), env.actor.id(), env.exec.id());
    let env = env.init(&cfg).await;
    let a = env.actor.id().as_str();
    let x = env.exec.id().as_str().to_string();
    let mut reference = env.pool().await;
    let mut ex7 = Ex7::new(256);
    let mut mode = "normal";
    let mut checkpoints = vec![];
    let start = Instant::now();
    for id in 0..300u32 {
        let normal = !(10..290).contains(&id);
        let wanted = if normal { "normal" } else { "reject" };
        if mode != wanted {
            env.mode(wanted).await;
            mode = wanted;
        }
        assert_eq!(env.submit_ok(&mut reference, &env.actor, 0, 1).await, id);
        same(&env, &reference, &format!("submit {id}")).await;
        ex7.check(&env.worker, &env.lock, &env.exec, &format!("submit {id}")).await;
        ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(a, id, env.now().await).unwrap();
        if normal {
            reference.acknowledge(id, true).unwrap();
        } else {
            reference.settle(&x, id, false).unwrap();
        }
        same(&env, &reference, &format!("dispatch {id}")).await;
        let (_, st) = ex7.check(&env.worker, &env.lock, &env.exec, &format!("dispatch {id}")).await;
        assert_eq!(st.newest, id);
        let last = st.jobs.last().unwrap();
        assert_eq!((last.id, last.status.as_str()), (id, if normal { "active" } else { "cancelled" }));
        if normal {
            let r = ok(&env.owner, env.exec.id(), "finish", json!({ "id": id, "completed": true })).await;
            assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]), "finish {id}");
            reference.settle(&x, id, true).unwrap();
            same(&env, &reference, &format!("finish {id}")).await;
            ex7.check(&env.worker, &env.lock, &env.exec, &format!("finish {id}")).await;
        }
        if CHECKPOINTS.contains(&id) {
            let p = env.pool().await;
            let ids = job_ids(&env.exec).await;
            assert_eq!(ids, (id.saturating_sub(255)..=id).collect::<Vec<u32>>(), "jobs after record {id}");
            checkpoints.push(json!({ "record": id, "first_id": p.first_id, "records": p.records.len(), "jobs": ids.len(), "job_ids": format!("{}..={}", ids[0], ids[ids.len() - 1]) }));
        }
    }
    let p = env.pool().await;
    assert_eq!(p, reference);
    assert_eq!((p.first_id, p.records.len()), (44, 256));
    assert_eq!(p.completed_units, vec![20]);
    assert!(p.records.iter().all(|c| matches!(c.status, Status::Completed | Status::Cancelled)));
    let ids = job_ids(&env.exec).await;
    assert_eq!(ids, (44..300).collect::<Vec<u32>>());
    let pages = json!({
        "get_commitments(0, 50)": "44..=93",
        "get_commitments(44, 50)": "44..=93",
        "get_commitments(290, 50)": "290..=299",
        "get_commitments(300, 50)": "empty",
        "get_commitments(100, 100)": "100..=149 (limit capped at 50)",
    });
    assert_eq!(page(&env, 0, 50).await, (44..94).collect::<Vec<u32>>());
    assert_eq!(page(&env, 44, 50).await, (44..94).collect::<Vec<u32>>());
    assert_eq!(page(&env, 290, 50).await, (290..300).collect::<Vec<u32>>());
    assert_eq!(page(&env, 300, 50).await, Vec::<u32>::new());
    assert_eq!(page(&env, 100, 100).await, (100..150).collect::<Vec<u32>>());
    // Pruned id 0: NotFound in the lock, forgotten by the executor; both
    // contracts byte-identical.
    let (raw, raw_exec) = (env.raw().await, env.raw_exec().await);
    let mut pruned = vec![];
    let now = env.now().await;
    for (signer, method, args, core) in [
        (&env.actor, "dispatch", json!({ "id": 0 }), reference.clone().dispatch(a, 0, now)),
        (&env.actor, "cancel", json!({ "id": 0 }), reference.clone().cancel(a, 0, now)),
        (env.exec.as_account(), "settle", json!({ "id": 0, "completed": true }), reference.clone().settle(&x, 0, true)),
        (&env.mallory, "reconcile", json!({ "id": 0 }), Err(Error::NotFound)),
    ] {
        assert_eq!(core, Err(Error::NotFound), "core {method}(0)");
        let (o, r) = fails_r(signer, env.lock.id(), method, args, "NotFound").await;
        assert!(receipts_on(&r, env.exec.id()).is_empty(), "{method}(0) called the executor");
        assert_eq!((env.raw().await, env.raw_exec().await), (raw.clone(), raw_exec.clone()), "{method}(0) changed storage");
        pruned.push(json!({ "call": format!("pool.{method}(0)"), "signer": signer.id(), "observed_panic": o, "storage_unchanged": true }));
    }
    for (method, args, msg) in [
        ("finish", json!({ "id": 0, "completed": true }), "unknown job"),
        ("confirm_not_executed", json!({ "id": 0, "recipe": 0, "units": 1 }), "stale id"),
    ] {
        let o = fails_containing(&env.owner, env.exec.id(), method, args, msg).await;
        assert_eq!((env.raw().await, env.raw_exec().await), (raw.clone(), raw_exec.clone()), "{method}(0) changed storage");
        pruned.push(json!({ "call": format!("exec.{method}(0)"), "signer": env.owner.id(), "expected": msg, "observed_panic": o, "storage_unchanged": true }));
    }
    let status: String = env.exec.view("status").args_json(json!({ "id": 0 })).await.unwrap().json().unwrap();
    assert_eq!(status, "unknown");
    pruned.push(json!({ "call": "exec.status(0) (view)", "result": status }));
    same(&env, &reference, "after pruned-id calls").await;
    record(
        "RT-WINDOW",
        json!({
            "property": "RT-WINDOW", "also": "EX-7 (checked after every transaction)", "finding": "F-3 (fixed in v1.1.0)", "result": "pass",
            "port_of": "upstream crates/sandbox/tests/integration.rs window_of_300_records_through_the_executor",
            "records": 300, "first_id": p.first_id, "retained": p.records.len(), "completed_units": p.completed_units,
            "executor_jobs": ids.len(), "executor_job_ids": "44..=299", "max_jobs_seen": ex7.max_jobs, "ex7_checks": ex7.checks,
            "checkpoints": checkpoints, "pages": pages, "pruned_id_0": pruned, "seconds": start.elapsed().as_secs(),
            "final": summary(&p),
        }),
    );
}

// ------------------------------------------------------------------- RT-D1

/// PROP: RT-D1 EX-7 EX-8. D-1 (owner decision Q4) on the real runtime
/// (upstream integration.rs `confirmed_failed_executes_keep_the_executor_at_256_jobs`,
/// ported with the core compared after every transaction, EX-7 checked after
/// every transaction, and an operator account apart from the owner): 300
/// records whose `execute` fails, each confirmed not executed by the
/// operator. Every confirmation is three receipts that all succeed (the
/// call, the lock's settle, on_settled), and on_settled raises `newest`, so
/// the executor holds exactly the 256 newest jobs at upstream's ten
/// checkpoints. Then: a forgotten id is "stale id" / "unknown job"; a refused
/// confirmation (an unassigned id) keeps its job, its on_settled runs and
/// changes nothing, and the transaction reports the lock's NotFound;
/// on_settled is private; a Reserved record confirmed too early is refused
/// InvalidState, its job pre-cancels the id at dispatch, and
/// `finish(300, false)` then forgets job 44; the next record runs end to end.
#[tokio::test]
async fn rt_d1_confirmed_failed_executes_keep_256_jobs() {
    let env = Env::bare().await;
    let operator = subaccount(&env.worker, "operator", 20).await;
    let cfg = window_config("confirmations", env.owner.id(), env.actor.id(), env.exec.id());
    let env = env.init_with(&cfg, operator.id()).await;
    let a = env.actor.id().as_str();
    let x = env.exec.id().as_str().to_string();
    // The owner is not the operator.
    fails_containing(&env.owner, env.exec.id(), "set_mode", json!({ "mode": "panic" }), "only independent operator").await;
    ok(&operator, env.exec.id(), "set_mode", json!({ "mode": "panic" })).await;
    let mut reference = env.pool().await;
    let mut ex7 = Ex7::new(256);
    let mut checkpoints = vec![];
    let mut last = None;
    let start = Instant::now();
    for id in 0..300u32 {
        assert_eq!(env.submit_ok(&mut reference, &env.actor, 0, 1).await, id);
        // execute fails (panic mode), so on_result marks the record Unknown.
        let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
        assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![false], vec![true, true]), "dispatch {id}");
        if id == 0 {
            let failed = receipts_on(&r, env.exec.id())[0];
            expect_panic(&format!("{:?}", failed.clone().into_result().unwrap_err()), "injected failure", false);
        }
        reference.dispatch(a, id, env.now().await).unwrap();
        reference.acknowledge(id, false).unwrap();
        same(&env, &reference, &format!("dispatch {id}")).await;
        let (_, st) = ex7.check(&env.worker, &env.lock, &env.exec, &format!("dispatch {id}")).await;
        assert_eq!(st.newest, id.saturating_sub(1), "a failed execute moves nothing");
        let r = ok(&operator, env.exec.id(), "confirm_not_executed", json!({ "id": id, "recipe": 0, "units": 1 })).await;
        assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]), "confirm {id}");
        reference.settle(&x, id, false).unwrap();
        same(&env, &reference, &format!("confirm {id}")).await;
        let (first_id, st) = ex7.check(&env.worker, &env.lock, &env.exec, &format!("confirm {id}")).await;
        assert_eq!(st.newest, id, "on_settled raised newest");
        if CHECKPOINTS.contains(&id) {
            let held: Vec<u32> = st.jobs.iter().map(|j| j.id).collect();
            assert_eq!(held, (id.saturating_sub(255)..=id).collect::<Vec<u32>>(), "executor jobs after the confirmation of {id}");
            checkpoints.push(json!({ "confirmation": id, "jobs": held.len(), "job_ids": format!("{}..={}", held[0], held[held.len() - 1]), "newest": st.newest, "first_id": first_id }));
        }
        last = Some(r);
    }
    assert_eq!(ex7.max_jobs, 256);
    let p = env.pool().await;
    assert_eq!(p, reference);
    assert_eq!((p.first_id, p.records.len()), (44, 256));
    assert!(p.records.iter().all(|c| c.status == Status::Cancelled));
    let all = env.jobs().await;
    assert!(all.iter().all(|j| j.status == "cancelled"));
    let window: Vec<u32> = (44..300).collect();
    assert_eq!(job_ids(&env.exec).await, window);
    let last = last.unwrap();
    let last_shape = shape(&last, env.exec.id(), env.lock.id());
    let mut steps = vec![];
    ex7.bound = 512;

    // Forgotten id 43: stale for a confirmation, unknown for finish.
    let (raw, raw_exec) = (env.raw().await, env.raw_exec().await);
    let o1 = fails_containing(&operator, env.exec.id(), "confirm_not_executed", json!({ "id": 43, "recipe": 0, "units": 1 }), "stale id").await;
    let o2 = fails_containing(&operator, env.exec.id(), "finish", json!({ "id": 43, "completed": false }), "unknown job").await;
    assert_eq!((env.raw().await, env.raw_exec().await), (raw, raw_exec));
    steps.push(json!({ "step": "operator confirm_not_executed(43), finish(43, false)", "observed_panic": [o1, o2], "storage_unchanged": true }));

    // A refused confirmation (an unassigned id): the transaction fails with
    // the lock's error; on_settled still runs, changes nothing, and the job
    // stays (no rollback).
    let raw = env.raw().await;
    let r = tx(&operator, env.exec.id(), "confirm_not_executed", json!({ "id": 10_000, "recipe": 0, "units": 1 })).await;
    let sh = shape(&r, env.exec.id(), env.lock.id());
    assert_eq!(sh, (vec![true, true], vec![false]));
    let text = failure_text(&r);
    assert!(text.contains("Smart contract panicked: NotFound"), "refused confirmation: {text}");
    let o = expect_panic(&text, "NotFound", true);
    let mut held = window.clone();
    held.push(10_000);
    assert_eq!(job_ids(&env.exec).await, held);
    assert_eq!(env.newest().await, 299, "a refused settle moves nothing");
    assert_eq!(env.raw().await, raw);
    same(&env, &reference, "refused confirmation").await;
    ex7.check(&env.worker, &env.lock, &env.exec, "refused confirmation").await;
    steps.push(json!({ "step": "operator confirm_not_executed(10000) (unassigned id)", "receipts_ok": {"executor": sh.0, "lock": sh.1}, "observed_panic": o, "job_kept": true, "newest": 299, "lock_storage_unchanged": true }));

    // on_settled is private, and needs the settle's result. An id of 600
    // would forget every job below 345 if either call went through.
    let raw_exec = env.raw_exec().await;
    let o1 = fails(&env.owner, env.exec.id(), "on_settled", json!({ "id": 600 }), "Method on_settled is private").await;
    let o2 = fails(&operator, env.exec.id(), "on_settled", json!({ "id": 600 }), "Method on_settled is private").await;
    let o3 = fails_containing(env.exec.as_account(), env.exec.id(), "on_settled", json!({ "id": 600 }), "not a settle callback").await;
    assert_eq!(env.raw_exec().await, raw_exec);
    assert_eq!(job_ids(&env.exec).await, held);
    steps.push(json!({ "step": "on_settled({id: 600}) by the owner, the operator, the executor's own key", "observed_panic": [o1, o2, o3], "executor_storage_unchanged": true }));

    // A Reserved record confirmed too early: the lock refuses (InvalidState),
    // the job stays and pre-cancels the id.
    ok(&operator, env.exec.id(), "set_mode", json!({ "mode": "normal" })).await;
    assert_eq!(env.submit_ok(&mut reference, &env.actor, 0, 1).await, 300);
    let raw = env.raw().await;
    let r = tx(&operator, env.exec.id(), "confirm_not_executed", json!({ "id": 300, "recipe": 0, "units": 1 })).await;
    let sh = shape(&r, env.exec.id(), env.lock.id());
    assert_eq!(sh, (vec![true, true], vec![false]));
    let o = expect_panic(&failure_text(&r), "InvalidState", true);
    assert_eq!(env.raw().await, raw);
    same(&env, &reference, "early confirmation").await;
    held.push(300);
    assert_eq!(job_ids(&env.exec).await, held, "job 300 pushed, 44 held");
    ex7.check(&env.worker, &env.lock, &env.exec, "early confirmation").await;
    steps.push(json!({ "step": "submit 300; operator confirm_not_executed(300) while Reserved", "receipts_ok": {"executor": sh.0, "lock": sh.1}, "observed_panic": o, "job_kept": true }));
    // Dispatch: execute answers "cancelled" from job 300, so the lock
    // cancels the record. An execute that finds a stored job moves no newest
    // id, so job 44 stays.
    let r = ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": 300 })).await;
    assert_eq!(event(&r, env.lock.id(), "reconciled"), json!({ "id": 300, "outcome": "cancelled", "error": null }));
    reference.dispatch(a, 300, env.now().await).unwrap();
    reference.settle(&x, 300, false).unwrap();
    same(&env, &reference, "dispatch 300").await;
    assert_eq!(env.status(300).await, Status::Cancelled);
    assert_eq!(job_ids(&env.exec).await, held);
    assert_eq!(env.newest().await, 299);
    ex7.check(&env.worker, &env.lock, &env.exec, "dispatch 300").await;
    steps.push(json!({ "step": "actor dispatch(300)", "execute_returns": "cancelled (stored job)", "status": "Cancelled", "newest": 299, "job_44_held": true }));
    // finish(300, false) is now accepted (a same-outcome no-op at the lock);
    // its on_settled raises the newest id to 300 and forgets job 44.
    let v = env.pool().await.version;
    let r = ok(&operator, env.exec.id(), "finish", json!({ "id": 300, "completed": false })).await;
    assert_eq!(shape(&r, env.exec.id(), env.lock.id()), (vec![true, true], vec![true]));
    reference.settle(&x, 300, false).unwrap();
    same(&env, &reference, "finish 300").await;
    assert_eq!(env.pool().await.version, v, "same-outcome settle is a no-op");
    let mut held: Vec<u32> = (45..300).collect();
    held.extend([10_000, 300]);
    assert_eq!(job_ids(&env.exec).await, held, "finish moved the newest id");
    assert_eq!(env.newest().await, 300);
    ex7.check(&env.worker, &env.lock, &env.exec, "finish 300").await;
    steps.push(json!({ "step": "operator finish(300, false)", "receipts_ok": {"executor": [true, true], "lock": [true]}, "newest": 300, "job_44_forgotten": true }));
    // Not wedged: the next record runs end to end.
    assert_eq!(env.submit_ok(&mut reference, &env.actor, 0, 1).await, 301);
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": 301 })).await;
    reference.dispatch(a, 301, env.now().await).unwrap();
    reference.acknowledge(301, true).unwrap();
    same(&env, &reference, "dispatch 301").await;
    let mut held: Vec<u32> = (46..300).collect();
    held.extend([10_000, 300, 301]);
    assert_eq!(job_ids(&env.exec).await, held, "execute forgot job 45");
    ex7.check(&env.worker, &env.lock, &env.exec, "dispatch 301").await;
    ok(&operator, env.exec.id(), "finish", json!({ "id": 301, "completed": true })).await;
    reference.settle(&x, 301, true).unwrap();
    let p = env.pool().await;
    assert_eq!(p, reference);
    assert_eq!(p.record(301).unwrap().status, Status::Completed);
    assert_eq!((p.first_id, p.completed_units.clone()), (46, vec![1]));
    let (_, st) = ex7.check(&env.worker, &env.lock, &env.exec, "finish 301").await;
    steps.push(json!({ "step": "submit, dispatch, finish(301, true)", "status": "Completed", "first_id": p.first_id, "completed_units": p.completed_units, "executor_jobs": st.jobs.len(), "newest": st.newest }));
    record(
        "RT-D1",
        json!({
            "property": "RT-D1", "also": "EX-7 (checked after every transaction), EX-8 (on_settled on the real runtime)", "decision": "D-1 (owner decision Q4)", "result": "pass",
            "port_of": "upstream crates/sandbox/tests/integration.rs confirmed_failed_executes_keep_the_executor_at_256_jobs (operator kept apart from the owner here)",
            "confirmations": 300, "max_jobs_during_confirmations": 256, "ex7_checks": ex7.checks,
            "checkpoints": checkpoints,
            "confirmation_299_receipts_ok": {"executor": last_shape.0, "lock": last_shape.1},
            "steps": steps, "seconds": start.elapsed().as_secs(),
            "final": summary(&p),
        }),
    );
}

// ----------------------------------------------------------------- SB-1

/// Finding SB-1 (v1.0.0; upstream NT-F1), fixed in v1.1.0 and inverted here:
/// the documented build (upstream scripts/build.sh, which now sets
/// RUSTFLAGS="--cfg near") reports every panic message. v1.0.0's build
/// turned each into a bare `unreachable` trap. Each core error text arrives
/// (Unauthorized, NotFound, Bounds, Pending), as do the SDK's and the
/// executor's messages; none of the failures is a bare trap.
#[tokio::test]
async fn sb1_documented_build_reports_panic_messages() {
    let env = Env::bare().await;
    let cfg = basic_config(env.owner.id(), &[(env.actor.id(), env.exec.id())]);
    let mut seen = serde_json::Map::new();
    let mut check = |key: &str, r: &near_workspaces::result::ExecutionFinalResult, msg: &str, exact: bool| {
        let text = failure_text(r);
        assert!(!text.contains(TRAP), "{key}: a bare trap: {text}");
        seen.insert(key.into(), json!(expect_panic(&text, msg, exact)));
    };
    check("pool.new by mallory", &tx(&env.mallory, env.lock.id(), "new", json!({ "config": cfg })).await, "Method new is private", true);
    let env = env.init(&cfg).await;
    check("pool.set_paused by mallory (core Unauthorized)", &tx(&env.mallory, env.lock.id(), "set_paused", json!({ "paused": true })).await, "Unauthorized", true);
    check("pool.dispatch(99) (core NotFound)", &tx(&env.actor, env.lock.id(), "dispatch", json!({ "id": 99 })).await, "NotFound", true);
    check("pool.observe [50, 1000001] (core Bounds)", &tx(&env.owner, env.lock.id(), "observe", json!({ "values": [50, 1_000_001] })).await, "Bounds", true);
    check("pool.on_result by mallory", &tx(&env.mallory, env.lock.id(), "on_result", json!({ "id": 0 })).await, "Method on_result is private", true);
    check("exec.finish by mallory", &tx(&env.mallory, env.exec.id(), "finish", json!({ "id": 0, "completed": true })).await, "only independent operator", false);
    check("exec.on_settled by mallory", &tx(&env.mallory, env.exec.id(), "on_settled", json!({ "id": 0 })).await, "Method on_settled is private", true);
    // An Active record: cancel and a changed observe are Pending.
    let mut reference = env.pool().await;
    let id = env.submit_ok(&mut reference, &env.actor, 0, 1).await;
    ok(&env.actor, env.lock.id(), "dispatch", json!({ "id": id })).await;
    check("pool.cancel of an Active record (core Pending)", &tx(&env.actor, env.lock.id(), "cancel", json!({ "id": id })).await, "Pending", true);
    check("pool.observe [50, 1] with a record open (core Pending)", &tx(&env.owner, env.lock.id(), "observe", json!({ "values": [50, 1] })).await, "Pending", true);
    check("pool.reconcile with 1 yoctoNEAR (NT-F2)", &tx_deposit(&env.mallory, env.lock.id(), "reconcile", json!({ "id": id }), 1).await, "Method reconcile doesn't accept deposit", true);
    record(
        "SB-1",
        json!({ "finding": "SB-1 (v1.0.0; upstream NT-F1)", "summary": "v1.1.0's scripts/build.sh sets RUSTFLAGS=\"--cfg near\", so near-sdk's panic_str calls panic_utf8 and every on-chain failure carries its message",
            "result": "fixed: no bare trap; every failure reads Smart contract panicked: <msg>",
            "build": "upstream scripts/build.sh (results/wasm.json)", "pool_contract_sha256": sha256_hex(&contract_wasm()), "pool_executor_sha256": sha256_hex(&executor_wasm()),
            "observed": seen }),
    );
}
