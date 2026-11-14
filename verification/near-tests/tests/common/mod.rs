//! Shared harness for the shared-pool shell tests (near-approval strategy.md
//! section 9 pattern).
//!
//! Upstream: difflabai/shared-pool-controls `crates/contract/src/lib.rs`,
//!           `crates/executor/src/lib.rs`, `crates/core/src/lib.rs`
//! Pin:      v1.1.0, 57b3f02d18e85149db3c1035431368c4e07a4d8a (merge of PR #4;
//!           tree identical to the PR head 6fcbd56)
//! sha256:   contract/src/lib.rs 3810d18d9afa6ba3a6fc6dac9c2e897acc343d8e532bd13b2c2bbc81da20c03c
//!           executor/src/lib.rs a2d63842c23d760882b36b1d0548062b6b4fe24f49b7ce5fda7c492cbe129198
//!           core/src/lib.rs     c3bb825b91f7c277dee8651beafd45955f7cf8935c858f928b6a1b62d607ba48
//!
//! Two ways to run the shell, both against near-sdk 5.29.1's mocked runtime
//! (the real `VMLogic` of near-vm-runner 0.37.4):
//!
//! - **Host path** (`call`, `view`, `deploy`): the `#[near]` methods called as
//!   Rust functions. Every call starts from storage (`state_read`, Borsh) and a
//!   successful call ends with `state_write`; a panicking call is discarded with
//!   every write (ASM-ATOMIC). Fast: used by `store_diff.rs` (SH-8).
//! - **Wasm path** (`wasm_call`, `Net`): the pinned wasm itself, built exactly
//!   as upstream `scripts/build.sh` builds it (`RUSTFLAGS="--cfg near"`,
//!   `cargo build --target wasm32-unknown-unknown --release --locked`; build
//!   `Near`; `wasm-surface.sh` SURF-3(a) checks the bytes equal build.sh's),
//!   run by the wasmi interpreter. Every `env` import is forwarded to the same
//!   mocked `VMLogic` (pointers translated between wasm memory and host
//!   memory). This exercises the code near-sdk generates only for wasm32:
//!   `#[init]` (init once), `#[private]`, the nonpayable deposit checks and
//!   JSON argument and return encoding. Any import other than the ones the
//!   shell legitimately uses traps as `forbidden host call`. The no-cfg build
//!   (`Plain`, v1.0.0's documented build) is kept only as a negative control
//!   (NT-F1, SURF-3(b)); it is not in `Build::all()`.
//!
//! Expected values are built here from the core (`pool_core::Pool`, the same
//! pinned crate) and from docs/api.md's names, never from the contract's
//! output.

#![allow(dead_code)]

use near_sdk::mock::{MockAction, Receipt};
use near_sdk::serde_json::{self, json, Value};
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{env, test_vm_config, AccountId, MockedBlockchain, NearToken, PromiseResult};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::RuntimeFeesConfig;
use pool_contract::Contract;
use pool_core::{Commitment, Config, Decision, Error, Metric, Pool, Recipe, Request, Shape, Status, Term};
use std::cell::Cell;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path as FsPath, PathBuf};
use std::sync::OnceLock;

pub type Storage = HashMap<Vec<u8>, Vec<u8>>;
pub const STATE_KEY: &[u8] = b"STATE";

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

pub const POOL: &str = "pool.near";
pub const OWNER: &str = "owner.near";
pub const A1: &str = "a1.near";
pub const A2: &str = "a2.near";
pub const X1: &str = "x1.near";
pub const X2: &str = "x2.near";
pub const Z: &str = "z.near";
pub const RELAY: &str = "relay.near";
pub const OPERATOR: &str = "operator.near";

pub fn acct(s: &str) -> AccountId {
    s.parse().unwrap_or_else(|e| panic!("bad account {s}: {e}"))
}

/// The conformance atoms (`conformance/src/main.rs`) as NEAR accounts.
pub fn atom(a: &str) -> &'static str {
    match a {
        "o" => OWNER,
        "a1" => A1,
        "a2" => A2,
        "x1" => X1,
        "x2" => X2,
        "z" => Z,
        _ => panic!("unknown atom {a}"),
    }
}

// ---------------------------------------------------------------------------
// Mocked runtime
// ---------------------------------------------------------------------------

/// Who calls, when, with what deposit (the runtime inputs the shell reads).
#[derive(Clone, Debug)]
pub struct Ctx {
    pub current: AccountId,
    pub predecessor: AccountId,
    pub signer: AccountId,
    pub ts_ns: u64,
    pub deposit: u128,
}

impl Ctx {
    /// A direct call to the pool at second `now` (plus `sub` nanoseconds).
    pub fn new(pred: &str, signer: &str, ts_ns: u64) -> Self {
        Ctx {
            current: acct(POOL),
            predecessor: acct(pred),
            signer: acct(signer),
            ts_ns,
            deposit: 0,
        }
    }
    pub fn by(who: &str, now: u64) -> Self {
        Self::new(who, who, now * 1_000_000_000 + 123_456_789)
    }
    pub fn on(mut self, current: &str) -> Self {
        self.current = acct(current);
        self
    }
    pub fn with_deposit(mut self, yocto: u128) -> Self {
        self.deposit = yocto;
        self
    }
    /// `now()` of the shell: block timestamp in whole seconds.
    pub fn now(&self) -> u64 {
        self.ts_ns / 1_000_000_000
    }
    pub fn caller(&self) -> &str {
        self.predecessor.as_str()
    }
}

/// `f()`, computed once per thread (`test_vm_config()` builds a whole
/// `RuntimeConfigStore`).
fn cached<T: Clone + 'static>(f: fn() -> T) -> T {
    use std::any::{Any, TypeId};
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<TypeId, Box<dyn Any>>> = Default::default();
    }
    CACHE.with(|c| {
        c.borrow_mut()
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(f()))
            .downcast_ref::<T>()
            .unwrap()
            .clone()
    })
}

/// Replaces the mocked blockchain with a fresh one for `ctx` over `storage`,
/// with `input` and the promise results of a callback. The results go through
/// `testing_env!`, which moves the storage over from the seeding blockchain.
pub fn install(ctx: &Ctx, storage: Storage, results: Vec<PromiseResult>, input: &[u8]) {
    let mut context = VMContextBuilder::new()
        .current_account_id(ctx.current.clone())
        .predecessor_account_id(ctx.predecessor.clone())
        .signer_account_id(ctx.signer.clone())
        .block_timestamp(ctx.ts_ns)
        .attached_deposit(NearToken::from_yoctonear(ctx.deposit))
        .build();
    context.input = input.to_vec().into();
    env::set_blockchain_interface(MockedBlockchain::new(
        context.clone(),
        cached(test_vm_config),
        cached(RuntimeFeesConfig::test),
        vec![],
        storage,
        Default::default(),
        None,
    ));
    if !results.is_empty() {
        near_sdk::testing_env!(
            context,
            cached(test_vm_config),
            cached(RuntimeFeesConfig::test),
            Default::default(),
            results
        );
    }
}

pub fn take_storage() -> Storage {
    near_sdk::mock::with_mocked_blockchain(|b| b.take_storage())
}

pub fn logs() -> Vec<String> {
    near_sdk::test_utils::get_logs()
}

thread_local! {
    static EXPECT_PANIC: Cell<bool> = const { Cell::new(false) };
}

/// Silences the default panic hook while a call is expected to panic.
pub fn quiet_expected_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !EXPECT_PANIC.with(|e| e.get()) {
                default(info)
            }
        }));
    });
}

pub fn catch_quiet<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    quiet_expected_panics();
    EXPECT_PANIC.with(|e| e.set(true));
    let r = panic::catch_unwind(AssertUnwindSafe(f));
    EXPECT_PANIC.with(|e| e.set(false));
    r
}

/// How a call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    /// `env::panic_str(m)`: near-sdk 5.29.1's mock turns the `panic_utf8` host
    /// call into `HostError(GuestPanic { panic_msg: m })` and unwraps it. On
    /// chain: `Smart contract panicked: m`.
    Guest(String),
    /// A plain Rust panic (`expect`, `assert!`) with this payload. On chain
    /// the panic hook reports it as `panicked at <file>:<line>:<col>:\n<msg>`.
    Rust(String),
}

impl Fail {
    pub fn msg(&self) -> &str {
        match self {
            Fail::Guest(m) | Fail::Rust(m) => m,
        }
    }
}

const PANIC_PREFIX: &str = "called `Result::unwrap()` on an `Err` value: HostError(GuestPanic { panic_msg: \"";
const PANIC_SUFFIX: &str = "\" })";

pub fn classify(payload: Box<dyn std::any::Any + Send>) -> Fail {
    let raw = if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else {
        return Fail::Rust("<non-string panic payload>".into());
    };
    match raw.strip_prefix(PANIC_PREFIX).and_then(|r| r.strip_suffix(PANIC_SUFFIX)) {
        Some(m) => Fail::Guest(m.to_string()),
        None => Fail::Rust(raw),
    }
}

/// Outcome of one call.
#[derive(Debug)]
pub struct Outcome<R> {
    pub result: Result<R, Fail>,
    /// Logs, including those of a failed call (they survive on chain too).
    pub logs: Vec<String>,
    /// Outgoing receipts (empty for a failed call: the runtime drops them).
    pub receipts: Vec<Receipt>,
    /// Storage after the call: written back on success, unchanged on failure.
    pub storage: Storage,
}

/// Runs one host-path call the way the runtime runs a receipt: fresh mocked
/// blockchain for `ctx` over `storage`, `state_read`, the method, then (for a
/// `&mut self` method, `write`) `state_write` on success. On a panic every
/// write and receipt is discarded. Promises must be dropped inside `f`
/// (near-sdk schedules a `Promise` when it is dropped).
pub fn call<R>(
    ctx: &Ctx,
    storage: Storage,
    results: Vec<PromiseResult>,
    write: bool,
    f: impl FnOnce(&mut Contract) -> R,
) -> Outcome<R> {
    call_t::<Contract, R>(ctx, storage, results, write, f)
}

/// `call` for any contract type (the executor too).
pub fn call_t<T, R>(
    ctx: &Ctx,
    storage: Storage,
    results: Vec<PromiseResult>,
    write: bool,
    f: impl FnOnce(&mut T) -> R,
) -> Outcome<R>
where
    T: near_sdk::borsh::BorshDeserialize + near_sdk::borsh::BorshSerialize,
{
    let before = storage.clone();
    install(ctx, storage, results, &[]);
    let mut c: T = env::state_read().expect("contract state");
    let r = catch_quiet(|| f(&mut c));
    let logs = logs();
    match r {
        Ok(v) => {
            let receipts = near_sdk::test_utils::get_created_receipts();
            if write {
                env::state_write(&c);
            }
            Outcome {
                result: Ok(v),
                logs,
                receipts,
                storage: take_storage(),
            }
        }
        Err(payload) => {
            let _ = take_storage();
            Outcome {
                result: Err(classify(payload)),
                logs,
                receipts: vec![],
                storage: before,
            }
        }
    }
}

/// A view on a contract freshly read from `storage`; checks it logged
/// nothing, created no receipt and wrote nothing.
pub fn view<R>(storage: &Storage, now: u64, f: impl FnOnce(&Contract) -> R) -> R {
    let ctx = Ctx::by(Z, now);
    install(&ctx, storage.clone(), vec![], &[]);
    let c: Contract = env::state_read().expect("contract state");
    let r = f(&c);
    assert!(logs().is_empty(), "a view logged: {:?}", logs());
    assert!(near_sdk::test_utils::get_created_receipts().is_empty(), "a view created receipts");
    drop(c);
    assert_eq!(&take_storage(), storage, "a view changed storage");
    r
}

/// Deploys `config` with `new`, called by the pool account itself at `now`.
pub fn deploy(config: Config, now: u64) -> Storage {
    let ctx = Ctx::by(POOL, now);
    install(&ctx, Storage::new(), vec![], &[]);
    let c = Contract::new(config);
    assert!(logs().is_empty(), "new() must not log");
    env::state_write(&c);
    drop(c);
    take_storage()
}

/// The storage a contract holding `pool` has: exactly `STATE` = Borsh(pool)
/// (`Contract` is a one-field struct).
pub fn storage_of(pool: &Pool) -> Storage {
    let mut s = Storage::new();
    s.insert(STATE_KEY.to_vec(), near_sdk::borsh::to_vec(pool).unwrap());
    s
}

pub fn pool_of(storage: &Storage) -> Pool {
    assert_eq!(storage.len(), 1, "storage keys: {:?}", storage.keys().collect::<Vec<_>>());
    near_sdk::borsh::from_slice(storage.get(STATE_KEY).expect("STATE")).expect("Borsh Pool")
}

// ---------------------------------------------------------------------------
// Operations: the contract's methods, and their expected effect from the core
// ---------------------------------------------------------------------------

/// One promise result a callback receives (near-sdk's `PromiseResult` is not
/// `Clone`): the raw bytes of a successful call, or a failed call.
#[derive(Clone, Debug, PartialEq)]
pub enum Res {
    Ok(Vec<u8>),
    Failed,
}

impl Res {
    /// The executor's usual answer: one JSON string.
    pub fn ok_str(s: &str) -> Res {
        Res::Ok(serde_json::to_vec(s).unwrap())
    }
    /// Any bytes (a number, `null`, an object, invalid UTF-8, ...).
    pub fn raw(b: &[u8]) -> Res {
        Res::Ok(b.to_vec())
    }
    pub fn failed() -> Res {
        Res::Failed
    }
    pub fn pr(&self) -> PromiseResult {
        match self {
            Res::Ok(b) => PromiseResult::Successful(b.clone()),
            Res::Failed => PromiseResult::Failed,
        }
    }
}

pub fn prs(r: &[Res]) -> Vec<PromiseResult> {
    r.iter().map(Res::pr).collect()
}

/// docs/api.md:40 (NT-F4): the status `on_result` reads from its promise
/// results. Exactly one result, a successful call, at most 64 bytes, and one
/// JSON string: that string. Anything else (a failed call, no result, two
/// results, more than 64 bytes, a number, `null`, an object, invalid bytes)
/// is "unknown".
pub fn outcome_of(results: &[Res]) -> String {
    match results {
        [Res::Ok(b)] if b.len() <= 64 => serde_json::from_slice::<String>(b).unwrap_or_else(|_| "unknown".into()),
        _ => "unknown".into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Submit { req: Request },
    Dispatch { id: u32 },
    Reconcile { id: u32 },
    /// The private callback with its raw promise results.
    OnResult { id: u32, results: Vec<Res> },
    Settle { id: u32, completed: bool },
    Cancel { id: u32 },
    Pause { paused: bool },
    Observe { values: Vec<i64> },
}

impl Op {
    /// `on_result(id)` after an executor answer: `Some(s)` the JSON string
    /// `s`, `None` a failed call (execute panicked).
    pub fn cb(id: u32, result: Option<&str>) -> Op {
        Op::OnResult {
            id,
            results: vec![result.map_or(Res::Failed, Res::ok_str)],
        }
    }
    pub fn method(&self) -> &'static str {
        match self {
            Op::Submit { .. } => "submit",
            Op::Dispatch { .. } => "dispatch",
            Op::Reconcile { .. } => "reconcile",
            Op::OnResult { .. } => "on_result",
            Op::Settle { .. } => "settle",
            Op::Cancel { .. } => "cancel",
            Op::Pause { .. } => "set_paused",
            Op::Observe { .. } => "observe",
        }
    }
    /// JSON arguments (docs/api.md table).
    pub fn args(&self) -> Value {
        match self {
            Op::Submit { req } => json!({ "request": request_json(req) }),
            Op::Dispatch { id } | Op::Reconcile { id } | Op::OnResult { id, .. } | Op::Cancel { id } => {
                json!({ "id": id })
            }
            Op::Settle { id, completed } => json!({ "id": id, "completed": completed }),
            Op::Pause { paused } => json!({ "paused": paused }),
            Op::Observe { values } => json!({ "values": values }),
        }
    }
    /// The promise results a callback receives.
    pub fn results(&self) -> Vec<PromiseResult> {
        match self {
            Op::OnResult { results, .. } => prs(results),
            _ => vec![],
        }
    }
}

pub fn request_json(r: &Request) -> Value {
    json!({ "recipe": r.recipe, "units": r.units, "version": r.version, "expires_at": r.expires_at })
}

/// Decision as docs/api.md describes it (field names of the README/api).
pub fn decision_json(d: &Decision) -> Value {
    json!({
        "allowed": d.allowed,
        "reason": d.reason,
        "before": d.before,
        "after": d.after,
        "terms_before": d.terms_before,
        "terms_after": d.terms_after,
        "candidate": d.candidate,
    })
}

pub fn status_json(s: &Status) -> &'static str {
    match s {
        Status::Reserved => "Reserved",
        Status::Dispatched => "Dispatched",
        Status::Active => "Active",
        Status::Unknown => "Unknown",
        Status::Completed => "Completed",
        Status::Cancelled => "Cancelled",
    }
}

pub fn commitment_json(c: &Commitment) -> Value {
    json!({ "id": c.id, "recipe": c.recipe, "units": c.units, "expires_at": c.expires_at, "status": status_json(&c.status) })
}

pub fn event(kind: &str, data: Value) -> Value {
    json!({ "standard": "shared_pool", "version": "1.0.0", "event": kind, "data": [data] })
}

pub fn parse_event(line: &str) -> Value {
    let body = line
        .strip_prefix("EVENT_JSON:")
        .unwrap_or_else(|| panic!("log is not an event: {line}"));
    serde_json::from_str(body).unwrap_or_else(|e| panic!("bad event JSON {body}: {e}"))
}

/// One outgoing function call, as a receipt carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub receiver: String,
    pub method: String,
    pub args: Value,
    pub deposit: u128,
    pub gas_tgas: u64,
    pub weight: u64,
    pub deps: Vec<u64>,
}

/// Receipts as `Call`s. Any action other than one function call per receipt
/// is reported as a `Call` with method `<forbidden ...>`.
pub fn calls_of(receipts: &[Receipt]) -> Vec<Call> {
    receipts
        .iter()
        .map(|r| {
            let deps = r.receipt_indices.iter().map(|i| *i as u64).collect();
            match r.actions.as_slice() {
                [MockAction::FunctionCallWeight {
                    method_name,
                    args,
                    attached_deposit,
                    prepaid_gas,
                    gas_weight,
                    ..
                }] => Call {
                    receiver: r.receiver_id.to_string(),
                    method: String::from_utf8(method_name.clone()).unwrap(),
                    args: serde_json::from_slice(args)
                        .unwrap_or_else(|_| Value::String(format!("<non-JSON {args:?}>"))),
                    deposit: attached_deposit.as_yoctonear(),
                    gas_tgas: prepaid_gas.as_gas() / 1_000_000_000_000,
                    weight: gas_weight.0,
                    deps,
                },
                other => Call {
                    receiver: r.receiver_id.to_string(),
                    method: format!("<forbidden {other:?}>"),
                    args: Value::Null,
                    deposit: 0,
                    gas_tgas: 0,
                    weight: 0,
                    deps,
                },
            }
        })
        .collect()
}

pub fn fcall(receiver: &str, method: &str, args: Value, tgas: u64, deps: Vec<u64>) -> Call {
    Call {
        receiver: receiver.into(),
        method: method.into(),
        args,
        deposit: 0,
        gas_tgas: tgas,
        // `Promise::function_call` (static gas, no weight): the runtime
        // records gas weight 0.
        weight: 0,
        deps,
    }
}

/// What the shell must do for one call, computed from the core.
#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    /// Return value (JSON; `null` for unit and Promise returns) or failure.
    pub result: Result<Value, Fail>,
    pub events: Vec<Value>,
    pub calls: Vec<Call>,
}

fn executor_of(pool: &Pool, c: &Commitment) -> String {
    pool.config.recipes[c.recipe as usize].executor.clone()
}

/// The expected effect of `op` by `ctx` on the reference `pool` (the core),
/// written from docs/api.md and the brief's shell semantics.
pub fn core_step(pool: &mut Pool, ctx: &Ctx, op: &Op) -> Expect {
    let caller = ctx.caller();
    let now = ctx.now();
    let guest = |e: Error| Expect {
        result: Err(Fail::Guest(e.to_string())),
        events: vec![],
        calls: vec![],
    };
    let unit = |kind: &str, data: Value| Expect {
        result: Ok(Value::Null),
        events: vec![event(kind, data)],
        calls: vec![],
    };
    match op {
        Op::Submit { req } => {
            let d = pool.preview(caller, req, now);
            let id = if d.allowed {
                match pool.submit(caller, req.clone(), now) {
                    Ok(id) => Some(id),
                    Err(e) => return guest(e),
                }
            } else {
                None
            };
            Expect {
                result: Ok(json!({ "decision": decision_json(&d), "id": id })),
                events: vec![event(
                    "decision",
                    json!({
                        "actor": caller, "request": request_json(req), "id": id,
                        "allowed": d.allowed, "reason": d.reason, "before": d.before,
                        "after": d.after, "version": pool.version,
                    }),
                )],
                calls: vec![],
            }
        }
        Op::Dispatch { id } => match pool.dispatch(caller, *id, now) {
            Err(e) => guest(e),
            Ok(()) => {
                let c = pool.record(*id).expect("dispatched record");
                Expect {
                    result: Ok(Value::Null),
                    events: vec![],
                    calls: vec![
                        fcall(
                            &executor_of(pool, c),
                            "execute",
                            json!({ "id": id, "recipe": c.recipe, "units": c.units }),
                            30,
                            vec![],
                        ),
                        fcall(ctx.current.as_str(), "on_result", json!({ "id": id }), 30, vec![0]),
                    ],
                }
            }
        },
        // docs/api.md:5: reconcile is nonpayable too (NT-F2 fixed: the shell's
        // own check, first); a missing or pruned id is the core's NotFound
        // through `must` (a Guest panic); the status guard is an `assert!`.
        Op::Reconcile { .. } if ctx.deposit > 0 => Expect {
            result: Err(Fail::Guest(RECONCILE_DEPOSIT.into())),
            events: vec![],
            calls: vec![],
        },
        Op::Reconcile { id } => match pool.record(*id) {
            Err(e) => guest(e),
            Ok(c) if !matches!(c.status, Status::Dispatched | Status::Active | Status::Unknown) => Expect {
                result: Err(Fail::Rust("InvalidState".into())),
                events: vec![],
                calls: vec![],
            },
            Ok(c) => Expect {
                result: Ok(Value::Null),
                events: vec![],
                calls: vec![
                    fcall(&executor_of(pool, c), "status", json!({ "id": id }), 10, vec![]),
                    fcall(ctx.current.as_str(), "on_result", json!({ "id": id }), 30, vec![0]),
                ],
            },
        },
        Op::OnResult { id, results } => {
            // docs/api.md:40: anything but one JSON string of at most 64
            // bytes is "unknown". docs/api.md:20: the callback never panics on
            // its input; a result for an id with no record (never issued, or
            // pruned) changes nothing and is logged with error NotFound.
            let status = outcome_of(results);
            let Ok(c) = pool.record(*id) else {
                return unit("reconciled", json!({ "id": id, "outcome": status, "error": "NotFound" }));
            };
            let exec = executor_of(pool, c);
            // docs/api.md: "completed" and "cancelled" are authoritative
            // terminal evidence; "active" acknowledges; runtime failures and
            // unexpected strings become Unknown.
            let r = match status.as_str() {
                "completed" => pool.settle(&exec, *id, true),
                "cancelled" => pool.settle(&exec, *id, false),
                "active" => pool.acknowledge(*id, true),
                _ => pool.acknowledge(*id, false),
            };
            unit(
                "reconciled",
                json!({ "id": id, "outcome": status, "error": r.err().map(|e| e.to_string()) }),
            )
        }
        Op::Settle { id, completed } => match pool.settle(caller, *id, *completed) {
            Err(e) => guest(e),
            Ok(()) => unit("settled", json!({ "id": id, "completed": completed })),
        },
        Op::Cancel { id } => match pool.cancel(caller, *id, now) {
            Err(e) => guest(e),
            Ok(()) => unit("cancelled", json!({ "id": id })),
        },
        Op::Pause { paused } => match pool.set_paused(caller, *paused) {
            Err(e) => guest(e),
            Ok(()) => unit("paused", json!({ "paused": paused })),
        },
        Op::Observe { values } => match pool.observe(caller, values.clone(), now) {
            Err(e) => guest(e),
            Ok(()) => unit("observed", json!({ "version": pool.version, "values": pool.values })),
        },
    }
}

/// Runs `op` through the contract's public method on the host path. The
/// return value is serialized to JSON (`null` for unit and Promise returns).
pub fn contract_step(ctx: &Ctx, storage: Storage, op: &Op) -> Outcome<Value> {
    let results = op.results();
    let op = op.clone();
    let write = !matches!(op, Op::Reconcile { .. }); // reconcile is &self
    call(ctx, storage, results, write, move |c| match op {
        Op::Submit { req } => serde_json::to_value(c.submit(req)).unwrap(),
        // Both return unit in v1.1.0; the receipt's result is the callback,
        // through promise_return (NT-F3).
        Op::Dispatch { id } => {
            c.dispatch(id);
            Value::Null
        }
        Op::Reconcile { id } => {
            c.reconcile(id);
            Value::Null
        }
        Op::OnResult { id, .. } => {
            #[cfg(not(nt_v100_contract))]
            c.on_result(id);
            #[cfg(nt_v100_contract)]
            c.on_result(id, v100_callback_result());
            Value::Null
        }
        Op::Settle { id, completed } => {
            c.settle(id, completed);
            Value::Null
        }
        Op::Cancel { id } => {
            c.cancel(id);
            Value::Null
        }
        Op::Pause { paused } => {
            c.set_paused(paused);
            Value::Null
        }
        Op::Observe { values } => {
            c.observe(values);
            Value::Null
        }
    })
}

/// Historical mutant s00a only (`run-mutants.sh` builds the harness with
/// `--cfg nt_v100_contract`): the v1.0.0 `on_result(id, #[callback_result]
/// result: Result<String, PromiseError>)` takes the argument near-sdk
/// 5.29.1's wrapper would read (near-sdk-macros attr_sig_info.rs:293-298).
#[cfg(nt_v100_contract)]
fn v100_callback_result() -> Result<String, near_sdk::PromiseError> {
    near_sdk::env::promise_result_checked(0, usize::MAX).map(|data| {
        serde_json::from_slice(&data)
            .unwrap_or_else(|e| near_sdk::env::panic_str(&format!("Failed to deserialize callback using JSON. Error: `{e}`")))
    })
}

/// One differential step on the host path: runs `op` on the contract (from
/// `storage`) and on the core (`pool`), and checks equal results, panics,
/// events, receipts and stored state (Borsh of the core's pool).
pub fn step(storage: Storage, pool: &mut Pool, ctx: &Ctx, op: &Op, what: &str) -> Outcome<Value> {
    step_opt(storage, pool, ctx, op, what, true)
}

/// LIVE-ALL at the shell (F-1, F-2, F-2b fixed in v1.1.0): in a reachable
/// state no release or settlement fails `Bounds` or `Arithmetic`, whether it
/// comes as `cancel`, `settle` or the `on_result` callback. `exp` is the
/// core's effect of `op`; a crafted state (`Sim::edit`) may be unreachable,
/// so its steps skip this check.
pub fn check_live_all(op: &Op, exp: &Expect, what: &str) {
    let bad = |e: &str| matches!(e, "Bounds" | "Arithmetic");
    match (op, &exp.result) {
        (Op::Cancel { .. } | Op::Settle { .. }, Err(Fail::Guest(e))) => {
            assert!(!bad(e), "{what}: LIVE-ALL: {} failed {e}", op.method())
        }
        (Op::OnResult { .. }, Ok(_)) => {
            let e = &exp.events[0]["data"][0]["error"];
            assert!(!e.as_str().is_some_and(bad), "{what}: LIVE-ALL: on_result recorded {e}");
        }
        _ => {}
    }
}

/// reconcile's deposit check (NT-F2 fixed, docs/api.md:5).
pub const RECONCILE_DEPOSIT: &str = "Method reconcile doesn't accept deposit";

/// The failure the host path reports for the expected `f`. near-sdk 5.29.1's
/// `require!(c, m)` (src/utils/mod.rs:67-75) is `assert!(c, "{}", m)` when
/// debug_assertions are on, as in this host test build, and
/// `env::panic_str(m)` in the release wasm. The contract's one `require!` is
/// reconcile's deposit check, so on the host path that failure is a Rust
/// panic with the same message. The documented build's is a Guest panic
/// (checked on the wasm path and in `nt_f1`).
pub fn on_host(f: Fail) -> Fail {
    match f {
        Fail::Guest(m) if m == RECONCILE_DEPOSIT => Fail::Rust(m),
        f => f,
    }
}

/// `step`, with the LIVE-ALL check only when `live_all` (a reachable state).
pub fn step_opt(storage: Storage, pool: &mut Pool, ctx: &Ctx, op: &Op, what: &str, live_all: bool) -> Outcome<Value> {
    let before = pool.clone();
    let out = contract_step(ctx, storage.clone(), op);
    let exp = core_step(pool, ctx, op);
    if live_all {
        check_live_all(op, &exp, what);
    }
    assert_eq!(out.result, exp.result.clone().map_err(on_host), "{what}: result");
    let events: Vec<Value> = out.logs.iter().map(|l| parse_event(l)).collect();
    assert_eq!(events, exp.events, "{what}: events");
    assert_eq!(calls_of(&out.receipts), exp.calls, "{what}: receipts");
    if exp.result.is_err() {
        // SH-5 / TX-0: a failed call leaves the Pool unchanged.
        assert_eq!(*pool, before, "{what}: core changed on Err");
        assert_eq!(out.storage, storage, "{what}: storage changed on a failed call");
    }
    assert_eq!(out.storage, storage_of(pool), "{what}: stored state != Borsh(core pool)");
    out
}

/// Every view equals the core's read (SH-9), on a contract reloaded from
/// storage: `get_pool`, `get_commitments` at the edges, and `preview`.
pub fn check_views(storage: &Storage, pool: &Pool, rng: &mut Rng, now: u64, what: &str) {
    let got = view(storage, now, |c| c.get_pool());
    assert_eq!(got, *pool, "{what}: get_pool");
    let first = pool.first_id;
    let next = first + pool.records.len() as u32;
    let mut pages = vec![
        (0, 0),
        (0, 50),
        (0, 51),
        (0, u32::MAX),
        (first.saturating_sub(1), 3),
        (first, 50),
        (next, 50),
        (next.saturating_add(1), 1),
        (u32::MAX, u32::MAX),
    ];
    for _ in 0..3 {
        let from = first.saturating_sub(2) + rng.below(next as u64 - first.saturating_sub(2) as u64 + 3) as u32;
        pages.push((from, rng.below(60) as u32));
    }
    for (from, limit) in pages {
        let got = view(storage, now, |c| c.get_commitments(from, limit));
        assert_eq!(got, expected_page(pool, from, limit), "{what}: get_commitments({from}, {limit})");
    }
    for _ in 0..2 {
        let accts = accounts_of(&pool.config);
        let actor = rng.pick(&accts).clone();
        let req = random_request(rng, pool, now);
        let got = view(storage, now, |c| c.preview(acct(&actor), req.clone()));
        assert_eq!(got, pool.preview(&actor, &req, now), "{what}: preview({actor}, {req:?})");
    }
}

/// docs/api.md:11: "Read page, max 50, from id `from`. Ids below `first_id`
/// were pruned; a page starting there begins at `first_id`."
pub fn expected_page(pool: &Pool, from: u32, limit: u32) -> Vec<Commitment> {
    pool.records.iter().filter(|c| c.id >= from).take(limit.min(50) as usize).cloned().collect()
}

// ---------------------------------------------------------------------------
// Configurations
// ---------------------------------------------------------------------------

fn metric(min: i64, max: i64, initial: i64) -> Metric {
    Metric {
        name: "m".into(),
        unit: "u".into(),
        initial,
        min,
        max,
    }
}

pub fn recipe(actor: &str, executor: &str, reserve: Vec<i64>, complete: Vec<i64>, max_units: u32) -> Recipe {
    Recipe {
        name: "r".into(),
        actor: actor.into(),
        executor: executor.into(),
        reserve,
        complete,
        max_units,
        requires: vec![],
    }
}

/// F-1 (brief): metrics m(0,10,5), m(-100,100,5); term t(1,0,1,target);
/// recipe hedge(a1, x1, reserve [-2,0], complete [2,-5], max 1).
pub fn wedge_config() -> Config {
    Config {
        name: "wedge".into(),
        owner: OWNER.into(),
        metrics: vec![metric(0, 10, 5), metric(-100, 100, 5)],
        terms: vec![Term {
            metric: 1,
            target: 0,
            weight: 1,
            shape: Shape::Target,
        }],
        recipes: vec![recipe(A1, X1, vec![-2, 0], vec![2, -5], 1)],
        max_age_seconds: 0,
    }
}

/// A pool where every request of recipe 0 (by a1, executor x1, up to 100
/// units) is admissible: zero effects, energy constant. Recipe 1 (a2, x2)
/// moves metric 0 by +1 per unit on completion.
pub fn open_config() -> Config {
    Config {
        name: "open".into(),
        owner: OWNER.into(),
        metrics: vec![metric(-1000, 1000, 0)],
        terms: vec![Term {
            metric: 0,
            target: 0,
            weight: 1,
            shape: Shape::Excess,
        }],
        recipes: vec![
            recipe(A1, X1, vec![0], vec![0], 100),
            recipe(A2, X2, vec![0], vec![-1], 100),
        ],
        max_age_seconds: 0,
    }
}

/// Every account a configuration names, plus an outsider and the pool.
pub fn accounts_of(c: &Config) -> Vec<String> {
    let mut v = vec![c.owner.clone(), Z.to_string(), POOL.to_string()];
    for r in &c.recipes {
        v.push(r.actor.clone());
        v.push(r.executor.clone());
    }
    v.sort();
    v.dedup();
    v
}

/// A random small configuration that `Config::validate` accepts (the
/// conformance generator's distribution, with NEAR account names).
pub fn random_config(rng: &mut Rng) -> Config {
    loop {
        let n = rng.range(1, 3) as usize;
        let metrics: Vec<Metric> = (0..n)
            .map(|i| {
                let min = rng.range(-5, 0);
                let max = min + rng.range(2, 10);
                Metric {
                    name: format!("m{i}"),
                    unit: "u".into(),
                    initial: rng.range(min, max),
                    min,
                    max,
                }
            })
            .collect();
        let terms = (0..rng.range(1, 3))
            .map(|_| {
                let metric = rng.range(0, n as i64 - 1) as u32;
                let m = &metrics[metric as usize];
                Term {
                    metric,
                    target: rng.range(m.min - 2, m.max + 2),
                    weight: rng.range(1, 3) as u32,
                    shape: match rng.range(0, 2) {
                        0 => Shape::Target,
                        1 => Shape::Excess,
                        _ => Shape::Deficit,
                    },
                }
            })
            .collect();
        let nr = rng.range(1, 3) as usize;
        let vec = |rng: &mut Rng| -> Vec<i64> {
            (0..n)
                .map(|_| if rng.chance(40) { 0 } else { rng.range(-3, 3) })
                .collect()
        };
        let recipes = (0..nr)
            .map(|i| {
                let requires = if i > 0 && rng.chance(30) {
                    vec![(rng.range(0, i as i64 - 1) as u32, rng.range(1, 3) as u32)]
                } else {
                    vec![]
                };
                Recipe {
                    name: format!("r{i}"),
                    actor: rng.pick(&[A1, A2]).to_string(),
                    executor: rng.pick(&[X1, X2]).to_string(),
                    reserve: vec(rng),
                    complete: vec(rng),
                    max_units: rng.range(1, 3) as u32,
                    requires,
                }
            })
            .collect();
        let c = Config {
            name: "rand".into(),
            owner: OWNER.into(),
            metrics,
            terms,
            recipes,
            max_age_seconds: if rng.chance(25) { rng.range(20, 60) as u32 } else { 0 },
        };
        if c.validate().is_ok() {
            return c;
        }
    }
}

// ---------------------------------------------------------------------------
// Random calls
// ---------------------------------------------------------------------------

/// xorshift64 (the conformance generator's).
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x2545_f491_4f6c_dd1d);
        if r.0 == 0 {
            r.0 = 1;
        }
        for _ in 0..4 {
            r.next();
        }
        r
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    pub fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
}

pub fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|v| v.parse().unwrap_or_else(|_| panic!("{name}={v} is not a number")))
        .unwrap_or(default)
}

pub fn random_request(rng: &mut Rng, pool: &Pool, now: u64) -> Request {
    let cfg = &pool.config;
    let recipe = if rng.chance(95) {
        rng.range(0, cfg.recipes.len() as i64 - 1)
    } else {
        cfg.recipes.len() as i64 + rng.range(0, 1)
    } as u32;
    let maxu = cfg.recipes.get(recipe as usize).map_or(3, |r| r.max_units) as i64;
    let units = if rng.chance(90) {
        rng.range(1, maxu)
    } else {
        rng.range(0, maxu + 1)
    } as u32;
    let version = if rng.chance(90) {
        pool.version
    } else {
        pool.version.wrapping_add(rng.range(0, 2) as u32).wrapping_sub(1)
    };
    let expires_at = if rng.chance(92) {
        now + rng.range(1, 60) as u64
    } else if rng.chance(50) {
        now
    } else {
        now + 86_401
    };
    Request {
        recipe,
        units,
        version,
        expires_at,
    }
}

/// Random raw promise results for `on_result`: mostly one status string, and
/// every malformed shape docs/api.md:40 maps to "unknown".
pub fn random_results(rng: &mut Rng) -> Vec<Res> {
    let one = match rng.below(24) {
        0..=6 => Res::ok_str("active"),
        7..=11 => Res::ok_str("completed"),
        12..=14 => Res::ok_str("cancelled"),
        15 => Res::ok_str("unknown"),
        16 => Res::ok_str(rng.pick(&["garbage", "Completed", "", " completed", "bogus"])),
        17 => Res::raw(rng.pick(&[&b"7"[..], b"null", b"{}", b"[\"completed\"]", b"completed", b"", b"\"compl\xffeted\""])),
        18 => {
            // "completed" padded with trailing JSON whitespace: 64 bytes are
            // read, 65 are "unknown" (upstream nt_f4 test).
            let pad = rng.range(0, 1) as usize;
            Res::raw(format!("\"completed\"{}", " ".repeat(53 + pad)).as_bytes())
        }
        19 => Res::raw(format!("\"{}\"", "x".repeat(10_000)).as_bytes()),
        20 => return vec![],
        21 => return vec![Res::ok_str("completed"), Res::ok_str("completed")],
        _ => Res::failed(),
    };
    vec![one]
}

/// A random next call, biased toward calls whose guards pass, with every
/// shell path reachable (relayed calls, on_result outcomes, reconcile).
pub fn random_step(rng: &mut Rng, pool: &Pool, ts_ns: u64) -> (Ctx, Op) {
    let cfg = &pool.config;
    let now = ts_ns / 1_000_000_000;
    let accts = accounts_of(cfg);
    let first = pool.first_id as i64;
    let nrec = pool.records.len() as i64;
    let open: Vec<u32> = pool
        .records
        .iter()
        .filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled))
        .map(|c| c.id)
        .collect();
    // Ids are `first_id + k` (v1.1.0 window); ids below `first_id` were
    // pruned, and one past the newest was never issued.
    let id = |rng: &mut Rng| -> u32 {
        if first > 0 && rng.chance(3) {
            rng.range(0, first - 1) as u32
        } else if nrec == 0 || rng.chance(5) {
            rng.range(first, first + nrec + 1) as u32
        } else if !open.is_empty() && rng.chance(80) {
            *rng.pick(&open)
        } else {
            rng.range(first, first + nrec - 1) as u32
        }
    };
    // The predecessor decides; the signer is usually the same account, and
    // sometimes another one (a relayed call: SH-1).
    let ctx = |rng: &mut Rng, who: &str| -> Ctx {
        let signer = if rng.chance(15) { rng.pick(&accts).clone() } else { who.to_string() };
        Ctx::new(who, &signer, ts_ns)
    };
    let other = |rng: &mut Rng| rng.pick(&accts).clone();
    let role = |rng: &mut Rng, i: u32, pick: fn(&Recipe) -> &String, pct: u64| -> String {
        match pool.record(i) {
            Ok(c) if rng.chance(pct) => pick(&cfg.recipes[c.recipe as usize]).clone(),
            _ => rng.pick(&accts).clone(),
        }
    };
    let k = rng.below(100);
    if k < 26 {
        let req = random_request(rng, pool, now);
        let who = match cfg.recipes.get(req.recipe as usize) {
            Some(r) if rng.chance(85) => r.actor.clone(),
            _ => other(rng),
        };
        (ctx(rng, &who), Op::Submit { req })
    } else if k < 42 {
        let i = id(rng);
        let who = role(rng, i, |r| &r.actor, 85);
        (ctx(rng, &who), Op::Dispatch { id: i })
    } else if k < 50 {
        // NT-F2 fixed: an attached deposit is refused, first (docs/api.md:5).
        let i = id(rng);
        let who = other(rng);
        let c = ctx(rng, &who);
        let c = if rng.chance(4) { c.with_deposit(1) } else { c };
        (c, Op::Reconcile { id: i })
    } else if k < 62 {
        // The private callback (the pool calls itself after execute/status).
        // The callback reads the raw promise results itself (NT-F4 fixed):
        // anything but exactly one JSON string of at most 64 bytes is
        // "unknown" (docs/api.md:40).
        let i = id(rng);
        let results = random_results(rng);
        (Ctx::new(POOL, rng.pick(&accts), ts_ns), Op::OnResult { id: i, results })
    } else if k < 68 {
        let i = id(rng);
        let who = match pool.record(i) {
            Ok(c) if rng.chance(80) => {
                if rng.chance(70) {
                    cfg.recipes[c.recipe as usize].actor.clone()
                } else {
                    cfg.owner.clone()
                }
            }
            _ => other(rng),
        };
        (ctx(rng, &who), Op::Cancel { id: i })
    } else if k < 86 {
        let i = id(rng);
        let who = role(rng, i, |r| &r.executor, 85);
        (ctx(rng, &who), Op::Settle { id: i, completed: rng.chance(70) })
    } else if k < 91 {
        let who = if rng.chance(85) { cfg.owner.clone() } else { other(rng) };
        let b = if pool.paused { rng.chance(20) } else { rng.chance(30) };
        (ctx(rng, &who), Op::Pause { paused: b })
    } else {
        let who = if rng.chance(90) { cfg.owner.clone() } else { other(rng) };
        let mut v: Vec<i64> = cfg
            .metrics
            .iter()
            .map(|m| {
                if rng.chance(70) {
                    rng.range(m.min, m.max)
                } else {
                    rng.range(m.min - 3, m.max + 3)
                }
            })
            .collect();
        if rng.chance(4) {
            v.pop();
        } else if rng.chance(3) {
            v[0] = pool_core::MAX_VALUE + 1;
        }
        (ctx(rng, &who), Op::Observe { values: v })
    }
}

// ---------------------------------------------------------------------------
// Wasm path
// ---------------------------------------------------------------------------

/// How the wasm under test was built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Build {
    /// `cargo build --target wasm32-unknown-unknown --release --locked` with
    /// no `--cfg near`: the v1.0.0 documented build (NT-F1). v1.1.0's
    /// build.sh refuses it (SURF-3(b)(i)); it is kept only as that negative
    /// control and for the NT-F1 regression test.
    Plain,
    /// upstream `scripts/build.sh` at v1.1.0: `RUSTFLAGS=--cfg near`, as
    /// cargo-near builds (near-sdk's on-chain cfg). SURF-3(a) checks its bytes
    /// equal build.sh's `artifacts/build.json`.
    Near,
}

impl Build {
    /// The build under test: the documented one (NT-F1 fixed).
    pub fn all() -> [Build; 1] {
        [Build::Near]
    }
    fn tag(self) -> &'static str {
        match self {
            Build::Plain => "plain",
            Build::Near => "near",
        }
    }
}

/// Host-side record of one wasm execution.
#[derive(Default)]
pub struct Host {
    /// Import names called, in order.
    pub calls: Vec<&'static str>,
    pub panic_msg: Option<String>,
    pub returned: Option<Vec<u8>>,
    pub promise_return: Option<u64>,
    pub forbidden: Vec<String>,
}

pub struct Wasm {
    pub build: Build,
    pub name: String,
    pub path: PathBuf,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    engine: wasmi::Engine,
    module: wasmi::Module,
    linker: wasmi::Linker<Host>,
}

/// Builds both wasm files of the pinned (or, under run-mutants.sh, patched)
/// source with the given build, once per process. `POOL_WASM_DIR_PLAIN` /
/// `POOL_WASM_DIR_NEAR` point at prebuilt `.../wasm32-unknown-unknown/release`
/// directories instead.
fn wasm_dir(build: Build) -> PathBuf {
    static DIRS: OnceLock<std::sync::Mutex<HashMap<Build, PathBuf>>> = OnceLock::new();
    let mut dirs = DIRS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = dirs.get(&build) {
        return d.clone();
    }
    let var = format!("POOL_WASM_DIR_{}", build.tag().to_uppercase());
    let dir = if let Ok(d) = std::env::var(&var) {
        PathBuf::from(d)
    } else {
        let here = FsPath::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = here.join("../source-mirror/Cargo.toml");
        let target = std::env::var("POOL_WASM_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| here.join("target/wasm"))
            .join(build.tag());
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        // From the source-mirror root, as build.sh runs: the same
        // rust-toolchain.toml and cargo configuration (SURF-3(a) checks the
        // bytes are build.sh's).
        cmd.current_dir(here.join("../source-mirror"))
            .args(["build", "--manifest-path"])
            .arg(&manifest)
            .args([
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--locked",
                "-p",
                "pool-contract",
                "-p",
                "pool-executor",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("CARGO_BUILD_RUSTFLAGS")
            .env_remove("RUSTFLAGS");
        if build == Build::Near {
            cmd.env("RUSTFLAGS", "--cfg near");
        }
        let out = cmd.output().expect("run cargo build for the wasm");
        assert!(
            out.status.success(),
            "wasm build ({}) failed:\n{}",
            build.tag(),
            String::from_utf8_lossy(&out.stderr)
        );
        target.join("wasm32-unknown-unknown/release")
    };
    dirs.insert(build, dir.clone());
    dir
}

/// The wasm `name` ("pool_contract" | "pool_executor") of `build`, loaded
/// once per process.
pub fn wasm(build: Build, name: &str) -> &'static Wasm {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<(Build, String), &'static Wasm>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = cache.get(&(build, name.to_string())) {
        return w;
    }
    let path = wasm_dir(build).join(format!("{name}.wasm"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, &bytes).expect("valid wasm");
    let imports: Vec<String> = module.imports().map(|i| format!("{}.{}", i.module(), i.name())).collect();
    let exports: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();
    let mut linker = wasmi::Linker::<Host>::new(&engine);
    bridge(&mut linker);
    // Imports the shell must never call: they trap and are recorded.
    for imp in module.imports() {
        if imp.module() == "env" && BRIDGED.contains(&imp.name()) {
            continue;
        }
        let ty = imp.ty().func().expect("function import").clone();
        let name: &'static str = Box::leak(imp.name().to_string().into_boxed_str());
        linker
            .func_new("env", name, ty, move |mut caller, _args, _res| {
                caller.data_mut().calls.push(name);
                caller.data_mut().forbidden.push(name.to_string());
                Err(wasmi::Error::new(format!("forbidden host call: {name}")))
            })
            .unwrap();
    }
    let w: &'static Wasm = Box::leak(Box::new(Wasm {
        build,
        name: name.to_string(),
        path,
        imports,
        exports,
        engine,
        module,
        linker,
    }));
    cache.insert((build, name.to_string()), w);
    w
}

/// The imports forwarded to the mocked VMLogic.
const BRIDGED: &[&str] = &[
    "read_register",
    "register_len",
    "current_account_id",
    "predecessor_account_id",
    "signer_account_id",
    "input",
    "block_timestamp",
    "block_index",
    "attached_deposit",
    "storage_read",
    "storage_write",
    "storage_has_key",
    "storage_remove",
    "value_return",
    "log_utf8",
    "panic_utf8",
    "panic",
    "promise_result",
    "promise_results_count",
    "promise_create",
    "promise_then",
    // v1.0.0's Promise type (NT-F3): kept so the historical mutant s00a runs.
    "promise_batch_create",
    "promise_batch_then",
    "promise_batch_action_function_call",
    "promise_batch_action_function_call_weight",
    "promise_return",
    "promise_and",
];

type C<'a> = wasmi::Caller<'a, Host>;
type R<T> = Result<T, wasmi::Error>;

fn memory(c: &C) -> wasmi::Memory {
    c.get_export("memory").and_then(|e| e.into_memory()).expect("exported memory")
}
fn rd(c: &C, ptr: u64, len: u64) -> R<Vec<u8>> {
    let mut buf = vec![0u8; len as usize];
    memory(c)
        .read(c, ptr as usize, &mut buf)
        .map_err(|e| wasmi::Error::new(format!("guest memory read: {e}")))?;
    Ok(buf)
}
fn wr(c: &mut C, ptr: u64, data: &[u8]) -> R<()> {
    let m = memory(c);
    m.write(c, ptr as usize, data)
        .map_err(|e| wasmi::Error::new(format!("guest memory write: {e}")))
}
fn p(v: &[u8]) -> u64 {
    v.as_ptr() as u64
}

fn bridge(l: &mut wasmi::Linker<Host>) {
    use near_sys as sys;
    macro_rules! log_call {
        ($c:expr, $n:literal) => {
            $c.data_mut().calls.push($n)
        };
    }
    l.func_wrap("env", "read_register", |mut c: C, reg: u64, ptr: u64| -> R<()> {
        log_call!(c, "read_register");
        let len = unsafe { sys::register_len(reg) };
        let buf = vec![0u8; if len == u64::MAX { 0 } else { len as usize }];
        unsafe { sys::read_register(reg, p(&buf)) };
        wr(&mut c, ptr, &buf)
    })
    .unwrap();
    l.func_wrap("env", "register_len", |mut c: C, reg: u64| -> u64 {
        log_call!(c, "register_len");
        unsafe { sys::register_len(reg) }
    })
    .unwrap();
    l.func_wrap("env", "current_account_id", |mut c: C, reg: u64| {
        log_call!(c, "current_account_id");
        unsafe { sys::current_account_id(reg) }
    })
    .unwrap();
    l.func_wrap("env", "predecessor_account_id", |mut c: C, reg: u64| {
        log_call!(c, "predecessor_account_id");
        unsafe { sys::predecessor_account_id(reg) }
    })
    .unwrap();
    l.func_wrap("env", "signer_account_id", |mut c: C, reg: u64| {
        log_call!(c, "signer_account_id");
        unsafe { sys::signer_account_id(reg) }
    })
    .unwrap();
    l.func_wrap("env", "input", |mut c: C, reg: u64| {
        log_call!(c, "input");
        unsafe { sys::input(reg) }
    })
    .unwrap();
    l.func_wrap("env", "block_timestamp", |mut c: C| -> u64 {
        log_call!(c, "block_timestamp");
        unsafe { sys::block_timestamp() }
    })
    .unwrap();
    l.func_wrap("env", "block_index", |mut c: C| -> u64 {
        log_call!(c, "block_index");
        unsafe { sys::block_index() }
    })
    .unwrap();
    l.func_wrap("env", "attached_deposit", |mut c: C, ptr: u64| -> R<()> {
        log_call!(c, "attached_deposit");
        let buf = [0u8; 16];
        unsafe { sys::attached_deposit(p(&buf)) };
        wr(&mut c, ptr, &buf)
    })
    .unwrap();
    l.func_wrap("env", "storage_read", |mut c: C, kl: u64, kp: u64, reg: u64| -> R<u64> {
        log_call!(c, "storage_read");
        let k = rd(&c, kp, kl)?;
        Ok(unsafe { sys::storage_read(kl, p(&k), reg) })
    })
    .unwrap();
    l.func_wrap(
        "env",
        "storage_write",
        |mut c: C, kl: u64, kp: u64, vl: u64, vp: u64, reg: u64| -> R<u64> {
            log_call!(c, "storage_write");
            let k = rd(&c, kp, kl)?;
            let v = rd(&c, vp, vl)?;
            Ok(unsafe { sys::storage_write(kl, p(&k), vl, p(&v), reg) })
        },
    )
    .unwrap();
    l.func_wrap("env", "storage_has_key", |mut c: C, kl: u64, kp: u64| -> R<u64> {
        log_call!(c, "storage_has_key");
        let k = rd(&c, kp, kl)?;
        Ok(unsafe { sys::storage_has_key(kl, p(&k)) })
    })
    .unwrap();
    l.func_wrap("env", "storage_remove", |mut c: C, kl: u64, kp: u64, reg: u64| -> R<u64> {
        log_call!(c, "storage_remove");
        let k = rd(&c, kp, kl)?;
        Ok(unsafe { sys::storage_remove(kl, p(&k), reg) })
    })
    .unwrap();
    l.func_wrap("env", "value_return", |mut c: C, len: u64, ptr: u64| -> R<()> {
        log_call!(c, "value_return");
        let v = rd(&c, ptr, len)?;
        unsafe { sys::value_return(len, p(&v)) };
        c.data_mut().returned = Some(v);
        Ok(())
    })
    .unwrap();
    l.func_wrap("env", "log_utf8", |mut c: C, len: u64, ptr: u64| -> R<()> {
        log_call!(c, "log_utf8");
        let v = rd(&c, ptr, len)?;
        unsafe { sys::log_utf8(len, p(&v)) };
        Ok(())
    })
    .unwrap();
    // A guest panic: recorded and turned into a trap (VMLogic would only
    // return GuestPanic; calling the mock would unwind through extern "C").
    l.func_wrap("env", "panic_utf8", |mut c: C, len: u64, ptr: u64| -> R<()> {
        log_call!(c, "panic_utf8");
        let v = rd(&c, ptr, len)?;
        let m = String::from_utf8_lossy(&v).to_string();
        c.data_mut().panic_msg = Some(m.clone());
        Err(wasmi::Error::new(format!("GuestPanic: {m}")))
    })
    .unwrap();
    l.func_wrap("env", "panic", |mut c: C| -> R<()> {
        log_call!(c, "panic");
        c.data_mut().panic_msg = Some("explicit guest panic".into());
        Err(wasmi::Error::new("GuestPanic: explicit guest panic"))
    })
    .unwrap();
    l.func_wrap("env", "promise_result", |mut c: C, idx: u64, reg: u64| -> u64 {
        log_call!(c, "promise_result");
        unsafe { sys::promise_result(idx, reg) }
    })
    .unwrap();
    l.func_wrap("env", "promise_results_count", |mut c: C| -> u64 {
        log_call!(c, "promise_results_count");
        unsafe { sys::promise_results_count() }
    })
    .unwrap();
    l.func_wrap(
        "env",
        "promise_create",
        |mut c: C, al: u64, ap: u64, fl: u64, fp: u64, gl: u64, gp: u64, amt: u64, gas: u64| -> R<u64> {
            log_call!(c, "promise_create");
            let a = rd(&c, ap, al)?;
            let f = rd(&c, fp, fl)?;
            let g = rd(&c, gp, gl)?;
            let m = rd(&c, amt, 16)?;
            Ok(unsafe { sys::promise_create(al, p(&a), fl, p(&f), gl, p(&g), p(&m), gas) })
        },
    )
    .unwrap();
    l.func_wrap(
        "env",
        "promise_then",
        |mut c: C, idx: u64, al: u64, ap: u64, fl: u64, fp: u64, gl: u64, gp: u64, amt: u64, gas: u64| -> R<u64> {
            log_call!(c, "promise_then");
            let a = rd(&c, ap, al)?;
            let f = rd(&c, fp, fl)?;
            let g = rd(&c, gp, gl)?;
            let m = rd(&c, amt, 16)?;
            Ok(unsafe { sys::promise_then(idx, al, p(&a), fl, p(&f), gl, p(&g), p(&m), gas) })
        },
    )
    .unwrap();
    l.func_wrap("env", "promise_batch_create", |mut c: C, al: u64, ap: u64| -> R<u64> {
        log_call!(c, "promise_batch_create");
        let a = rd(&c, ap, al)?;
        Ok(unsafe { sys::promise_batch_create(al, p(&a)) })
    })
    .unwrap();
    l.func_wrap("env", "promise_batch_then", |mut c: C, idx: u64, al: u64, ap: u64| -> R<u64> {
        log_call!(c, "promise_batch_then");
        let a = rd(&c, ap, al)?;
        Ok(unsafe { sys::promise_batch_then(idx, al, p(&a)) })
    })
    .unwrap();
    l.func_wrap(
        "env",
        "promise_batch_action_function_call",
        |mut c: C, idx: u64, fl: u64, fp: u64, al: u64, ap: u64, amt: u64, gas: u64| -> R<()> {
            log_call!(c, "promise_batch_action_function_call");
            let f = rd(&c, fp, fl)?;
            let a = rd(&c, ap, al)?;
            let m = rd(&c, amt, 16)?;
            unsafe { sys::promise_batch_action_function_call(idx, fl, p(&f), al, p(&a), p(&m), gas) };
            Ok(())
        },
    )
    .unwrap();
    l.func_wrap(
        "env",
        "promise_batch_action_function_call_weight",
        |mut c: C, idx: u64, fl: u64, fp: u64, al: u64, ap: u64, amt: u64, gas: u64, w: u64| -> R<()> {
            log_call!(c, "promise_batch_action_function_call_weight");
            let f = rd(&c, fp, fl)?;
            let a = rd(&c, ap, al)?;
            let m = rd(&c, amt, 16)?;
            unsafe {
                sys::promise_batch_action_function_call_weight(idx, fl, p(&f), al, p(&a), p(&m), gas, w)
            };
            Ok(())
        },
    )
    .unwrap();
    l.func_wrap("env", "promise_return", |mut c: C, idx: u64| {
        log_call!(c, "promise_return");
        unsafe { sys::promise_return(idx) };
        c.data_mut().promise_return = Some(idx);
    })
    .unwrap();
    l.func_wrap("env", "promise_and", |mut c: C, ptr: u64, n: u64| -> R<u64> {
        log_call!(c, "promise_and");
        let v = rd(&c, ptr, n * 8)?;
        Ok(unsafe { sys::promise_and(p(&v), n) })
    })
    .unwrap();
}

/// How a wasm call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmFail {
    /// The `panic_utf8` message, if the guest called it (`Smart contract
    /// panicked: <msg>` on chain). `None`: the guest trapped without one.
    pub panic_msg: Option<String>,
    /// The trap as wasmi reports it.
    pub trap: String,
}

#[derive(Debug)]
pub struct WasmOutcome {
    /// The `value_return` bytes (`None`: nothing returned) or the failure.
    pub result: Result<Option<Vec<u8>>, WasmFail>,
    pub logs: Vec<String>,
    /// Outgoing receipts (empty for a failed call).
    pub receipts: Vec<Receipt>,
    pub storage: Storage,
    /// Imports called, in order.
    pub host_calls: Vec<&'static str>,
    pub forbidden: Vec<String>,
    pub promise_return: Option<u64>,
}

impl WasmOutcome {
    pub fn json(&self) -> Value {
        match &self.result {
            Ok(Some(b)) => serde_json::from_slice(b).expect("JSON return"),
            Ok(None) => Value::Null,
            Err(e) => panic!("call failed: {e:?}"),
        }
    }
    pub fn fail(&self) -> &WasmFail {
        self.result.as_ref().expect_err("call succeeded")
    }
    pub fn called(&self, name: &str) -> bool {
        self.host_calls.contains(&name)
    }
}

/// Runs export `method` of `w` the way the runtime runs a function-call
/// receipt: a fresh instance, the mocked blockchain for `ctx` over `storage`
/// with `input` and the promise `results`. A failed call (trap) leaves
/// storage unchanged and sends no receipt.
pub fn wasm_call(
    w: &Wasm,
    ctx: &Ctx,
    storage: Storage,
    results: Vec<PromiseResult>,
    method: &str,
    input: &[u8],
) -> WasmOutcome {
    let before = storage.clone();
    install(ctx, storage, results, input);
    let mut store = wasmi::Store::new(&w.engine, Host::default());
    let inst = w
        .linker
        .instantiate_and_start(&mut store, &w.module)
        .expect("instantiate");
    let f = inst
        .get_typed_func::<(), ()>(&store, method)
        .unwrap_or_else(|e| panic!("export {method}: {e}"));
    let r = catch_quiet(|| f.call(&mut store, ()));
    let logs = logs();
    let host = std::mem::take(store.data_mut());
    let fail = match r {
        Ok(Ok(())) => None,
        Ok(Err(trap)) => Some(WasmFail {
            panic_msg: host.panic_msg.clone(),
            trap: trap.to_string(),
        }),
        Err(payload) => Some(WasmFail {
            panic_msg: None,
            trap: format!("host error: {:?}", classify(payload)),
        }),
    };
    let (result, receipts, storage) = match fail {
        None => (
            Ok(host.returned.clone()),
            near_sdk::test_utils::get_created_receipts(),
            take_storage(),
        ),
        Some(f) => {
            let _ = take_storage();
            (Err(f), vec![], before)
        }
    };
    WasmOutcome {
        result,
        logs,
        receipts,
        storage,
        host_calls: host.calls,
        forbidden: host.forbidden,
        promise_return: host.promise_return,
    }
}

/// Deploys the pool wasm with `new({config})`, called by the pool itself.
pub fn wasm_deploy(build: Build, config: &Config, now: u64) -> Storage {
    let w = wasm(build, "pool_contract");
    let out = wasm_call(
        w,
        &Ctx::by(POOL, now),
        Storage::new(),
        vec![],
        "new",
        &serde_json::to_vec(&json!({ "config": config })).unwrap(),
    );
    assert!(out.result.is_ok(), "new failed: {:?}", out.result);
    assert!(out.logs.is_empty());
    out.storage
}

/// Runs `op` through the wasm: JSON args in, JSON return out. The result is
/// the parsed return (`null` when nothing is returned) or the failure.
pub fn wasm_step(build: Build, ctx: &Ctx, storage: Storage, op: &Op) -> WasmOutcome {
    let w = wasm(build, "pool_contract");
    wasm_call(w, ctx, storage, op.results(), op.method(), &serde_json::to_vec(&op.args()).unwrap())
}

/// The panic message a build reports for an expected failure: the `Near`
/// build reports `env::panic_str` messages exactly and Rust panics through the
/// panic hook (`panicked at <file>:<l>:<c>:\n<msg>`); the `Plain` build
/// reports nothing (NT-F1).
pub fn expected_panic_msg(build: Build, f: &Fail) -> Option<String> {
    match build {
        Build::Plain => None,
        Build::Near => Some(f.msg().to_string()),
    }
}

/// Checks a wasm failure against the expected failure: no state change, no
/// receipts, and the message as the build reports it.
pub fn assert_wasm_fail(build: Build, out: &WasmOutcome, exp: &Fail, before: &Storage, what: &str) {
    let f = out
        .result
        .as_ref()
        .err()
        .unwrap_or_else(|| panic!("{what}: expected failure {exp:?}, got {:?}", out.result));
    assert!(out.forbidden.is_empty(), "{what}: forbidden host calls {:?}", out.forbidden);
    assert_eq!(&out.storage, before, "{what}: failed call changed storage");
    assert!(out.receipts.is_empty());
    match (build, exp) {
        (Build::Plain, _) => assert_eq!(f.panic_msg, None, "{what}: plain build reported a message"),
        (Build::Near, Fail::Guest(m)) => assert_eq!(f.panic_msg.as_deref(), Some(m.as_str()), "{what}"),
        (Build::Near, Fail::Rust(m)) => {
            let got = f.panic_msg.as_deref().unwrap_or("");
            assert!(
                got.starts_with("panicked at crates/contract/src/lib.rs:") && got.ends_with(&format!(":\n{m}")),
                "{what}: panic hook message {got:?}, expected ...:\\n{m}"
            );
        }
    }
}

/// One differential step on the wasm path: the same checks as `step`, with
/// the JSON return of the real wrappers.
pub fn wasm_diff_step(build: Build, storage: Storage, pool: &mut Pool, ctx: &Ctx, op: &Op, what: &str) -> WasmOutcome {
    wasm_diff_step_opt(build, storage, pool, ctx, op, what, true)
}

/// `wasm_diff_step`, with the LIVE-ALL check only when `live_all`.
pub fn wasm_diff_step_opt(
    build: Build,
    storage: Storage,
    pool: &mut Pool,
    ctx: &Ctx,
    op: &Op,
    what: &str,
    live_all: bool,
) -> WasmOutcome {
    let before = pool.clone();
    let out = wasm_step(build, ctx, storage.clone(), op);
    let exp = core_step(pool, ctx, op);
    if live_all {
        check_live_all(op, &exp, what);
    }
    assert!(out.forbidden.is_empty(), "{what}: forbidden host calls {:?}", out.forbidden);
    match &exp.result {
        Ok(v) => {
            assert!(out.result.is_ok(), "{what}: expected Ok, got {:?}", out.result);
            if !v.is_null() {
                assert_eq!(&out.json(), v, "{what}: return value");
            } else if matches!(op, Op::Dispatch { .. } | Op::Reconcile { .. }) {
                // v1.1.0 (NT-F3): `call_then_callback` returns the callback
                // (promise index 1: the executor call is 0) with
                // promise_return, and the method returns unit, so no
                // value_return. docs/api.md:3-4: "Promise to executor
                // execute, then on_result".
                assert_eq!(out.promise_return, Some(1), "{what}: the callback is not the returned promise");
                assert!(!out.called("value_return"), "{what}: value_return beside promise_return: {:?}", out.host_calls);
                assert_eq!(out.result.as_ref().unwrap(), &None, "{what}: value returned");
            } else {
                assert_eq!(out.result.as_ref().unwrap(), &None, "{what}: unit method returned a value");
            }
            let events: Vec<Value> = out.logs.iter().map(|l| parse_event(l)).collect();
            assert_eq!(events, exp.events, "{what}: events");
            assert_eq!(calls_of(&out.receipts), exp.calls, "{what}: receipts");
        }
        Err(f) => {
            assert_eq!(*pool, before);
            assert_wasm_fail(build, &out, f, &storage, what);
            // A failed receipt's logs survive; the shell logs nothing before
            // it fails, except the Plain build which logs nothing at all.
            assert!(out.logs.is_empty(), "{what}: logs {:?}", out.logs);
        }
    }
    assert_eq!(out.storage, storage_of(pool), "{what}: stored state != Borsh(core pool)");
    out
}

// ---------------------------------------------------------------------------
// Receipt router (wasm path, two contracts)
// ---------------------------------------------------------------------------

/// The pool and executor wasm deployed side by side, with a FIFO of receipts
/// executed in order: a callback receipt runs after the receipt it depends
/// on, with that receipt's result as its promise result (ASM-CALLBACK). A
/// receipt that returns a promise (`promise_return`) has that promise's
/// result as its own: that is how the transaction's result is found.
pub struct Net {
    pub build: Build,
    pub pool: Storage,
    pub exec: Storage,
    pub exec_id: String,
    pub now: u64,
    /// Every executed receipt: (receiver, method, outcome summary).
    pub trace: Vec<(String, String, Result<Option<Vec<u8>>, WasmFail>, Vec<String>)>,
    /// The last `tx`'s result: its first receipt's, following
    /// `promise_return` to the returned receipt (`Err(())`: it failed).
    pub result: Option<Result<Vec<u8>, ()>>,
}

pub struct Pending {
    pub predecessor: String,
    pub receiver: String,
    pub method: String,
    pub args: Vec<u8>,
    pub deps: Vec<usize>,
}

impl Net {
    /// Pool with `config` (whose recipes must name `exec_id` as executor) and
    /// executor (`new({lock: pool, operator})`).
    pub fn new(build: Build, config: &Config, exec_id: &str, now: u64) -> Net {
        let pool = wasm_deploy(build, config, now);
        let w = wasm(build, "pool_executor");
        let out = wasm_call(
            w,
            &Ctx::by(exec_id, now).on(exec_id),
            Storage::new(),
            vec![],
            "new",
            &serde_json::to_vec(&json!({ "lock": POOL, "operator": OPERATOR })).unwrap(),
        );
        assert!(out.result.is_ok(), "executor new: {:?}", out.result);
        Net {
            build,
            pool,
            exec: out.storage,
            exec_id: exec_id.into(),
            now,
            trace: vec![],
            result: None,
        }
    }

    fn storage_mut(&mut self, receiver: &str) -> &mut Storage {
        if receiver == POOL {
            &mut self.pool
        } else if receiver == self.exec_id {
            &mut self.exec
        } else {
            panic!("no contract at {receiver}")
        }
    }

    /// Runs one call by `signer` (= predecessor) and every receipt it spawns,
    /// to completion. Returns the outcome of the first call.
    pub fn tx(&mut self, signer: &str, receiver: &str, method: &str, args: Value) -> WasmOutcome {
        let first = self.exec_one(signer, receiver, method, &serde_json::to_vec(&args).unwrap(), vec![]);
        let mut queue: Vec<(Pending, Option<usize>)> = vec![];
        let mut results: Vec<Option<Result<Vec<u8>, ()>>> = vec![];
        // Queue position of the promise each executed receipt returned.
        let ret_of = |out: &WasmOutcome, off: usize| out.promise_return.map(|k| off + k as usize);
        let first_ret = if first.result.is_ok() { ret_of(&first, 0) } else { None };
        let mut rets: Vec<Option<usize>> = vec![];
        Self::enqueue(&mut queue, &mut results, receiver, &first, 0);
        rets.resize(queue.len(), None);
        let mut i = 0;
        while i < queue.len() {
            let (p, _) = &queue[i];
            let deps: Vec<PromiseResult> = p
                .deps
                .iter()
                .map(|d| match self.final_result(&results, &rets, *d) {
                    Ok(v) => PromiseResult::Successful(v),
                    Err(()) => PromiseResult::Failed,
                })
                .collect();
            let (pred, recv, meth, args) = (p.predecessor.clone(), p.receiver.clone(), p.method.clone(), p.args.clone());
            let out = self.exec_one(&pred, &recv, &meth, &args, deps);
            results[i] = Some(match &out.result {
                Ok(v) => Ok(v.clone().unwrap_or_default()),
                Err(_) => Err(()),
            });
            let off = queue.len();
            if out.result.is_ok() {
                rets[i] = ret_of(&out, off);
            }
            Self::enqueue(&mut queue, &mut results, &recv, &out, off);
            rets.resize(queue.len(), None);
            i += 1;
        }
        self.result = Some(match (&first.result, first_ret) {
            (Err(_), _) => Err(()),
            (Ok(_), Some(r)) => self.final_result(&results, &rets, r),
            (Ok(v), None) => Ok(v.clone().unwrap_or_default()),
        });
        first
    }

    /// The result of queued receipt `i`, or of the promise it returned.
    fn final_result(&self, results: &[Option<Result<Vec<u8>, ()>>], rets: &[Option<usize>], mut i: usize) -> Result<Vec<u8>, ()> {
        while let Some(j) = rets[i] {
            i = j;
        }
        results[i].clone().expect("dependency ran first")
    }

    fn enqueue(
        queue: &mut Vec<(Pending, Option<usize>)>,
        results: &mut Vec<Option<Result<Vec<u8>, ()>>>,
        from: &str,
        out: &WasmOutcome,
        offset: usize,
    ) {
        for c in calls_of(&out.receipts) {
            assert!(!c.method.starts_with("<forbidden"), "forbidden action {c:?}");
            queue.push((
                Pending {
                    predecessor: from.to_string(),
                    receiver: c.receiver.clone(),
                    method: c.method.clone(),
                    args: serde_json::to_vec(&c.args).unwrap(),
                    deps: c.deps.iter().map(|d| offset + *d as usize).collect(),
                },
                None,
            ));
            results.push(None);
        }
    }

    fn exec_one(&mut self, pred: &str, receiver: &str, method: &str, args: &[u8], results: Vec<PromiseResult>) -> WasmOutcome {
        let name = if receiver == POOL { "pool_contract" } else { "pool_executor" };
        let w = wasm(self.build, name);
        let ctx = Ctx::new(pred, pred, self.now * 1_000_000_000 + 1).on(receiver);
        let storage = self.storage_mut(receiver).clone();
        let out = wasm_call(w, &ctx, storage, results, method, args);
        assert!(out.forbidden.is_empty(), "forbidden host calls {:?}", out.forbidden);
        *self.storage_mut(receiver) = out.storage.clone();
        self.trace.push((receiver.to_string(), method.to_string(), out.result.clone(), out.logs.clone()));
        out
    }

    pub fn pool_state(&self) -> Pool {
        pool_of(&self.pool)
    }

    /// The executor's stored state (Borsh mirror; `newest` is private).
    pub fn exec_state(&self) -> ExecM {
        decode_exec(&self.exec)
    }

    /// The executor's jobs through its `get_jobs` view.
    pub fn jobs(&self) -> Value {
        let w = wasm(self.build, "pool_executor");
        let out = wasm_call(w, &Ctx::by(Z, self.now).on(&self.exec_id), self.exec.clone(), vec![], "get_jobs", b"{}");
        out.json()
    }
}

// ---------------------------------------------------------------------------
// The executor's state (its fields are private upstream)
// ---------------------------------------------------------------------------

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
#[borsh(crate = "near_sdk::borsh")]
pub struct JobM {
    pub id: u32,
    pub recipe: u32,
    pub units: u32,
    pub status: String,
}

/// Borsh mirror of v1.1.0's `Executor { lock, operator, jobs, mode, newest }`
/// (crates/executor/src/lib.rs:25-36).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
#[borsh(crate = "near_sdk::borsh")]
pub struct ExecM {
    pub lock: AccountId,
    pub operator: AccountId,
    pub jobs: Vec<JobM>,
    pub mode: String,
    pub newest: u32,
}

pub fn job(id: u32, recipe: u32, units: u32, status: &str) -> JobM {
    JobM {
        id,
        recipe,
        units,
        status: status.into(),
    }
}

/// `new({lock: pool, operator})`.
pub fn fresh_exec() -> ExecM {
    ExecM {
        lock: acct(POOL),
        operator: acct(OPERATOR),
        jobs: vec![],
        mode: "normal".into(),
        newest: 0,
    }
}

pub fn decode_exec(s: &Storage) -> ExecM {
    assert_eq!(s.len(), 1, "executor storage is one STATE key");
    #[cfg(not(nt_v100_executor))]
    let m: ExecM = borsh::from_slice(&s[STATE_KEY]).expect("Borsh mirror of Executor");
    #[cfg(nt_v100_executor)]
    let m = {
        let v: ExecV100 = borsh::from_slice(&s[STATE_KEY]).expect("Borsh mirror of the v1.0.0 Executor");
        let newest = v.jobs.iter().map(|j| j.id).max().unwrap_or(0);
        ExecM { lock: v.lock, operator: v.operator, jobs: v.jobs, mode: v.mode, newest }
    };
    assert_eq!(encode_exec(&m)[STATE_KEY], s[STATE_KEY], "mirror round trip");
    m
}

pub fn encode_exec(m: &ExecM) -> Storage {
    let mut s = Storage::new();
    #[cfg(not(nt_v100_executor))]
    s.insert(STATE_KEY.to_vec(), borsh::to_vec(m).unwrap());
    #[cfg(nt_v100_executor)]
    s.insert(
        STATE_KEY.to_vec(),
        borsh::to_vec(&ExecV100 { lock: m.lock.clone(), operator: m.operator.clone(), jobs: m.jobs.clone(), mode: m.mode.clone() }).unwrap(),
    );
    s
}

/// Historical mutant s00b only (`--cfg nt_v100_executor`): v1.0.0's
/// `Executor { lock, operator, jobs, mode }` has no `newest`. The mirror
/// reads it as the largest job id (v1.0.0 forgets no job) and drops it on
/// encoding, so the tests compare the four stored fields.
#[cfg(nt_v100_executor)]
#[derive(BorshSerialize, BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
struct ExecV100 {
    lock: AccountId,
    operator: AccountId,
    jobs: Vec<JobM>,
    mode: String,
}

/// Runs `f` on a thread with a 256 MiB stack (wasmi in a debug test build
/// needs more than the default 2 MiB). The mocked blockchain is thread-local,
/// so the whole test body runs there.
pub fn big_stack<F: FnOnce() + Send + 'static>(f: F) {
    let h = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .expect("spawn");
    if let Err(p) = h.join() {
        std::panic::resume_unwind(p)
    }
}

// ---------------------------------------------------------------------------
// A contract and its core twin, on either path
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Host,
    Wasm(Build),
}

/// The host path and the documented build (NT-F1 fixed: build.sh builds with
/// `--cfg near`; the plain build is only SURF-3(b)'s negative control).
pub fn paths() -> [Path; 2] {
    [Path::Host, Path::Wasm(Build::Near)]
}

/// What one checked step returned (already compared with the core).
#[derive(Debug)]
pub struct Ran {
    /// The return value, or the failure the core predicts (which the path
    /// was checked to report, as far as its build reports messages).
    pub result: Result<Value, Fail>,
    pub events: Vec<Value>,
    pub calls: Vec<Call>,
    /// Wasm path: imports called (empty on the host path).
    pub host_calls: Vec<&'static str>,
    pub promise_return: Option<u64>,
}

impl Ran {
    pub fn value(&self) -> &Value {
        self.result.as_ref().unwrap_or_else(|e| panic!("call failed: {e:?}"))
    }
    pub fn event(&self) -> &Value {
        assert_eq!(self.events.len(), 1, "expected one event: {:?}", self.events);
        &self.events[0]["data"][0]
    }
}

/// The contract (on `path`) and the core side by side; every call goes
/// through `step` / `wasm_diff_step`.
pub struct Sim {
    pub path: Path,
    pub storage: Storage,
    pub pool: Pool,
    pub n: usize,
    /// The state is reachable (no `edit`), so every step checks LIVE-ALL.
    pub live_all: bool,
}

impl Sim {
    pub fn new(path: Path, cfg: Config, now: u64) -> Sim {
        let storage = match path {
            Path::Host => deploy(cfg.clone(), now),
            Path::Wasm(b) => wasm_deploy(b, &cfg, now),
        };
        let pool = Pool::new(cfg, now).expect("valid config");
        assert_eq!(storage, storage_of(&pool), "new() stored state");
        Sim {
            path,
            storage,
            pool,
            n: 0,
            live_all: true,
        }
    }

    pub fn run(&mut self, ctx: &Ctx, op: Op) -> Ran {
        self.n += 1;
        let what = format!("{:?} call {}: {op:?} by {} (signer {})", self.path, self.n, ctx.predecessor, ctx.signer);
        let exp = core_step(&mut self.pool.clone(), ctx, &op);
        match self.path {
            Path::Host => {
                let out = step_opt(self.storage.clone(), &mut self.pool, ctx, &op, &what, self.live_all);
                self.storage = out.storage;
                Ran {
                    // As on the wasm path: the core's failure, which the
                    // host was checked to report (see `on_host`).
                    result: match out.result {
                        Ok(v) => Ok(v),
                        Err(_) => Err(exp.result.clone().expect_err("checked")),
                    },
                    events: out.logs.iter().map(|l| parse_event(l)).collect(),
                    calls: calls_of(&out.receipts),
                    host_calls: vec![],
                    promise_return: None,
                }
            }
            Path::Wasm(b) => {
                let out = wasm_diff_step_opt(b, self.storage.clone(), &mut self.pool, ctx, &op, &what, self.live_all);
                self.storage = out.storage.clone();
                Ran {
                    result: match &out.result {
                        Ok(_) => Ok(exp.result.clone().expect("checked")),
                        Err(_) => Err(exp.result.clone().expect_err("checked")),
                    },
                    events: out.logs.iter().map(|l| parse_event(l)).collect(),
                    calls: calls_of(&out.receipts),
                    host_calls: out.host_calls,
                    promise_return: out.promise_return,
                }
            }
        }
    }

    pub fn ok(&mut self, ctx: &Ctx, op: Op) -> Ran {
        let r = self.run(ctx, op.clone());
        assert!(r.result.is_ok(), "{op:?} failed: {:?}", r.result);
        r
    }

    /// `op` fails with exactly `e`, writes nothing and logs nothing.
    pub fn err(&mut self, ctx: &Ctx, op: Op, e: Fail) {
        let before = self.storage.clone();
        let r = self.run(ctx, op.clone());
        assert_eq!(r.result.as_ref().err(), Some(&e), "{op:?}");
        assert!(r.events.is_empty());
        assert!(r.calls.is_empty());
        assert_eq!(self.storage, before, "a failed call changed storage");
    }

    /// Rewrites the stored state (a crafted, well-formed state: the core
    /// decides on it exactly as on a reachable one).
    /// LIVE-ALL is not checked from then on: a crafted state may be one no
    /// call sequence reaches.
    pub fn edit(&mut self, f: impl FnOnce(&mut Pool)) {
        f(&mut self.pool);
        self.storage = storage_of(&self.pool);
        self.live_all = false;
    }

    /// The stored status of record `id` (by id: `first_id + k`).
    pub fn status(&self, id: u32) -> Status {
        pool_of(&self.storage).record(id).expect("retained record").status.clone()
    }
}

pub fn submit(recipe: u32, units: u32, version: u32, expires_at: u64) -> Op {
    Op::Submit {
        req: Request {
            recipe,
            units,
            version,
            expires_at,
        },
    }
}
