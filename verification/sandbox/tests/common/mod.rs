//! Shared helpers for the shared-pool sandbox tests (near-approval strategy.md
//! section 9 pattern). Adapted from upstream `crates/sandbox/tests/
//! integration.rs` at the pin (`call`, `fail`, `state`): the same calls, plus
//! raw-state snapshots, event parsing, receipt lookups and result recording.
//! v1.1.0: one build (upstream scripts/build.sh, RUSTFLAGS="--cfg near"), so
//! every failure must carry its message; the executor's job window
//! (`get_jobs`, and `newest` read from its borsh state).
#![allow(dead_code)]

use near_workspaces::network::Sandbox;
use near_workspaces::result::{ExecutionFinalResult, ExecutionOutcome};
use near_workspaces::types::NearToken;
use near_workspaces::{Account, AccountId, Contract, Worker};
use pool_core::{Commitment, Config, Metric, Pool, Recipe, Shape, Status, Term};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

/// NEAR's per-transaction gas limit (and near-workspaces' `max_gas`).
pub const TX_GAS_LIMIT: u64 = 300_000_000_000_000;
pub const TGAS: u64 = 1_000_000_000_000;
pub const PANIC: &str = "Smart contract panicked: ";

pub fn sandbox_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(var: &str, default: PathBuf, hint: &str) -> Vec<u8> {
    let path = std::env::var(var).map(PathBuf::from).unwrap_or(default);
    std::fs::read(&path).unwrap_or_else(|e| panic!("can't read {}: {e}. {hint}", path.display()))
}

/// The build under test. v1.0.0 had two (`upstream`, a plain cargo build,
/// and `cfg-near`); since v1.1.0 upstream scripts/build.sh sets
/// RUSTFLAGS="--cfg near" itself, so there is one, `upstream`
/// (scripts/build-wasm.sh). Result files go to results/upstream/.
pub fn variant() -> String {
    "upstream".into()
}

fn target_dir() -> PathBuf {
    sandbox_dir().join("target/pool/wasm32-unknown-unknown/release")
}

/// The pool contract under test: `POOL_CONTRACT_WASM`, else the output of
/// `scripts/build-wasm.sh` (a copy of build.sh's, hash-checked there).
pub fn contract_wasm() -> Vec<u8> {
    read("POOL_CONTRACT_WASM", target_dir().join("pool_contract.wasm"), "Run scripts/build-wasm.sh or set POOL_CONTRACT_WASM")
}

pub fn executor_wasm() -> Vec<u8> {
    read("POOL_EXECUTOR_WASM", target_dir().join("pool_executor.wasm"), "Run scripts/build-wasm.sh or set POOL_EXECUTOR_WASM")
}

pub fn relay_wasm() -> Vec<u8> {
    read(
        "RELAY_WASM",
        sandbox_dir().join("target/wasm32-unknown-unknown/release/sandbox_relay.wasm"),
        "Run scripts/build-res.sh",
    )
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes `results/upstream/<id>.json` (observations: the test's own asserts
/// are the pass/fail criterion; the file is the record), tagged with the
/// build and the sha256 of both contracts under test.
pub fn record(id: &str, value: Value) {
    let dir = sandbox_dir().join("results").join(variant());
    std::fs::create_dir_all(&dir).unwrap();
    let mut v = value;
    if let Value::Object(m) = &mut v {
        m.insert("wasm_variant".into(), json!(variant()));
        m.insert("pool_contract_sha256".into(), json!(sha256_hex(&contract_wasm())));
        m.insert("pool_executor_sha256".into(), json!(sha256_hex(&executor_wasm())));
        m.insert("near_sandbox".into(), json!("2.13.4 (near-workspaces 0.23.0 / near-sandbox 0.3.16 default)"));
    }
    let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
    eprintln!("{id}: {text}");
    std::fs::write(dir.join(format!("{id}.json")), text).unwrap();
}

pub async fn sandbox() -> Worker<Sandbox> {
    near_workspaces::sandbox().await.expect("start near-sandbox")
}

pub async fn subaccount(worker: &Worker<Sandbox>, name: &str, near: u128) -> Account {
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

pub async fn deploy(worker: &Worker<Sandbox>, name: &str, near: u128, wasm: &[u8]) -> Contract {
    subaccount(worker, name, near)
        .await
        .deploy(wasm)
        .await
        .unwrap()
        .into_result()
        .unwrap()
}

/// Seconds of the latest block: what the contract's `now()` is about to see.
pub async fn block_now(worker: &Worker<Sandbox>) -> u64 {
    worker.view_block().await.unwrap().timestamp() / 1_000_000_000
}

/// A transaction from `a` to `receiver.method(args)` with 300 Tgas.
pub async fn tx(a: &Account, receiver: &AccountId, method: &str, args: Value) -> ExecutionFinalResult {
    a.call(receiver, method)
        .args_json(args)
        .max_gas()
        .transact()
        .await
        .unwrap_or_else(|e| panic!("{method}: rpc error {e:?}"))
}

/// `tx` with `gas` prepaid instead of 300 Tgas (RES-1's low-gas runs).
pub async fn tx_gas(a: &Account, receiver: &AccountId, method: &str, args: Value, gas: u64) -> ExecutionFinalResult {
    a.call(receiver, method)
        .args_json(args)
        .gas(near_workspaces::types::Gas::from_gas(gas))
        .transact()
        .await
        .unwrap_or_else(|e| panic!("{method} with {gas} gas: rpc error {e:?}"))
}

/// The outcome of a `transact_async` transaction once `done` holds for it.
/// near-workspaces 0.23's own await of a `TransactionStatus` polls with
/// `wait_until: Included` and returns the first outcome whose final status is
/// known. The executor's `finish` and `confirm_not_executed` return the lock's
/// `settle` value and run `on_settled` as a separate callback (upstream
/// `notify`: `promise_then` on `settle`, then `promise_return(settle)`), so that
/// first outcome can lack `on_settled`. (A plain `transact` is
/// `broadcast_tx_commit`, which waits until every receipt has run.) Polls every
/// 100 ms; fails after 20 s.
pub async fn complete(t: &near_workspaces::operations::TransactionStatus, done: impl Fn(&ExecutionFinalResult) -> bool) -> ExecutionFinalResult {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if let std::task::Poll::Ready(r) = t.status().await.unwrap() {
            if done(&r) {
                return r;
            }
            assert!(std::time::Instant::now() < deadline, "transaction still incomplete after 20 s: {:?}", r.receipt_outcomes());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// The runtime's error when a receipt needs more gas than it was given.
pub const OUT_OF_GAS: &str = "Exceeded the prepaid gas.";

/// `tx` with a deposit of `yocto` attached (NT-F2: reconcile is nonpayable).
pub async fn tx_deposit(a: &Account, receiver: &AccountId, method: &str, args: Value, yocto: u128) -> ExecutionFinalResult {
    a.call(receiver, method)
        .args_json(args)
        .deposit(NearToken::from_yoctonear(yocto))
        .max_gas()
        .transact()
        .await
        .unwrap_or_else(|e| panic!("{method}: rpc error {e:?}"))
}

/// `tx` that must succeed (every receipt, not only the final value).
pub async fn ok(a: &Account, receiver: &AccountId, method: &str, args: Value) -> ExecutionFinalResult {
    let r = tx(a, receiver, method, args).await;
    assert!(r.is_success(), "{method} failed: {:?}", r.failures());
    r
}

/// The text of a transaction's failure; panics if it succeeded.
pub fn failure_text(r: &ExecutionFinalResult) -> String {
    assert!(r.is_failure(), "expected failure, got success: logs {:?}", r.logs());
    format!("{:?}", r.clone().into_result().unwrap_err())
}

/// What a wasm `unreachable` trap looks like in a failure: v1.0.0's
/// documented build failed every call this way (finding SB-1).
pub const TRAP: &str = "WebAssembly trap: An `unreachable` opcode was executed.";

/// The panic message in a failure text: `Smart contract panicked: <msg>`
/// gives `<msg>`; a bare unreachable trap gives `TRAP`.
pub fn panic_message(text: &str) -> String {
    if let Some(i) = text.find(PANIC) {
        let rest = &text[i + PANIC.len()..];
        rest[..rest.find('"').unwrap_or(rest.len())].replace("\\n", "\n")
    } else if text.contains(TRAP) {
        TRAP.to_string()
    } else {
        text.to_string()
    }
}

/// Checks that a failure carries the panic `msg` and returns the observed
/// message. `exact`: the whole message is `msg` (env::panic_str of a core
/// error); otherwise it contains `msg` (Rust assert!/expect messages carry a
/// "panicked at file:line" prefix). The documented build has cfg(near), so
/// every failure must say `Smart contract panicked: <msg>`; a bare trap
/// (SB-1) fails the check.
pub fn expect_panic(text: &str, msg: &str, exact: bool) -> String {
    assert!(text.contains(PANIC), "expected {PANIC:?} with {msg:?}, got {text}");
    let observed = panic_message(text);
    let hit = if exact { observed == msg } else { observed.contains(msg) };
    assert!(hit, "expected panic {msg:?}, got {observed:?} in {text}");
    observed
}

/// `tx` that must fail with the core error / panic string `msg` (exact).
/// Returns the observed message.
pub async fn fails(a: &Account, receiver: &AccountId, method: &str, args: Value, msg: &str) -> String {
    let r = tx(a, receiver, method, args).await;
    expect_panic(&failure_text(&r), msg, true)
}

/// `fails`, returning the outcome too.
pub async fn fails_r(a: &Account, receiver: &AccountId, method: &str, args: Value, msg: &str) -> (String, ExecutionFinalResult) {
    let r = tx(a, receiver, method, args).await;
    (expect_panic(&failure_text(&r), msg, true), r)
}

/// `tx` that must fail with a panic containing `msg`.
pub async fn fails_containing(a: &Account, receiver: &AccountId, method: &str, args: Value, msg: &str) -> String {
    let r = tx(a, receiver, method, args).await;
    expect_panic(&failure_text(&r), msg, false)
}

pub async fn pool(c: &Contract) -> Pool {
    c.view("get_pool").await.unwrap().json().unwrap()
}

/// The executor's jobs (`get_jobs`, a public view since v1.1.0).
pub async fn jobs(c: &Contract) -> Vec<ExecJob> {
    let v: Vec<Value> = c.view("get_jobs").await.unwrap().json().unwrap();
    v.iter()
        .map(|j| ExecJob {
            id: j["id"].as_u64().unwrap() as u32,
            recipe: j["recipe"].as_u64().unwrap() as u32,
            units: j["units"].as_u64().unwrap() as u32,
            status: j["status"].as_str().unwrap().to_string(),
        })
        .collect()
}

/// The executor's job ids, in the order it holds them.
pub async fn job_ids(c: &Contract) -> Vec<u32> {
    jobs(c).await.iter().map(|j| j.id).collect()
}

/// A job, as `get_jobs` returns it and as the executor stores it (borsh).
#[derive(borsh::BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExecJob {
    pub id: u32,
    pub recipe: u32,
    pub units: u32,
    pub status: String,
}

/// The executor's contract state at the pin (crates/executor/src/lib.rs
/// `struct Executor`), field for field; `AccountId` is a borsh string.
#[derive(borsh::BorshDeserialize, Clone, Debug)]
pub struct ExecState {
    pub lock: String,
    pub operator: String,
    pub jobs: Vec<ExecJob>,
    pub mode: String,
    pub newest: u32,
}

/// The executor's whole state, decoded from its raw `STATE` value. The
/// decode must consume every byte, so a layout change fails here.
pub async fn exec_state(worker: &Worker<Sandbox>, id: &AccountId) -> ExecState {
    let raw = raw_state(worker, id).await;
    let bytes = raw.get(b"STATE".as_slice()).expect("executor STATE");
    borsh::from_slice(bytes).expect("decode executor STATE")
}

/// EX-7 on the real runtime: the executor never forgets the job of a record
/// the lock retains. `check` reads both contracts after a transaction: every
/// job seen before whose id the lock still retains (id >= first_id) must
/// still be held, every job that vanished must be below first_id, the jobs
/// must match the borsh state, and the job count must stay within `bound`.
pub struct Ex7 {
    pub seen: BTreeSet<u32>,
    pub bound: usize,
    pub checks: u32,
    pub max_jobs: usize,
}

impl Ex7 {
    pub fn new(bound: usize) -> Ex7 {
        Ex7 { seen: BTreeSet::new(), bound, checks: 0, max_jobs: 0 }
    }

    /// Returns (first_id, the executor's state).
    pub async fn check(&mut self, worker: &Worker<Sandbox>, lock: &Contract, exec: &Contract, what: &str) -> (u32, ExecState) {
        let p = pool(lock).await;
        let st = exec_state(worker, exec.id()).await;
        let held: BTreeSet<u32> = st.jobs.iter().map(|j| j.id).collect();
        assert_eq!(jobs(exec).await, st.jobs, "{what}: get_jobs differs from the stored jobs");
        for id in &self.seen {
            if p.record(*id).is_ok() {
                assert!(held.contains(id), "{what}: EX-7: job {id} forgotten while the lock retains it (first_id {})", p.first_id);
            }
        }
        assert!(st.jobs.len() <= self.bound, "{what}: {} jobs > {}", st.jobs.len(), self.bound);
        for j in &st.jobs {
            assert!(u64::from(j.id) + 256 > u64::from(st.newest), "{what}: job {} kept with newest {}", j.id, st.newest);
        }
        self.seen.extend(held.iter().copied());
        self.checks += 1;
        self.max_jobs = self.max_jobs.max(st.jobs.len());
        (p.first_id, st)
    }
}

/// Every storage key/value of `id` (the whole contract state, raw bytes).
pub async fn raw_state(worker: &Worker<Sandbox>, id: &AccountId) -> HashMap<Vec<u8>, Vec<u8>> {
    worker.view_state(id).await.unwrap()
}

pub fn events(logs: &[String]) -> Vec<Value> {
    logs.iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .map(|j| serde_json::from_str(j).unwrap())
        .collect()
}

/// Receipt outcomes executed on `account`, in the order the RPC lists them.
pub fn receipts_on<'a>(r: &'a ExecutionFinalResult, account: &AccountId) -> Vec<&'a ExecutionOutcome> {
    r.receipt_outcomes().iter().filter(|o| &o.executor_id == account).collect()
}

/// Events emitted by receipts on `account`.
pub fn events_on(r: &ExecutionFinalResult, account: &AccountId) -> Vec<Value> {
    let logs: Vec<String> = receipts_on(r, account).iter().flat_map(|o| o.logs.clone()).collect();
    events(&logs)
}

/// The `data[0]` of the single event named `kind` emitted on `account`.
pub fn event(r: &ExecutionFinalResult, account: &AccountId, kind: &str) -> Value {
    let all: Vec<Value> = events_on(r, account).into_iter().filter(|e| e["event"] == kind).collect();
    assert_eq!(all.len(), 1, "expected one {kind} event, got {all:?}");
    all[0]["data"][0].clone()
}

pub async fn block_height(worker: &Worker<Sandbox>, o: &ExecutionOutcome) -> u64 {
    worker.view_block().block_hash(o.block_hash).await.unwrap().height()
}

pub fn tgas(g: u64) -> f64 {
    (g as f64 / TGAS as f64 * 100.0).round() / 100.0
}

// ---------------------------------------------------------------- configs

pub fn metric(i: usize, min: i64, max: i64, initial: i64) -> Metric {
    Metric { name: format!("m{i}"), unit: "u".into(), initial, min, max }
}

pub fn term(metric: u32, target: i64, weight: u32, shape: Shape) -> Term {
    Term { metric, target, weight, shape }
}

pub fn recipe(name: &str, actor: &AccountId, executor: &AccountId, reserve: Vec<i64>, complete: Vec<i64>, max_units: u32) -> Recipe {
    Recipe {
        name: name.into(),
        actor: actor.to_string(),
        executor: executor.to_string(),
        reserve,
        complete,
        max_units,
        requires: vec![],
    }
}

/// The small configuration most tests use. Metric 1 carries the effects, so
/// a reservation, a completion and a release are each visible in `values[1]`;
/// metric 0 carries the energy term and stays on target.
/// Recipe i: `recipes[i] = (actor, executor)`, reserve [0, 2], complete [0, 3].
pub fn basic_config(owner: &AccountId, recipes: &[(&AccountId, &AccountId)]) -> Config {
    Config {
        name: "sandbox".into(),
        owner: owner.to_string(),
        metrics: vec![metric(0, 0, 100, 50), metric(1, -100, 100, 0)],
        terms: vec![term(0, 50, 1, Shape::Target)],
        recipes: recipes
            .iter()
            .enumerate()
            .map(|(i, (a, x))| recipe(&format!("r{i}"), a, x, vec![0, 2], vec![0, 3], 10))
            .collect(),
        max_age_seconds: 0,
    }
}

/// F-1's configuration (v1.0.0 brief, "Confirmed finding"): m(0,10,5),
/// m(-100,100,5); t(1,0,1,target); r(a1,x1,[-2,0],[2,-5],1,[]). v1.1.0
/// refuses its submit after observe [12, 5] (Bounds).
pub fn wedge_config(owner: &AccountId, actor: &AccountId, executor: &AccountId) -> Config {
    Config {
        name: "wedge".into(),
        owner: owner.to_string(),
        metrics: vec![metric(0, 0, 10, 5), metric(1, -100, 100, 5)],
        terms: vec![term(1, 0, 1, Shape::Target)],
        recipes: vec![recipe("r", actor, executor, vec![-2, 0], vec![2, -5], 1)],
        max_age_seconds: 0,
    }
}

/// The cure-by-completion hedge of upstream crates/core/src/findings.rs (F-1
/// fixed): F-1's metrics and term, recipe reserve [0, -1], complete [-3, 0],
/// max 1. After observe [12, 5] it is admitted ([12, 4]) and its completion
/// brings metric 0 back into range ([9, 4]).
pub fn cure_config(owner: &AccountId, actor: &AccountId, executor: &AccountId) -> Config {
    Config {
        name: "cure".into(),
        owner: owner.to_string(),
        metrics: vec![metric(0, 0, 10, 5), metric(1, -100, 100, 5)],
        terms: vec![term(1, 0, 1, Shape::Target)],
        recipes: vec![recipe("r", actor, executor, vec![0, -1], vec![-3, 0], 1)],
        max_age_seconds: 0,
    }
}

/// One record holds one unit of a 10-unit capacity until it ends either way:
/// upstream integration.rs's window and D-1 configuration.
pub fn window_config(name: &str, owner: &AccountId, actor: &AccountId, executor: &AccountId) -> Config {
    Config {
        name: name.into(),
        owner: owner.to_string(),
        metrics: vec![Metric { name: "In flight".into(), unit: "jobs".into(), initial: 0, min: 0, max: 10 }],
        terms: vec![term(0, 10, 1, Shape::Excess)],
        recipes: vec![recipe("job", actor, executor, vec![1], vec![-1], 1)],
        max_age_seconds: 0,
    }
}

// ---------------------------------------------------------------- setup

/// One sandbox with the pool (`pool`), the upstream executor (`exec`,
/// operator = owner), an owner, an actor and an unrelated account.
pub struct Env {
    pub worker: Worker<Sandbox>,
    pub owner: Account,
    pub actor: Account,
    pub mallory: Account,
    pub lock: Contract,
    pub exec: Contract,
}

impl Env {
    /// Accounts and code only; nothing initialized.
    pub async fn bare() -> Env {
        let worker = sandbox().await;
        let owner = subaccount(&worker, "owner", 20).await;
        let actor = subaccount(&worker, "actor", 20).await;
        let mallory = subaccount(&worker, "mallory", 20).await;
        let lock = deploy(&worker, "pool", 50, &contract_wasm()).await;
        let exec = deploy(&worker, "exec", 20, &executor_wasm()).await;
        Env { worker, owner, actor, mallory, lock, exec }
    }

    /// Another pool and executor in the same sandbox (`pool-<tag>`,
    /// `exec-<tag>`), with the same owner, actor and mallory; nothing
    /// initialized.
    pub async fn fresh(&self, tag: &str) -> Env {
        let lock = deploy(&self.worker, &format!("pool-{tag}"), 50, &contract_wasm()).await;
        let exec = deploy(&self.worker, &format!("exec-{tag}"), 20, &executor_wasm()).await;
        Env {
            worker: self.worker.clone(),
            owner: self.owner.clone(),
            actor: self.actor.clone(),
            mallory: self.mallory.clone(),
            lock,
            exec,
        }
    }

    /// `bare` plus `new` on both contracts, each called by its own account
    /// (both are #[private]); checks the initial state against Pool::new.
    pub async fn init(self, cfg: &Config) -> Env {
        let operator = self.owner.id().clone();
        self.init_with(cfg, &operator).await
    }

    /// `init` with the executor's operator `operator` (RT-D1 keeps it apart
    /// from the owner).
    pub async fn init_with(self, cfg: &Config, operator: &AccountId) -> Env {
        ok(self.lock.as_account(), self.lock.id(), "new", json!({ "config": cfg })).await;
        ok(
            self.exec.as_account(),
            self.exec.id(),
            "new",
            json!({ "lock": self.lock.id(), "operator": operator }),
        )
        .await;
        let p = pool(&self.lock).await;
        assert_eq!(p, Pool::new(cfg.clone(), p.observed_at).unwrap());
        self
    }

    pub async fn pool(&self) -> Pool {
        pool(&self.lock).await
    }

    pub async fn raw(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        raw_state(&self.worker, self.lock.id()).await
    }

    pub async fn now(&self) -> u64 {
        block_now(&self.worker).await
    }

    /// A request for `recipe` × `units` at the pool's current version,
    /// expiring in an hour.
    pub async fn request(&self, recipe: u32, units: u32) -> pool_core::Request {
        pool_core::Request {
            recipe,
            units,
            version: self.pool().await.version,
            expires_at: self.now().await + 3600,
        }
    }

    /// `actor` submits; returns (outcome, Submission JSON).
    pub async fn submit(&self, actor: &Account, req: &pool_core::Request) -> (ExecutionFinalResult, Value) {
        let r = ok(actor, self.lock.id(), "submit", json!({ "request": req })).await;
        let v: Value = r.clone().json().unwrap();
        (r, v)
    }

    /// Submit that must be accepted, applied to `reference` too; returns the id.
    pub async fn submit_ok(&self, reference: &mut Pool, actor: &Account, recipe: u32, units: u32) -> u32 {
        let req = self.request(recipe, units).await;
        let (_, v) = self.submit(actor, &req).await;
        assert_eq!(v["decision"]["allowed"], true, "submit rejected: {v}");
        let id = reference.submit(actor.id().as_str(), req, self.now().await).unwrap();
        assert_eq!(v["id"], id);
        id
    }

    pub async fn mode(&self, mode: &str) {
        ok(&self.owner, self.exec.id(), "set_mode", json!({ "mode": mode })).await;
    }

    pub async fn status(&self, id: u32) -> Status {
        self.pool().await.record(id).unwrap().status.clone()
    }

    pub async fn jobs(&self) -> Vec<ExecJob> {
        jobs(&self.exec).await
    }

    pub async fn newest(&self) -> u32 {
        exec_state(&self.worker, self.exec.id()).await.newest
    }

    pub async fn raw_exec(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        raw_state(&self.worker, self.exec.id()).await
    }
}

/// Order-insensitive summary of a pool for result files.
pub fn summary(p: &Pool) -> Value {
    let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
    for c in &p.records {
        *by_status.entry(format!("{:?}", c.status)).or_default() += 1;
    }
    json!({ "values": p.values, "version": p.version, "paused": p.paused, "records": p.records.len(), "first_id": p.first_id, "completed_units": p.completed_units, "by_status": by_status })
}

pub fn commitment(p: &Pool, id: u32) -> Commitment {
    p.record(id).unwrap().clone()
}

/// (receipt successes on the executor, on the lock) of a transaction, in the
/// order the RPC lists them (upstream integration.rs's `on`).
pub fn shape(r: &ExecutionFinalResult, exec: &AccountId, lock: &AccountId) -> (Vec<bool>, Vec<bool>) {
    let on = |a: &AccountId| receipts_on(r, a).iter().map(|o| o.is_success()).collect::<Vec<bool>>();
    (on(exec), on(lock))
}
