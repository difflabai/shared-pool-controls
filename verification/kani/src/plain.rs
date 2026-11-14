//! Plain mode: every harness body with the random source. **Not a proof.**
//! Each test also requires every cover it evaluated to have fired, the
//! plain-mode counterpart of Kani's cover check.

use crate::checks::{self, PART_ALL};
use crate::spec::{self, Ghost, Op};
use crate::src::{take_covers, Cfg, Dom, Rng, Shape, Ver};

/// Every part, REF included (`tla.rs`, the v1.1.0 SharedPool.tla).
const BASE: Cfg = Cfg {
    val: Dom::Small(6),
    delta: Dom::Small(4),
    max_units: 3,
    any_weight: false,
    shape: Shape::ANY,
    ver: Ver::Free,
    part: PART_ALL,
};

fn run_with(name: &str, seed: u64, iters: u32, base: Cfg, reach_pct: u8, body: &dyn Fn(&mut Rng)) {
    let _ = take_covers();
    let mut rng = Rng::new(seed, base);
    rng.reach_pct = reach_pct;
    for _ in 0..iters {
        body(&mut rng);
    }
    let covers = take_covers();
    let missed: Vec<_> = covers.iter().filter(|(_, (_, hit))| *hit == 0).collect();
    assert!(missed.is_empty(), "{name}: unsatisfied covers {missed:?}");
}

fn run(name: &str, seed: u64, iters: u32, body: fn(&mut Rng)) {
    run_with(name, seed, iters, BASE, 50, &body)
}

#[test]
fn h_init() {
    run("h_init", 0x11, 20_000, checks::init);
}

#[test]
fn h_init_inv() {
    run("h_init_inv", 0x15, 20_000, checks::init_inv);
}

#[test]
fn h_validate() {
    run("h_validate", 0x12, 200_000, checks::validate);
}

#[test]
fn h_energy() {
    run("h_energy", 0x13, 50_000, checks::energy);
    run("h_energy_max", 0x14, 1, checks::energy_max);
}

#[test]
fn h_step_submit() {
    run("h_step_submit", 0x21, 300_000, checks::step_submit);
}

#[test]
fn h_step_dispatch() {
    run("h_step_dispatch", 0x22, 100_000, checks::step_dispatch);
}

#[test]
fn h_step_ack() {
    run("h_step_ack", 0x23, 100_000, checks::step_ack);
}

#[test]
fn h_step_cancel() {
    run("h_step_cancel", 0x24, 2_000_000, checks::step_cancel);
}

#[test]
fn h_step_settle() {
    run("h_step_settle", 0x25, 2_000_000, checks::step_settle);
}

#[test]
fn h_step_pause() {
    run("h_step_pause", 0x26, 50_000, checks::step_pause);
}

#[test]
fn h_step_observe() {
    run("h_step_observe", 0x27, 100_000, checks::step_observe);
}

#[test]
fn h_version_cap() {
    let base = Cfg { ver: Ver::AtCap, ..BASE };
    run_with("h_version_cap", 0x28, 300_000, base, 0, &|s: &mut Rng| checks::version_cap(s, None));
}

#[test]
fn h_version_max() {
    let base = Cfg { ver: Ver::Max, ..BASE };
    run_with("h_version_max", 0x29, 100_000, base, 0, &|s: &mut Rng| checks::version_max(s, None));
}

/// `h_wf_*`: WF states (`first_id`, `completed_units`, values and version
/// free), REF-1..3 included: its covers require a submit refused at the id
/// overflow and a completing settle whose `completed_units` saturates, the
/// two `tla.rs` lines no INV state and no conformance trace reaches.
#[test]
fn h_wf() {
    run_with("h_wf", 0x2a, 1_000_000, BASE, 0, &|s: &mut Rng| checks::wf_step(s, None));
}

#[test]
fn h_preview() {
    run("h_preview", 0x31, 100_000, checks::preview);
}

#[test]
fn h_best() {
    run("h_best", 0x32, 100_000, checks::best);
}

#[test]
fn h_live_all() {
    run("h_live_all", 0x41, 300_000, checks::live_all);
}

#[test]
fn h_wedge() {
    run_with("h_wedge", 0x42, 200_000, BASE, 0, &checks::wedge);
}

#[test]
fn h_wedge_healthy() {
    run_with("h_wedge_healthy", 0x46, 200_000, BASE, 0, &checks::wedge_healthy);
}

#[test]
fn h_bmc_from_init() {
    run_with("h_bmc_from_init", 0x43, 1_000_000, BASE, 0, &|r: &mut Rng| checks::bmc_from_init(r, 5));
}

/// The Kani instance of `h_bmc_from_init_*` (two operations, the first a
/// submit or a pause or observe; deviation 20) with the random source.
#[test]
fn h_bmc_two() {
    run_with("h_bmc_two", 0x47, 500_000, BASE, 0, &|r: &mut Rng| {
        let first = if r.next() % 2 == 0 { crate::gen::SUBMIT } else { crate::gen::PAUSE };
        checks::bmc_two(r, first, None)
    });
}

#[test]
fn h_full() {
    run_with("h_full", 0x44, 100_000, BASE, 0, &|s: &mut Rng| checks::full(s, None));
}

/// `h_slot_cycle_0`, `h_slot_cycle_2` (F-3): the cycle from `Pool::new`
/// and from INV states (random shapes, up to 3 records, often reached from
/// `Pool::new` by a random walk).
#[test]
fn h_slot_cycle() {
    run_with("h_slot_cycle_0", 0x48, 100_000, BASE, 0, &|s: &mut Rng| checks::slot_cycle(s, true));
    run_with("h_slot_cycle", 0x49, 300_000, BASE, 50, &|s: &mut Rng| checks::slot_cycle(s, false));
}

/// `window_walk`: 60 seeds of 3,000 window operations each from
/// `Pool::new` (`checks::window_walk`): enough accepted submits to fill the
/// window and prune it, every step checked against the spec and every v1.1
/// invariant, LIVE-ALL every 20 steps.
#[test]
fn window_walk() {
    let _ = take_covers();
    for seed in 0..60u64 {
        let mut rng = Rng::new(0x51_0000 + 2 * seed, BASE);
        checks::window_walk(&mut rng, 3_000);
    }
    let covers = take_covers();
    let missed: Vec<_> = covers.iter().filter(|(_, (_, hit))| *hit == 0).collect();
    assert!(missed.is_empty(), "window_walk: unsatisfied covers {missed:?}");
}

/// `cap_walk`: 200 seeds, each a short window walk, then the version set
/// just below the cap and 200 more operations (`checks::cap_walk`).
#[test]
fn cap_walk() {
    let _ = take_covers();
    for seed in 0..200u64 {
        let mut rng = Rng::new(0x52_0000 + 2 * seed, BASE);
        checks::cap_walk(&mut rng, 200);
    }
    let covers = take_covers();
    let missed: Vec<_> = covers.iter().filter(|(_, (_, hit))| *hit == 0).collect();
    assert!(missed.is_empty(), "cap_walk: unsatisfied covers {missed:?}");
}

/// The brief's configuration (traces 0 and 207).
fn brief_config(reserve: Vec<i64>, complete: Vec<i64>) -> pool_core::Config {
    use pool_core::*;
    let m = |min, max| Metric { name: "m".into(), unit: "u".into(), initial: 5, min, max };
    Config {
        name: "p".into(),
        owner: "o_".into(),
        metrics: vec![m(0, 10), m(-100, 100)],
        terms: vec![Term { metric: 1, target: 0, weight: 1, shape: Shape::Target }],
        recipes: vec![Recipe {
            name: "r".into(),
            actor: "a1".into(),
            executor: "x1".into(),
            reserve,
            complete,
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    }
}

/// Runs `op` from `p` with every check of `checks::checked_step` (INV,
/// LIVE-ALL, exactness and frame against the spec).
fn step(p: &mut pool_core::Pool, g: &mut Ghost, op: Op) -> spec::Res {
    checks::checked_step(p, g, &op)
}

/// [`step`] of a submit at the current version, expiring at 1000.
fn submit(p: &mut pool_core::Pool, g: &mut Ghost, caller: &'static str, recipe: u32, units: u32, now: u64) -> spec::Res {
    let req = pool_core::Request {
        recipe,
        units,
        version: p.version,
        expires_at: 1000,
    };
    step(p, g, Op::Submit { caller, req, now })
}

/// F-3 fixed, end to end on the brief's configuration from `Pool::new`:
/// 300 submit/cancel cycles by one actor are all accepted (v1.0.0: `Full`
/// from the 257th), with ids `0..300`; 44 records are pruned (`first_id`
/// 44, 256 retained), and a pruned id is NotFound.
#[test]
fn f3_slot_exhaustion() {
    use pool_core::*;
    let mut p = Pool::new(brief_config(vec![-2, 0], vec![2, -5]), 100).unwrap();
    let mut g = Ghost::init(&p);
    let start = p.values.clone();
    for k in 0..300u32 {
        let req = Request { recipe: 0, units: 1, version: p.version, expires_at: 200 };
        assert_eq!(step(&mut p, &mut g, Op::Submit { caller: "a1", req, now: 100 }), Ok(Some(k)));
        assert_eq!(step(&mut p, &mut g, Op::Cancel { caller: "a1", id: k, now: 100 }), Ok(None));
        assert_eq!(p.values, start);
    }
    assert_eq!((p.records.len(), p.first_id), (256, 44));
    assert_eq!((p.records[0].id, p.records[255].id), (44, 299));
    assert!(p.records.iter().all(|c| c.status == Status::Cancelled));
    let before = p.clone();
    assert_eq!(p.record(43), Err(Error::NotFound));
    assert_eq!(step(&mut p, &mut g, Op::Cancel { caller: "a1", id: 43, now: 100 }), Err(Error::NotFound));
    assert_eq!(step(&mut p, &mut g, Op::Settle { caller: "x1", id: 0, completed: true }), Err(Error::NotFound));
    assert_eq!(p, before);
}

/// F-1 fixed, on the brief's instance (trace 0): after the disturbing
/// observe the hedge that cures metric 0 only by its reservation is refused
/// (Bounds), nothing changes, and with nothing open the owner restates
/// in-bounds values. The admitted shape (upstream
/// `f1_recovery_by_completion_is_admitted_and_live`): a hedge reserving only
/// collateral and curing metric 0 when it completes is admitted (values
/// `[12, 4]`), dispatched, and settles either way (`[9, 4]` completed,
/// `[12, 5]` released); the owner's cancel is Pending in flight and the
/// owner's refresh is Ok.
#[test]
fn f1_brief_instance() {
    use pool_core::*;
    let mut p = Pool::new(brief_config(vec![-2, 0], vec![2, -5]), 100).unwrap();
    let mut g = Ghost::init(&p);
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: "o_", vals: vec![12, 5], now: 100 }), Ok(None));
    let before = p.clone();
    let req = Request { recipe: 0, units: 1, version: p.version, expires_at: 200 };
    let d = p.preview("a1", &req, 100);
    assert!(!d.allowed && d.reason == "Bounds");
    assert_eq!(p.best_quantity("a1", &req, 100), None);
    assert_eq!(step(&mut p, &mut g, Op::Submit { caller: "a1", req, now: 100 }), Err(Error::Bounds));
    assert_eq!(p, before);
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: "o_", vals: vec![5, 5], now: 103 }), Ok(None));

    let mut p = Pool::new(brief_config(vec![0, -1], vec![-3, 0]), 100).unwrap();
    let mut g = Ghost::init(&p);
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: "o_", vals: vec![12, 5], now: 100 }), Ok(None));
    let req = Request { recipe: 0, units: 1, version: p.version, expires_at: 200 };
    assert_eq!(step(&mut p, &mut g, Op::Submit { caller: "a1", req, now: 100 }), Ok(Some(0)));
    assert_eq!(p.values, vec![12, 4]);
    assert_eq!(step(&mut p, &mut g, Op::Dispatch { caller: "a1", id: 0, now: 100 }), Ok(None));
    assert_eq!(step(&mut p, &mut g, Op::Cancel { caller: "o_", id: 0, now: 100 }), Err(Error::Pending));
    let (mut q, mut h) = (p.clone(), g.clone());
    assert_eq!(step(&mut q, &mut h, Op::Settle { caller: "x1", id: 0, completed: false }), Ok(None));
    assert_eq!(q.values, vec![12, 5]);
    let (mut q, mut h) = (p.clone(), g.clone());
    let v = q.values.clone();
    assert_eq!(step(&mut q, &mut h, Op::Observe { caller: "o_", vals: v, now: 101 }), Ok(None));
    assert_eq!(step(&mut p, &mut g, Op::Settle { caller: "x1", id: 0, completed: true }), Ok(None));
    assert_eq!(p.values, vec![9, 4]);
}

/// F-2 fixed, on v1.0.0's small instance (`checks::f2_instance`): the
/// hedge that relied on the position's reservation is refused at admission
/// (Bounds), so the script stops there and nothing wedges.
#[test]
fn f2_small_instance() {
    use pool_core::*;
    let _ = take_covers();
    let out = checks::wedge_healthy_on(checks::f2_instance());
    assert_eq!(out, vec![Ok(Some(0)), Err(Error::Bounds)], "the F-2 hedge is refused");
    let _ = take_covers();
}

/// F-2 fixed, on the supplied `risk` fixture (no observe), as upstream
/// `f2_hedge_cannot_rely_on_an_unexecuted_position` and
/// `f2_reserved_position_can_always_be_cancelled`: with a credit position
/// reserved, a selloff hedge of 8 or 7 units is refused (Bounds) and
/// `best_quantity` suggests 6; the 6-unit hedge completes, and then the
/// executor's honest settle(cancelled) of the position is Ok (values[0] 20,
/// then 0). Reserved form: the position is cancelled by the owner or its
/// actor, also after its request expired.
#[test]
fn f2_risk_fixture() {
    use pool_core::*;
    let leak = |s: &str| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let cfg = fixtures::config("risk").unwrap();
    let owner = leak(&cfg.owner);
    let credit = leak(&cfg.recipes[0].actor);
    let hedge = leak(&cfg.recipes[3].actor);
    let exec = leak(&cfg.recipes[0].executor);
    let hexec = leak(&cfg.recipes[3].executor);
    let fresh = || {
        let p = Pool::new(fixtures::config("risk").unwrap(), 100).unwrap();
        let g = Ghost::init(&p);
        (p, g)
    };
    let req = |recipe, units, p: &Pool| Request { recipe, units, version: p.version, expires_at: 1000 };
    let (mut p, mut g) = fresh();
    let id0 = submit(&mut p, &mut g, credit, 0, 1, 100).unwrap().unwrap();
    let before = p.clone();
    assert_eq!(submit(&mut p, &mut g, hedge, 3, 8, 100), Err(Error::Bounds));
    assert_eq!(submit(&mut p, &mut g, hedge, 3, 7, 100), Err(Error::Bounds));
    assert_eq!(p, before);
    assert_eq!(p.best_quantity(hedge, &req(3, 8, &p), 100).map(|q| q.units), Some(6));
    let id1 = submit(&mut p, &mut g, hedge, 3, 6, 100).unwrap().unwrap();
    let (mut r, mut rg) = (p.clone(), g.clone());
    step(&mut p, &mut g, Op::Dispatch { caller: credit, id: id0, now: 101 }).unwrap();
    step(&mut p, &mut g, Op::Dispatch { caller: hedge, id: id1, now: 101 }).unwrap();
    step(&mut p, &mut g, Op::Settle { caller: hexec, id: id1, completed: true }).unwrap();
    assert_eq!(p.values[0], 20);
    assert_eq!(step(&mut p, &mut g, Op::Cancel { caller: owner, id: id0, now: 102 }), Err(Error::Pending));
    let v = p.values.clone();
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: owner, vals: v, now: 103 }), Ok(None));
    assert_eq!(step(&mut p, &mut g, Op::Settle { caller: exec, id: id0, completed: false }), Ok(None));
    assert_eq!(p.values[0], 0);
    // Reserved form: the hedge completes, the position is never dispatched.
    step(&mut r, &mut rg, Op::Dispatch { caller: hedge, id: id1, now: 101 }).unwrap();
    step(&mut r, &mut rg, Op::Settle { caller: hexec, id: id1, completed: true }).unwrap();
    let (mut q, mut qg) = (r.clone(), rg.clone());
    assert_eq!(step(&mut q, &mut qg, Op::Cancel { caller: owner, id: id0, now: 1000 }), Ok(None));
    assert_eq!(step(&mut r, &mut rg, Op::Cancel { caller: credit, id: id0, now: 1000 }), Ok(None));
    assert_eq!(r.records[id0 as usize].status, Status::Cancelled);
}

/// F-5 fixed (upstream `f5_open_records_resolve_above_the_version_cap` and
/// `f5_paused_pool_at_the_cap_still_settles`, on the `gpu` fixture): at
/// `VERSION_CAP`, submit, dispatch, set_paused and observe fail Arithmetic
/// and change nothing; the open records still resolve (ack, ack, settle;
/// cancel), ending at `VERSION_CAP + 4`; nothing new starts afterwards.
#[test]
fn f5_open_records_resolve_above_the_cap() {
    use pool_core::*;
    let leak = |s: &str| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let cfg = fixtures::config("gpu").unwrap();
    let owner = leak(&cfg.owner);
    let a = |k: usize| leak(&cfg.recipes[k].actor);
    let x = |k: usize| leak(&cfg.recipes[k].executor);
    let mut p = Pool::new(fixtures::config("gpu").unwrap(), 100).unwrap();
    let mut g = Ghost::init(&p);
    let req = |recipe, p: &Pool| Request { recipe, units: 1, version: p.version, expires_at: 1000 };
    let flight = submit(&mut p, &mut g, a(0), 0, 1, 100).unwrap().unwrap();
    step(&mut p, &mut g, Op::Dispatch { caller: a(0), id: flight, now: 100 }).unwrap();
    let reserved = submit(&mut p, &mut g, a(2), 2, 1, 100).unwrap().unwrap();
    p.version = VERSION_CAP;
    g.eff = None;
    let frozen = p.clone();
    let q = req(4, &p);
    let d = p.preview(a(4), &q, 100);
    assert!(!d.allowed && d.reason == "Arithmetic");
    assert_eq!(p.best_quantity(a(4), &q, 100), None);
    assert_eq!(step(&mut p, &mut g, Op::Submit { caller: a(4), req: q, now: 100 }), Err(Error::Arithmetic));
    assert_eq!(step(&mut p, &mut g, Op::Dispatch { caller: a(2), id: reserved, now: 100 }), Err(Error::Arithmetic));
    assert_eq!(step(&mut p, &mut g, Op::Pause { caller: owner, paused: true }), Err(Error::Arithmetic));
    let v = frozen.values.clone();
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: owner, vals: v, now: 100 }), Err(Error::Arithmetic));
    assert_eq!(p, frozen);
    step(&mut p, &mut g, Op::Ack { id: flight, accepted: false }).unwrap();
    step(&mut p, &mut g, Op::Ack { id: flight, accepted: true }).unwrap();
    step(&mut p, &mut g, Op::Settle { caller: x(0), id: flight, completed: true }).unwrap();
    step(&mut p, &mut g, Op::Cancel { caller: a(2), id: reserved, now: 100 }).unwrap();
    assert_eq!(p.version, VERSION_CAP + 4);
    assert!(p.records.iter().all(|c| !spec::open(&c.status)));
    let v = p.values.clone();
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: owner, vals: v, now: 100 }), Err(Error::Arithmetic));
    let q = req(4, &p);
    assert_eq!(step(&mut p, &mut g, Op::Submit { caller: a(4), req: q, now: 100 }), Err(Error::Arithmetic));

    // Paused form: the last pause that fits reaches the cap; then the open
    // records still resolve.
    let mut p = Pool::new(fixtures::config("gpu").unwrap(), 100).unwrap();
    let mut g = Ghost::init(&p);
    let initial = p.values.clone();
    let flight = submit(&mut p, &mut g, a(0), 0, 1, 100).unwrap().unwrap();
    step(&mut p, &mut g, Op::Dispatch { caller: a(0), id: flight, now: 100 }).unwrap();
    let reserved = submit(&mut p, &mut g, a(2), 2, 1, 100).unwrap().unwrap();
    p.version = VERSION_CAP - 1;
    g.eff = None;
    assert_eq!(step(&mut p, &mut g, Op::Pause { caller: owner, paused: true }), Ok(None));
    assert_eq!(p.version, VERSION_CAP);
    assert_eq!(step(&mut p, &mut g, Op::Pause { caller: owner, paused: false }), Err(Error::Arithmetic));
    step(&mut p, &mut g, Op::Settle { caller: x(0), id: flight, completed: false }).unwrap();
    step(&mut p, &mut g, Op::Cancel { caller: owner, id: reserved, now: 100 }).unwrap();
    assert_eq!(p.values, initial);
    assert_eq!(p.version, VERSION_CAP + 2);
    assert!(p.paused);
}

/// F-6 fixed (upstream `f6_owner_refreshes_the_snapshot_with_records_open`,
/// on the `risk` fixture, max_age 300, observed at 100): at 401 submit and
/// dispatch fail SnapshotExpired; new values are still Pending with records
/// open, a non-owner is Unauthorized, and the owner's restatement of the
/// current values is Ok: only `observed_at` and the version change; then
/// dispatch and submit pass again.
#[test]
fn f6_refresh_with_records_open() {
    use pool_core::*;
    let leak = |s: &str| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let cfg = fixtures::config("risk").unwrap();
    let owner = leak(&cfg.owner);
    let a = |k: usize| leak(&cfg.recipes[k].actor);
    let mut p = Pool::new(fixtures::config("risk").unwrap(), 100).unwrap();
    let mut g = Ghost::init(&p);
    let slow = submit(&mut p, &mut g, a(0), 0, 1, 100).unwrap().unwrap();
    step(&mut p, &mut g, Op::Dispatch { caller: a(0), id: slow, now: 100 }).unwrap();
    step(&mut p, &mut g, Op::Ack { id: slow, accepted: true }).unwrap();
    let next = submit(&mut p, &mut g, a(3), 3, 1, 100).unwrap().unwrap();
    assert_eq!(submit(&mut p, &mut g, a(1), 1, 1, 401), Err(Error::SnapshotExpired));
    assert_eq!(step(&mut p, &mut g, Op::Dispatch { caller: a(3), id: next, now: 401 }), Err(Error::SnapshotExpired));
    let before = p.clone();
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: owner, vals: vec![60, 40, 30, 40], now: 401 }), Err(Error::Pending));
    let current = p.values.clone();
    let other = if a(0) == owner { a(1) } else { a(0) };
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: other, vals: current.clone(), now: 401 }), Err(Error::Unauthorized));
    assert_eq!(p, before);
    assert_eq!(step(&mut p, &mut g, Op::Observe { caller: owner, vals: current.clone(), now: 401 }), Ok(None));
    assert_eq!((p.observed_at, p.version), (401, before.version + 1));
    assert_eq!((&p.values, &p.records), (&current, &before.records));
    step(&mut p, &mut g, Op::Dispatch { caller: a(3), id: next, now: 401 }).unwrap();
    submit(&mut p, &mut g, a(3), 3, 1, 401).unwrap();
}

#[test]
#[should_panic(expected = "canary")]
fn h_canary() {
    run("h_canary", 0x45, 2_000, checks::canary);
}
