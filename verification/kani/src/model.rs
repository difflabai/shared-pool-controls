//! Fixed-capacity models of `Vec::with_capacity_in` and `Vec::reserve` (Kani
//! only; RESULTS.md, "Cost ladder" and deviation 1), and the free
//! `Config::validate` of `h_init` (deviation 23).
//!
//! The core copies vectors whose length CBMC cannot fold to a constant: the
//! fallible `collect` in `add` merges the error path (fewer elements) into
//! the success path, so `next.len()` is a symbolic expression, and
//! `envelope`'s `values.to_vec()` then asks for an allocation of symbolic
//! size. CBMC encodes such an object with the array theory, whose constraints
//! grow quadratically with the reads and writes (a single submit with one
//! metric, one term, one recipe and no record ran out of 20 GB). This model
//! allocates a constant capacity instead.
//!
//! It is a refinement of `with_capacity_in`'s contract: std guarantees only
//! that the vector holds *at least* `capacity` elements without
//! reallocating, and the model returns an empty vector with capacity
//! `max_capacity::<T>() >= capacity`. The core never reads a capacity. A
//! request above the constant fails the harness on the assertion below, so
//! the model never hides a path: it can only make a harness fail.

/// The constant capacity: 24 bytes for strings (account names, and the
/// decimal `u64`s of `preview`, at most 20 digits); 3 elements of a large
/// type (metrics and recipes: at most 2, or 3 in a raw configuration); 4
/// elements otherwise (3 records plus one submit; values, deltas, terms).
/// Small objects matter: a read at a symbolic index (`recipes[req.recipe]`)
/// is a selection over every byte of the object.
pub const fn max_capacity<T>() -> usize {
    let size = std::mem::size_of::<T>();
    if size == 1 {
        24
    } else if size > 32 {
        3
    } else {
        4
    }
}

/// Model of `std::vec::Vec::with_capacity_in` (and, through it, of
/// `with_capacity`, `<[T]>::to_vec` and `collect` of an exact-size iterator).
pub fn with_capacity_in<T, A: std::alloc::Allocator>(capacity: usize, alloc: A) -> Vec<T, A> {
    kani::assert(
        capacity <= max_capacity::<T>(),
        "harness model: Vec::with_capacity_in within the constant capacity",
    );
    let mut v = Vec::new_in(alloc);
    v.reserve_exact(max_capacity::<T>());
    v
}

/// Model of `std::vec::Vec::reserve` (with [`with_capacity_in`]; RESULTS.md,
/// deviation 1). The core's `collect`s of a `TrustedLen` iterator (`release`'s
/// `undo`) reserve the iterator's length, which CBMC cannot fold (it is read
/// through a recipe at a symbolic index), so std's growth path stays live and
/// allocates a symbolic size: the array theory again (cancel: 41M of 45M
/// clauses). std reserves only when `capacity - len < additional`; every
/// vector here already has the constant capacity, so the model asserts that
/// the request fits and does nothing, as std does then. A vector without a
/// buffer (`String::new()`, `Vec::new()`) gets the constant capacity first.
/// A request that does not fit fails the harness: the model never hides a
/// path.
pub fn reserve<T, A: std::alloc::Allocator>(v: &mut Vec<T, A>, additional: usize) {
    if v.capacity() == 0 && additional > 0 {
        v.reserve_exact(max_capacity::<T>());
    }
    kani::assert(
        additional <= v.capacity() - v.len(),
        "harness model: Vec::reserve within the constant capacity",
    );
}

/// A static's contents behind a tag that no constant has. Kani 0.68.0 keeps
/// one global per allocation *content*, and a static and a constant with
/// the same bytes share it: in `h_best` the zero-initialized `AtomicUsize`
/// `CAND_LAST` was the storage of `RawVec`'s constant `ZERO_CAP`, so the
/// stub's store of the units to it gave every later `Vec::new_in` that
/// capacity with a dangling pointer (CBMC trace in RESULTS.md, "Tool
/// issue"). Every static the models write is therefore `Tagged`, with a
/// distinct 64-bit tag in front of the value, so its bytes match no
/// constant.
#[repr(C)]
pub struct Tagged<T> {
    #[allow(dead_code)]
    tag: u64,
    v: T,
}

impl<T> std::ops::Deref for Tagged<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.v
    }
}

/// `validate`'s result in [`validate`]: 0 not called, 1 `Ok`, 2 `Err`.
pub static VALIDATED: Tagged<std::sync::atomic::AtomicU8> = Tagged {
    tag: 0x5641_4c49_4441_5445,
    v: std::sync::atomic::AtomicU8::new(0),
};

/// `h_init`'s stub of `pool_core::Config::validate` (RESULTS.md, deviation
/// 23): a free result, recorded in [`VALIDATED`]. `h_init` then shows that
/// `Pool::new` is `Ok` exactly when `validate` is, fails with `validate`'s
/// error, and builds the initial pool, for every configuration and either
/// outcome; `h_validate` shows that `validate` is the spec's validity.
pub fn validate(_c: &pool_core::Config) -> Result<(), pool_core::Error> {
    let ok: bool = kani::any();
    VALIDATED.store(if ok { 1 } else { 2 }, std::sync::atomic::Ordering::SeqCst);
    if ok {
        Ok(())
    } else {
        Err(pool_core::Error::InvalidConfig)
    }
}

/// Calls `h_best`'s stub of `Pool::candidate` records, by request units
/// (`1..CAND_N`; `CAND_N` is past the harness's largest request).
pub const CAND_N: usize = 8;

/// One record per units value `u`: the number of calls, the result (`Ok` or
/// not, and the `Ok` values), and the other arguments of the last call.
pub struct CandCall {
    pub calls: std::sync::atomic::AtomicU8,
    pub ok: std::sync::atomic::AtomicBool,
    pub vals: [std::sync::atomic::AtomicI64; crate::spec::M],
    pub caller: std::sync::atomic::AtomicUsize,
    pub caller_len: std::sync::atomic::AtomicUsize,
    pub recipe: std::sync::atomic::AtomicU32,
    pub version: std::sync::atomic::AtomicU32,
    pub expires_at: std::sync::atomic::AtomicU64,
    pub now: std::sync::atomic::AtomicU64,
}

#[allow(clippy::declare_interior_mutable_const)]
const CAND_INIT: CandCall = CandCall {
    calls: std::sync::atomic::AtomicU8::new(0),
    ok: std::sync::atomic::AtomicBool::new(false),
    vals: [const { std::sync::atomic::AtomicI64::new(0) }; crate::spec::M],
    caller: std::sync::atomic::AtomicUsize::new(0),
    caller_len: std::sync::atomic::AtomicUsize::new(0),
    recipe: std::sync::atomic::AtomicU32::new(0),
    version: std::sync::atomic::AtomicU32::new(0),
    expires_at: std::sync::atomic::AtomicU64::new(0),
    now: std::sync::atomic::AtomicU64::new(0),
};

/// The calls of [`candidate`], by units; `CAND_OUT` counts calls with units
/// 0 or `>= CAND_N`.
pub static CAND: Tagged<[CandCall; CAND_N]> = Tagged {
    tag: 0x4341_4e44_4341_4c4c,
    v: [CAND_INIT; CAND_N],
};
pub static CAND_OUT: Tagged<std::sync::atomic::AtomicU8> = Tagged {
    tag: 0x4341_4e44_5f4f_5554,
    v: std::sync::atomic::AtomicU8::new(0),
};

/// `h_best`'s stub of the core's private `Pool::candidate` (RESULTS.md,
/// deviation 24): a free result, `Ok` with any values of length n in
/// `[-1e6, 1e6]` (every `Ok` of the real `candidate` is one: it is `add`'s
/// result) or `Err`, recorded with the call's arguments. `h_best` then
/// shows that `best_quantity` is the documented search over whatever
/// `candidate` (and [`energy`]) return; that the real `candidate` is the
/// spec's is `h_preview`'s (`Ok`/`Err` and the reason) and
/// `h_step_submit_tx`'s and `_post`'s (the `Ok` values: `submit` is
/// `candidate` then the version bump).
pub fn candidate(
    p: &pool_core::Pool,
    caller: &str,
    req: &pool_core::Request,
    now: u64,
) -> Result<Vec<i64>, pool_core::Error> {
    use std::sync::atomic::Ordering::SeqCst;
    let u = req.units as usize;
    if u == 0 || u >= CAND_N {
        CAND_OUT.fetch_add(1, SeqCst);
        CAND_LAST.store(0, SeqCst);
        return Err(pool_core::Error::InvalidUnits);
    }
    CAND_LAST.store(u, SeqCst);
    let c = &CAND[u];
    c.calls.fetch_add(1, SeqCst);
    c.caller.store(caller.as_ptr() as usize, SeqCst);
    c.caller_len.store(caller.len(), SeqCst);
    c.recipe.store(req.recipe, SeqCst);
    c.version.store(req.version, SeqCst);
    c.expires_at.store(req.expires_at, SeqCst);
    c.now.store(now, SeqCst);
    let ok: bool = kani::any();
    c.ok.store(ok, SeqCst);
    if !ok {
        return Err(pool_core::Error::Bounds);
    }
    let n = p.config.metrics.len();
    kani::assert(n <= crate::spec::M, "harness bound: at most M metrics");
    let mut v = Vec::with_capacity(crate::spec::M);
    for i in 0..crate::spec::M {
        if i < n {
            let x: i64 = kani::any();
            kani::assume(crate::spec::bounded(x));
            c.vals[i].store(x, SeqCst);
            v.push(x);
        }
    }
    Ok(v)
}

/// The units of the last call of [`candidate`] (0 if out of range).
pub static CAND_LAST: Tagged<std::sync::atomic::AtomicUsize> = Tagged {
    tag: 0x4341_4e44_4c41_5354,
    v: std::sync::atomic::AtomicUsize::new(0),
};

/// One record per units value `u` of the calls of [`energy`] made right
/// after `candidate` for `u`: the number of calls, whether the argument was
/// that candidate's `Ok` values, and the free result.
pub struct EnergyCall {
    pub calls: std::sync::atomic::AtomicU8,
    pub args: std::sync::atomic::AtomicBool,
    pub ok: std::sync::atomic::AtomicBool,
    pub e: std::sync::atomic::AtomicU64,
}

#[allow(clippy::declare_interior_mutable_const)]
const ENERGY_INIT: EnergyCall = EnergyCall {
    calls: std::sync::atomic::AtomicU8::new(0),
    args: std::sync::atomic::AtomicBool::new(false),
    ok: std::sync::atomic::AtomicBool::new(false),
    e: std::sync::atomic::AtomicU64::new(0),
};

pub static ENERGY: Tagged<[EnergyCall; CAND_N]> = Tagged {
    tag: 0x454e_4552_4759_4341,
    v: [ENERGY_INIT; CAND_N],
};

/// `h_best`'s stub of `Pool::energy` (RESULTS.md, deviation 24): a free
/// result, `Ok` with any `u64` or `Err`, recorded under the units of the
/// last `candidate` call together with whether the argument is exactly that
/// candidate's `Ok` values. `h_energy` shows that the real `energy` is `Ok`
/// and the spec's on every bounded vector of length n (NUM-1). Ranking the
/// candidates by the spec's energy (or by the core's) in the harness put
/// two multiplier circuits per quantity into one formula, and its last SAT
/// call did not finish in 30 minutes.
pub fn energy(p: &pool_core::Pool, values: &[i64]) -> Result<u64, pool_core::Error> {
    use std::sync::atomic::Ordering::SeqCst;
    let u = CAND_LAST.load(SeqCst);
    let k = &ENERGY[u];
    k.calls.fetch_add(1, SeqCst);
    let c = &CAND[u];
    let n = p.config.metrics.len();
    let mut same = u != 0 && c.ok.load(SeqCst) && values.len() == n;
    for i in 0..crate::spec::M {
        if i < n && i < values.len() {
            same = same && values[i] == c.vals[i].load(SeqCst);
        }
    }
    k.args.store(same, SeqCst);
    let ok: bool = kani::any();
    let e: u64 = kani::any();
    k.ok.store(ok, SeqCst);
    k.e.store(e, SeqCst);
    if ok {
        Ok(e)
    } else {
        Err(pool_core::Error::Arithmetic)
    }
}
