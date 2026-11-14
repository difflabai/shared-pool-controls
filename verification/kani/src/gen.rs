//! Bounded state and operation builders.
//!
//! Bounds (RESULTS.md, "Bounds"): at most 2 metrics, 2 terms, 2 recipes and
//! 3 records; accounts from [`ACCTS`]; values from the harness's [`Dom`].
//! Times, versions, ids, request recipes and units are free (`u64`/`u32`);
//! they are only compared, so leaving them free costs little.
//!
//! `inv_state` builds a state together with its ghost `(base, k0)` and fixes
//! the values by INV-LEDGER (`values = ledger(base, records, k0)`): under
//! the assumption `inv` this loses nothing, and it is far cheaper than
//! drawing the values and assuming the equation. INV states have
//! `first_id = 0` and `completed_units` = the retained Completed units
//! (exact: fewer than 256 records means nothing was pruned, WIN-1; K-D2).
//! `wf_state` builds well-formed states that are not reachable-closed
//! (free `first_id`, `completed_units` and version; K-D2), for the paths
//! INV makes dead. `raw_config` builds any configuration of bounded size
//! (for `validate` and `new`).

use crate::spec::{self, Ghost, Op, ACCTS};
use crate::src::{assume, Cfg, Dom, Src, Ver};
use pool_core::*;

pub const MAX_M: u8 = 2;
pub const MAX_T: u8 = 2;
pub const MAX_R: u8 = 2;
pub const MAX_C: u8 = 3;

pub fn acct(src: &mut impl Src) -> &'static str {
    let k = src.below(4);
    acct_at(k)
}

/// `ACCTS[k % 4]`, as a choice among the constant strings.
pub fn acct_at(k: u32) -> &'static str {
    match k % 4 {
        0 => ACCTS[0],
        1 => ACCTS[1],
        2 => ACCTS[2],
        _ => ACCTS[3],
    }
}

/// A value from the domain `d` (always within ±1e6).
pub fn val(src: &mut impl Src, d: Dom) -> i64 {
    match d {
        Dom::Small(k) => src.range(-k, k),
        Dom::Anchored(k) => {
            let a = match src.anchor() {
                0 => -1_000_000,
                1 => 0,
                _ => 1_000_000,
            };
            let v = a + src.range(-k, k);
            if v > 1_000_000 {
                1_000_000
            } else if v < -1_000_000 {
                -1_000_000
            } else {
                v
            }
        }
        Dom::Full => src.range(-1_000_000, 1_000_000),
    }
}

/// A weight: any `1..=1000`, or one of `{1, 2, 1000}`.
pub fn weight(src: &mut impl Src, any: bool) -> u32 {
    if any {
        1 + src.below(1000)
    } else {
        match src.below(3) {
            0 => 1,
            1 => 2,
            _ => 1000,
        }
    }
}

pub fn shape_of(src: &mut impl Src) -> Shape {
    match src.below(3) {
        0 => Shape::Target,
        1 => Shape::Excess,
        _ => Shape::Deficit,
    }
}

pub fn status(src: &mut impl Src) -> Status {
    match src.below(6) {
        0 => Status::Reserved,
        1 => Status::Dispatched,
        2 => Status::Active,
        3 => Status::Unknown,
        4 => Status::Completed,
        _ => Status::Cancelled,
    }
}

fn size(src: &mut impl Src, fixed: Option<u8>, max: u8, min: u8) -> usize {
    match fixed {
        Some(k) => k as usize,
        None => (min as u32 + src.below((max - min) as u32 + 1)) as usize,
    }
}

/// A valid configuration within the bounds (CFG holds by construction;
/// `h_init` and `h_validate` cover arbitrary ones).
pub fn config(src: &mut impl Src, c: &Cfg) -> Config {
    let nm = size(src, c.shape.metrics, MAX_M, 1);
    let nt = size(src, c.shape.terms, MAX_T, 1);
    let nr = size(src, c.shape.recipes, MAX_R, 1);
    let mut metrics = Vec::with_capacity(MAX_M as usize);
    for i in 0..MAX_M as usize {
        if i < nm {
            let lo = val(src, c.val);
            let hi = val(src, c.val);
            let lo2 = if lo <= hi { lo } else { hi };
            let hi2 = if lo <= hi { hi } else { lo };
            metrics.push(Metric {
                name: "m".into(),
                unit: "u".into(),
                // Only `new` reads `initial`; see `init_config` for that case.
                initial: lo2,
                min: lo2,
                max: hi2,
            });
        }
    }
    let mut terms = Vec::with_capacity(MAX_T as usize);
    for i in 0..MAX_T as usize {
        if i < nt {
            terms.push(Term {
                metric: src.below(nm as u32),
                target: val(src, c.val),
                weight: weight(src, c.any_weight),
                shape: shape_of(src),
            });
        }
    }
    let mut recipes = Vec::with_capacity(MAX_R as usize);
    for i in 0..MAX_R as usize {
        if i < nr {
            let mut reserve = Vec::with_capacity(MAX_M as usize);
            let mut complete = Vec::with_capacity(MAX_M as usize);
            for k in 0..MAX_M as usize {
                if k < nm {
                    reserve.push(val(src, c.delta));
                    complete.push(val(src, c.delta));
                }
            }
            let mut requires = Vec::with_capacity(1);
            if i > 0 && src.bool() {
                let p = src.below(i as u32);
                let cap = if src.hint(70) { 4 } else { 100 };
                requires.push((p, 1 + src.below(cap)));
            }
            // CFG-2 (v1.1): the executor is never the actor; it may be the
            // owner (Q3).
            let a = src.below(4);
            let e = a + 1 + src.below(3);
            recipes.push(Recipe {
                name: "r".into(),
                actor: acct_at(a).into(),
                executor: acct_at(e).into(),
                reserve,
                complete,
                max_units: 1 + src.below(c.max_units),
                requires,
            });
        }
    }
    let max_age_seconds = if src.hint(50) { 0 } else { src.u32() };
    Config {
        name: "p".into(),
        owner: acct(src).into(),
        metrics,
        terms,
        recipes,
        max_age_seconds,
    }
}

/// `Pool::new(cfg, 0)` for a valid `cfg` (its `Ok` value by CFG-1), without
/// the validation.
pub fn pool_of(cfg: Config) -> Pool {
    let mut values = Vec::with_capacity(MAX_M as usize);
    for i in 0..MAX_M as usize {
        if i < cfg.metrics.len() {
            values.push(cfg.metrics[i].initial);
        }
    }
    let mut completed_units = Vec::with_capacity(MAX_R as usize + 1);
    for r in 0..MAX_R as usize + 1 {
        if r < cfg.recipes.len() {
            completed_units.push(0);
        }
    }
    Pool {
        config: cfg,
        values,
        version: 0,
        observed_at: 0,
        paused: false,
        records: Vec::new(),
        first_id: 0,
        completed_units,
    }
}

/// A valid configuration whose initial values are free within `[min, max]`.
pub fn init_config(src: &mut impl Src, c: &Cfg) -> Config {
    let mut cfg = config(src, c);
    for m in cfg.metrics.iter_mut() {
        let x = val(src, c.val);
        let x = if x < m.min {
            m.min
        } else if x > m.max {
            m.max
        } else {
            x
        };
        m.initial = x;
    }
    cfg
}

/// The records and ghost of a generated state: `nc` records with ids `0..nc`
/// (`first_id = 0`), the first `k0` terminal; values by INV-LEDGER;
/// `completed_units` and the ghost's `done` the retained Completed units;
/// `version >= nc` (WIN-1), and `>= VERSION_CAP` with `Ver::AtCap`. `None`
/// if the values leave ±1e6.
pub fn state_from(src: &mut impl Src, c: &Cfg, cfg: Config) -> Option<(Pool, Ghost)> {
    let nm = cfg.metrics.len();
    let nr = cfg.recipes.len() as u32;
    let mut base = Vec::with_capacity(MAX_M as usize);
    for i in 0..MAX_M as usize {
        if i < nm {
            base.push(val(src, c.val));
        }
    }
    let nc = size(src, c.shape.records, MAX_C, 0);
    let k0 = src.below(nc as u32 + 1);
    // All MAX_C records are written at concrete offsets, then truncated: a
    // conditional push writes at a symbolic offset (RESULTS.md, deviation 3).
    let mut records = Vec::with_capacity(MAX_C as usize + 1);
    for j in 0..MAX_C as usize {
        let recipe = src.below(nr);
        let units = 1 + src.below(spec::rcp_of(&cfg, recipe).max_units);
        let st = if (j as u32) < k0 {
            if src.bool() {
                Status::Completed
            } else {
                Status::Cancelled
            }
        } else {
            status(src)
        };
        records.push(Commitment {
            id: j as u32,
            recipe,
            units,
            expires_at: src.u64(),
            status: st,
        });
    }
    records.truncate(nc);
    let l = spec::ledger(&cfg, &base, &records, k0);
    let mut values = Vec::with_capacity(MAX_M as usize);
    for i in 0..MAX_M as usize {
        if i < nm {
            if !assume(spec::bounded(l[i])) {
                return None;
            }
            values.push(l[i] as i64);
        }
    }
    let mut completed_units = Vec::with_capacity(MAX_R as usize + 1);
    let mut done = [0u64; spec::GR];
    for r in 0..MAX_R as usize {
        if r < nr as usize {
            let d = spec::retained_done(&records, r as u32);
            completed_units.push(d as u32);
            done[r] = d;
        }
    }
    let g = Ghost {
        base,
        k0,
        eff: None,
        pruned: [0; spec::M],
        done,
    };
    let version = match c.ver {
        Ver::Max => u32::MAX,
        // Plain mode: mostly just at or above the cap with `AtCap`, and
        // sometimes with `Free` (Kani: free).
        Ver::AtCap if src.hint(80) => spec::VERSION_CAP + src.below(16),
        Ver::Free if src.hint(10) => spec::VERSION_CAP - 8 + src.below(24),
        _ => src.u32(),
    };
    // WIN-1: every record was a submit, an effective transition.
    if !assume(version as u64 >= nc as u64) {
        return None;
    }
    if c.ver == Ver::AtCap && !assume(version >= spec::VERSION_CAP) {
        return None;
    }
    let p = Pool {
        config: cfg,
        values,
        version,
        observed_at: src.u64(),
        paused: src.bool(),
        records,
        first_id: 0,
        completed_units,
    };
    Some((p, g))
}

/// An INV state within the bounds, with its ghost: in plain mode often a
/// random-walk state from `Pool::new`; otherwise built and filtered by
/// `assume(inv)`.
pub fn inv_state(src: &mut impl Src, c: &Cfg) -> Option<(Pool, Ghost)> {
    if c.ver == Ver::Free {
        if let Some((p, mut g)) = src.reach() {
            g.eff = None;
            return Some((p, g));
        }
    }
    let cfg = config(src, c);
    let (p, g) = state_from(src, c, cfg)?;
    // WIN-1, INV-LEDGER and INV-CU hold by construction; the rest is
    // assumed (K-D6).
    if !assume(spec::inv_res(&p))
        || !assume(spec::inv_pv(&p, &g))
        || !assume(spec::inv_vercap(&p))
        || !assume(spec::inv_dep(&p, &g))
    {
        return None;
    }
    #[cfg(not(kani))]
    assert!(spec::inv(&p, &g), "gen: state is not INV");
    Some((p, g))
}

/// Records of a WF state (`wf_state`).
pub const WF_C: u8 = 2;

/// A well-formed state that need not be reachable (K-D2, the WF domain): a
/// valid configuration; up to [`WF_C`] records with ids `first_id + j` and
/// free statuses, recipes and units; `first_id` free with
/// `first_id + len <= 2^32` (so the id overflow of submit is reachable) or
/// small; `completed_units` free (one per recipe); values free and bounded
/// (not ledger-derived); the version free, or `u32::MAX` with `Ver::Max`.
/// The ghost is a placeholder (`eff: None`): only the `_tx`/`_post` checks
/// run on WF states, and in plain mode REF-1..3 with the placeholder as the
/// TLA+ ghost (`checks::wf_step`). It exercises the paths INV makes dead:
/// `index` with `first_id > 0`, pruned ids, the id overflow, the plain
/// increment at `u32::MAX`, a release that fails Bounds or Arithmetic, the
/// saturating count.
pub fn wf_state(src: &mut impl Src, c: &Cfg) -> Option<(Pool, Ghost)> {
    let cfg = config(src, c);
    let nm = cfg.metrics.len();
    let nr = cfg.recipes.len() as u32;
    let first_id = if src.bool() { src.below(4) } else { src.u32() };
    let len = size(src, c.shape.records.map(|k| if k > WF_C { WF_C } else { k }), WF_C, 0);
    if !assume(first_id as u64 + len as u64 <= u32::MAX as u64 + 1) {
        return None;
    }
    let mut records = Vec::with_capacity(WF_C as usize + 1);
    for j in 0..WF_C as usize {
        let recipe = src.below(nr);
        let units = 1 + src.below(spec::rcp_of(&cfg, recipe).max_units);
        records.push(Commitment {
            id: first_id.wrapping_add(j as u32),
            recipe,
            units,
            expires_at: src.u64(),
            status: status(src),
        });
    }
    records.truncate(len);
    let mut values = Vec::with_capacity(MAX_M as usize);
    for i in 0..MAX_M as usize {
        if i < nm {
            values.push(val(src, c.val));
        }
    }
    let mut completed_units = Vec::with_capacity(MAX_R as usize + 1);
    for r in 0..MAX_R as usize {
        if r < nr as usize {
            let x = if src.hint(50) { src.below(4) } else { src.u32() };
            completed_units.push(x);
        }
    }
    let version = match c.ver {
        Ver::Max => u32::MAX,
        _ => src.u32(),
    };
    let g = Ghost {
        base: copy_vec(&values, MAX_M as usize + 1),
        k0: first_id,
        eff: None,
        pruned: [0; spec::M],
        done: [0; spec::GR],
    };
    let p = Pool {
        config: cfg,
        values,
        version,
        observed_at: src.u64(),
        paused: src.bool(),
        records,
        first_id,
        completed_units,
    };
    Some((p, g))
}

// ---------------------------------------------------------------------------
// Fixed-capacity copies
// ---------------------------------------------------------------------------

/// `v.clone()` into a buffer of capacity `cap` (≥ `v.len()` at every call
/// site). `Clone` allocates exactly `len`, and a symbolic `len` is an
/// allocation of symbolic size, which CBMC encodes with the array theory at
/// every later read and write (RESULTS.md, cost ladder). Same value as
/// `clone`; only the capacity differs, and `Pool`'s equality ignores it.
pub fn copy_vec<T: Clone>(v: &[T], cap: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(cap);
    for x in v {
        out.push(x.clone());
    }
    out
}

/// `cfg.clone()` with fixed capacities (see [`copy_vec`]).
pub fn copy_config(cfg: &Config) -> Config {
    let mut recipes = Vec::with_capacity(MAX_R as usize + 1);
    for r in &cfg.recipes {
        recipes.push(Recipe {
            name: r.name.clone(),
            actor: r.actor.clone(),
            executor: r.executor.clone(),
            reserve: copy_vec(&r.reserve, MAX_M as usize + 1),
            complete: copy_vec(&r.complete, MAX_M as usize + 1),
            max_units: r.max_units,
            requires: copy_vec(&r.requires, 2),
        });
    }
    Config {
        name: cfg.name.clone(),
        owner: cfg.owner.clone(),
        metrics: copy_vec(&cfg.metrics, MAX_M as usize + 1),
        terms: copy_vec(&cfg.terms, MAX_T as usize + 1),
        recipes,
        max_age_seconds: cfg.max_age_seconds,
    }
}

/// `p.clone()` with fixed capacities; `cap` bounds the record count.
pub fn copy_pool(p: &Pool, cap: usize) -> Pool {
    Pool {
        config: copy_config(&p.config),
        values: copy_vec(&p.values, MAX_M as usize + 1),
        version: p.version,
        observed_at: p.observed_at,
        paused: p.paused,
        records: copy_vec(&p.records, cap),
        first_id: p.first_id,
        completed_units: copy_vec(&p.completed_units, MAX_R as usize + 1),
    }
}

/// [`copy_pool`] for the generated states (at most `MAX_C` records, plus
/// one push).
pub fn copy(p: &Pool) -> Pool {
    copy_pool(p, MAX_C as usize + 1)
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

pub const SUBMIT: u32 = 0;
pub const DISPATCH: u32 = 1;
pub const ACK: u32 = 2;
pub const CANCEL: u32 = 3;
pub const SETTLE: u32 = 4;
pub const PAUSE: u32 = 5;
pub const OBSERVE: u32 = 6;
pub const KINDS: u32 = 7;

/// A time: free, or (plain mode) near `observed_at` / an expiry.
fn time(src: &mut impl Src, p: &Pool) -> u64 {
    if src.hint(70) {
        p.observed_at.saturating_add(src.below(3) as u64)
    } else {
        src.u64()
    }
}

/// A record id: one that exists (plain mode, mostly), any `u32` (plain
/// mode), or `first_id + 0..8` (Kani: at most 4 records, so `+4..8` are
/// ids not yet issued, NotFound), or with `first_id > 0` one of the two ids
/// below it (pruned, NotFound; WF states).
fn id(src: &mut impl Src, p: &Pool) -> u32 {
    let f = p.first_id;
    if !p.records.is_empty() && src.hint(85) {
        f.wrapping_add(src.below(p.records.len() as u32))
    } else if src.wide() {
        src.u32()
    } else {
        let k = src.below(8);
        if f > 0 && src.bool() {
            f.saturating_sub(1 + (k & 1))
        } else {
            f.wrapping_add(k)
        }
    }
}

/// A caller: any account, or (plain mode) the role `want`.
fn caller(src: &mut impl Src, want: &str) -> &'static str {
    for a in ACCTS {
        if a == want {
            return a;
        }
    }
    acct(src)
}

fn record_role<'a>(p: &'a Pool, id: u32, executor: bool) -> &'a str {
    match spec::rec(p, id) {
        Some(c) => {
            let r = spec::rcp_of(&p.config, c.recipe);
            if executor {
                &r.executor
            } else {
                &r.actor
            }
        }
        None => "",
    }
}

/// An operation of kind `k` (one of the constants above) with free
/// arguments (Kani) or arguments steered toward passing guards (plain).
pub fn op(src: &mut impl Src, p: &Pool, k: u32, d: Dom) -> Op {
    match k {
        SUBMIT => {
            // Kani: recipe in 0..4 (2 recipes, so 2 and 3 are InvalidRecipe).
            let recipe = if src.hint(90) {
                src.below(p.config.recipes.len() as u32)
            } else if src.wide() {
                src.u32()
            } else {
                src.below(4)
            };
            let caller = if src.hint(70) {
                let actor = spec::rcp(&p.config, recipe).map(|r| r.actor.clone()).unwrap_or_default();
                caller(src, &actor)
            } else {
                acct(src)
            };
            let now = time(src, p);
            // Kani: units in 0..8 (max_units <= 3, so 0 and 4..8 are
            // InvalidUnits; units reach arithmetic only after that guard).
            let units = if src.hint(80) {
                1 + src.below(3)
            } else if src.wide() {
                src.u32()
            } else {
                src.below(8)
            };
            let version = if src.hint(85) { p.version } else { src.u32() };
            let expires_at = if src.hint(80) {
                now.saturating_add(1 + src.below(86_400) as u64)
            } else {
                src.u64()
            };
            Op::Submit {
                caller,
                req: Request {
                    recipe,
                    units,
                    version,
                    expires_at,
                },
                now,
            }
        }
        DISPATCH => {
            let id = id(src, p);
            let caller = if src.hint(70) {
                let actor = record_role(p, id, false).to_string();
                caller(src, &actor)
            } else {
                acct(src)
            };
            let now = if src.hint(60) {
                match spec::rec(p, id) {
                    Some(c) => c.expires_at.saturating_sub(1 + src.below(2) as u64),
                    None => src.u64(),
                }
            } else {
                time(src, p)
            };
            Op::Dispatch { caller, id, now }
        }
        ACK => Op::Ack {
            id: id(src, p),
            accepted: src.bool(),
        },
        CANCEL => {
            let id = id(src, p);
            let caller = if src.hint(70) {
                let want = if src.bool() {
                    record_role(p, id, false).to_string()
                } else {
                    p.config.owner.clone()
                };
                caller(src, &want)
            } else {
                acct(src)
            };
            Op::Cancel {
                caller,
                id,
                now: src.u64(),
            }
        }
        SETTLE => {
            let id = id(src, p);
            let caller = if src.hint(70) {
                let ex = record_role(p, id, true).to_string();
                caller(src, &ex)
            } else {
                acct(src)
            };
            Op::Settle {
                caller,
                id,
                completed: src.bool(),
            }
        }
        PAUSE => {
            let caller = if src.hint(70) {
                let owner = p.config.owner.clone();
                caller(src, &owner)
            } else {
                acct(src)
            };
            Op::Pause {
                caller,
                paused: src.bool(),
            }
        }
        _ => {
            let caller = if src.hint(70) {
                let owner = p.config.owner.clone();
                caller(src, &owner)
            } else {
                acct(src)
            };
            let nm = p.config.metrics.len();
            // The current values (the refresh, C16: allowed with records
            // open) need exact equality, which a free vector rarely meets.
            let same = src.bool() && !src.hint(60);
            let len = if src.hint(0) || src.bool() {
                nm
            } else {
                src.below(MAX_M as u32 + 2) as usize
            };
            let mut vals = Vec::with_capacity(MAX_M as usize + 1);
            for i in 0..MAX_M as usize + 1 {
                if same {
                    if i < p.values.len() {
                        vals.push(p.values[i]);
                    }
                } else if i < len {
                    let x = match src.below(8) {
                        0 => -1_000_001,
                        1 => 1_000_001,
                        2 => i64::MIN,
                        3 => i64::MAX,
                        _ => val(src, d),
                    };
                    let x = if src.hint(90) { val(src, d) } else { x };
                    vals.push(x);
                }
            }
            Op::Observe {
                caller,
                vals,
                now: src.u64(),
            }
        }
    }
}

/// Plain mode: a random walk of `steps` operations from `Pool::new` over a
/// random small configuration, with its ghost.
#[cfg(not(kani))]
pub fn random_walk(src: &mut crate::src::Rng, steps: usize) -> (Pool, Ghost) {
    let c = src.cfg();
    let cfg = init_config(src, &c);
    let now = src.u64() % 1000;
    let mut p = Pool::new(cfg, now).expect("valid config");
    let mut g = Ghost::init(&p);
    for _ in 0..steps {
        let k = src.below(KINDS);
        let o = op(src, &p, k, c.val);
        if p.records.len() >= MAX_C as usize && k == SUBMIT {
            continue;
        }
        let x = spec::expected(&p, &o);
        let pre = copy(&p);
        let _ = spec::apply(&mut p, &o);
        g = spec::ghost_step(&g, &pre, &o, &x);
    }
    (p, g)
}

// ---------------------------------------------------------------------------
// Raw configurations (h_validate, h_init)
// ---------------------------------------------------------------------------

/// A string for `short`: empty, a short account, or (with `long`) 128 bytes
/// (the longest allowed) or 129 (too long).
pub fn raw_str(src: &mut impl Src, long: bool) -> String {
    const LONG: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const MAXLEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    if src.hint(97) {
        return acct(src).into();
    }
    match src.below(8) {
        0 => String::new(),
        1 if long => LONG.into(),
        2 if long => MAXLEN.into(),
        _ => acct(src).into(),
    }
}

/// An `i64` for a raw configuration: in range, just outside, or extreme.
pub fn raw_i64(src: &mut impl Src, d: Dom) -> i64 {
    if src.hint(97) {
        return val(src, d);
    }
    match src.below(8) {
        0 => -1_000_001,
        1 => 1_000_001,
        2 => i64::MIN,
        3 => i64::MAX,
        _ => val(src, d),
    }
}

fn raw_u32(src: &mut impl Src, edges: &[u32]) -> u32 {
    if src.hint(90) {
        return edges[1];
    }
    if src.bool() {
        edges[src.below(edges.len() as u32) as usize]
    } else {
        src.u32()
    }
}

/// Most metrics (and values per delta vector) of a raw configuration:
/// `MAX_M`+1 in plain mode, `MAX_M` under Kani (RESULTS.md, deviation 23).
pub const RAW_M: u8 = if cfg!(kani) { MAX_M } else { MAX_M + 1 };

/// Any configuration with at most [`RAW_M`] metrics, `MAX_T` terms,
/// `MAX_R`+1 recipes and 2 requirements per recipe; every field free
/// (strings from [`raw_str`], 128 and 129 bytes long only with `long`).
pub fn raw_config(src: &mut impl Src, c: &Cfg, long: bool) -> Config {
    // Every slot is written at a concrete offset and the vectors are then
    // truncated to their drawn lengths: a conditional push writes at a
    // symbolic offset (deviation 3), and with them `h_init` and `h_validate`
    // ran out of 40 GB in the propositional encoding.
    let nm = size(src, None, RAW_M, 0);
    let nt = size(src, None, MAX_T, 0);
    let nr = size(src, None, MAX_R + 1, 0);
    let mut metrics = Vec::with_capacity(RAW_M as usize);
    for _ in 0..RAW_M as usize {
        metrics.push(Metric {
            name: raw_str(src, long),
            unit: raw_str(src, long),
            initial: raw_i64(src, c.val),
            min: raw_i64(src, c.val),
            max: raw_i64(src, c.val),
        });
    }
    metrics.truncate(nm);
    let mut terms = Vec::with_capacity(MAX_T as usize);
    for _ in 0..MAX_T as usize {
        terms.push(Term {
            metric: if src.hint(90) && nm > 0 { src.below(nm as u32) } else { raw_u32(src, &[0, 1, 2, 3]) },
            target: raw_i64(src, c.val),
            weight: raw_u32(src, &[0, 1, 1000, 1001]),
            shape: shape_of(src),
        });
    }
    terms.truncate(nt);
    let mut recipes = Vec::with_capacity(MAX_R as usize + 1);
    for i in 0..MAX_R as usize + 1 {
        let lr = if src.hint(95) { nm } else { size(src, None, RAW_M, 0) };
        let lc = if src.hint(95) { nm } else { size(src, None, RAW_M, 0) };
        let mut reserve = Vec::with_capacity(RAW_M as usize);
        let mut complete = Vec::with_capacity(RAW_M as usize);
        for _ in 0..RAW_M as usize {
            reserve.push(raw_i64(src, c.delta));
            complete.push(raw_i64(src, c.delta));
        }
        reserve.truncate(lr);
        complete.truncate(lc);
        let nq = size(src, None, 2, 0);
        let mut requires = Vec::with_capacity(2);
        for j in 0..2u32 {
            let p = if src.hint(80) && i > 0 { j % i as u32 } else { raw_u32(src, &[0, 1, 2]) };
            requires.push((p, raw_u32(src, &[0, 1, 100, 101])));
        }
        requires.truncate(nq);
        recipes.push(Recipe {
            name: raw_str(src, long),
            actor: raw_str(src, long),
            executor: raw_str(src, long),
            reserve,
            complete,
            max_units: raw_u32(src, &[0, 1, 100, 101]),
            requires,
        });
    }
    recipes.truncate(nr);
    Config {
        name: raw_str(src, long),
        owner: raw_str(src, long),
        metrics,
        terms,
        recipes,
        max_age_seconds: src.u32(),
    }
}

// ---------------------------------------------------------------------------
// Window walks (plain mode)
// ---------------------------------------------------------------------------

/// Plain mode: a configuration for walks long enough to fill and prune the
/// window (`checks::window_walk`, `checks::cap_walk`), after upstream's
/// `window_on_random_runs`: metric 0 in `[-L, L]` (`L` in `60..=150`) with
/// every recipe's effects in `-10..=10`; metric 1 in `[-100, 100]`, touched
/// by no recipe, usually the only term's metric (so admission is limited by
/// the hard bounds, not by energy). 2 or 3 recipes (`spec::GR`), a later one
/// sometimes requiring 1-3 completed units of an earlier one.
#[cfg(not(kani))]
pub fn window_config(src: &mut crate::src::Rng) -> Config {
    let l = 60 + src.below(91) as i64;
    let nr = 2 + src.below(2);
    let mut recipes = Vec::new();
    for i in 0..nr {
        let requires = if i > 0 && src.hint(60) {
            vec![(src.below(i), 1 + src.below(3))]
        } else {
            vec![]
        };
        let a = src.below(4);
        let e = a + 1 + src.below(3);
        recipes.push(Recipe {
            name: "r".into(),
            actor: acct_at(a).into(),
            executor: acct_at(e).into(),
            reserve: vec![src.range(-10, 10), 0],
            complete: vec![src.range(-10, 10), 0],
            max_units: 1 + src.below(3),
            requires,
        });
    }
    let m = |min, max| Metric {
        name: "m".into(),
        unit: "u".into(),
        initial: 0,
        min,
        max,
    };
    let term = if src.hint(80) {
        Term {
            metric: 1,
            target: 0,
            weight: 1,
            shape: Shape::Target,
        }
    } else {
        Term {
            metric: 0,
            target: src.range(-20, 20),
            weight: 1,
            shape: shape_of(src),
        }
    };
    Config {
        name: "w".into(),
        owner: acct(src).into(),
        metrics: vec![m(-l, l), m(-100, 100)],
        terms: vec![term],
        recipes,
        max_age_seconds: 0,
    }
}

/// Plain mode: the id of a record to act on: the oldest a quarter of the
/// time, an open one half of the time (never one whose id is 7 mod 32, so
/// the front is sometimes open when the window is full), otherwise any
/// retained id or one just outside the window (pruned or not yet issued).
#[cfg(not(kani))]
pub fn window_id(src: &mut crate::src::Rng, p: &Pool) -> u32 {
    let f = p.first_id;
    let len = p.records.len() as u32;
    if len > 0 && src.hint(25) {
        return f;
    }
    if src.hint(65) {
        let open: Vec<u32> = p
            .records
            .iter()
            .filter(|c| spec::open(&c.status) && c.id % 32 != 7)
            .map(|c| c.id)
            .collect();
        if !open.is_empty() {
            return open[src.below(open.len() as u32) as usize];
        }
    }
    if len > 0 && src.hint(85) {
        return f + src.below(len);
    }
    let lo = f.saturating_sub(3);
    lo + src.below(f - lo + len + 3)
}

/// Plain mode: one operation of a window walk, steered toward accepted
/// calls (current version, far expiry, the role that may act).
#[cfg(not(kani))]
pub fn window_op(src: &mut crate::src::Rng, p: &Pool) -> Op {
    let cfg = &p.config;
    let now = p.observed_at;
    let owner = caller(src, &cfg.owner.clone());
    let role = |src: &mut crate::src::Rng, id: u32, executor: bool| {
        let want = record_role(p, id, executor).to_string();
        if src.hint(95) {
            caller(src, &want)
        } else {
            acct(src)
        }
    };
    let k = src.below(100);
    if p.paused && src.hint(30) {
        return Op::Pause { caller: owner, paused: false };
    }
    match k {
        0..=44 => {
            let recipe = src.below(cfg.recipes.len() as u32);
            let r = &cfg.recipes[recipe as usize];
            let caller = if src.hint(95) { caller(src, &r.actor.clone()) } else { acct(src) };
            let version = if src.hint(95) { p.version } else { p.version.wrapping_sub(1) };
            Op::Submit {
                caller,
                req: Request {
                    recipe,
                    units: 1 + src.below(r.max_units),
                    version,
                    expires_at: now + 80_000,
                },
                now,
            }
        }
        45..=56 => {
            let id = window_id(src, p);
            Op::Dispatch { caller: role(src, id, false), id, now }
        }
        57..=63 => Op::Ack {
            id: window_id(src, p),
            accepted: src.bool(),
        },
        64..=79 => {
            let id = window_id(src, p);
            // Recipe 0 rarely completes, so that its completions get pruned
            // while a later recipe still requires them.
            let r0 = matches!(spec::rec(p, id), Some(c) if c.recipe == 0);
            Op::Settle {
                caller: role(src, id, true),
                id,
                completed: src.hint(if r0 { 8 } else { 60 }),
            }
        }
        80..=93 => {
            let id = window_id(src, p);
            let caller = if src.bool() { owner } else { role(src, id, false) };
            Op::Cancel { caller, id, now }
        }
        94..=98 => {
            let open = p.records.iter().any(|c| spec::open(&c.status));
            let vals = if src.hint(if open { 80 } else { 30 }) {
                p.values.clone()
            } else {
                let mut v = Vec::new();
                for m in &cfg.metrics {
                    let x = if src.hint(90) {
                        src.range(m.min, m.max)
                    } else {
                        src.range(m.min - 5, m.max + 5)
                    };
                    v.push(x);
                }
                v
            };
            Op::Observe {
                caller: owner,
                vals,
                now: now + src.below(2) as u64,
            }
        }
        _ => Op::Pause {
            caller: owner,
            paused: src.hint(30),
        },
    }
}
