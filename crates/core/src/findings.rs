//! Regression tests for the v1.0.0 findings in docs/verification-findings.md.
//!
//! Each v1.1 fix rewrites its reproducer into a test of the fixed
//! behaviour. A reproducer whose fix has not landed yet still pins v1.0.0,
//! including the faulty result, so the fix commit has to change it.
use super::*;

const OWNER: &str = "owner.test.near";
const EXEC: &str = "executor.test.near";
const EXPIRES: u64 = 1_000;

fn pool(name: &str) -> Pool {
    Pool::new(fixtures::config(name).unwrap(), 100).unwrap()
}
fn actor(p: &Pool, recipe: u32) -> String {
    p.config.recipes[recipe as usize].actor.clone()
}
fn req(p: &Pool, recipe: u32, units: u32) -> Request {
    Request {
        recipe,
        units,
        version: p.version,
        expires_at: EXPIRES,
    }
}
fn submit(p: &mut Pool, recipe: u32, units: u32, now: u64) -> Result<u32, Error> {
    let a = actor(p, recipe);
    let q = req(p, recipe, units);
    p.submit(&a, q, now)
}
/// Reserved -> Dispatched -> Active, as the actor's dispatch and an "active" callback.
fn dispatch_active(p: &mut Pool, id: u32, now: u64) {
    let a = actor(p, p.records[id as usize].recipe);
    p.dispatch(&a, id, now).unwrap();
    p.acknowledge(id, true).unwrap();
}
fn metric(min: i64, max: i64, initial: i64) -> Metric {
    Metric {
        name: "m".into(),
        unit: "u".into(),
        initial,
        min,
        max,
    }
}
fn recipe(actor: &str, reserve: Vec<i64>, complete: Vec<i64>, max_units: u32) -> Recipe {
    Recipe {
        name: "r".into(),
        actor: actor.into(),
        executor: EXEC.into(),
        reserve,
        complete,
        max_units,
        requires: vec![],
    }
}
fn config(metrics: Vec<Metric>, recipes: Vec<Recipe>) -> Config {
    Config {
        name: "finding".into(),
        owner: OWNER.into(),
        metrics,
        terms: vec![Term {
            metric: 1,
            target: 0,
            weight: 1,
            shape: Shape::Target,
        }],
        recipes,
        max_age_seconds: 0,
    }
}

/// LIVE: every open record can be resolved. On a clone, a Reserved record
/// is cancelled by its actor and by the owner, and a Dispatched, Active or
/// Unknown record is settled both ways by its executor. Every one of these
/// exits must be Ok and must not raise any metric's violation.
fn assert_live(p: &Pool) {
    let before = p.violations(&p.values);
    for c in p.records.iter().filter(|c| open(&c.status)) {
        let r = &p.config.recipes[c.recipe as usize];
        let mut tries = [p.clone(), p.clone()];
        let (names, results) = if c.status == Status::Reserved {
            (
                ["cancel by actor", "cancel by owner"],
                [
                    tries[0].cancel(&r.actor, c.id, u64::MAX),
                    tries[1].cancel(&p.config.owner, c.id, u64::MAX),
                ],
            )
        } else {
            (
                ["settle(true)", "settle(false)"],
                [
                    tries[0].settle(&r.executor, c.id, true),
                    tries[1].settle(&r.executor, c.id, false),
                ],
            )
        };
        for ((q, result), name) in tries.iter().zip(results).zip(names) {
            assert_eq!(
                result,
                Ok(()),
                "record {} ({:?}): {name} fails in {p:?}",
                c.id,
                c.status
            );
            let after = q.violations(&q.values);
            assert!(
                after.iter().zip(&before).all(|(a, b)| a <= b),
                "record {}: {name} raises a violation in {p:?}",
                c.id
            );
        }
    }
}

/// INV-RES: per metric, the interval spanned by every outcome of every open
/// record (stay open, complete, release) stays in the value range and has
/// the same violation as the values. Computed here independently of
/// Pool::envelope.
fn assert_inv_res(p: &Pool) {
    let mut low = p.values.clone();
    let mut high = p.values.clone();
    for c in p.records.iter().filter(|c| open(&c.status)) {
        let r = &p.config.recipes[c.recipe as usize];
        let u = i64::from(c.units);
        for i in 0..p.values.len() {
            let (d, x) = (r.complete[i] * u, -r.reserve[i] * u);
            low[i] += d.min(x).min(0);
            high[i] += d.max(x).max(0);
        }
    }
    let point = p.violations(&p.values);
    for (i, m) in p.config.metrics.iter().enumerate() {
        assert!(
            bounded(low[i]) && bounded(high[i]),
            "metric {i} interval leaves the value range in {p:?}"
        );
        let interval = (m.min - low[i]).max(0) as u64 + (high[i] - m.max).max(0) as u64;
        assert_eq!(interval, point[i], "INV-RES fails for metric {i} in {p:?}");
    }
}

/// F-1 (high), fixed: an observe puts metric 0 at 12 (max 10). v1.0.0
/// admitted a hedge that reserves -2 and completes +2 on metric 0, and the
/// record could then never become terminal. Releasing it would put metric 0
/// back to 12, above the 10 its reservation reached, so v1.1 refuses it.
/// formalver trace 0.
#[test]
fn f1_hedge_that_cures_by_reservation_is_refused() {
    const HEDGER: &str = "hedger.test.near";
    let c = config(
        vec![metric(0, 10, 5), metric(-100, 100, 5)],
        vec![recipe(HEDGER, vec![-2, 0], vec![2, -5], 1)],
    );
    let mut p = Pool::new(c, 100).unwrap();
    p.observe(OWNER, vec![12, 5], 100).unwrap();
    let before = p.clone();
    let q = req(&p, 0, 1);
    let d = p.preview(HEDGER, &q, 100);
    assert!(!d.allowed);
    assert_eq!(d.reason, "Bounds");
    assert_eq!(p.best_quantity(HEDGER, &q, 100), None);
    assert_eq!(p.submit(HEDGER, q, 100), Err(Error::Bounds));
    assert_eq!(p, before);
    // Nothing is open, so the owner can still restate the measured values.
    p.observe(OWNER, vec![5, 5], 103).unwrap();
}

/// F-1, the shape that stays allowed: in the same disturbed state, a hedge
/// that reserves only collateral (metric 1) and cures metric 0 when it
/// completes is admitted, and every exit stays open. The same holds for the
/// risk fixture's selloff hedge at selloff loss 110 (max 100).
#[test]
fn f1_recovery_by_completion_is_admitted_and_live() {
    const HEDGER: &str = "hedger.test.near";
    let c = config(
        vec![metric(0, 10, 5), metric(-100, 100, 5)],
        vec![recipe(HEDGER, vec![0, -1], vec![-3, 0], 1)],
    );
    let mut p = Pool::new(c, 100).unwrap();
    p.observe(OWNER, vec![12, 5], 100).unwrap();
    let id = submit(&mut p, 0, 1, 100).unwrap();
    assert_eq!(p.values, vec![12, 4]);
    assert_live(&p);
    dispatch_active(&mut p, id, 100);
    assert_live(&p);
    p.settle(EXEC, id, true).unwrap();
    assert_eq!(p.values, vec![9, 4]);

    let mut r = pool("risk");
    r.observe(OWNER, vec![110, 40, 30, 40], 100).unwrap();
    let hedge = submit(&mut r, 3, 2, 100).unwrap();
    assert_eq!(r.values, vec![110, 40, 30, 36]);
    assert_live(&r);
    dispatch_active(&mut r, hedge, 100);
    assert_live(&r);
    r.settle(EXEC, hedge, true).unwrap();
    assert_eq!(r.values[0], 90);
}

/// F-2 (high), fixed: the risk fixture, healthy. v1.0.0 admitted a hedge
/// of 8 units only because a credit position's +20 selloff loss was
/// counted; after the hedge completed, the position's honest settle(false)
/// failed. v1.1 sizes the hedge as if the position were released: 8 and 7
/// units are refused and best_quantity suggests 6. Once the position has
/// executed, it does back 8 units. formalver trace 209.
#[test]
fn f2_hedge_cannot_rely_on_an_unexecuted_position() {
    let mut p = pool("risk");
    let position = submit(&mut p, 0, 1, 100).unwrap();
    let before = p.clone();
    assert_eq!(submit(&mut p, 3, 8, 100), Err(Error::Bounds));
    assert_eq!(submit(&mut p, 3, 7, 100), Err(Error::Bounds));
    assert_eq!(p, before);
    let best = p.best_quantity("macro.test.near", &req(&p, 3, 8), 100);
    assert_eq!(best.map(|q| q.units), Some(6));
    let hedge = submit(&mut p, 3, 6, 100).unwrap();
    assert_live(&p);
    dispatch_active(&mut p, position, 100);
    dispatch_active(&mut p, hedge, 100);
    p.settle(EXEC, hedge, true).unwrap();
    assert_eq!(p.values[0], 20);
    assert_live(&p);
    p.settle(EXEC, position, false).unwrap();
    assert_eq!(p.values[0], 0);
    p.observe(OWNER, vec![60, 40, 30, 40], 102).unwrap();

    let mut q = pool("risk");
    let done = submit(&mut q, 0, 1, 100).unwrap();
    dispatch_active(&mut q, done, 100);
    q.settle(EXEC, done, true).unwrap();
    submit(&mut q, 3, 8, 100).unwrap();
    assert_live(&q);
}

/// F-2, Reserved form, fixed: the position is never dispatched and the
/// hedge completes. The actor and the owner can cancel the position, also
/// at now = 1000, when its request has expired and the snapshot is stale.
#[test]
fn f2_reserved_position_can_always_be_cancelled() {
    let mut p = pool("risk");
    let position = submit(&mut p, 0, 1, 100).unwrap();
    assert_eq!(submit(&mut p, 3, 7, 100), Err(Error::Bounds));
    let hedge = submit(&mut p, 3, 6, 100).unwrap();
    dispatch_active(&mut p, hedge, 100);
    p.settle(EXEC, hedge, true).unwrap();
    assert_eq!(p.values, vec![20, 45, 30, 23]);
    assert_live(&p);
    p.clone().cancel(OWNER, position, 101).unwrap();
    p.clone().cancel(OWNER, position, EXPIRES).unwrap();
    p.cancel("credit.test.near", position, EXPIRES).unwrap();
    assert_eq!(p.values, vec![0, 40, 30, 28]);
    assert_eq!(p.records[position as usize].status, Status::Cancelled);
    p.observe(OWNER, vec![60, 40, 30, 40], EXPIRES).unwrap();
}

/// F-2b, fixed: in v1.0.0 record b's completion relied on record a's
/// reservation of -1,000,000, so undoing a left the value range and failed
/// Arithmetic. v1.1 counts a's release in b's interval: b x100 and x67 fail
/// Arithmetic at admission, x66 is admitted, and after b completes a can be
/// cancelled or settled false. formalver trace 208.
#[test]
fn f2b_release_stays_in_the_value_range() {
    const A: &str = "agent.test.near";
    let c = config(
        vec![metric(-MAX_VALUE, MAX_VALUE, 0), metric(-10, 10, 0)],
        vec![
            recipe(A, vec![-MAX_VALUE, 0], vec![0, 0], 1),
            recipe(A, vec![0, 0], vec![15_000, 0], 100),
        ],
    );
    let mut p = Pool::new(c, 100).unwrap();
    let a = submit(&mut p, 0, 1, 100).unwrap();
    let before = p.clone();
    assert_eq!(submit(&mut p, 1, 100, 100), Err(Error::Arithmetic));
    assert_eq!(submit(&mut p, 1, 67, 100), Err(Error::Arithmetic));
    assert_eq!(p, before);
    let best = p.best_quantity(A, &req(&p, 1, 100), 100);
    assert_eq!(best.map(|q| q.units), Some(66));
    let b = submit(&mut p, 1, 66, 100).unwrap();
    assert_live(&p);
    dispatch_active(&mut p, b, 100);
    p.settle(EXEC, b, true).unwrap();
    assert_eq!(p.values[0], -10_000);
    assert_live(&p);
    let mut flight = p.clone();
    dispatch_active(&mut flight, a, 101);
    flight.settle(EXEC, a, false).unwrap();
    assert_eq!(flight.values[0], 990_000);
    p.clone().cancel(OWNER, a, 101).unwrap();
    p.cancel(A, a, 101).unwrap();
    assert_eq!(p.values[0], 990_000);
}

/// xorshift64, so the random test needs no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform enough in [lo, hi].
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
    fn coin(&mut self) -> bool {
        self.next() & 1 == 0
    }
}
/// Half of the effects are zero, the rest in [-3, 3].
fn effect(g: &mut Rng) -> i64 {
    if g.coin() {
        0
    } else {
        g.range(-3, 3)
    }
}
/// Small random configuration: 1-3 metrics with narrow bounds, 1-2 terms,
/// 1-3 recipes with small effects, two actors, two executors.
fn random_config(g: &mut Rng) -> Config {
    let n = g.range(1, 3) as usize;
    let metrics = (0..n)
        .map(|_| {
            let min = g.range(-6, 2);
            let max = min + g.range(2, 12);
            metric(min, max, g.range(min, max))
        })
        .collect();
    let shapes = [Shape::Target, Shape::Excess, Shape::Deficit];
    let terms = (0..g.range(1, 2))
        .map(|_| Term {
            metric: g.range(0, n as i64 - 1) as u32,
            target: g.range(-4, 6),
            weight: g.range(1, 3) as u32,
            shape: shapes[g.range(0, 2) as usize].clone(),
        })
        .collect();
    let recipes = (0..g.range(1, 3))
        .map(|i| Recipe {
            name: "r".into(),
            actor: ["a1.test.near", "a2.test.near"][g.range(0, 1) as usize].into(),
            executor: ["x1.test.near", "x2.test.near"][g.range(0, 1) as usize].into(),
            reserve: (0..n).map(|_| effect(g)).collect(),
            complete: (0..n).map(|_| effect(g)).collect(),
            max_units: g.range(1, 3) as u32,
            requires: if i > 0 && g.range(0, 4) == 0 {
                vec![(0, 1)]
            } else {
                vec![]
            },
        })
        .collect();
    Config {
        name: "random".into(),
        owner: OWNER.into(),
        metrics,
        terms,
        recipes,
        max_age_seconds: if g.range(0, 3) == 0 { 50 } else { 0 },
    }
}

/// Measured values up to 3 outside each metric's bounds.
fn observed(g: &mut Rng, p: &Pool) -> Vec<i64> {
    let m = &p.config.metrics;
    m.iter().map(|m| g.range(m.min - 3, m.max + 3)).collect()
}
/// LIVE on random runs: 300 seeded random configurations, 60 random calls
/// each, including observes that put metrics out of bounds. After every
/// call: a rejected call leaves the pool unchanged; preview agrees with
/// submit; an admission leaves every metric's violation unchanged and no
/// other call but observe raises one (HARD-STEP); an observe with a record
/// open changes no value or record; INV-RES holds; and every open record
/// can be resolved (assert_live).
#[test]
fn live_all_on_random_runs() {
    let (mut admitted, mut admitted_disturbed, mut exits, mut exits_disturbed) = (0, 0, 0, 0);
    let mut refreshes = 0;
    for seed in 1..=300u64 {
        let mut g = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut p = Pool::new(random_config(&mut g), 100).unwrap();
        let mut now = 100u64;
        // Half of the runs start from an observed, often disturbed, state.
        if g.coin() {
            let values = observed(&mut g, &p);
            p.observe(OWNER, values, now).unwrap();
        }
        for _ in 0..60 {
            if g.range(0, 9) == 0 {
                now += g.range(1, 30) as u64;
            }
            let before = p.clone();
            let violated = p.violations(&p.values).iter().any(|v| *v > 0);
            let next = p.records.len() as i64;
            let id = g.range((next - 6).max(0), next) as u32;
            let known = p.records.get(id as usize).map(|c| {
                let r = &p.config.recipes[c.recipe as usize];
                (r.actor.clone(), r.executor.clone())
            });
            let (actor, executor) = known.unwrap_or(("a1.test.near".into(), "x1.test.near".into()));
            let k = g.range(0, 99);
            let (result, is_submit, is_observe) = if k < 30 {
                let recipe = g.range(0, p.config.recipes.len() as i64 - 1) as u32;
                let r = &p.config.recipes[recipe as usize];
                let a = r.actor.clone();
                // Mostly a valid quantity; one call in eight asks for one too many.
                let most = i64::from(r.max_units) + i64::from(g.range(0, 7) == 0);
                let q = Request {
                    recipe,
                    units: g.range(1, most) as u32,
                    version: p.version,
                    expires_at: now + 100,
                };
                let allowed = p.preview(&a, &q, now).allowed;
                let result = p.submit(&a, q, now).map(|_| ());
                assert_eq!(allowed, result.is_ok(), "preview disagrees with submit");
                (result, true, false)
            } else if k < 48 {
                (p.dispatch(&actor, id, now), false, false)
            } else if k < 60 {
                (p.acknowledge(id, g.coin()), false, false)
            } else if k < 78 {
                (p.settle(&executor, id, g.coin()), false, false)
            } else if k < 88 {
                let who = if g.coin() { OWNER.to_string() } else { actor };
                (p.cancel(&who, id, now), false, false)
            } else if k < 96 {
                let values = if g.range(0, 2) == 0 {
                    p.values.clone()
                } else {
                    observed(&mut g, &p)
                };
                (p.observe(OWNER, values, now), false, true)
            } else {
                // Paused about a quarter of the time.
                (p.set_paused(OWNER, g.range(0, 3) == 0), false, false)
            };
            if result.is_err() {
                assert_eq!(p, before, "a rejected call changed the pool");
            } else if is_observe {
                // With a record open, only a restatement of the values passes.
                if before.records.iter().any(|c| open(&c.status)) {
                    assert_eq!((&p.values, &p.records), (&before.values, &before.records));
                    refreshes += 1;
                }
            } else {
                let (old, new) = (before.violations(&before.values), p.violations(&p.values));
                if is_submit {
                    assert_eq!(new, old, "an admission changed a violation");
                    admitted += 1;
                    admitted_disturbed += usize::from(violated);
                } else {
                    assert!(new.iter().zip(&old).all(|(a, b)| a <= b), "HARD-STEP");
                    let closed = before
                        .records
                        .iter()
                        .zip(&p.records)
                        .any(|(a, b)| open(&a.status) && !open(&b.status));
                    exits += usize::from(closed);
                    exits_disturbed += usize::from(closed && violated);
                }
            }
            assert_inv_res(&p);
            assert_live(&p);
        }
    }
    // The run is not vacuous: it admits and resolves records, also in
    // states where some metric is out of bounds.
    println!("admitted {admitted} ({admitted_disturbed} disturbed), exits {exits} ({exits_disturbed} disturbed), refreshes with records open {refreshes}");
    assert!(admitted > 500 && admitted_disturbed > 30);
    assert!(exits > 200 && exits_disturbed > 20);
    assert!(refreshes > 20);
}

/// F-3 (low): one actor fills the 256 lifetime records with submit and
/// cancel. Every later submit, by any actor, fails Full, also after an
/// observe. formalver trace 207 (a smaller config).
#[test]
fn f3_submit_cancel_cycles_exhaust_lifetime_records() {
    let mut p = pool("gpu");
    for _ in 0..MAX_RECORDS {
        let id = submit(&mut p, 0, 1, 100).unwrap();
        p.cancel("research.test.near", id, 100).unwrap();
    }
    assert_eq!(p.records.len(), MAX_RECORDS);
    assert_eq!(p.version, 2 * MAX_RECORDS as u32);
    assert_eq!(submit(&mut p, 2, 1, 100), Err(Error::Full));
    let values = p.values.clone();
    p.observe(OWNER, values, 101).unwrap();
    assert_eq!(submit(&mut p, 2, 1, 101), Err(Error::Full));
}

/// F-4 (low), fixed: v1.0.0 accepted a recipe whose actor is its executor,
/// and one whose actor and executor are both the owner; in the hiring
/// fixture each could then accept its own work (spec:64). v1.1 rejects
/// both. The owner may still be a recipe's actor or its executor, but not
/// both. formalver trace 210 (a smaller config).
#[test]
fn f4_actor_cannot_be_its_own_executor() {
    const WORKER: &str = "research.test.near";
    let mut c = fixtures::config("hiring").unwrap();
    c.recipes[0].executor = WORKER.into();
    assert_eq!(c.validate(), Err(Error::InvalidConfig));
    assert_eq!(Pool::new(c, 100), Err(Error::InvalidConfig));
    let mut c = fixtures::config("hiring").unwrap();
    c.recipes[1].actor = OWNER.into();
    c.recipes[1].executor = OWNER.into();
    assert_eq!(c.validate(), Err(Error::InvalidConfig));
    assert_eq!(Pool::new(c, 100), Err(Error::InvalidConfig));

    // Owner as actor, with an independent executor: valid and usable.
    let mut c = fixtures::config("hiring").unwrap();
    c.recipes[1].actor = OWNER.into();
    let mut p = Pool::new(c, 100).unwrap();
    let o = submit(&mut p, 1, 3, 100).unwrap();
    dispatch_active(&mut p, o, 100);
    assert_eq!(p.settle(OWNER, o, true), Err(Error::Unauthorized));
    p.settle(EXEC, o, true).unwrap();
    assert_eq!(p.values[6], 3);
    // Owner as executor of another account's recipe: valid.
    let mut c = fixtures::config("hiring").unwrap();
    c.recipes[1].executor = OWNER.into();
    assert_eq!(c.validate(), Ok(()));
    for name in fixtures::NAMES {
        assert_eq!(fixtures::config(name).map(|c| c.validate()), Ok(Ok(())));
    }
}

/// F-5 (low), fixed: v1.0.0 froze every transition at version u32::MAX,
/// settlement of in-flight records included. v1.1 stops submit, dispatch,
/// set_paused and observe at VERSION_CAP, 768 versions below u32::MAX, and
/// keeps the rest as a reserve: acknowledgment, settlement and cancellation
/// still work above the cap. Reaching the cap takes about 2^32 effective
/// transitions, so the test sets the field.
#[test]
fn f5_open_records_resolve_above_the_version_cap() {
    let mut p = pool("gpu");
    let flight = submit(&mut p, 0, 1, 100).unwrap();
    p.dispatch("research.test.near", flight, 100).unwrap();
    let reserved = submit(&mut p, 2, 1, 100).unwrap();
    p.version = VERSION_CAP;
    let frozen = p.clone();
    // preview now agrees with submit (v1.0.0 said allowed here).
    let q = req(&p, 4, 1);
    let d = p.preview("data.test.near", &q, 100);
    assert!(!d.allowed);
    assert_eq!(d.reason, "Arithmetic");
    assert_eq!(p.best_quantity("data.test.near", &q, 100), None);
    assert_eq!(p.submit("data.test.near", q, 100), Err(Error::Arithmetic));
    assert_eq!(
        p.dispatch("evaluation.test.near", reserved, 100),
        Err(Error::Arithmetic)
    );
    assert_eq!(p.set_paused(OWNER, true), Err(Error::Arithmetic));
    assert_eq!(
        p.observe(OWNER, frozen.values.clone(), 100),
        Err(Error::Arithmetic)
    );
    assert_eq!(p, frozen);
    assert_live(&p);
    // The longest path of the in-flight record, and the cancel.
    p.acknowledge(flight, false).unwrap();
    p.acknowledge(flight, true).unwrap();
    p.settle(EXEC, flight, true).unwrap();
    p.cancel("evaluation.test.near", reserved, 100).unwrap();
    assert_eq!(p.version, VERSION_CAP + 4);
    assert!(p.records.iter().all(|c| !open(&c.status)));
    // The pool is retired: nothing new starts, also with every record terminal.
    let values = p.values.clone();
    assert_eq!(p.observe(OWNER, values, 100), Err(Error::Arithmetic));
    assert_eq!(submit(&mut p, 4, 1, 100), Err(Error::Arithmetic));
}

/// F-5, paused form, fixed: in v1.0.0 a paused pool at version u32::MAX
/// could not be unpaused, observed or settled. In v1.1 the last pause that
/// fits takes the version to VERSION_CAP. From there pause, unpause and
/// observe fail Arithmetic, and the open records still resolve.
#[test]
fn f5_paused_pool_at_the_cap_still_settles() {
    let mut p = pool("gpu");
    let initial = p.values.clone();
    let flight = submit(&mut p, 0, 1, 100).unwrap();
    p.dispatch("research.test.near", flight, 100).unwrap();
    let reserved = submit(&mut p, 2, 1, 100).unwrap();
    p.version = VERSION_CAP - 1;
    p.set_paused(OWNER, true).unwrap();
    assert_eq!(p.version, VERSION_CAP);
    let frozen = p.clone();
    assert_eq!(p.set_paused(OWNER, false), Err(Error::Arithmetic));
    assert_eq!(p.set_paused(OWNER, true), Err(Error::Arithmetic));
    assert_eq!(
        p.observe(OWNER, frozen.values.clone(), 100),
        Err(Error::Arithmetic)
    );
    assert_eq!(p, frozen);
    assert_live(&p);
    p.settle(EXEC, flight, false).unwrap();
    p.cancel(OWNER, reserved, 100).unwrap();
    assert_eq!(p.values, initial);
    assert_eq!(p.version, VERSION_CAP + 2);
    assert!(p.paused);
}

/// F-5: the reserve is exactly enough. All 256 records are Dispatched when
/// the version reaches VERSION_CAP. Each then takes the longest path
/// (Unknown, Active, Completed), three increments, and the version ends at
/// exactly u32::MAX.
#[test]
fn f5_reserve_is_exactly_enough() {
    const A: &str = "agent.test.near";
    assert_eq!(
        u64::from(VERSION_CAP) + 3 * MAX_RECORDS as u64,
        u64::from(u32::MAX)
    );
    let c = config(
        vec![metric(0, 10, 5), metric(-100, 100, 5)],
        vec![recipe(A, vec![0, 0], vec![0, 0], 1)],
    );
    let mut p = Pool::new(c, 100).unwrap();
    let last = MAX_RECORDS as u32 - 1;
    for id in 0..last {
        assert_eq!(submit(&mut p, 0, 1, 100), Ok(id));
        p.dispatch(A, id, 100).unwrap();
    }
    // The last submit and dispatch are the last two increments below the cap.
    p.version = VERSION_CAP - 2;
    assert_eq!(submit(&mut p, 0, 1, 100), Ok(last));
    p.dispatch(A, last, 100).unwrap();
    assert_eq!(p.version, VERSION_CAP);
    for id in 0..=last {
        p.acknowledge(id, false).unwrap();
        p.acknowledge(id, true).unwrap();
        p.settle(EXEC, id, true).unwrap();
    }
    assert_eq!(p.version, u32::MAX);
    assert!(p.records.iter().all(|c| c.status == Status::Completed));
}

/// F-6 (low), fixed: the risk fixture has max_age 300 and was observed at
/// 100. At now = 401, with one record Active and one Reserved, submit and
/// dispatch fail SnapshotExpired. In v1.0.0 observe failed Pending until
/// every record was terminal. v1.1 lets the owner restate the current
/// values with records open: the values and records stay as they are, the
/// snapshot timestamp is renewed and the version moves by one. New values
/// still wait for every record to be terminal. formalver trace 211 (a
/// smaller config).
#[test]
fn f6_owner_refreshes_the_snapshot_with_records_open() {
    let mut p = pool("risk");
    let slow = submit(&mut p, 0, 1, 100).unwrap();
    dispatch_active(&mut p, slow, 100);
    let next = submit(&mut p, 3, 1, 100).unwrap();
    assert_eq!(submit(&mut p, 1, 1, 401), Err(Error::SnapshotExpired));
    assert_eq!(
        p.dispatch("macro.test.near", next, 401),
        Err(Error::SnapshotExpired)
    );
    let before = p.clone();
    let fixture = vec![60, 40, 30, 40];
    assert_eq!(p.observe(OWNER, fixture, 401), Err(Error::Pending));
    let current = p.values.clone();
    assert_eq!(
        p.observe("credit.test.near", current.clone(), 401),
        Err(Error::Unauthorized)
    );
    assert_eq!(p, before);
    p.observe(OWNER, current.clone(), 401).unwrap();
    assert_eq!(p.observed_at, 401);
    assert_eq!(p.version, before.version + 1);
    assert_eq!(p.values, current);
    assert_eq!(p.records, before.records);
    assert_eq!(p.records[slow as usize].status, Status::Active);
    assert_live(&p);
    p.dispatch("macro.test.near", next, 401).unwrap();
    submit(&mut p, 3, 1, 401).unwrap();
    assert_live(&p);
}

/// div-cancel-terminal-name: cancelling a Completed or Dispatched record
/// fails with Pending, a name that suggests an open commitment.
#[test]
fn div_cancel_of_completed_or_dispatched_record_says_pending() {
    let mut p = pool("gpu");
    let done = submit(&mut p, 0, 1, 100).unwrap();
    dispatch_active(&mut p, done, 100);
    p.settle(EXEC, done, true).unwrap();
    let sent = submit(&mut p, 0, 1, 100).unwrap();
    p.dispatch("research.test.near", sent, 100).unwrap();
    assert_eq!(
        p.cancel("research.test.near", done, 100),
        Err(Error::Pending)
    );
    assert_eq!(p.cancel(OWNER, sent, 100), Err(Error::Pending));
}
