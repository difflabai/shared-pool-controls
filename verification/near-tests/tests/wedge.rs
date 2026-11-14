//! F-1 (wedge) and F-2 (a hedge relying on an unexecuted position) through
//! the shell, on the host path and the documented build, and end to end with
//! the executor (RT-WEDGE in the mocked runtime).
//!
//! These were reproducers of v1.0.0 findings. v1.1.0 fixes both, so they are
//! now regression properties of F-1/F-2 (fixed in v1.1): if the fix is
//! reverted they fail.
//!
//! F-1 brief: metrics m(0,10,5), m(-100,100,5); term t(1,0,1,target); recipe
//! r(a1,x1,[-2,0],[2,-5],1,[]); observe [12,5]. v1.0.0 admitted the hedge,
//! after which settle(true) and settle(false) both failed Bounds and observe
//! failed Pending for good. v1.1.0 refuses the submit with Bounds and writes
//! nothing. The hedge shape that cures by completion (upstream
//! crates/core/src/findings.rs:175: reserve [0,-1], complete [-3,0]) is
//! admitted in the same disturbed state, and every exit stays open.
//! F-2 (the `risk` fixture, findings.rs:209,241): a selloff hedge may not
//! rely on a credit position's reservation; once the hedge completes, the
//! position's honest release (settle(false) via `confirm_not_executed`, or
//! cancel while Reserved) is Ok.
//!
//! F-1 has no catalogue ID. It is the counterexample to LIVE-2, so these
//! tests carry LIVE-2, the control carries LIVE-1, and the end-to-end test
//! also carries RT-WEDGE (the sandbox property it mirrors in the mocked
//! runtime).
//!
// PROP: LIVE-2 LIVE-1 RT-WEDGE SH-3 SH-5 EX-4 EX-5

mod common;
use common::*;
use near_sdk::serde_json::{json, Value};
use pool_core::{Config, Pool, Request, Status};

/// The F-1 hedge shape that stays allowed: it reserves collateral only
/// (metric 1) and cures metric 0 when it completes.
fn cure_config() -> Config {
    let mut c = wedge_config();
    c.name = "cure".into();
    c.recipes = vec![recipe(A1, X1, vec![0, -1], vec![-3, 0], 1)];
    c
}

/// An independent copy of `s` (same path, storage and core state).
fn fork(s: &Sim) -> Sim {
    Sim {
        path: s.path,
        storage: s.storage.clone(),
        pool: s.pool.clone(),
        n: s.n,
        live_all: s.live_all,
    }
}

/// Observe [12,5] (metric 0 above its max), then submit 1 unit of recipe 0.
fn disturbed_submit(s: &mut Sim) -> Ran {
    s.ok(&Ctx::by(OWNER, 100), Op::Observe { values: vec![12, 5] });
    s.ok(&Ctx::by(A1, 101), submit(0, 1, s.pool.version, 150))
}

/// LIVE: every open record can be ended both ways. On a copy, a Reserved
/// record is cancelled by its actor and by the owner; an in-flight record is
/// settled true and false by its executor. Each exit is Ok, and once nothing
/// is open the owner can observe again.
fn assert_resolvable(s: &Sim) {
    let open: Vec<_> = s
        .pool
        .records
        .iter()
        .filter(|c| !matches!(c.status, Status::Completed | Status::Cancelled))
        .cloned()
        .collect();
    for c in &open {
        let r = s.pool.config.recipes[c.recipe as usize].clone();
        let exits: Vec<(&str, Op)> = if c.status == Status::Reserved {
            vec![(r.actor.as_str(), Op::Cancel { id: c.id }), (OWNER, Op::Cancel { id: c.id })]
        } else {
            vec![
                (r.executor.as_str(), Op::Settle { id: c.id, completed: true }),
                (r.executor.as_str(), Op::Settle { id: c.id, completed: false }),
            ]
        };
        for (who, op) in exits {
            let mut t = fork(s);
            t.ok(&Ctx::by(who, 102), op);
            assert!(matches!(t.status(c.id), Status::Completed | Status::Cancelled));
            if open.len() == 1 {
                let values = t.pool.values.clone();
                t.ok(&Ctx::by(OWNER, 103), Op::Observe { values });
            }
        }
    }
}

// PROP: LIVE-2 SH-3 SH-5
#[test]
fn f1_hedge_is_refused_through_the_shell() {
    big_stack(|| {
        for path in paths() {
            // The wedge hedge: refused with Bounds, nothing written.
            let mut s = Sim::new(path, wedge_config(), 100);
            s.ok(&Ctx::by(OWNER, 100), Op::Observe { values: vec![12, 5] });
            let (storage, pool) = (s.storage.clone(), s.pool.clone());
            let r = s.ok(&Ctx::by(A1, 101), submit(0, 1, s.pool.version, 150));
            assert_eq!(r.value()["decision"]["allowed"], json!(false));
            assert_eq!(r.value()["decision"]["reason"], json!("Bounds"), "F-1: the reservation-cure hedge is refused");
            assert_eq!(r.value()["id"], Value::Null);
            assert_eq!(r.event()["id"], Value::Null);
            assert_eq!((&s.storage, &s.pool), (&storage, &pool), "a refused submit writes nothing");
            assert!(s.pool.records.is_empty());
            // Nothing is open, so the owner can restate the values.
            s.ok(&Ctx::by(OWNER, 103), Op::Observe { values: vec![5, 5] });

            // The cure-by-completion hedge: admitted, and every exit stays
            // open at each step, whatever the executor answers.
            for answer in [None, Some("active"), Some("completed")] {
                let mut s = Sim::new(path, cure_config(), 100);
                let r = disturbed_submit(&mut s);
                assert_eq!(r.value()["decision"]["reason"], json!("Allowed"));
                assert_eq!(r.value()["id"], json!(0));
                assert_eq!(s.pool.values, vec![12, 4]);
                assert_resolvable(&s);
                s.ok(&Ctx::by(A1, 101), Op::Dispatch { id: 0 });
                assert_resolvable(&s);
                let r = s.ok(&Ctx::by(POOL, 102), Op::cb(0, answer));
                assert_eq!(r.event()["error"], Value::Null, "{answer:?}");
                let expect = match answer {
                    None => Status::Unknown,
                    Some("active") => Status::Active,
                    _ => Status::Completed,
                };
                assert_eq!(s.status(0), expect);
                assert_resolvable(&s);
                if expect != Status::Completed {
                    s.ok(&Ctx::by(X1, 102), Op::Settle { id: 0, completed: true });
                }
                assert_eq!(s.status(0), Status::Completed);
                assert_eq!(s.pool.values, vec![9, 4]);
                s.ok(&Ctx::by(OWNER, 103), Op::Observe { values: vec![5, 5] });
            }
        }
    });
}

/// Control: the same recipe from an undisturbed baseline settles normally
/// (LIVE-1 holds when the base violation is 0).
// PROP: LIVE-1
#[test]
fn f1_control_without_the_disturbing_observe() {
    big_stack(|| {
        for path in paths() {
            let mut s = Sim::new(path, wedge_config(), 100);
            s.ok(&Ctx::by(A1, 101), submit(0, 1, 0, 150));
            s.ok(&Ctx::by(A1, 101), Op::Dispatch { id: 0 });
            s.ok(&Ctx::by(X1, 102), Op::Settle { id: 0, completed: true });
            assert_eq!(s.status(0), Status::Completed);
            assert_eq!(s.pool.values, vec![5, 0]);
            s.ok(&Ctx::by(OWNER, 103), Op::Observe { values: vec![5, 5] });
        }
    });
}

fn submit_tx(net: &mut Net, who: &str, recipe: u32, units: u32, expires_at: u64) -> Value {
    let q = Request { recipe, units, version: net.pool_state().version, expires_at };
    net.tx(who, POOL, "submit", json!({ "request": request_json(&q) })).json()
}

fn methods(net: &Net, from: usize) -> Vec<(String, bool)> {
    net.trace[from..].iter().map(|(_, m, r, _)| (m.clone(), r.is_ok())).collect()
}

fn ms(v: &[(&str, bool)]) -> Vec<(String, bool)> {
    v.iter().map(|(m, ok)| (m.to_string(), *ok)).collect()
}

/// RT-WEDGE in the mocked runtime: pool and executor wasm with the receipt
/// router. F-1: the wedge hedge is refused and nothing moves; the
/// cure-by-completion hedge runs `dispatch -> execute -> on_result`, and the
/// operator's `finish(true)` settles it (`settle` Ok, then `on_settled`); the
/// operator cannot switch outcome afterwards, and the owner can observe.
/// F-2 (risk fixture): the hedge is sized as if the position were released;
/// after it completes, the position's honest release is Ok, whether the
/// position was dispatched (its execute failed, so the operator confirms
/// non-execution) or is still Reserved (its actor cancels).
// PROP: LIVE-2 RT-WEDGE EX-4 EX-5
#[test]
fn f1_no_wedge_end_to_end_with_the_executor() {
    big_stack(|| {
        for b in Build::all() {
            // F-1, the wedge hedge.
            let mut net = Net::new(b, &wedge_config(), X1, 100);
            assert!(net.tx(OWNER, POOL, "observe", json!({ "values": [12, 5] })).result.is_ok());
            let before = net.pool_state();
            let out = submit_tx(&mut net, A1, 0, 1, 150);
            assert_eq!((&out["decision"]["reason"], &out["id"]), (&json!("Bounds"), &Value::Null));
            assert_eq!(net.pool_state(), before, "nothing written");
            let out = net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert_eq!(out.fail().panic_msg.as_deref(), Some("NotFound"));
            assert!(net.tx(OWNER, POOL, "observe", json!({ "values": [5, 5] })).result.is_ok());

            // F-1, the cure-by-completion hedge.
            let mut net = Net::new(b, &cure_config(), X1, 100);
            assert!(net.tx(OWNER, POOL, "observe", json!({ "values": [12, 5] })).result.is_ok());
            assert_eq!(submit_tx(&mut net, A1, 0, 1, 150)["id"], json!(0));
            assert_eq!(net.pool_state().values, vec![12, 4]);
            net.tx(A1, POOL, "dispatch", json!({ "id": 0 }));
            assert_eq!(net.pool_state().records[0].status, Status::Active);
            let n = net.trace.len();
            assert!(net.tx(OPERATOR, X1, "finish", json!({ "id": 0, "completed": true })).result.is_ok());
            assert_eq!(methods(&net, n), ms(&[("finish", true), ("settle", true), ("on_settled", true)]));
            assert_eq!(net.result, Some(Ok(vec![])), "the lock accepted the settle");
            let p = net.pool_state();
            assert_eq!((p.records[0].status.clone(), p.values.clone()), (Status::Completed, vec![9, 4]));
            assert_eq!(net.jobs(), json!([{ "id": 0, "recipe": 0, "units": 1, "status": "completed" }]));
            // The operator cannot report the other outcome instead.
            let other = net.tx(OPERATOR, X1, "finish", json!({ "id": 0, "completed": false }));
            assert!(other.fail().panic_msg.as_deref().unwrap_or("").contains("terminal outcome conflict"));
            assert!(net.tx(OWNER, POOL, "observe", json!({ "values": [5, 5] })).result.is_ok());

            // F-2, the risk fixture, with the core as reference.
            const EXEC: &str = "executor.test.near";
            const CREDIT: &str = "credit.test.near";
            const MACRO: &str = "macro.test.near";
            const OWNER_R: &str = "owner.test.near";
            for reserved in [false, true] {
                let cfg = pool_core::fixtures::config("risk").expect("risk fixture");
                let mut net = Net::new(b, &cfg, EXEC, 100);
                let mut core = Pool::new(cfg, 100).unwrap();
                let q = |p: &Pool, recipe: u32, units: u32| Request { recipe, units, version: p.version, expires_at: 1_000 };
                let position = submit_tx(&mut net, CREDIT, 0, 1, 1_000)["id"].as_u64().unwrap() as u32;
                core.submit(CREDIT, q(&core, 0, 1), 100).unwrap();
                for units in [8, 7] {
                    let out = submit_tx(&mut net, MACRO, 3, units, 1_000);
                    assert_eq!((&out["decision"]["reason"], &out["id"]), (&json!("Bounds"), &Value::Null), "{units} units");
                }
                let hedge = submit_tx(&mut net, MACRO, 3, 6, 1_000)["id"].as_u64().unwrap() as u32;
                core.submit(MACRO, q(&core, 3, 6), 100).unwrap();
                assert_eq!(net.pool_state(), core);
                if !reserved {
                    // The position is dispatched; its execute fails, so it is
                    // Unknown until the operator's evidence.
                    net.tx(OPERATOR, EXEC, "set_mode", json!({ "mode": "panic" }));
                    net.tx(CREDIT, POOL, "dispatch", json!({ "id": position }));
                    core.dispatch(CREDIT, position, 100).unwrap();
                    core.acknowledge(position, false).unwrap();
                    net.tx(OPERATOR, EXEC, "set_mode", json!({ "mode": "normal" }));
                }
                net.tx(MACRO, POOL, "dispatch", json!({ "id": hedge }));
                core.dispatch(MACRO, hedge, 100).unwrap();
                core.acknowledge(hedge, true).unwrap();
                assert!(net.tx(OPERATOR, EXEC, "finish", json!({ "id": hedge, "completed": true })).result.is_ok());
                assert_eq!(net.result, Some(Ok(vec![])));
                core.settle(EXEC, hedge, true).unwrap();
                assert_eq!(net.pool_state(), core);
                assert_eq!(core.values[0], 20);
                // The honest release of the position.
                if reserved {
                    assert!(net.tx(CREDIT, POOL, "cancel", json!({ "id": position })).result.is_ok());
                    core.cancel(CREDIT, position, 100).unwrap();
                } else {
                    let n = net.trace.len();
                    assert!(net.tx(OPERATOR, EXEC, "confirm_not_executed", json!({ "id": position, "recipe": 0, "units": 1 })).result.is_ok());
                    assert_eq!(methods(&net, n), ms(&[("confirm_not_executed", true), ("settle", true), ("on_settled", true)]));
                    assert_eq!(net.result, Some(Ok(vec![])), "F-2: the honest settle(false) is Ok");
                    core.settle(EXEC, position, false).unwrap();
                }
                let p = net.pool_state();
                assert_eq!(p, core);
                assert_eq!(p.values, vec![0, 40, 30, 28]);
                assert_eq!(p.record(position).unwrap().status, Status::Cancelled);
                assert!(net.tx(OWNER_R, POOL, "observe", json!({ "values": [60, 40, 30, 40] })).result.is_ok());
            }
        }
    });
}
