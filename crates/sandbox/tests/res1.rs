//! RES-1: gas at the largest state, measured on the Wasm that
//! `scripts/build.sh` writes. Slow (about eight minutes), so `scripts/sandbox.sh`
//! skips it. Run it after `scripts/build.sh` with
//! `cargo test -p pool-sandbox --locked --test res1 -- --ignored --nocapture`.
//! It writes `artifacts/res1.json`. The figures in docs/api.md come from it.
//!
//! Adapted from the outside verification's RES-1 sandbox test, which ran on
//! v1.0.0. The configuration has every size at its limit: 32 metrics with
//! 128-byte names and units, 64 terms, 32 recipes with 128-byte names, 64-byte
//! actor and executor ids, full effect vectors, recipe i requires every j < i.
//! The state is built with real transactions only: 31 Completed
//! prerequisites, then recipe-31 reservations of 100 units up to 256 records,
//! then 31 more submits that each prune a Completed front record, so the pool
//! retains 256 open records and the next submit is `Full`.
//!
//! At that state it measures the gas burnt by each receipt, and the least
//! whole Tgas of prepaid gas for `dispatch`, `reconcile`, and the executor's
//! `finish` and `confirm_not_executed`. Every smaller amount from 40 Tgas up
//! must fail with "Exceeded the prepaid gas." in its first receipt, run
//! nothing on the other contract, log nothing, and leave both contracts' raw
//! storage byte-identical and the pool equal to the host-side core.
//!
//! Then it fills the executor to its 512-job cap and measures `status`,
//! `finish` and `execute` there.
#![allow(clippy::too_many_arguments)]

use near_workspaces::network::Sandbox;
use near_workspaces::result::{ExecutionFinalResult, ExecutionOutcome};
use near_workspaces::types::NearToken;
use near_workspaces::{Account, AccountId, Contract, Worker};
use pool_core::{Config, Metric, Pool, Recipe, Request, Shape, Status, Term, MAX_RECORDS};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

const TGAS: u64 = 1_000_000_000_000;
const TX_GAS_LIMIT: u64 = 300 * TGAS;
const OUT_OF_GAS: &str = "Exceeded the prepaid gas.";
const N_METRICS: usize = 32;
const N_RECIPES: usize = 32;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn wasm(name: &str) -> Vec<u8> {
    std::fs::read(root().join(format!("target/wasm32-unknown-unknown/release/{name}.wasm")))
        .expect("run scripts/build.sh first")
}

fn tgas(g: u64) -> f64 {
    (g as f64 / TGAS as f64 * 100.0).round() / 100.0
}

async fn subaccount(worker: &Worker<Sandbox>, name: &str, near: u128) -> Account {
    worker
        .root_account()
        .unwrap()
        .create_subaccount(name)
        .initial_balance(NearToken::from_near(near))
        .transact()
        .await
        .unwrap()
        .into_result()
        .unwrap()
}

async fn deploy(worker: &Worker<Sandbox>, name: &str, near: u128, code: &[u8]) -> Contract {
    subaccount(worker, name, near)
        .await
        .deploy(code)
        .await
        .unwrap()
        .into_result()
        .unwrap()
}

async fn block_now(worker: &Worker<Sandbox>) -> u64 {
    worker.view_block().await.unwrap().timestamp() / 1_000_000_000
}

async fn tx(a: &Account, receiver: &AccountId, method: &str, args: Value) -> ExecutionFinalResult {
    a.call(receiver, method)
        .args_json(args)
        .max_gas()
        .transact()
        .await
        .unwrap_or_else(|e| panic!("{method}: rpc error {e:?}"))
}

async fn tx_gas(
    a: &Account,
    receiver: &AccountId,
    method: &str,
    args: Value,
    gas: u64,
) -> ExecutionFinalResult {
    a.call(receiver, method)
        .args_json(args)
        .gas(near_workspaces::types::Gas::from_gas(gas))
        .transact()
        .await
        .unwrap_or_else(|e| panic!("{method} with {gas} gas: rpc error {e:?}"))
}

/// The final result must be a success (a failed inner receipt is allowed).
async fn ok(a: &Account, receiver: &AccountId, method: &str, args: Value) -> ExecutionFinalResult {
    let r = tx(a, receiver, method, args).await;
    assert!(r.is_success(), "{method} failed: {:?}", r.failures());
    r
}

async fn pool(c: &Contract) -> Pool {
    c.view("get_pool").await.unwrap().json().unwrap()
}

async fn jobs(exec: &Contract) -> Vec<Value> {
    exec.view("get_jobs").await.unwrap().json().unwrap()
}

async fn raw_state(worker: &Worker<Sandbox>, id: &AccountId) -> HashMap<Vec<u8>, Vec<u8>> {
    worker.view_state(id).await.unwrap()
}

fn events(logs: &[String]) -> Vec<Value> {
    logs.iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| serde_json::from_str(j).unwrap())
        .collect()
}

fn receipts_on<'a>(r: &'a ExecutionFinalResult, account: &AccountId) -> Vec<&'a ExecutionOutcome> {
    r.receipt_outcomes()
        .iter()
        .filter(|o| &o.executor_id == account)
        .collect()
}

/// The `reconciled` events logged on `account`.
fn reconciled(r: &ExecutionFinalResult, account: &AccountId) -> Vec<Value> {
    let logs: Vec<String> = receipts_on(r, account)
        .iter()
        .flat_map(|o| o.logs.clone())
        .collect();
    events(&logs)
        .into_iter()
        .filter(|e| e["event"] == "reconciled")
        .map(|e| e["data"][0].clone())
        .collect()
}

fn long(c: char, len: usize, tag: usize) -> String {
    let t = tag.to_string();
    format!("{}{}", c.to_string().repeat(len - t.len()), t)
}

fn max_config(owner: &AccountId, actor: &AccountId, executor: &AccountId) -> Config {
    let metrics = (0..N_METRICS)
        .map(|i| Metric {
            name: long('M', 128, i),
            unit: long('U', 128, i),
            initial: 0,
            min: -1_000_000,
            max: 1_000_000,
        })
        .collect();
    let mut terms = vec![];
    for i in 0..N_METRICS as u32 {
        terms.push(Term {
            metric: i,
            target: 0,
            weight: 1000,
            shape: Shape::Target,
        });
        terms.push(Term {
            metric: i,
            target: 0,
            weight: 1000,
            shape: if i % 2 == 0 {
                Shape::Excess
            } else {
                Shape::Deficit
            },
        });
    }
    let recipes = (0..N_RECIPES)
        .map(|i| Recipe {
            name: long('R', 128, i),
            actor: actor.to_string(),
            executor: executor.to_string(),
            reserve: vec![0; N_METRICS],
            complete: (0..N_METRICS)
                .map(|j| if (i + j) % 2 == 0 { 1 } else { -1 })
                .collect(),
            max_units: 100,
            requires: (0..i as u32).map(|j| (j, 1)).collect(),
        })
        .collect();
    Config {
        name: long('N', 128, 0),
        owner: owner.to_string(),
        metrics,
        terms,
        recipes,
        max_age_seconds: 86_400,
    }
}

struct Gas {
    rows: Vec<Value>,
}

impl Gas {
    /// Records the gas burnt by each receipt of `r` on `on` and the
    /// transaction total; returns the largest receipt. `budget`: receipt
    /// index i (in the order of `on`) must burn less than the static gas b.
    fn add(
        &mut self,
        what: &str,
        r: &ExecutionFinalResult,
        on: &[(&AccountId, &str)],
        budget: Option<(usize, u64)>,
    ) -> u64 {
        let mut receipts = vec![];
        for (account, label) in on {
            for o in receipts_on(r, account) {
                receipts.push(json!({ "on": label, "gas_tgas": tgas(o.gas_burnt.as_gas()), "gas": o.gas_burnt.as_gas(), "ok": o.is_success() }));
            }
        }
        let max = receipts
            .iter()
            .map(|v| v["gas"].as_u64().unwrap())
            .max()
            .unwrap_or(0);
        let total = r.total_gas_burnt.as_gas();
        let mut row = json!({ "call": what, "tx_total_tgas": tgas(total), "receipts": receipts, "margin_to_300_tgas": tgas(TX_GAS_LIMIT - total) });
        if let Some((i, b)) = budget {
            let g = row["receipts"][i]["gas"].as_u64().unwrap();
            row["attached_budget_tgas"] = json!(b / TGAS);
            row["margin_to_budget_tgas"] = json!(tgas(b.saturating_sub(g)));
            assert!(g < b, "{what}: receipt {i} burnt {g} of its {b} budget");
        }
        assert!(total < TX_GAS_LIMIT, "{what}: {total} gas");
        eprintln!("RES-1 {row}");
        self.rows.push(row);
        max
    }
}

/// `actor` submits recipe 31 x 100 at the reference's version; must be
/// admitted; applied to the reference. Returns (id, outcome).
async fn submit_open(
    actor: &Account,
    lock: &Contract,
    reference: &mut Pool,
    expires_at: u64,
    now: u64,
    curve: &mut Vec<(usize, u32, u64)>,
) -> (u32, ExecutionFinalResult) {
    let req = Request {
        recipe: 31,
        units: 100,
        version: reference.version,
        expires_at,
    };
    let r = ok(actor, lock.id(), "submit", json!({ "request": req })).await;
    let v: Value = r.clone().json().unwrap();
    assert_eq!(
        v["decision"]["allowed"],
        true,
        "submit at {} records: {v}",
        reference.records.len()
    );
    let id = reference.submit(actor.id().as_str(), req, now).unwrap();
    assert_eq!(v["id"], id);
    let open = reference
        .records
        .iter()
        .filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled))
        .count();
    curve.push((
        reference.records.len(),
        open as u32,
        receipts_on(&r, lock.id())[0].gas_burnt.as_gas(),
    ));
    (id, r)
}

/// One call with `tgas_prepaid` Tgas. On success returns the outcome. On
/// failure checks that it ran out of gas in its first receipt (on
/// `receiver`) and that nothing else happened; returns the row.
async fn attempt(
    worker: &Worker<Sandbox>,
    signer: &Account,
    receiver: &AccountId,
    method: &str,
    args: &Value,
    tgas_prepaid: u64,
    lock: &Contract,
    exec: &Contract,
    reference: &Pool,
    id: u32,
    status: Status,
) -> Result<ExecutionFinalResult, Value> {
    let (pool_raw, exec_raw) = (
        raw_state(worker, lock.id()).await,
        raw_state(worker, exec.id()).await,
    );
    let r = tx_gas(signer, receiver, method, args.clone(), tgas_prepaid * TGAS).await;
    if r.is_success() {
        return Ok(r);
    }
    let what = format!("{method} {args} with {tgas_prepaid} Tgas");
    let failures = r.failures();
    assert_eq!(failures.len(), 1, "{what}: {failures:?}");
    let f = failures[0];
    assert_eq!(
        &f.executor_id, receiver,
        "{what}: failed on {}",
        f.executor_id
    );
    let text = format!("{:?}", r.clone().into_result().unwrap_err());
    assert!(
        text.contains(OUT_OF_GAS),
        "{what}: not an out-of-gas failure: {text}"
    );
    assert_eq!(
        receipts_on(&r, receiver).len(),
        1,
        "{what}: more than one receipt on {receiver}"
    );
    for other in [lock.id(), exec.id()]
        .into_iter()
        .filter(|o| *o != receiver)
    {
        assert!(
            receipts_on(&r, other).is_empty(),
            "{what}: a receipt ran on {other}"
        );
    }
    let logs: Vec<String> = r.logs().iter().map(|l| l.to_string()).collect();
    assert!(
        logs.is_empty() && events(&logs).is_empty(),
        "{what}: log or event from a failed call: {logs:?}"
    );
    assert_eq!(
        raw_state(worker, lock.id()).await,
        pool_raw,
        "{what}: pool storage changed"
    );
    assert_eq!(
        raw_state(worker, exec.id()).await,
        exec_raw,
        "{what}: executor storage changed"
    );
    let p = pool(lock).await;
    assert_eq!(&p, reference, "{what}: pool differs from the reference");
    assert_eq!(p.record(id).unwrap().status, status, "{what}");
    Err(json!({
        "call": method, "args": args, "prepaid_tgas": tgas_prepaid, "ok": false, "failure": OUT_OF_GAS,
        "failed_receipt_on": receiver, "failed_receipt_burnt_tgas": tgas(f.gas_burnt.as_gas()),
        "receipts_on_other_contract": 0, "events": 0, "pool_storage_unchanged": true, "executor_storage_unchanged": true,
        "record_status_after": format!("{status:?}"),
    }))
}

/// The smallest whole-Tgas prepaid gas, counting up from `from`, for which
/// the call succeeds; every smaller amount must fail as `attempt` checks.
async fn min_prepaid(
    worker: &Worker<Sandbox>,
    signer: &Account,
    receiver: &AccountId,
    method: &str,
    args: Value,
    from: u64,
    lock: &Contract,
    exec: &Contract,
    reference: &Pool,
    id: u32,
    status: Status,
    rows: &mut Vec<Value>,
) -> (u64, ExecutionFinalResult) {
    let mut g = from;
    loop {
        assert!(
            g * TGAS <= TX_GAS_LIMIT,
            "{method} {args}: no success up to 300 Tgas"
        );
        match attempt(
            worker,
            signer,
            receiver,
            method,
            &args,
            g,
            lock,
            exec,
            reference,
            id,
            status.clone(),
        )
        .await
        {
            Ok(r) => {
                eprintln!(
                    "RES-1 {method} {args}: smallest whole-Tgas prepaid gas that succeeds: {g}"
                );
                return (g, r);
            }
            Err(row) => rows.push(row),
        }
        g += 1;
    }
}

fn open_count(p: &Pool) -> usize {
    p.records
        .iter()
        .filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled))
        .count()
}

#[tokio::test]
#[ignore = "slow (about eight minutes); run with --ignored after scripts/build.sh"]
async fn res_1_gas_at_max_state() {
    let t0 = Instant::now();
    let worker = near_workspaces::sandbox()
        .await
        .expect("start near-sandbox");
    let owner = subaccount(&worker, &"o".repeat(54), 40).await;
    let actor = subaccount(&worker, &"a".repeat(54), 60).await;
    let lock = deploy(&worker, "pool", 50, &wasm("pool_contract")).await;
    let exec = deploy(&worker, &"x".repeat(54), 50, &wasm("pool_executor")).await;
    assert_eq!(
        (actor.id().as_str().len(), exec.id().as_str().len()),
        (64, 64)
    );
    let cfg = max_config(owner.id(), actor.id(), exec.id());
    cfg.validate().expect("max config validates");
    let r_new = ok(
        lock.as_account(),
        lock.id(),
        "new",
        json!({ "config": cfg }),
    )
    .await;
    let _ = ok(
        exec.as_account(),
        exec.id(),
        "new",
        json!({ "lock": lock.id(), "operator": owner.id() }),
    )
    .await;
    let mut reference: Pool = pool(&lock).await;
    assert_eq!(
        reference,
        Pool::new(cfg.clone(), reference.observed_at).unwrap()
    );
    let (a, x, o) = (
        actor.id().to_string(),
        exec.id().to_string(),
        owner.id().to_string(),
    );
    let now = block_now(&worker).await;
    let expires_at = now + 3 * 3600;
    let pool_on = [(lock.id(), "pool")];
    let both = [(lock.id(), "pool"), (exec.id(), "exec")];
    let mut gas = Gas { rows: vec![] };
    let mut curve: Vec<(usize, u32, u64)> = vec![];
    let mut below = vec![];
    let mut minimum = vec![];
    gas.add("new (max config)", &r_new, &pool_on, None);

    // ---- A. 31 Completed prerequisites (recipe i x 1), then open
    // recipe-31 reservations of 100 units up to 255 records.
    for i in 0..31u32 {
        let req = Request {
            recipe: i,
            units: 1,
            version: reference.version,
            expires_at,
        };
        let r = ok(&actor, lock.id(), "submit", json!({ "request": req })).await;
        let v: Value = r.clone().json().unwrap();
        assert_eq!(v["decision"]["allowed"], true, "prerequisite {i}: {v}");
        let id = reference.submit(&a, req, now).unwrap();
        curve.push((
            reference.records.len(),
            1,
            receipts_on(&r, lock.id())[0].gas_burnt.as_gas(),
        ));
        let _ = ok(&actor, lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(&a, id, now).unwrap();
        reference.acknowledge(id, true).unwrap();
        let _ = ok(
            &owner,
            exec.id(),
            "finish",
            json!({ "id": id, "completed": true }),
        )
        .await;
        reference.settle(&x, id, true).unwrap();
    }
    assert_eq!(pool(&lock).await, reference, "after prerequisites");
    assert_eq!(&reference.completed_units[..31], &[1u32; 31][..]);
    while reference.records.len() < MAX_RECORDS - 1 {
        let _ = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    }
    assert_eq!(pool(&lock).await, reference, "at 255 records");
    eprintln!("255 records in {:?}", t0.elapsed());

    // ---- B. Admissions.
    let req = Request {
        recipe: 31,
        units: 100,
        version: reference.version,
        expires_at,
    };
    let r = ok(
        &actor,
        lock.id(),
        "preview",
        json!({ "actor": actor.id(), "request": req }),
    )
    .await;
    let d: Value = r.clone().json().unwrap();
    assert_eq!(d["allowed"], true, "{d}");
    gas.add(
        "preview at 255 records, 224 open (as a transaction)",
        &r,
        &pool_on,
        None,
    );
    // #256: 224 open records in the envelope, no prune (v1.0.0's worst case).
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (255, 0));
    gas.add(
        "submit #256 (224 open records, no prune)",
        &r,
        &pool_on,
        None,
    );
    // #257: the oldest record (id 0) is Completed, so this submit prunes it.
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!(
        (id, reference.first_id, reference.records.len()),
        (256, 1, 256)
    );
    gas.add(
        "submit #257 (225 open records, prunes id 0)",
        &r,
        &pool_on,
        None,
    );
    assert_eq!(pool(&lock).await, reference, "after the first prune");
    while reference.first_id < 30 {
        let _ = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    }
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (286, 31));
    let submit_max = gas.add(
        "submit #287 (255 open records, prunes the last Completed record)",
        &r,
        &pool_on,
        None,
    );
    assert_eq!(open_count(&reference), 256);
    assert_eq!(pool(&lock).await, reference, "256 open records");
    // Full: the oldest retained record (id 31) is open.
    let req = Request {
        recipe: 31,
        units: 100,
        version: reference.version,
        expires_at,
    };
    let r = ok(&actor, lock.id(), "submit", json!({ "request": req })).await;
    let v: Value = r.clone().json().unwrap();
    assert_eq!(
        (v["decision"]["reason"].as_str(), v["id"].is_null()),
        (Some("Full"), true),
        "{v}"
    );
    gas.add(
        "submit #288 (rejected Full: the oldest record is open)",
        &r,
        &pool_on,
        None,
    );
    assert_eq!(pool(&lock).await, reference, "after Full");

    // Reads.
    let r = ok(&actor, lock.id(), "get_pool", json!({})).await;
    let json_len = r.clone().into_result().unwrap().raw_bytes().unwrap().len();
    let get_pool_tx = gas.add("get_pool (as a transaction)", &r, &pool_on, None);
    let r = ok(
        &actor,
        lock.id(),
        "get_commitments",
        json!({ "from": 0, "limit": 1000 }),
    )
    .await;
    let page: Vec<Value> = r.clone().json().unwrap();
    assert_eq!(
        (page.len(), page[0]["id"].as_u64(), page[49]["id"].as_u64()),
        (50, Some(31), Some(80))
    );
    gas.add(
        "get_commitments(0, 1000) -> ids 31..80 (as a transaction)",
        &r,
        &pool_on,
        None,
    );
    let state_bytes = raw_state(&worker, lock.id())
        .await
        .get(b"STATE".as_slice())
        .map(Vec::len)
        .unwrap();
    let storage_usage = lock.view_account().await.unwrap().storage_usage;

    // ---- C. Owner calls and a release at 256 open records.
    for paused in [true, false] {
        let r = ok(&owner, lock.id(), "set_paused", json!({ "paused": paused })).await;
        reference.set_paused(&o, paused).unwrap();
        gas.add(
            &format!("set_paused({paused}), 256 open records"),
            &r,
            &pool_on,
            None,
        );
    }
    let r = ok(
        &owner,
        lock.id(),
        "observe",
        json!({ "values": reference.values }),
    )
    .await;
    let chain = pool(&lock).await;
    reference
        .observe(&o, reference.values.clone(), chain.observed_at)
        .unwrap();
    assert_eq!(chain, reference, "after the refresh");
    gas.add(
        "observe (restates the values), 256 open records",
        &r,
        &pool_on,
        None,
    );
    // cancel the oldest record: its release is checked against 255 other open records.
    let r = ok(&actor, lock.id(), "cancel", json!({ "id": 31 })).await;
    reference.cancel(&a, 31, now).unwrap();
    gas.add(
        "cancel (release; 255 other open records)",
        &r,
        &pool_on,
        None,
    );
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!(
        (id, reference.first_id, open_count(&reference)),
        (287, 32, 256)
    );
    gas.add(
        "submit #288 after the cancel (255 open records, prunes id 31)",
        &r,
        &pool_on,
        None,
    );
    assert_eq!(pool(&lock).await, reference, "256 open records again");

    // ---- D. Prepaid gas sweeps at 256 open records.
    assert_eq!(
        jobs(&exec).await.len(),
        31,
        "executor jobs before the dispatch sweep"
    );
    let (ra, rb, rc) = (287u32, 286u32, 285u32);
    let (min_dispatch, r) = min_prepaid(
        &worker,
        &actor,
        lock.id(),
        "dispatch",
        json!({ "id": ra }),
        40,
        &lock,
        &exec,
        &reference,
        ra,
        Status::Reserved,
        &mut below,
    )
    .await;
    reference.dispatch(&a, ra, now).unwrap();
    reference.acknowledge(ra, true).unwrap();
    assert_eq!(
        pool(&lock).await,
        reference,
        "after dispatch at the minimum"
    );
    let dispatch_burnt = receipts_on(&r, lock.id())[0].gas_burnt.as_gas();
    gas.add(
        &format!("dispatch #{ra} with {min_dispatch} Tgas prepaid (the minimum)"),
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    minimum.push(json!({
        "call": "dispatch", "state": "pool 256 records, all open (Reserved); executor 31 jobs",
        "min_prepaid_tgas": min_dispatch, "fails_at_tgas": format!("40..={}", min_dispatch - 1),
        "static_gas_attached_tgas": "execute 30 + on_result 30", "first_receipt_burnt_tgas": tgas(dispatch_burnt),
    }));
    for g in [40, min_dispatch - 1] {
        let row = attempt(
            &worker,
            &actor,
            lock.id(),
            "dispatch",
            &json!({ "id": rb }),
            g,
            &lock,
            &exec,
            &reference,
            rb,
            Status::Reserved,
        )
        .await
        .expect_err("dispatch below the minimum succeeded on a second record");
        below.push(row);
    }
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": rb })).await;
    reference.dispatch(&a, rb, now).unwrap();
    reference.acknowledge(rb, true).unwrap();
    gas.add(
        &format!("dispatch #{rb} with 300 Tgas after failed low-gas attempts"),
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let r = tx_gas(
        &actor,
        lock.id(),
        "dispatch",
        json!({ "id": rc }),
        min_dispatch * TGAS,
    )
    .await;
    assert!(
        r.is_success(),
        "dispatch #{rc} with {min_dispatch} Tgas: {:?}",
        r.failures()
    );
    reference.dispatch(&a, rc, now).unwrap();
    reference.acknowledge(rc, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add(
        &format!("dispatch #{rc} with {min_dispatch} Tgas prepaid (the minimum, again)"),
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );

    let (min_reconcile, r) = min_prepaid(
        &worker,
        &owner,
        lock.id(),
        "reconcile",
        json!({ "id": ra }),
        40,
        &lock,
        &exec,
        &reference,
        ra,
        Status::Active,
        &mut below,
    )
    .await;
    assert_eq!(
        pool(&lock).await,
        reference,
        "reconcile of an Active record is a no-op"
    );
    gas.add(
        &format!("reconcile #{ra} with {min_reconcile} Tgas prepaid (the minimum)"),
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    minimum.push(json!({
        "call": "reconcile", "state": "pool 256 records, all open, record Active; executor 3 jobs",
        "min_prepaid_tgas": min_reconcile, "fails_at_tgas": format!("40..={}", min_reconcile - 1),
        "static_gas_attached_tgas": "status 10 + on_result 30", "first_receipt_burnt_tgas": tgas(receipts_on(&r, lock.id())[0].gas_burnt.as_gas()),
    }));

    // on_result that releases (execute returns "cancelled"): checked against 255 other open records.
    let _ = ok(&owner, exec.id(), "set_mode", json!({ "mode": "reject" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 284 })).await;
    reference.dispatch(&a, 284, now).unwrap();
    reference.settle(&x, 284, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "on_result released 284");
    assert_eq!(
        reconciled(&r, lock.id()),
        vec![json!({ "id": 284, "outcome": "cancelled", "error": null })]
    );
    gas.add(
        "dispatch, execute returns cancelled, on_result releases (255 other open records)",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    // on_result for a failed execute: Unknown.
    let _ = ok(&owner, exec.id(), "set_mode", json!({ "mode": "panic" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 283 })).await;
    reference.dispatch(&a, 283, now).unwrap();
    reference.acknowledge(283, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "283 Unknown");
    gas.add(
        "dispatch, execute fails, on_result marks Unknown",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let _ = ok(&owner, exec.id(), "set_mode", json!({ "mode": "normal" })).await;

    // ---- E. The executor at its retention bound (256 jobs: ids 32..=287).
    for id in 32..282u32 {
        let _ = ok(&actor, lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(&a, id, now).unwrap();
        reference.acknowledge(id, true).unwrap();
        if id % 32 == 0 {
            assert_eq!(pool(&lock).await, reference, "at record {id}");
        }
    }
    assert_eq!(pool(&lock).await, reference, "254 jobs");
    assert_eq!(jobs(&exec).await.len(), 254);
    eprintln!("254 jobs in {:?}", t0.elapsed());
    // confirm_not_executed of 283 (Unknown, no job): 254 -> 255 jobs, then settle(false).
    let (min_confirm, r) = min_prepaid(
        &worker,
        &owner,
        exec.id(),
        "confirm_not_executed",
        json!({ "id": 283, "recipe": 31, "units": 100 }),
        40,
        &lock,
        &exec,
        &reference,
        283,
        Status::Unknown,
        &mut below,
    )
    .await;
    reference.settle(&x, 283, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "after confirm at the minimum");
    let exec_first = [(exec.id(), "exec"), (lock.id(), "pool")];
    gas.add(&format!("confirm_not_executed with {min_confirm} Tgas prepaid (the minimum) -> notify settle(false) [40 Tgas]"), &r, &exec_first, Some((1, 40 * TGAS)));
    minimum.push(json!({
        "call": "confirm_not_executed (executor)", "state": "executor 254 jobs (255 after); pool 256 records, 254 open, the record Unknown",
        "min_prepaid_tgas": min_confirm, "fails_at_tgas": format!("40..={}", min_confirm - 1),
        "static_gas_attached_tgas": "notify (pool.settle) 40", "first_receipt_burnt_tgas": tgas(receipts_on(&r, exec.id())[0].gas_burnt.as_gas()),
    }));
    // The 256th job.
    let last = 282u32;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": last })).await;
    reference.dispatch(&a, last, now).unwrap();
    reference.acknowledge(last, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add(
        "dispatch at the retention bound (exec.execute: 255 jobs -> 256)",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let execute_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    assert!(execute_256 < 30 * TGAS);
    let js = jobs(&exec).await;
    let mut ids: Vec<u64> = js.iter().map(|j| j["id"].as_u64().unwrap()).collect();
    assert_eq!(js.last().unwrap()["id"].as_u64(), Some(last as u64));
    ids.sort();
    assert_eq!(
        ids,
        (32..=287).collect::<Vec<u64>>(),
        "executor holds exactly the window"
    );
    let r = ok(&owner, lock.id(), "reconcile", json!({ "id": last })).await;
    assert_eq!(
        pool(&lock).await,
        reference,
        "reconcile of an Active record is a no-op"
    );
    gas.add(
        "reconcile at the retention bound (exec.status: 256 jobs, the job is last)",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let status_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    assert!(status_256 < 10 * TGAS);
    let (min_finish, r) = min_prepaid(
        &worker,
        &owner,
        exec.id(),
        "finish",
        json!({ "id": last, "completed": true }),
        40,
        &lock,
        &exec,
        &reference,
        last,
        Status::Active,
        &mut below,
    )
    .await;
    reference.settle(&x, last, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after finish at the minimum");
    gas.add(&format!("finish at the retention bound with {min_finish} Tgas prepaid (the minimum) -> notify settle(true) [40 Tgas]"), &r, &exec_first, Some((1, 40 * TGAS)));
    let finish_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    minimum.push(json!({
        "call": "finish (executor)", "state": "executor 256 jobs (the last job); pool 256 records, 253 open",
        "min_prepaid_tgas": min_finish, "fails_at_tgas": format!("40..={}", min_finish - 1),
        "static_gas_attached_tgas": "notify (pool.settle) 40", "first_receipt_burnt_tgas": tgas(finish_256),
    }));
    // settle both ways by the executor key.
    for (id, completed) in [(100u32, true), (101u32, false)] {
        let r = ok(
            exec.as_account(),
            lock.id(),
            "settle",
            json!({ "id": id, "completed": completed }),
        )
        .await;
        reference.settle(&x, id, completed).unwrap();
        gas.add(
            &format!("settle({completed}) by the executor key (about 250 other open records)"),
            &r,
            &pool_on,
            None,
        );
    }
    assert_eq!(pool(&lock).await, reference, "256 jobs, final");
    let exec_state = raw_state(&worker, exec.id())
        .await
        .get(b"STATE".as_slice())
        .map(Vec::len)
        .unwrap();

    // ---- F. The executor at its job cap (512). The operator confirms the
    // 256 ids 289..=544, above the newest executed id (287), that the lock has
    // not assigned yet. Each confirmation adds a "cancelled" job; its notify
    // fails NotFound on the lock, which changes nothing there. (Honest use
    // reaches this state only through 256 dispatches whose execute failed,
    // each confirmed by the operator; the executor's work is the same.)
    let mut confirm_512 = 0;
    for id in 289..=544u32 {
        let r = tx(
            &owner,
            exec.id(),
            "confirm_not_executed",
            json!({ "id": id, "recipe": 31, "units": 100 }),
        )
        .await;
        let (on_exec, on_lock) = (receipts_on(&r, exec.id()), receipts_on(&r, lock.id()));
        assert!(
            on_exec.len() == 1 && on_exec[0].is_success(),
            "confirm {id}: {:?}",
            r.failures()
        );
        assert!(
            on_lock.len() == 1 && on_lock[0].is_failure(),
            "confirm {id}: the lock's settle should fail"
        );
        let text = format!("{:?}", r.clone().into_result().unwrap_err());
        assert!(text.contains("NotFound"), "confirm {id}: {text}");
        if id == 544 {
            confirm_512 = on_exec[0].gas_burnt.as_gas();
            gas.add("confirm_not_executed of an unassigned id, 511 jobs -> 512 (the lock's settle fails NotFound)", &r, &exec_first, None);
        }
    }
    assert_eq!(jobs(&exec).await.len(), 512);
    assert_eq!(
        pool(&lock).await,
        reference,
        "the confirmations changed nothing on the lock"
    );
    eprintln!("512 jobs in {:?}", t0.elapsed());
    // One more confirmation fails "full" on the executor and never reaches the lock.
    let r = tx(
        &owner,
        exec.id(),
        "confirm_not_executed",
        json!({ "id": 545, "recipe": 31, "units": 100 }),
    )
    .await;
    let text = format!("{:?}", r.clone().into_result().unwrap_err());
    assert!(
        text.contains("full") && receipts_on(&r, lock.id()).is_empty(),
        "{text}"
    );
    assert_eq!(jobs(&exec).await.len(), 512);
    let exec_state_512 = raw_state(&worker, exec.id())
        .await
        .get(b"STATE".as_slice())
        .map(Vec::len)
        .unwrap();
    // status with 512 jobs: reconcile of an Active record changes nothing.
    let r = ok(&owner, lock.id(), "reconcile", json!({ "id": 281 })).await;
    assert_eq!(
        pool(&lock).await,
        reference,
        "reconcile at the job cap is a no-op"
    );
    gas.add(
        "reconcile at the job cap (exec.status: 512 jobs)",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let status_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    assert!(status_512 < 10 * TGAS);
    // finish with 512 jobs: the least prepaid gas.
    let (min_finish_512, r) = min_prepaid(
        &worker,
        &owner,
        exec.id(),
        "finish",
        json!({ "id": 281, "completed": true }),
        40,
        &lock,
        &exec,
        &reference,
        281,
        Status::Active,
        &mut below,
    )
    .await;
    reference.settle(&x, 281, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after finish at the job cap");
    gas.add(&format!("finish at the job cap with {min_finish_512} Tgas prepaid (the minimum) -> notify settle(true) [40 Tgas]"), &r, &exec_first, Some((1, 40 * TGAS)));
    let finish_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    minimum.push(json!({
        "call": "finish (executor) at the job cap", "state": "executor 512 jobs; pool 256 records, 251 open",
        "min_prepaid_tgas": min_finish_512, "fails_at_tgas": format!("40..={}", min_finish_512 - 1),
        "static_gas_attached_tgas": "notify (pool.settle) 40", "first_receipt_burnt_tgas": tgas(finish_512),
    }));
    // execute of a new id with 512 jobs: finish the front record (32), submit
    // (prunes it, id 288) and dispatch. execute forgets job 32 and adds 288.
    let _ = ok(
        &owner,
        exec.id(),
        "finish",
        json!({ "id": 32, "completed": true }),
    )
    .await;
    reference.settle(&x, 32, true).unwrap();
    let (id, _) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (288, 33));
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 288 })).await;
    reference.dispatch(&a, 288, now).unwrap();
    reference.acknowledge(288, true).unwrap();
    assert_eq!(
        pool(&lock).await,
        reference,
        "after dispatch at the job cap"
    );
    gas.add(
        "dispatch at the job cap (exec.execute: 512 jobs, forgets id 32, adds 288)",
        &r,
        &both,
        Some((1, 30 * TGAS)),
    );
    let execute_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    assert!(execute_512 < 30 * TGAS);
    let ids: Vec<u64> = jobs(&exec)
        .await
        .iter()
        .map(|j| j["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids.len(), 512);
    assert!(!ids.contains(&32) && ids.contains(&288));
    assert_eq!(pool(&lock).await, reference, "final");

    let worst = gas
        .rows
        .iter()
        .filter(|r| !r["call"].as_str().unwrap().starts_with("new"))
        .flat_map(|r| {
            r["receipts"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| x["on"] == "pool")
                .map(|x| x["gas"].as_u64().unwrap())
        })
        .max()
        .unwrap();
    let table: Vec<String> = minimum
        .iter()
        .map(|m| {
            format!(
                "{:<32} min {:>3} Tgas, fails at {:>8}, first receipt {:>5} Tgas, {}",
                m["call"].as_str().unwrap(),
                m["min_prepaid_tgas"],
                m["fails_at_tgas"].as_str().unwrap(),
                m["first_receipt_burnt_tgas"],
                m["state"].as_str().unwrap()
            )
        })
        .collect();
    eprintln!(
        "RES-1 minimum prepaid gas (whole Tgas):\n{}",
        table.join("\n")
    );
    for row in &gas.rows {
        let rs: Vec<String> = row["receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| format!("{} {}", r["on"].as_str().unwrap(), r["gas_tgas"]))
            .collect();
        eprintln!(
            "RES-1 {:<100} tx {:>6} Tgas | {}",
            row["call"].as_str().unwrap(),
            row["tx_total_tgas"],
            rs.join(", ")
        );
    }
    let curve_rows: Vec<Value> = curve
        .iter()
        .enumerate()
        .filter(|(i, _)| *i == 0 || *i % 32 == 31 || *i >= 254)
        .map(|(i, (n, open, g))| json!({ "submit": i + 1, "records_after": n, "open_after": open, "pool_receipt_tgas": tgas(*g) }))
        .collect();
    let out = json!({
        "property": "RES-1",
        "result": "pass",
        "build": std::fs::read(root().join("artifacts/build.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()),
        "state": {
            "metrics": 32, "terms": 64, "recipes": 32, "max_requires": 31, "records": 256, "open_at_full": 256,
            "state_bytes_at_full": state_bytes, "storage_usage_bytes_at_full": storage_usage, "get_pool_json_bytes_at_full": json_len,
            "executor_state_bytes_at_256_jobs": exec_state, "executor_state_bytes_at_512_jobs": exec_state_512, "built_by": "real transactions only",
        },
        "worst_pool_receipt_tgas": tgas(worst),
        "submit_287_pool_receipt_tgas": tgas(submit_max),
        "get_pool_tx_tgas": tgas(get_pool_tx),
        "minimum_prepaid_gas": minimum,
        "executor": {
            "execute_256th_tgas": tgas(execute_256), "status_at_256_tgas": tgas(status_256), "finish_last_of_256_tgas": tgas(finish_256),
            "confirm_511_to_512_tgas": tgas(confirm_512), "status_at_512_tgas": tgas(status_512), "finish_at_512_tgas": tgas(finish_512),
            "finish_min_prepaid_at_512_tgas": min_finish_512, "execute_at_512_tgas": tgas(execute_512),
        },
        "calls": gas.rows,
        "submit_gas_by_submit": curve_rows,
        "below_minimum_attempts": below,
        "seconds": t0.elapsed().as_secs(),
    });
    let text = serde_json::to_string_pretty(&out).unwrap() + "\n";
    let path = root().join("artifacts/res1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &text).unwrap();
    eprintln!("wrote {} after {:?}", path.display(), t0.elapsed());
}
