//! Real Wasm/receipt integration. Run scripts/sandbox.sh after building both Wasm files.
use near_workspaces::{types::NearToken, Account, Contract};
use pool_core::{fixtures, Pool, Request, Status};
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
