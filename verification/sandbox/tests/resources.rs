//! RES-1: gas at a maximum-size state, on the real wasm (upstream
//! scripts/build.sh's, via scripts/build-wasm.sh) in near-sandbox 2.13.4.
//!
//! v1.1.0 method: upstream crates/sandbox/tests/res1.rs at the pin, which
//! upstream adapted from this file's v1.0.0 version; ported back here with
//! formalver's two-test split and helpers. The acceptance thresholds are
//! upstream docs/api.md:44-59 (the gas table); upstream's own figures
//! (`artifacts/res1.json`, quoted there) are an informational cross-check.
//!
//! The state is reached through real transactions only (no state patching):
//! 32 metrics with 128-byte names and units, 64 terms, 32 recipes with
//! 128-byte names, 64-byte actor and executor ids, full 32-entry effect
//! vectors, recipe i requires every j < i. `build_max`: (A) 31 Completed
//! prerequisites (recipes 0..30 x 1), then recipe-31 reservations of 100
//! units up to 255 records; (B) the 256th submit (224 open records, no
//! prune), then 31 more submits that each prune a Completed front record, so
//! the pool retains 256 open records and the next submit is `Full`; reads;
//! (C) set_paused both ways, observe restating the values, cancel of the
//! oldest record and one more submit, so ids 32..=287 are retained, all open.
//!
//! `res_1_gas_at_max_state` records A-C and then measures, at 300 Tgas, the
//! receipts of dispatch, reconcile, a releasing and an Unknown on_result, the
//! executor's finish and confirm_not_executed (the call, the lock's settle,
//! then on_settled) and settles by the executor key.
//!
//! `res_1_min_prepaid_gas_and_executor_cap` runs upstream's D-F: the least
//! whole Tgas of prepaid gas for dispatch and reconcile (256 open records),
//! confirm_not_executed (executor at 254 jobs) and finish (256 jobs, then
//! 512, its cap, reached through 256 confirmations the lock refuses). Every
//! smaller amount from 40 Tgas up must fail with "Exceeded the prepaid gas."
//! in its first receipt, run nothing on the other contract, log nothing, and
//! leave both contracts' raw storage byte-identical and the pool equal to
//! the host-side core.
//!
//! Thresholds (docs/api.md:44-59), asserted: every pool receipt (other than
//! `new`) under 30 Tgas, the documented attach for submit, cancel, settle,
//! observe and set_paused, and the largest receipt of those five calls at
//! least 10 Tgas below it (docs/api.md:50: each Attach is at least 10 Tgas
//! above the least that succeeded; for calls without promises that is their
//! largest receipt); each least prepaid amount at least 10 Tgas below
//! its documented attach (dispatch 100, reconcile 60, finish and
//! confirm_not_executed 70); every measured on_settled under 6 Tgas
//! (res1.rs ON_SETTLED_LIMIT); each promise receipt under the gas attached
//! to it (execute 30, on_result 30, status 10, settle 40, on_settled 10).
#![allow(unused_must_use, clippy::too_many_arguments)]
mod common;

use common::*;
use near_workspaces::network::Sandbox;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::{Account, AccountId, Contract, Worker};
use pool_core::{Config, Metric, Pool, Recipe, Request, Shape, Status, Term, MAX_RECORDS};
use serde_json::{json, Value};
use std::time::Instant;

const N_METRICS: usize = 32;
const N_RECIPES: usize = 32;
/// Gas the executor attaches to its `on_settled`, and the most any measured
/// `on_settled` may burn (upstream res1.rs).
const ON_SETTLED_TGAS: u64 = 10;
const ON_SETTLED_LIMIT: u64 = 6 * TGAS;
/// docs/api.md's Attach column.
const ATTACH_DISPATCH: u64 = 100;
const ATTACH_RECONCILE: u64 = 60;
const ATTACH_EXECUTOR: u64 = 70;
const ATTACH_OTHER: u64 = 30;
/// docs/api.md:50: each Attach is at least this much above the least that
/// succeeded (for the calls without promises: their largest receipt).
const ATTACH_MARGIN_TGAS: u64 = 10;
/// docs/api.md's "Least that succeeded" column (informational).
const DOC_MIN_DISPATCH: u64 = 70;
const DOC_MIN_RECONCILE: u64 = 46;
const DOC_MIN_CONFIRM_254: u64 = 55;
const DOC_MIN_FINISH_256: u64 = 55;
const DOC_MIN_FINISH_512: u64 = 56;
const DOC_LARGEST_OTHER_TGAS: f64 = 14.81;

fn long(c: char, len: usize, tag: usize) -> String {
    let t = tag.to_string();
    format!("{}{}", c.to_string().repeat(len - t.len()), t)
}

fn max_config(owner: &AccountId, actor: &AccountId, executor: &AccountId) -> Config {
    let metrics = (0..N_METRICS)
        .map(|i| Metric { name: long('M', 128, i), unit: long('U', 128, i), initial: 0, min: -1_000_000, max: 1_000_000 })
        .collect();
    let mut terms = vec![];
    for i in 0..N_METRICS as u32 {
        terms.push(Term { metric: i, target: 0, weight: 1000, shape: Shape::Target });
        terms.push(Term { metric: i, target: 0, weight: 1000, shape: if i % 2 == 0 { Shape::Excess } else { Shape::Deficit } });
    }
    let recipes = (0..N_RECIPES)
        .map(|i| Recipe {
            name: long('R', 128, i),
            actor: actor.to_string(),
            executor: executor.to_string(),
            // Reservations are energy-neutral, so every one is admissible;
            // the checked arithmetic costs the same for any value.
            reserve: vec![0; N_METRICS],
            complete: (0..N_METRICS).map(|j| if (i + j) % 2 == 0 { 1 } else { -1 }).collect(),
            max_units: 100,
            requires: (0..i as u32).map(|j| (j, 1)).collect(),
        })
        .collect();
    Config { name: long('N', 128, 0), owner: owner.to_string(), metrics, terms, recipes, max_age_seconds: 86_400 }
}

struct Gas {
    rows: Vec<Value>,
}

impl Gas {
    /// Records the gas burnt by each receipt of `r` on `on` and the
    /// transaction total; returns the largest receipt. `budgets`: for each
    /// (i, b), receipt i (in the order of `on`) must burn less than the
    /// static gas b attached to it.
    fn add(&mut self, what: &str, r: &ExecutionFinalResult, on: &[(&AccountId, &str)], budgets: &[(usize, u64)]) -> u64 {
        let mut receipts = vec![];
        for (account, label) in on {
            for o in receipts_on(r, account) {
                receipts.push(json!({ "on": label, "gas_tgas": tgas(o.gas_burnt.as_gas()), "gas": o.gas_burnt.as_gas(), "ok": o.is_success() }));
            }
        }
        let max = receipts.iter().map(|v| v["gas"].as_u64().unwrap()).max().unwrap_or(0);
        let total = r.total_gas_burnt.as_gas();
        let mut row = json!({ "call": what, "tx_total_tgas": tgas(total), "receipts": receipts, "margin_to_300_tgas": tgas(TX_GAS_LIMIT - total) });
        let mut checked = vec![];
        for &(i, b) in budgets {
            let g = row["receipts"][i]["gas"].as_u64().unwrap_or_else(|| panic!("{what}: no receipt {i}: {row}"));
            checked.push(json!({ "receipt": i, "on": row["receipts"][i]["on"], "attached_budget_tgas": b / TGAS, "margin_to_budget_tgas": tgas(b.saturating_sub(g)) }));
            assert!(g < b, "{what}: receipt {i} burnt {g} of its {b} budget");
        }
        if !checked.is_empty() {
            row["budgets"] = json!(checked);
        }
        assert!(total < TX_GAS_LIMIT, "{what}: {total} gas");
        eprintln!("RES-1 {row}");
        self.rows.push(row);
        max
    }

    /// The largest pool receipt over every row but `new`.
    fn worst_pool(&self) -> u64 {
        self.rows
            .iter()
            .filter(|r| !r["call"].as_str().unwrap().starts_with("new"))
            .flat_map(|r| r["receipts"].as_array().unwrap().iter().filter(|x| x["on"] == "pool").map(|x| x["gas"].as_u64().unwrap()))
            .max()
            .unwrap_or(0)
    }
}

/// A `finish` or `confirm_not_executed`: the executor's call and its
/// `on_settled` both succeed, and the lock's `settle` succeeds iff
/// `accepted`. Returns the gas `on_settled` burnt, which must be under
/// ON_SETTLED_LIMIT (upstream res1.rs `on_settled`).
fn on_settled(what: &str, r: &ExecutionFinalResult, exec: &AccountId, lock: &AccountId, accepted: bool) -> u64 {
    let (on_exec, on_lock) = (receipts_on(r, exec), receipts_on(r, lock));
    assert!(on_exec.len() == 2 && on_exec.iter().all(|o| o.is_success()), "{what}: executor receipts {:?}", r.failures());
    assert!(on_lock.len() == 1 && on_lock[0].is_success() == accepted, "{what}: the lock's settle should {}", if accepted { "succeed" } else { "fail" });
    let g = on_exec[1].gas_burnt.as_gas();
    assert!(g < ON_SETTLED_LIMIT, "{what}: on_settled burnt {g} gas, the limit is {ON_SETTLED_LIMIT}");
    g
}

/// The `reconciled` events logged on `account`.
fn reconciled(r: &ExecutionFinalResult, account: &AccountId) -> Vec<Value> {
    events_on(r, account).into_iter().filter(|e| e["event"] == "reconciled").map(|e| e["data"][0].clone()).collect()
}

fn open_count(p: &Pool) -> usize {
    p.records.iter().filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled)).count()
}

/// `actor` submits recipe 31 x 100 at the reference's version; must be
/// admitted; applied to the reference. Returns (id, outcome).
async fn submit_open(actor: &Account, lock: &Contract, reference: &mut Pool, expires_at: u64, now: u64, curve: &mut Vec<(usize, u32, u64)>) -> (u32, ExecutionFinalResult) {
    let req = Request { recipe: 31, units: 100, version: reference.version, expires_at };
    let r = ok(actor, lock.id(), "submit", json!({ "request": req })).await;
    let v: Value = r.clone().json().unwrap();
    assert_eq!(v["decision"]["allowed"], true, "submit at {} records: {v}", reference.records.len());
    let id = reference.submit(actor.id().as_str(), req, now).unwrap();
    assert_eq!(v["id"], id);
    curve.push((reference.records.len(), open_count(reference) as u32, receipts_on(&r, lock.id())[0].gas_burnt.as_gas()));
    (id, r)
}

/// The maximum-size state (module doc, A-C), built with real transactions
/// only, with the host-side core in step; A-C's gas rows in `gas`.
struct Max {
    worker: Worker<Sandbox>,
    owner: Account,
    actor: Account,
    lock: Contract,
    exec: Contract,
    reference: Pool,
    a: String,
    x: String,
    now: u64,
    expires_at: u64,
    gas: Gas,
    curve: Vec<(usize, u32, u64)>,
    state_bytes: usize,
    storage_usage: u64,
    json_len: usize,
    submit_max: u64,
    get_pool_tx: u64,
}

async fn build_max(t0: Instant) -> Max {
    let worker = sandbox().await;
    // 64-byte account ids (54 + ".test.near") for the ids stored per recipe.
    let owner = subaccount(&worker, &"o".repeat(54), 40).await;
    let actor = subaccount(&worker, &"a".repeat(54), 60).await;
    let lock = deploy(&worker, "pool", 50, &contract_wasm()).await;
    let exec = deploy(&worker, &"x".repeat(54), 50, &executor_wasm()).await;
    assert_eq!((actor.id().as_str().len(), exec.id().as_str().len()), (64, 64));
    let cfg = max_config(owner.id(), actor.id(), exec.id());
    cfg.validate().expect("max config validates");
    assert_eq!((cfg.metrics.len(), cfg.terms.len(), cfg.recipes.len()), (32, 64, 32));
    let r_new = ok(lock.as_account(), lock.id(), "new", json!({ "config": cfg })).await;
    ok(exec.as_account(), exec.id(), "new", json!({ "lock": lock.id(), "operator": owner.id() })).await;
    let mut reference: Pool = pool(&lock).await;
    assert_eq!(reference, Pool::new(cfg.clone(), reference.observed_at).unwrap());
    let (a, x, o) = (actor.id().to_string(), exec.id().to_string(), owner.id().to_string());
    let now = block_now(&worker).await;
    let expires_at = now + 3 * 3600;
    let pool_on = [(lock.id(), "pool")];
    let mut gas = Gas { rows: vec![] };
    let mut curve: Vec<(usize, u32, u64)> = vec![];
    gas.add("new (max config)", &r_new, &pool_on, &[]);

    // ---- A. 31 Completed prerequisites (recipe i x 1), then open recipe-31
    // reservations of 100 units up to 255 records.
    for i in 0..31u32 {
        let req = Request { recipe: i, units: 1, version: reference.version, expires_at };
        let r = ok(&actor, lock.id(), "submit", json!({ "request": req })).await;
        let v: Value = r.clone().json().unwrap();
        assert_eq!(v["decision"]["allowed"], true, "prerequisite {i}: {v}");
        let id = reference.submit(&a, req, now).unwrap();
        curve.push((reference.records.len(), 1, receipts_on(&r, lock.id())[0].gas_burnt.as_gas()));
        ok(&actor, lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(&a, id, now).unwrap();
        reference.acknowledge(id, true).unwrap();
        ok(&owner, exec.id(), "finish", json!({ "id": id, "completed": true })).await;
        reference.settle(&x, id, true).unwrap();
    }
    assert_eq!(pool(&lock).await, reference, "after prerequisites");
    assert_eq!(&reference.completed_units[..31], &[1u32; 31][..]);
    while reference.records.len() < MAX_RECORDS - 1 {
        submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    }
    assert_eq!(pool(&lock).await, reference, "at 255 records");
    eprintln!("255 records in {:?}", t0.elapsed());

    // ---- B. Admissions.
    let req = Request { recipe: 31, units: 100, version: reference.version, expires_at };
    let r = ok(&actor, lock.id(), "preview", json!({ "actor": actor.id(), "request": req })).await;
    let d: Value = r.clone().json().unwrap();
    assert_eq!(d["allowed"], true, "{d}");
    gas.add("preview at 255 records, 224 open (as a transaction)", &r, &pool_on, &[]);
    // #256: 224 open records in the envelope, no prune (v1.0.0's worst case).
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (255, 0));
    gas.add("submit #256 (224 open records, no prune)", &r, &pool_on, &[]);
    // #257: the oldest record (id 0) is Completed, so this submit prunes it.
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id, reference.records.len()), (256, 1, 256));
    gas.add("submit #257 (225 open records, prunes id 0)", &r, &pool_on, &[]);
    assert_eq!(pool(&lock).await, reference, "after the first prune");
    while reference.first_id < 30 {
        submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    }
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (286, 31));
    let submit_max = gas.add("submit #287 (255 open records, prunes the last Completed record)", &r, &pool_on, &[]);
    assert_eq!(open_count(&reference), 256);
    assert_eq!(pool(&lock).await, reference, "256 open records");
    // Full: the oldest retained record (id 31) is open. Nothing is written.
    let raw = raw_state(&worker, lock.id()).await;
    let req = Request { recipe: 31, units: 100, version: reference.version, expires_at };
    let r = ok(&actor, lock.id(), "submit", json!({ "request": req })).await;
    let v: Value = r.clone().json().unwrap();
    assert_eq!((v["decision"]["reason"].as_str(), v["id"].is_null()), (Some("Full"), true), "{v}");
    assert_eq!(reference.preview(&a, &req, now).reason, "Full");
    gas.add("submit #288 (rejected Full: the oldest record is open)", &r, &pool_on, &[]);
    assert_eq!(raw_state(&worker, lock.id()).await, raw, "a Full submit wrote state");
    assert_eq!(pool(&lock).await, reference, "after Full");

    // Reads.
    let r = ok(&actor, lock.id(), "get_pool", json!({})).await;
    let json_len = r.clone().into_result().unwrap().raw_bytes().unwrap().len();
    let get_pool_tx = gas.add("get_pool (as a transaction)", &r, &pool_on, &[]);
    let r = ok(&actor, lock.id(), "get_commitments", json!({ "from": 0, "limit": 1000 })).await;
    let page: Vec<Value> = r.clone().json().unwrap();
    assert_eq!((page.len(), page[0]["id"].as_u64(), page[49]["id"].as_u64()), (50, Some(31), Some(80)));
    gas.add("get_commitments(0, 1000) -> ids 31..80 (as a transaction)", &r, &pool_on, &[]);
    let state_bytes = raw_state(&worker, lock.id()).await.get(b"STATE".as_slice()).map(Vec::len).unwrap();
    let storage_usage = lock.view_account().await.unwrap().storage_usage;

    // ---- C. Owner calls and a release at 256 open records.
    for paused in [true, false] {
        let r = ok(&owner, lock.id(), "set_paused", json!({ "paused": paused })).await;
        reference.set_paused(&o, paused).unwrap();
        gas.add(&format!("set_paused({paused}), 256 open records"), &r, &pool_on, &[]);
    }
    let r = ok(&owner, lock.id(), "observe", json!({ "values": reference.values })).await;
    let chain = pool(&lock).await;
    reference.observe(&o, reference.values.clone(), chain.observed_at).unwrap();
    assert_eq!(chain, reference, "after the refresh");
    gas.add("observe (restates the values), 256 open records", &r, &pool_on, &[]);
    // cancel the oldest record: its release is checked against 255 other open records.
    let r = ok(&actor, lock.id(), "cancel", json!({ "id": 31 })).await;
    reference.cancel(&a, 31, now).unwrap();
    gas.add("cancel (release; 255 other open records)", &r, &pool_on, &[]);
    let (id, r) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id, open_count(&reference)), (287, 32, 256));
    gas.add("submit #288 after the cancel (255 open records, prunes id 31)", &r, &pool_on, &[]);
    assert_eq!(pool(&lock).await, reference, "256 open records again");
    eprintln!("maximum state (A-C) in {:?}", t0.elapsed());
    Max { worker, owner, actor, lock, exec, reference, a, x, now, expires_at, gas, curve, state_bytes, storage_usage, json_len, submit_max, get_pool_tx }
}

/// The submit-gas curve: the first submit, every 32nd, and the last ones.
fn curve_rows(curve: &[(usize, u32, u64)]) -> Vec<Value> {
    curve
        .iter()
        .enumerate()
        .filter(|(i, _)| *i == 0 || *i % 32 == 31 || *i >= 254)
        .map(|(i, (n, open, g))| json!({ "submit": i + 1, "records_after": n, "open_after": open, "pool_receipt_tgas": tgas(*g) }))
        .collect()
}

/// The largest pool receipt over the rows whose call starts with one of
/// docs/api.md's "other calls" (no promises): submit, cancel, settle,
/// observe, set_paused. Asserts docs/api.md:50's rule for their row: the
/// 30 Tgas attach is at least 10 Tgas above that receipt.
fn worst_other(gas: &Gas) -> (u64, String) {
    let mut worst = (0, String::new());
    for r in &gas.rows {
        let call = r["call"].as_str().unwrap();
        if ["submit", "cancel", "settle", "observe", "set_paused"].iter().any(|p| call.starts_with(p)) {
            for x in r["receipts"].as_array().unwrap().iter().filter(|x| x["on"] == "pool") {
                let g = x["gas"].as_u64().unwrap();
                if g > worst.0 {
                    worst = (g, call.to_string());
                }
            }
        }
    }
    let (g, call) = &worst;
    assert!(!call.is_empty(), "no submit, cancel, settle, observe or set_paused row");
    assert!(
        g + ATTACH_MARGIN_TGAS * TGAS <= ATTACH_OTHER * TGAS,
        "{call}: a pool receipt burnt {g} gas, not {ATTACH_MARGIN_TGAS} Tgas below the documented attach {ATTACH_OTHER} Tgas"
    );
    worst
}

/// PROP: RES-1
#[tokio::test]
async fn res_1_gas_at_max_state() {
    let t0 = Instant::now();
    let Max { worker, owner, actor, lock, exec, mut reference, a, x, now, expires_at: _, mut gas, curve, state_bytes, storage_usage, json_len, submit_max, get_pool_tx } =
        build_max(t0).await;
    let build_seconds = t0.elapsed().as_secs();
    let pool_on = [(lock.id(), "pool")];
    // Pool receipts first (the call, then on_result), then the executor's.
    let both = [(lock.id(), "pool"), (exec.id(), "exec")];
    // The executor's receipts (its call, then on_settled), then the lock's settle.
    let exec_first = [(exec.id(), "exec"), (lock.id(), "pool")];
    let notify_budgets = [(1, ON_SETTLED_TGAS * TGAS), (2, 40 * TGAS)];
    let mut on_settled_g = vec![];

    // dispatch at 300 Tgas, 256 open records: dispatch, on_result [30], execute [30].
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 287 })).await;
    reference.dispatch(&a, 287, now).unwrap();
    reference.acknowledge(287, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add("dispatch #287 at 300 Tgas (pool.dispatch, pool.on_result [30], exec.execute [30])", &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    // reconcile of the Active record: reconcile, on_result [30], status [10]; a no-op.
    let r = ok(&owner, lock.id(), "reconcile", json!({ "id": 287 })).await;
    assert_eq!(pool(&lock).await, reference, "reconcile of an Active record is a no-op");
    gas.add("reconcile #287 at 300 Tgas (pool.reconcile, pool.on_result [30], exec.status [10])", &r, &both, &[(1, 30 * TGAS), (2, 10 * TGAS)]);
    // on_result that releases (execute returns "cancelled").
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "reject" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 284 })).await;
    reference.dispatch(&a, 284, now).unwrap();
    reference.settle(&x, 284, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "on_result released 284");
    assert_eq!(reconciled(&r, lock.id()), vec![json!({ "id": 284, "outcome": "cancelled", "error": null })]);
    gas.add("dispatch #284, execute returns cancelled, on_result releases (255 other open records)", &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    // on_result for a failed execute: Unknown.
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "panic" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 283 })).await;
    reference.dispatch(&a, 283, now).unwrap();
    reference.acknowledge(283, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "283 Unknown");
    gas.add("dispatch #283, execute fails, on_result marks Unknown", &r, &both, &[(1, 30 * TGAS)]);
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "normal" })).await;
    // finish(true) through the executor: finish, on_settled [10], the lock's settle [40].
    let r = ok(&owner, exec.id(), "finish", json!({ "id": 287, "completed": true })).await;
    reference.settle(&x, 287, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add("finish #287 at 300 Tgas -> settle(true) [40 Tgas], on_settled [10 Tgas]", &r, &exec_first, &notify_budgets);
    on_settled_g.push(("finish #287", on_settled("finish 287", &r, exec.id(), lock.id(), true)));
    // confirm_not_executed of the Unknown record: settle(false) releases.
    let r = ok(&owner, exec.id(), "confirm_not_executed", json!({ "id": 283, "recipe": 31, "units": 100 })).await;
    reference.settle(&x, 283, false).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add("confirm_not_executed #283 at 300 Tgas -> settle(false) [40 Tgas], on_settled [10 Tgas]", &r, &exec_first, &notify_budgets);
    on_settled_g.push(("confirm #283", on_settled("confirm 283", &r, exec.id(), lock.id(), true)));
    // settle both ways by the executor key.
    for (id, completed) in [(286u32, true), (285u32, false)] {
        ok(&actor, lock.id(), "dispatch", json!({ "id": id })).await;
        reference.dispatch(&a, id, now).unwrap();
        reference.acknowledge(id, true).unwrap();
        let r = ok(exec.as_account(), lock.id(), "settle", json!({ "id": id, "completed": completed })).await;
        reference.settle(&x, id, completed).unwrap();
        gas.add(&format!("settle({completed}) of #{id} by the executor key"), &r, &pool_on, &[]);
    }
    // release of a Reserved record through cancel.
    let r = ok(&actor, lock.id(), "cancel", json!({ "id": 282 })).await;
    reference.cancel(&a, 282, now).unwrap();
    gas.add("cancel #282 (release)", &r, &pool_on, &[]);
    assert_eq!(pool(&lock).await, reference, "final");

    let worst = gas.worst_pool();
    assert!(worst < ATTACH_OTHER * TGAS, "a pool receipt burnt {worst} gas, not under {ATTACH_OTHER} Tgas");
    // Also asserts the largest of them is at least 10 Tgas under the attach.
    let (other, other_call) = worst_other(&gas);
    let on_settled_max = on_settled_g.iter().map(|(_, g)| *g).max().unwrap();
    record(
        "RES-1",
        json!({
            "property": "RES-1",
            "test": "res_1_gas_at_max_state",
            "result": "pass",
            "method": "upstream crates/sandbox/tests/res1.rs (v1.1.0) sections A-C, then 300 Tgas measurements of every promise path",
            "thresholds": "docs/api.md:44-59: every pool receipt < 30 Tgas (the attach for submit, cancel, settle, observe, set_paused); the largest receipt of those five calls at least 10 Tgas below their 30 Tgas attach (docs/api.md:50); promise receipts under their attached gas; on_settled < 6 Tgas",
            "state": {
                "metrics": 32, "terms": 64, "recipes": 32, "max_requires": 31, "records": 256, "open_at_full": 256,
                "state_bytes_at_full": state_bytes, "storage_usage_bytes_at_full": storage_usage, "get_pool_json_bytes_at_full": json_len,
                "built_by": "real transactions only", "build_seconds": build_seconds,
            },
            "worst_pool_receipt_tgas": tgas(worst),
            "worst_margin_to_30_tgas": tgas(ATTACH_OTHER * TGAS - worst),
            "largest_other_call_receipt": { "tgas": tgas(other), "call": other_call, "margin_to_attach_tgas": tgas(ATTACH_OTHER * TGAS - other), "docs_api_md_tgas": DOC_LARGEST_OTHER_TGAS, "matches_docs": tgas(other) == DOC_LARGEST_OTHER_TGAS },
            "submit_287_pool_receipt_tgas": tgas(submit_max),
            "get_pool_tx_tgas": tgas(get_pool_tx),
            "on_settled_tgas": on_settled_g.iter().map(|(w, g)| json!({ "call": w, "tgas": tgas(*g) })).collect::<Vec<Value>>(),
            "on_settled_max_tgas": tgas(on_settled_max),
            "calls": gas.rows,
            "submit_gas_by_submit": curve_rows(&curve),
            "seconds": t0.elapsed().as_secs(),
        }),
    );
    drop(worker);
}

// ------------------------------------------- prepaid gas below 300 Tgas

/// One call with `tgas_prepaid` Tgas. On success returns the outcome. On
/// failure checks that the call ran out of gas in its first receipt (on
/// `receiver`) and that nothing else happened: no receipt on the other
/// contract, no event, the raw storage of both contracts byte-identical, the
/// pool equal to `reference` and record `id` still `status`. Returns the row.
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
    let (pool_raw, exec_raw) = (raw_state(worker, lock.id()).await, raw_state(worker, exec.id()).await);
    let r = tx_gas(signer, receiver, method, args.clone(), tgas_prepaid * TGAS).await;
    if r.is_success() {
        return Ok(r);
    }
    let what = format!("{method} {args} with {tgas_prepaid} Tgas");
    let failures = r.failures();
    assert_eq!(failures.len(), 1, "{what}: {failures:?}");
    let f = failures[0];
    assert_eq!(&f.executor_id, receiver, "{what}: failed on {}", f.executor_id);
    let text = format!("{:?}", r.clone().into_result().unwrap_err());
    assert!(text.contains(OUT_OF_GAS), "{what}: not an out-of-gas failure: {text}");
    assert_eq!(receipts_on(&r, receiver).len(), 1, "{what}: more than one receipt on {receiver}");
    for other in [lock.id(), exec.id()].into_iter().filter(|o| *o != receiver) {
        assert!(receipts_on(&r, other).is_empty(), "{what}: a receipt ran on {other}");
    }
    let logs: Vec<String> = r.logs().iter().map(|l| l.to_string()).collect();
    assert!(logs.is_empty() && events(&logs).is_empty(), "{what}: log or event from a failed call: {logs:?}");
    assert_eq!(raw_state(worker, lock.id()).await, pool_raw, "{what}: pool storage changed");
    assert_eq!(raw_state(worker, exec.id()).await, exec_raw, "{what}: executor storage changed");
    let p = pool(lock).await;
    assert_eq!(&p, reference, "{what}: pool differs from the core");
    assert_eq!(p.record(id).unwrap().status, status, "{what}");
    Err(json!({
        "call": method, "args": args, "prepaid_tgas": tgas_prepaid, "ok": false, "failure": OUT_OF_GAS,
        "failed_receipt_on": receiver, "failed_receipt_burnt_tgas": tgas(f.gas_burnt.as_gas()),
        "receipts_on_other_contract": 0, "events": 0,
        "pool_storage_unchanged": true, "executor_storage_unchanged": true,
        "record_status_after": format!("{status:?}"),
    }))
}

/// The smallest whole-Tgas prepaid gas, counting up from `from`, for which
/// the call succeeds; every smaller amount tried must fail as `attempt`
/// checks (its rows go to `rows`). Returns the minimum and its outcome.
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
        assert!(g * TGAS <= TX_GAS_LIMIT, "{method} {args}: no success up to 300 Tgas");
        match attempt(worker, signer, receiver, method, &args, g, lock, exec, reference, id, status.clone()).await {
            Ok(r) => {
                eprintln!("RES-1 {method} {args}: smallest whole-Tgas prepaid gas that succeeds: {g}");
                return (g, r);
            }
            Err(row) => rows.push(row),
        }
        g += 1;
    }
}

/// A minimum against docs/api.md: at least 10 Tgas below the documented
/// attach (asserted), and equal to the documented least (recorded).
fn against_docs(call: &str, min: u64, attach: u64, documented: u64) -> Value {
    assert!(min > 40, "{call}: succeeded with 40 Tgas, the sweep's start");
    assert!(min + ATTACH_MARGIN_TGAS <= attach, "{call}: least prepaid {min} Tgas is not {ATTACH_MARGIN_TGAS} Tgas below the documented attach {attach}");
    json!({ "call": call, "min_prepaid_tgas": min, "docs_attach_tgas": attach, "margin_to_attach_tgas": attach - min, "docs_least_tgas": documented, "matches_docs": min == documented })
}

/// PROP: RES-1. Prepaid gas below 300 Tgas at the maximum-size state, and the
/// executor at its retention bound (256 jobs) and its cap (512 jobs):
/// upstream res1.rs's sections D-F. Below each minimum, the call fails with
/// "Exceeded the prepaid gas." and writes nothing.
#[tokio::test]
async fn res_1_min_prepaid_gas_and_executor_cap() {
    let t0 = Instant::now();
    let Max { worker, owner, actor, lock, exec, mut reference, a, x, now, expires_at, mut curve, .. } = build_max(t0).await;
    let both = [(lock.id(), "pool"), (exec.id(), "exec")];
    let exec_first = [(exec.id(), "exec"), (lock.id(), "pool")];
    let notify_budgets = [(1, ON_SETTLED_TGAS * TGAS), (2, 40 * TGAS)];
    let mut gas = Gas { rows: vec![] };
    let mut below = vec![];
    let mut minimum = vec![];
    let mut docs = vec![];

    // ---- D. Prepaid gas sweeps at 256 open records.
    assert_eq!(jobs(&exec).await.len(), 31, "executor jobs before the dispatch sweep");
    let (ra, rb, rc) = (287u32, 286u32, 285u32);
    let (min_dispatch, r) =
        min_prepaid(&worker, &actor, lock.id(), "dispatch", json!({ "id": ra }), 40, &lock, &exec, &reference, ra, Status::Reserved, &mut below).await;
    reference.dispatch(&a, ra, now).unwrap();
    reference.acknowledge(ra, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after dispatch at the minimum");
    let dispatch_burnt = receipts_on(&r, lock.id())[0].gas_burnt.as_gas();
    gas.add(&format!("dispatch #{ra} with {min_dispatch} Tgas prepaid (the minimum)"), &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    minimum.push(json!({
        "call": "dispatch", "state": "pool 256 records, all open (Reserved); executor 31 jobs",
        "min_prepaid_tgas": min_dispatch, "fails_at_tgas": format!("40..={}", min_dispatch - 1),
        "static_gas_attached_tgas": "execute 30 + on_result 30", "first_receipt_burnt_tgas": tgas(dispatch_burnt),
    }));
    docs.push(against_docs("dispatch", min_dispatch, ATTACH_DISPATCH, DOC_MIN_DISPATCH));
    // A second record: 40 and minimum - 1 fail and change nothing; then 300 Tgas succeeds.
    for g in [40, min_dispatch - 1] {
        let row = attempt(&worker, &actor, lock.id(), "dispatch", &json!({ "id": rb }), g, &lock, &exec, &reference, rb, Status::Reserved)
            .await
            .expect_err("dispatch below the minimum succeeded on a second record");
        below.push(row);
    }
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": rb })).await;
    reference.dispatch(&a, rb, now).unwrap();
    reference.acknowledge(rb, true).unwrap();
    let dispatch_burnt_300 = receipts_on(&r, lock.id())[0].gas_burnt.as_gas();
    gas.add(&format!("dispatch #{rb} with 300 Tgas after failed low-gas attempts"), &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    // A third record: exactly the minimum succeeds again.
    let r = tx_gas(&actor, lock.id(), "dispatch", json!({ "id": rc }), min_dispatch * TGAS).await;
    assert!(r.is_success(), "dispatch #{rc} with {min_dispatch} Tgas: {:?}", r.failures());
    reference.dispatch(&a, rc, now).unwrap();
    reference.acknowledge(rc, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add(&format!("dispatch #{rc} with {min_dispatch} Tgas prepaid (the minimum, again)"), &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);

    let (min_reconcile, r) =
        min_prepaid(&worker, &owner, lock.id(), "reconcile", json!({ "id": ra }), 40, &lock, &exec, &reference, ra, Status::Active, &mut below).await;
    assert_eq!(pool(&lock).await, reference, "reconcile of an Active record is a no-op");
    gas.add(&format!("reconcile #{ra} with {min_reconcile} Tgas prepaid (the minimum)"), &r, &both, &[(1, 30 * TGAS), (2, 10 * TGAS)]);
    minimum.push(json!({
        "call": "reconcile", "state": "pool 256 records, all open, record Active; executor 3 jobs (285..=287: dispatching 287 let execute forget ids 0..=30)",
        "min_prepaid_tgas": min_reconcile, "fails_at_tgas": format!("40..={}", min_reconcile - 1),
        "static_gas_attached_tgas": "status 10 + on_result 30", "first_receipt_burnt_tgas": tgas(receipts_on(&r, lock.id())[0].gas_burnt.as_gas()),
    }));
    docs.push(against_docs("reconcile", min_reconcile, ATTACH_RECONCILE, DOC_MIN_RECONCILE));

    // on_result that releases (execute returns "cancelled"), then a failed execute (Unknown).
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "reject" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 284 })).await;
    reference.dispatch(&a, 284, now).unwrap();
    reference.settle(&x, 284, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "on_result released 284");
    assert_eq!(reconciled(&r, lock.id()), vec![json!({ "id": 284, "outcome": "cancelled", "error": null })]);
    gas.add("dispatch, execute returns cancelled, on_result releases (255 other open records)", &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "panic" })).await;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 283 })).await;
    reference.dispatch(&a, 283, now).unwrap();
    reference.acknowledge(283, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "283 Unknown");
    gas.add("dispatch, execute fails, on_result marks Unknown", &r, &both, &[(1, 30 * TGAS)]);
    ok(&owner, exec.id(), "set_mode", json!({ "mode": "normal" })).await;

    // ---- E. The executor at its retention bound (256 jobs: ids 32..=287).
    for id in 32..282u32 {
        ok(&actor, lock.id(), "dispatch", json!({ "id": id })).await;
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
        &worker, &owner, exec.id(), "confirm_not_executed", json!({ "id": 283, "recipe": 31, "units": 100 }), 40, &lock, &exec, &reference, 283, Status::Unknown, &mut below,
    )
    .await;
    reference.settle(&x, 283, false).unwrap();
    assert_eq!(pool(&lock).await, reference, "after confirm at the minimum");
    gas.add(&format!("confirm_not_executed with {min_confirm} Tgas prepaid (the minimum) -> settle(false) [40 Tgas], on_settled [10 Tgas]"), &r, &exec_first, &notify_budgets);
    let on_settled_after_confirm = on_settled("confirm 283", &r, exec.id(), lock.id(), true);
    minimum.push(json!({
        "call": "confirm_not_executed (executor)", "state": "executor 254 jobs (255 after); pool 256 records, 254 open, the record Unknown",
        "min_prepaid_tgas": min_confirm, "fails_at_tgas": format!("40..={}", min_confirm - 1),
        "static_gas_attached_tgas": "settle 40 + on_settled 10", "first_receipt_burnt_tgas": tgas(receipts_on(&r, exec.id())[0].gas_burnt.as_gas()),
        "on_settled_burnt_tgas": tgas(on_settled_after_confirm),
    }));
    docs.push(against_docs("confirm_not_executed (254 jobs)", min_confirm, ATTACH_EXECUTOR, DOC_MIN_CONFIRM_254));
    // The 256th job.
    let last = 282u32;
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": last })).await;
    reference.dispatch(&a, last, now).unwrap();
    reference.acknowledge(last, true).unwrap();
    assert_eq!(pool(&lock).await, reference);
    gas.add("dispatch at the retention bound (exec.execute: 255 jobs -> 256)", &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    let execute_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    let st = exec_state(&worker, exec.id()).await;
    let mut ids: Vec<u32> = st.jobs.iter().map(|j| j.id).collect();
    assert_eq!(st.jobs.last().unwrap().id, last);
    ids.sort();
    assert_eq!(ids, (32..=287).collect::<Vec<u32>>(), "executor holds exactly the window");
    assert_eq!(st.newest, 287);
    let r = ok(&owner, lock.id(), "reconcile", json!({ "id": last })).await;
    assert_eq!(pool(&lock).await, reference, "reconcile of an Active record is a no-op");
    gas.add("reconcile at the retention bound (exec.status: 256 jobs, the job is last)", &r, &both, &[(1, 30 * TGAS), (2, 10 * TGAS)]);
    let status_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    let (min_finish, r) = min_prepaid(
        &worker, &owner, exec.id(), "finish", json!({ "id": last, "completed": true }), 40, &lock, &exec, &reference, last, Status::Active, &mut below,
    )
    .await;
    reference.settle(&x, last, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after finish at the minimum");
    gas.add(&format!("finish at the retention bound with {min_finish} Tgas prepaid (the minimum) -> settle(true) [40 Tgas], on_settled [10 Tgas]"), &r, &exec_first, &notify_budgets);
    let finish_256 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    let on_settled_256 = on_settled("finish 282", &r, exec.id(), lock.id(), true);
    minimum.push(json!({
        "call": "finish (executor)", "state": "executor 256 jobs (the last job); pool 256 records, 253 open",
        "min_prepaid_tgas": min_finish, "fails_at_tgas": format!("40..={}", min_finish - 1),
        "static_gas_attached_tgas": "settle 40 + on_settled 10", "first_receipt_burnt_tgas": tgas(finish_256),
        "on_settled_burnt_tgas": tgas(on_settled_256),
    }));
    docs.push(against_docs("finish (256 jobs)", min_finish, ATTACH_EXECUTOR, DOC_MIN_FINISH_256));
    // settle both ways by the executor key.
    for (id, completed) in [(100u32, true), (101u32, false)] {
        let r = ok(exec.as_account(), lock.id(), "settle", json!({ "id": id, "completed": completed })).await;
        reference.settle(&x, id, completed).unwrap();
        gas.add(&format!("settle({completed}) by the executor key (about 250 other open records)"), &r, &[(lock.id(), "pool")], &[]);
    }
    assert_eq!(pool(&lock).await, reference, "256 jobs, final");
    let exec_state_256 = raw_state(&worker, exec.id()).await.get(b"STATE".as_slice()).map(Vec::len).unwrap();

    // ---- F. The executor at its job cap (512), the largest state it can
    // reach. An accepted confirmation moves the executor's newest id (its
    // on_settled), so honest use does not get here: only confirmations the
    // lock refuses (operator error), or whose on_settled has not run yet, do,
    // and the executor's work is the same either way. The cheapest route:
    // the operator confirms the 256 ids 289..=544, above the newest id (287),
    // that the lock has not assigned. Each adds a "cancelled" job; its settle
    // fails NotFound on the lock, which changes nothing there, and its
    // on_settled takes the refused branch.
    let (mut confirm_512, mut on_settled_refused) = (0, 0);
    for id in 289..=544u32 {
        let r = tx(&owner, exec.id(), "confirm_not_executed", json!({ "id": id, "recipe": 31, "units": 100 })).await;
        let g = on_settled(&format!("confirm {id}"), &r, exec.id(), lock.id(), false);
        on_settled_refused = on_settled_refused.max(g);
        expect_panic(&failure_text(&r), "NotFound", true);
        if id == 544 {
            confirm_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
            gas.add("confirm_not_executed of an unassigned id, 511 jobs -> 512 (the lock's settle fails NotFound; on_settled changes nothing)", &r, &exec_first, &notify_budgets);
        }
    }
    assert_eq!(jobs(&exec).await.len(), 512);
    assert_eq!(exec_state(&worker, exec.id()).await.newest, 287, "refused settles moved nothing");
    assert_eq!(pool(&lock).await, reference, "the confirmations changed nothing on the lock");
    eprintln!("512 jobs in {:?}", t0.elapsed());
    // One more confirmation fails "full" on the executor and never reaches the lock.
    let raw = raw_state(&worker, exec.id()).await;
    let r = tx(&owner, exec.id(), "confirm_not_executed", json!({ "id": 545, "recipe": 31, "units": 100 })).await;
    let full = expect_panic(&failure_text(&r), "full", false);
    assert!(receipts_on(&r, lock.id()).is_empty());
    assert_eq!(raw_state(&worker, exec.id()).await, raw);
    assert_eq!(jobs(&exec).await.len(), 512);
    let exec_state_512 = raw_state(&worker, exec.id()).await.get(b"STATE".as_slice()).map(Vec::len).unwrap();
    // status with 512 jobs: reconcile of an Active record changes nothing.
    let r = ok(&owner, lock.id(), "reconcile", json!({ "id": 281 })).await;
    assert_eq!(pool(&lock).await, reference, "reconcile at the job cap is a no-op");
    gas.add("reconcile at the job cap (exec.status: 512 jobs)", &r, &both, &[(1, 30 * TGAS), (2, 10 * TGAS)]);
    let status_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    // finish with 512 jobs: the least prepaid gas.
    let (min_finish_512, r) = min_prepaid(
        &worker, &owner, exec.id(), "finish", json!({ "id": 281, "completed": true }), 40, &lock, &exec, &reference, 281, Status::Active, &mut below,
    )
    .await;
    reference.settle(&x, 281, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after finish at the job cap");
    gas.add(&format!("finish at the job cap with {min_finish_512} Tgas prepaid (the minimum) -> settle(true) [40 Tgas], on_settled [10 Tgas]"), &r, &exec_first, &notify_budgets);
    let finish_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    // Accepted: on_settled scans and rewrites all 512 jobs.
    let on_settled_512 = on_settled("finish 281", &r, exec.id(), lock.id(), true);
    assert_eq!(jobs(&exec).await.len(), 512, "finish 281 forgot nothing");
    minimum.push(json!({
        "call": "finish (executor) at the job cap", "state": "executor 512 jobs; pool 256 records, 251 open",
        "min_prepaid_tgas": min_finish_512, "fails_at_tgas": format!("40..={}", min_finish_512 - 1),
        "static_gas_attached_tgas": "settle 40 + on_settled 10", "first_receipt_burnt_tgas": tgas(finish_512),
        "on_settled_burnt_tgas": tgas(on_settled_512),
    }));
    docs.push(against_docs("finish (512 jobs)", min_finish_512, ATTACH_EXECUTOR, DOC_MIN_FINISH_512));
    // execute of a new id with 512 jobs: finish the front record (32), submit
    // (prunes it, id 288) and dispatch. execute forgets job 32 and adds 288.
    ok(&owner, exec.id(), "finish", json!({ "id": 32, "completed": true })).await;
    reference.settle(&x, 32, true).unwrap();
    let (id, _) = submit_open(&actor, &lock, &mut reference, expires_at, now, &mut curve).await;
    assert_eq!((id, reference.first_id), (288, 33));
    let r = ok(&actor, lock.id(), "dispatch", json!({ "id": 288 })).await;
    reference.dispatch(&a, 288, now).unwrap();
    reference.acknowledge(288, true).unwrap();
    assert_eq!(pool(&lock).await, reference, "after dispatch at the job cap");
    gas.add("dispatch at the job cap (exec.execute: 512 jobs, forgets id 32, adds 288)", &r, &both, &[(1, 30 * TGAS), (2, 30 * TGAS)]);
    let execute_512 = receipts_on(&r, exec.id())[0].gas_burnt.as_gas();
    let ids = job_ids(&exec).await;
    assert_eq!(ids.len(), 512);
    assert!(!ids.contains(&32) && ids.contains(&288));
    assert_eq!(pool(&lock).await, reference, "final");

    let on_settled_max = [on_settled_after_confirm, on_settled_256, on_settled_512, on_settled_refused].into_iter().max().unwrap();
    assert!(on_settled_max < ON_SETTLED_LIMIT);
    let worst = gas.worst_pool();
    assert!(worst < ATTACH_OTHER * TGAS, "a pool receipt burnt {worst} gas, not under {ATTACH_OTHER} Tgas");
    // The settles by the executor key: at least 10 Tgas under the attach.
    let (other, other_call) = worst_other(&gas);
    let table: Vec<String> = minimum
        .iter()
        .map(|m| format!("{:<32} min {:>3} Tgas, fails at {:>8}, first receipt {:>5} Tgas, {}", m["call"].as_str().unwrap(), m["min_prepaid_tgas"], m["fails_at_tgas"].as_str().unwrap(), m["first_receipt_burnt_tgas"], m["state"].as_str().unwrap()))
        .collect();
    eprintln!("RES-1 minimum prepaid gas (whole Tgas):\n{}", table.join("\n"));
    for row in &gas.rows {
        let rs: Vec<String> = row["receipts"].as_array().unwrap().iter().map(|r| format!("{} {}", r["on"].as_str().unwrap(), r["gas_tgas"])).collect();
        eprintln!("RES-1 {:<100} tx {:>6} Tgas | {}", row["call"].as_str().unwrap(), row["tx_total_tgas"], rs.join(", "));
    }
    record(
        "RES-1-gas",
        json!({
            "property": "RES-1",
            "test": "res_1_min_prepaid_gas_and_executor_cap",
            "result": "pass",
            "method": "upstream crates/sandbox/tests/res1.rs (v1.1.0) sections D-F: whole-Tgas sweep from 40 Tgas up; every amount below the minimum fails with the runtime's out-of-gas error in its first receipt, runs no other receipt, emits no event, and leaves both contracts' raw storage byte-identical and the pool equal to the core",
            "thresholds": "docs/api.md:44-59: each least prepaid amount at least 10 Tgas below its documented attach (dispatch 100, reconcile 60, finish/confirm_not_executed 70); every on_settled < 6 Tgas; every pool receipt < 30 Tgas, and the largest receipt of submit, cancel, settle, observe and set_paused at least 10 Tgas below their 30 Tgas attach (docs/api.md:50); promise receipts under their attached gas",
            "minimum_prepaid_gas": minimum,
            "against_docs_api_md": docs,
            "dispatch_pool_receipt_burnt_tgas": { "at_minimum_prepaid": tgas(dispatch_burnt), "at_300_tgas": tgas(dispatch_burnt_300) },
            "executor": {
                "execute_256th_tgas": tgas(execute_256), "status_at_256_tgas": tgas(status_256), "finish_last_of_256_tgas": tgas(finish_256),
                "confirm_511_to_512_tgas": tgas(confirm_512), "status_at_512_tgas": tgas(status_512), "finish_at_512_tgas": tgas(finish_512),
                "execute_at_512_tgas": tgas(execute_512),
                "on_settled_after_confirm_tgas": tgas(on_settled_after_confirm), "on_settled_at_256_tgas": tgas(on_settled_256),
                "on_settled_at_512_tgas": tgas(on_settled_512), "on_settled_refused_tgas": tgas(on_settled_refused),
                "on_settled_attached_tgas": ON_SETTLED_TGAS, "on_settled_limit_tgas": ON_SETTLED_LIMIT / TGAS, "on_settled_margin_tgas": tgas(ON_SETTLED_TGAS * TGAS - on_settled_max),
                "state_bytes_at_256_jobs": exec_state_256, "state_bytes_at_512_jobs": exec_state_512,
                "confirm_not_executed_at_cap": { "result": "fails", "observed_panic": full, "lock_receipts": 0 },
            },
            "worst_pool_receipt_tgas": tgas(worst),
            "largest_other_call_receipt": { "tgas": tgas(other), "call": other_call, "margin_to_attach_tgas": tgas(ATTACH_OTHER * TGAS - other) },
            "below_minimum_attempts": below,
            "calls": gas.rows,
            "seconds": t0.elapsed().as_secs(),
        }),
    );
}
