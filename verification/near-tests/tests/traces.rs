//! Replays every conformance trace (projects/shared-pool/conformance/
//! traces/core.txt, generated from the real pool-core of v1.1.0; the link
//! shared with the Prolog, TLA+ and Lean layers) through the shell, on the
//! host path and on the documented wasm build. Each core call becomes the
//! shell call that performs it (`ack(Id, true)` is the private callback
//! `on_result` with "active", `ack(Id, false)` the callback with "unknown" or
//! a failed promise); the shell's result must be the trace's result and its
//! stored state the trace's state after every call.
//!
//! The state term is `s/7`: values, version, observed_at, paused, the
//! retained records `c(recipe, units, expires_at, status)` (record k has id
//! `first_id + k`), first_id, and the lifetime completed units per recipe.
//! A trace whose config `Pool::new` refuses is `init_error(invalid_config)`
//! with no calls: the shell's `new` must fail `InvalidConfig` (F-4, trace
//! 210). An `ack` of a missing (never assigned or pruned) record is
//! `err(not_found)`; the shell's callback logs it as an event with error
//! "NotFound" and succeeds (v1.1.0, docs/api.md:20).
//!
//! Trace 0 is the F-1 configuration, whose hedge v1.1.0 refuses with Bounds.
//! F-1 is a finding, not a catalogue ID: it is the counterexample to LIVE-2,
//! so trace 0 is tagged LIVE-2. The replay also fixes the callback mapping
//! of acks (SH-3), rejected submits (SH-4) and error panics (SH-5).
//!
// PROP: SH-8 SH-3 SH-4 SH-5 LIVE-2 LIVE-ALL

mod common;
use common::*;
use near_sdk::serde_json::{json, Value};
use pool_core::{Config, Metric, Pool, Recipe, Request, Shape, Status, Term};

// ---------------------------------------------------------------------------
// A parser for the trace terms (integers, atoms, compounds, lists)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum T {
    Int(i64),
    Atom(String),
    Comp(String, Vec<T>),
    List(Vec<T>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> u8 {
        self.s[self.i]
    }
    fn eat(&mut self, c: u8) {
        assert_eq!(self.peek() as char, c as char, "at {} in {}", self.i, String::from_utf8_lossy(self.s));
        self.i += 1;
    }
    fn term(&mut self) -> T {
        let c = self.peek();
        if c == b'[' {
            self.i += 1;
            let mut xs = vec![];
            if self.peek() != b']' {
                loop {
                    xs.push(self.term());
                    if self.peek() == b',' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
            }
            self.eat(b']');
            T::List(xs)
        } else if c == b'-' || c.is_ascii_digit() {
            let st = self.i;
            self.i += 1;
            while self.i < self.s.len() && self.peek().is_ascii_digit() {
                self.i += 1;
            }
            T::Int(std::str::from_utf8(&self.s[st..self.i]).unwrap().parse().unwrap())
        } else {
            let st = self.i;
            while self.i < self.s.len() && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[st..self.i]).unwrap().to_string();
            assert!(!name.is_empty(), "bad term at {}", self.i);
            if self.i < self.s.len() && self.peek() == b'(' {
                self.i += 1;
                let mut args = vec![];
                loop {
                    args.push(self.term());
                    if self.peek() == b',' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                self.eat(b')');
                T::Comp(name, args)
            } else {
                T::Atom(name)
            }
        }
    }
}

fn parse(line: &str) -> T {
    let line = line.trim().strip_suffix('.').expect("term ends with '.'");
    let mut p = P { s: line.as_bytes(), i: 0 };
    let t = p.term();
    assert_eq!(p.i, line.len(), "trailing input in {line}");
    t
}

impl T {
    fn int(&self) -> i64 {
        match self {
            T::Int(v) => *v,
            t => panic!("not an int: {t:?}"),
        }
    }
    fn atom(&self) -> &str {
        match self {
            T::Atom(a) => a,
            t => panic!("not an atom: {t:?}"),
        }
    }
    fn list(&self) -> &[T] {
        match self {
            T::List(xs) => xs,
            t => panic!("not a list: {t:?}"),
        }
    }
    fn comp(&self, name: &str) -> &[T] {
        match self {
            T::Comp(n, a) if n == name => a,
            t => panic!("not {name}(..): {t:?}"),
        }
    }
    fn boolean(&self) -> bool {
        match self.atom() {
            "true" => true,
            "false" => false,
            a => panic!("not a boolean: {a}"),
        }
    }
    fn ints(&self) -> Vec<i64> {
        self.list().iter().map(T::int).collect()
    }
}

// ---------------------------------------------------------------------------
// Trace terms as shell inputs
// ---------------------------------------------------------------------------

fn config_of(n: usize, t: &T) -> Config {
    let a = t.comp("config");
    Config {
        name: format!("trace{n}"),
        owner: atom(a[0].atom()).into(),
        max_age_seconds: a[1].int() as u32,
        metrics: a[2]
            .list()
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let m = m.comp("m");
                Metric { name: format!("m{i}"), unit: "u".into(), min: m[0].int(), max: m[1].int(), initial: m[2].int() }
            })
            .collect(),
        terms: a[3]
            .list()
            .iter()
            .map(|t| {
                let t = t.comp("t");
                Term {
                    metric: t[0].int() as u32,
                    target: t[1].int(),
                    weight: t[2].int() as u32,
                    shape: match t[3].atom() {
                        "target" => Shape::Target,
                        "excess" => Shape::Excess,
                        "deficit" => Shape::Deficit,
                        s => panic!("shape {s}"),
                    },
                }
            })
            .collect(),
        recipes: a[4]
            .list()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let r = r.comp("r");
                Recipe {
                    name: format!("r{i}"),
                    actor: atom(r[0].atom()).into(),
                    executor: atom(r[1].atom()).into(),
                    reserve: r[2].ints(),
                    complete: r[3].ints(),
                    max_units: r[4].int() as u32,
                    requires: r[5]
                        .list()
                        .iter()
                        .map(|p| {
                            let p = p.comp("p");
                            (p[0].int() as u32, p[1].int() as u32)
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

/// The trace's state term for a pool.
fn state_term(p: &Pool) -> T {
    let st = |s: &Status| {
        match s {
            Status::Reserved => "reserved",
            Status::Dispatched => "dispatched",
            Status::Active => "active",
            Status::Unknown => "unknown",
            Status::Completed => "completed",
            Status::Cancelled => "cancelled",
        }
        .to_string()
    };
    T::Comp(
        "s".into(),
        vec![
            T::List(p.values.iter().map(|v| T::Int(*v)).collect()),
            T::Int(p.version as i64),
            T::Int(p.observed_at as i64),
            T::Atom(if p.paused { "true" } else { "false" }.into()),
            T::List(
                p.records
                    .iter()
                    .map(|c| {
                        T::Comp(
                            "c".into(),
                            vec![T::Int(c.recipe as i64), T::Int(c.units as i64), T::Int(c.expires_at as i64), T::Atom(st(&c.status))],
                        )
                    })
                    .collect(),
            ),
            T::Int(p.first_id as i64),
            T::List(p.completed_units.iter().map(|u| T::Int(*u as i64)).collect()),
        ],
    )
}

/// snake_case error atom -> the shell's error string (Debug name).
fn camel(e: &str) -> String {
    e.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect()
}

/// The shell call for one trace op, at the op's time (or the last time).
fn shell_op(op: &T, now: &mut u64, k: usize) -> (Ctx, Op) {
    let ts = |now: u64| now * 1_000_000_000 + (k as u64 % 7) * 142_857_142;
    let (name, a) = match op {
        T::Comp(n, a) => (n.as_str(), a),
        t => panic!("op {t:?}"),
    };
    match name {
        "submit" => {
            *now = a[5].int() as u64;
            let req = Request { recipe: a[1].int() as u32, units: a[2].int() as u32, version: a[3].int() as u32, expires_at: a[4].int() as u64 };
            (Ctx::new(atom(a[0].atom()), atom(a[0].atom()), ts(*now)), Op::Submit { req })
        }
        "dispatch" | "cancel" => {
            *now = a[2].int() as u64;
            let id = a[1].int() as u32;
            let op = if name == "dispatch" { Op::Dispatch { id } } else { Op::Cancel { id } };
            (Ctx::new(atom(a[0].atom()), atom(a[0].atom()), ts(*now)), op)
        }
        "settle" => (
            Ctx::new(atom(a[0].atom()), atom(a[0].atom()), ts(*now)),
            Op::Settle { id: a[1].int() as u32, completed: a[2].boolean() },
        ),
        "pause" => (Ctx::new(atom(a[0].atom()), atom(a[0].atom()), ts(*now)), Op::Pause { paused: a[1].boolean() }),
        "observe" => {
            *now = a[2].int() as u64;
            (Ctx::new(atom(a[0].atom()), atom(a[0].atom()), ts(*now)), Op::Observe { values: a[1].ints() })
        }
        "ack" => {
            let id = a[0].int() as u32;
            // The executor's answer: "active" acknowledges; "unknown" or a
            // failed execute leave the record Unknown.
            let result = if a[1].boolean() {
                Some("active")
            } else if k % 2 == 0 {
                Some("unknown")
            } else {
                None
            };
            (Ctx::new(POOL, Z, ts(*now)), Op::cb(id, result))
        }
        n => panic!("unknown op {n}"),
    }
}

/// The shell's outcome agrees with the trace's result term.
fn agrees(op: &Op, r: &Ran, result: &T) -> bool {
    match (op, result) {
        (Op::Submit { .. }, T::Comp(n, a)) if n == "ok" => r.result.as_ref().map_or(false, |v| v["id"] == json!(a[0].int())),
        (Op::Submit { .. }, T::Comp(n, a)) if n == "err" => {
            let e = camel(a[0].atom());
            match &r.result {
                Ok(v) => v["id"].is_null() && v["decision"]["reason"] == json!(e),
                Err(f) => f == &Fail::Guest(e),
            }
        }
        (Op::OnResult { .. }, T::Atom(ok)) if ok == "ok" => r.result.is_ok() && r.event()["error"] == Value::Null,
        // The callback never fails: the core's error (NotFound for a missing
        // record included) is the event's.
        (Op::OnResult { .. }, T::Comp(n, a)) if n == "err" => r.result.is_ok() && r.event()["error"] == json!(camel(a[0].atom())),
        (_, T::Atom(ok)) if ok == "ok" => r.result.is_ok(),
        (_, T::Comp(n, a)) if n == "err" => r.result.as_ref().err() == Some(&Fail::Guest(camel(a[0].atom()))),
        _ => false,
    }
}

struct Trace {
    n: usize,
    config: Config,
    /// `init(T0, S0)`, or the error of `init_error(E)`.
    init: Result<(u64, T), String>,
    calls: Vec<(T, T, T)>,
}

fn load() -> Vec<Trace> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../conformance/traces/core.txt");
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut out = vec![];
    let mut cur: Option<Trace> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('%')) {
        let t = parse(line);
        match &t {
            T::Comp(n, a) if n == "trace" => {
                assert!(cur.is_none());
                cur = Some(Trace { n: a[0].int() as usize, config: Config { name: String::new(), owner: String::new(), metrics: vec![], terms: vec![], recipes: vec![], max_age_seconds: 0 }, init: Err("none".into()), calls: vec![] });
            }
            T::Comp(n, _) if n == "config" => {
                let c = cur.as_mut().unwrap();
                c.config = config_of(c.n, &t);
            }
            T::Comp(n, a) if n == "init" => cur.as_mut().unwrap().init = Ok((a[0].int() as u64, a[1].clone())),
            T::Comp(n, a) if n == "init_error" => cur.as_mut().unwrap().init = Err(camel(a[0].atom())),
            T::Comp(n, a) if n == "call" => cur.as_mut().unwrap().calls.push((a[0].clone(), a[1].clone(), a[2].clone())),
            T::Atom(e) if e == "end" => {
                let tr = cur.take().unwrap();
                assert!(tr.init.is_ok() || tr.calls.is_empty(), "trace {}: calls after init_error", tr.n);
                assert_ne!(tr.init, Err("none".into()), "trace {}: no init", tr.n);
                out.push(tr)
            }
            t => panic!("unexpected line {t:?}"),
        }
    }
    assert!(cur.is_none());
    out
}

/// The shell's `new(config)` on `path` fails with `e`, writing nothing.
fn new_fails(path: Path, config: &Config, e: &str, what: &str) {
    let ctx = Ctx::by(POOL, 100);
    match path {
        Path::Host => {
            install(&ctx, Storage::new(), vec![], &[]);
            let c = config.clone();
            let r = catch_quiet(move || drop(pool_contract::Contract::new(c)));
            let f = classify(r.expect_err(what));
            assert_eq!(f, Fail::Guest(e.into()), "{what}");
            assert!(take_storage().is_empty(), "{what}: new wrote");
        }
        Path::Wasm(b) => {
            let input = near_sdk::serde_json::to_vec(&json!({ "config": config })).unwrap();
            let out = wasm_call(wasm(b, "pool_contract"), &ctx, Storage::new(), vec![], "new", &input);
            assert_wasm_fail(b, &out, &Fail::Guest(e.into()), &Storage::new(), what);
        }
    }
}

fn replay(path: Path, traces: &[Trace]) -> usize {
    let mut calls = 0;
    for tr in traces {
        let (t0, s0) = match &tr.init {
            Ok(i) => i,
            Err(e) => {
                assert!(Pool::new(tr.config.clone(), 100).is_err());
                new_fails(path, &tr.config, e, &format!("{path:?} trace {} init_error", tr.n));
                continue;
            }
        };
        let mut s = Sim::new(path, tr.config.clone(), *t0);
        assert_eq!(state_term(&pool_of(&s.storage)), *s0, "{path:?} trace {} init", tr.n);
        let mut now = *t0;
        for (k, (op, result, after)) in tr.calls.iter().enumerate() {
            let (ctx, sop) = shell_op(op, &mut now, k);
            let r = s.run(&ctx, sop.clone());
            assert!(
                agrees(&sop, &r, result),
                "{path:?} trace {} call {k}: {op:?} -> trace says {result:?}, shell gave {:?} {:?}",
                tr.n,
                r.result,
                r.events
            );
            assert_eq!(state_term(&pool_of(&s.storage)), *after, "{path:?} trace {} call {k}: {op:?} state", tr.n);
            calls += 1;
        }
    }
    calls
}

// PROP: SH-8 SH-3 SH-4 SH-5 LIVE-2 LIVE-ALL
#[test]
fn conformance_traces_through_the_shell() {
    let traces = load();
    assert_eq!(traces.len(), 216, "traces/core.txt: 216 traces");
    assert_eq!(traces[0].config, wedge_config_as_trace(), "trace 0 is the F-1 configuration");
    let refused: Vec<usize> = traces.iter().filter(|t| t.init.is_err()).map(|t| t.n).collect();
    assert_eq!(refused, vec![210], "trace 210 (F-4, actor == executor) is the one init_error");
    let traces = std::sync::Arc::new(traces);
    let hs: Vec<_> = paths()
        .into_iter()
        .map(|path| {
            let traces = traces.clone();
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || {
                    quiet_expected_panics();
                    (path, replay(path, &traces))
                })
                .unwrap()
        })
        .collect();
    let total: usize = traces.iter().map(|t| t.calls.len()).sum();
    for h in hs {
        let (path, n) = h.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
        println!("{path:?}: {n} calls of {} traces agree", traces.len());
        assert_eq!(n, total);
    }
}

/// `wedge_config()` with the trace's generated names.
fn wedge_config_as_trace() -> Config {
    let mut c = wedge_config();
    c.name = "trace0".into();
    for (i, m) in c.metrics.iter_mut().enumerate() {
        m.name = format!("m{i}");
    }
    for (i, r) in c.recipes.iter_mut().enumerate() {
        r.name = format!("r{i}");
    }
    c
}
