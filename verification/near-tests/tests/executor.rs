//! The synthetic executor (crates/executor/src/lib.rs, v1.1.0), alone and
//! wired to the pool (EX-1..EX-8).
//!
//! Alone: every call runs on the host path (`call_t::<Executor>`) and on the
//! wasm of the documented build, from a stored state that is checked byte
//! for byte against a Borsh mirror of `Executor` (fields are private
//! upstream; v1.1.0 appends `newest`). The window tests (EX-6, EX-8) run
//! through `X`, which compares every call with a reference model written
//! from docs/api.md:42 ("Executor protocol"): result or panic, receipts, and
//! the stored bytes.
//! Wired: `Net` runs the pool and executor wasm with a receipt router, so
//! `dispatch -> execute -> on_result`, `reconcile -> status -> on_result` and
//! `finish/confirm_not_executed -> settle -> on_settled` run as the runtime
//! would chain them. `ex7_*` is the mocked-runtime port of upstream's
//! sandbox D-1 test, checking EX-7 after every transaction.
//!
//! The end-to-end flows also check the pool's side of the chain (SH-3,
//! SH-6, SH-7). `ex5_settle_before_callback_is_absorbed` is pool-only: the
//! late-callback half of RT-RACE in the mocked runtime.
//!
// PROP: EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8 SH-3 SH-6 SH-7 RT-RACE

mod common;
use common::*;
use near_sdk::env;
use near_sdk::serde_json::{self, json, Value};
use pool_core::{Config, Metric, Pool, Recipe, Request, Shape, Status, Term};
use pool_executor::Executor;
use std::collections::BTreeSet;

/// The executor's account in these tests (recipe 0 of `open_config` names it).
const EX: &str = X1;
/// docs/api.md:42: a job is forgotten 256 ids behind the newest; 512 jobs.
const WINDOW: u32 = 256;
const MAX_JOBS: usize = 512;

fn fresh() -> ExecM {
    fresh_exec()
}
fn decode(s: &Storage) -> ExecM {
    decode_exec(s)
}
fn encode(m: &ExecM) -> Storage {
    encode_exec(m)
}

fn ctx(who: &str) -> Ctx {
    Ctx::by(who, 100).on(EX)
}

/// One executor call on `path`.
struct XOut {
    /// JSON return (`null` for unit), or the failure message as the path
    /// reports it (`None`: the Plain build reports none).
    result: Result<Value, Option<String>>,
    storage: Storage,
    calls: Vec<Call>,
    promise_return: Option<u64>,
    host_calls: Vec<&'static str>,
}

fn ex(path: Path, c: &Ctx, st: &Storage, method: &str, args: Value) -> XOut {
    ex_r(path, c, st, method, args, &[])
}

/// `ex` with the promise results a callback (`on_settled`) receives.
fn ex_r(path: Path, c: &Ctx, st: &Storage, method: &str, args: Value, results: &[Res]) -> XOut {
    match path {
        Path::Host => {
            let write = !matches!(method, "status" | "get_jobs");
            let m = method.to_string();
            let a = args.clone();
            let u = move |k: &str| a[k].as_u64().unwrap_or_else(|| panic!("arg {k}")) as u32;
            let out = call_t::<Executor, Value>(c, st.clone(), prs(results), write, move |e| match m.as_str() {
                "execute" => json!(e.execute(u("id"), u("recipe"), u("units"))),
                "status" => json!(e.status(u("id"))),
                "finish" => {
                    e.finish(u("id"), args["completed"].as_bool().unwrap());
                    Value::Null
                }
                "confirm_not_executed" => {
                    e.confirm_not_executed(u("id"), u("recipe"), u("units"));
                    Value::Null
                }
                "set_mode" => {
                    e.set_mode(args["mode"].as_str().unwrap().into());
                    Value::Null
                }
                "on_settled" => {
                    // Historical mutant s00b (`--cfg nt_v100_executor`): the
                    // v1.0.0 executor has no on_settled.
                    #[cfg(nt_v100_executor)]
                    near_sdk::env::panic_str("the v1.0.0 executor has no on_settled");
                    #[cfg(not(nt_v100_executor))]
                    e.on_settled(u("id"));
                    #[allow(unreachable_code)]
                    Value::Null
                }
                "get_jobs" => serde_json::to_value(e.get_jobs()).unwrap(),
                other => panic!("no method {other}"),
            });
            assert!(out.logs.is_empty(), "the executor logs nothing");
            XOut {
                result: out.result.map_err(|f| Some(f.msg().to_string())),
                storage: out.storage,
                calls: calls_of(&out.receipts),
                promise_return: None,
                host_calls: vec![],
            }
        }
        Path::Wasm(b) => {
            let w = wasm(b, "pool_executor");
            let out = wasm_call(w, c, st.clone(), prs(results), method, &serde_json::to_vec(&args).unwrap());
            assert!(out.forbidden.is_empty(), "{:?}", out.forbidden);
            assert!(out.logs.is_empty(), "the executor logs nothing");
            XOut {
                result: match &out.result {
                    Ok(Some(v)) => Ok(serde_json::from_slice(v).unwrap()),
                    Ok(None) => Ok(Value::Null),
                    Err(f) => Err(f.panic_msg.clone()),
                },
                storage: out.storage.clone(),
                calls: calls_of(&out.receipts),
                promise_return: out.promise_return,
                host_calls: out.host_calls,
            }
        }
    }
}

/// The call failed with a message containing `needle` (as far as the path
/// reports messages), changed nothing and sent nothing.
fn fails(path: Path, out: &XOut, before: &Storage, needle: &str, what: &str) {
    let m = out.result.as_ref().expect_err(what);
    assert_eq!(&out.storage, before, "{what}: failed call changed state");
    assert!(out.calls.is_empty());
    match path {
        Path::Wasm(Build::Plain) => assert_eq!(m, &None, "{what}"),
        _ => {
            let m = m.as_deref().unwrap_or_else(|| panic!("{what}: no message"));
            assert!(m.contains(needle), "{what}: {m:?} lacks {needle:?}");
        }
    }
}

fn ok(out: &XOut, what: &str) -> Value {
    out.result.clone().unwrap_or_else(|e| panic!("{what}: failed {e:?}"))
}

/// Deploys the executor on `path` (`new({lock: POOL, operator: OPERATOR})`
/// by the executor account) and checks the mirror.
fn deploy_ex(path: Path) -> Storage {
    let st = match path {
        Path::Host => {
            install(&ctx(EX), Storage::new(), vec![], &[]);
            let e = Executor::new(acct(POOL), acct(OPERATOR));
            env::state_write(&e);
            drop(e);
            take_storage()
        }
        Path::Wasm(b) => {
            let out = wasm_call(
                wasm(b, "pool_executor"),
                &ctx(EX),
                Storage::new(),
                vec![],
                "new",
                &serde_json::to_vec(&json!({ "lock": POOL, "operator": OPERATOR })).unwrap(),
            );
            assert!(out.result.is_ok(), "{:?}", out.result);
            out.storage
        }
    };
    assert_eq!(decode(&st), fresh());
    st
}

fn exec_args(id: u32, recipe: u32, units: u32) -> Value {
    json!({ "id": id, "recipe": recipe, "units": units })
}

/// What `notify` sends (docs/api.md:42 and the Gas table): the lock's
/// `settle` with 40 Tgas, then this executor's `on_settled` with 10 Tgas,
/// waiting on it.
fn notify_calls(lock: &str, id: u32, completed: bool) -> Vec<Call> {
    vec![
        fcall(lock, "settle", json!({ "id": id, "completed": completed }), 40, vec![]),
        fcall(EX, "on_settled", json!({ "id": id }), 10, vec![0]),
    ]
}

fn range(a: u32, b: u32) -> Vec<u32> {
    (a..=b).collect()
}

// ---------------------------------------------------------------------------
// Reference model of the executor (docs/api.md:42)
// ---------------------------------------------------------------------------

/// Raises `newest` to `id` and forgets every job 256 or more behind it.
fn advance(m: &mut ExecM, id: u32) {
    m.newest = m.newest.max(id);
    let n = u64::from(m.newest);
    m.jobs.retain(|j| u64::from(j.id) + u64::from(WINDOW) > n);
}

/// The expected effect of `method` by `pred` on `m`: the JSON result and the
/// receipts, or the panic message. On `Err` the caller discards `m`.
fn model(m: &mut ExecM, pred: &str, method: &str, a: &Value, results: &[Res]) -> Result<(Value, Vec<Call>), &'static str> {
    let u = |k: &str| a[k].as_u64().unwrap() as u32;
    let operator = |m: &ExecM| if pred == m.operator.as_str() { Ok(()) } else { Err("only independent operator") };
    let stale = |m: &ExecM, id: u32| u64::from(id) + u64::from(WINDOW) <= u64::from(m.newest);
    match method {
        "execute" => {
            if pred != m.lock.as_str() {
                return Err("only lock may execute");
            }
            let (id, recipe, units) = (u("id"), u("recipe"), u("units"));
            // An id that has a job answers from it and moves nothing.
            if let Some(j) = m.jobs.iter().find(|j| j.id == id) {
                if (j.recipe, j.units) != (recipe, units) {
                    return Err("payload mismatch");
                }
                return Ok((json!(j.status), vec![]));
            }
            // A forgotten id returns "cancelled" and starts nothing.
            if stale(m, id) {
                return Ok((json!("cancelled"), vec![]));
            }
            advance(m, id);
            // At the cap, an execute that frees no job fails "full".
            if m.jobs.len() >= MAX_JOBS {
                return Err("full");
            }
            if m.mode == "panic" {
                return Err("injected failure");
            }
            let status = if m.mode == "reject" { "cancelled" } else { "active" };
            m.jobs.push(job(id, recipe, units, status));
            Ok((json!(if m.mode == "unknown" { "unknown" } else { status }), vec![]))
        }
        "status" => {
            let id = u("id");
            Ok((json!(m.jobs.iter().find(|j| j.id == id).map_or("unknown", |j| j.status.as_str())), vec![]))
        }
        "finish" => {
            operator(m)?;
            let (id, completed) = (u("id"), a["completed"].as_bool().unwrap());
            let lock = m.lock.to_string();
            let j = m.jobs.iter_mut().find(|j| j.id == id).ok_or("unknown job")?;
            let next = if completed { "completed" } else { "cancelled" };
            if j.status != "active" && j.status != next {
                return Err("terminal outcome conflict");
            }
            j.status = next.into();
            Ok((Value::Null, notify_calls(&lock, id, completed)))
        }
        "confirm_not_executed" => {
            operator(m)?;
            let (id, recipe, units) = (u("id"), u("recipe"), u("units"));
            if m.jobs.iter().any(|j| j.id == id) {
                return Err("job exists");
            }
            if stale(m, id) {
                return Err("stale id");
            }
            if m.jobs.len() >= MAX_JOBS {
                return Err("full");
            }
            m.jobs.push(job(id, recipe, units, "cancelled"));
            Ok((Value::Null, notify_calls(m.lock.as_str(), id, false)))
        }
        "set_mode" => {
            operator(m)?;
            let mode = a["mode"].as_str().unwrap();
            if !["normal", "reject", "unknown", "panic"].contains(&mode) {
                return Err("invalid mode");
            }
            m.mode = mode.into();
            Ok((Value::Null, vec![]))
        }
        "on_settled" => {
            // Exactly one promise result; any success counts as accepted.
            if results.len() != 1 {
                return Err("not a settle callback");
            }
            if results[0] != Res::Failed {
                advance(m, u("id"));
            }
            Ok((Value::Null, vec![]))
        }
        other => panic!("no model for {other}"),
    }
}

/// The executor on one path beside the model; every call is compared.
struct X {
    path: Path,
    st: Storage,
    m: ExecM,
    n: usize,
}

impl X {
    fn new(path: Path) -> X {
        X { path, st: deploy_ex(path), m: fresh(), n: 0 }
    }
    /// Starts from a crafted state (any state the model allows).
    fn with(path: Path, m: ExecM) -> X {
        X { path, st: encode(&m), m, n: 0 }
    }
    /// `method` by `who`, checked against the model: the same result or
    /// panic message, the same receipts, the same stored bytes.
    fn call(&mut self, who: &str, method: &str, args: Value, results: &[Res]) -> Result<Value, String> {
        self.n += 1;
        let what = format!("{:?} call {}: {method} {args} by {who} with {results:?}", self.path, self.n);
        let mut next = self.m.clone();
        // `#[private]` exists only in the wasm entry point.
        let exp = if method == "on_settled" && who != EX && matches!(self.path, Path::Wasm(_)) {
            Err("Method on_settled is private")
        } else {
            model(&mut next, who, method, &args, results)
        };
        let out = ex_r(self.path, &Ctx::by(who, 100).on(EX), &self.st, method, args, results);
        match exp {
            Ok((v, calls)) => {
                assert_eq!(out.result, Ok(v.clone()), "{what}");
                assert_eq!(out.calls, calls, "{what}: receipts");
                if let (Path::Wasm(_), false) = (self.path, calls.is_empty()) {
                    // The call's result is the lock's settle (index 0).
                    assert_eq!(out.promise_return, Some(0), "{what}");
                }
                self.m = next;
                self.st = out.storage;
                assert_eq!(self.st, encode(&self.m), "{what}: stored state != model");
                Ok(v)
            }
            Err(msg) => {
                fails(self.path, &out, &self.st, msg, &what);
                Err(msg.into())
            }
        }
    }
    fn exec(&mut self, id: u32) -> Result<Value, String> {
        self.call(POOL, "execute", exec_args(id, 0, 1), &[])
    }
    fn confirm(&mut self, id: u32) -> Result<Value, String> {
        self.call(OPERATOR, "confirm_not_executed", exec_args(id, 0, 1), &[])
    }
    fn finish(&mut self, id: u32, completed: bool) -> Result<Value, String> {
        self.call(OPERATOR, "finish", json!({ "id": id, "completed": completed }), &[])
    }
    /// `on_settled(id)` after the lock accepted (`ok`) or refused the settle.
    fn settled(&mut self, id: u32, ok: bool) {
        let r = if ok { Res::raw(b"") } else { Res::failed() };
        self.call(EX, "on_settled", json!({ "id": id }), &[r]).expect("on_settled");
    }
    fn ids(&self) -> Vec<u32> {
        self.m.jobs.iter().map(|j| j.id).collect()
    }
    fn newest(&self) -> u32 {
        self.m.newest
    }
}

// ---------------------------------------------------------------------------
// EX-1: only the lock executes
// ---------------------------------------------------------------------------

// PROP: EX-1
#[test]
fn ex1_only_the_lock_executes() {
    big_stack(|| {
        for path in paths() {
            let st = deploy_ex(path);
            for (pred, signer) in [(Z, Z), (OPERATOR, OPERATOR), (A1, A1), (EX, EX), (OWNER, POOL), (Z, POOL)] {
                let out = ex(path, &Ctx::new(pred, signer, 100_000_000_000).on(EX), &st, "execute", exec_args(0, 0, 1));
                fails(path, &out, &st, "only lock may execute", &format!("{path:?} execute by {pred}"));
            }
            // The lock's own receipt does, whoever signed the transaction.
            let out = ex(path, &Ctx::new(POOL, Z, 100_000_000_000).on(EX), &st, "execute", exec_args(0, 0, 1));
            assert_eq!(ok(&out, "execute by lock"), json!("active"));
            assert_eq!(decode(&out.storage).jobs, vec![job(0, 0, 1, "active")]);
            assert!(out.calls.is_empty(), "execute sends nothing");
            // status is a view anyone may call.
            for who in [Z, POOL, OPERATOR] {
                let s = ex(path, &ctx(who), &out.storage, "status", json!({ "id": 0 }));
                assert_eq!(ok(&s, "status"), json!("active"));
                assert_eq!(s.storage, out.storage);
                assert!(!s.host_calls.contains(&"storage_write"), "status (&self) wrote");
                let s = ex(path, &ctx(who), &out.storage, "status", json!({ "id": 1 }));
                assert_eq!(ok(&s, "status"), json!("unknown"));
            }
        }
    });
}

/// The executor's `new` is private and runs once; its mutating methods
/// reject deposits (near-sdk wrappers, wasm only).
// PROP: EX-1 EX-8
#[test]
fn ex1_executor_wrappers() {
    big_stack(|| {
        for b in Build::all() {
            let w = wasm(b, "pool_executor");
            let input = serde_json::to_vec(&json!({ "lock": POOL, "operator": OPERATOR })).unwrap();
            let empty = Storage::new();
            for who in [POOL, OPERATOR, Z] {
                let out = wasm_call(w, &Ctx::new(who, EX, 100_000_000_000).on(EX), empty.clone(), vec![], "new", &input);
                assert_wasm_fail(b, &out, &Fail::Guest("Method new is private".into()), &empty, "executor new");
            }
            let st = deploy_ex(Path::Wasm(b));
            let hijack = serde_json::to_vec(&json!({ "lock": Z, "operator": Z })).unwrap();
            let out = wasm_call(w, &ctx(EX), st.clone(), vec![], "new", &hijack);
            assert_wasm_fail(b, &out, &Fail::Guest("The contract has already been initialized".into()), &st, "re-init");
            let mut active = fresh();
            active.jobs.push(job(0, 0, 1, "active"));
            let st = encode(&active);
            for (method, who, args, results) in [
                ("execute", POOL, exec_args(1, 0, 1), vec![]),
                ("finish", OPERATOR, json!({ "id": 0, "completed": true }), vec![]),
                ("confirm_not_executed", OPERATOR, exec_args(1, 0, 1), vec![]),
                ("set_mode", OPERATOR, json!({ "mode": "reject" }), vec![]),
                ("on_settled", EX, json!({ "id": 1 }), vec![Res::raw(b"")]),
            ] {
                let out = wasm_call(w, &ctx(who).with_deposit(1), st.clone(), prs(&results), method, &serde_json::to_vec(&args).unwrap());
                assert_wasm_fail(b, &out, &Fail::Guest(format!("Method {method} doesn't accept deposit")), &st, method);
                let out = wasm_call(w, &ctx(who), st.clone(), prs(&results), method, &serde_json::to_vec(&args).unwrap());
                assert!(out.result.is_ok(), "{method}: {:?}", out.result);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// EX-2: a job is bound to its payload; execute is idempotent per id
// ---------------------------------------------------------------------------

// PROP: EX-2 EX-6
#[test]
fn ex2_payload_binding_and_dedupe() {
    big_stack(|| {
        for path in paths() {
            let pool = Ctx::by(POOL, 100).on(EX);
            let mut st = deploy_ex(path);
            let out = ex(path, &pool, &st, "execute", exec_args(4, 1, 3));
            assert_eq!(ok(&out, "first"), json!("active"));
            st = out.storage;
            assert_eq!(decode(&st).newest, 4);
            // Same payload: the recorded status, no second job.
            let again = ex(path, &pool, &st, "execute", exec_args(4, 1, 3));
            assert_eq!(ok(&again, "replay"), json!("active"));
            assert_eq!(again.storage, st);
            // Another payload under the same id: refused.
            for (r, u) in [(1, 4), (0, 3), (2, 2)] {
                let out = ex(path, &pool, &st, "execute", exec_args(4, r, u));
                fails(path, &out, &st, "payload mismatch", &format!("{path:?} ({r},{u})"));
            }
            // A replay returns the job's current status, in any mode.
            let fin = ex(path, &ctx(OPERATOR), &st, "finish", json!({ "id": 4, "completed": true }));
            ok(&fin, "finish");
            st = fin.storage;
            for mode in ["normal", "reject", "unknown", "panic"] {
                let m = ex(path, &ctx(OPERATOR), &st, "set_mode", json!({ "mode": mode }));
                let s = m.storage;
                let out = ex(path, &pool, &s, "execute", exec_args(4, 1, 3));
                assert_eq!(ok(&out, mode), json!("completed"), "{path:?} {mode}");
                assert_eq!(decode(&out.storage).jobs, vec![job(4, 1, 3, "completed")]);
            }
            // Modes for a new id.
            let with_mode = |st: &Storage, mode: &str| {
                let mut m = decode(st);
                m.mode = mode.into();
                encode(&m)
            };
            let s = with_mode(&st, "reject");
            let out = ex(path, &pool, &s, "execute", exec_args(5, 0, 1));
            assert_eq!(ok(&out, "reject"), json!("cancelled"));
            assert_eq!(decode(&out.storage).jobs[1], job(5, 0, 1, "cancelled"));
            let s = with_mode(&st, "unknown");
            let out = ex(path, &pool, &s, "execute", exec_args(5, 0, 1));
            assert_eq!(ok(&out, "unknown"), json!("unknown"));
            assert_eq!(decode(&out.storage).jobs[1], job(5, 0, 1, "active"));
            let s = with_mode(&st, "panic");
            let out = ex(path, &pool, &s, "execute", exec_args(5, 0, 1));
            fails(path, &out, &s, "injected failure", "panic mode");
            // At 512 jobs (254 at or below newest 255, 258 refused
            // confirmations above it) a new id that frees no job is refused;
            // known ids still answer.
            let mut full = fresh();
            full.newest = 255;
            full.jobs = (1..=255).filter(|i| *i != 100).map(|i| job(i, 0, 1, "active")).collect();
            full.jobs.extend((300..558).map(|i| job(i, 0, 1, "cancelled")));
            assert_eq!(full.jobs.len(), MAX_JOBS);
            let s = encode(&full);
            let out = ex(path, &pool, &s, "execute", exec_args(100, 0, 1));
            fails(path, &out, &s, "full", "full");
            let out = ex(path, &pool, &s, "execute", exec_args(510, 0, 1));
            assert_eq!(ok(&out, "known at full"), json!("cancelled"));
            assert_eq!(out.storage, s);
            let out = ex(path, &ctx(OPERATOR), &s, "confirm_not_executed", exec_args(100, 0, 1));
            fails(path, &out, &s, "full", "confirm at full");
        }
    });
}

// ---------------------------------------------------------------------------
// EX-3: only the operator finishes, confirms non-execution, or sets the mode
// ---------------------------------------------------------------------------

// PROP: EX-3
#[test]
fn ex3_only_the_operator_attests() {
    big_stack(|| {
        for path in paths() {
            let mut m = fresh();
            m.jobs.push(job(0, 0, 1, "active"));
            let st = encode(&m);
            for (pred, signer) in [(POOL, POOL), (Z, Z), (EX, EX), (A1, OPERATOR), (OWNER, OWNER)] {
                let c = Ctx::new(pred, signer, 100_000_000_000).on(EX);
                for (method, args) in [
                    ("finish", json!({ "id": 0, "completed": true })),
                    ("finish", json!({ "id": 0, "completed": false })),
                    ("confirm_not_executed", exec_args(1, 0, 1)),
                    ("set_mode", json!({ "mode": "reject" })),
                ] {
                    let out = ex(path, &c, &st, method, args);
                    fails(path, &out, &st, "only independent operator", &format!("{path:?} {method} by {pred}"));
                }
            }
            for mode in ["normal", "reject", "unknown", "panic"] {
                let out = ex(path, &Ctx::new(OPERATOR, Z, 100_000_000_000).on(EX), &st, "set_mode", json!({ "mode": mode }));
                ok(&out, mode);
                assert_eq!(decode(&out.storage).mode, mode);
                assert_eq!(decode(&out.storage).jobs, m.jobs);
            }
            for bad in ["", "Normal", "normal ", "cancelled"] {
                let out = ex(path, &ctx(OPERATOR), &st, "set_mode", json!({ "mode": bad }));
                fails(path, &out, &st, "invalid mode", bad);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// EX-4: a terminal outcome is never replaced by the other one
// ---------------------------------------------------------------------------

// PROP: EX-4
#[test]
fn ex4_terminal_conflict_rejected() {
    big_stack(|| {
        for path in paths() {
            let mut m = fresh();
            m.jobs = vec![job(0, 0, 1, "active"), job(1, 0, 2, "active"), job(2, 1, 1, "cancelled")];
            let st = encode(&m);
            let op = ctx(OPERATOR);
            let done = ex(path, &op, &st, "finish", json!({ "id": 0, "completed": true }));
            ok(&done, "complete 0");
            assert_eq!(decode(&done.storage).jobs[0].status, "completed");
            let gone = ex(path, &op, &st, "finish", json!({ "id": 1, "completed": false }));
            ok(&gone, "cancel 1");
            assert_eq!(decode(&gone.storage).jobs[1].status, "cancelled");
            // The same outcome again is accepted (and notifies again).
            let again = ex(path, &op, &done.storage, "finish", json!({ "id": 0, "completed": true }));
            ok(&again, "complete 0 again");
            assert_eq!(again.storage, done.storage);
            assert_eq!(again.calls, notify_calls(POOL, 0, true));
            // The other outcome is refused.
            let out = ex(path, &op, &done.storage, "finish", json!({ "id": 0, "completed": false }));
            fails(path, &out, &done.storage, "terminal outcome conflict", "completed -> cancelled");
            let out = ex(path, &op, &gone.storage, "finish", json!({ "id": 1, "completed": true }));
            fails(path, &out, &gone.storage, "terminal outcome conflict", "cancelled -> completed");
            let out = ex(path, &op, &st, "finish", json!({ "id": 2, "completed": true }));
            fails(path, &out, &st, "terminal outcome conflict", "confirmed-not-executed -> completed");
            // Unknown jobs, and confirming non-execution of a job that exists.
            let out = ex(path, &op, &st, "finish", json!({ "id": 9, "completed": true }));
            fails(path, &out, &st, "unknown job", "finish unknown");
            for id in [0, 1, 2] {
                let out = ex(path, &op, &st, "confirm_not_executed", exec_args(id, 0, 1));
                fails(path, &out, &st, "job exists", "confirm existing");
            }
            let out = ex(path, &op, &st, "confirm_not_executed", exec_args(3, 1, 5));
            ok(&out, "confirm new");
            assert_eq!(decode(&out.storage).jobs[3], job(3, 1, 5, "cancelled"));
            // ...after which the lock's late execute replays "cancelled".
            let late = ex(path, &Ctx::by(POOL, 100).on(EX), &out.storage, "execute", exec_args(3, 1, 5));
            assert_eq!(ok(&late, "late execute"), json!("cancelled"));
        }
    });
}

// ---------------------------------------------------------------------------
// EX-5: notify calls lock.settle({id, completed}), then on_settled({id})
// ---------------------------------------------------------------------------

// PROP: EX-5 EX-8
#[test]
fn ex5_notify_calls_lock_settle() {
    big_stack(|| {
        for path in paths() {
            let mut m = fresh();
            m.jobs = vec![job(0, 0, 1, "active"), job(7, 1, 2, "active")];
            let st = encode(&m);
            let op = ctx(OPERATOR);
            for (id, completed) in [(0u32, true), (7, false)] {
                let out = ex(path, &op, &st, "finish", json!({ "id": id, "completed": completed }));
                ok(&out, "finish");
                assert_eq!(out.calls, notify_calls(POOL, id, completed), "{path:?}");
                if let Path::Wasm(_) = path {
                    // The call's result stays the lock's: settle (index 0) is
                    // returned, on_settled waits on it (NT-F3: low-level
                    // promise functions only).
                    assert_eq!(out.promise_return, Some(0), "finish returns the settle promise");
                    assert!(out.host_calls.contains(&"promise_create") && out.host_calls.contains(&"promise_then"));
                    assert!(!out.host_calls.iter().any(|h| h.starts_with("promise_batch")), "{:?}", out.host_calls);
                }
            }
            let out = ex(path, &op, &st, "confirm_not_executed", exec_args(3, 1, 1));
            ok(&out, "confirm");
            assert_eq!(out.calls, notify_calls(POOL, 3, false));
            // The lock is the account given at init, not the caller.
            let mut other = m.clone();
            other.lock = acct(Z);
            let out = ex(path, &op, &encode(&other), "finish", json!({ "id": 0, "completed": true }));
            assert_eq!(out.calls, notify_calls(Z, 0, true));
        }
    });
}

// ---------------------------------------------------------------------------
// EX-6: the executor's window (docs/api.md:42)
// ---------------------------------------------------------------------------

/// The window constants match the lock's: `WINDOW == MAX_RECORDS` (a job is
/// forgotten exactly when the lock has pruned its record) and `MAX_JOBS >=
/// 2 * WINDOW` (the cap is reached only with WINDOW jobs above `newest`).
/// The constants are private, so this reads the pinned source text.
// PROP: EX-6 EX-7
#[test]
fn ex6_window_constants_match_the_lock() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../source-mirror/crates/executor/src/lib.rs")).unwrap();
    let konst = |name: &str| -> u64 {
        let line = src
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("const {name}:")))
            .unwrap_or_else(|| panic!("no const {name}"));
        let v = line.split('=').nth(1).unwrap().trim().trim_end_matches(';').replace('_', "");
        v.parse().unwrap_or_else(|_| panic!("{name} = {v}"))
    };
    let (window, max_jobs) = (konst("WINDOW"), konst("MAX_JOBS"));
    assert_eq!(window, pool_core::MAX_RECORDS as u64, "WINDOW != MAX_RECORDS");
    assert!(max_jobs >= 2 * window, "MAX_JOBS {max_jobs} < 2 * WINDOW");
    assert_eq!((window, max_jobs as usize), (u64::from(WINDOW), MAX_JOBS), "the model's constants");
    assert_eq!((konst("SETTLE_TGAS"), konst("ON_SETTLED_TGAS")), (40, 10), "notify's gas");
}

/// Upstream's F-3 executor tests on both paths (each call checked against
/// the model): execute of id N + 256 forgets job N; a forgotten id's execute
/// returns "cancelled" and stores nothing; its confirmation fails "stale
/// id"; a refused far confirmation moves nothing; one execute of a higher id
/// frees the cap.
// PROP: EX-6 EX-2
#[test]
fn ex6_window_forgets_and_caps() {
    big_stack(|| {
        for path in paths() {
            // f3_execute_forgets_jobs_256_ids_behind_the_newest
            let mut x = X::new(path);
            for id in 0..256 {
                assert_eq!(x.exec(id), Ok(json!("active")));
            }
            assert_eq!(x.ids(), range(0, 255));
            assert_eq!(x.exec(256), Ok(json!("active")));
            assert_eq!((x.newest(), x.ids()), (256, range(1, 256)));
            assert_eq!(x.call(Z, "status", json!({ "id": 0 }), &[]), Ok(json!("unknown")));
            assert_eq!(x.exec(0), Ok(json!("cancelled")));
            assert_eq!((x.newest(), x.ids()), (256, range(1, 256)));
            assert_eq!(x.exec(1), Ok(json!("active")), "a stored job answers");
            assert_eq!(x.exec(300), Ok(json!("active")));
            let mut expected = range(45, 256);
            expected.push(300);
            assert_eq!(x.ids(), expected);
            // Below the newest but within the window: starts once.
            assert_eq!(x.exec(299), Ok(json!("active")));
            assert_eq!(x.newest(), 300);
            assert_eq!(x.exec(44), Ok(json!("cancelled")));
            assert_eq!(x.exec(45), Ok(json!("active")), "45 + 256 > 300: kept");
            // The boundary on both sides: 44 + 256 == 300 is forgotten.
            assert_eq!(x.confirm(44), Err("stale id".into()));
            assert_eq!(x.confirm(45), Err("job exists".into()));
            assert_eq!(x.finish(44, false), Err("unknown job".into()));

            // f3_confirm_is_accepted_up_to_255_ids_behind
            let mut x = X::new(path);
            x.exec(256).unwrap();
            x.confirm(1_000_000).unwrap();
            x.settled(1_000_000, false);
            assert_eq!(x.newest(), 256);
            x.confirm(1).unwrap();
            assert_eq!(x.ids(), vec![256, 1_000_000, 1]);
            // f3_stale_confirm_not_executed_panics
            assert_eq!(x.confirm(0), Err("stale id".into()));

            // f3_an_execute_frees_the_cap
            let mut x = X::new(path);
            for id in 1..=512 {
                x.confirm(id).unwrap();
            }
            assert_eq!((x.ids().len(), x.newest()), (512, 0));
            assert_eq!(x.confirm(513), Err("full".into()));
            assert_eq!(x.exec(256), Ok(json!("cancelled")), "a stored job answers at the cap");
            assert_eq!(x.exec(0), Err("full".into()), "0 frees nothing: 1 + 256 > 0");
            assert_eq!(x.newest(), 0, "the failed execute moved nothing");
            assert_eq!(x.exec(600), Ok(json!("active")));
            let mut expected = range(345, 512);
            expected.push(600);
            assert_eq!(x.ids(), expected);
            x.confirm(601).unwrap();
            assert_eq!(x.ids().len(), 170);
        }
    });
}

// ---------------------------------------------------------------------------
// EX-8: on_settled
// ---------------------------------------------------------------------------

/// Upstream's D-1 executor tests on both paths (each call checked against the
/// model), plus the wasm-only `#[private]` check: `on_settled` is private,
/// needs exactly one promise result, and advances `newest` unless the settle
/// failed (a success with a value, `TooLong`, counts).
// PROP: EX-8 EX-6
#[test]
fn ex8_on_settled_protocol() {
    big_stack(|| {
        for path in paths() {
            // d1_an_accepted_settle_moves_newest_and_forgets
            let mut x = X::new(path);
            for id in 0..=255 {
                x.exec(id).unwrap();
            }
            x.confirm(300).unwrap();
            assert_eq!((x.newest(), x.ids().len()), (255, 257));
            x.settled(300, true);
            let mut expected = range(45, 255);
            expected.push(300);
            assert_eq!((x.newest(), x.ids()), (300, expected.clone()));
            let before = x.st.clone();
            x.settled(100, true);
            assert_eq!(x.st, before, "a lower accepted settle moves nothing");
            assert_eq!(x.exec(44), Ok(json!("cancelled")));
            assert_eq!((x.newest(), x.ids()), (300, expected.clone()));
            // d1_on_settled_needs_one_promise_result: none, or two.
            let none = x.call(EX, "on_settled", json!({ "id": 600 }), &[]);
            assert_eq!(none, Err("not a settle callback".into()));
            let two = x.call(EX, "on_settled", json!({ "id": 600 }), &[Res::raw(b""), Res::raw(b"")]);
            assert_eq!(two, Err("not a settle callback".into()));
            // #[private] (wasm only): another account, even the operator or
            // the lock, with a successful result in hand.
            if let Path::Wasm(_) = path {
                for who in [OPERATOR, POOL, Z] {
                    let r = x.call(who, "on_settled", json!({ "id": 600 }), &[Res::raw(b"")]);
                    assert_eq!(r, Err("Method on_settled is private".into()));
                }
            }
            assert_eq!((x.newest(), x.ids()), (300, expected));

            // d1_a_refused_settle_changes_nothing
            let mut x = X::new(path);
            for id in 0..=10 {
                x.exec(id).unwrap();
            }
            x.confirm(400).unwrap();
            assert_eq!(x.call(Z, "status", json!({ "id": 400 }), &[]), Ok(json!("cancelled")));
            for id in [400, 5, 1_000] {
                let before = x.st.clone();
                x.settled(id, false);
                assert_eq!(x.st, before, "refused settle of {id}");
            }
            assert_eq!((x.newest(), x.ids().len()), (10, 12));

            // d1_a_settle_success_with_a_value_counts_as_accepted
            let mut x = X::new(path);
            for id in 0..=255 {
                x.exec(id).unwrap();
            }
            x.confirm(300).unwrap();
            x.call(EX, "on_settled", json!({ "id": 300 }), &[Res::raw(&[1])]).unwrap();
            let mut expected = range(45, 255);
            expected.push(300);
            assert_eq!((x.newest(), x.ids()), (300, expected));

            // d1_finish_moves_newest_too
            let mut x = X::new(path);
            for id in 0..=255 {
                x.exec(id).unwrap();
            }
            x.confirm(300).unwrap();
            x.settled(300, false);
            assert_eq!((x.newest(), x.ids().len()), (255, 257));
            assert_eq!(x.exec(300), Ok(json!("cancelled")));
            assert_eq!((x.newest(), x.ids().len()), (255, 257));
            x.finish(300, false).unwrap();
            x.settled(300, true);
            let mut expected = range(45, 255);
            expected.push(300);
            assert_eq!((x.newest(), x.ids()), (300, expected));

            // d1_a_refused_job_is_forgotten_256_past_its_id
            let mut x = X::new(path);
            x.confirm(300).unwrap();
            x.settled(300, false);
            x.exec(400).unwrap();
            assert_eq!(x.ids(), vec![300, 400]);
            x.confirm(555).unwrap();
            x.settled(555, true);
            assert_eq!((x.newest(), x.ids()), (555, vec![300, 400, 555]));
            x.confirm(556).unwrap();
            x.settled(556, true);
            assert_eq!((x.newest(), x.ids()), (556, vec![400, 555, 556]));

            // d1_accepted_confirmations_keep_256_jobs (300 here)
            let mut x = X::new(path);
            for id in 0..300u32 {
                x.confirm(id).unwrap();
                x.settled(id, true);
                assert_eq!(x.ids().len(), (id as usize + 1).min(256), "after {id}");
            }
            assert_eq!((x.newest(), x.ids()), (299, range(44, 299)));

            // d1_256_refused_jobs_above_a_full_window_fill_the_cap
            let mut x = X::new(path);
            for id in 0..=255 {
                x.exec(id).unwrap();
            }
            for id in 300..556u32 {
                x.confirm(id).unwrap();
                x.settled(id, false);
            }
            assert_eq!((x.newest(), x.ids().len()), (255, 512));
            assert_eq!(x.confirm(556), Err("full".into()));
            // A stored id answers at the cap; an execute 256 past the oldest
            // jobs frees them.
            assert_eq!(x.exec(255), Ok(json!("active")));
            assert_eq!(x.exec(556), Ok(json!("active")));
            let mut expected = range(301, 555);
            expected.push(556);
            assert_eq!((x.newest(), x.ids()), (556, expected));

            // d1_with_fewer_jobs_below_newest_more_refused_jobs_fit
            let mut x = X::new(path);
            for id in 0..=99 {
                x.exec(id).unwrap();
            }
            for id in 300..712u32 {
                x.confirm(id).unwrap();
                x.settled(id, false);
            }
            assert_eq!((x.newest(), x.ids().len()), (99, 512));
            assert_eq!(x.confirm(712), Err("full".into()));
        }
    });
}

/// d1_callbacks_in_any_order: 300 confirmations in flight, then their
/// callbacks forward or backward; the stored state is the same.
// PROP: EX-8 EX-6
#[test]
fn ex8_callbacks_in_any_order() {
    big_stack(|| {
        for path in paths() {
            let run = |reverse: bool| {
                let mut x = X::new(path);
                for id in 0..300u32 {
                    x.confirm(id).unwrap();
                }
                assert_eq!((x.newest(), x.ids().len()), (0, 300));
                let mut order: Vec<u32> = (0..300).collect();
                if reverse {
                    order.reverse();
                }
                for id in order {
                    x.settled(id, true);
                }
                assert_eq!((x.newest(), x.ids()), (299, range(44, 299)));
                x.st
            };
            assert_eq!(run(true), run(false), "{path:?}");
        }
    });
}

/// EX-6, EX-8 as a differential: random executor calls on both paths,
/// each compared with the model (result or panic message, receipts, stored
/// bytes). Ids cluster around `newest` and the window edge; every third
/// sequence starts near the cap (256 jobs at or below `newest`, 250 refused
/// confirmations above it).
// PROP: EX-6 EX-8 EX-2 EX-3 EX-4
#[test]
fn ex6_random_differential_against_the_model() {
    big_stack(|| {
        let seqs = env_u64("EXEC_DIFF_SEQS", 24);
        let steps = env_u64("EXEC_DIFF_STEPS", 400);
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for path in paths() {
            let seqs = if path == Path::Host { seqs } else { seqs.div_ceil(4) };
            for seq in 0..seqs {
                let mut rng = Rng::new(0xe6 + seq * 7919);
                let cap_mode = seq % 3 == 2;
                let mut x = if cap_mode {
                    let mut m = fresh();
                    m.newest = 255;
                    m.jobs = (0..=255).map(|i| job(i, 0, 1, "active")).collect();
                    m.jobs.extend((300..550).map(|i| job(i, 0, 1, "cancelled")));
                    X::with(path, m)
                } else {
                    X::new(path)
                };
                for _ in 0..steps {
                    let n = x.newest() as i64;
                    let near = |rng: &mut Rng| -> u32 {
                        match rng.below(10) {
                            0..=3 => (n + rng.range(-260, 40)).max(0) as u32,
                            4..=5 => (n - 256 + rng.range(-2, 2)).max(0) as u32,
                            6..=7 => (n + rng.range(1, 300)) as u32,
                            _ => rng.range(0, 2_000) as u32,
                        }
                    };
                    let held = x.ids();
                    let id = if !held.is_empty() && rng.chance(30) { *rng.pick(&held) } else { near(&mut rng) };
                    let k = rng.below(100);
                    let r = if k < 30 && !cap_mode {
                        let who = if rng.chance(92) { POOL } else { *rng.pick(&[Z, OPERATOR, EX]) };
                        let (rc, u) = if rng.chance(90) { (0, 1) } else { (1, 2) };
                        let r = x.call(who, "execute", exec_args(id, rc, u), &[]);
                        if let Ok(Value::String(s)) = &r {
                            seen.insert(format!("execute:{s}"));
                        }
                        r
                    } else if k < 55 {
                        let who = if rng.chance(95) { OPERATOR } else { Z };
                        x.call(who, "confirm_not_executed", exec_args(id, 0, 1), &[])
                    } else if k < 65 {
                        let who = if rng.chance(95) { OPERATOR } else { POOL };
                        x.call(who, "finish", json!({ "id": id, "completed": rng.chance(50) }), &[])
                    } else if k < 92 {
                        let results = match rng.below(20) {
                            0..=9 if !cap_mode => vec![Res::raw(b"")],
                            0..=14 => vec![Res::failed()],
                            15 => vec![Res::raw(b"1")],
                            16 => vec![],
                            17 => vec![Res::raw(b""), Res::failed()],
                            _ => vec![Res::raw(b"")],
                        };
                        let who = if rng.chance(95) { EX } else { OPERATOR };
                        let before = x.newest();
                        let r = x.call(who, "on_settled", json!({ "id": id }), &results);
                        if x.newest() > before {
                            seen.insert("on_settled:advanced".into());
                        }
                        r
                    } else if k < 96 {
                        x.call(Z, "status", json!({ "id": id }), &[])
                    } else {
                        let mode = *rng.pick(&["normal", "normal", "reject", "unknown", "panic", "bogus"]);
                        x.call(OPERATOR, "set_mode", json!({ "mode": mode }), &[])
                    };
                    if let Err(e) = r {
                        seen.insert(format!("err:{e}"));
                    }
                    // The bounds docs/api.md:42 states.
                    assert!(x.ids().len() <= MAX_JOBS);
                    let at_or_below = x.m.jobs.iter().filter(|j| j.id <= x.newest()).count();
                    assert!(at_or_below <= WINDOW as usize, "{path:?}: {at_or_below} jobs at or below newest");
                    assert!(x.m.jobs.iter().all(|j| u64::from(j.id) + u64::from(WINDOW) > u64::from(x.newest())));
                }
            }
        }
        println!("ex6 differential reached: {seen:?}");
        if seqs >= 24 && steps >= 400 {
            for k in [
                "err:full",
                "err:stale id",
                "err:job exists",
                "err:payload mismatch",
                "err:not a settle callback",
                "err:unknown job",
                "err:terminal outcome conflict",
                "err:Method on_settled is private",
                "execute:cancelled",
                "on_settled:advanced",
            ] {
                assert!(seen.contains(k), "never reached {k}");
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Pool + executor end to end (wasm, receipt router)
// ---------------------------------------------------------------------------

fn trace_methods(net: &Net, from: usize) -> Vec<(String, bool)> {
    net.trace[from..].iter().map(|(_, m, r, _)| (m.clone(), r.is_ok())).collect()
}

fn ms(v: &[(&str, bool)]) -> Vec<(String, bool)> {
    v.iter().map(|(m, ok)| (m.to_string(), *ok)).collect()
}

fn reconciled(net: &Net, idx: usize) -> Value {
    let (recv, method, r, logs) = &net.trace[idx];
    assert_eq!((recv.as_str(), method.as_str()), (POOL, "on_result"));
    assert!(r.is_ok());
    assert_eq!(logs.len(), 1);
    parse_event(&logs[0])["data"][0].clone()
}

fn submit_args(net: &Net, recipe: u32, units: u32) -> Value {
    json!({ "request": { "recipe": recipe, "units": units, "version": net.pool_state().version, "expires_at": net.now + 50 } })
}

// PROP: EX-5 EX-8 SH-3 SH-6 SH-7
#[test]
fn ex5_end_to_end_normal_reject_unknown_panic() {
    big_stack(|| {
        for b in Build::all() {
            // normal: execute -> "active" -> Active; finish(true) -> settle ->
            // Completed -> on_settled (newest stays 0).
            let mut net = Net::new(b, &open_config(), EX, 100);
            let a = submit_args(&net, 0, 2);
            assert!(net.tx(A1, POOL, "submit", a).result.is_ok());
            let n = net.trace.len();
            let d = net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert!(d.result.is_ok());
            assert_eq!(trace_methods(&net, n), ms(&[("dispatch", true), ("execute", true), ("on_result", true)]));
            assert_eq!(net.result, Some(Ok(vec![])), "the transaction's result is on_result's");
            assert_eq!(reconciled(&net, n + 2), json!({ "id": 0, "outcome": "active", "error": null }));
            assert_eq!(net.pool_state().records[0].status, Status::Active);
            assert_eq!(net.jobs(), json!([{ "id": 0, "recipe": 0, "units": 2, "status": "active" }]));
            let n = net.trace.len();
            assert!(net.tx(OPERATOR, EX, "finish", json!({ "id": 0, "completed": true })).result.is_ok());
            assert_eq!(trace_methods(&net, n), ms(&[("finish", true), ("settle", true), ("on_settled", true)]));
            assert_eq!(net.result, Some(Ok(vec![])), "finish's result is the lock's settle");
            assert_eq!(net.pool_state().records[0].status, Status::Completed);
            let v = net.pool_state().version;
            // A second finish re-notifies; settle is idempotent.
            assert!(net.tx(OPERATOR, EX, "finish", json!({ "id": 0, "completed": true })).result.is_ok());
            assert_eq!(net.result, Some(Ok(vec![])));
            assert_eq!(net.pool_state().version, v);

            // reject: execute -> "cancelled" -> settle(false) inside on_result
            let mut net = Net::new(b, &open_config(), EX, 100);
            net.tx(OPERATOR, EX, "set_mode", json!({ "mode": "reject" }));
            let a = submit_args(&net, 0, 1);
            net.tx(A1, POOL, "submit", a);
            let n = net.trace.len();
            net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert_eq!(reconciled(&net, n + 2), json!({ "id": 0, "outcome": "cancelled", "error": null }));
            assert_eq!(net.pool_state().records[0].status, Status::Cancelled);
            assert_eq!(net.jobs()[0]["status"], json!("cancelled"));

            // unknown: execute -> "unknown" (job active) -> Unknown; reconcile
            // by anyone -> status "active" -> Active; later reconciles are
            // no-ops (the version moves at most twice per record).
            let mut net = Net::new(b, &open_config(), EX, 100);
            net.tx(OPERATOR, EX, "set_mode", json!({ "mode": "unknown" }));
            let a = submit_args(&net, 0, 1);
            net.tx(A1, POOL, "submit", a);
            let v0 = net.pool_state().version;
            let n = net.trace.len();
            net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert_eq!(reconciled(&net, n + 2)["outcome"], json!("unknown"));
            assert_eq!(net.pool_state().records[0].status, Status::Unknown);
            for i in 0..5 {
                let n = net.trace.len();
                assert!(net.tx(Z, POOL, "reconcile", json!({ "id": 0 })).result.is_ok());
                assert_eq!(trace_methods(&net, n), ms(&[("reconcile", true), ("status", true), ("on_result", true)]));
                assert_eq!(reconciled(&net, n + 2)["outcome"], json!("active"));
                assert_eq!(net.pool_state().records[0].status, Status::Active, "reconcile {i}");
            }
            assert_eq!(net.pool_state().version, v0 + 3, "dispatch + Unknown + Active");
            net.tx(OPERATOR, EX, "finish", json!({ "id": 0, "completed": false }));
            assert_eq!(net.pool_state().records[0].status, Status::Cancelled);

            // panic: execute fails -> on_result(failed) -> Unknown; status
            // says "unknown" (no job) -> stays Unknown; confirm_not_executed
            // -> settle(false) -> Cancelled -> on_settled.
            let mut net = Net::new(b, &open_config(), EX, 100);
            net.tx(OPERATOR, EX, "set_mode", json!({ "mode": "panic" }));
            let a = submit_args(&net, 0, 3);
            net.tx(A1, POOL, "submit", a);
            let n = net.trace.len();
            net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert_eq!(trace_methods(&net, n), ms(&[("dispatch", true), ("execute", false), ("on_result", true)]));
            assert_eq!(reconciled(&net, n + 2), json!({ "id": 0, "outcome": "unknown", "error": null }));
            let v = net.pool_state().version;
            net.tx(Z, POOL, "reconcile", json!({ "id": 0 }));
            assert_eq!(net.pool_state().records[0].status, Status::Unknown);
            assert_eq!(net.pool_state().version, v);
            assert_eq!(net.jobs(), json!([]));
            let n = net.trace.len();
            assert!(net.tx(OPERATOR, EX, "confirm_not_executed", exec_args(0, 0, 3)).result.is_ok());
            assert_eq!(trace_methods(&net, n), ms(&[("confirm_not_executed", true), ("settle", true), ("on_settled", true)]));
            assert_eq!(net.pool_state().records[0].status, Status::Cancelled);
            assert_eq!(net.jobs(), json!([{ "id": 0, "recipe": 0, "units": 3, "status": "cancelled" }]));
        }
    });
}

/// The operator's settle may reach the pool before the dispatch callback
/// (different receipts): the late callback is then absorbed.
// PROP: SH-3 RT-RACE
#[test]
fn ex5_settle_before_callback_is_absorbed() {
    big_stack(|| {
        for path in paths() {
            let mut s = Sim::new(path, open_config(), 100);
            s.ok(&Ctx::by(A2, 100), submit(1, 2, 0, 150));
            s.ok(&Ctx::by(A2, 101), Op::Dispatch { id: 0 });
            s.ok(&Ctx::by(X2, 101), Op::Settle { id: 0, completed: true });
            let v = s.pool.version;
            for late in [Some("active"), Some("unknown"), None, Some("completed")] {
                let r = s.ok(&Ctx::by(POOL, 102), Op::cb(0, late));
                assert_eq!(r.event()["error"], Value::Null);
            }
            let r = s.ok(&Ctx::by(POOL, 102), Op::cb(0, Some("cancelled")));
            assert_eq!(r.event()["error"], json!("InvalidState"));
            assert_eq!((s.status(0), s.pool.version), (Status::Completed, v));
        }
    });
}

// ---------------------------------------------------------------------------
// EX-7: the executor never forgets the job of a record the lock retains
// ---------------------------------------------------------------------------

/// Upstream's sandbox D-1 configuration: one record holds one unit of a
/// 10-unit capacity until it ends either way.
fn confirmations_config() -> Config {
    Config {
        name: "confirmations".into(),
        owner: OWNER.into(),
        metrics: vec![Metric { name: "In flight".into(), unit: "jobs".into(), initial: 0, min: 0, max: 10 }],
        terms: vec![Term { metric: 0, target: 10, weight: 1, shape: Shape::Excess }],
        recipes: vec![Recipe {
            name: "job".into(),
            actor: A1.into(),
            executor: EX.into(),
            reserve: vec![1],
            complete: vec![-1],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    }
}

/// EX-7 and the executor's bounds after a transaction: every job is within
/// the window, at most 256 sit at or below `newest`, `newest` is an id the
/// lock retains or retained (`newest < first_id + 256`), and every retained
/// record that ever had a job still has it.
fn check_ex7(net: &Net, ever: &BTreeSet<u32>, what: &str) {
    let e = net.exec_state();
    let p = net.pool_state();
    assert!(e.jobs.len() <= MAX_JOBS, "{what}");
    assert!(e.jobs.iter().all(|j| u64::from(j.id) + u64::from(WINDOW) > u64::from(e.newest)), "{what}: a forgotten job is held");
    assert!(e.jobs.iter().filter(|j| j.id <= e.newest).count() <= WINDOW as usize, "{what}");
    assert!(
        u64::from(e.newest) < u64::from(p.first_id) + u64::from(WINDOW),
        "{what}: newest {} past the lock's window at {}",
        e.newest,
        p.first_id
    );
    for c in &p.records {
        if ever.contains(&c.id) {
            assert!(e.jobs.iter().any(|j| j.id == c.id), "{what}: EX-7: job {} forgotten while the lock retains it", c.id);
        }
    }
}

/// Upstream's sandbox D-1 test (crates/sandbox/tests/integration.rs:419) in
/// the mocked runtime: 300 records whose `execute` fails, each confirmed not
/// executed. Each accepted settle's `on_settled` moves `newest`, so the
/// executor holds at most 256 jobs, and EX-7 holds after every transaction.
/// Then a refused confirmation keeps its job and the transaction reports the
/// lock's error; `on_settled` is private and needs the settle's result; a
/// Reserved record confirmed too early is cancelled when dispatched, after
/// which `finish(id, false)` moves `newest`; the pool is not wedged.
// PROP: EX-7 EX-6 EX-8
#[test]
fn ex7_d1_confirmed_failed_executes_stay_within_256() {
    big_stack(|| {
        for b in Build::all() {
            let cfg = confirmations_config();
            let mut net = Net::new(b, &cfg, EX, 100);
            let mut reference = Pool::new(cfg, 100).unwrap();
            assert_eq!(net.pool_state(), reference);
            let now = net.now;
            let mut ever = BTreeSet::new();
            assert!(net.tx(OPERATOR, EX, "set_mode", json!({ "mode": "panic" })).result.is_ok());
            let req = |p: &Pool| Request { recipe: 0, units: 1, version: p.version, expires_at: now + 3600 };
            let ids = |net: &Net| net.exec_state().jobs.iter().map(|j| j.id).collect::<Vec<u32>>();
            for id in 0..300u32 {
                let q = req(&reference);
                let out = net.tx(A1, POOL, "submit", json!({ "request": request_json(&q) }));
                assert_eq!(out.json()["id"], json!(id));
                reference.submit(A1, q, now).unwrap();
                // execute fails (panic mode), so on_result marks it Unknown.
                net.tx(A1, POOL, "dispatch", json!({ "id": id }));
                reference.dispatch(A1, id, now).unwrap();
                reference.acknowledge(id, false).unwrap();
                check_ex7(&net, &ever, &format!("dispatch {id}"));
                let n = net.trace.len();
                assert!(net.tx(OPERATOR, EX, "confirm_not_executed", exec_args(id, 0, 1)).result.is_ok());
                assert_eq!(trace_methods(&net, n), ms(&[("confirm_not_executed", true), ("settle", true), ("on_settled", true)]));
                assert_eq!(net.result, Some(Ok(vec![])), "confirm {id}: the lock accepted");
                ever.insert(id);
                reference.settle(EX, id, false).unwrap();
                check_ex7(&net, &ever, &format!("confirm {id}"));
                assert_eq!((net.exec_state().newest, ids(&net)), (id, range(id.saturating_sub(255), id)), "after {id}");
                if id % 50 == 0 || id >= 255 {
                    assert_eq!(net.pool_state(), reference, "record {id}");
                }
            }
            let p = net.pool_state();
            assert_eq!(p, reference);
            assert_eq!((p.first_id, p.records.len()), (44, 256));
            assert!(p.records.iter().all(|c| c.status == Status::Cancelled));
            let window = range(44, 299);
            assert_eq!(ids(&net), window);
            // Forgotten id 43: stale for a confirmation, unknown for finish.
            let out = net.tx(OPERATOR, EX, "confirm_not_executed", exec_args(43, 0, 1));
            assert!(out.fail().panic_msg.as_deref().unwrap_or("").contains("stale id"));
            let out = net.tx(OPERATOR, EX, "finish", json!({ "id": 43, "completed": false }));
            assert!(out.fail().panic_msg.as_deref().unwrap_or("").contains("unknown job"));
            // A refused confirmation (an unassigned id): the transaction fails
            // with the lock's error; on_settled still runs, changes nothing,
            // and the job stays.
            let n = net.trace.len();
            assert!(net.tx(OPERATOR, EX, "confirm_not_executed", exec_args(10_000, 0, 1)).result.is_ok());
            assert_eq!(trace_methods(&net, n), ms(&[("confirm_not_executed", true), ("settle", false), ("on_settled", true)]));
            assert_eq!(net.trace[n + 1].2.as_ref().unwrap_err().panic_msg.as_deref(), Some("NotFound"));
            assert_eq!(net.result, Some(Err(())), "the transaction reports the lock's error");
            let mut held = window.clone();
            held.push(10_000);
            assert_eq!(ids(&net), held);
            assert_eq!(net.exec_state().newest, 299);
            assert_eq!(net.pool_state(), reference);
            // on_settled is private, and needs the settle's result. An id of
            // 600 would forget every job below 345 if either went through.
            let out = net.tx(OPERATOR, EX, "on_settled", json!({ "id": 600 }));
            assert_eq!(out.fail().panic_msg.as_deref(), Some("Method on_settled is private"));
            let out = net.tx(EX, EX, "on_settled", json!({ "id": 600 }));
            assert!(out.fail().panic_msg.as_deref().unwrap_or("").contains("not a settle callback"));
            assert_eq!(ids(&net), held);
            // A Reserved record confirmed too early: the lock refuses
            // (InvalidState); the job stays and answers the later execute.
            net.tx(OPERATOR, EX, "set_mode", json!({ "mode": "normal" }));
            let q = req(&reference);
            assert_eq!(net.tx(A1, POOL, "submit", json!({ "request": request_json(&q) })).json()["id"], json!(300));
            reference.submit(A1, q, now).unwrap();
            let n = net.trace.len();
            net.tx(OPERATOR, EX, "confirm_not_executed", exec_args(300, 0, 1));
            assert_eq!(net.trace[n + 1].2.as_ref().unwrap_err().panic_msg.as_deref(), Some("InvalidState"));
            assert_eq!(net.result, Some(Err(())));
            assert_eq!(net.pool_state(), reference);
            held.push(300);
            assert_eq!(ids(&net), held, "job 300 pushed, 44 still held");
            check_ex7(&net, &ever, "early confirmation");
            // Dispatch: execute answers "cancelled" from job 300, so the lock
            // cancels the record. A stored job moves nothing.
            net.tx(A1, POOL, "dispatch", json!({ "id": 300 }));
            reference.dispatch(A1, 300, now).unwrap();
            reference.settle(EX, 300, false).unwrap();
            let p = net.pool_state();
            assert_eq!(p, reference);
            assert_eq!(p.record(300).unwrap().status, Status::Cancelled);
            assert_eq!(ids(&net), held);
            ever.insert(300);
            check_ex7(&net, &ever, "dispatch 300");
            // finish(300, false) is accepted (a same-outcome no-op at the
            // lock); its on_settled raises newest to 300 and forgets job 44.
            net.tx(OPERATOR, EX, "finish", json!({ "id": 300, "completed": false }));
            assert_eq!(net.result, Some(Ok(vec![])));
            reference.settle(EX, 300, false).unwrap();
            assert_eq!(net.pool_state(), reference);
            let mut held = range(45, 299);
            held.extend([10_000, 300]);
            assert_eq!((net.exec_state().newest, ids(&net)), (300, held), "finish moved the newest id");
            check_ex7(&net, &ever, "finish 300");
            // Not wedged: the next record runs end to end.
            let q = req(&reference);
            assert_eq!(net.tx(A1, POOL, "submit", json!({ "request": request_json(&q) })).json()["id"], json!(301));
            reference.submit(A1, q, now).unwrap();
            net.tx(A1, POOL, "dispatch", json!({ "id": 301 }));
            reference.dispatch(A1, 301, now).unwrap();
            reference.acknowledge(301, true).unwrap();
            assert_eq!(net.pool_state(), reference);
            ever.insert(301);
            let mut held = range(46, 299);
            held.extend([10_000, 300, 301]);
            assert_eq!(ids(&net), held, "execute of 301 forgot job 45");
            check_ex7(&net, &ever, "dispatch 301");
            net.tx(OPERATOR, EX, "finish", json!({ "id": 301, "completed": true }));
            assert_eq!(net.result, Some(Ok(vec![])));
            reference.settle(EX, 301, true).unwrap();
            let p = net.pool_state();
            assert_eq!(p, reference);
            assert_eq!(p.record(301).unwrap().status, Status::Completed);
            assert_eq!((p.first_id, p.completed_units.clone()), (46, vec![1]));
            check_ex7(&net, &ever, "finish 301");
        }
    });
}
