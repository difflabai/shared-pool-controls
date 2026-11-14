//! Dual-mode value sources (the near-approval design, strategy.md section 6.1).
//!
//! Harness bodies are written once, against [`Src`]. Under `cfg(kani)` they
//! are instantiated with [`Sym`], whose values are symbolic: **that is the
//! proof** (within the bounds of the harness). Under `cargo test` they run
//! with [`Rng`], a xorshift generator with hints toward interesting states:
//! **that is a test, not a proof**.
//!
//! `assume!` prunes a path (Kani) or abandons a sample (plain). `cover!` is a
//! Kani cover (must be SATISFIED, see `run.sh`) or a plain-mode hit counter
//! that the plain driver requires to fire at least once.

use crate::spec::Ghost;
use pool_core::Pool;

/// Value domain of a harness instance (the cost ladder in RESULTS.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dom {
    /// Values in `-k..=k`.
    Small(i64),
    /// Values `a + o` with `a ∈ {-1e6, 0, 1e6}` and `o ∈ -k..=k`, clipped to
    /// `[-1e6, 1e6]`: small values and both extremes.
    Anchored(i64),
    /// Any value in `[-1e6, 1e6]`.
    Full,
}

/// Sizes of a generated configuration and state. `None`: symbolic up to the
/// bound (`gen::MAX_*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub metrics: Option<u8>,
    pub terms: Option<u8>,
    pub recipes: Option<u8>,
    pub records: Option<u8>,
}

impl Shape {
    pub const ANY: Shape = Shape {
        metrics: None,
        terms: None,
        recipes: None,
        records: None,
    };
}

/// The version of a generated state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ver {
    /// Any version the domain allows (INV states: `first_id + len <= version`
    /// and INV-VERCAP).
    Free,
    /// `version >= VERSION_CAP` (`h_version_cap_*`, INV states: INV-VERCAP
    /// keeps the reserve for the open records).
    AtCap,
    /// `version = u32::MAX` (`h_version_max_*`, WF states).
    Max,
}

/// Per-harness parameters (fixed under Kani, random in plain mode).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cfg {
    /// Values: metric bounds, initial and base values, targets.
    pub val: Dom,
    /// Recipe deltas (`reserve`, `complete`).
    pub delta: Dom,
    /// Largest `max_units` of a generated recipe.
    pub max_units: u32,
    /// Weights: `true` any `1..=1000`, `false` one of `{1, 2, 1000}`.
    pub any_weight: bool,
    pub shape: Shape,
    /// The version of generated states (`h_version_cap`, `h_version_max`).
    pub ver: Ver,
    /// Which groups of checks to run (`checks::PART_*`).
    pub part: u8,
}

pub trait Src {
    /// Any `u64`.
    fn u64(&mut self) -> u64;
    /// Any `u32`.
    fn u32(&mut self) -> u32;
    fn bool(&mut self) -> bool;
    /// A value in `0..n` (`n > 0`).
    fn below(&mut self, n: u32) -> u32;
    /// A value in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64;
    /// Plain mode only: true with roughly `percent` probability, steering
    /// samples toward states where guards pass. Always false under Kani, so
    /// the unconstrained branch is the one that is verified.
    fn hint(&mut self, percent: u8) -> bool;
    /// Plain mode only: a state reached from `Pool::new` by a random walk,
    /// with its ghost. Always `None` under Kani.
    fn reach(&mut self) -> Option<(Pool, Ghost)>;
    /// Whether an operation argument that is only compared before it reaches
    /// arithmetic or an index (request recipe and units, record ids) is drawn
    /// from all of `u32` (plain mode, sometimes) or from a small range that
    /// already covers every guard outcome (always under Kani; RESULTS.md,
    /// "Bounds").
    fn wide(&mut self) -> bool;
    /// The harness parameters.
    fn cfg(&mut self) -> Cfg;
    /// An anchor index in `0..3` for `Dom::Anchored` (plain mode: usually
    /// the same one throughout a sample, so values meet near one extreme).
    fn anchor(&mut self) -> u32 {
        self.below(3)
    }
}

/// Prune the current path (Kani) or abandon the current sample (plain).
#[macro_export]
macro_rules! assume {
    ($cond:expr) => {{
        if !$crate::src::assume($cond) {
            return;
        }
    }};
}

/// Function form of `assume!`. Under Kani it assumes `c` and returns the
/// constant `true`, so the caller's early return is dead code that symbolic
/// execution never enters. That matters: a live early return (say `None`
/// from a state builder) merges with the other path at the join point, and
/// every field of the returned value becomes a conditional over an
/// unconstrained value, so even the fixed vector lengths turn symbolic
/// (RESULTS.md, cost ladder). In plain mode it returns `c`.
pub fn assume(c: bool) -> bool {
    #[cfg(kani)]
    {
        kani::assume(c);
        true
    }
    #[cfg(not(kani))]
    c
}

/// A reachability goal: a Kani cover (the run fails if it is not SATISFIED in
/// some instance of its family, see `run.sh`) or a plain-mode hit counter.
#[macro_export]
macro_rules! cover {
    ($cond:expr, $msg:literal) => {{
        let c: bool = $cond;
        #[cfg(kani)]
        kani::cover!(c, $msg);
        #[cfg(not(kani))]
        $crate::src::record_cover(c, $msg);
    }};
}

// ---------------------------------------------------------------------------
// Symbolic source
// ---------------------------------------------------------------------------

#[cfg(kani)]
pub struct Sym(pub Cfg);

#[cfg(kani)]
impl Src for Sym {
    fn u64(&mut self) -> u64 {
        kani::any()
    }
    fn u32(&mut self) -> u32 {
        kani::any()
    }
    fn bool(&mut self) -> bool {
        kani::any()
    }
    /// Built from 4 or 8 symbolic bits when `n` allows, so the high bits are
    /// constant zero: a product or an index with such a value is a narrow
    /// circuit, not a 64-bit one (RESULTS.md, cost ladder).
    fn below(&mut self, n: u32) -> u32 {
        let v = if n <= 16 {
            (kani::any::<u8>() & 0xF) as u32
        } else if n <= 256 {
            kani::any::<u8>() as u32
        } else {
            kani::any::<u32>()
        };
        kani::assume(v < n);
        v
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = hi as i128 - lo as i128;
        if 0 <= span && span < 256 {
            lo + self.below(span as u32 + 1) as i64
        } else {
            kani::any_where(|v: &i64| lo <= *v && *v <= hi)
        }
    }
    fn hint(&mut self, _percent: u8) -> bool {
        false
    }
    fn wide(&mut self) -> bool {
        false
    }
    fn reach(&mut self) -> Option<(Pool, Ghost)> {
        None
    }
    fn cfg(&mut self) -> Cfg {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Random source (plain mode)
// ---------------------------------------------------------------------------

#[cfg(not(kani))]
pub struct Rng {
    pub state: u64,
    /// The sample's anchor (set by `cfg`).
    pub anchor: u32,
    /// The parameters plain mode samples around: `cfg()` randomizes the
    /// value domains and shapes, but keeps `part`.
    pub base: Cfg,
    /// Probability (percent) that `reach` returns a random-walk state.
    pub reach_pct: u8,
}

#[cfg(not(kani))]
impl Rng {
    pub fn new(seed: u64, base: Cfg) -> Rng {
        Rng {
            state: seed | 1,
            anchor: 1,
            base,
            reach_pct: 50,
        }
    }
    pub fn next(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(not(kani))]
impl Src for Rng {
    fn u64(&mut self) -> u64 {
        // Mostly small values (times, ids) and the arithmetic edges.
        match self.next() % 8 {
            0 => u64::MAX - self.next() % 3,
            1 => self.next(),
            2 => 86_400 + self.next() % 300,
            _ => self.next() % 300,
        }
    }
    fn u32(&mut self) -> u32 {
        match self.next() % 8 {
            0 => u32::MAX - (self.next() % 3) as u32,
            1 => self.next() as u32,
            2 => 100 + (self.next() % 3) as u32,
            _ => (self.next() % 5) as u32,
        }
    }
    fn bool(&mut self) -> bool {
        self.next() % 2 == 0
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi as i128 - lo as i128 + 1) as u128;
        (lo as i128 + (self.next() as u128 % span) as i128) as i64
    }
    fn hint(&mut self, percent: u8) -> bool {
        self.next() % 100 < percent as u64
    }
    fn wide(&mut self) -> bool {
        self.hint(25)
    }
    fn reach(&mut self) -> Option<(Pool, Ghost)> {
        if !self.hint(self.reach_pct) {
            return None;
        }
        let steps = (self.next() % 24) as usize;
        Some(crate::gen::random_walk(self, steps))
    }
    fn anchor(&mut self) -> u32 {
        if self.hint(80) {
            self.anchor
        } else {
            self.below(3)
        }
    }
    fn cfg(&mut self) -> Cfg {
        let mut c = self.base;
        self.anchor = (self.next() % 3) as u32;
        // Plain mode mixes value domains: mostly small, sometimes the extremes.
        c.val = match self.next() % 10 {
            0 => Dom::Full,
            1..=3 => Dom::Anchored(4),
            _ => Dom::Small(6),
        };
        c.delta = match self.next() % 10 {
            0 => Dom::Full,
            1 => Dom::Anchored(3),
            _ => Dom::Small(4),
        };
        c.max_units = if self.hint(10) { 100 } else { c.max_units };
        c.any_weight = self.hint(20);
        c
    }
}

#[cfg(not(kani))]
thread_local! {
    static COVERS: std::cell::RefCell<std::collections::BTreeMap<&'static str, (u64, u64)>> =
        Default::default();
}

#[cfg(not(kani))]
pub fn record_cover(hit: bool, msg: &'static str) {
    COVERS.with(|c| {
        let mut c = c.borrow_mut();
        let e = c.entry(msg).or_default();
        e.0 += 1;
        e.1 += hit as u64;
    });
}

/// Plain driver: clears and returns `(evaluated, satisfied)` per cover.
#[cfg(not(kani))]
pub fn take_covers() -> std::collections::BTreeMap<&'static str, (u64, u64)> {
    COVERS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
