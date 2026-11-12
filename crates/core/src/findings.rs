//! Reproducers for docs/verification-findings.md.
//!
//! These tests pin v1.0.0 behaviour; each v1.1 fix flips its test. Every
//! assertion states what the core returns today, including the faulty
//! results, so CI stays green on v1.0.0 and a fix commit has to change the
//! test that pins the behaviour it fixes.
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

/// F-1 (high): an observe puts metric 0 at 12 (max 10). The recovery rule
/// (spec:23) admits a hedge that reserves -2 and completes +2 on metric 0.
/// Both settlements then fail Bounds, cancel and observe fail Pending, and
/// the record can never become terminal. formalver trace 0.
#[test]
fn f1_disturbed_state_hedge_can_never_settle() {
    const HEDGER: &str = "hedger.test.near";
    let c = config(
        vec![metric(0, 10, 5), metric(-100, 100, 5)],
        vec![recipe(HEDGER, vec![-2, 0], vec![2, -5], 1)],
    );
    let mut p = Pool::new(c, 100).unwrap();
    p.observe(OWNER, vec![12, 5], 100).unwrap();
    let id = submit(&mut p, 0, 1, 100).unwrap();
    assert_eq!(p.values, vec![10, 5]);
    dispatch_active(&mut p, id, 101);
    let wedged = p.clone();
    assert_eq!(p.settle(EXEC, id, true), Err(Error::Bounds));
    assert_eq!(p.settle(EXEC, id, false), Err(Error::Bounds));
    assert_eq!(p.cancel(OWNER, id, 102), Err(Error::Pending));
    assert_eq!(p.cancel(HEDGER, id, 102), Err(Error::Pending));
    assert_eq!(p.observe(OWNER, vec![5, 5], 103), Err(Error::Pending));
    // A later callback for an Active record is a no-op.
    p.acknowledge(id, true).unwrap();
    p.acknowledge(id, false).unwrap();
    assert_eq!(p, wedged);
    assert_eq!(p.records[id as usize].status, Status::Active);
}

/// F-2 (high): the supplied `risk` fixture, healthy, no observe. A hedge of
/// 8 units is admitted only because a credit position's +20 selloff loss
/// is counted. After the hedge completes, the executor's honest
/// settle(false) for the position fails Bounds, and cancel and observe fail
/// Pending. Inside the snapshot window, only new exposure that adds back 20
/// selloff loss lets the release pass. After it, only settle(true), for
/// work that did not run, ends the record. formalver trace 209.
#[test]
fn f2_risk_position_cannot_be_released_after_hedge_completes() {
    let mut p = pool("risk");
    // Without the position, 8 hedge units would take selloff loss to 60 - 80.
    let mut alone = p.clone();
    assert_eq!(submit(&mut alone, 3, 8, 100), Err(Error::Bounds));
    let position = submit(&mut p, 0, 1, 100).unwrap();
    let hedge = submit(&mut p, 3, 8, 100).unwrap();
    assert_eq!(p.values, vec![80, 45, 30, 19]);
    dispatch_active(&mut p, position, 100);
    dispatch_active(&mut p, hedge, 100);
    p.settle(EXEC, hedge, true).unwrap();
    assert_eq!(p.values[0], 0);
    let wedged = p.clone();
    assert_eq!(p.settle(EXEC, position, false), Err(Error::Bounds));
    assert_eq!(p.cancel(OWNER, position, 101), Err(Error::Pending));
    assert_eq!(
        p.cancel("credit.test.near", position, 101),
        Err(Error::Pending)
    );
    assert_eq!(
        p.observe(OWNER, vec![60, 40, 30, 40], 102),
        Err(Error::Pending)
    );
    assert_eq!(p, wedged);
    // Inside the snapshot window, 2 new equity units add 20 selloff loss.
    let mut more = p.clone();
    submit(&mut more, 1, 2, 101).unwrap();
    more.settle(EXEC, position, false).unwrap();
    // Observed at 100 with max_age 300: at 401 that way is closed too.
    assert_eq!(submit(&mut p, 1, 2, 401), Err(Error::SnapshotExpired));
    p.settle(EXEC, position, true).unwrap();
    p.observe(OWNER, vec![60, 40, 30, 40], 402).unwrap();
}

/// F-2, Reserved form: the same release check blocks cancel of a position
/// that was never dispatched. At now = 1000 its request has expired and the
/// snapshot is stale: dispatch fails Expired, submit SnapshotExpired, settle
/// InvalidState, observe Pending. No call changes the record or the values
/// again; set_paused only changes the pause flag and the version.
#[test]
fn f2_risk_reserved_position_is_stuck_after_hedge_completes() {
    let mut p = pool("risk");
    let position = submit(&mut p, 0, 1, 100).unwrap();
    let hedge = submit(&mut p, 3, 7, 100).unwrap();
    dispatch_active(&mut p, hedge, 100);
    p.settle(EXEC, hedge, true).unwrap();
    assert_eq!(p.values[0], 10);
    let wedged = p.clone();
    assert_eq!(
        p.cancel("credit.test.near", position, 101),
        Err(Error::Bounds)
    );
    assert_eq!(p.cancel(OWNER, position, 101), Err(Error::Bounds));
    assert_eq!(p.cancel(OWNER, position, EXPIRES), Err(Error::Bounds));
    assert_eq!(
        p.dispatch("credit.test.near", position, EXPIRES),
        Err(Error::Expired)
    );
    let late = Request {
        expires_at: 2 * EXPIRES,
        ..req(&p, 1, 2)
    };
    assert_eq!(
        p.submit("equity.test.near", late, EXPIRES),
        Err(Error::SnapshotExpired)
    );
    assert_eq!(p.settle(EXEC, position, true), Err(Error::InvalidState));
    assert_eq!(p.settle(EXEC, position, false), Err(Error::InvalidState));
    assert_eq!(
        p.observe(OWNER, vec![60, 40, 30, 40], EXPIRES),
        Err(Error::Pending)
    );
    assert_eq!(p, wedged);
    assert_eq!(p.records[position as usize].status, Status::Reserved);
}

/// F-2b: a release is not bounded by the envelope. Record b's completion
/// relied on record a's reservation of -1,000,000; undoing it would give
/// 1,500,000, so cancel and the honest settle(false) fail Arithmetic.
/// formalver trace 208.
#[test]
fn f2b_release_outside_value_range_fails_arithmetic() {
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
    let b = submit(&mut p, 1, 100, 100).unwrap();
    dispatch_active(&mut p, b, 100);
    p.settle(EXEC, b, true).unwrap();
    assert_eq!(p.values[0], 500_000);
    let before = p.clone();
    assert_eq!(p.cancel(A, a, 101), Err(Error::Arithmetic));
    assert_eq!(p.cancel(OWNER, a, 101), Err(Error::Arithmetic));
    assert_eq!(p, before);
    dispatch_active(&mut p, a, 101);
    let wedged = p.clone();
    assert_eq!(p.settle(EXEC, a, false), Err(Error::Arithmetic));
    assert_eq!(p.observe(OWNER, vec![500_000, 0], 102), Err(Error::Pending));
    assert_eq!(p, wedged);
    p.settle(EXEC, a, true).unwrap();
    p.observe(OWNER, vec![500_000, 0], 103).unwrap();
}

/// F-3 (low): one actor fills the 256 lifetime records with submit and
/// cancel. Every later submit, by any actor, fails Full, also after an
/// observe. formalver trace 207.
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

/// F-4 (low): validate accepts a recipe whose actor is its executor, and
/// one whose actor and executor are the owner. In the hiring fixture each
/// of them accepts its own work (spec:64). formalver trace 210.
#[test]
fn f4_validate_accepts_actor_or_owner_as_executor() {
    const WORKER: &str = "research.test.near";
    let mut c = fixtures::config("hiring").unwrap();
    c.recipes[0].executor = WORKER.into();
    c.recipes[1].actor = OWNER.into();
    c.recipes[1].executor = OWNER.into();
    assert_eq!(c.validate(), Ok(()));
    let mut p = Pool::new(c, 100).unwrap();
    let w = submit(&mut p, 0, 2, 100).unwrap();
    dispatch_active(&mut p, w, 100);
    p.settle(WORKER, w, true).unwrap();
    let o = submit(&mut p, 1, 3, 100).unwrap();
    dispatch_active(&mut p, o, 100);
    p.settle(OWNER, o, true).unwrap();
    // Paid lifetime 2 + 3, with no independent acceptance.
    assert_eq!(p.values[6], 5);
}

/// F-5 (low): at version u32::MAX every transition fails Arithmetic,
/// including settlement of in-flight records. Reaching it takes 2^32 - 1
/// effective transitions, so the test sets the field.
#[test]
fn f5_version_max_freezes_settlement_and_every_transition() {
    let mut p = pool("gpu");
    let flight = submit(&mut p, 0, 1, 100).unwrap();
    p.dispatch("research.test.near", flight, 100).unwrap();
    let reserved = submit(&mut p, 2, 1, 100).unwrap();
    p.version = u32::MAX;
    let frozen = p.clone();
    assert_eq!(p.settle(EXEC, flight, true), Err(Error::Arithmetic));
    assert_eq!(p.settle(EXEC, flight, false), Err(Error::Arithmetic));
    assert_eq!(p.acknowledge(flight, true), Err(Error::Arithmetic));
    assert_eq!(p.acknowledge(flight, false), Err(Error::Arithmetic));
    assert_eq!(
        p.dispatch("evaluation.test.near", reserved, 100),
        Err(Error::Arithmetic)
    );
    assert_eq!(
        p.cancel("evaluation.test.near", reserved, 100),
        Err(Error::Arithmetic)
    );
    assert_eq!(p.set_paused(OWNER, true), Err(Error::Arithmetic));
    assert_eq!(
        p.observe(OWNER, frozen.values.clone(), 100),
        Err(Error::Pending)
    );
    // Observation, not a finding: preview says allowed, submit fails.
    let q = req(&p, 4, 1);
    assert!(p.preview("data.test.near", &q, 100).allowed);
    assert_eq!(p.submit("data.test.near", q, 100), Err(Error::Arithmetic));
    assert_eq!(p, frozen);
}

/// F-5, paused form: a paused pool at version u32::MAX cannot be unpaused
/// or observed.
#[test]
fn f5_paused_pool_at_version_max_cannot_unpause() {
    let mut p = pool("gpu");
    p.set_paused(OWNER, true).unwrap();
    p.version = u32::MAX;
    let frozen = p.clone();
    assert_eq!(p.set_paused(OWNER, false), Err(Error::Arithmetic));
    assert_eq!(p.observe(OWNER, vec![0; 5], 100), Err(Error::Arithmetic));
    assert_eq!(p, frozen);
}

/// F-6 (low): the risk fixture has max_age 300 and was observed at 100. At
/// now = 401, with one record still Active, submit and dispatch fail
/// SnapshotExpired and observe fails Pending until the executor settles it.
/// formalver trace 211.
#[test]
fn f6_open_record_blocks_the_snapshot_refresh() {
    let mut p = pool("risk");
    let slow = submit(&mut p, 0, 1, 100).unwrap();
    dispatch_active(&mut p, slow, 100);
    let next = submit(&mut p, 3, 1, 100).unwrap();
    let measured = vec![60, 40, 30, 40];
    assert_eq!(submit(&mut p, 1, 1, 401), Err(Error::SnapshotExpired));
    assert_eq!(
        p.dispatch("macro.test.near", next, 401),
        Err(Error::SnapshotExpired)
    );
    assert_eq!(p.observe(OWNER, measured.clone(), 401), Err(Error::Pending));
    p.cancel("macro.test.near", next, 401).unwrap();
    assert_eq!(p.observe(OWNER, measured.clone(), 401), Err(Error::Pending));
    p.settle(EXEC, slow, true).unwrap();
    p.observe(OWNER, measured, 401).unwrap();
    assert!(submit(&mut p, 1, 1, 401).is_ok());
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
