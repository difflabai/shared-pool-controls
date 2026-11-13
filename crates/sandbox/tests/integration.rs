//! Real Wasm/receipt integration. Run scripts/sandbox.sh after building both Wasm files.
use near_workspaces::{types::NearToken, Account, Contract};
use pool_core::{fixtures, Config, Metric, Pool, Recipe, Request, Shape, Status, Term};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn wasm(name: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/wasm32-unknown-unknown/release/{name}.wasm"
    )))
    .expect("run scripts/build.sh first")
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
async fn state(c: &Contract) -> Pool {
    c.view("get_pool").await.unwrap().json().unwrap()
}
async fn call(a: &Account, c: &Contract, method: &str, args: Value) -> Value {
    let result = a
        .call(c.id(), method)
        .args_json(args)
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success(), "{method}: {result:?}");
    result.json().unwrap_or(Value::Null)
}
/// The call must fail with a panic message that contains `expected`. A Wasm
/// built without `--cfg near` traps with no message, so this also checks the build.
async fn fail(a: &Account, c: &Contract, method: &str, args: Value, expected: &str) {
    fail_paying(a, c, method, args, NearToken::from_yoctonear(0), expected).await
}
/// `fail` with a deposit attached.
async fn fail_paying(
    a: &Account,
    c: &Contract,
    method: &str,
    args: Value,
    deposit: NearToken,
    expected: &str,
) {
    let r = a
        .call(c.id(), method)
        .args_json(args)
        .deposit(deposit)
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(r.is_failure(), "expected {method} to fail");
    let text = r.into_result().unwrap_err().to_string();
    assert!(
        text.contains("Smart contract panicked: ") && text.contains(expected),
        "{method}: expected a panic with {expected:?}, got {text}"
    );
}
#[tokio::test]
async fn five_scenarios_execute_through_the_actual_gate() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let owner = worker.dev_create_account().await.unwrap();
    for name in fixtures::NAMES {
        let lock = worker.dev_deploy(&wasm("pool_contract")).await.unwrap();
        let executor = worker.dev_deploy(&wasm("pool_executor")).await.unwrap();
        let mut cfg = fixtures::config(name).unwrap();
        cfg.owner = owner.id().to_string();
        let mut actors = BTreeMap::new();
        for r in &mut cfg.recipes {
            let original = r.actor.clone();
            if !actors.contains_key(&original) {
                actors.insert(original.clone(), worker.dev_create_account().await.unwrap());
            }
            r.actor = actors[&original].id().to_string();
            r.executor = executor.id().to_string();
        }
        call(lock.as_account(), &lock, "new", json!({"config":cfg})).await;
        call(
            executor.as_account(),
            &executor,
            "new",
            json!({"lock":lock.id(),"operator":owner.id()}),
        )
        .await;
        let actor_for = |id: &str| actors.values().find(|a| a.id().as_str() == id).unwrap();
        let mut reference = state(&lock).await;
        for (r, u) in fixtures::trace(name) {
            let req = Request {
                recipe: r,
                units: u,
                version: reference.version,
                expires_at: now() + 3600,
            };
            let actor = actor_for(&reference.config.recipes[r as usize].actor);
            let expected = reference.preview(actor.id().as_str(), &req, now());
            let preview: pool_core::Decision = lock
                .view("preview")
                .args_json(json!({"actor":actor.id(),"request":req}))
                .await
                .unwrap()
                .json()
                .unwrap();
            assert_eq!(preview, expected);
            let result = call(actor, &lock, "submit", json!({"request":req})).await;
            assert_eq!(result["decision"], serde_json::to_value(&expected).unwrap());
            if expected.allowed {
                let id = reference.submit(actor.id().as_str(), req, now()).unwrap();
                assert_eq!(result["id"], id);
                // A bypass attempt cannot start execution.
                fail(
                    actor,
                    &executor,
                    "execute",
                    json!({"id":id,"recipe":r,"units":u}),
                    "only lock may execute",
                )
                .await;
                fail(
                    actor,
                    &lock,
                    "settle",
                    json!({"id":id,"completed":true}),
                    "Unauthorized",
                )
                .await;
                call(actor, &lock, "dispatch", json!({"id":id})).await;
                reference.dispatch(actor.id().as_str(), id, now()).unwrap();
                reference.acknowledge(id, true).unwrap();
                assert_eq!(state(&lock).await, reference);
                call(
                    &owner,
                    &executor,
                    "finish",
                    json!({"id":id,"completed":true}),
                )
                .await;
                reference.settle(executor.id().as_str(), id, true).unwrap();
                // Same final evidence is idempotent.
                call(
                    &owner,
                    &executor,
                    "finish",
                    json!({"id":id,"completed":true}),
                )
                .await;
            }
            assert_eq!(state(&lock).await, reference, "scenario {name}, recipe {r}");
        }
        println!(
            "PASS {name}: energy {}, {} commitments",
            reference.energy(&reference.values).unwrap(),
            reference.records.len()
        );
        fail(
            &owner,
            &lock,
            "on_result",
            json!({"id":0}),
            "Method on_result is private",
        )
        .await;
    }
}
#[tokio::test]
async fn unknown_outcomes_stale_requests_and_reconciliation() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let owner = worker.dev_create_account().await.unwrap();
    let actor = worker.dev_create_account().await.unwrap();
    let lock = worker.dev_deploy(&wasm("pool_contract")).await.unwrap();
    let executor = worker.dev_deploy(&wasm("pool_executor")).await.unwrap();
    let mut cfg = fixtures::config("gpu").unwrap();
    cfg.owner = owner.id().to_string();
    for r in &mut cfg.recipes {
        r.actor = actor.id().to_string();
        r.executor = executor.id().to_string();
    }
    call(lock.as_account(), &lock, "new", json!({"config":cfg})).await;
    call(
        executor.as_account(),
        &executor,
        "new",
        json!({"lock":lock.id(),"operator":owner.id()}),
    )
    .await;
    let req = json!({"recipe":0,"units":4,"version":0,"expires_at":now()+3600});
    assert_eq!(
        call(&actor, &lock, "submit", json!({"request":req})).await["id"],
        0
    );
    assert_eq!(
        call(&actor, &lock, "submit", json!({"request":req})).await["decision"]["reason"],
        "Stale"
    );
    call(&owner, &executor, "set_mode", json!({"mode":"unknown"})).await;
    call(&actor, &lock, "dispatch", json!({"id":0})).await;
    assert_eq!(state(&lock).await.records[0].status, Status::Unknown);
    fail(&actor, &lock, "cancel", json!({"id":0}), "Pending").await;
    assert_eq!(state(&lock).await.values[3], 4);
    // NT-F2: reconcile is nonpayable like every other mutating method.
    fail_paying(
        &actor,
        &lock,
        "reconcile",
        json!({"id":0}),
        NearToken::from_yoctonear(1),
        "Method reconcile doesn't accept deposit",
    )
    .await;
    assert_eq!(state(&lock).await.records[0].status, Status::Unknown);
    call(&actor, &lock, "reconcile", json!({"id":0})).await;
    assert_eq!(state(&lock).await.records[0].status, Status::Active);
    call(
        &owner,
        &executor,
        "finish",
        json!({"id":0,"completed":false}),
    )
    .await;
    assert_eq!(state(&lock).await.values[3], 0);
    // Execution itself fails; a failed receipt does not free the reservation.
    call(&owner, &executor, "set_mode", json!({"mode":"panic"})).await;
    let p = state(&lock).await;
    call(
        &actor,
        &lock,
        "submit",
        json!({"request":{"recipe":0,"units":2,"version":p.version,"expires_at":now()+3600}}),
    )
    .await;
    // The callback succeeds even though the execute receipt fails, so inspect final state.
    let _ = actor
        .call(lock.id(), "dispatch")
        .args_json(json!({"id":1}))
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert_eq!(state(&lock).await.records[1].status, Status::Unknown);
    assert_eq!(state(&lock).await.values[3], 2);
    call(
        &owner,
        &executor,
        "confirm_not_executed",
        json!({"id":1,"recipe":0,"units":2}),
    )
    .await;
    assert_eq!(state(&lock).await.values[3], 0);
    fail(
        &actor,
        &lock,
        "observe",
        json!({"values":[0,0,0,0,0]}),
        "Unauthorized",
    )
    .await;
}
/// F-3: 300 records dispatched and settled end to end. The lock keeps the
/// 256 most recent (first_id 44) and the executor at most 256 jobs. In v1.0.0
/// the 257th record could not be admitted, and the executor would have
/// panicked "full" at the 257th id. A pruned id is NotFound in the lock and
/// forgotten by the executor. Most records run in reject mode, where the
/// execute result settles them through on_result; ids 0-9 and 290-299 run
/// in normal mode and the operator finishes them.
#[tokio::test]
async fn window_of_300_records_through_the_executor() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let owner = worker.dev_create_account().await.unwrap();
    let actor = worker.dev_create_account().await.unwrap();
    let lock = worker.dev_deploy(&wasm("pool_contract")).await.unwrap();
    let executor = worker.dev_deploy(&wasm("pool_executor")).await.unwrap();
    // One record holds one unit of a 10-unit capacity until it ends either way.
    let cfg = Config {
        name: "window".into(),
        owner: owner.id().to_string(),
        metrics: vec![Metric {
            name: "In flight".into(),
            unit: "jobs".into(),
            initial: 0,
            min: 0,
            max: 10,
        }],
        terms: vec![Term {
            metric: 0,
            target: 10,
            weight: 1,
            shape: Shape::Excess,
        }],
        recipes: vec![Recipe {
            name: "job".into(),
            actor: actor.id().to_string(),
            executor: executor.id().to_string(),
            reserve: vec![1],
            complete: vec![-1],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    call(lock.as_account(), &lock, "new", json!({"config":cfg})).await;
    call(
        executor.as_account(),
        &executor,
        "new",
        json!({"lock":lock.id(),"operator":owner.id()}),
    )
    .await;
    let mut reference = state(&lock).await;
    let x = executor.id().as_str();
    let mut mode = "normal";
    for id in 0..300u32 {
        let normal = !(10..290).contains(&id);
        let wanted = if normal { "normal" } else { "reject" };
        if mode != wanted {
            call(&owner, &executor, "set_mode", json!({"mode":wanted})).await;
            mode = wanted;
        }
        let req = Request {
            recipe: 0,
            units: 1,
            version: reference.version,
            expires_at: now() + 3600,
        };
        let result = call(&actor, &lock, "submit", json!({"request":req})).await;
        assert_eq!(result["id"], id, "{result}");
        reference.submit(actor.id().as_str(), req, now()).unwrap();
        call(&actor, &lock, "dispatch", json!({"id":id})).await;
        reference.dispatch(actor.id().as_str(), id, now()).unwrap();
        if normal {
            reference.acknowledge(id, true).unwrap();
            call(
                &owner,
                &executor,
                "finish",
                json!({"id":id,"completed":true}),
            )
            .await;
            reference.settle(x, id, true).unwrap();
        } else {
            reference.settle(x, id, false).unwrap();
        }
        if id % 50 == 0 || id == 256 {
            assert_eq!(state(&lock).await, reference, "record {id}");
        }
    }
    let p = state(&lock).await;
    assert_eq!(p, reference);
    assert_eq!((p.first_id, p.records.len()), (44, 256));
    assert_eq!(p.completed_units, vec![20]);
    assert!(p
        .records
        .iter()
        .all(|c| matches!(c.status, Status::Completed | Status::Cancelled)));
    let jobs: Vec<Value> = executor.view("get_jobs").await.unwrap().json().unwrap();
    let ids: Vec<u64> = jobs.iter().map(|j| j["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, (44..300).collect::<Vec<u64>>());
    let page = |from: u32| {
        let lock = &lock;
        async move {
            let v: Vec<Value> = lock
                .view("get_commitments")
                .args_json(json!({"from":from,"limit":50}))
                .await
                .unwrap()
                .json()
                .unwrap();
            v.iter()
                .map(|c| c["id"].as_u64().unwrap())
                .collect::<Vec<u64>>()
        }
    };
    assert_eq!(page(0).await, (44..94).collect::<Vec<u64>>());
    assert_eq!(page(290).await, (290..300).collect::<Vec<u64>>());
    // Pruned id 0: NotFound in the lock, forgotten by the executor.
    fail(&actor, &lock, "reconcile", json!({"id":0}), "NotFound").await;
    fail(&actor, &lock, "cancel", json!({"id":0}), "NotFound").await;
    fail(
        &owner,
        &executor,
        "finish",
        json!({"id":0,"completed":true}),
        "unknown job",
    )
    .await;
    fail(
        &owner,
        &executor,
        "confirm_not_executed",
        json!({"id":0,"recipe":0,"units":1}),
        "stale id",
    )
    .await;
    let status: String = executor
        .view("status")
        .args_json(json!({"id":0}))
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status, "unknown");
    assert_eq!(state(&lock).await, reference);
    println!(
        "PASS window: first_id {}, {} records, {} executor jobs",
        p.first_id,
        p.records.len(),
        ids.len()
    );
}
/// The executor's job ids, in the order it holds them.
async fn job_ids(executor: &Contract) -> Vec<u64> {
    let jobs: Vec<Value> = executor.view("get_jobs").await.unwrap().json().unwrap();
    jobs.iter().map(|j| j["id"].as_u64().unwrap()).collect()
}
/// D-1 (owner decision Q4): 300 records whose `execute` fails, each confirmed
/// not executed by the operator. The lock accepts each confirmation's settle,
/// and the executor's private callback `on_settled` then moves its newest id,
/// so it holds at most 256 jobs. Before this change it forgot nothing without
/// a successful `execute`: 257 jobs after the confirmation of id 256, and the
/// 512-job cap after 512. Then, on the real host: a refused confirmation
/// keeps its job and the transaction reports the lock's error; `on_settled`
/// is private; a Reserved record confirmed too early is cancelled when it is
/// dispatched, after which `finish(id, false)` moves the newest id; the pool
/// is not wedged.
#[tokio::test]
async fn confirmed_failed_executes_keep_the_executor_at_256_jobs() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let owner = worker.dev_create_account().await.unwrap();
    let actor = worker.dev_create_account().await.unwrap();
    let lock = worker.dev_deploy(&wasm("pool_contract")).await.unwrap();
    let executor = worker.dev_deploy(&wasm("pool_executor")).await.unwrap();
    // One record holds one unit of a 10-unit capacity until it ends either way.
    let cfg = Config {
        name: "confirmations".into(),
        owner: owner.id().to_string(),
        metrics: vec![Metric {
            name: "In flight".into(),
            unit: "jobs".into(),
            initial: 0,
            min: 0,
            max: 10,
        }],
        terms: vec![Term {
            metric: 0,
            target: 10,
            weight: 1,
            shape: Shape::Excess,
        }],
        recipes: vec![Recipe {
            name: "job".into(),
            actor: actor.id().to_string(),
            executor: executor.id().to_string(),
            reserve: vec![1],
            complete: vec![-1],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    call(lock.as_account(), &lock, "new", json!({"config":cfg})).await;
    call(
        executor.as_account(),
        &executor,
        "new",
        json!({"lock":lock.id(),"operator":owner.id()}),
    )
    .await;
    call(&owner, &executor, "set_mode", json!({"mode":"panic"})).await;
    let mut reference = state(&lock).await;
    let (a, x) = (actor.id().as_str(), executor.id().as_str());
    let mut last = None;
    for id in 0..300u32 {
        let req = Request {
            recipe: 0,
            units: 1,
            version: reference.version,
            expires_at: now() + 3600,
        };
        let result = call(&actor, &lock, "submit", json!({"request":req})).await;
        assert_eq!(result["id"], id, "{result}");
        reference.submit(a, req, now()).unwrap();
        // execute fails (panic mode), so on_result marks the record Unknown.
        call(&actor, &lock, "dispatch", json!({"id":id})).await;
        reference.dispatch(a, id, now()).unwrap();
        reference.acknowledge(id, false).unwrap();
        let r = owner
            .call(executor.id(), "confirm_not_executed")
            .args_json(json!({"id":id,"recipe":0,"units":1}))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(r.is_success(), "confirm {id}: {:?}", r.failures());
        reference.settle(x, id, false).unwrap();
        if [0, 50, 100, 150, 200, 250, 255, 256, 257, 299].contains(&id) {
            // The bound: the newest id is `id`, so at most 256 jobs remain.
            let held = job_ids(&executor).await;
            assert_eq!(
                held,
                (u64::from(id.saturating_sub(255))..=u64::from(id)).collect::<Vec<u64>>(),
                "executor jobs after the confirmation of {id}: {} jobs",
                held.len()
            );
            assert_eq!(state(&lock).await, reference, "record {id}");
        }
        last = Some(r);
    }
    let p = state(&lock).await;
    assert_eq!(p, reference);
    assert_eq!((p.first_id, p.records.len()), (44, 256));
    assert!(p.records.iter().all(|c| c.status == Status::Cancelled));
    let jobs: Vec<Value> = executor.view("get_jobs").await.unwrap().json().unwrap();
    assert!(jobs.iter().all(|j| j["status"] == "cancelled"));
    let window: Vec<u64> = (44..300).collect();
    assert_eq!(job_ids(&executor).await, window);
    // The confirmation of 299: the executor's call and its on_settled, and
    // the lock's settle, all succeed.
    let r = last.unwrap();
    let on = |account: &near_workspaces::AccountId| {
        r.receipt_outcomes()
            .iter()
            .filter(|o| &o.executor_id == account)
            .map(|o| o.is_success())
            .collect::<Vec<bool>>()
    };
    assert_eq!(
        (on(executor.id()), on(lock.id())),
        (vec![true, true], vec![true])
    );
    // Forgotten id 43: stale for a confirmation, unknown for finish.
    fail(
        &owner,
        &executor,
        "confirm_not_executed",
        json!({"id":43,"recipe":0,"units":1}),
        "stale id",
    )
    .await;
    fail(
        &owner,
        &executor,
        "finish",
        json!({"id":43,"completed":false}),
        "unknown job",
    )
    .await;
    // A refused confirmation (an unassigned id): the transaction fails with
    // the lock's error; on_settled still runs, changes nothing, and the job
    // stays (no rollback).
    let r = owner
        .call(executor.id(), "confirm_not_executed")
        .args_json(json!({"id":10_000,"recipe":0,"units":1}))
        .max_gas()
        .transact()
        .await
        .unwrap();
    let on = |account: &near_workspaces::AccountId| {
        r.receipt_outcomes()
            .iter()
            .filter(|o| &o.executor_id == account)
            .map(|o| o.is_success())
            .collect::<Vec<bool>>()
    };
    assert_eq!(
        (on(executor.id()), on(lock.id())),
        (vec![true, true], vec![false])
    );
    let text = r.into_result().unwrap_err().to_string();
    assert!(
        text.contains("Smart contract panicked: NotFound"),
        "refused confirmation: {text}"
    );
    let mut held = window.clone();
    held.push(10_000);
    assert_eq!(job_ids(&executor).await, held);
    assert_eq!(state(&lock).await, reference);
    // on_settled is private, and needs the settle's result. An id of 600
    // would forget every job below 345 if either call went through.
    fail(
        &owner,
        &executor,
        "on_settled",
        json!({"id":600}),
        "Method on_settled is private",
    )
    .await;
    fail(
        executor.as_account(),
        &executor,
        "on_settled",
        json!({"id":600}),
        "not a settle callback",
    )
    .await;
    assert_eq!(job_ids(&executor).await, held);
    // A Reserved record confirmed too early: the lock refuses (InvalidState),
    // the job stays and pre-cancels the id.
    call(&owner, &executor, "set_mode", json!({"mode":"normal"})).await;
    let req = Request {
        recipe: 0,
        units: 1,
        version: reference.version,
        expires_at: now() + 3600,
    };
    let result = call(&actor, &lock, "submit", json!({"request":req})).await;
    assert_eq!(result["id"], 300, "{result}");
    reference.submit(a, req, now()).unwrap();
    fail(
        &owner,
        &executor,
        "confirm_not_executed",
        json!({"id":300,"recipe":0,"units":1}),
        "InvalidState",
    )
    .await;
    assert_eq!(state(&lock).await, reference);
    held.push(300);
    assert_eq!(job_ids(&executor).await, held, "job 300 pushed, 44 held");
    // Dispatch: execute answers "cancelled" from job 300, so the lock
    // cancels the record.
    call(&actor, &lock, "dispatch", json!({"id":300})).await;
    reference.dispatch(a, 300, now()).unwrap();
    reference.settle(x, 300, false).unwrap();
    let p = state(&lock).await;
    assert_eq!(p, reference);
    assert_eq!(p.record(300).unwrap().status, Status::Cancelled);
    assert_eq!(job_ids(&executor).await, held);
    // finish(300, false) is now accepted (a same-outcome no-op at the lock);
    // its on_settled raises the newest id to 300 and forgets job 44.
    call(
        &owner,
        &executor,
        "finish",
        json!({"id":300,"completed":false}),
    )
    .await;
    reference.settle(x, 300, false).unwrap();
    assert_eq!(state(&lock).await, reference);
    let mut held: Vec<u64> = (45..300).collect();
    held.extend([10_000, 300]);
    assert_eq!(job_ids(&executor).await, held, "finish moved the newest id");
    // Not wedged: the next record runs end to end.
    let req = Request {
        recipe: 0,
        units: 1,
        version: reference.version,
        expires_at: now() + 3600,
    };
    let result = call(&actor, &lock, "submit", json!({"request":req})).await;
    assert_eq!(result["id"], 301, "{result}");
    reference.submit(a, req, now()).unwrap();
    call(&actor, &lock, "dispatch", json!({"id":301})).await;
    reference.dispatch(a, 301, now()).unwrap();
    reference.acknowledge(301, true).unwrap();
    assert_eq!(state(&lock).await, reference);
    let mut held: Vec<u64> = (46..300).collect();
    held.extend([10_000, 300, 301]);
    assert_eq!(job_ids(&executor).await, held, "execute forgot job 45");
    call(
        &owner,
        &executor,
        "finish",
        json!({"id":301,"completed":true}),
    )
    .await;
    reference.settle(x, 301, true).unwrap();
    let p = state(&lock).await;
    assert_eq!(p, reference);
    assert_eq!(p.record(301).unwrap().status, Status::Completed);
    assert_eq!((p.first_id, p.completed_units.clone()), (46, vec![1]));
    println!(
        "PASS confirmed failed executes: first_id {}, {} records, at most 256 executor jobs from confirmations, {} jobs at the end",
        p.first_id,
        p.records.len(),
        held.len()
    );
}
/// A Wasm executor whose `execute` returns the JSON number `5` and whose
/// `status` returns a JSON string of 70 bytes. Neither is one JSON string of
/// at most 64 bytes. Hand-assembled: one import (`env.value_return`), one page
/// of memory, two exports, two data segments.
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
/// NT-F4 on the real host: an executor result that is not one JSON string of
/// at most 64 bytes leaves the record Unknown, and the callback does not fail.
#[tokio::test]
async fn non_string_executor_results_leave_the_record_unknown() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let owner = worker.dev_create_account().await.unwrap();
    let actor = worker.dev_create_account().await.unwrap();
    let lock = worker.dev_deploy(&wasm("pool_contract")).await.unwrap();
    let executor = worker.dev_deploy(&non_string_executor()).await.unwrap();
    // The executor returns these exact bytes; it does not fail.
    let long = format!("\"{}\"", "a".repeat(68));
    for (method, value) in [("execute", "5"), ("status", long.as_str())] {
        let r = executor.view(method).await.unwrap();
        assert_eq!(r.result, value.as_bytes(), "{method}");
    }
    let mut cfg = fixtures::config("gpu").unwrap();
    cfg.owner = owner.id().to_string();
    for r in &mut cfg.recipes {
        r.actor = actor.id().to_string();
        r.executor = executor.id().to_string();
    }
    call(lock.as_account(), &lock, "new", json!({"config":cfg})).await;
    let mut reference = state(&lock).await;
    let req = Request {
        recipe: 0,
        units: 4,
        version: 0,
        expires_at: now() + 3600,
    };
    assert_eq!(
        call(&actor, &lock, "submit", json!({"request":req})).await["id"],
        0
    );
    reference.submit(actor.id().as_str(), req, now()).unwrap();
    let reconciled = |r: &near_workspaces::result::ExecutionFinalResult| -> Vec<Value> {
        r.logs()
            .iter()
            .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
            .map(|j| serde_json::from_str::<Value>(j).unwrap())
            .filter(|e| e["event"] == "reconciled")
            .map(|e| e["data"][0].clone())
            .collect()
    };
    let expected = vec![json!({"id":0,"outcome":"unknown","error":null})];
    // execute returns 5.
    let r = actor
        .call(lock.id(), "dispatch")
        .args_json(json!({"id":0}))
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(r.is_success(), "dispatch: {:?}", r.failures());
    assert_eq!(reconciled(&r), expected);
    reference.dispatch(actor.id().as_str(), 0, now()).unwrap();
    reference.acknowledge(0, false).unwrap();
    assert_eq!(state(&lock).await, reference);
    assert_eq!(reference.records[0].status, Status::Unknown);
    // status returns a 70-byte string: still Unknown, no version change.
    let r = actor
        .call(lock.id(), "reconcile")
        .args_json(json!({"id":0}))
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(r.is_success(), "reconcile: {:?}", r.failures());
    assert_eq!(reconciled(&r), expected);
    assert_eq!(state(&lock).await, reference);
    // The reservation is held until the executor account reports.
    call(
        executor.as_account(),
        &lock,
        "settle",
        json!({"id":0,"completed":false}),
    )
    .await;
    reference.settle(executor.id().as_str(), 0, false).unwrap();
    assert_eq!(state(&lock).await, reference);
    assert_eq!(reference.records[0].status, Status::Cancelled);
    println!("PASS non-string results: 5 and a 70-byte string leave the record Unknown");
}
