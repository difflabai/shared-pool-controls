//! The TLA+ model `tla/SharedPool.tla` (v1.1.0, frozen at formalver
//! 538c97a), transcribed, and the abstraction map `alpha` from a pool plus
//! its ghost to a TLA+ state (near-approval strategy.md section 17.2;
//! shared-pool strategy.md section 17.2).
//!
//! The `h_ref_*` harnesses (and, in plain mode, every step test, every step
//! of the window and cap walks, and every step of every conformance trace)
//! check that one call of the real core refines the TLA+ spec, for every
//! INV state within the step bounds and every operation of the step
//! domains (in plain mode `h_wf` also checks it on WF states, which reach
//! `SubmitErr`'s id overflow and `Settle`'s `DoneCap`):
//!
//! - REF-1 (guard exactness): the core's error is the TLA+ error operator's
//!   (`SubmitErr`, `DispatchErr`, `AckErr`, `CancelErr`, `SettleErr`,
//!   `PauseErr`, `ObserveErr`), and the core returns `Ok` exactly when the
//!   operator gives "Ok" (the action is enabled) or "Noop" (the core's
//!   idempotent `Ok(())`, which the model treats as a stuttering step);
//! - REF-2 (effect exactness): when the operator gives "Ok", `alpha(post)`
//!   is the action's post-state ([`T::next`]), and submit returns the new
//!   record's id `firstId + NR` (TraceCheck's `Agrees`);
//! - REF-3 (stutter): otherwise (an error, or "Noop"),
//!   `alpha(post) = alpha(pre)`.
//!
//! Written from the `.tla` text, operator by operator (the TLA+ name is in
//! each doc comment); it does not use `spec.rs`'s semantics, only its `Op`
//! and `Res` types and its string equality `eq_str` (equality, encoded for
//! CBMC). Integers are exact: every quantity fits `i64`/`u64` by a wide
//! margin on well-formed states (values, deltas and targets within ±1e6,
//! units ≤ 100, at most 256 records, ids and versions below 2^33), and the
//! products' preconditions are asserted, so a wrap is a failed check, never
//! a silent value.
//!
//! **The state image.** The TLA+ variables are `cfg, values, version,
//! observedAt, paused, records, firstId, doneU` (the `Pool`, field by field)
//! and the ghost `base, k0, steps, last`. `alpha(pool, base, k0)` maps the
//! first ten. `base` and `k0` are the implementation's ghost (plan 2.2,
//! [`ghost_after`]): the values at the last re-basing `observe` (or `new`)
//! and the first id not yet counted then; an observe `Ok` re-bases them
//! unless it restates the current values while a record is open (the
//! refresh), and a submit that prunes a counted record folds its
//! contribution into `base` and moves `k0` past it (`Submit`). `k0` is an
//! id. No operator reads a pruned record, so `alpha` is total on the
//! window (plan-kani 3.7). `steps` and `last` are history variables that
//! only `Step` writes (the count of effective calls and the last effective
//! call's arguments); no Pool field corresponds to them and no error
//! operator or action reads them, so `alpha` leaves them out (as
//! near-approval's `alpha` leaves its ghost out).
//!
//! `alpha` is partial, as the module says: "records[k] has id firstId + k -
//! 1, so ids are implicit (WIN-1)", `doneU` has one entry per recipe, and
//! `ValidCfg` gives every recipe vector the length `Len(cfg.metrics)`.
//! `alpha` asserts all three, so a post-state outside its domain (a wrong
//! id, a missing count, a recipe vector of another length) fails the
//! harness instead of being mapped. Names and units of metrics and recipes
//! and the pool's name are not modelled (the core only validates their
//! lengths). Accounts are the account strings themselves, borrowed from the
//! pool and compared with `eq_str`.
//!
//! **Capacities.** The image has fixed arrays (`NM` metrics, `NT` terms,
//! `NR` recipes, `NQ` requirements per recipe, `NC` records) and counts, so
//! the same code runs symbolically (Kani: [`TK`], the step bounds, `firstId
//! = 0`, no prune) and over the conformance traces (plain mode: [`TB`], the
//! core's own limits, prunes included: the window's REF is the traces').
//! Every
//! loop runs over a capacity and tests the count, so CBMC sees constant trip
//! counts (RESULTS.md, cost ladder); every read of the pool is at a concrete
//! offset.
//!
//! PROP: REF-1 REF-2 REF-3

use crate::spec::{eq_str, Op, Res};
use pool_core::{Error, Pool, Shape, Status};

/// `MaxValue` (MAX_VALUE).
pub const MAX_VALUE: i64 = 1_000_000;
/// `Window`: submit's `expires_at <= now + 86400`.
pub const WINDOW: u64 = 86_400;
/// The CONSTANT `MaxRecords`, at the code's value (the TLC configurations
/// scale it, C-5).
pub const MAX_RECORDS: usize = 256;
/// The CONSTANT `MaxVersion` (u32::MAX), at the code's value (the TLC
/// configurations use 2^31 - 1).
pub const MAX_VERSION: u64 = u32::MAX as u64;
/// `VersionCap == MaxVersion - 3 * MaxRecords` (VERSION_CAP, U:24).
pub const VERSION_CAP: u64 = MAX_VERSION - 3 * MAX_RECORDS as u64;
/// The CONSTANT `DoneCap` (u32::MAX: `completed_units` saturates there), at
/// the code's value.
pub const DONE_CAP: u64 = u32::MAX as u64;

/// The value of an error operator: the guard strings of `GuardOrder`, plus
/// "Ok" (the action is enabled) and "Noop" (an idempotent `Ok(())`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TErr {
    Ok,
    Noop,
    Paused,
    Stale,
    Expired,
    SnapshotExpired,
    Full,
    InvalidRecipe,
    Unauthorized,
    InvalidUnits,
    Prerequisite,
    Arithmetic,
    Bounds,
    Energy,
    NotFound,
    InvalidState,
    Pending,
}

/// The TLA+ string of a core error. `None` for `InvalidConfig`, which no
/// entry point of the model returns.
pub fn code(e: &Error) -> Option<TErr> {
    Some(match e {
        Error::InvalidConfig => return None,
        Error::Unauthorized => TErr::Unauthorized,
        Error::Paused => TErr::Paused,
        Error::Stale => TErr::Stale,
        Error::Expired => TErr::Expired,
        Error::SnapshotExpired => TErr::SnapshotExpired,
        Error::InvalidRecipe => TErr::InvalidRecipe,
        Error::InvalidUnits => TErr::InvalidUnits,
        Error::Prerequisite => TErr::Prerequisite,
        Error::Full => TErr::Full,
        Error::Arithmetic => TErr::Arithmetic,
        Error::Bounds => TErr::Bounds,
        Error::Energy => TErr::Energy,
        Error::InvalidState => TErr::InvalidState,
        Error::NotFound => TErr::NotFound,
        Error::Pending => TErr::Pending,
    })
}

/// `Statuses`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum St {
    Reserved,
    Dispatched,
    Active,
    Unknown,
    Completed,
    Cancelled,
}

/// `Shapes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sh {
    Target,
    Excess,
    Deficit,
}

/// `cfg.metrics[m]`: `[min, max, init]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TMetric {
    pub min: i64,
    pub max: i64,
    pub init: i64,
}

/// `cfg.terms[k]`: `[metric, target, weight, shape]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TTerm {
    pub metric: u32,
    pub target: i64,
    pub weight: u32,
    pub shape: Sh,
}

/// `cfg.recipes[k]`: `[actor, executor, reserve, complete, maxUnits,
/// requires]`; `requires` is a sequence of `<<recipe, units>>` (`nq` long).
#[derive(Clone, Copy, Debug)]
pub struct TRcp<'a, const NM: usize, const NQ: usize> {
    pub actor: &'a str,
    pub executor: &'a str,
    pub reserve: [i64; NM],
    pub complete: [i64; NM],
    pub max_units: u32,
    pub nq: usize,
    pub requires: [(u32, u32); NQ],
}

/// `records[k + 1]` (id `firstId + k`): `[recipe, units, exp, status]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TRec {
    pub recipe: u32,
    pub units: u32,
    pub exp: u64,
    pub status: St,
}

/// A TLA+ state of `SharedPool` (without `steps` and `last`, see the module
/// doc). Sequences are a capacity-sized array and a length.
#[derive(Clone, Copy, Debug)]
pub struct T<'a, const NM: usize, const NT: usize, const NR: usize, const NQ: usize, const NC: usize> {
    // cfg
    pub owner: &'a str,
    pub max_age: u32,
    pub nm: usize,
    pub metrics: [TMetric; NM],
    pub nt: usize,
    pub terms: [TTerm; NT],
    pub nrcp: usize,
    pub recipes: [TRcp<'a, NM, NQ>; NR],
    // values, version, observedAt, paused, records, firstId, doneU
    pub nv: usize,
    pub values: [i64; NM],
    pub version: u64,
    pub observed_at: u64,
    pub paused: bool,
    pub nrec: usize,
    pub records: [TRec; NC],
    pub first_id: u64,
    /// `doneU[p + 1]` is `done_u[p]`; its length is `nrcp`.
    pub done_u: [u64; NR],
    // ghost: base, k0 (an id)
    pub nb: usize,
    pub base: [i64; NM],
    pub k0: u64,
}

/// The image the `h_ref_*` harnesses use: the step bounds (at most 2
/// metrics, 2 terms, 2 recipes, 2 requirements, 3 records plus one submit;
/// `firstId = 0`).
pub type TK<'a> = T<'a, 2, 2, 2, 2, 4>;
/// The image for the conformance traces (plain mode): the core's limits
/// (MAX_METRICS, MAX_TERMS, MAX_RECIPES, MAX_RECIPES requirements,
/// MAX_RECORDS).
pub type TB<'a> = T<'a, 32, 64, 32, 32, 256>;

fn st(s: &Status) -> St {
    match s {
        Status::Reserved => St::Reserved,
        Status::Dispatched => St::Dispatched,
        Status::Active => St::Active,
        Status::Unknown => St::Unknown,
        Status::Completed => St::Completed,
        Status::Cancelled => St::Cancelled,
    }
}

fn sh(s: &Shape) -> Sh {
    match s {
        Shape::Target => Sh::Target,
        Shape::Excess => Sh::Excess,
        Shape::Deficit => Sh::Deficit,
    }
}

// ---------------------------------------------------------------------------
// alpha
// ---------------------------------------------------------------------------

/// `alpha(pool, base, k0)`: the TLA+ state of a pool with the ghost `(base,
/// k0)`. Asserts that the pool is in the domain of the map (record `k` has
/// id `first_id + k`, one completed-units count per recipe, recipe vectors
/// span the metrics) and within the image's capacity.
pub fn alpha<'a, const NM: usize, const NT: usize, const NR: usize, const NQ: usize, const NC: usize>(
    p: &'a Pool,
    base: &[i64],
    k0: u64,
) -> T<'a, NM, NT, NR, NQ, NC> {
    let c = &p.config;
    let (nm, nt, nrcp, nv, nrec, nb) = (
        c.metrics.len(),
        c.terms.len(),
        c.recipes.len(),
        p.values.len(),
        p.records.len(),
        base.len(),
    );
    assert!(
        nm <= NM && nt <= NT && nrcp <= NR && nv <= NM && nrec <= NC && nb <= NM,
        "REF alpha: the pool within the image's capacity"
    );
    assert!(
        p.completed_units.len() == nrcp,
        "REF alpha: one completed_units count per recipe (DOMAIN doneU = 1 .. Len(cfg.recipes))"
    );
    let blank_rcp = TRcp {
        actor: "",
        executor: "",
        reserve: [0; NM],
        complete: [0; NM],
        max_units: 0,
        nq: 0,
        requires: [(0, 0); NQ],
    };
    let mut t = T {
        owner: &c.owner,
        max_age: c.max_age_seconds,
        nm,
        metrics: [TMetric { min: 0, max: 0, init: 0 }; NM],
        nt,
        terms: [TTerm {
            metric: 0,
            target: 0,
            weight: 0,
            shape: Sh::Target,
        }; NT],
        nrcp,
        recipes: [blank_rcp; NR],
        nv,
        values: [0; NM],
        version: p.version as u64,
        observed_at: p.observed_at,
        paused: p.paused,
        nrec,
        records: [TRec {
            recipe: 0,
            units: 0,
            exp: 0,
            status: St::Reserved,
        }; NC],
        first_id: p.first_id as u64,
        done_u: [0; NR],
        nb,
        base: [0; NM],
        k0,
    };
    for i in 0..NM {
        if i < nm {
            let m = &c.metrics[i];
            t.metrics[i] = TMetric {
                min: m.min,
                max: m.max,
                init: m.initial,
            };
        }
        if i < nv {
            t.values[i] = p.values[i];
        }
        if i < nb {
            t.base[i] = base[i];
        }
    }
    for k in 0..NT {
        if k < nt {
            let x = &c.terms[k];
            t.terms[k] = TTerm {
                metric: x.metric,
                target: x.target,
                weight: x.weight,
                shape: sh(&x.shape),
            };
        }
    }
    for r in 0..NR {
        if r < nrcp {
            let x = &c.recipes[r];
            assert!(
                x.reserve.len() == nm && x.complete.len() == nm && x.requires.len() <= NQ,
                "REF alpha: recipe vectors span the metrics (ValidCfg), requirements within capacity"
            );
            let mut y = blank_rcp;
            y.actor = &x.actor;
            y.executor = &x.executor;
            y.max_units = x.max_units;
            y.nq = x.requires.len();
            for i in 0..NM {
                if i < nm {
                    y.reserve[i] = x.reserve[i];
                    y.complete[i] = x.complete[i];
                }
            }
            for q in 0..NQ {
                if q < y.nq {
                    y.requires[q] = x.requires[q];
                }
            }
            t.recipes[r] = y;
            t.done_u[r] = p.completed_units[r] as u64;
        }
    }
    for j in 0..NC {
        if j < nrec {
            let x = &p.records[j];
            assert!(
                x.id as u64 == p.first_id as u64 + j as u64,
                "REF alpha: records[j].id = first_id + j (WIN-1, ids are implicit)"
            );
            t.records[j] = TRec {
                recipe: x.recipe,
                units: x.units,
                exp: x.expires_at,
                status: st(&x.status),
            };
        }
    }
    t
}

// ---------------------------------------------------------------------------
// Arithmetic helpers
// ---------------------------------------------------------------------------

/// `Max0`.
fn max0(x: i64) -> i64 {
    if x > 0 {
        x
    } else {
        0
    }
}

/// `Min0`.
fn min0(x: i64) -> i64 {
    if x < 0 {
        x
    } else {
        0
    }
}

/// `Min(x, y)`.
fn min(x: i64, y: i64) -> i64 {
    if x < y {
        x
    } else {
        y
    }
}

/// `Max(x, y)`.
fn max(x: i64, y: i64) -> i64 {
    if x > y {
        x
    } else {
        y
    }
}

/// `Bounded`.
fn bounded(v: i64) -> bool {
    -MAX_VALUE <= v && v <= MAX_VALUE
}

/// `d * u` for a delta of a valid configuration (`|d| <= MaxValue`) and
/// units `u <= MaxUnit`: every call site (request units after the
/// `InvalidUnits` guard, record units by INV-WF). The preconditions are
/// asserted, so the product (`|d * u| <= 1e8 < 2^31`) is exact in 32 bits,
/// a quarter of a 64-bit multiplier with its overflow check (RESULTS.md,
/// deviation 2).
fn mul(d: i64, u: u32) -> i64 {
    assert!(-MAX_VALUE <= d && d <= MAX_VALUE && u <= 100, "REF tla: |d| <= MaxValue, u <= MaxUnit");
    (d as i32).wrapping_mul(u as i32) as i64
}

/// `Open(s)`.
fn open(s: St) -> bool {
    !matches!(s, St::Completed | St::Cancelled)
}

/// `InFlight(s)`.
fn in_flight(s: St) -> bool {
    matches!(s, St::Dispatched | St::Active | St::Unknown)
}

/// `AckTo(acc)`.
fn ack_to(acc: bool) -> St {
    if acc {
        St::Active
    } else {
        St::Unknown
    }
}

/// `<<LoTerm(rc, u, m), HiTerm(rc, u, m)>>`: the envelope's terms of one
/// record or the extra on metric `m`, `min(d, x, 0)` and `max(d, x, 0)` for
/// the completion `d = complete * u` and the release `x = -(reserve * u)`.
fn terms<'a, const NM: usize, const NQ: usize>(rc: &TRcp<'a, NM, NQ>, u: u32, m: usize) -> (i64, i64) {
    let d = mul(rc.complete[m], u);
    let x = -mul(rc.reserve[m], u);
    (min0(min(d, x)), max0(max(d, x)))
}

// ---------------------------------------------------------------------------
// The core's private functions, and the error operators. Arguments that are
// record ids, recipes and units are the code's u32s; `skip` is the TLA+
// integer (-1 for None).
// ---------------------------------------------------------------------------

impl<'a, const NM: usize, const NT: usize, const NR: usize, const NQ: usize, const NC: usize>
    T<'a, NM, NT, NR, NQ, NC>
{
    /// `Rcp(r)`. By value, as `rec` and `rec_rcp`: under Kani 0.68.0 a
    /// reference into `recipes` at an index CBMC does not fold, passed to
    /// another function (`neg(&rcp(r).reserve)`), read unconstrained values
    /// there (RESULTS.md, "Tool issue"); a copy is read in place.
    fn rcp(&self, r: u32) -> TRcp<'a, NM, NQ> {
        self.recipes[r as usize]
    }

    /// `Found(id)`: `firstId <= id < firstId + NR`.
    fn found(&self, id: u32) -> bool {
        self.first_id <= id as u64 && (id as u64) < self.first_id + self.nrec as u64
    }

    /// `Pos(id) - 1`, the 0-based position of a found id.
    fn pos(&self, id: u32) -> usize {
        (id as u64 - self.first_id) as usize
    }

    /// `Rec(id)` (for a found id).
    fn rec(&self, id: u32) -> TRec {
        self.records[self.pos(id)]
    }

    /// `RecRcp(id)`.
    fn rec_rcp(&self, id: u32) -> TRcp<'a, NM, NQ> {
        self.rcp(self.rec(id).recipe)
    }

    /// `PV(vals, m)` (0-based `m`).
    fn pv(&self, vals: &[i64; NM], m: usize) -> i64 {
        let x = &self.metrics[m];
        max0(x.min - vals[m]) + max0(vals[m] - x.max)
    }

    /// `Violating`.
    fn violating(&self) -> bool {
        let mut any = false;
        for m in 0..NM {
            if m < self.nm && self.pv(&self.values, m) > 0 {
                any = true;
            }
        }
        any
    }

    /// `AnyOpen`.
    fn any_open(&self) -> bool {
        let mut any = false;
        for k in 0..NC {
            if k < self.nrec && open(self.records[k].status) {
                any = true;
            }
        }
        any
    }

    /// `WinFull`: the window is full and its oldest record is open.
    fn win_full(&self) -> bool {
        self.nrec >= MAX_RECORDS && open(self.records[0].status)
    }

    /// `AddVec(b, d, u)` with `DOMAIN b = 1 .. Len(values)` (every `b` here
    /// is `values` or `AddVec` of it).
    fn add_vec(&self, b: &[i64; NM], d: &[i64; NM], u: u32) -> [i64; NM] {
        let mut out = [0; NM];
        for m in 0..NM {
            if m < self.nv {
                out[m] = b[m] + mul(d[m], u);
            }
        }
        out
    }

    /// `AddFails(b, d, u)`.
    fn add_fails(&self, b: &[i64; NM], d: &[i64; NM], u: u32) -> bool {
        let mut any = false;
        for m in 0..NM {
            if m < self.nv && !bounded(b[m] + mul(d[m], u)) {
                any = true;
            }
        }
        any
    }

    /// `Neg(d)`.
    fn neg(&self, d: &[i64; NM]) -> [i64; NM] {
        let mut out = [0; NM];
        for m in 0..NM {
            if m < self.nm {
                out[m] = -d[m];
            }
        }
        out
    }

    /// `<<OpenSum(m, skip, TRUE), OpenSum(m, skip, FALSE)>>`: both sums in
    /// one pass over the records (the same function as two calls). Record
    /// `k` has id `firstId + k`; `skip` is an id (-1 for None).
    fn open_sums(&self, m: usize, skip: i64) -> (i64, i64) {
        let (mut lo, mut hi) = (0, 0);
        for k in 0..NC {
            if k < self.nrec {
                let c = &self.records[k];
                if open(c.status) && (self.first_id + k as u64) as i64 != skip {
                    let (l, h) = terms(&self.rcp(c.recipe), c.units, m);
                    lo += l;
                    hi += h;
                }
            }
        }
        (lo, hi)
    }

    /// `<<ExtraPart(er, eu, m, TRUE), ExtraPart(er, eu, m, FALSE)>>`.
    fn extra_parts(&self, er: u32, eu: u32, m: usize) -> (i64, i64) {
        if eu == 0 {
            (0, 0)
        } else {
            terms(&self.rcp(er), eu, m)
        }
    }

    /// `CheckBounds(cand, er, eu, skip)`: Bounds if a metric's envelope
    /// violation exceeds the smaller of its violation now and at `cand`.
    fn check_bounds(&self, cand: &[i64; NM], er: u32, eu: u32, skip: i64) -> TErr {
        let mut lo = [0; NM];
        let mut hi = [0; NM];
        for m in 0..NM {
            if m < self.nm {
                let (ol, oh) = self.open_sums(m, skip);
                let (el, eh) = self.extra_parts(er, eu, m);
                // Low(cand, er, eu, skip, m), High(cand, er, eu, skip, m)
                lo[m] = cand[m] + ol + el;
                hi[m] = cand[m] + oh + eh;
            }
        }
        let mut arith = false;
        for m in 0..NM {
            if m < self.nm && (!bounded(lo[m]) || !bounded(hi[m])) {
                arith = true;
            }
        }
        if arith {
            return TErr::Arithmetic;
        }
        let mut bounds = false;
        for m in 0..NM {
            if m < self.nm {
                let x = &self.metrics[m];
                if max0(x.min - lo[m]) + max0(hi[m] - x.max) > min(self.pv(&self.values, m), self.pv(cand, m)) {
                    bounds = true;
                }
            }
        }
        if bounds {
            TErr::Bounds
        } else {
            TErr::Ok
        }
    }

    /// `Energy(vals)`: the sum over the terms of `weight * r * r`, `r =
    /// TermR(vals, t)`. Each term is the product `(r * r) * weight` in `u64`
    /// after asserting `r <= 2e6` and `weight <= 1000`, so it is exact (no
    /// wrap below 4e15) and the same circuit as the code's (RESULTS.md,
    /// deviation 9).
    fn energy(&self, vals: &[i64; NM]) -> u64 {
        let mut e: u64 = 0;
        for k in 0..NT {
            if k < self.nt {
                let t = &self.terms[k];
                // TermR(vals, t)
                let d = vals[t.metric as usize] - t.target;
                let r = match t.shape {
                    Sh::Target => {
                        if d < 0 {
                            -d
                        } else {
                            d
                        }
                    }
                    Sh::Excess => max0(d),
                    Sh::Deficit => max0(-d),
                };
                assert!(r <= 2 * MAX_VALUE && t.weight <= 1000, "REF tla: r <= 2e6, weight <= MaxWeight");
                let r = r as u64;
                e += r.wrapping_mul(r).wrapping_mul(t.weight as u64);
            }
        }
        e
    }

    /// `VerErr` (bump).
    fn ver_err(&self) -> TErr {
        if self.version >= MAX_VERSION {
            TErr::Arithmetic
        } else {
            TErr::Ok
        }
    }

    /// `CapErr` (bump_new): the cap, then `VerErr`.
    fn cap_err(&self) -> TErr {
        if self.version >= VERSION_CAP {
            TErr::Arithmetic
        } else {
            self.ver_err()
        }
    }

    /// `SnapExpired(now)`.
    fn snap_expired(&self, now: u64) -> bool {
        self.max_age > 0 && now as u128 > self.observed_at as u128 + self.max_age as u128
    }

    /// `SubmitErr(c, r, u, v, exp, now)`.
    pub fn submit_err(&self, c: &str, r: u32, u: u32, v: u32, exp: u64, now: u64) -> TErr {
        if self.paused {
            return TErr::Paused;
        }
        if v as u64 != self.version {
            return TErr::Stale;
        }
        if exp <= now || exp as u128 > now as u128 + WINDOW as u128 {
            return TErr::Expired;
        }
        if self.snap_expired(now) {
            return TErr::SnapshotExpired;
        }
        if self.win_full() {
            return TErr::Full;
        }
        if r as usize >= self.nrcp {
            return TErr::InvalidRecipe;
        }
        let x = self.rcp(r);
        if !eq_str(c, x.actor) {
            return TErr::Unauthorized;
        }
        if u == 0 || u > x.max_units {
            return TErr::InvalidUnits;
        }
        let mut unmet = false;
        for q in 0..NQ {
            if q < x.nq {
                let (p, units) = x.requires[q];
                // doneU[p + 1] < units
                if self.done_u[p as usize] < units as u64 {
                    unmet = true;
                }
            }
        }
        if unmet {
            return TErr::Prerequisite;
        }
        if self.add_fails(&self.values, &x.reserve, u) {
            return TErr::Arithmetic;
        }
        let next = self.add_vec(&self.values, &x.reserve, u);
        let cb = self.check_bounds(&next, r, u, -1);
        if cb != TErr::Ok {
            return cb;
        }
        let e0 = self.energy(&self.values);
        if self.energy(&next) > e0 {
            return TErr::Energy;
        }
        let violating = self.violating();
        if violating && self.add_fails(&next, &x.complete, u) {
            return TErr::Arithmetic;
        }
        if violating && self.energy(&self.add_vec(&next, &x.complete, u)) >= e0 {
            return TErr::Energy;
        }
        if self.cap_err() != TErr::Ok {
            return TErr::Arithmetic;
        }
        // submit's id = first_id + len (U:363-366)
        if self.first_id + self.nrec as u64 > MAX_VERSION {
            return TErr::Arithmetic;
        }
        TErr::Ok
    }

    /// `DispatchErr(c, id, now)`.
    pub fn dispatch_err(&self, c: &str, id: u32, now: u64) -> TErr {
        if !self.found(id) {
            return TErr::NotFound;
        }
        if !eq_str(c, self.rec_rcp(id).actor) {
            return TErr::Unauthorized;
        }
        if self.paused {
            return TErr::Paused;
        }
        let x = self.rec(id);
        if x.status != St::Reserved {
            return TErr::InvalidState;
        }
        if now >= x.exp {
            return TErr::Expired;
        }
        if self.snap_expired(now) {
            return TErr::SnapshotExpired;
        }
        self.cap_err()
    }

    /// `AckErr(id, acc)`.
    pub fn ack_err(&self, id: u32, acc: bool) -> TErr {
        if !self.found(id) {
            return TErr::NotFound;
        }
        let s = self.rec(id).status;
        if s != St::Dispatched && s != St::Unknown {
            return TErr::Noop;
        }
        if s == ack_to(acc) {
            return TErr::Noop;
        }
        self.ver_err()
    }

    /// `ReleaseErr(id)`: the envelope skips the released record.
    fn release_err(&self, id: u32) -> TErr {
        let d = self.neg(&self.rec_rcp(id).reserve);
        let u = self.rec(id).units;
        if self.add_fails(&self.values, &d, u) {
            return TErr::Arithmetic;
        }
        let cb = self.check_bounds(&self.add_vec(&self.values, &d, u), 0, 0, id as i64);
        if cb != TErr::Ok {
            return cb;
        }
        self.ver_err()
    }

    /// `CancelErr(c, id, now)` (`now` is unused, as in the core).
    pub fn cancel_err(&self, c: &str, id: u32) -> TErr {
        if !self.found(id) {
            return TErr::NotFound;
        }
        if !eq_str(c, self.rec_rcp(id).actor) && !eq_str(c, self.owner) {
            return TErr::Unauthorized;
        }
        let s = self.rec(id).status;
        if s == St::Cancelled {
            return TErr::Noop;
        }
        if s == St::Completed {
            return TErr::InvalidState;
        }
        if s != St::Reserved {
            return TErr::Pending;
        }
        self.release_err(id)
    }

    /// `SettleErr(c, id, done)`.
    pub fn settle_err(&self, c: &str, id: u32, done: bool) -> TErr {
        if !self.found(id) {
            return TErr::NotFound;
        }
        if !eq_str(c, self.rec_rcp(id).executor) {
            return TErr::Unauthorized;
        }
        let s = self.rec(id).status;
        if (done && s == St::Completed) || (!done && s == St::Cancelled) {
            return TErr::Noop;
        }
        if !in_flight(s) {
            return TErr::InvalidState;
        }
        if !done {
            return self.release_err(id);
        }
        let d = &self.rec_rcp(id).complete;
        let u = self.rec(id).units;
        if self.add_fails(&self.values, d, u) {
            return TErr::Arithmetic;
        }
        let cb = self.check_bounds(&self.add_vec(&self.values, d, u), 0, 0, id as i64);
        if cb != TErr::Ok {
            return cb;
        }
        self.ver_err()
    }

    /// `PauseErr(c, b)`.
    pub fn pause_err(&self, c: &str) -> TErr {
        if !eq_str(c, self.owner) {
            return TErr::Unauthorized;
        }
        self.cap_err()
    }

    /// `vals # values` as TLA+ sequences (another length, or some element).
    fn differs(&self, vals: &[i64]) -> bool {
        let mut d = vals.len() != self.nv;
        for k in 0..NM {
            if k < self.nv && k < vals.len() && vals[k] != self.values[k] {
                d = true;
            }
        }
        d
    }

    /// `ObserveErr(c, vals, now)`: Pending only if the values change while
    /// a record is open. `Len(vals) # NM \/ \E k : ~Bounded(vals[k])`: the
    /// elements past the capacity need no test, as the length test is then
    /// true already (`nm <= NM`).
    pub fn observe_err(&self, c: &str, vals: &[i64]) -> TErr {
        if !eq_str(c, self.owner) {
            return TErr::Unauthorized;
        }
        if self.differs(vals) && self.any_open() {
            return TErr::Pending;
        }
        let mut bad = vals.len() != self.nm;
        for k in 0..NM {
            if k < vals.len() && !bounded(vals[k]) {
                bad = true;
            }
        }
        if bad {
            return TErr::Bounds;
        }
        self.cap_err()
    }

    /// The error operator of `op`'s entry point at this state.
    pub fn err(&self, op: &Op) -> TErr {
        match op {
            Op::Submit { caller, req, now } => {
                self.submit_err(caller, req.recipe, req.units, req.version, req.expires_at, *now)
            }
            Op::Dispatch { caller, id, now } => self.dispatch_err(caller, *id, *now),
            Op::Ack { id, accepted } => self.ack_err(*id, *accepted),
            Op::Cancel { caller, id, .. } => self.cancel_err(caller, *id),
            Op::Settle {
                caller,
                id,
                completed,
            } => self.settle_err(caller, *id, *completed),
            Op::Pause { caller, .. } => self.pause_err(caller),
            Op::Observe { caller, vals, .. } => self.observe_err(caller, vals),
        }
    }

    // -----------------------------------------------------------------------
    // Actions: the effect of an "Ok" call
    // -----------------------------------------------------------------------

    /// `Contrib(c, m)`: a record's contribution to `values` since its
    /// admission (its reservation unless Cancelled, plus its completion).
    fn contrib(&self, c: &TRec, m: usize) -> i64 {
        if c.status == St::Cancelled {
            0
        } else {
            let x = self.rcp(c.recipe);
            mul(x.reserve[m], c.units) + if c.status == St::Completed { mul(x.complete[m], c.units) } else { 0 }
        }
    }

    /// `Release(id)`'s effect on `u` (from this state).
    fn release(&self, u: &mut Self, id: u32) {
        let x = self.rec(id);
        u.values = self.add_vec(&self.values, &self.neg(&self.rec_rcp(id).reserve), x.units);
        u.records[self.pos(id)].status = St::Cancelled;
        u.version = self.version + 1;
    }

    /// The post-state of `op`'s action (`Submit`, `Dispatch`, `Ack`,
    /// `Cancel`, `Settle`, `SetPaused`, `Observe`) without `steps` and
    /// `last`. Only meaningful when `self.err(op)` is "Ok".
    pub fn next(&self, op: &Op) -> Self {
        let mut u = *self;
        match op {
            Op::Submit { req, .. } => {
                let new = TRec {
                    recipe: req.recipe,
                    units: req.units,
                    exp: req.expires_at,
                    status: St::Reserved,
                };
                if self.nrec >= MAX_RECORDS {
                    // prune: records' = Append(Tail(records), new),
                    // firstId' = firstId + 1; a counted pruned record
                    // (k0 <= firstId) is folded into base, k0' = firstId + 1.
                    for k in 0..NC {
                        if k + 1 < self.nrec {
                            u.records[k] = self.records[k + 1];
                        }
                    }
                    u.records[self.nrec - 1] = new;
                    u.first_id = self.first_id + 1;
                    if self.k0 <= self.first_id {
                        for m in 0..NM {
                            if m < self.nm {
                                u.base[m] = self.base[m] + self.contrib(&self.records[0], m);
                            }
                        }
                        u.nb = self.nm;
                        u.k0 = self.first_id + 1;
                    }
                } else {
                    u.records[self.nrec] = new;
                    u.nrec = self.nrec + 1;
                }
                u.values = self.add_vec(&self.values, &self.rcp(req.recipe).reserve, req.units);
                u.version = self.version + 1;
            }
            Op::Dispatch { id, .. } => {
                u.records[self.pos(*id)].status = St::Dispatched;
                u.version = self.version + 1;
            }
            Op::Ack { id, accepted } => {
                u.records[self.pos(*id)].status = ack_to(*accepted);
                u.version = self.version + 1;
            }
            Op::Cancel { id, .. } => self.release(&mut u, *id),
            Op::Settle { id, completed, .. } => {
                if *completed {
                    let x = self.rec(*id);
                    u.values = self.add_vec(&self.values, &self.rec_rcp(*id).complete, x.units);
                    u.records[self.pos(*id)].status = St::Completed;
                    // doneU[r + 1] = Min(@ + units, DoneCap)
                    let r = x.recipe as usize;
                    let s = self.done_u[r] + x.units as u64;
                    u.done_u[r] = if s < DONE_CAP { s } else { DONE_CAP };
                    u.version = self.version + 1;
                } else {
                    self.release(&mut u, *id);
                }
            }
            Op::Pause { paused, .. } => {
                u.paused = *paused;
                u.version = self.version + 1;
            }
            Op::Observe { vals, now, .. } => {
                // values' = vals (Len(vals) = NM <= capacity when the
                // action is enabled), observedAt' = now; the refresh (vals
                // = values with a record open) keeps base and k0, any other
                // observe sets base' = vals, k0' = firstId + NR.
                let refresh = !self.differs(vals) && self.any_open();
                u.nv = vals.len();
                for i in 0..NM {
                    if i < vals.len() {
                        u.values[i] = vals[i];
                    }
                }
                if !refresh {
                    u.nb = vals.len();
                    for i in 0..NM {
                        if i < vals.len() {
                            u.base[i] = vals[i];
                        }
                    }
                    u.k0 = self.first_id + self.nrec as u64;
                }
                u.observed_at = *now;
                u.version = self.version + 1;
            }
        }
        u
    }
}

/// `a = b` as TLA+ values: field by field, up to the lengths (unused slots
/// are not compared), accounts with `eq_str`.
pub fn same<'a, const NM: usize, const NT: usize, const NR: usize, const NQ: usize, const NC: usize>(
    a: &T<'a, NM, NT, NR, NQ, NC>,
    b: &T<'a, NM, NT, NR, NQ, NC>,
) -> bool {
    let mut ok = eq_str(a.owner, b.owner)
        && a.max_age == b.max_age
        && a.nm == b.nm
        && a.nt == b.nt
        && a.nrcp == b.nrcp
        && a.nv == b.nv
        && a.version == b.version
        && a.observed_at == b.observed_at
        && a.paused == b.paused
        && a.nrec == b.nrec
        && a.first_id == b.first_id
        && a.nb == b.nb
        && a.k0 == b.k0;
    for i in 0..NM {
        if i < a.nm && a.metrics[i] != b.metrics[i] {
            ok = false;
        }
        if i < a.nv && a.values[i] != b.values[i] {
            ok = false;
        }
        if i < a.nb && a.base[i] != b.base[i] {
            ok = false;
        }
    }
    for k in 0..NT {
        if k < a.nt && a.terms[k] != b.terms[k] {
            ok = false;
        }
    }
    for r in 0..NR {
        if r < a.nrcp {
            let (x, y) = (&a.recipes[r], &b.recipes[r]);
            if !(eq_str(x.actor, y.actor) && eq_str(x.executor, y.executor))
                || x.max_units != y.max_units
                || x.nq != y.nq
                || a.done_u[r] != b.done_u[r]
            {
                ok = false;
            }
            for i in 0..NM {
                if i < a.nm && (x.reserve[i] != y.reserve[i] || x.complete[i] != y.complete[i]) {
                    ok = false;
                }
            }
            for q in 0..NQ {
                if q < x.nq && x.requires[q] != y.requires[q] {
                    ok = false;
                }
            }
        }
    }
    for j in 0..NC {
        if j < a.nrec && a.records[j] != b.records[j] {
            ok = false;
        }
    }
    ok
}

/// The implementation's ghost `(base, k0)` after a call ([`ghost_after`]),
/// in the image's capacity (`base` is `nb` long).
#[derive(Clone, Copy, Debug)]
pub struct TGhost<const NM: usize> {
    pub nb: usize,
    pub base: [i64; NM],
    pub k0: u64,
}

/// The implementation's ghost after the call `pre --op--> post` with result
/// `res`, from the ghost `(base, k0)` of `pre` (plan 2.2), read off the
/// core's pools, not the TLA+ state:
///
/// - an observe that returns `Ok` re-bases (`base` = the new values, `k0` =
///   the next id, `first_id + len`) unless it restated the current values
///   while some record was open (the refresh), which keeps both;
/// - a submit that returns `Ok` from a full window pruned the oldest
///   record; if that record was counted (`k0 <= its id`), its contribution
///   (its reservation unless Cancelled, plus its completion if Completed)
///   moves into `base` and `k0` passes it (`its id + 1`);
/// - every other call keeps both.
pub fn ghost_after<const NM: usize>(pre: &Pool, base: &[i64], k0: u64, op: &Op, res: &Res, post: &Pool) -> TGhost<NM> {
    let mut g = TGhost {
        nb: base.len(),
        base: [0; NM],
        k0,
    };
    assert!(base.len() <= NM && pre.values.len() <= NM, "REF ghost: within the image's capacity");
    for i in 0..NM {
        if i < base.len() {
            g.base[i] = base[i];
        }
    }
    match (op, res) {
        (Op::Observe { vals, .. }, Ok(_)) => {
            let mut same_vals = vals.len() == pre.values.len();
            for i in 0..NM {
                if i < pre.values.len() && i < vals.len() && vals[i] != pre.values[i] {
                    same_vals = false;
                }
            }
            let mut any_open = false;
            for c in &pre.records {
                if !matches!(c.status, Status::Completed | Status::Cancelled) {
                    any_open = true;
                }
            }
            if !(same_vals && any_open) {
                assert!(post.values.len() <= NM, "REF ghost: within the image's capacity");
                g.nb = post.values.len();
                for i in 0..NM {
                    if i < post.values.len() {
                        g.base[i] = post.values[i];
                    }
                }
                g.k0 = post.first_id as u64 + post.records.len() as u64;
            }
        }
        (Op::Submit { .. }, Ok(_)) if pre.records.len() >= pool_core::MAX_RECORDS => {
            let c = &pre.records[0];
            if k0 <= c.id as u64 {
                let r = &pre.config.recipes[c.recipe as usize];
                let u = c.units as i64;
                for i in 0..NM {
                    if i < g.nb && c.status != Status::Cancelled {
                        g.base[i] += r.reserve[i] * u;
                        if c.status == Status::Completed {
                            g.base[i] += r.complete[i] * u;
                        }
                    }
                }
                g.k0 = c.id as u64 + 1;
            }
        }
        _ => {}
    }
    g
}

/// REF-1..3 for one call of the core: `pre --op--> post` with result `res`,
/// from the ghost `(base, k0)` of `pre` (the TLA+ ghost: pruned records
/// folded into `base`, `k0` an id; a caller that replays a sequence takes
/// the next ghost from [`ghost_after`]). Returns the TLA+ error operator's
/// value (for the covers).
pub fn assert_refines<'a, const NM: usize, const NT: usize, const NR: usize, const NQ: usize, const NC: usize>(
    pre: &'a Pool,
    base: &'a [i64],
    k0: impl Into<u64>,
    op: &Op,
    res: &Res,
    post: &'a Pool,
) -> TErr {
    let k0: u64 = k0.into();
    let t: T<'a, NM, NT, NR, NQ, NC> = alpha(pre, base, k0);
    let want = t.err(op);
    // PROP: REF-1
    let agree = match (res, want) {
        (Ok(_), TErr::Ok | TErr::Noop) => true,
        (Err(e), w) => code(e) == Some(w),
        _ => false,
    };
    assert!(agree, "REF-1 guard exactness: the core's result is the TLA+ error operator's");
    let g2: TGhost<NM> = ghost_after(pre, base, k0, op, res, post);
    let after: T<'a, NM, NT, NR, NQ, NC> = alpha(post, &g2.base[..g2.nb], g2.k0);
    if want == TErr::Ok {
        // PROP: REF-2
        assert!(same(&after, &t.next(op)), "REF-2 effect exactness: alpha(post) is the TLA+ action's post-state");
        if matches!(op, Op::Submit { .. }) {
            assert!(
                *res == Ok(Some((t.first_id + t.nrec as u64) as u32)),
                "REF-2 effect exactness: submit returns the new record's id firstId + NR"
            );
        }
    } else {
        // PROP: REF-3
        assert!(same(&after, &t), "REF-3 rejected or idempotent call stutters: alpha(post) = alpha(pre)");
    }
    want
}
