//! Plain mode: REF-1..3 on every step of every conformance trace
//! (`conformance/traces/core.txt`, the real core's traces that the TLA+
//! (REF-5), Lean (REF-5L) and Prolog (MC-2) layers replay). **A test, not a
//! proof.**
//!
//! Each trace is rebuilt as a `Pool` from its `config` and `init` lines and
//! replayed on the pinned core call by call (state terms `s/7`: the records'
//! ids are `FirstId + j`, and `FirstId` and the completed units are
//! compared too). A trace whose config the core refuses has
//! `init_error(invalid_config)` instead of `init` and no calls (CFG-2, trace
//! 210): `Pool::new` must refuse it with that error. Every call's result and
//! post-state must be the recorded ones (so the replay is the trace), and
//! every call must refine the TLA+ transcription (`tla::assert_refines` with
//! the core's own limits as the image's capacity, `tla::TB`): the core's
//! result is the error operator's, an enabled action's post-state is
//! `alpha(post)`, every other call stutters. The test fails if the file is
//! missing or smaller than expected, or if some value of the error
//! operators ("Ok", "Noop", every error the traces contain) never occurs.
//!
//! The format is documented at the top of `conformance/src/main.rs`.

use crate::spec::{self, Op, Res};
use crate::tla::{self, TErr};
use pool_core::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
enum Term {
    Int(i128),
    Atom(String),
    List(Vec<Term>),
    App(String, Vec<Term>),
}

struct Parser<'s> {
    b: &'s [u8],
    i: usize,
}

impl<'s> Parser<'s> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: u8) {
        assert!(self.eat(c), "trace parse: expected {:?} at {}", c as char, self.i);
    }
    fn args(&mut self, close: u8) -> Vec<Term> {
        let mut v = Vec::new();
        if self.eat(close) {
            return v;
        }
        loop {
            v.push(self.term());
            if self.eat(close) {
                return v;
            }
            self.expect(b',');
        }
    }
    fn term(&mut self) -> Term {
        self.ws();
        let c = self.b[self.i];
        if c == b'[' {
            self.i += 1;
            return Term::List(self.args(b']'));
        }
        if c == b'-' || c.is_ascii_digit() {
            let s = self.i;
            self.i += 1;
            while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
            }
            let t = std::str::from_utf8(&self.b[s..self.i]).unwrap();
            return Term::Int(t.parse().expect("trace parse: integer"));
        }
        assert!(c.is_ascii_lowercase(), "trace parse: unexpected {:?} at {}", c as char, self.i);
        let s = self.i;
        while self.i < self.b.len() && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_') {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.b[s..self.i]).unwrap().to_string();
        if self.i < self.b.len() && self.b[self.i] == b'(' {
            self.i += 1;
            Term::App(name, self.args(b')'))
        } else {
            Term::Atom(name)
        }
    }
}

/// One clause `Head.` of a line.
fn clause(line: &str) -> Term {
    let mut p = Parser { b: line.as_bytes(), i: 0 };
    let t = p.term();
    p.expect(b'.');
    p.ws();
    assert!(p.i == p.b.len(), "trace parse: trailing text in {line:?}");
    t
}

fn int(t: &Term) -> i128 {
    match t {
        Term::Int(x) => *x,
        _ => panic!("trace: integer expected, got {t:?}"),
    }
}
fn i64_(t: &Term) -> i64 {
    i64::try_from(int(t)).expect("trace: i64")
}
fn u32_(t: &Term) -> u32 {
    u32::try_from(int(t)).expect("trace: u32")
}
fn u64_(t: &Term) -> u64 {
    u64::try_from(int(t)).expect("trace: u64")
}
fn atom(t: &Term) -> &str {
    match t {
        Term::Atom(a) => a,
        _ => panic!("trace: atom expected, got {t:?}"),
    }
}
fn boolean(t: &Term) -> bool {
    match atom(t) {
        "true" => true,
        "false" => false,
        a => panic!("trace: boolean expected, got {a}"),
    }
}
fn list(t: &Term) -> &[Term] {
    match t {
        Term::List(v) => v,
        _ => panic!("trace: list expected, got {t:?}"),
    }
}
fn app<'t>(t: &'t Term, name: &str, n: usize) -> &'t [Term] {
    match t {
        Term::App(f, a) if f == name && a.len() == n => a,
        _ => panic!("trace: {name}/{n} expected, got {t:?}"),
    }
}

/// The trace accounts (`ACCOUNTS` of the generator).
fn acct(t: &Term) -> &'static str {
    const ACCOUNTS: [&str; 6] = ["o", "a1", "a2", "x1", "x2", "z"];
    let a = atom(t);
    ACCOUNTS.iter().copied().find(|x| *x == a).unwrap_or_else(|| panic!("trace: unknown account {a}"))
}

fn config(t: &Term) -> Config {
    let a = app(t, "config", 5);
    let metrics = list(&a[2])
        .iter()
        .map(|m| {
            let m = app(m, "m", 3);
            Metric {
                name: "m".into(),
                unit: "u".into(),
                min: i64_(&m[0]),
                max: i64_(&m[1]),
                initial: i64_(&m[2]),
            }
        })
        .collect();
    let terms = list(&a[3])
        .iter()
        .map(|x| {
            let x = app(x, "t", 4);
            Term_ {
                metric: u32_(&x[0]),
                target: i64_(&x[1]),
                weight: u32_(&x[2]),
                shape: match atom(&x[3]) {
                    "target" => Shape::Target,
                    "excess" => Shape::Excess,
                    "deficit" => Shape::Deficit,
                    s => panic!("trace: shape {s}"),
                },
            }
        })
        .collect();
    let recipes = list(&a[4])
        .iter()
        .map(|r| {
            let r = app(r, "r", 6);
            Recipe {
                name: "r".into(),
                actor: acct(&r[0]).into(),
                executor: acct(&r[1]).into(),
                reserve: list(&r[2]).iter().map(i64_).collect(),
                complete: list(&r[3]).iter().map(i64_).collect(),
                max_units: u32_(&r[4]),
                requires: list(&r[5])
                    .iter()
                    .map(|q| {
                        let q = app(q, "p", 2);
                        (u32_(&q[0]), u32_(&q[1]))
                    })
                    .collect(),
            }
        })
        .collect();
    Config {
        name: "p".into(),
        owner: acct(&a[0]).into(),
        metrics,
        terms,
        recipes,
        max_age_seconds: u32_(&a[1]),
    }
}

use pool_core::Term as Term_;

fn status(t: &Term) -> Status {
    match atom(t) {
        "reserved" => Status::Reserved,
        "dispatched" => Status::Dispatched,
        "active" => Status::Active,
        "unknown" => Status::Unknown,
        "completed" => Status::Completed,
        "cancelled" => Status::Cancelled,
        s => panic!("trace: status {s}"),
    }
}

/// The pool state `s(Values, Version, ObservedAt, Paused, Records, FirstId,
/// DoneUnits)` matches. Record `j` of `Records` has id `FirstId + j`.
fn state_is(p: &Pool, t: &Term) -> bool {
    let a = app(t, "s", 7);
    let values: Vec<i64> = list(&a[0]).iter().map(i64_).collect();
    let first_id = u32_(&a[5]);
    let done: Vec<u32> = list(&a[6]).iter().map(u32_).collect();
    let records: Vec<Commitment> = list(&a[4])
        .iter()
        .enumerate()
        .map(|(j, c)| {
            let c = app(c, "c", 4);
            Commitment {
                id: first_id.checked_add(j as u32).expect("trace: record id"),
                recipe: u32_(&c[0]),
                units: u32_(&c[1]),
                expires_at: u64_(&c[2]),
                status: status(&c[3]),
            }
        })
        .collect();
    p.values == values
        && p.version == u32_(&a[1])
        && p.observed_at == u64_(&a[2])
        && p.paused == boolean(&a[3])
        && p.records == records
        && p.first_id == first_id
        && p.completed_units == done
}

fn op(t: &Term) -> Op {
    match t {
        Term::App(f, a) => match (f.as_str(), a.len()) {
            ("submit", 6) => Op::Submit {
                caller: acct(&a[0]),
                req: Request {
                    recipe: u32_(&a[1]),
                    units: u32_(&a[2]),
                    version: u32_(&a[3]),
                    expires_at: u64_(&a[4]),
                },
                now: u64_(&a[5]),
            },
            ("dispatch", 3) => Op::Dispatch {
                caller: acct(&a[0]),
                id: u32_(&a[1]),
                now: u64_(&a[2]),
            },
            ("ack", 2) => Op::Ack {
                id: u32_(&a[0]),
                accepted: boolean(&a[1]),
            },
            ("cancel", 3) => Op::Cancel {
                caller: acct(&a[0]),
                id: u32_(&a[1]),
                now: u64_(&a[2]),
            },
            ("settle", 3) => Op::Settle {
                caller: acct(&a[0]),
                id: u32_(&a[1]),
                completed: boolean(&a[2]),
            },
            ("pause", 2) => Op::Pause {
                caller: acct(&a[0]),
                paused: boolean(&a[1]),
            },
            ("observe", 3) => Op::Observe {
                caller: acct(&a[0]),
                vals: list(&a[1]).iter().map(i64_).collect(),
                now: u64_(&a[2]),
            },
            _ => panic!("trace: unknown call {t:?}"),
        },
        _ => panic!("trace: call expected, got {t:?}"),
    }
}

fn error(name: &str) -> Error {
    match name {
        "invalid_config" => Error::InvalidConfig,
        "unauthorized" => Error::Unauthorized,
        "paused" => Error::Paused,
        "stale" => Error::Stale,
        "expired" => Error::Expired,
        "snapshot_expired" => Error::SnapshotExpired,
        "invalid_recipe" => Error::InvalidRecipe,
        "invalid_units" => Error::InvalidUnits,
        "prerequisite" => Error::Prerequisite,
        "full" => Error::Full,
        "arithmetic" => Error::Arithmetic,
        "bounds" => Error::Bounds,
        "energy" => Error::Energy,
        "invalid_state" => Error::InvalidState,
        "not_found" => Error::NotFound,
        "pending" => Error::Pending,
        e => panic!("trace: error {e}"),
    }
}

fn result(t: &Term) -> Res {
    match t {
        Term::Atom(a) if a == "ok" => Ok(None),
        Term::App(f, a) if f == "ok" && a.len() == 1 => Ok(Some(u32_(&a[0]))),
        Term::App(f, a) if f == "err" && a.len() == 1 => Err(error(atom(&a[0]))),
        _ => panic!("trace: result expected, got {t:?}"),
    }
}

/// The traces file (the conformance crate's checked-in output).
fn traces_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance/traces/core.txt")
}

/// REF-1..3 on every call of every conformance trace, with the TLA+ value
/// of each call counted.
#[test]
fn h_ref_traces() {
    let path = traces_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("h_ref_traces: cannot read {}: {e}", path.display()));
    let mut seen: BTreeMap<TErr, u64> = BTreeMap::new();
    let (mut traces, mut calls) = (0u32, 0u64);
    // init_error traces, observe Ok refreshes and re-bases, prunes of a
    // counted record (folded into the ghost), the largest first_id.
    let (mut refused, mut refreshes, mut rebases, mut folds, mut pruned_max) = (0u32, 0u64, 0u64, 0u64, 0u32);
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    while let Some(l) = lines.next() {
        let n = int(&app(&clause(l), "trace", 1)[0]);
        let cfg = config(&clause(lines.next().expect("config line")));
        let init = clause(lines.next().expect("init line"));
        if matches!(&init, Term::App(f, _) if f == "init_error") {
            // PROP: CFG-2
            let want = error(atom(&app(&init, "init_error", 1)[0]));
            assert!(
                Pool::new(cfg, 0).err() == Some(want),
                "trace {n}: Pool::new refuses the config with the recorded error"
            );
            assert!(
                clause(lines.next().expect("end line")) == Term::Atom("end".into()),
                "trace {n}: no calls after init_error"
            );
            refused += 1;
            traces += 1;
            continue;
        }
        let a = app(&init, "init", 2);
        let mut p = Pool::new(cfg, u64_(&a[0])).unwrap_or_else(|e| panic!("trace {n}: Pool::new failed: {e:?}"));
        assert!(state_is(&p, &a[1]), "trace {n}: the initial state is the recorded one");
        // The implementation's ghost (plan 2.2, `tla::ghost_after`): the
        // values at the last re-basing observe (or new), with every counted
        // pruned record folded in, and `k0`, the first id not counted then.
        // An observe Ok that restates the current values with some record
        // open (the refresh) keeps both; any other observe Ok re-bases them
        // (`k0 = first_id + len`); a submit that prunes a counted record
        // folds its contribution into `base` and moves `k0` past it.
        let mut base = p.values.clone();
        let mut k0 = 0u64;
        let mut step = 0;
        loop {
            let c = clause(lines.next().expect("call or end line"));
            if c == Term::Atom("end".into()) {
                break;
            }
            step += 1;
            let a = app(&c, "call", 3);
            let o = op(&a[0]);
            let pre = p.clone();
            let res = spec::apply(&mut p, &o);
            assert!(res == result(&a[1]), "trace {n} call {step}: the core's result is the recorded one");
            assert!(state_is(&p, &a[2]), "trace {n} call {step}: the core's post-state is the recorded one");
            // PROP: REF-1 REF-2 REF-3
            let w = tla::assert_refines::<32, 64, 32, 32, 256>(&pre, &base, k0, &o, &res, &p);
            *seen.entry(w).or_default() += 1;
            let g = tla::ghost_after::<32>(&pre, &base, k0, &o, &res, &p);
            if let (Op::Observe { vals, .. }, Ok(_)) = (&o, &res) {
                let refresh = *vals == pre.values
                    && pre.records.iter().any(|c| !matches!(c.status, Status::Completed | Status::Cancelled));
                if refresh {
                    refreshes += 1;
                } else {
                    rebases += 1;
                }
            }
            if p.first_id != pre.first_id && g.k0 != k0 {
                folds += 1;
            }
            base = g.base[..g.nb].to_vec();
            k0 = g.k0;
            pruned_max = pruned_max.max(p.first_id);
            calls += 1;
        }
        traces += 1;
    }
    eprintln!(
        "h_ref_traces: {traces} traces ({refused} init_error), {calls} calls, observe Ok {refreshes} refreshes \
         {rebases} re-bases, {folds} prunes folded into the ghost, largest first_id {pruned_max}, TLA+ values {seen:?}"
    );
    // Vacuity: the whole file was replayed (216 traces, 8,865 calls), with
    // a refused config, both ghost branches of observe and a prune of a
    // counted record, and every value of the error operators that the
    // traces contain occurred.
    assert!(traces >= 216 && calls >= 8_800, "h_ref_traces: fewer traces or calls than the checked-in file has");
    assert!(refused >= 1, "h_ref_traces: no init_error trace");
    assert!(refreshes >= 1 && rebases >= 1, "h_ref_traces: an observe ghost branch never occurred");
    assert!(pruned_max >= 1 && folds >= 1, "h_ref_traces: no trace prunes a counted record");
    for w in [
        TErr::Ok,
        TErr::Noop,
        TErr::Paused,
        TErr::Stale,
        TErr::Expired,
        TErr::SnapshotExpired,
        TErr::Full,
        TErr::InvalidRecipe,
        TErr::Unauthorized,
        TErr::InvalidUnits,
        TErr::Prerequisite,
        TErr::Arithmetic,
        TErr::Bounds,
        TErr::Energy,
        TErr::NotFound,
        TErr::InvalidState,
        TErr::Pending,
    ] {
        assert!(seen.get(&w).copied().unwrap_or(0) > 0, "h_ref_traces: {w:?} never occurred");
    }
}
