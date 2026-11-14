//! Conformance traces of the real `pool-core` (strategy.md section 10).
//!
//! One generator, several replayers: every trace in `traces/core.txt` is
//! replayed by the Prolog reference semantics (MC-2), the TLA+ model (REF-5)
//! and the Lean machine (REF-5L). The format is Prolog terms, one per line:
//!
//! ```text
//! trace(N).
//! config(Owner, MaxAge, [m(Min,Max,Initial),..], [t(Metric,Target,Weight,Shape),..],
//!        [r(Actor,Executor,[Reserve..],[Complete..],MaxUnits,[p(Recipe,Units),..]),..]).
//! init(Now, State).                 % or init_error(Error). for a refused config
//! call(Op, Result, State).          % one per step
//! end.
//!
//! Shape  = target | excess | deficit
//! Op     = submit(Caller,Recipe,Units,Version,ExpiresAt,Now) | dispatch(Caller,Id,Now)
//!        | ack(Id,Accepted) | cancel(Caller,Id,Now) | settle(Caller,Id,Completed)
//!        | pause(Caller,Paused) | observe(Caller,[Values..],Now)
//! Result = ok | ok(Id) | err(Error)   Error = snake_case of pool_core::Error
//! State  = s([Values..], Version, ObservedAt, Paused, [c(Recipe,Units,ExpiresAt,Status),..],
//!            FirstId, [DoneUnits..])
//! Status = reserved | dispatched | active | unknown | completed | cancelled
//! ```
//!
//! Accounts are the atoms `o`, `a1`, `a2`, `x1`, `x2`, `z`; fixture account
//! names are renamed to these. Booleans are `true`/`false`. Record `j` of the
//! list has id `FirstId + j` (`Pool::first_id`; the window keeps at most
//! MAX_RECORDS records). `DoneUnits` is `Pool::completed_units`, one entry
//! per recipe. `ok(Id)` is `submit`'s result; every other success is `ok`.
//! A configuration `Config::validate` refuses is written as
//! `init_error(invalid_config).` with no `call` line (trace 210). The
//! contract section below fixes the trace numbers and the coverage classes.
//!
//! A second file, `traces/reads.txt` (COV-2), records the real core's two
//! reads, `preview` and `best_quantity`, at states of those traces. The Prolog
//! layer (tests/test_reads.pl) rebuilds each state and compares the model's
//! `preview_allowed/3`, guard walk and `best_quantity/4` with it:
//!
//! ```text
//! reads(Trace, Call).               % the pre-state of call Call (1-based) of
//!                                   % trace Trace of core.txt; Call = calls+1:
//!                                   % the final state; Call = 0: a state at
//!                                   % the version cap (below)
//! config(..).                       % as in core.txt
//! state(State).                     % as in core.txt (s/7)
//! q(Caller,Recipe,Units,Version,ExpiresAt,Now, Preview, Best).   % one per request
//! end.
//!
//! Preview = preview(Allowed, Reason, Candidate, Before, After)
//!   Reason    = allowed | Error (snake_case, as in core.txt)
//!   Candidate = none | [Values..]     (Decision::candidate)
//!   Before    = Energy                (Decision::before)
//!   After     = none | Energy         (Decision::after)
//! Best    = none | some(Units)        (best_quantity's units)
//! ```
//!
//! States: every pre-state of the wedge, fixture and finding traces (but
//! `window`, 523 calls), every READ_STRIDE-th pre-state of the others, every
//! pre-state with MAX_RECORDS records retained, and each trace's final state;
//! a state equal to the last one read (same `now`) is skipped. Trace 210
//! (`init_error`) has no state. States at the version cap: for traces 1-5
//! and 207, the final state with `version` set to VERSION_CAP - 1,
//! VERSION_CAP and u32::MAX (`reads(N, 0)`, three blocks each). They are not
//! reachable by the trace's calls (core.txt holds no state at or above
//! VERSION_CAP), but each block is self-contained. Requests: see
//! `requests`. The generator also checks, on the real core, that `submit` on
//! a copy of the pool agrees with every `preview` (Ok with id `FirstId + len`
//! and the candidate as the new values, or the same error; at u32::MAX
//! v1.0.0 disagreed), and fails if a read class in `REQUIRED_READS` never
//! occurs.
//!
//! usage: shared-pool-conformance [--traces N] [--len L] [--seed S]
//!        [--out FILE] [--check FILE] [--reads-out FILE] [--reads-check FILE]
//! `--check FILE` regenerates and fails if FILE differs (CI: the checked-in
//! traces are current); `--reads-out` and `--reads-check` do the same for
//! reads.txt. With none of the four, core.txt goes to stdout. Every run fails
//! if a class of `REQUIRED` never occurs or one of `FORBIDDEN` does;
//! `coverage.py` recomputes both from any core.txt.
//!
//! # v1.1.0 trace contract (frozen in W1 of the v1.1.0 re-verification)
//!
//! v1.0.0 (pin a4bb5c3) wrote `s/5` (no `FirstId`, `DoneUnits`), had no
//! `init_error` and 212 traces, with trace 207 `full`. For v1.1.0 (pin
//! 57b3f02) the generator (W2, this file) and every replayer (TLA+, Lean,
//! Prolog, the shell) follow this contract.
//!
//! ```text
//! State  = s([Values..], Version, ObservedAt, Paused, [c(Recipe,Units,ExpiresAt,Status),..],
//!            FirstId, [DoneUnits..])
//! ```
//!
//! - The first five positions of `s/7` are unchanged, and `c/4` is
//!   unchanged. Record `j` of the list has id `FirstId + j` (no longer its
//!   index); `DoneUnits` is `completed_units`, one entry per recipe:
//!   `min(u32::MAX, lifetime Completed units of the recipe)`.
//! - A refused configuration (`Config::validate` fails, e.g. actor =
//!   executor) is the whole trace:
//!   `trace(N). config(...). init_error(invalid_config). end.`
//! - Numbering: traces 0-211 keep their numbers (fixtures are chosen by
//!   name). Appended: 212 `station`, 213 `owner_exec`, 214
//!   `f1_recovery_done`, 215 `f1_recovery_released`: 216 traces. Traces 208
//!   and 209 keep their calls; only the results change.
//! - Trace 207 `window` (was `full`) gains a third recipe `gate` with
//!   `requires: [(0, 1)]`. Calls in order: id 0 (`nop`) driven to Completed;
//!   id 1 submitted and cancelled; id 2 kept Reserved; ids 3-255 submitted
//!   and cancelled; a `nop` submit gives `ok(256)` and prunes id 0; a `gate`
//!   submit gives `ok(257)` and prunes id 1 (its prerequisite met by
//!   `completed_units` only, id 0 being gone); a submit gives `err(full)` on
//!   the open front, id 2; cancel(2); a submit gives `ok(258)`; four calls on
//!   the pruned id 0 give `err(not_found)`; observe. About 520 calls; still
//!   the only 256-record trace.
//! - Trace 213 ends with an `observe` of the current values with no record
//!   open, so every replayer exercises the ghost choice (re-base, not
//!   refresh).
//! - Traces 214 and 215 use the configuration of upstream
//!   `crates/core/src/findings.rs:175` (a disturbing observe, the recovery
//!   hedge admitted, dispatch, ack, then `settle` true in 214 and false in
//!   215).
//! - Coverage (`coverage.py`): `required` adds `submit:prune`,
//!   `submit:full_open_front`, `submit:prereq_after_prune`,
//!   `submit:recovery_admitted`, `<op>:not_found_pruned`, `observe:refresh`,
//!   `cancel:invalid_state` and `init:invalid_config`. `forbidden` is every
//!   `cancel` or `settle` result `bounds` or `arithmetic`.
//! - No state in `core.txt` has a version at or above `VERSION_CAP`
//!   (u32::MAX - 768; TLC's integers); states at the cap are in `reads.txt`
//!   only.
//! - Ghosts, for the replayers that keep them: on observe Ok, `base` and
//!   `k0` are kept iff the new values equal the current values and some
//!   record is open (the refresh); otherwise `base = values`,
//!   `k0 = FirstId + len` (an id) and the pruned sum is 0.
//!
//! Coverage classes (`REQUIRED`, `FORBIDDEN`; `classes` and `coverage.py`
//! compute them from each call's pre-state, post-state and result):
//! `Op:Result` with Result `ok` (also for `ok(Id)`) or the error name;
//! `init:invalid_config` for a refused configuration; and
//!
//! - `submit:prune`: submit Ok and `FirstId` grew;
//! - `submit:full_open_front`: submit `full` with MAX_RECORDS records
//!   retained and the oldest one open;
//! - `submit:prereq_after_prune`: submit Ok of a recipe with a prerequisite
//!   that the retained Completed records alone do not meet;
//! - `submit:recovery_admitted`: submit Ok in a disturbed pre-state (a value
//!   outside its metric's range);
//! - `<op>:not_found_pruned`: dispatch, ack, cancel or settle `not_found` on
//!   an id below `FirstId`;
//! - `observe:refresh`: observe Ok with a record open.

use pool_core::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

const ACCOUNTS: [&str; 6] = ["o", "a1", "a2", "x1", "x2", "z"];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    fn pct(&mut self, p: u64) -> bool {
        self.next() % 100 < p
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

fn err_name(e: &Error) -> String {
    snake(&format!("{e:?}"))
}
/// `SnapshotExpired` -> `snapshot_expired`.
fn snake(d: &str) -> String {
    let mut s = String::new();
    for (i, ch) in d.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                s.push('_');
            }
            s.push(ch.to_ascii_lowercase());
        } else {
            s.push(ch);
        }
    }
    s
}
fn status_name(s: &Status) -> &'static str {
    match s {
        Status::Reserved => "reserved",
        Status::Dispatched => "dispatched",
        Status::Active => "active",
        Status::Unknown => "unknown",
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
    }
}
fn shape_name(s: &Shape) -> &'static str {
    match s {
        Shape::Target => "target",
        Shape::Excess => "excess",
        Shape::Deficit => "deficit",
    }
}
fn ints(v: &[i64]) -> String {
    let parts: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", parts.join(","))
}
fn state(p: &Pool) -> String {
    let recs: Vec<String> = p
        .records
        .iter()
        .map(|c| format!("c({},{},{},{})", c.recipe, c.units, c.expires_at, status_name(&c.status)))
        .collect();
    let done: Vec<String> = p.completed_units.iter().map(|u| u.to_string()).collect();
    format!(
        "s({},{},{},{},[{}],{},[{}])",
        ints(&p.values),
        p.version,
        p.observed_at,
        p.paused,
        recs.join(","),
        p.first_id,
        done.join(",")
    )
}
fn config_term(c: &Config) -> String {
    let ms: Vec<String> = c
        .metrics
        .iter()
        .map(|m| format!("m({},{},{})", m.min, m.max, m.initial))
        .collect();
    let ts: Vec<String> = c
        .terms
        .iter()
        .map(|t| format!("t({},{},{},{})", t.metric, t.target, t.weight, shape_name(&t.shape)))
        .collect();
    let rs: Vec<String> = c
        .recipes
        .iter()
        .map(|r| {
            let req: Vec<String> = r.requires.iter().map(|(p, u)| format!("p({p},{u})")).collect();
            format!(
                "r({},{},{},{},{},[{}])",
                r.actor,
                r.executor,
                ints(&r.reserve),
                ints(&r.complete),
                r.max_units,
                req.join(",")
            )
        })
        .collect();
    format!(
        "config({},{},[{}],[{}],[{}]).",
        c.owner,
        c.max_age_seconds,
        ms.join(","),
        ts.join(","),
        rs.join(",")
    )
}

/// A random small configuration that `Config::validate` accepts.
fn random_config(rng: &mut Rng) -> Config {
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
                .map(|_| if rng.pct(40) { 0 } else { rng.range(-3, 3) })
                .collect()
        };
        let recipes = (0..nr)
            .map(|i| {
                let requires = if i > 0 && rng.pct(30) {
                    vec![(rng.range(0, i as i64 - 1) as u32, rng.range(1, 3) as u32)]
                } else {
                    vec![]
                };
                Recipe {
                    name: format!("r{i}"),
                    actor: rng.pick(&["a1", "a2"]).into(),
                    executor: rng.pick(&["x1", "x2"]).into(),
                    reserve: vec(rng),
                    complete: vec(rng),
                    max_units: rng.range(1, 3) as u32,
                    requires,
                }
            })
            .collect();
        let c = Config {
            name: "rand".into(),
            owner: "o".into(),
            metrics,
            terms,
            recipes,
            max_age_seconds: if rng.pct(25) { rng.range(20, 60) as u32 } else { 0 },
        };
        if c.validate().is_ok() {
            return c;
        }
    }
}

/// A fixture with its account names renamed to the trace atoms: the first
/// actor is `a1` and every other actor `a2` (likewise `x1`, `x2` for
/// executors). TLA+'s TraceCheck fixes the account set, so `station`'s three
/// actors collapse to `a1`, `a2`, `a2`; trace 212 does not show their
/// separation (SCN-1 and the sandbox's six-fixture test do).
fn fixture(name: &str) -> Config {
    let mut c = fixtures::config(name).expect("fixture");
    let mut actors: BTreeMap<String, &str> = BTreeMap::new();
    let mut execs: BTreeMap<String, &str> = BTreeMap::new();
    c.owner = "o".into();
    for r in &mut c.recipes {
        let n = actors.len();
        let a = *actors.entry(r.actor.clone()).or_insert(["a1", "a2"][n.min(1)]);
        let n = execs.len();
        let x = *execs.entry(r.executor.clone()).or_insert(["x1", "x2"][n.min(1)]);
        r.actor = a.into();
        r.executor = x.into();
    }
    c
}

/// F-1: in a disturbed state, a hedge that cures by reservation. v1.0.0
/// admitted it and it could never settle; v1.1.0 refuses the submit
/// (`bounds`), so the later calls find no record.
fn wedge() -> (Config, Vec<Op>) {
    let m = |min, max, initial| Metric { name: "m".into(), unit: "u".into(), initial, min, max };
    let c = Config {
        name: "wedge".into(),
        owner: "o".into(),
        metrics: vec![m(0, 10, 5), m(-100, 100, 5)],
        terms: vec![Term { metric: 1, target: 0, weight: 1, shape: Shape::Target }],
        recipes: vec![Recipe {
            name: "hedge".into(),
            actor: "a1".into(),
            executor: "x1".into(),
            reserve: vec![-2, 0],
            complete: vec![2, -5],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    let ops = vec![
        Op::Observe("o".into(), vec![12, 5], 100),
        Op::Submit("a1".into(), Request { recipe: 0, units: 1, version: 1, expires_at: 200 }, 100),
        Op::Dispatch("a1".into(), 0, 101),
        Op::Ack(0, true),
        Op::Settle("x1".into(), 0, true),
        Op::Settle("x1".into(), 0, false),
        Op::Cancel("o".into(), 0, 102),
        Op::Observe("o".into(), vec![5, 5], 103),
    ];
    (c, ops)
}

/// Arithmetic at the value bound (submit's add and the envelope), and the
/// edges of the expiry window: dispatch at now = expires_at - 1 and = expires_at,
/// submit with expires_at = now + 86400 (accepted) and + 86401 (Expired).
fn arith() -> (Config, Vec<Op>) {
    let m = Metric { name: "m".into(), unit: "u".into(), initial: 999_990, min: -MAX_VALUE, max: MAX_VALUE };
    let r = |name: &str, reserve: i64, complete: i64| Recipe {
        name: name.into(),
        actor: "a1".into(),
        executor: "x1".into(),
        reserve: vec![reserve],
        complete: vec![complete],
        max_units: 3,
        requires: vec![],
    };
    let c = Config {
        name: "arith".into(),
        owner: "o".into(),
        metrics: vec![m],
        // Target next to the bound keeps the residual (and w*r^2) small: TLC
        // integers are 32-bit, the core computes energy in i128.
        terms: vec![Term { metric: 0, target: 999_990, weight: 1, shape: Shape::Excess }],
        recipes: vec![r("up", 5, 0), r("tail", 0, 5), r("down", -1, 0)],
        max_age_seconds: 0,
    };
    let q = |recipe, units, version, expires_at| Request { recipe, units, version, expires_at };
    let ops = vec![
        Op::Submit("a1".into(), q(0, 3, 0, 150), 100),    // 999990 + 15 > 1e6: add Arithmetic
        Op::Submit("a1".into(), q(0, 2, 0, 150), 100),    // = 1e6 exactly: energy rises, Energy
        Op::Submit("a1".into(), q(1, 3, 0, 150), 100),    // envelope high 1000005: Arithmetic
        Op::Submit("a1".into(), q(2, 1, 0, 100 + 86_400), 100), // horizon bound accepted
        Op::Submit("a1".into(), q(2, 1, 1, 100 + 86_401), 100), // one past: Expired
        Op::Submit("a1".into(), q(2, 1, 1, 105), 100),
        Op::Dispatch("a1".into(), 1, 104),                // now = expires_at - 1: Ok
        Op::Submit("a1".into(), q(2, 1, 3, 106), 100),
        Op::Dispatch("a1".into(), 2, 106),                // now = expires_at: Expired
        Op::Cancel("a1".into(), 2, 106),
        Op::Observe("o".into(), vec![MAX_VALUE], 107),    // Pending (records 0, 1 open)
    ];
    (c, ops)
}

/// Builds a trace on a scratch pool: each submit carries the pool's current
/// version, and each call must give `want`, the shape the header promises
/// the replayers (a core that does otherwise stops the generator).
struct Script {
    name: &'static str,
    p: Pool,
    ops: Vec<Op>,
}
impl Script {
    fn new(name: &'static str, c: &Config) -> Self {
        Script { name, p: Pool::new(c.clone(), 100).expect("valid config"), ops: vec![] }
    }
    fn call(&mut self, op: Op, want: &str) {
        let r = apply(&mut self.p, &op);
        assert_eq!(r, want, "{}: call {} {}", self.name, self.ops.len() + 1, op_term(&op));
        self.ops.push(op);
    }
    fn submit(&mut self, caller: &str, recipe: u32, expires_at: u64, now: u64, want: &str) {
        let req = Request { recipe, units: 1, version: self.p.version, expires_at };
        self.call(Op::Submit(caller.into(), req, now), want);
    }
}

/// F-3 (trace 207; v1.0.0's `full` ended Full for everyone after 256
/// lifetime records): the window of MAX_RECORDS records, its prune, Full
/// only with an open oldest record, a prerequisite met across a prune, and
/// NotFound for a pruned id. The calls are listed in the header.
fn window() -> (Config, Vec<Op>) {
    let m = Metric { name: "m".into(), unit: "u".into(), initial: 0, min: -10, max: 10 };
    let r = |name: &str, actor: &str, requires: Vec<(u32, u32)>| Recipe {
        name: name.into(),
        actor: actor.into(),
        executor: "x1".into(),
        reserve: vec![0],
        complete: vec![0],
        max_units: 1,
        requires,
    };
    let c = Config {
        name: "window".into(),
        owner: "o".into(),
        metrics: vec![m],
        terms: vec![Term { metric: 0, target: 0, weight: 1, shape: Shape::Target }],
        recipes: vec![r("nop", "a1", vec![]), r("other", "a2", vec![]), r("gate", "a1", vec![(0, 1)])],
        max_age_seconds: 0,
    };
    let mut s = Script::new("window", &c);
    // id 0 to Completed: completed_units[nop] = 1.
    s.submit("a1", 0, 500, 100, "ok(0)");
    s.call(Op::Dispatch("a1".into(), 0, 100), "ok");
    s.call(Op::Ack(0, true), "ok");
    s.call(Op::Settle("x1".into(), 0, true), "ok");
    s.submit("a1", 0, 500, 100, "ok(1)");
    s.call(Op::Cancel("a1".into(), 1, 100), "ok");
    // id 2 stays Reserved until it is the oldest record.
    s.submit("a2", 1, 500, 100, "ok(2)");
    for id in 3..MAX_RECORDS as u32 {
        s.submit("a1", 0, 500, 100, &format!("ok({id})"));
        s.call(Op::Cancel("a1".into(), id, 100), "ok");
    }
    // 256 retained, the oldest (id 0) Completed: the submit prunes it.
    s.submit("a1", 0, 500, 100, "ok(256)");
    // gate needs 1 completed unit of nop: only completed_units has it now.
    s.submit("a1", 2, 500, 100, "ok(257)");
    // The oldest record is id 2, open: Full until it is resolved.
    s.submit("a1", 0, 500, 100, "err(full)");
    s.call(Op::Cancel("a2".into(), 2, 100), "ok");
    s.submit("a1", 0, 500, 100, "ok(258)");
    s.call(Op::Dispatch("a1".into(), 0, 100), "err(not_found)");
    s.call(Op::Ack(0, true), "err(not_found)");
    s.call(Op::Cancel("a1".into(), 0, 100), "err(not_found)");
    s.call(Op::Settle("x1".into(), 0, true), "err(not_found)");
    // Ids 256-258 are open: an observe of the current values is the refresh.
    s.call(Op::Observe("o".into(), vec![0], 101), "ok");
    (c, s.ops)
}

/// F-2b, release `Arithmetic` (found by the TLA+ layer): a release undoes a
/// reservation, which the v1.0.0 envelope did not bound, so after a
/// sibling's completion relied on that reservation, `values - reserve*units`
/// left +-MAX_VALUE: cancel and settle(false) of A failed Arithmetic. v1.1.0
/// counts the release in the envelope and refuses B's submit
/// (`arithmetic`); the calls are unchanged, only the results differ.
fn release_arith() -> (Config, Vec<Op>) {
    let m = |name: &str, lim: i64| Metric { name: name.into(), unit: "u".into(), initial: 0, min: -lim, max: lim };
    let r = |name: &str, reserve: Vec<i64>, complete: Vec<i64>, max_units| Recipe {
        name: name.into(),
        actor: "a1".into(),
        executor: "x1".into(),
        reserve,
        complete,
        max_units,
        requires: vec![],
    };
    let c = Config {
        name: "release-arith".into(),
        owner: "o".into(),
        metrics: vec![m("m0", MAX_VALUE), m("m1", 10)],
        terms: vec![Term { metric: 1, target: 0, weight: 1, shape: Shape::Target }],
        recipes: vec![r("a", vec![-MAX_VALUE, 0], vec![0, 0], 1), r("b", vec![0, 0], vec![15_000, 0], 100)],
        max_age_seconds: 0,
    };
    let q = |recipe, units, version| Request { recipe, units, version, expires_at: 500 };
    // Results are v1.1.0's; [v1.0.0: ...] is the old core's, for the record.
    let ops = vec![
        Op::Submit("a1".into(), q(0, 1, 0), 100),   // m0 = -1e6
        Op::Submit("a1".into(), q(1, 100, 1), 100), // high -1e6 + 1e6 (A's release) + 1.5e6: Arithmetic [v1.0.0: Ok(1)]
        Op::Dispatch("a1".into(), 1, 100),          // NotFound [v1.0.0: Ok]
        Op::Ack(1, true),                           // NotFound [v1.0.0: Ok]
        Op::Settle("x1".into(), 1, true),           // NotFound [v1.0.0: Ok, m0 = 5e5]
        Op::Cancel("a1".into(), 0, 101),            // release: m0 = 0, Ok [v1.0.0: 1.5e6, Arithmetic]
        Op::Cancel("o".into(), 0, 101),             // already Cancelled: Ok, no change [v1.0.0: Arithmetic]
        Op::Dispatch("a1".into(), 0, 101),          // InvalidState [v1.0.0: Ok]
        Op::Ack(0, true),                           // late ack of a Cancelled record: Ok, no change [v1.0.0: Ok]
        Op::Settle("x1".into(), 0, false),          // Cancelled: Ok, no change [v1.0.0: release again, Arithmetic]
        Op::Observe("o".into(), vec![500_000, 0], 102), // none open: Ok [v1.0.0: Pending]
        Op::Settle("x1".into(), 0, true),           // InvalidState [v1.0.0: Ok, completion still worked]
        Op::Observe("o".into(), vec![500_000, 0], 103), // Ok
    ];
    (c, ops)
}

/// F-2 (Bounds form) in the supplied `risk` fixture, healthy start, no
/// observe: v1.0.0 admitted a hedge (recipe 3) on the strength of a credit
/// position's (recipe 0) reservation; after the hedge completed, the
/// executor's honest settle(false) for the position failed Bounds. v1.1.0
/// refuses the hedge's submit (`bounds`); the calls are unchanged.
fn f2_risk() -> (Config, Vec<Op>) {
    let c = fixture("risk");
    let q = |recipe, units, version| Request { recipe, units, version, expires_at: 200 };
    let ops = vec![
        Op::Submit("a1".into(), q(0, 1, 0), 100),
        Op::Submit("a2".into(), q(3, 8, 1), 100),
        Op::Dispatch("a1".into(), 0, 100),
        Op::Ack(0, true),
        Op::Dispatch("a2".into(), 1, 100),
        Op::Ack(1, true),
        Op::Settle("x1".into(), 1, true),
        Op::Settle("x1".into(), 0, false), // honest non-execution: Bounds
        Op::Cancel("o".into(), 0, 101),    // Pending
        Op::Cancel("a1".into(), 0, 101),   // Pending
        Op::Observe("o".into(), vec![60, 40, 30, 40], 102), // Pending
        Op::Settle("x1".into(), 0, true),  // only a completion report resolves it
        Op::Observe("o".into(), vec![60, 40, 30, 40], 103),
    ];
    (c, ops)
}

/// F-4: v1.0.0's `validate` accepted actor = executor (recipe 0) and owner =
/// executor (recipe 1): a principal attests its own completion. v1.1.0
/// refuses the configuration, so trace 210 is `init_error(invalid_config)`
/// and the calls below (the ones v1.0.0 accepted) are not written.
fn f4_self() -> (Config, Vec<Op>) {
    let m = Metric { name: "m".into(), unit: "u".into(), initial: 5, min: 0, max: 10 };
    let r = |name: &str, actor: &str, executor: &str| Recipe {
        name: name.into(),
        actor: actor.into(),
        executor: executor.into(),
        reserve: vec![-1],
        complete: vec![0],
        max_units: 2,
        requires: vec![],
    };
    let c = Config {
        name: "f4-self".into(),
        owner: "o".into(),
        metrics: vec![m],
        terms: vec![Term { metric: 0, target: 0, weight: 1, shape: Shape::Excess }],
        recipes: vec![r("self", "a1", "a1"), r("owner", "o", "o")],
        max_age_seconds: 0,
    };
    let q = |recipe, version| Request { recipe, units: 1, version, expires_at: 200 };
    let ops = vec![
        Op::Submit("a1".into(), q(0, 0), 100),
        Op::Dispatch("a1".into(), 0, 100),
        Op::Ack(0, true),
        Op::Settle("a1".into(), 0, true), // the actor attests its own work
        Op::Submit("o".into(), q(1, 4), 100),
        Op::Dispatch("o".into(), 1, 100),
        Op::Ack(1, true),
        Op::Settle("o".into(), 1, true),  // the owner attests its own work
    ];
    (c, ops)
}

/// F-6: with max_age > 0, one long-open record blocked the only snapshot
/// refresh in v1.0.0 (observe Pending), so admissions and dispatches stopped
/// (SnapshotExpired) until that record settled. v1.1.0 accepts an observe of
/// the current values with records open (the refresh), so the observe at
/// call 7 is Ok; the calls are unchanged.
fn f6_stale() -> (Config, Vec<Op>) {
    let m = Metric { name: "m".into(), unit: "u".into(), initial: 5, min: 0, max: 10 };
    let r = |name: &str| Recipe {
        name: name.into(),
        actor: "a1".into(),
        executor: "x1".into(),
        reserve: vec![-1],
        complete: vec![0],
        max_units: 2,
        requires: vec![],
    };
    let c = Config {
        name: "f6-stale".into(),
        owner: "o".into(),
        metrics: vec![m],
        terms: vec![Term { metric: 0, target: 0, weight: 1, shape: Shape::Excess }],
        recipes: vec![r("slow"), r("next")],
        max_age_seconds: 50,
    };
    let q = |recipe, version, expires_at| Request { recipe, units: 1, version, expires_at };
    let ops = vec![
        Op::Submit("a1".into(), q(0, 0, 1000), 100),
        Op::Dispatch("a1".into(), 0, 100),
        Op::Ack(0, true),
        Op::Submit("a1".into(), q(1, 3, 1000), 120),
        Op::Submit("a1".into(), q(1, 4, 1000), 151),        // SnapshotExpired
        Op::Dispatch("a1".into(), 1, 151),                  // SnapshotExpired
        Op::Observe("o".into(), vec![3], 151),              // Pending: record 0 open
        Op::Cancel("a1".into(), 1, 152),
        Op::Observe("o".into(), vec![4], 152),              // still Pending
        Op::Settle("x1".into(), 0, true),
        Op::Observe("o".into(), vec![4], 153),
        Op::Submit("a1".into(), q(1, 7, 1000), 153),         // admissions resume
    ];
    (c, ops)
}

/// Q3 (trace 213; F-4's residual, an upstream owner decision): the owner may
/// be a recipe's executor when the actor is another account. The owner
/// settles; the actor's settle is Unauthorized. The trace ends with an
/// observe of the current values with no record open, which re-bases the
/// replayers' ghosts (header).
fn owner_exec() -> (Config, Vec<Op>) {
    let m = Metric { name: "m".into(), unit: "u".into(), initial: 5, min: 0, max: 10 };
    let c = Config {
        name: "owner-exec".into(),
        owner: "o".into(),
        metrics: vec![m],
        terms: vec![Term { metric: 0, target: 0, weight: 1, shape: Shape::Excess }],
        recipes: vec![Recipe {
            name: "owner-executes".into(),
            actor: "a1".into(),
            executor: "o".into(),
            reserve: vec![-1],
            complete: vec![0],
            max_units: 2,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    let mut s = Script::new("owner_exec", &c);
    s.submit("a1", 0, 200, 100, "ok(0)");
    s.call(Op::Dispatch("a1".into(), 0, 100), "ok");
    s.call(Op::Ack(0, true), "ok");
    s.call(Op::Settle("o".into(), 0, true), "ok");
    s.call(Op::Settle("a1".into(), 0, true), "err(unauthorized)");
    let values = s.p.values.clone();
    s.call(Op::Observe("o".into(), values, 101), "ok");
    (c, s.ops)
}

/// F-1, the shape that stays allowed (traces 214 and 215; upstream
/// `crates/core/src/findings.rs:175`, `f1_recovery_by_completion_is_admitted_and_live`):
/// an observe puts metric 0 at 12 (max 10); a hedge that reserves only
/// collateral (metric 1) and cures metric 0 when it completes is admitted,
/// dispatched and acknowledged, then settled: completed (214) or not (215).
fn f1_recovery(completed: bool) -> (Config, Vec<Op>) {
    let m = |min, max, initial| Metric { name: "m".into(), unit: "u".into(), initial, min, max };
    let c = Config {
        name: "f1-recovery".into(),
        owner: "o".into(),
        metrics: vec![m(0, 10, 5), m(-100, 100, 5)],
        terms: vec![Term { metric: 1, target: 0, weight: 1, shape: Shape::Target }],
        recipes: vec![Recipe {
            name: "hedge".into(),
            actor: "a1".into(),
            executor: "x1".into(),
            reserve: vec![0, -1],
            complete: vec![-3, 0],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    let mut s = Script::new("f1_recovery", &c);
    s.call(Op::Observe("o".into(), vec![12, 5], 100), "ok");
    s.submit("a1", 0, 200, 100, "ok(0)");
    s.call(Op::Dispatch("a1".into(), 0, 100), "ok");
    s.call(Op::Ack(0, true), "ok");
    s.call(Op::Settle("x1".into(), 0, completed), "ok");
    (c, s.ops)
}

#[derive(Clone, Debug)]
enum Op {
    Submit(String, Request, u64),
    Dispatch(String, u32, u64),
    Ack(u32, bool),
    Cancel(String, u32, u64),
    Settle(String, u32, bool),
    Pause(String, bool),
    Observe(String, Vec<i64>, u64),
}

fn op_term(op: &Op) -> String {
    match op {
        Op::Submit(c, r, now) => format!(
            "submit({c},{},{},{},{},{now})",
            r.recipe, r.units, r.version, r.expires_at
        ),
        Op::Dispatch(c, id, now) => format!("dispatch({c},{id},{now})"),
        Op::Ack(id, a) => format!("ack({id},{a})"),
        Op::Cancel(c, id, now) => format!("cancel({c},{id},{now})"),
        Op::Settle(c, id, done) => format!("settle({c},{id},{done})"),
        Op::Pause(c, p) => format!("pause({c},{p})"),
        Op::Observe(c, v, now) => format!("observe({c},{},{now})", ints(v)),
    }
}

fn apply(p: &mut Pool, op: &Op) -> String {
    let unit = |r: Result<(), Error>| match r {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("err({})", err_name(&e)),
    };
    match op.clone() {
        Op::Submit(c, r, now) => match p.submit(&c, r, now) {
            Ok(id) => format!("ok({id})"),
            Err(e) => format!("err({})", err_name(&e)),
        },
        Op::Dispatch(c, id, now) => unit(p.dispatch(&c, id, now)),
        Op::Ack(id, a) => unit(p.acknowledge(id, a)),
        Op::Cancel(c, id, now) => unit(p.cancel(&c, id, now)),
        Op::Settle(c, id, done) => unit(p.settle(&c, id, done)),
        Op::Pause(c, b) => unit(p.set_paused(&c, b)),
        Op::Observe(c, v, now) => unit(p.observe(&c, v, now)),
    }
}

/// A random next call, biased toward calls whose guards pass. Ids are
/// `first_id + k`; while `first_id` is 0 (every trace but 207, which is not
/// random) the draws are v1.0.0's, so traces 1-205 keep their calls.
fn random_op(rng: &mut Rng, p: &Pool, now: u64) -> Op {
    let cfg = &p.config;
    let nrec = p.records.len() as i64;
    let first = i64::from(p.first_id);
    let other = |rng: &mut Rng| rng.pick(&ACCOUNTS).to_string();
    let id = |rng: &mut Rng| -> u32 {
        if nrec == 0 || rng.pct(5) {
            // Any id up to two past the newest; once records were pruned,
            // half of these draws are a pruned id instead.
            if first > 0 && rng.pct(50) {
                rng.range(0, first - 1) as u32
            } else {
                (first + rng.range(0, nrec + 1)) as u32
            }
        } else {
            // Prefer recent and open records.
            let open: Vec<u32> = p
                .records
                .iter()
                .filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled))
                .map(|c| c.id)
                .collect();
            if !open.is_empty() && rng.pct(80) {
                open[(rng.next() % open.len() as u64) as usize]
            } else {
                (first + rng.range(0, nrec - 1)) as u32
            }
        }
    };
    let k = rng.range(0, 99);
    if k < 30 {
        let recipe = if rng.pct(95) { rng.range(0, cfg.recipes.len() as i64 - 1) } else { cfg.recipes.len() as i64 } as u32;
        let r = cfg.recipes.get(recipe as usize);
        let caller = match r {
            Some(r) if rng.pct(85) => r.actor.clone(),
            _ => other(rng),
        };
        let maxu = r.map_or(3, |r| r.max_units) as i64;
        let units = if rng.pct(90) { rng.range(1, maxu) } else { rng.range(0, maxu + 1) } as u32;
        let version = if rng.pct(90) { p.version } else { p.version.wrapping_add(rng.range(0, 2) as u32).wrapping_sub(1) };
        let expires_at = if rng.pct(92) {
            now + rng.range(1, 60) as u64
        } else if rng.pct(50) {
            now
        } else {
            now + 86_401
        };
        Op::Submit(caller, Request { recipe, units, version, expires_at }, now)
    } else if k < 48 {
        let i = id(rng);
        let caller = match p.record(i).ok() {
            Some(c) if rng.pct(85) => cfg.recipes[c.recipe as usize].actor.clone(),
            _ => other(rng),
        };
        Op::Dispatch(caller, i, now)
    } else if k < 60 {
        Op::Ack(id(rng), rng.pct(60))
    } else if k < 68 {
        let i = id(rng);
        let caller = match p.record(i).ok() {
            Some(c) if rng.pct(80) => {
                if rng.pct(70) { cfg.recipes[c.recipe as usize].actor.clone() } else { cfg.owner.clone() }
            }
            _ => other(rng),
        };
        Op::Cancel(caller, i, now)
    } else if k < 88 {
        let i = id(rng);
        let caller = match p.record(i).ok() {
            Some(c) if rng.pct(85) => cfg.recipes[c.recipe as usize].executor.clone(),
            _ => other(rng),
        };
        Op::Settle(caller, i, rng.pct(70))
    } else if k < 93 {
        let caller = if rng.pct(85) { cfg.owner.clone() } else { other(rng) };
        let b = if p.paused { rng.pct(20) } else { rng.pct(30) };
        Op::Pause(caller, b)
    } else {
        let caller = if rng.pct(90) { cfg.owner.clone() } else { other(rng) };
        let mut v: Vec<i64> = cfg
            .metrics
            .iter()
            .map(|m| if rng.pct(70) { rng.range(m.min, m.max) } else { rng.range(m.min - 3, m.max + 3) })
            .collect();
        if rng.pct(4) {
            v.pop();
        } else if rng.pct(3) {
            v[0] = MAX_VALUE + 1;
        }
        Op::Observe(caller, v, now)
    }
}

type Hits = BTreeMap<String, usize>;

fn hit(hits: &mut Hits, k: &str) {
    *hits.entry(k.to_string()).or_insert(0) += 1;
}
fn op_name(op: &Op) -> &'static str {
    match op {
        Op::Submit(..) => "submit",
        Op::Dispatch(..) => "dispatch",
        Op::Ack(..) => "ack",
        Op::Cancel(..) => "cancel",
        Op::Settle(..) => "settle",
        Op::Pause(..) => "pause",
        Op::Observe(..) => "observe",
    }
}
fn is_open(s: &Status) -> bool {
    !matches!(s, Status::Completed | Status::Cancelled)
}
/// Some value is outside its metric's range.
fn disturbed(p: &Pool) -> bool {
    p.values.iter().zip(&p.config.metrics).any(|(v, m)| *v < m.min || *v > m.max)
}

/// The classes of one call (header, "Coverage classes"): `Op:Result`, then
/// the derived ones, from the pre-state `a`, the post-state `b` and the
/// result `res` as written to core.txt.
fn classes(op: &Op, res: &str, a: &Pool, b: &Pool) -> Vec<String> {
    let name = op_name(op);
    let err = res.strip_prefix("err(").and_then(|r| r.strip_suffix(')'));
    let mut ks = vec![format!("{name}:{}", err.unwrap_or("ok"))];
    match (op, err) {
        (Op::Submit(_, req, _), None) => {
            if b.first_id > a.first_id {
                ks.push("submit:prune".into());
            }
            if disturbed(a) {
                ks.push("submit:recovery_admitted".into());
            }
            let retained = |q: u32| -> u64 {
                a.records
                    .iter()
                    .filter(|c| c.recipe == q && c.status == Status::Completed)
                    .map(|c| u64::from(c.units))
                    .sum()
            };
            let r = &a.config.recipes[req.recipe as usize];
            if r.requires.iter().any(|(q, u)| retained(*q) < u64::from(*u)) {
                ks.push("submit:prereq_after_prune".into());
            }
        }
        (Op::Submit(..), Some("full")) if a.records.len() >= MAX_RECORDS && is_open(&a.records[0].status) => {
            ks.push("submit:full_open_front".into());
        }
        (Op::Dispatch(_, id, _) | Op::Ack(id, _) | Op::Cancel(_, id, _) | Op::Settle(_, id, _), Some("not_found"))
            if *id < a.first_id =>
        {
            ks.push(format!("{name}:not_found_pruned"));
        }
        (Op::Observe(..), None) if a.records.iter().any(|c| is_open(&c.status)) => {
            ks.push("observe:refresh".into());
        }
        _ => {}
    }
    ks
}

/// Writes one trace of core.txt and counts its classes.
fn emit(out: &mut String, hits: &mut Hits, n: usize, cfg: &Config, ops: &[Op], now0: u64) {
    writeln!(out, "trace({n}).").unwrap();
    writeln!(out, "{}", config_term(cfg)).unwrap();
    let mut p = match Pool::new(cfg.clone(), now0) {
        Ok(p) => p,
        Err(e) => {
            // A refused configuration is the whole trace (its calls are not written).
            writeln!(out, "init_error({}).", err_name(&e)).unwrap();
            writeln!(out, "end.").unwrap();
            hit(hits, &format!("init:{}", err_name(&e)));
            return;
        }
    };
    writeln!(out, "init({now0},{}).", state(&p)).unwrap();
    for op in ops {
        let a = p.clone();
        let r = apply(&mut p, op);
        // TLC's integers: no state of core.txt reaches the version cap.
        assert!(p.version < VERSION_CAP, "trace {n}: version {} reaches VERSION_CAP", p.version);
        writeln!(out, "call({},{r},{}).", op_term(op), state(&p)).unwrap();
        for k in classes(op, &r, &a, &p) {
            hit(hits, &k);
        }
    }
    writeln!(out, "end.").unwrap();
}

/// One trace of core.txt. `dense`: reads.txt reads every state of it, not
/// only every READ_STRIDE-th. `cap`: reads.txt also reads its final state
/// at the version cap.
struct Trace {
    cfg: Config,
    ops: Vec<Op>,
    dense: bool,
    cap: bool,
}

/// A fixture trace: the fixture's own scenario trace, then `len` random
/// calls seeded by the trace number `n`.
fn fixture_trace(name: &str, n: usize, len: usize, seed: u64) -> Trace {
    let c = fixture(name);
    let mut rng = Rng(seed ^ (0x9e37_79b9_7f4a_7c15 + n as u64));
    let mut p = Pool::new(c.clone(), 100).unwrap();
    let mut ops = vec![];
    let mut now = 100;
    for (r, u) in fixtures::trace(name) {
        let actor = c.recipes[r as usize].actor.clone();
        let op = Op::Submit(actor, Request { recipe: r, units: u, version: p.version, expires_at: now + 50 }, now);
        apply(&mut p, &op);
        ops.push(op);
    }
    for _ in 0..len {
        now += rng.range(0, 2) as u64;
        let op = random_op(&mut rng, &p, now);
        apply(&mut p, &op);
        ops.push(op);
    }
    Trace { cfg: c, ops, dense: true, cap: false }
}

/// The fixtures of traces 1-5, by name: upstream's `fixtures::NAMES` now
/// starts with `station`, which is trace 212.
const FIXTURES: [&str; 5] = ["gpu", "friday", "inventory", "hiring", "risk"];

/// The traces of core.txt, in file order (the trace number is the index).
fn all_traces(traces: usize, len: usize, seed: u64) -> Vec<Trace> {
    let mut all = vec![];
    let (cfg, ops) = wedge();
    all.push(Trace { cfg, ops, dense: true, cap: false });
    for name in FIXTURES {
        let mut t = fixture_trace(name, all.len(), len, seed);
        t.cap = true;
        all.push(t);
    }
    for t in 0..traces {
        let mut rng = Rng(seed.wrapping_mul(t as u64 + 1) ^ 0x2545_f491_4f6c_dd1d);
        for _ in 0..4 {
            rng.next();
        }
        let c = random_config(&mut rng);
        let mut p = Pool::new(c.clone(), 100).unwrap();
        let mut ops = vec![];
        let mut now = 100;
        for _ in 0..len {
            now += rng.range(0, 3) as u64;
            let op = random_op(&mut rng, &p, now);
            apply(&mut p, &op);
            ops.push(op);
        }
        all.push(Trace { cfg: c, ops, dense: false, cap: false });
    }
    // Appended so the wedge, fixture and random trace numbers stay stable
    // (the Prolog and shell layers address fixtures 1-5 by number). `window`
    // (523 calls of near-identical states) is read at the stride.
    for ((cfg, ops), dense, cap) in [
        (arith(), true, false),
        (window(), false, true),
        (release_arith(), true, false),
        (f2_risk(), true, false),
        (f4_self(), true, false),
        (f6_stale(), true, false),
    ] {
        all.push(Trace { cfg, ops, dense, cap });
    }
    // v1.1.0's additions (212-215).
    let station = fixture_trace("station", all.len(), len, seed);
    all.push(station);
    for (cfg, ops) in [owner_exec(), f1_recovery(true), f1_recovery(false)] {
        all.push(Trace { cfg, ops, dense: true, cap: false });
    }
    all
}

fn generate(all: &[Trace]) -> (String, Hits) {
    let mut out = String::new();
    let mut hits = Hits::new();
    for (n, t) in all.iter().enumerate() {
        emit(&mut out, &mut hits, n, &t.cfg, &t.ops, 100);
    }
    (out, hits)
}

/// Classes core.txt must reach (header, "Coverage classes"): every reachable
/// error of every call, and the v1.1.0 behaviours the replayers must meet on
/// the real core: the window (prune, Full on an open oldest record, a
/// prerequisite met across a prune, NotFound for a pruned id), the
/// admitted recovery hedge (F-1), the refresh (F-6), cancel of a Completed
/// record (div-cancel) and a refused configuration (F-4). `coverage.py`
/// holds the same list.
const REQUIRED: [&str; 47] = [
    "submit:ok", "submit:arithmetic", "submit:full", "submit:paused", "submit:stale", "submit:expired", "submit:snapshot_expired",
    "submit:invalid_recipe", "submit:unauthorized", "submit:invalid_units", "submit:prerequisite",
    "submit:bounds", "submit:energy", "dispatch:ok", "dispatch:not_found", "dispatch:unauthorized",
    "dispatch:paused", "dispatch:invalid_state", "dispatch:expired", "dispatch:snapshot_expired",
    "ack:ok", "ack:not_found", "cancel:ok", "cancel:not_found", "cancel:unauthorized", "cancel:pending",
    "cancel:invalid_state", "settle:ok", "settle:not_found", "settle:unauthorized", "settle:invalid_state",
    "pause:ok", "pause:unauthorized", "observe:ok", "observe:unauthorized", "observe:pending", "observe:bounds",
    "submit:prune", "submit:full_open_front", "submit:prereq_after_prune", "submit:recovery_admitted",
    "dispatch:not_found_pruned", "ack:not_found_pruned", "cancel:not_found_pruned", "settle:not_found_pruned",
    "observe:refresh", "init:invalid_config",
];
/// Classes that must not occur: a cancel or settle never fails Bounds or
/// Arithmetic (LIVE-ALL on the real core; v1.0.0's F-1, F-2 and F-2b).
/// `coverage.py` holds the same list.
const FORBIDDEN: [&str; 4] = ["cancel:bounds", "cancel:arithmetic", "settle:bounds", "settle:arithmetic"];

// ---------------------------------------------------------------------------
// traces/reads.txt (COV-2): the real core's `preview` and `best_quantity`
// ---------------------------------------------------------------------------

/// Non-dense traces are read at every READ_STRIDE-th pre-state (and the final one).
const READ_STRIDE: usize = 10;
/// A units value far above MAX_UNITS, where `best_quantity` caps its search.
const LARGE_UNITS: u32 = 1_000_000;

/// Read classes reads.txt must reach; `preview:<reason>` for each error
/// `candidate` can return (Arithmetic split by branch: the reserve's `add`,
/// the envelope, and the version cap; Energy split by branch), both
/// `preview.candidate` cases, both `best_quantity` cases, an energy tie among
/// the allowed units (broken to the larger), a best that is not the largest
/// allowed units, a best for a request above MAX_UNITS; and states paused,
/// with a full window whose oldest record is open (submit is Full) or
/// terminal (submit prunes it), after a prune (`first_id > 0`), at or above
/// VERSION_CAP, and disturbed.
const REQUIRED_READS: [&str; 29] = [
    "preview:allowed", "preview:paused", "preview:stale", "preview:expired", "preview:snapshot_expired",
    "preview:full", "preview:invalid_recipe", "preview:unauthorized", "preview:invalid_units",
    "preview:prerequisite", "preview:arithmetic:reserve", "preview:arithmetic:envelope",
    "preview:arithmetic:version", "preview:bounds", "preview:energy:rise", "preview:energy:recovery",
    "candidate:some", "candidate:none", "best:some", "best:none", "best:tie", "best:not_largest", "best:capped",
    "state:paused", "state:full_open_front", "state:window_terminal_front", "state:pruned", "state:version_cap",
    "state:disturbed",
];
/// The versions of the states at the cap (header).
const CAP_VERSIONS: [u32; 3] = [VERSION_CAP - 1, VERSION_CAP, u32::MAX];

fn op_now(op: &Op) -> Option<u64> {
    match op {
        Op::Submit(_, _, n) | Op::Dispatch(_, _, n) | Op::Cancel(_, _, n) | Op::Observe(_, _, n) => Some(*n),
        Op::Ack(..) | Op::Settle(..) | Op::Pause(..) => None,
    }
}

/// The requests read at one state; `now` is the time of the call that follows
/// the state. For every recipe: its actor with units 0, 1, 2, max_units,
/// max_units + 1, MAX_UNITS, MAX_UNITS + 1 and LARGE_UNITS. For the index one
/// past the last recipe: a1 with units 1 and MAX_UNITS + 1. For one recipe
/// (rotating with `rot`) at its max_units: every account as caller; the
/// version plus 1 and, above 0, minus 1; expires_at = now - 1, now,
/// now + 86,400 and now + 86,401; with a max age, now = observed_at + max_age
/// and one second later. Duplicates are dropped.
fn requests(p: &Pool, now: u64, rot: usize) -> Vec<(String, Request, u64)> {
    let cfg = &p.config;
    let v = p.version;
    let nr = cfg.recipes.len();
    let mut qs: Vec<(String, Request, u64)> = vec![];
    let mut q = |caller: &str, recipe: usize, units: u32, version: u32, expires_at: u64, now: u64| {
        let r = (caller.to_string(), Request { recipe: recipe as u32, units, version, expires_at }, now);
        if !qs.contains(&r) {
            qs.push(r);
        }
    };
    for (r, rec) in cfg.recipes.iter().enumerate() {
        let mu = rec.max_units;
        for u in [0, 1, 2, mu, mu + 1, MAX_UNITS, MAX_UNITS + 1, LARGE_UNITS] {
            q(&rec.actor, r, u, v, now + 50, now);
        }
    }
    for u in [1, MAX_UNITS + 1] {
        q("a1", nr, u, v, now + 50, now);
    }
    let r = rot % nr;
    let (a, mu) = (cfg.recipes[r].actor.as_str(), cfg.recipes[r].max_units);
    for c in ACCOUNTS {
        q(c, r, mu, v, now + 50, now);
    }
    q(a, r, mu, v.wrapping_add(1), now + 50, now);
    if v > 0 {
        q(a, r, mu, v - 1, now + 50, now);
    }
    for e in [now.saturating_sub(1), now, now + 86_400, now + 86_401] {
        q(a, r, mu, v, e, now);
    }
    if cfg.max_age_seconds > 0 {
        let t = p.observed_at + u64::from(cfg.max_age_seconds);
        for t in [t, t + 1] {
            q(a, r, mu, v, t + 50, t);
        }
    }
    qs
}

#[derive(Default)]
struct Reads {
    out: String,
    hits: BTreeMap<String, usize>,
    /// `preview` and `submit` on a copy of the pool disagree (RD-PREVIEW on the real core).
    bad: Vec<String>,
    states: usize,
    queries: usize,
}

impl Reads {
    fn hit(&mut self, k: &str) {
        *self.hits.entry(k.to_string()).or_insert(0) += 1;
    }

    fn state(&mut self, n: usize, call: usize, p: &Pool, now: u64, rot: usize) {
        writeln!(self.out, "reads({n},{call}).").unwrap();
        writeln!(self.out, "{}", config_term(&p.config)).unwrap();
        writeln!(self.out, "state({}).", state(p)).unwrap();
        self.states += 1;
        if p.paused {
            self.hit("state:paused");
        }
        if p.records.len() >= MAX_RECORDS {
            self.hit(if is_open(&p.records[0].status) { "state:full_open_front" } else { "state:window_terminal_front" });
        }
        if p.first_id > 0 {
            self.hit("state:pruned");
        }
        if p.version >= VERSION_CAP {
            self.hit("state:version_cap");
        }
        if disturbed(p) {
            self.hit("state:disturbed");
        }
        for (caller, req, now) in requests(p, now, rot) {
            self.query(n, call, p, &caller, &req, now);
        }
        writeln!(self.out, "end.").unwrap();
    }

    fn query(&mut self, n: usize, call: usize, p: &Pool, caller: &str, req: &Request, now: u64) {
        let d = p.preview(caller, req, now);
        let best = p.best_quantity(caller, req, now);
        let reason = if d.allowed { "allowed".to_string() } else { snake(&d.reason) };
        let cand = d.candidate.as_deref().map_or("none".to_string(), ints);
        let after = d.after.clone().unwrap_or_else(|| "none".into());
        let b = best.as_ref().map_or("none".to_string(), |r| format!("some({})", r.units));
        writeln!(
            self.out,
            "q({caller},{},{},{},{},{now},preview({},{reason},{cand},{},{after}),{b}).",
            req.recipe, req.units, req.version, req.expires_at, d.allowed, d.before
        )
        .unwrap();
        self.queries += 1;

        // Classes, for the coverage check only (the oracle is the Prolog model).
        let class = match reason.as_str() {
            "arithmetic" if d.candidate.is_none() => "arithmetic:reserve".to_string(),
            "arithmetic" if only_the_cap(p, caller, req, now) => "arithmetic:version".to_string(),
            "arithmetic" => "arithmetic:envelope".to_string(),
            "energy" => {
                let e = |s: &Option<String>| s.as_deref().and_then(|x| x.parse::<u64>().ok());
                if e(&d.after) > e(&Some(d.before.clone())) { "energy:rise" } else { "energy:recovery" }.to_string()
            }
            r => r.to_string(),
        };
        self.hit(&format!("preview:{class}"));
        self.hit(if d.candidate.is_some() { "candidate:some" } else { "candidate:none" });
        self.hit(if best.is_some() { "best:some" } else { "best:none" });
        if best.is_some() {
            // Every allowed units value 1..=min(units, MAX_UNITS) by preview,
            // with its candidate's energy.
            let allowed: Vec<u64> = (1..=req.units.min(MAX_UNITS))
                .map(|u| p.preview(caller, &Request { units: u, ..req.clone() }, now))
                .filter(|e| e.allowed)
                .filter_map(|e| e.after.and_then(|x| x.parse().ok()))
                .collect();
            if let Some(&emin) = allowed.iter().min() {
                if allowed.iter().filter(|e| **e == emin).count() >= 2 {
                    self.hit("best:tie");
                }
                if allowed.last() != Some(&emin) {
                    self.hit("best:not_largest");
                }
                if req.units > MAX_UNITS {
                    self.hit("best:capped");
                }
            }
        }

        // RD-PREVIEW on the real core: submit on a copy agrees with preview,
        // with the id after the window's newest (a prune keeps it).
        let mut c = p.clone();
        let next_id = u64::from(p.first_id) + p.records.len() as u64;
        match c.submit(caller, req.clone(), now) {
            Ok(id) if d.allowed && u64::from(id) == next_id && d.candidate.as_ref() == Some(&c.values) => {}
            Err(e) if !d.allowed && e.to_string() == d.reason => {}
            r => self.bad.push(format!(
                "trace {n} call {call}: {caller} {req:?} at {now}: preview {} {}, submit {r:?}",
                d.allowed, d.reason
            )),
        }
    }
}

/// An Arithmetic preview at or above VERSION_CAP that only the cap refuses:
/// the same request at version VERSION_CAP - 1 is allowed. Every guard
/// before the cap reads the version only through Stale, which the request
/// passes, so the earlier guards decide alike at both versions.
fn only_the_cap(p: &Pool, caller: &str, req: &Request, now: u64) -> bool {
    if p.version < VERSION_CAP || req.version != p.version {
        return false;
    }
    let mut below = p.clone();
    below.version = VERSION_CAP - 1;
    below.preview(caller, &Request { version: below.version, ..req.clone() }, now).allowed
}

/// reads.txt: see the header of this file.
fn reads(all: &[Trace]) -> Reads {
    let mut rd = Reads::default();
    for (n, t) in all.iter().enumerate() {
        // A refused configuration (trace 210) has no state to read.
        let Ok(mut p) = Pool::new(t.cfg.clone(), 100) else { continue };
        let mut now = 100;
        let mut last: Option<(String, u64)> = None;
        let mut k = 0;
        for i in 0..=t.ops.len() {
            let at = t.ops.get(i).and_then(op_now).unwrap_or(now);
            if t.dense || i % READ_STRIDE == 0 || i == t.ops.len() || p.records.len() >= MAX_RECORDS {
                let key = (state(&p), at);
                if last.as_ref() != Some(&key) {
                    rd.state(n, i + 1, &p, at, n + k);
                    k += 1;
                    last = Some(key);
                }
            }
            if let Some(op) = t.ops.get(i) {
                apply(&mut p, op);
                now = op_now(op).unwrap_or(now);
            }
        }
        if t.cap {
            // The final state at the version cap (header): not reachable by
            // the trace, so core.txt has no such state; `reads(N, 0)`.
            for v in CAP_VERSIONS {
                let mut q = p.clone();
                q.version = v;
                rd.state(n, 0, &q, now, n + k);
                k += 1;
            }
        }
    }
    rd
}

/// `--check F`: F must equal `text`; `--out F`: write it.
fn check_or_write(text: &str, check: Option<String>, out: Option<String>, out_flag: &str) -> bool {
    if let Some(f) = check {
        let have = std::fs::read_to_string(&f).unwrap_or_default();
        if have != text {
            eprintln!("{f} is stale: regenerate with {out_flag} {f}");
            return false;
        }
        eprintln!("{f} is current");
    } else if let Some(f) = out {
        std::fs::write(&f, text).expect("write");
    }
    true
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str, d: u64| -> u64 {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map_or(d, |v| v.parse().expect("number"))
    };
    let path = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let all = all_traces(get("--traces", 200) as usize, get("--len", 40) as usize, get("--seed", 0x5eed));
    let (out, hits) = generate(&all);
    // Coverage the replayers rely on (REQUIRED), and LIVE-ALL (FORBIDDEN).
    for (k, v) in &hits {
        eprintln!("{k:32} {v}");
    }
    let missing: Vec<&str> = REQUIRED.iter().copied().filter(|k| !hits.contains_key(*k)).collect();
    let forbidden: Vec<&str> = FORBIDDEN.iter().copied().filter(|k| hits.contains_key(*k)).collect();
    if !missing.is_empty() {
        eprintln!("coverage: missing {missing:?}");
    }
    if !forbidden.is_empty() {
        eprintln!("coverage: forbidden classes occur {forbidden:?}");
    }
    if !missing.is_empty() || !forbidden.is_empty() {
        std::process::exit(1);
    }
    let (check, write) = (path("--check"), path("--out"));
    let (reads_check, reads_write) = (path("--reads-check"), path("--reads-out"));
    let quiet = check.is_some() || write.is_some() || reads_check.is_some() || reads_write.is_some();
    let mut ok = check_or_write(&out, check, write, "--out");
    if reads_check.is_some() || reads_write.is_some() {
        let rd = reads(&all);
        for (k, v) in &rd.hits {
            eprintln!("reads {k:26} {v}");
        }
        eprintln!("reads: {} states, {} requests", rd.states, rd.queries);
        if !rd.bad.is_empty() {
            for b in rd.bad.iter().take(10) {
                eprintln!("preview and submit disagree: {b}");
            }
            eprintln!("reads: {} preview/submit disagreements", rd.bad.len());
            std::process::exit(1);
        }
        let missing: Vec<&str> = REQUIRED_READS.iter().copied().filter(|k| !rd.hits.contains_key(*k)).collect();
        if !missing.is_empty() {
            eprintln!("reads coverage: missing {missing:?}");
            std::process::exit(1);
        }
        ok &= check_or_write(&rd.out, reads_check, reads_write, "--reads-out");
    }
    if !quiet {
        print!("{out}");
    }
    if !ok {
        std::process::exit(1);
    }
}
