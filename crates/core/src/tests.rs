use super::*;
fn pool(name: &str) -> Pool {
    Pool::new(fixtures::config(name).unwrap(), 100).unwrap()
}
fn req(p: &Pool, r: u32, u: u32) -> Request {
    Request {
        recipe: r,
        units: u,
        version: p.version,
        expires_at: 200,
    }
}
fn submit(p: &mut Pool, r: u32, u: u32) -> Result<u32, Error> {
    let a = p.config.recipes[r as usize].actor.clone();
    let q = req(p, r, u);
    p.submit(&a, q, 100)
}
fn finish(p: &mut Pool, id: u32) {
    let c = p.records[id as usize].clone();
    let r = p.config.recipes[c.recipe as usize].clone();
    p.dispatch(&r.actor, id, 100).unwrap();
    p.acknowledge(id, true).unwrap();
    p.settle(&r.executor, id, true).unwrap();
}
#[test]
fn capacity_trace() {
    for name in ["gpu", "friday"] {
        let mut p = pool(name);
        assert_eq!(p.energy(&p.values), Ok(48));
        submit(&mut p, 0, 4).unwrap();
        assert_eq!(p.energy(&p.values), Ok(32));
        submit(&mut p, 2, 2).unwrap();
        let before = p.clone();
        assert_eq!(submit(&mut p, 4, 4), Err(Error::Energy));
        assert_eq!(p, before);
        let a = p.config.recipes[4].actor.clone();
        let best = p.best_quantity(&a, &req(&p, 4, 4), 100).unwrap();
        assert_eq!(best.units, 2);
        submit(&mut p, 4, 2).unwrap();
        assert_eq!(p.energy(&p.values), Ok(8));
        submit(&mut p, 5, 2).unwrap();
        submit(&mut p, 3, 2).unwrap();
        assert_eq!(p.energy(&p.values), Ok(0));
    }
}
#[test]
fn hard_constraint_is_separate() {
    let mut p = pool("friday");
    p.config.metrics[5].max = 1;
    let before = p.clone();
    assert_eq!(submit(&mut p, 0, 2), Err(Error::Bounds));
    assert_eq!(p, before);
}
#[test]
fn inventory_conservation_and_pending_receipt() {
    let mut p = pool("inventory");
    assert_eq!(p.energy(&p.values), Ok(24));
    assert_eq!(submit(&mut p, 0, 2), Err(Error::Energy));
    let a = submit(&mut p, 1, 2).unwrap();
    assert_eq!(p.values[5], 0);
    assert_eq!(p.energy(&p.values), Ok(8));
    let b = submit(&mut p, 2, 2).unwrap();
    assert_eq!(p.energy(&p.values), Ok(0));
    assert_eq!(p.values[3..].iter().sum::<i64>(), 12);
    finish(&mut p, a);
    finish(&mut p, b);
    assert_eq!(p.values, vec![4, 4, 4, 4, 4, 4, 0]);
}
#[test]
fn hiring_review_is_earmarked_and_requires_acceptance() {
    let mut p = pool("hiring");
    assert_eq!(submit(&mut p, 0, 5), Err(Error::Energy));
    assert_eq!(submit(&mut p, 2, 2), Err(Error::Prerequisite));
    let a = submit(&mut p, 0, 2).unwrap();
    let b = submit(&mut p, 1, 3).unwrap();
    assert_eq!(p.values[3], 0);
    assert_eq!(p.values[4], 2);
    finish(&mut p, a);
    finish(&mut p, b);
    let c = submit(&mut p, 2, 2).unwrap();
    finish(&mut p, c);
    assert_eq!(p.values, vec![2, 3, 2, 0, 0, 0, 7, 3]);
    assert_eq!(p.energy(&p.values), Ok(0));
    assert_eq!(submit(&mut p, 0, 1), Err(Error::Bounds));
}
#[test]
fn risk_pool_pending_hedge_grants_no_credit() {
    let mut p = pool("risk");
    let a = p.config.recipes[0].actor.clone();
    let d = p.preview(&a, &req(&p, 0, 2), 100);
    assert_eq!(d.after, Some("400".into()));
    assert_eq!(submit(&mut p, 0, 2), Err(Error::Energy));
    submit(&mut p, 0, 1).unwrap();
    assert_eq!(submit(&mut p, 1, 1), Err(Error::Energy));
    let h = submit(&mut p, 3, 1).unwrap();
    assert_eq!(submit(&mut p, 1, 1), Err(Error::Energy));
    finish(&mut p, h);
    assert!(submit(&mut p, 1, 1).is_ok());
}
#[test]
fn stale_and_unauthorized_leave_state_unchanged() {
    let mut p = pool("gpu");
    let q = req(&p, 0, 1);
    let before = p.clone();
    assert_eq!(
        p.submit("attacker", q.clone(), 100),
        Err(Error::Unauthorized)
    );
    assert_eq!(p, before);
    submit(&mut p, 0, 1).unwrap();
    assert_eq!(p.submit("research.test.near", q, 100), Err(Error::Stale));
}
#[test]
fn unknown_execution_retains_reservations() {
    let mut p = pool("gpu");
    let id = submit(&mut p, 0, 4).unwrap();
    p.dispatch("research.test.near", id, 100).unwrap();
    p.acknowledge(id, false).unwrap();
    assert_eq!(
        p.cancel("research.test.near", id, 1000),
        Err(Error::Pending)
    );
    assert_eq!(p.values[3], 4);
    assert_eq!(p.settle("attacker", id, false), Err(Error::Unauthorized));
    p.settle("executor.test.near", id, false).unwrap();
    assert_eq!(p.values[3], 0);
    let before = p.clone();
    p.settle("executor.test.near", id, false).unwrap();
    assert_eq!(before, p);
}
#[test]
fn duplicate_completion_does_not_mint_stock() {
    let mut p = pool("inventory");
    let id = submit(&mut p, 1, 2).unwrap();
    finish(&mut p, id);
    let before = p.clone();
    p.settle("executor.test.near", id, true).unwrap();
    p.acknowledge(id, false).unwrap();
    assert_eq!(p, before);
}
#[test]
fn snapshot_expiry_and_observation() {
    let mut p = pool("risk");
    let q = Request {
        expires_at: 600,
        ..req(&p, 0, 1)
    };
    assert_eq!(
        p.submit("credit.test.near", q, 401),
        Err(Error::SnapshotExpired)
    );
    let owner = p.config.owner.clone();
    p.observe(&owner, vec![90, 40, 30, 40], 200).unwrap();
    assert_eq!(p.energy(&p.values), Ok(100));
    assert!(p.observe("attacker", vec![0; 4], 201).is_err());
}
#[test]
fn malformed_policies_rejected() {
    let mut c = fixtures::config("gpu").unwrap();
    c.terms[0].metric = 100;
    assert_eq!(Pool::new(c, 0), Err(Error::InvalidConfig));
    let mut c = fixtures::config("gpu").unwrap();
    c.recipes[0].requires = vec![(1, 1)];
    assert_eq!(Pool::new(c, 0), Err(Error::InvalidConfig));
}
#[test]
fn expiry_pause_and_cancel() {
    let mut p = pool("gpu");
    let id = submit(&mut p, 0, 2).unwrap();
    assert_eq!(
        p.dispatch("research.test.near", id, 200),
        Err(Error::Expired)
    );
    p.cancel("research.test.near", id, 200).unwrap();
    let owner = p.config.owner.clone();
    p.set_paused(&owner, true).unwrap();
    assert_eq!(submit(&mut p, 0, 1), Err(Error::Paused));
}
#[test]
fn bounded_exhaustive_single_admissions_preserve_policy() {
    for name in fixtures::NAMES {
        let p = pool(name);
        for r in 0..p.config.recipes.len() {
            for u in 0..=21 {
                let mut q = p.clone();
                let before = q.energy(&q.values).unwrap();
                let saved = q.clone();
                if submit(&mut q, r as u32, u).is_ok() {
                    assert!(q.energy(&q.values).unwrap() <= before);
                    assert!(q
                        .values
                        .iter()
                        .zip(&q.config.metrics)
                        .all(|(v, m)| *v >= m.min && *v <= m.max));
                } else {
                    assert_eq!(q, saved)
                }
            }
        }
    }
}
#[test]
fn arithmetic_and_record_limits() {
    let mut p = pool("gpu");
    assert_eq!(submit(&mut p, 0, u32::MAX), Err(Error::InvalidUnits));
    p.records = (0..MAX_RECORDS)
        .map(|id| Commitment {
            id: id as u32,
            recipe: 0,
            units: 1,
            expires_at: 200,
            status: Status::Cancelled,
        })
        .collect();
    assert_eq!(submit(&mut p, 0, 1), Err(Error::Full));
}

#[test]
fn partially_finished_milestone_does_not_unlock_review() {
    let mut p = pool("hiring");
    let a = submit(&mut p, 0, 1).unwrap();
    let b = submit(&mut p, 1, 3).unwrap();
    finish(&mut p, a);
    finish(&mut p, b);
    assert_eq!(submit(&mut p, 2, 2), Err(Error::Prerequisite));
}
#[test]
fn recovery_hedge_waits_for_fill_and_does_not_hide_violations() {
    let mut p = pool("risk");
    let owner = p.config.owner.clone();
    p.observe(&owner, vec![110, 40, 30, 40], 100).unwrap();
    assert_eq!(submit(&mut p, 0, 1), Err(Error::Bounds));
    let id = submit(&mut p, 3, 1).unwrap();
    assert_eq!(p.values[0], 110);
    finish(&mut p, id);
    assert_eq!(p.values[0], 100);
    assert_eq!(p.energy(&p.values), Ok(400));
}

#[test]
fn stale_risk_snapshot_blocks_dispatch_without_releasing() {
    let mut p = pool("risk");
    let q = Request {
        recipe: 0,
        units: 1,
        version: 0,
        expires_at: 1000,
    };
    let id = p.submit("credit.test.near", q, 100).unwrap();
    assert_eq!(
        p.dispatch("credit.test.near", id, 401),
        Err(Error::SnapshotExpired)
    );
    assert_eq!(p.records[0].status, Status::Reserved);
    assert_eq!(p.values[0], 80);
}
#[test]
fn maximum_energy_stays_exact_and_within_u64() {
    let mut c = fixtures::config("gpu").unwrap();
    c.metrics[0].min = -MAX_VALUE;
    c.metrics[0].max = MAX_VALUE;
    c.metrics[0].initial = MAX_VALUE;
    c.terms = vec![
        Term {
            metric: 0,
            target: -MAX_VALUE,
            weight: MAX_WEIGHT,
            shape: Shape::Target
        };
        MAX_TERMS
    ];
    let p = Pool::new(c, 100).unwrap();
    assert_eq!(p.energy(&p.values), Ok(256_000_000_000_000_000));
}
#[test]
fn completion_envelope_prevents_overcommitting_incoming_capacity() {
    let mut c = fixtures::config("gpu").unwrap();
    c.recipes[0].reserve = vec![0; 5];
    c.recipes[0].complete = vec![0, 0, 0, 1, 0];
    let mut p = Pool::new(c, 100).unwrap();
    submit(&mut p, 0, 12).unwrap();
    let prior = p.clone();
    assert_eq!(submit(&mut p, 0, 1), Err(Error::Bounds));
    assert_eq!(p, prior);
}
