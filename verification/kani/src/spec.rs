//! Executable specification of `pool-core` (shared-pool brief, "Exact core
//! semantics"; property IDs from its catalogue).
//!
//! Upstream: difflabai/shared-pool-controls `crates/core/src/lib.rs`
//! Pin:      57b3f02d18e85149db3c1035431368c4e07a4d8a (v1.1.0, merge commit of PR #4; tree = PR head 6fcbd56)
//! sha256:   c3bb825b91f7c277dee8651beafd45955f7cf8935c858f928b6a1b62d607ba48
//! Covers:   Config::validate, Pool::new, energy_terms, energy, submit
//!           (candidate, envelope, check_bounds, add, the prune of the
//!           window), record (index), dispatch, acknowledge, cancel
//!           (release), settle, set_paused, observe (refresh), preview,
//!           best_quantity; the fields first_id and completed_units.
//!
//! Everything here is written from the brief, not from the core: the guard
//! order, the exact results and post-states, and the invariants. The core is
//! only ever *called* (in `apply`) and compared against it. Arithmetic is
//! exact (`Z`, `u128`, see [`Z`]): no checked, saturating or wrapping operation of the
//! code is reused, so the overflow behaviour of the code is checked against
//! the mathematical value, not against itself.
//!
//! Simplifications:
//! - At most [`M`] metrics (the harness bound is 2): value vectors are fixed
//!   arrays `[Z; M]` with the length taken from the configuration.
//! - `expected` needs no invariant: it is total on well-formed states (every
//!   record's recipe in range, values of length `n` and bounded), which every
//!   generated state is. Energy errors of the code are not modelled: on
//!   bounded values energy cannot fail (NUM-1, `h_energy`), and every
//!   energy evaluation of the code is on bounded values.

use pool_core::*;

/// Accounts: short distinct strings. 4 values realise every equality pattern
/// among the at most 3 role holders one step compares (owner, the touched
/// recipe's actor and executor) and the caller. They stand for the brief's
/// `o, a1, x1, z`; all four are 2 bytes long, because a symbolic choice among
/// strings of different lengths is a `String` of symbolic length, which CBMC
/// pays for at every clone and comparison (RESULTS.md, cost ladder). The core
/// only compares accounts for equality (and `validate` checks `1..=128`
/// bytes), so the names themselves carry no meaning.
pub const ACCTS: [&str; 4] = ["o_", "a1", "x1", "z_"];

/// Most metrics the spec's fixed arrays hold: 2 under Kani (the harness
/// bound, `gen::MAX_M`); 8 in plain mode, so that the regression tests on
/// the core's fixtures (the `risk` fixture has 4 metrics, `gpu` 5) run
/// through the same spec. The generators draw at most `gen::MAX_M` (raw
/// configurations `MAX_M + 1`) either way.
pub const M: usize = if cfg!(kani) { 2 } else { 8 };

/// The spec's integers. `i64` with overflow checks on (Kani checks every
/// arithmetic overflow; plain mode runs with `overflow-checks = true`), so
/// spec arithmetic is exact: a wrap would be a failed check, never a silent
/// value. Every spec quantity fits by a wide margin: `|d·u| ≤ 1e6·100`,
/// envelope and ledger sums `≤ 1e6 + 256·1e8`, energy `≤ 64·1000·(2e6)²`
/// `= 2.56e17` (RESULTS.md, deviation 2: `i128` doubled every multiplier).
pub type Z = i64;
pub type Vals = [Z; M];

pub const MAXV: Z = 1_000_000;
pub const DAY: u128 = 86_400;

/// Most retained records (the window).
pub const MAXREC: usize = 256;

/// Submit, dispatch, set_paused and observe fail Arithmetic from this
/// version on (VER-CAP): `u32::MAX` minus 3 increments per retained record,
/// the reserve that resolves every open record (INV-VERCAP).
pub const VERSION_CAP: u32 = u32::MAX - 3 * MAXREC as u32;

/// Slots of the ghost's lifetime Completed units ([`Ghost::done`]): the
/// generators' recipes (`gen::MAX_R`), plus one for the raw configurations
/// of `h_init`; 8 in plain mode (the fixtures have up to 6 recipes).
pub const GR: usize = if cfg!(kani) { 3 } else { 8 };

/// `d·u` for a delta `|d| ≤ 1e6` and units `u ≤ 100` (validated configs and
/// guarded units: every call site). The preconditions are asserted, so the
/// product (`|d·u| ≤ 1e8 < 2^31`) is exact in 32 bits; this keeps the
/// multiplier at a quarter of the 64-bit one with its overflow check.
pub fn mul_du(d: i64, u: u32) -> Z {
    assert!(-1_000_000 <= d && d <= 1_000_000 && u <= 100, "spec: |d| <= 1e6, u <= 100");
    (d as i32).wrapping_mul(u as i32) as Z
}

pub fn bounded(x: Z) -> bool {
    -MAXV <= x && x <= MAXV
}

pub fn short(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
}

/// Longest string the step harnesses compare (accounts are 2 bytes, other
/// names 1; `gen`).
pub const STR_MAX: usize = 2;

/// String equality. Plain mode: `==`. Under Kani, `==` on `str` is a
/// `memcmp` whose length CBMC cannot fold, and every byte it reads is a case
/// split over every string object of the state (17k clauses a byte; half the
/// clauses of a step harness, RESULTS.md cost ladder). So the lengths are
/// compared first and then at most `STR_MAX` bytes, which is the same
/// function on strings of at most `STR_MAX` bytes; a longer string fails
/// the harness on the assertion, it is never compared partially.
pub fn eq_str(a: &str, b: &str) -> bool {
    eq_text::<STR_MAX>(a, b)
}

/// [`eq_str`] for strings of at most `K` bytes under Kani. (`preview`'s
/// strings use [`Text`] and [`is_dec`], which read the heap string once.)
pub fn eq_text<const K: usize>(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    #[cfg(kani)]
    {
        kani::assert(a.len() <= K, "harness bound: strings within the compared length");
        let mut eq = true;
        for i in 0..K {
            if i < a.len() && a[i] != b[i] {
                eq = false;
            }
        }
        eq
    }
    #[cfg(not(kani))]
    {
        a == b
    }
}

/// `s == x.to_string()`: `s` is the canonical decimal numeral of `x`
/// (1 to 20 digits, no leading zero unless it is "0") and its value is `x`.
/// Under Kani `s` is parsed (multiplications by 10: shifts and adds) instead
/// of formatting `x` (a 64-bit division per digit pair); the canonical
/// numeral of a number is unique, so the two are equal.
pub fn is_dec(s: &str, x: u64) -> bool {
    #[cfg(kani)]
    {
        parse_dec(s, x)
    }
    #[cfg(not(kani))]
    {
        let r = s == x.to_string();
        assert!(parse_dec(s, x) == r, "spec: is_dec agrees with to_string");
        r
    }
}

fn parse_dec(s: &str, x: u64) -> bool {
    let t = Text::<24>::of(s);
    if t.len == 0 || t.len > 20 || (t.len > 1 && t.b[0] == b'0') {
        return false;
    }
    let mut digits = true;
    let mut n: u128 = 0;
    for i in 0..20 {
        if i < t.len {
            let d = t.b[i].wrapping_sub(b'0');
            if d > 9 {
                digits = false;
            }
            n = n * 10 + d as u128;
        }
    }
    digits && n == x as u128
}

/// A string of at most `K` bytes copied into a local array (under Kani each
/// byte of a heap string is read once, not once per comparison: a read is a
/// case split over every string object the pointer may refer to).
pub struct Text<const K: usize> {
    len: usize,
    b: [u8; K],
}

impl<const K: usize> Text<K> {
    pub fn of(s: &str) -> Self {
        let s = s.as_bytes();
        assert!(s.len() <= K, "harness bound: strings within the compared length");
        let mut b = [0u8; K];
        for i in 0..K {
            if i < s.len() {
                b[i] = s[i];
            }
        }
        Text { len: s.len(), b }
    }

    /// Equality with `t` (any length).
    pub fn eq(&self, t: &str) -> bool {
        let t = t.as_bytes();
        if t.len() != self.len {
            return false;
        }
        let mut eq = true;
        for i in 0..K {
            if i < t.len() && self.b[i] != t[i] {
                eq = false;
            }
        }
        eq
    }
}

/// `a == b` on `i64` vectors, element by element (`==` is a byte-wise
/// `memcmp` over the same imprecise pointers as [`eq_str`]).
pub fn eq_i64s(a: &[i64], b: &[i64]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut eq = true;
    for i in 0..M + 2 {
        if i < a.len() && a[i] != b[i] {
            eq = false;
        }
    }
    assert!(a.len() <= M + 2, "spec: vectors of at most M + 2 values");
    eq
}

/// `a == b` on `u32` vectors (`completed_units`), element by element. Under
/// Kani the vectors have at most [`GR`] elements (asserted); in plain mode
/// any length (fixtures with more recipes).
pub fn eq_u32s(a: &[u32], b: &[u32]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    #[cfg(kani)]
    {
        assert!(a.len() <= GR, "spec: completed_units of at most GR recipes");
        let mut eq = true;
        for i in 0..GR {
            if i < a.len() && a[i] != b[i] {
                eq = false;
            }
        }
        eq
    }
    #[cfg(not(kani))]
    {
        a == b
    }
}

/// `a == b` on [`Vals`] (`==` on `[i64; M]` is a 16-byte `memcmp` loop).
pub fn eq_vals(a: &Vals, b: &Vals) -> bool {
    let mut eq = true;
    for i in 0..M {
        if a[i] != b[i] {
            eq = false;
        }
    }
    eq
}

/// `a == b` on configurations (field by field, with [`eq_str`]).
pub fn same_config(a: &Config, b: &Config) -> bool {
    let mut ok = eq_str(&a.name, &b.name)
        && eq_str(&a.owner, &b.owner)
        && a.max_age_seconds == b.max_age_seconds
        && a.metrics.len() == b.metrics.len()
        && a.terms.len() == b.terms.len()
        && a.recipes.len() == b.recipes.len()
        && a.terms == b.terms;
    if !ok {
        return false;
    }
    for (x, y) in a.metrics.iter().zip(&b.metrics) {
        if !(eq_str(&x.name, &y.name) && eq_str(&x.unit, &y.unit))
            || x.initial != y.initial
            || x.min != y.min
            || x.max != y.max
        {
            ok = false;
        }
    }
    for (x, y) in a.recipes.iter().zip(&b.recipes) {
        if !(eq_str(&x.name, &y.name) && eq_str(&x.actor, &y.actor) && eq_str(&x.executor, &y.executor))
            || !eq_i64s(&x.reserve, &y.reserve)
            || !eq_i64s(&x.complete, &y.complete)
            || x.max_units != y.max_units
            || x.requires != y.requires
        {
            ok = false;
        }
    }
    ok
}

/// `a == b` on everything a step may change. The configuration is compared
/// too in plain mode; under Kani it is left to the frame assertion of
/// [`post_matches`] (the `_post` parts prove that no step changes it), as
/// comparing it costs more than the step itself (RESULTS.md, cost ladder).
pub fn same_state(a: &Pool, b: &Pool) -> bool {
    #[cfg(not(kani))]
    if !same_config(&a.config, &b.config) {
        return false;
    }
    eq_i64s(&a.values, &b.values)
        && a.version == b.version
        && a.observed_at == b.observed_at
        && a.paused == b.paused
        && a.records == b.records
        && a.first_id == b.first_id
        && eq_u32s(&a.completed_units, &b.completed_units)
}

pub fn open(s: &Status) -> bool {
    !matches!(s, Status::Completed | Status::Cancelled)
}

pub fn terminal(s: &Status) -> bool {
    !open(s)
}

fn max0(x: Z) -> Z {
    if x > 0 {
        x
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// One public mutating call. Callers are drawn from [`ACCTS`].
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Submit {
        caller: &'static str,
        req: Request,
        now: u64,
    },
    Dispatch {
        caller: &'static str,
        id: u32,
        now: u64,
    },
    Ack {
        id: u32,
        accepted: bool,
    },
    Cancel {
        caller: &'static str,
        id: u32,
        now: u64,
    },
    Settle {
        caller: &'static str,
        id: u32,
        completed: bool,
    },
    Pause {
        caller: &'static str,
        paused: bool,
    },
    Observe {
        caller: &'static str,
        vals: Vec<i64>,
        now: u64,
    },
}

/// Result of an operation: `Ok(Some(id))` for submit, `Ok(None)` otherwise.
pub type Res = Result<Option<u32>, Error>;

/// Runs `op` through the real core.
pub fn apply(p: &mut Pool, op: &Op) -> Res {
    match op {
        Op::Submit { caller, req, now } => p.submit(caller, req.clone(), *now).map(Some),
        Op::Dispatch { caller, id, now } => p.dispatch(caller, *id, *now).map(|_| None),
        Op::Ack { id, accepted } => p.acknowledge(*id, *accepted).map(|_| None),
        Op::Cancel { caller, id, now } => p.cancel(caller, *id, *now).map(|_| None),
        Op::Settle {
            caller,
            id,
            completed,
        } => p.settle(caller, *id, *completed).map(|_| None),
        Op::Pause { caller, paused } => p.set_paused(caller, *paused).map(|_| None),
        Op::Observe { caller, vals, now } => p
            .observe(caller, crate::gen::copy_vec(vals, M + 1), *now)
            .map(|_| None),
    }
}

// ---------------------------------------------------------------------------
// Notation
// ---------------------------------------------------------------------------

/// `cfg.recipes[k]`, read over concrete slots. Every lookup by a symbolic
/// index goes through this or [`rec`]: an index into a heap buffer is a
/// symbolic byte offset, which CBMC encodes as a mux over every byte of the
/// buffer (RESULTS.md, deviation 3).
pub fn rcp(cfg: &Config, k: u32) -> Option<&Recipe> {
    let mut out = None;
    for (j, r) in cfg.recipes.iter().enumerate() {
        if j as u64 == k as u64 {
            out = Some(r);
        }
    }
    out
}

/// [`rcp`] for an index known to be in range (well-formed states).
pub fn rcp_of(cfg: &Config, k: u32) -> &Recipe {
    match rcp(cfg, k) {
        Some(r) => r,
        None => panic!("spec: recipe index in range"),
    }
}

/// The retained record with id `id` (`Pool::record`): `records[k]` with
/// `first_id + k = id`, read over concrete slots. `None` (NotFound) for a
/// pruned id (`id < first_id`) or one not yet issued. The position is the
/// brief's `id − first_id`; the record's own `id` field is not read (WIN-1
/// says they agree, and the step checks compare it).
pub fn rec(p: &Pool, id: u32) -> Option<&Commitment> {
    let mut out = None;
    for (j, c) in p.records.iter().enumerate() {
        if p.first_id as u64 + j as u64 == id as u64 {
            out = Some(c);
        }
    }
    out
}

/// Is some record open?
pub fn any_open(recs: &[Commitment]) -> bool {
    let mut any = false;
    for c in recs {
        if open(&c.status) {
            any = true;
        }
    }
    any
}

pub fn n(cfg: &Config) -> usize {
    cfg.metrics.len()
}

/// Values of a well-formed state as exact integers.
pub fn vals(cfg: &Config, v: &[i64]) -> Vals {
    let mut out = [0; M];
    let k = n(cfg);
    assert!(k <= M && v.len() == k, "spec: values of length n <= M");
    for i in 0..M {
        if i < k {
            out[i] = v[i] as Z;
        }
    }
    out
}

/// `PV_i(v) = max(0, min_i − v) + max(0, v − max_i)`.
pub fn pv(m: &Metric, v: Z) -> Z {
    max0(m.min as Z - v) + max0(v - m.max as Z)
}

/// Is any metric of `v` outside its `[min, max]`?
pub fn violating(cfg: &Config, v: &Vals) -> bool {
    let mut any = false;
    for i in 0..M {
        if i < n(cfg) && pv(&cfg.metrics[i], v[i]) > 0 {
            any = true;
        }
    }
    any
}

/// `base + d·u`, or `None` (Arithmetic) if some component leaves ±1e6.
pub fn add(cfg: &Config, base: &Vals, d: &[i64], u: u32) -> Option<Vals> {
    let mut out = [0; M];
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) {
            let x = base[i] + mul_du(d[i], u);
            if !bounded(x) {
                ok = false;
            }
            out[i] = x;
        }
    }
    if ok {
        Some(out)
    } else {
        None
    }
}

/// One open record's outcomes on metric `i` (INV-RES): it stays open (0),
/// completes (`d = complete_i·u`) or releases its reservation
/// (`x = −reserve_i·u`). Returns `(min(0, d, x), max(0, d, x))`.
fn outcome(r: &Recipe, u: u32, i: usize) -> (Z, Z) {
    let d = mul_du(r.complete[i], u);
    let x = -mul_du(r.reserve[i], u);
    let lo = if d < x { d } else { x };
    let hi = if d > x { d } else { x };
    (if lo < 0 { lo } else { 0 }, if hi > 0 { hi } else { 0 })
}

/// Envelope over open records (minus `skip`, plus `extra = (recipe, units)`),
/// with the release term (C5): `low_i = v_i + Σ min(0, d, x)`,
/// `high_i = v_i + Σ max(0, d, x)`, `d = complete_i·u`, `x = −reserve_i·u`.
/// `None` (Arithmetic) if a final bound leaves ±1e6: the partial sums are
/// monotone from a bounded start, so this is the same as the code's check
/// after every record.
pub fn envelope(
    cfg: &Config,
    recs: &[Commitment],
    v: &Vals,
    extra: Option<(u32, u32)>,
    skip: Option<u32>,
) -> Option<(Vals, Vals)> {
    let mut lo = *v;
    let mut hi = *v;
    let k = n(cfg);
    for c in recs {
        if open(&c.status) && Some(c.id) != skip {
            let r = rcp_of(cfg, c.recipe);
            for i in 0..M {
                if i < k {
                    let (a, b) = outcome(r, c.units, i);
                    lo[i] += a;
                    hi[i] += b;
                }
            }
        }
    }
    if let Some((rid, u)) = extra {
        let r = rcp_of(cfg, rid);
        for i in 0..M {
            if i < k {
                let (a, b) = outcome(r, u, i);
                lo[i] += a;
                hi[i] += b;
            }
        }
    }
    for i in 0..M {
        if i < k && (!bounded(lo[i]) || !bounded(hi[i])) {
            return None;
        }
    }
    Some((lo, hi))
}

/// `EV_i = max(0, min_i − low_i) + max(0, high_i − max_i)`.
pub fn ev(m: &Metric, lo: Z, hi: Z) -> Z {
    max0(m.min as Z - lo) + max0(hi - m.max as Z)
}

/// Which guard decided a result: distinguishes the several `Arithmetic`,
/// `Bounds`, `Energy`, `InvalidState` and no-op sites for the covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// A plain guard, or an effective `Ok`.
    Plain,
    /// `add` left ±1e6.
    Add,
    /// The envelope left ±1e6.
    Envelope,
    /// The recovery `add(next, complete, units)` left ±1e6.
    RecoveryAdd,
    /// `version = u32::MAX` (the plain increment: ack, cancel, settle; and
    /// submit's own increment, dead behind the cap).
    Version,
    /// `version >= VERSION_CAP` (VER-CAP: submit's candidate, dispatch,
    /// set_paused, observe).
    Cap,
    /// submit: `first_id + len` leaves `u32` (WF states only: on reachable
    /// states `first_id + len <= version < VERSION_CAP`).
    IdOverflow,
    /// cancel of a Completed record (TERM-1, v1.1: InvalidState).
    CancelCompleted,
    /// Energy increased (`after > before`).
    EnergyUp,
    /// Recovery rule: completion does not strictly decrease energy.
    Recovery,
    /// A no-op `Ok` (state unchanged, version included).
    Noop,
    /// settle: the opposite terminal status.
    Opposite,
    /// release (cancel or settle(false)).
    Release,
}

/// `check_bounds(next, extra, skip)` against the current values `cur`: no
/// metric's envelope violation may exceed its violation now or at `next`
/// (C6: the smaller of the two).
pub fn check_bounds(
    cfg: &Config,
    recs: &[Commitment],
    cur: &Vals,
    next: &Vals,
    extra: Option<(u32, u32)>,
    skip: Option<u32>,
) -> Result<(), (Error, Why)> {
    let Some((lo, hi)) = envelope(cfg, recs, next, extra, skip) else {
        return Err((Error::Arithmetic, Why::Envelope));
    };
    for i in 0..M {
        if i < n(cfg) {
            let m = &cfg.metrics[i];
            let (a, b) = (pv(m, cur[i]), pv(m, next[i]));
            if ev(m, lo[i], hi[i]) > if a < b { a } else { b } {
                return Err((Error::Bounds, Why::Plain));
            }
        }
    }
    Ok(())
}

/// One energy term: `w·r²` with `r = |v − t|`, `max(0, v − t)` or `max(0, t − v)`.
pub fn term(t: &Term, v: Z) -> Z {
    let d = v - t.target as Z;
    let r = match t.shape {
        Shape::Target => {
            if d < 0 {
                -d
            } else {
                d
            }
        }
        Shape::Excess => max0(d),
        Shape::Deficit => max0(-d),
    };
    // `r ≤ 2e6 < 2^21` and `w ≤ 1000 < 2^10` (asserted), so `w·r² < 2^52`
    // is exact in 64 bits without an overflow-checked multiply.
    assert!(r <= 2_000_000 && t.weight <= 1000, "spec: r <= 2e6, w <= 1000");
    let r = r as u64;
    r.wrapping_mul(r).wrapping_mul(t.weight as u64) as Z
}

/// Energy: `Σ_terms w·r²`, exact.
pub fn energy(cfg: &Config, v: &Vals) -> Z {
    let mut e: Z = 0;
    for t in &cfg.terms {
        e += term(t, v[t.metric as usize]);
    }
    e
}

/// Units of Completed records of recipe `p` among `recs` (a scan of the
/// retained records: INV-CU and INV-DEP compare it with the ghost).
pub fn retained_done(recs: &[Commitment], p: u32) -> u64 {
    let mut s = 0u64;
    for c in recs {
        if c.recipe == p && c.status == Status::Completed {
            s += c.units as u64;
        }
    }
    s
}

/// v1.0's rule: every prerequisite of `r` is met by Completed units in
/// `recs` (DEP-1 cross-check on states where nothing was pruned).
pub fn prereqs_met_scan(r: &Recipe, recs: &[Commitment]) -> bool {
    let mut ok = true;
    for (p, u) in &r.requires {
        if retained_done(recs, *p) < *u as u64 {
            ok = false;
        }
    }
    ok
}

/// `cu[k]`, read over concrete slots; `None` if `k` is out of range.
pub fn cu_at(cu: &[u32], k: u32) -> Option<u32> {
    let mut out = None;
    for (j, x) in cu.iter().enumerate() {
        if j as u64 == k as u64 {
            out = Some(*x);
        }
    }
    out
}

/// `true` iff every prerequisite `(p, u)` of `r` has `completed_units[p] >= u`
/// (C8: the lifetime count, which a prune keeps).
pub fn prereqs_met(r: &Recipe, cu: &[u32]) -> bool {
    let mut ok = true;
    for (p, u) in &r.requires {
        match cu_at(cu, *p) {
            Some(x) => {
                if x < *u {
                    ok = false;
                }
            }
            None => panic!("spec: prerequisite recipe within completed_units"),
        }
    }
    ok
}

/// `submit`'s admission (the code's private `candidate`): the reserved
/// values, or the first failing guard in code order.
pub fn candidate(p: &Pool, caller: &str, req: &Request, now: u64) -> Result<Vals, (Error, Why)> {
    let cfg = &p.config;
    let plain = |e| Err((e, Why::Plain));
    if p.paused {
        return plain(Error::Paused);
    }
    if req.version != p.version {
        return plain(Error::Stale);
    }
    // Exact sums: `now + 86400` and `observed_at + max_age` never wrap here.
    let now_x = now as u128;
    let exp_x = req.expires_at as u128;
    if exp_x <= now_x || exp_x > now_x + DAY {
        return plain(Error::Expired);
    }
    if cfg.max_age_seconds > 0 && now_x > p.observed_at as u128 + cfg.max_age_seconds as u128 {
        return plain(Error::SnapshotExpired);
    }
    // Full only while the oldest retained record is open (C7): a full
    // window with a terminal front prunes it.
    if p.records.len() >= MAXREC && open(&p.records[0].status) {
        return plain(Error::Full);
    }
    let Some(r) = rcp(cfg, req.recipe) else {
        return plain(Error::InvalidRecipe);
    };
    if !eq_str(caller, &r.actor) {
        return plain(Error::Unauthorized);
    }
    if req.units == 0 || req.units > r.max_units {
        return plain(Error::InvalidUnits);
    }
    if !prereqs_met(r, &p.completed_units) {
        return plain(Error::Prerequisite);
    }
    let v = vals(cfg, &p.values);
    let Some(next) = add(cfg, &v, &r.reserve, req.units) else {
        return Err((Error::Arithmetic, Why::Add));
    };
    check_bounds(cfg, &p.records, &v, &next, Some((req.recipe, req.units)), None)?;
    let before = energy(cfg, &v);
    if energy(cfg, &next) > before {
        return Err((Error::Energy, Why::EnergyUp));
    }
    if violating(cfg, &v) {
        let Some(done) = add(cfg, &next, &r.complete, req.units) else {
            return Err((Error::Arithmetic, Why::RecoveryAdd));
        };
        if energy(cfg, &done) >= before {
            return Err((Error::Energy, Why::Recovery));
        }
    }
    // The cap is the last guard (C9), so `preview` agrees with `submit`.
    if p.version >= VERSION_CAP {
        return Err((Error::Arithmetic, Why::Cap));
    }
    Ok(next)
}

// ---------------------------------------------------------------------------
// Expected result and post-state
// ---------------------------------------------------------------------------

/// The expected outcome of one operation from a well-formed pre-state.
#[derive(Clone, Debug, PartialEq)]
pub struct Exp {
    pub res: Res,
    pub why: Why,
    /// Post-state (equal to the pre-state unless `res` is an effective `Ok`).
    pub values: Vals,
    pub version: u32,
    pub observed_at: u64,
    pub paused: bool,
    /// A record appended (submit).
    pub push: Option<Commitment>,
    /// submit drops the oldest record (the window was full, its front
    /// terminal).
    pub prune: bool,
    pub first_id: u32,
    /// A record whose status changes, by id.
    pub status: Option<(u32, Status)>,
    /// `completed_units[recipe]` becomes the value (settle(true)).
    pub cu: Option<(u32, u32)>,
}

impl Exp {
    fn unchanged(p: &Pool, res: Res, why: Why) -> Exp {
        Exp {
            res,
            why,
            values: vals(&p.config, &p.values),
            version: p.version,
            observed_at: p.observed_at,
            paused: p.paused,
            push: None,
            prune: false,
            first_id: p.first_id,
            status: None,
            cu: None,
        }
    }
    fn err(p: &Pool, e: Error) -> Exp {
        Exp::unchanged(p, Err(e), Why::Plain)
    }
    fn err_at(p: &Pool, (e, why): (Error, Why)) -> Exp {
        Exp::unchanged(p, Err(e), why)
    }
    /// An `Ok` that changes nothing, version included (TX-NOOP).
    pub fn noop(&self) -> bool {
        self.res.is_ok() && self.why == Why::Noop
    }
    /// An `Ok` that is not a no-op: increments the version (TX-VER).
    pub fn effective(&self) -> bool {
        self.res.is_ok() && self.why != Why::Noop
    }
}

/// `release(id)`: undo the reservation, check bounds without `id`, bump.
fn release(p: &Pool, id: u32) -> Exp {
    let cfg = &p.config;
    let c = rec(p, id).expect("spec: release of an existing record");
    let r = rcp_of(cfg, c.recipe);
    let v = vals(cfg, &p.values);
    let mut next = [0; M];
    for i in 0..M {
        if i < n(cfg) {
            next[i] = v[i] - mul_du(r.reserve[i], c.units);
            if !bounded(next[i]) {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Add);
            }
        }
    }
    if let Err(e) = check_bounds(cfg, &p.records, &v, &next, None, Some(id)) {
        return Exp::err_at(p, e);
    }
    if p.version == u32::MAX {
        return Exp::unchanged(p, Err(Error::Arithmetic), Why::Version);
    }
    let mut x = Exp::unchanged(p, Ok(None), Why::Release);
    x.values = next;
    x.version = p.version + 1;
    x.status = Some((id, Status::Cancelled));
    x
}

/// The exact result in guard order and the exact post-state of `op` from `p`.
pub fn expected(p: &Pool, op: &Op) -> Exp {
    let cfg = &p.config;
    let nrec = p.records.len();
    match op {
        Op::Submit { caller, req, now } => match candidate(p, caller, req, *now) {
            Err(e) => Exp::err_at(p, e),
            Ok(next) => {
                // Dead behind the cap (candidate), kept in code order.
                if p.version == u32::MAX {
                    return Exp::unchanged(p, Err(Error::Arithmetic), Why::Version);
                }
                // The new id: first_id + len, exact.
                let id = p.first_id as u64 + nrec as u64;
                if id > u32::MAX as u64 {
                    return Exp::unchanged(p, Err(Error::Arithmetic), Why::IdOverflow);
                }
                let mut x = Exp::unchanged(p, Ok(Some(id as u32)), Why::Plain);
                x.values = next;
                x.version = p.version + 1;
                x.prune = nrec >= MAXREC;
                x.first_id = if x.prune { p.first_id + 1 } else { p.first_id };
                x.push = Some(Commitment {
                    id: id as u32,
                    recipe: req.recipe,
                    units: req.units,
                    expires_at: req.expires_at,
                    status: Status::Reserved,
                });
                x
            }
        },
        Op::Dispatch { caller, id, now } => {
            let Some(c) = rec(p, *id) else {
                return Exp::err(p, Error::NotFound);
            };
            let r = rcp_of(cfg, c.recipe);
            if !eq_str(caller, &r.actor) {
                return Exp::err(p, Error::Unauthorized);
            }
            if p.paused {
                return Exp::err(p, Error::Paused);
            }
            if c.status != Status::Reserved {
                return Exp::err(p, Error::InvalidState);
            }
            if *now >= c.expires_at {
                return Exp::err(p, Error::Expired);
            }
            if cfg.max_age_seconds > 0
                && *now as u128 > p.observed_at as u128 + cfg.max_age_seconds as u128
            {
                return Exp::err(p, Error::SnapshotExpired);
            }
            if p.version >= VERSION_CAP {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Cap);
            }
            let mut x = Exp::unchanged(p, Ok(None), Why::Plain);
            x.version = p.version + 1;
            x.status = Some((*id, Status::Dispatched));
            x
        }
        Op::Ack { id, accepted } => {
            let Some(c) = rec(p, *id) else {
                return Exp::err(p, Error::NotFound);
            };
            let s = &c.status;
            if !matches!(s, Status::Dispatched | Status::Unknown) {
                return Exp::unchanged(p, Ok(None), Why::Noop);
            }
            let next = if *accepted {
                Status::Active
            } else {
                Status::Unknown
            };
            if *s == next {
                return Exp::unchanged(p, Ok(None), Why::Noop);
            }
            if p.version == u32::MAX {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Version);
            }
            let mut x = Exp::unchanged(p, Ok(None), Why::Plain);
            x.version = p.version + 1;
            x.status = Some((*id, next));
            x
        }
        Op::Cancel { caller, id, .. } => {
            let Some(c) = rec(p, *id) else {
                return Exp::err(p, Error::NotFound);
            };
            let r = rcp_of(cfg, c.recipe);
            if !eq_str(caller, &r.actor) && !eq_str(caller, &cfg.owner) {
                return Exp::err(p, Error::Unauthorized);
            }
            if c.status == Status::Cancelled {
                return Exp::unchanged(p, Ok(None), Why::Noop);
            }
            // Conflicting terminal evidence, as in settle (C14).
            if c.status == Status::Completed {
                return Exp::unchanged(p, Err(Error::InvalidState), Why::CancelCompleted);
            }
            if c.status != Status::Reserved {
                return Exp::err(p, Error::Pending);
            }
            release(p, *id)
        }
        Op::Settle {
            caller,
            id,
            completed,
        } => {
            let Some(c) = rec(p, *id) else {
                return Exp::err(p, Error::NotFound);
            };
            let r = rcp_of(cfg, c.recipe);
            if !eq_str(caller, &r.executor) {
                return Exp::err(p, Error::Unauthorized);
            }
            if (*completed && c.status == Status::Completed)
                || (!*completed && c.status == Status::Cancelled)
            {
                return Exp::unchanged(p, Ok(None), Why::Noop);
            }
            if !matches!(
                c.status,
                Status::Dispatched | Status::Active | Status::Unknown
            ) {
                let why = if terminal(&c.status) {
                    Why::Opposite
                } else {
                    Why::Plain
                };
                return Exp::unchanged(p, Err(Error::InvalidState), why);
            }
            if !*completed {
                return release(p, *id);
            }
            let v = vals(cfg, &p.values);
            let Some(next) = add(cfg, &v, &r.complete, c.units) else {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Add);
            };
            if let Err(e) = check_bounds(cfg, &p.records, &v, &next, None, Some(*id)) {
                return Exp::err_at(p, e);
            }
            if p.version == u32::MAX {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Version);
            }
            let mut x = Exp::unchanged(p, Ok(None), Why::Plain);
            x.values = next;
            x.version = p.version + 1;
            x.status = Some((*id, Status::Completed));
            // The lifetime count for prerequisites, saturating (C8).
            let Some(old) = cu_at(&p.completed_units, c.recipe) else {
                panic!("spec: settled recipe within completed_units");
            };
            let sum = old as u64 + c.units as u64;
            x.cu = Some((c.recipe, if sum > u32::MAX as u64 { u32::MAX } else { sum as u32 }));
            x
        }
        Op::Pause { caller, paused } => {
            if !eq_str(caller, &cfg.owner) {
                return Exp::err(p, Error::Unauthorized);
            }
            if p.version >= VERSION_CAP {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Cap);
            }
            let mut x = Exp::unchanged(p, Ok(None), Why::Plain);
            x.version = p.version + 1;
            x.paused = *paused;
            x
        }
        Op::Observe { caller, vals: obs, now } => {
            if !eq_str(caller, &cfg.owner) {
                return Exp::err(p, Error::Unauthorized);
            }
            // New values wait for every record to be terminal; restating the
            // current ones (same length and elements) does not (C16, F-6).
            if !eq_i64s(obs, &p.values) && any_open(&p.records) {
                return Exp::err(p, Error::Pending);
            }
            if obs.len() != n(cfg) {
                return Exp::err(p, Error::Bounds);
            }
            let mut ok = true;
            for x in obs {
                if !bounded(*x as Z) {
                    ok = false;
                }
            }
            if !ok {
                return Exp::err(p, Error::Bounds);
            }
            if p.version >= VERSION_CAP {
                return Exp::unchanged(p, Err(Error::Arithmetic), Why::Cap);
            }
            let mut x = Exp::unchanged(p, Ok(None), Why::Plain);
            x.values = vals(cfg, obs);
            x.version = p.version + 1;
            x.observed_at = *now;
            x
        }
    }
}

/// The status of `pre.records[j]` after `x` (`x.status` names a record by
/// id; `pre.records[j]` has id `first_id + j`).
pub fn exp_status(pre: &Pool, x: &Exp, j: usize) -> Status {
    match &x.status {
        Some((id, s)) if pre.first_id as u64 + j as u64 == *id as u64 => s.clone(),
        _ => pre.records[j].status.clone(),
    }
}

/// `completed_units[r]` after `x`.
pub fn exp_cu(pre: &Pool, x: &Exp, r: usize) -> u32 {
    match x.cu {
        Some((k, v)) if k as usize == r => v,
        _ => pre.completed_units[r],
    }
}

/// TX-*: `post` is exactly the expected post-state: config, values, version,
/// observed_at, paused, first_id, completed_units and records (frame: every
/// other record unchanged; a prune drops exactly the front).
pub fn post_matches(pre: &Pool, post: &Pool, x: &Exp) -> bool {
    let cfg = &pre.config;
    let mut ok = same_config(&post.config, &pre.config)
        && post.values.len() == n(cfg)
        && post.version == x.version
        && post.observed_at == x.observed_at
        && post.paused == x.paused
        && post.first_id == x.first_id
        && post.completed_units.len() == pre.completed_units.len();
    for i in 0..M {
        if i < n(cfg) && i < post.values.len() && post.values[i] as Z != x.values[i] {
            ok = false;
        }
    }
    if !ok {
        return false;
    }
    if !cu_matches(pre, post, x) {
        ok = false;
    }
    if x.prune {
        // Only at 256 records: `h_full_256_prune` checks its own post-state
        // under Kani, where no other harness reaches a prune.
        #[cfg(kani)]
        {
            panic!("spec: post_matches is not used on a prune under Kani");
        }
        #[cfg(not(kani))]
        {
            return ok && records_match_prune(pre, post, x);
        }
    }
    let k = pre.records.len();
    let k2 = k + x.push.is_some() as usize;
    if post.records.len() != k2 {
        return false;
    }
    for j in 0..k {
        let a = &pre.records[j];
        let b = &post.records[j];
        if b.id != a.id
            || b.recipe != a.recipe
            || b.units != a.units
            || b.expires_at != a.expires_at
            || b.status != exp_status(pre, x, j)
        {
            ok = false;
        }
    }
    if let Some(c) = &x.push {
        let mut found = false;
        for (j, b) in post.records.iter().enumerate() {
            if j == k {
                found = b == c;
            }
        }
        if !found {
            ok = false;
        }
    }
    ok
}

/// `post.completed_units` per `x.cu` (element-wise over concrete slots).
fn cu_matches(pre: &Pool, post: &Pool, x: &Exp) -> bool {
    let k = pre.completed_units.len();
    #[cfg(kani)]
    assert!(k <= GR, "spec: completed_units of at most GR recipes");
    let mut ok = true;
    for r in 0..(if cfg!(kani) { GR } else { k }) {
        if r < k && post.completed_units[r] != exp_cu(pre, x, r) {
            ok = false;
        }
    }
    ok
}

/// The records after a submit that prunes: `post.records[j]` is
/// `pre.records[j + 1]` (statuses unchanged), then the push (plain mode,
/// and the reference for `h_full_256_prune`'s own checks).
#[cfg(not(kani))]
fn records_match_prune(pre: &Pool, post: &Pool, x: &Exp) -> bool {
    let k = pre.records.len();
    let Some(c) = &x.push else { return false };
    post.records.len() == k
        && terminal(&pre.records[0].status)
        && post.records[..k - 1] == pre.records[1..]
        && post.records[k - 1] == *c
}

// ---------------------------------------------------------------------------
// Lifecycle (LIFE-1)
// ---------------------------------------------------------------------------

/// LIFE-1: the allowed one-step status changes (or none).
pub fn life_step(a: &Status, b: &Status) -> bool {
    use Status::*;
    a == b
        || matches!(
            (a, b),
            (Reserved, Dispatched)
                | (Reserved, Cancelled)
                | (Dispatched, Active)
                | (Dispatched, Unknown)
                | (Unknown, Active)
                | (Dispatched, Completed)
                | (Dispatched, Cancelled)
                | (Active, Completed)
                | (Active, Cancelled)
                | (Unknown, Completed)
                | (Unknown, Cancelled)
        )
}

// ---------------------------------------------------------------------------
// Invariants and ghost
// ---------------------------------------------------------------------------

/// Ghost state, a summary of the history (K-D5):
/// - `base`, `k0`: the values at the last value-changing `observe` (or
///   `new`), and the first id issued after it (`k0` is an id);
/// - `pruned`: the summed ledger contribution of pruned records with id
///   `>= k0` (reserve, plus completion if Completed);
/// - `eff`: the number of effective transitions (INV-VER; tracked from
///   `Init` only, `None` in step harnesses);
/// - `done[r]`: recipe `r`'s lifetime Completed units, unsaturated (INV-CU).
#[derive(Clone, Debug, PartialEq)]
pub struct Ghost {
    pub base: Vec<i64>,
    pub k0: u32,
    pub eff: Option<u32>,
    pub pruned: Vals,
    pub done: [u64; GR],
}

impl Ghost {
    pub fn init(p: &Pool) -> Ghost {
        Ghost {
            base: crate::gen::copy_vec(&p.values, M + 1),
            k0: 0,
            eff: Some(0),
            pruned: [0; M],
            done: [0; GR],
        }
    }
}

/// `first_id + len` (the next id), saturated: it exceeds `u32` only in WF
/// states, whose ghost is not checked.
pub fn next_id(p: &Pool) -> u32 {
    let k = p.first_id as u64 + p.records.len() as u64;
    if k > u32::MAX as u64 {
        u32::MAX
    } else {
        k as u32
    }
}

/// The ghost after `op` with expected outcome `x` from `pre`.
///
/// Observe Ok keeps `base`, `k0` and `pruned` iff it restates the current
/// values with some record open (the refresh); otherwise it re-bases:
/// `base = vals`, `k0 = first_id + len`, `pruned = 0` (plan 2.2). A prune
/// moves the front's contribution into `pruned` if its id is `>= k0`.
/// settle(true) adds the units to `done`.
pub fn ghost_step(g: &Ghost, pre: &Pool, op: &Op, x: &Exp) -> Ghost {
    let mut h = Ghost {
        base: crate::gen::copy_vec(&g.base, M + 1),
        k0: g.k0,
        eff: g.eff,
        pruned: g.pruned,
        done: g.done,
    };
    if let (Op::Observe { vals, .. }, Ok(_)) = (op, &x.res) {
        let refresh = eq_i64s(vals, &pre.values) && any_open(&pre.records);
        if !refresh {
            h.base = crate::gen::copy_vec(vals, M + 1);
            h.k0 = next_id(pre);
            h.pruned = [0; M];
        }
    }
    if x.prune && x.res.is_ok() {
        let c = &pre.records[0];
        if c.id >= g.k0 && c.status == Status::Completed {
            let r = rcp_of(&pre.config, c.recipe);
            for i in 0..M {
                if i < n(&pre.config) {
                    h.pruned[i] += mul_du(r.reserve[i], c.units) + mul_du(r.complete[i], c.units);
                }
            }
        }
    }
    if let Some((r, _)) = x.cu {
        if let Op::Settle { id, .. } = op {
            let c = rec(pre, *id).expect("spec: settled record exists");
            assert!((r as usize) < GR, "spec: the ghost tracks at most GR recipes");
            for k in 0..GR {
                if k as u64 == r as u64 {
                    h.done[k] += c.units as u64;
                }
            }
        }
    }
    if x.effective() {
        h.eff = g.eff.map(|e| e + 1);
    }
    h
}

/// `B_i = PV_i(base)`.
pub fn b_of(cfg: &Config, g: &Ghost) -> Vals {
    let b = vals(cfg, &g.base);
    let mut out = [0; M];
    for i in 0..M {
        if i < n(cfg) {
            out[i] = pv(&cfg.metrics[i], b[i]);
        }
    }
    out
}

/// `B = 0`: the base at the last observation is inside every `[min, max]`.
pub fn healthy_base(cfg: &Config, g: &Ghost) -> bool {
    let b = b_of(cfg, g);
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) && b[i] != 0 {
            ok = false;
        }
    }
    ok
}

/// CFG-1/CFG-2: `validate`, written from the brief.
pub fn valid(cfg: &Config) -> bool {
    let k = cfg.metrics.len();
    if !short(&cfg.name)
        || !short(&cfg.owner)
        || k == 0
        || k > 32
        || cfg.recipes.is_empty()
        || cfg.recipes.len() > 32
        || cfg.terms.is_empty()
        || cfg.terms.len() > 64
    {
        return false;
    }
    let mut ok = true;
    for m in &cfg.metrics {
        let (lo, hi, x) = (m.min as Z, m.max as Z, m.initial as Z);
        if !short(&m.name) || !short(&m.unit) || !bounded(lo) || !bounded(hi) || !(lo <= x && x <= hi) {
            ok = false;
        }
    }
    for t in &cfg.terms {
        if t.metric as usize >= k || !bounded(t.target as Z) || !(1..=1000).contains(&t.weight) {
            ok = false;
        }
    }
    for (i, r) in cfg.recipes.iter().enumerate() {
        if !short(&r.name)
            || !short(&r.actor)
            || !short(&r.executor)
            || r.reserve.len() != k
            || r.complete.len() != k
            || !(1..=100).contains(&r.max_units)
            || r.requires.len() > 32
        {
            ok = false;
        }
        // CFG-2 (v1.1, F-4): a recipe's actor is not its executor. The
        // owner may hold either role, not both.
        if eq_str(&r.actor, &r.executor) {
            ok = false;
        }
        // Two loops, not one over the chain: each stays within the harness
        // unwind (vectors of at most 3 values in raw configurations).
        for x in r.reserve.iter() {
            if !bounded(*x as Z) {
                ok = false;
            }
        }
        for x in r.complete.iter() {
            if !bounded(*x as Z) {
                ok = false;
            }
        }
        for (j, (p, u)) in r.requires.iter().enumerate() {
            if *p as usize >= i || !(1..=100).contains(u) {
                ok = false;
            }
            for (q, _) in &r.requires[..j] {
                if q == p {
                    ok = false;
                }
            }
        }
    }
    ok
}

/// WIN-1 and the well-formedness every step needs (v1.0's INV-WF):
/// - values of length `n <= M` and bounded; the ghost's base likewise;
/// - `records[j].id = first_id + j`; recipe in range; `1 <= units <= max_units`;
/// - at most 256 records, and `first_id = 0` until the window is full
///   (`first_id = 0 || len = 256`: only a submit at 256 prunes);
/// - `first_id + len <= version` (every submit is an effective transition);
/// - `completed_units` has one entry per recipe;
/// - ghost: `k0 <= first_id + len`, every record with id `< k0` terminal.
pub fn win(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    let k = n(cfg);
    let len = p.records.len();
    if k > M || p.values.len() != k || g.base.len() != k || len > MAXREC {
        return false;
    }
    let next = p.first_id as u64 + len as u64;
    let mut ok = (p.first_id == 0 || len == MAXREC)
        && next <= p.version as u64
        && g.k0 as u64 <= next
        && p.completed_units.len() == cfg.recipes.len();
    for i in 0..M {
        if i < k && (!bounded(p.values[i] as Z) || !bounded(g.base[i] as Z)) {
            ok = false;
        }
    }
    for (j, c) in p.records.iter().enumerate() {
        if c.id as u64 != p.first_id as u64 + j as u64 || c.recipe as usize >= cfg.recipes.len() {
            ok = false;
        } else {
            let r = rcp_of(cfg, c.recipe);
            if c.units == 0 || c.units > r.max_units {
                ok = false;
            }
        }
        if c.id < g.k0 && open(&c.status) {
            ok = false;
        }
    }
    ok
}

/// `base + Σ_{id ≥ k0, ¬Cancelled} reserve·u + Σ_{id ≥ k0, Completed} complete·u`
/// over the retained records `recs` (the pruned ones are the ghost's
/// `pruned`, added by [`inv_ledger`]).
pub fn ledger(cfg: &Config, base: &[i64], recs: &[Commitment], k0: u32) -> Vals {
    let mut v = vals(cfg, base);
    for c in recs {
        if c.id >= k0 && c.status != Status::Cancelled {
            let r = rcp_of(cfg, c.recipe);
            for i in 0..M {
                if i < n(cfg) {
                    v[i] += mul_du(r.reserve[i], c.units);
                    if c.status == Status::Completed {
                        v[i] += mul_du(r.complete[i], c.units);
                    }
                }
            }
        }
    }
    v
}

/// INV-LEDGER (needs WIN-1): `values = ledger + pruned`.
pub fn inv_ledger(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    let l = ledger(cfg, &g.base, &p.records, g.k0);
    let v = vals(cfg, &p.values);
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) && l[i] + g.pruned[i] != v[i] {
            ok = false;
        }
    }
    ok
}

/// INV-RES (needs WIN-1; ghost-free): the release-aware envelope over the
/// open records is computable (both endpoints within ±1e6) and its
/// violation is the current one, `EV_i = PV_i(values)` (`EV >= PV` always:
/// the box contains `values`).
pub fn inv_res(p: &Pool) -> bool {
    let cfg = &p.config;
    let v = vals(cfg, &p.values);
    let Some((lo, hi)) = envelope(cfg, &p.records, &v, None, None) else {
        return false;
    };
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) {
            let m = &cfg.metrics[i];
            if ev(m, lo[i], hi[i]) != pv(m, v[i]) {
                ok = false;
            }
        }
    }
    ok
}

/// INV-PV: `PV_i(values) <= B_i` (no step increases a violation; INV-HARD
/// is INV-RES and INV-PV together).
pub fn inv_pv(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    let v = vals(cfg, &p.values);
    let b = b_of(cfg, g);
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) && pv(&cfg.metrics[i], v[i]) > b[i] {
            ok = false;
        }
    }
    ok
}

/// The increments an open record may still take before it is terminal:
/// Reserved 3 (dispatch, ack, settle; or 1 by cancel), Dispatched 3
/// (Unknown, Active, settle), Unknown 2, Active 1, terminal 0 (plan 2.2;
/// the core's `VERSION_CAP` comment).
pub fn rem(s: &Status) -> u64 {
    match s {
        Status::Reserved | Status::Dispatched => 3,
        Status::Unknown => 2,
        Status::Active => 1,
        Status::Completed | Status::Cancelled => 0,
    }
}

/// INV-VERCAP: `version + Σ rem(status) <= u32::MAX`, so every ack,
/// cancel and settle of an open record has its increment.
pub fn inv_vercap(p: &Pool) -> bool {
    let mut s = p.version as u64;
    for c in &p.records {
        s += rem(&c.status);
    }
    s <= u32::MAX as u64
}

/// `g.done[r]` over concrete slots (`r < GR`, asserted).
pub fn done_at(g: &Ghost, r: u32) -> u64 {
    assert!((r as usize) < GR, "spec: the ghost tracks at most GR recipes");
    let mut out = 0;
    for k in 0..GR {
        if k as u64 == r as u64 {
            out = g.done[k];
        }
    }
    out
}

/// INV-CU (tagged WIN-1): `completed_units[r] = min(done[r], u32::MAX)`,
/// the saturated lifetime count.
pub fn inv_cu(p: &Pool, g: &Ghost) -> bool {
    let nr = p.config.recipes.len();
    if p.completed_units.len() != nr || nr > GR {
        return false;
    }
    let mut ok = true;
    for r in 0..GR {
        if r < nr {
            let d = g.done[r];
            let want = if d > u32::MAX as u64 { u32::MAX } else { d as u32 };
            if p.completed_units[r] != want {
                ok = false;
            }
        }
    }
    ok
}

/// INV-HARD (corollary of INV-RES and INV-PV; asserted in plain mode only):
/// the envelope over open records is computable and `EV_i ≤ B_i`.
pub fn inv_hard(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    let v = vals(cfg, &p.values);
    let Some((lo, hi)) = envelope(cfg, &p.records, &v, None, None) else {
        return false;
    };
    let b = b_of(cfg, g);
    let mut ok = true;
    for i in 0..M {
        if i < n(cfg) && ev(&cfg.metrics[i], lo[i], hi[i]) > b[i] {
            ok = false;
        }
    }
    ok
}

/// Most open records whose outcome combinations [`inv_healthy`] enumerates.
pub const HEALTHY_MAX: usize = 5;

/// INV-HEALTHY (corollary of INV-HARD; plain mode only): with `B = 0`, every
/// combination of outcomes of the open records (each stays open, completes
/// or releases its reservation) lands inside every `[min, max]`. States with
/// more than [`HEALTHY_MAX`] open records are not enumerated (`3^k`
/// combinations); INV-HARD covers them.
pub fn inv_healthy(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    if !healthy_base(cfg, g) {
        return true;
    }
    let v = vals(cfg, &p.values);
    let mut outs: Vec<[[Z; M]; 3]> = Vec::new();
    for c in &p.records {
        if open(&c.status) {
            let r = rcp_of(cfg, c.recipe);
            let mut o = [[0 as Z; M]; 3];
            for i in 0..M {
                if i < n(cfg) {
                    o[1][i] = mul_du(r.complete[i], c.units);
                    o[2][i] = -mul_du(r.reserve[i], c.units);
                }
            }
            outs.push(o);
        }
    }
    if outs.len() > HEALTHY_MAX {
        return true;
    }
    let mut ok = true;
    let mut combos = 1usize;
    for _ in 0..outs.len() {
        combos *= 3;
    }
    for mut code in 0..combos {
        let mut x = v;
        for o in &outs {
            let pick = code % 3;
            code /= 3;
            for i in 0..M {
                x[i] += o[pick][i];
            }
        }
        for i in 0..M {
            if i < n(cfg) && pv(&cfg.metrics[i], x[i]) != 0 {
                ok = false;
            }
        }
    }
    ok
}

/// INV-DEP: every retained record's prerequisites were met at its
/// admission. For record `j` and prerequisite `(q, u)`: the lifetime
/// Completed units of `q` minus those of retained records from `j` on (all
/// completed after `j`'s admission) is at least `u`. With nothing pruned
/// this is v1.0's "met by Completed records of lower index".
pub fn inv_dep(p: &Pool, g: &Ghost) -> bool {
    let cfg = &p.config;
    let mut ok = true;
    for (j, c) in p.records.iter().enumerate() {
        let r = rcp_of(cfg, c.recipe);
        for (q, u) in &r.requires {
            if done_at(g, *q) < retained_done(&p.records[j..], *q) + *u as u64 {
                ok = false;
            }
        }
    }
    ok
}

/// INV-VER: version = number of effective transitions (from `Init` only).
pub fn inv_ver(p: &Pool, g: &Ghost) -> bool {
    match g.eff {
        Some(e) => p.version == e,
        None => true,
    }
}

/// The whole invariant (with the configuration's validity, CFG; K-D6).
/// INV-HARD and INV-HEALTHY follow from it (plain mode asserts them).
pub fn inv(p: &Pool, g: &Ghost) -> bool {
    valid(&p.config)
        && win(p, g)
        && inv_ledger(p, g)
        && inv_res(p)
        && inv_pv(p, g)
        && inv_vercap(p)
        && inv_cu(p, g)
        && inv_dep(p, g)
        && inv_ver(p, g)
}

/// Debug name of an error (`preview`'s reason), written out.
pub fn reason(e: &Error) -> &'static str {
    match e {
        Error::InvalidConfig => "InvalidConfig",
        Error::Unauthorized => "Unauthorized",
        Error::Paused => "Paused",
        Error::Stale => "Stale",
        Error::Expired => "Expired",
        Error::SnapshotExpired => "SnapshotExpired",
        Error::InvalidRecipe => "InvalidRecipe",
        Error::InvalidUnits => "InvalidUnits",
        Error::Prerequisite => "Prerequisite",
        Error::Full => "Full",
        Error::Arithmetic => "Arithmetic",
        Error::Bounds => "Bounds",
        Error::Energy => "Energy",
        Error::InvalidState => "InvalidState",
        Error::NotFound => "NotFound",
        Error::Pending => "Pending",
    }
}
