//! Harness bodies, generic over [`Src`].
//!
//! Each body is the proof under Kani (`proofs.rs`) and a random test in plain
//! mode (`plain.rs`). Property IDs are tagged `PROP:` where they are checked.

use crate::gen::{self, ACK, CANCEL, DISPATCH, OBSERVE, PAUSE, SETTLE, SUBMIT};
use crate::spec::{self, *};
use crate::spec::VERSION_CAP;
use crate::src::Src;
use pool_core::*;

/// `_tx`: exact result, TX-0, TX-NOOP, TX-VER, AUTH-1, STALE-1, ADM-1/2,
/// DEP-1, PAUSE-1 (blocked ops), OBS-1, OBS-2, REFRESH-1, and LIVE-ALL (on
/// INV states).
pub const PART_TX: u8 = 1;
/// `_post`: exact post-state and frame, LIFE-1, ASYNC-1, TERM-1, HARD-STEP,
/// PAUSE-1 (unaffected ops).
pub const PART_POST: u8 = 2;
/// `_inv`: INV-* after the step (with the ghost update).
pub const PART_INV: u8 = 4;
/// `_cov`: the covers only. Each SATISFIED cover is one more SAT call, and
/// with the assertions in the same run every call carries the full
/// equivalence proof (RESULTS.md, cost ladder).
pub const PART_COV: u8 = 8;
/// `h_ref_*`: REF-1..3, one step of the core refines the TLA+ model
/// (`tla.rs`). Alone in the `h_ref_*` harnesses (the spec's outcome is not
/// computed there), so the other parts keep their cost; plain mode runs it
/// inside every step test (`PART_ALL`).
pub const PART_REF: u8 = 16;
pub const PART_ALL: u8 = PART_TX | PART_POST | PART_INV | PART_COV | PART_REF;

// ---------------------------------------------------------------------------
// Shared checks
// ---------------------------------------------------------------------------

/// Every state invariant (K-D6), asserted one by one so Kani reports each.
/// INV-HARD and INV-HEALTHY are corollaries (INV-RES and INV-PV give
/// `EV <= PV <= B`); they are asserted in plain mode only.
pub fn assert_inv(p: &Pool, g: &Ghost) {
    assert!(spec::valid(&p.config), "INV-WF: config valid"); // PROP: INV-WF CFG-2
    assert!(spec::win(p, g), "WIN-1: window and well-formedness"); // PROP: INV-WF WIN-1
    assert!(spec::inv_ledger(p, g), "INV-LEDGER"); // PROP: INV-LEDGER
    assert!(spec::inv_res(p), "INV-RES: release-aware envelope at the current violation"); // PROP: INV-RES
    assert!(spec::inv_pv(p, g), "INV-PV: no violation above the base's"); // PROP: INV-HARD (INV-PV)
    assert!(spec::inv_vercap(p), "INV-VERCAP: version + remaining increments <= u32::MAX"); // PROP: INV-VERCAP
    assert!(spec::inv_cu(p, g), "INV-CU: completed_units = min(lifetime, u32::MAX)"); // PROP: WIN-1 (INV-CU)
    assert!(spec::inv_dep(p, g), "INV-DEP"); // PROP: INV-DEP
    assert!(spec::inv_ver(p, g), "INV-VER"); // PROP: INV-VER
    #[cfg(not(kani))]
    {
        assert!(spec::inv_hard(p, g), "INV-HARD"); // PROP: INV-HARD (corollary)
        assert!(spec::inv_healthy(p, g), "INV-HEALTHY"); // PROP: INV-HEALTHY (corollary, every outcome combination)
    }
}

fn vals_of(p: &Pool) -> Vals {
    spec::vals(&p.config, &p.values)
}

/// The role that may make `op` effective (AUTH-1), or `None` (ack: the shell
/// makes it private; out-of-range ids).
fn authorized(pre: &Pool, op: &Op) -> bool {
    let cfg = &pre.config;
    let rec_recipe = |id: u32| spec::rec(pre, id).map(|c| spec::rcp_of(cfg, c.recipe));
    match op {
        Op::Submit { caller, req, .. } => match spec::rcp(cfg, req.recipe) {
            Some(r) => spec::eq_str(caller, &r.actor),
            None => false,
        },
        Op::Dispatch { caller, id, .. } => match rec_recipe(*id) {
            Some(r) => spec::eq_str(caller, &r.actor),
            None => false,
        },
        Op::Ack { .. } => true,
        Op::Cancel { caller, id, .. } => match rec_recipe(*id) {
            Some(r) => spec::eq_str(caller, &r.actor) || spec::eq_str(caller, &cfg.owner),
            None => false,
        },
        Op::Settle { caller, id, .. } => match rec_recipe(*id) {
            Some(r) => spec::eq_str(caller, &r.executor),
            None => false,
        },
        Op::Pause { caller, .. } | Op::Observe { caller, .. } => spec::eq_str(caller, &cfg.owner),
    }
}

/// `PART_TX` assertions for one step `pre --op--> post` with result `res`.
pub fn check_tx(pre: &Pool, post: &Pool, op: &Op, res: &Res, x: &Exp) {
    // PROP: TX-SUBMIT TX-DISPATCH TX-ACK TX-CANCEL TX-SETTLE TX-PAUSE TX-OBSERVE (result) NUM-2 (`add`'s Arithmetic exits, `Why::Add`)
    assert!(*res == x.res, "TX: exact result in guard order");
    let same = spec::same_state(post, pre);
    if res.is_err() {
        assert!(same, "TX-0: an error leaves the pool unchanged"); // PROP: TX-0
    }
    if x.noop() {
        assert!(same, "TX-NOOP: idempotent Ok changes nothing"); // PROP: TX-NOOP
    }
    if res.is_ok() && x.effective() {
        // PROP: TX-VER
        assert!(
            post.version as u64 == pre.version as u64 + 1,
            "TX-VER: effective Ok increments version by 1"
        );
    }
    let changed = !same;
    if changed {
        assert!(authorized(pre, op), "AUTH-1: effective call by the authorized role"); // PROP: AUTH-1
    }
    match op {
        Op::Submit { req, .. } => {
            if pre.paused {
                assert!(!changed, "PAUSE-1: paused blocks submit"); // PROP: PAUSE-1
            }
            if res.is_ok() {
                // PROP: STALE-1
                assert!(req.version == pre.version, "STALE-1: accepted request is at the current version");
                assert!(post.version > pre.version, "STALE-1: version strictly increases");
                let cfg = &pre.config;
                let r = spec::rcp_of(cfg, req.recipe);
                let e0 = spec::energy(cfg, &vals_of(pre));
                let v1 = vals_of(post);
                assert!(spec::energy(cfg, &v1) <= e0, "ADM-1: energy does not increase"); // PROP: ADM-1
                if spec::violating(cfg, &vals_of(pre)) {
                    // PROP: ADM-2
                    match spec::add(cfg, &v1, &r.complete, req.units) {
                        Some(done) => assert!(
                            spec::energy(cfg, &done) < e0,
                            "ADM-2: completion strictly decreases energy"
                        ),
                        None => panic!("ADM-2: completion leaves the value range"),
                    }
                }
                // PROP: DEP-1 (v1.1: the lifetime count, which a prune keeps)
                assert!(spec::prereqs_met(r, &pre.completed_units), "DEP-1: prerequisites met by completed_units");
            }
        }
        Op::Dispatch { .. } => {
            if pre.paused {
                assert!(!changed, "PAUSE-1: paused blocks dispatch"); // PROP: PAUSE-1
            }
        }
        Op::Observe { caller, vals, now } => {
            let any_open = spec::any_open(&pre.records);
            if res.is_ok() {
                // PROP: OBS-1: new values only with no open record.
                assert!(
                    !any_open || spec::eq_i64s(&post.values, &pre.values),
                    "OBS-1: observe changes the values only with no open record"
                );
                assert!(spec::same_config(&post.config, &pre.config), "OBS-1: config unchanged");
                if any_open {
                    // PROP: OBS-2 (frame of the refresh, F-6)
                    assert!(
                        spec::eq_i64s(&post.values, &pre.values)
                            && post.records == pre.records
                            && post.first_id == pre.first_id
                            && spec::eq_u32s(&post.completed_units, &pre.completed_units)
                            && post.paused == pre.paused
                            && post.observed_at == *now,
                        "OBS-2: a refresh with records open changes only observed_at and the version"
                    );
                }
            }
            // PROP: REFRESH-1: below the cap the owner can always restate the
            // current values (F-6 fixed).
            if spec::eq_str(caller, &pre.config.owner) && spec::eq_i64s(vals, &pre.values) && pre.version < VERSION_CAP {
                assert!(res.is_ok(), "REFRESH-1: the owner's observe of the current values is Ok below the cap");
            }
        }
        _ => {}
    }
}

/// LIVE-ALL (F-1, F-2, F-2b, F-5 fixed): from an INV state, with no premise
/// on the base or the version, every open record can be resolved. An
/// authorized cancel of a Reserved record is Ok, the executor's settle of a
/// Dispatched, Active or Unknown record is Ok either way, and the
/// acknowledgment of a Dispatched or Unknown record never fails Arithmetic.
/// In v1.0 these failed Bounds or Arithmetic (the `h_wedge*` covers and
/// "cancel: Bounds"/"settle: Arithmetic"); they are assertions now (K-D3).
/// Not for WF states (`wf_step`), where the release check may fail.
pub fn check_live(pre: &Pool, op: &Op, res: &Res) {
    match op {
        Op::Cancel { id, .. } => {
            if let Some(c) = spec::rec(pre, *id) {
                if c.status == Status::Reserved && authorized(pre, op) {
                    // PROP: LIVE-ALL
                    assert!(res.is_ok(), "LIVE-ALL: an authorized cancel of a Reserved record is Ok");
                }
            }
        }
        Op::Settle { id, .. } => {
            if let Some(c) = spec::rec(pre, *id) {
                if matches!(c.status, Status::Dispatched | Status::Active | Status::Unknown) && authorized(pre, op) {
                    // PROP: LIVE-ALL
                    assert!(res.is_ok(), "LIVE-ALL: the executor's settle of an in-flight record is Ok, either way");
                }
            }
        }
        Op::Ack { id, .. } => {
            if let Some(c) = spec::rec(pre, *id) {
                if matches!(c.status, Status::Dispatched | Status::Unknown) {
                    // PROP: LIVE-ALL INV-VERCAP
                    assert!(*res != Err(Error::Arithmetic), "LIVE-ALL: acknowledging an in-flight record never fails Arithmetic");
                }
            }
        }
        _ => {}
    }
}

/// `PART_POST` assertions.
pub fn check_post(pre: &Pool, post: &Pool, op: &Op, res: &Res, x: &Exp) {
    // PROP: TX-SUBMIT TX-DISPATCH TX-ACK TX-CANCEL TX-SETTLE TX-PAUSE TX-OBSERVE (post-state, frame)
    assert!(spec::post_matches(pre, post, x), "TX: exact post-state and frame");

    // PROP: LIFE-1 WIN-2
    let k = pre.records.len();
    // `s`: records pruned from the front (0 or 1). Only a submit at 256
    // records prunes, so on the Kani step domains (at most 3 records) it is
    // asserted to be 0 and the loops below are v1.0's; `h_full_256_prune`
    // checks the prune.
    #[cfg(kani)]
    let s = {
        assert!(post.first_id == pre.first_id, "WIN-2: no prune below 256 records");
        0usize
    };
    #[cfg(not(kani))]
    let s = {
        let s = post.first_id.wrapping_sub(pre.first_id) as usize;
        assert!(s <= 1, "WIN-2: at most one record pruned per step");
        if s == 1 {
            assert!(
                matches!(op, Op::Submit { .. }) && res.is_ok() && k == spec::MAXREC,
                "WIN-2: only an accepted submit at 256 records prunes"
            );
            assert!(spec::terminal(&pre.records[0].status), "WIN-2: a pruned record was terminal");
        }
        s
    };
    assert!(
        post.records.len() + s == k || post.records.len() + s == k + 1,
        "LIFE-1: records are append-only (a prune drops the front), at most one per step"
    );
    if post.records.len() + s == k + 1 {
        assert!(matches!(op, Op::Submit { .. }), "LIFE-1: only submit appends");
        let c = &post.records[k - s];
        assert!(
            c.id as u64 == pre.first_id as u64 + k as u64 && c.status == Status::Reserved,
            "LIFE-1: new record Reserved with id = first_id + len"
        );
    }
    for j in 0..k {
        if j >= s && j - s < post.records.len() {
            let (a, b) = (&pre.records[j], &post.records[j - s]);
            assert!(
                a.id == b.id && a.recipe == b.recipe && a.units == b.units && a.expires_at == b.expires_at,
                "LIFE-1: id, recipe, units, expires_at immutable"
            );
            assert!(spec::life_step(&a.status, &b.status), "LIFE-1: status graph");
            if spec::terminal(&a.status) {
                assert!(a.status == b.status, "LIFE-1: terminal is absorbing");
            }
            // PROP: ASYNC-1
            if matches!(a.status, Status::Active | Status::Unknown) && b.status == Status::Cancelled {
                assert!(
                    matches!(op, Op::Settle { completed: false, .. }) && authorized(pre, op),
                    "ASYNC-1: an acknowledged or unknown record is released only by the executor"
                );
            }
        }
    }
    match op {
        Op::Ack { .. } => {
            assert!(spec::eq_i64s(&post.values, &pre.values), "ASYNC-1: acknowledge never changes values"); // PROP: ASYNC-1
        }
        Op::Cancel { id, .. } => {
            if let Some(c) = spec::rec(pre, *id) {
                if authorized(pre, op) && matches!(c.status, Status::Dispatched | Status::Active | Status::Unknown) {
                    // PROP: ASYNC-1
                    assert!(*res == Err(Error::Pending), "ASYNC-1: in-flight records keep their reservation");
                }
                if authorized(pre, op) && c.status == Status::Completed {
                    // PROP: TERM-1 (v1.1: conflicting terminal evidence, as in settle)
                    assert!(
                        *res == Err(Error::InvalidState) && spec::same_state(post, pre),
                        "TERM-1: cancel of a Completed record is InvalidState and changes nothing"
                    );
                }
            }
        }
        Op::Settle { id, completed, .. } => {
            if let Some(c) = spec::rec(pre, *id) {
                if authorized(pre, op) && spec::terminal(&c.status) {
                    // PROP: TERM-1
                    let same = (*completed && c.status == Status::Completed)
                        || (!*completed && c.status == Status::Cancelled);
                    if same {
                        assert!(*res == Ok(None) && spec::same_state(post, pre), "TERM-1: same terminal is an Ok no-op");
                    } else {
                        assert!(*res == Err(Error::InvalidState), "TERM-1: opposite terminal is InvalidState");
                    }
                }
            }
        }
        _ => {}
    }
    // PROP: HARD-STEP (v1.1: an accepted submit leaves every metric's
    // violation as it was: its envelope holds both the current and the
    // reserved values)
    if res.is_ok() && !matches!(op, Op::Observe { .. }) {
        let cfg = &pre.config;
        let (a, b) = (vals_of(pre), vals_of(post));
        let submit = matches!(op, Op::Submit { .. });
        for i in 0..M {
            if i < spec::n(cfg) {
                let m = &cfg.metrics[i];
                assert!(spec::pv(m, b[i]) <= spec::pv(m, a[i]), "HARD-STEP: no metric's violation grows");
                if submit {
                    assert!(spec::pv(m, b[i]) == spec::pv(m, a[i]), "HARD-STEP: submit keeps every metric's violation");
                }
            }
        }
    }
    // PROP: PAUSE-1: settle, acknowledge, cancel and observe are unaffected
    // by `paused`. Under Kani: the spec's outcome from the pre-state with
    // `paused` flipped is the same (with the flag kept flipped), and `_tx` and
    // `_post` show that the code's result and post-state are the spec's from
    // every state of the domain, `paused` either way; so the code's outcome
    // does not depend on `paused` either. Calling the code a second time here
    // doubled the formula (cancel ran out of 32 GB; RESULTS.md, cost ladder).
    // Plain mode also calls the code again.
    if matches!(op, Op::Ack { .. } | Op::Cancel { .. } | Op::Settle { .. } | Op::Observe { .. }) {
        let mut q = gen::copy(pre);
        q.paused = !q.paused;
        let y = spec::expected(&q, op);
        assert!(
            y.res == x.res
                && y.why == x.why
                && spec::eq_vals(&y.values, &x.values)
                && y.version == x.version
                && y.observed_at == x.observed_at
                && y.paused == !x.paused
                && y.push == x.push
                && y.prune == x.prune
                && y.first_id == x.first_id
                && y.status == x.status
                && y.cu == x.cu,
            "PAUSE-1: the spec's outcome is independent of paused"
        );
        #[cfg(not(kani))]
        {
            let r2 = spec::apply(&mut q, op);
            q.paused = !q.paused;
            assert!(r2 == *res && spec::same_state(&q, post), "PAUSE-1: result and effect independent of paused");
        }
    }
}

// ---------------------------------------------------------------------------
// Covers (in the `_cov` part only; one family per step)
// ---------------------------------------------------------------------------

fn covers(pre: &Pool, op: &Op, res: &Res, x: &Exp, g: &Ghost) {
    use Error::*;
    let w = x.why;
    let healthy = spec::healthy_base(&pre.config, g);
    let cap = pre.version >= VERSION_CAP;
    match op {
        Op::Submit { req, .. } => {
            let needs = matches!(spec::rcp(&pre.config, req.recipe), Some(r) if !r.requires.is_empty());
            cover!(res.is_ok() && !spec::violating(&pre.config, &vals_of(pre)), "submit: Ok");
            cover!(res.is_ok() && spec::violating(&pre.config, &vals_of(pre)), "submit: Ok in a violating state (recovery)");
            cover!(res.is_ok() && pre.records.len() >= 2, "submit: Ok with 2 earlier records");
            cover!(res.is_ok() && needs, "submit: Prerequisite met by completed units");
            cover!(*res == Err(Paused), "submit: Paused");
            cover!(*res == Err(Stale), "submit: Stale");
            cover!(*res == Err(Expired), "submit: Expired");
            cover!(*res == Err(SnapshotExpired), "submit: SnapshotExpired");
            cover!(*res == Err(InvalidRecipe), "submit: InvalidRecipe");
            cover!(*res == Err(Unauthorized), "submit: Unauthorized");
            cover!(*res == Err(InvalidUnits), "submit: InvalidUnits");
            cover!(*res == Err(Prerequisite), "submit: Prerequisite");
            cover!(*res == Err(Arithmetic) && w == Why::Add, "submit: Arithmetic (reserve)");
            cover!(*res == Err(Arithmetic) && w == Why::Envelope, "submit: Arithmetic (envelope)");
            cover!(*res == Err(Arithmetic) && w == Why::Cap, "submit: Arithmetic (cap)");
            cover!(*res == Err(Bounds), "submit: Bounds");
            cover!(*res == Err(Energy) && w == Why::EnergyUp, "submit: Energy (increase)");
            cover!(*res == Err(Energy) && w == Why::Recovery, "submit: Energy (recovery rule)");
        }
        Op::Dispatch { .. } => {
            cover!(res.is_ok(), "dispatch: Ok");
            cover!(*res == Err(NotFound), "dispatch: NotFound");
            cover!(*res == Err(Unauthorized), "dispatch: Unauthorized");
            cover!(*res == Err(Paused), "dispatch: Paused");
            cover!(*res == Err(InvalidState), "dispatch: InvalidState");
            cover!(*res == Err(Expired), "dispatch: Expired");
            cover!(*res == Err(SnapshotExpired), "dispatch: SnapshotExpired");
            cover!(*res == Err(Arithmetic) && w == Why::Cap, "dispatch: Arithmetic (cap)");
        }
        Op::Ack { id, accepted } => {
            let st = spec::rec(pre, *id).map(|c| c.status.clone());
            cover!(res.is_ok() && w != Why::Noop && st == Some(Status::Dispatched) && *accepted, "ack: Dispatched -> Active");
            cover!(res.is_ok() && w != Why::Noop && st == Some(Status::Dispatched) && !*accepted, "ack: Dispatched -> Unknown");
            cover!(res.is_ok() && w != Why::Noop && st == Some(Status::Unknown), "ack: Unknown -> Active");
            cover!(res.is_ok() && w != Why::Noop && cap, "ack: Ok at or above the cap");
            cover!(x.noop() && st == Some(Status::Unknown), "ack: no-op Unknown -> Unknown");
            cover!(x.noop() && st == Some(Status::Active), "ack: no-op on Active (late ack)");
            cover!(x.noop() && st == Some(Status::Completed), "ack: no-op on a terminal record");
            cover!(*res == Err(NotFound), "ack: NotFound");
        }
        Op::Cancel { .. } => {
            cover!(res.is_ok() && w == Why::Release, "cancel: Ok (released)");
            cover!(res.is_ok() && w == Why::Release && !healthy, "cancel: Ok released after a disturbing observe");
            cover!(res.is_ok() && w == Why::Release && cap, "cancel: Ok released at or above the cap");
            cover!(x.noop(), "cancel: no-op on Cancelled");
            cover!(*res == Err(NotFound), "cancel: NotFound");
            cover!(*res == Err(Unauthorized), "cancel: Unauthorized");
            cover!(*res == Err(Pending), "cancel: Pending");
            cover!(*res == Err(InvalidState) && w == Why::CancelCompleted, "cancel: InvalidState (Completed)");
        }
        Op::Settle { completed, .. } => {
            cover!(res.is_ok() && *completed && w != Why::Noop, "settle: Ok completed");
            cover!(res.is_ok() && *completed && w != Why::Noop && !healthy, "settle: Ok completed after a disturbing observe");
            cover!(res.is_ok() && *completed && w != Why::Noop && cap, "settle: Ok completed at or above the cap");
            cover!(res.is_ok() && !*completed && w == Why::Release, "settle: Ok cancelled (released)");
            cover!(res.is_ok() && !*completed && w == Why::Release && !healthy, "settle: Ok cancelled after a disturbing observe");
            cover!(x.noop() && *completed, "settle: no-op Completed again");
            cover!(x.noop() && !*completed, "settle: no-op Cancelled again");
            cover!(*res == Err(NotFound), "settle: NotFound");
            cover!(*res == Err(Unauthorized), "settle: Unauthorized");
            cover!(*res == Err(InvalidState) && w == Why::Plain, "settle: InvalidState (Reserved)");
            cover!(*res == Err(InvalidState) && w == Why::Opposite, "settle: InvalidState (opposite terminal)");
        }
        Op::Pause { paused, .. } => {
            cover!(res.is_ok() && *paused, "pause: Ok pause");
            cover!(res.is_ok() && !*paused, "pause: Ok unpause");
            cover!(res.is_ok() && *paused == pre.paused, "pause: Ok without change still bumps");
            cover!(*res == Err(Error::Unauthorized), "pause: Unauthorized");
            cover!(*res == Err(Error::Arithmetic) && w == Why::Cap, "pause: Arithmetic (cap)");
        }
        Op::Observe { vals, .. } => {
            let viol = vals.len() == spec::n(&pre.config)
                && vals.iter().all(|v| spec::bounded(*v as spec::Z))
                && spec::violating(&pre.config, &spec::vals(&pre.config, vals));
            let open = spec::any_open(&pre.records);
            cover!(res.is_ok() && !viol, "observe: Ok");
            cover!(res.is_ok() && viol, "observe: Ok exposing a violation");
            cover!(res.is_ok() && !pre.records.is_empty() && !open, "observe: Ok after terminal records");
            cover!(res.is_ok() && open, "observe: Ok refresh with an open record");
            cover!(*res == Err(Unauthorized), "observe: Unauthorized");
            cover!(*res == Err(Pending), "observe: Pending (values differ, record open)");
            cover!(*res == Err(Bounds) && vals.len() != spec::n(&pre.config), "observe: Bounds (length)");
            cover!(*res == Err(Bounds) && vals.len() == spec::n(&pre.config), "observe: Bounds (value)");
            cover!(*res == Err(Arithmetic) && w == Why::Cap, "observe: Arithmetic (cap)");
        }
    }
}

// ---------------------------------------------------------------------------
// Step harness bodies
// ---------------------------------------------------------------------------

/// One operation of kind `kind` from an INV state.
pub fn step(src: &mut impl Src, kind: u32) {
    let c = src.cfg();
    let Some((mut p, g)) = gen::inv_state(src, &c) else {
        return;
    };
    let op = gen::op(src, &p, kind, c.val);
    let pre = gen::copy(&p);
    let x = spec::expected(&pre, &op);
    let res = spec::apply(&mut p, &op); // PROP: ROB-1 (Kani's panic, overflow and bounds checks, in every harness that calls the core)
    if c.part & PART_TX != 0 {
        check_tx(&pre, &p, &op, &res, &x);
        check_live(&pre, &op, &res);
    }
    if c.part & PART_COV != 0 {
        covers(&pre, &op, &res, &x, &g);
    }
    if c.part & PART_POST != 0 {
        check_post(&pre, &p, &op, &res, &x);
    }
    if c.part & PART_INV != 0 {
        let h = spec::ghost_step(&g, &pre, &op, &x);
        assert_inv(&p, &h);
    }
    // Plain mode only (the Kani step harnesses' code is unchanged; under
    // Kani REF-1..3 are `step_ref`'s, the `h_ref_*` harnesses).
    #[cfg(not(kani))]
    {
        if c.part & PART_REF != 0 {
            refines(&pre, &g, &op, &res, &p);
        }
    }
}

/// `h_ref_*`: the step's state and operation (the same generators, `Cfg`
/// and bounds as `step`), the real core applied, then REF-1..3 alone (no
/// spec outcome).
pub fn step_ref(src: &mut impl Src, kind: u32) {
    let c = src.cfg();
    let Some((mut p, g)) = gen::inv_state(src, &c) else {
        return;
    };
    let op = gen::op(src, &p, kind, c.val);
    let pre = gen::copy(&p);
    let res = spec::apply(&mut p, &op);
    refines(&pre, &g, &op, &res, &p);
}

/// The TLA+ ghost `(base, k0)` of a state with the Kani ghost `g`.
/// SharedPool.tla folds a counted pruned record's contribution into `base`
/// and moves `k0` past it (`Submit`); `spec::ghost_step` keeps that
/// contribution in `pruned` and leaves `k0` (plan 2.2, K-D5). So the TLA+
/// ghost is `base + pruned` and `max(k0, first_id)`: every id below
/// `first_id` is pruned, and a record pruned with `id >= k0` is in
/// `pruned`. Equal to `(base, k0)` on the INV domain (`first_id = 0`).
pub fn tla_ghost(p: &Pool, g: &Ghost) -> ([i64; M], usize, u64) {
    let nb = g.base.len();
    assert!(nb <= M, "spec: the ghost base has at most M values");
    let mut b = [0i64; M];
    for i in 0..M {
        if i < nb {
            b[i] = g.base[i] + g.pruned[i];
        }
    }
    let k0 = if g.k0 < p.first_id { p.first_id } else { g.k0 };
    (b, nb, k0 as u64)
}

/// REF-1..3 for the step `pre --op--> post` (`tla::assert_refines` with
/// the step bounds' image, `tla::TK`), and the `h_ref` covers.
pub fn refines(pre: &Pool, g: &Ghost, op: &Op, res: &Res, post: &Pool) {
    let (b, nb, k0) = tla_ghost(pre, g);
    let w = crate::tla::assert_refines::<2, 2, 2, 2, 4>(pre, &b[..nb], k0, op, res, post); // PROP: REF-1 REF-2 REF-3
    ref_covers(pre, op, w);
}

/// The `h_ref` cover family: every value of each TLA+ error operator that
/// the step domains reach ("Ok": the action is enabled, REF-2; "Noop" and
/// every error: a stutter, REF-3), and both ghost branches of `Observe`
/// (the refresh keeps `base` and `k0`). `Full` and the prune need 256
/// records (`h_full_*`, and the conformance traces in plain mode). From
/// v1.1.0 no INV state reaches `AckErr`'s Arithmetic or `CancelErr`'s and
/// `SettleErr`'s Arithmetic and Bounds (INV-VERCAP and INV-RES: LIVE-ALL,
/// the F-2 and F-5 fixes; asserted by the `_tx` parts and `h_live_all`), so
/// those values have no cover here; v1.0.0's did (its findings).
fn ref_covers(pre: &Pool, op: &Op, w: crate::tla::TErr) {
    use crate::tla::TErr as W;
    match op {
        Op::Submit { .. } => {
            cover!(w == W::Ok, "ref submit: Ok (Submit enabled)");
            cover!(w == W::Paused, "ref submit: Paused");
            cover!(w == W::Stale, "ref submit: Stale");
            cover!(w == W::Expired, "ref submit: Expired");
            cover!(w == W::SnapshotExpired, "ref submit: SnapshotExpired");
            cover!(w == W::InvalidRecipe, "ref submit: InvalidRecipe");
            cover!(w == W::Unauthorized, "ref submit: Unauthorized");
            cover!(w == W::InvalidUnits, "ref submit: InvalidUnits");
            cover!(w == W::Prerequisite, "ref submit: Prerequisite");
            cover!(w == W::Arithmetic, "ref submit: Arithmetic");
            cover!(w == W::Bounds, "ref submit: Bounds");
            cover!(w == W::Energy, "ref submit: Energy");
        }
        Op::Dispatch { .. } => {
            cover!(w == W::Ok, "ref dispatch: Ok (Dispatch enabled)");
            cover!(w == W::NotFound, "ref dispatch: NotFound");
            cover!(w == W::Unauthorized, "ref dispatch: Unauthorized");
            cover!(w == W::Paused, "ref dispatch: Paused");
            cover!(w == W::InvalidState, "ref dispatch: InvalidState");
            cover!(w == W::Expired, "ref dispatch: Expired");
            cover!(w == W::SnapshotExpired, "ref dispatch: SnapshotExpired");
            cover!(w == W::Arithmetic, "ref dispatch: Arithmetic");
        }
        Op::Ack { .. } => {
            cover!(w == W::Ok, "ref ack: Ok (Ack enabled)");
            cover!(w == W::Noop, "ref ack: Noop");
            cover!(w == W::NotFound, "ref ack: NotFound");
        }
        Op::Cancel { .. } => {
            cover!(w == W::Ok, "ref cancel: Ok (Cancel enabled)");
            cover!(w == W::Noop, "ref cancel: Noop");
            cover!(w == W::NotFound, "ref cancel: NotFound");
            cover!(w == W::Unauthorized, "ref cancel: Unauthorized");
            cover!(w == W::InvalidState, "ref cancel: InvalidState (Completed)");
            cover!(w == W::Pending, "ref cancel: Pending");
        }
        Op::Settle { .. } => {
            cover!(w == W::Ok, "ref settle: Ok (Settle enabled)");
            cover!(w == W::Noop, "ref settle: Noop");
            cover!(w == W::NotFound, "ref settle: NotFound");
            cover!(w == W::Unauthorized, "ref settle: Unauthorized");
            cover!(w == W::InvalidState, "ref settle: InvalidState");
        }
        Op::Pause { .. } => {
            cover!(w == W::Ok, "ref pause: Ok (SetPaused enabled)");
            cover!(w == W::Unauthorized, "ref pause: Unauthorized");
            cover!(w == W::Arithmetic, "ref pause: Arithmetic");
        }
        Op::Observe { vals, .. } => {
            let refresh = spec::eq_i64s(vals, &pre.values) && spec::any_open(&pre.records);
            cover!(w == W::Ok && !refresh, "ref observe: Ok (Observe enabled, re-base)");
            cover!(w == W::Ok && refresh, "ref observe: Ok (Observe enabled, refresh keeps base and k0)");
            cover!(w == W::Unauthorized, "ref observe: Unauthorized");
            cover!(w == W::Pending, "ref observe: Pending");
            cover!(w == W::Bounds, "ref observe: Bounds");
            cover!(w == W::Arithmetic, "ref observe: Arithmetic");
        }
    }
}

pub fn step_submit(src: &mut impl Src) {
    step(src, SUBMIT)
}
pub fn step_dispatch(src: &mut impl Src) {
    step(src, DISPATCH)
}
pub fn step_ack(src: &mut impl Src) {
    step(src, ACK)
}
pub fn step_cancel(src: &mut impl Src) {
    step(src, CANCEL)
}
pub fn step_settle(src: &mut impl Src) {
    step(src, SETTLE)
}
pub fn step_pause(src: &mut impl Src) {
    step(src, PAUSE)
}
pub fn step_observe(src: &mut impl Src) {
    step(src, OBSERVE)
}

/// `h_version_cap_<op>`: an operation of kind `kind` (any kind if `None`)
/// from an INV state at `version >= VERSION_CAP`. PROP: VER-CAP (submit,
/// dispatch, set_paused and observe fail Arithmetic and change nothing:
/// the 768 increments above the cap are kept for the open records),
/// INV-VERCAP and LIVE-ALL near the cap (ack, cancel and settle are exactly
/// the spec's, never Arithmetic, and resolve every open record), TX-0. Under
/// Kani one harness per kind (RESULTS.md, cost ladder).
pub fn version_cap(src: &mut impl Src, kind: Option<u32>) {
    let c = src.cfg();
    let Some((mut p, _g)) = gen::inv_state(src, &c) else {
        return;
    };
    assume!(p.version >= VERSION_CAP);
    let kind = match kind {
        Some(k) => k,
        None => src.below(gen::KINDS),
    };
    let op = gen::op(src, &p, kind, c.val);
    let pre = gen::copy(&p);
    let x = spec::expected(&pre, &op);
    let res = spec::apply(&mut p, &op);
    // Plain mode only (the Kani harnesses' code is unchanged). Without the
    // `h_ref` covers: at the cap most of them are unreachable (no submit is
    // enabled).
    #[cfg(not(kani))]
    {
        if c.part & PART_REF != 0 {
            let (b, nb, k0) = tla_ghost(&pre, &_g);
            crate::tla::assert_refines::<2, 2, 2, 2, 4>(&pre, &b[..nb], k0, &op, &res, &p); // PROP: REF-1 REF-2 REF-3
        }
    }
    assert!(res == x.res, "TX: exact result at or above the cap");
    if res.is_err() {
        assert!(spec::same_state(&p, &pre), "TX-0: an error leaves the pool unchanged"); // PROP: TX-0
    }
    let capped = kind == SUBMIT || kind == DISPATCH || kind == PAUSE || kind == OBSERVE;
    if capped {
        // PROP: VER-CAP
        assert!(res.is_err(), "VER-CAP: submit, dispatch, set_paused and observe fail at or above the cap");
        assert!(spec::same_state(&p, &pre), "VER-CAP: nothing changes at or above the cap");
    } else {
        // PROP: INV-VERCAP
        assert!(res != Err(Error::Arithmetic), "INV-VERCAP: ack, cancel and settle have their increment");
        check_live(&pre, &op, &res); // PROP: LIVE-ALL (near the cap)
        if x.effective() {
            assert!(p.version as u64 == pre.version as u64 + 1, "TX-VER: effective Ok increments version by 1");
        }
    }
    let ar = res == Err(Error::Arithmetic);
    let eff = res.is_ok() && x.effective();
    cover!(ar && kind == SUBMIT, "version cap: submit Arithmetic");
    cover!(ar && kind == DISPATCH, "version cap: dispatch Arithmetic");
    cover!(ar && kind == PAUSE, "version cap: pause Arithmetic");
    cover!(ar && kind == OBSERVE, "version cap: observe Arithmetic");
    cover!(eff && kind == ACK, "version cap: ack Ok");
    cover!(eff && kind == CANCEL, "version cap: cancel Ok (released)");
    cover!(eff && kind == SETTLE, "version cap: settle Ok");
}

/// `h_version_max_<op>`: an acknowledgment, cancel or settle (any of the
/// three if `None`) from a WF state at `version = u32::MAX` (K-D2: INV
/// states cannot have an open record there). PROP: TX-VER (no wrap: every
/// effective transition fails Arithmetic), TX-NOOP (idempotent paths still
/// succeed), TX-0.
pub fn version_max(src: &mut impl Src, kind: Option<u32>) {
    let c = src.cfg();
    let Some((mut p, _g)) = gen::wf_state(src, &c) else {
        return;
    };
    assume!(p.version == u32::MAX);
    let kind = match kind {
        Some(k) => k,
        None => [ACK, CANCEL, SETTLE][src.below(3) as usize],
    };
    let op = gen::op(src, &p, kind, c.val);
    let pre = gen::copy(&p);
    let x = spec::expected(&pre, &op);
    let res = spec::apply(&mut p, &op);
    assert!(res == x.res, "TX: exact result at version MAX");
    assert!(spec::same_state(&p, &pre), "TX-VER: at version MAX nothing changes (no wrap)"); // PROP: TX-VER TX-0
    if res.is_ok() {
        assert!(x.noop(), "TX-NOOP: only idempotent paths succeed at version MAX"); // PROP: TX-NOOP
    }
    let ar = res == Err(Error::Arithmetic) && x.why == Why::Version;
    cover!(ar && kind == ACK, "version max: ack Arithmetic");
    cover!(ar && kind == CANCEL, "version max: cancel Arithmetic");
    cover!(ar && kind == SETTLE, "version max: settle Arithmetic");
    cover!(res.is_ok() && kind == ACK, "version max: ack no-op Ok");
    cover!(res.is_ok() && kind == CANCEL, "version max: cancel no-op Ok");
    cover!(res.is_ok() && kind == SETTLE, "version max: settle no-op Ok");
}

/// `h_wf_<op>`: an operation of kind `kind` (any kind if `None`) from a WF
/// state (K-D2): `first_id`, `completed_units`, values and the version free.
/// TX exactness, TX-0, TX-NOOP, TX-VER and the exact post-state, on the
/// paths INV makes dead: `index` with `first_id > 0` and pruned ids, the id
/// overflow, a release that fails Bounds or Arithmetic, the saturating
/// count. In plain mode also REF-1..3 (`PART_REF`). No INV, no LIVE-ALL
/// (WF states need not be reachable).
pub fn wf_step(src: &mut impl Src, kind: Option<u32>) {
    let c = src.cfg();
    let Some((mut p, _g)) = gen::wf_state(src, &c) else {
        return;
    };
    let kind = match kind {
        Some(k) => k,
        None => src.below(gen::KINDS),
    };
    let op = gen::op(src, &p, kind, c.val);
    let pre = gen::copy(&p);
    let x = spec::expected(&pre, &op);
    let res = spec::apply(&mut p, &op);
    if c.part & PART_TX != 0 {
        check_tx(&pre, &p, &op, &res, &x);
    }
    // Plain mode only (the Kani `h_wf_*` code is unchanged; their REF is
    // not measured). REF-1..3 on WF states, with the placeholder ghost as
    // the TLA+ ghost: the only REF checks that reach the transcription's
    // id-overflow exit (`SubmitErr`'s last `Arithmetic`) and `DoneCap`
    // (`Settle`'s `Min(@ + units, DoneCap)`), which INV states and the
    // conformance traces do not.
    #[cfg(not(kani))]
    {
        if c.part & PART_REF != 0 {
            let (b, nb, k0) = tla_ghost(&pre, &_g);
            let w = crate::tla::assert_refines::<2, 2, 2, 2, 4>(&pre, &b[..nb], k0, &op, &res, &p); // PROP: REF-1 REF-2 REF-3
            let over = |r: u32, u: u32| pre.completed_units[r as usize] as u64 + u as u64 > u32::MAX as u64;
            cover!(
                w == crate::tla::TErr::Arithmetic && x.why == Why::IdOverflow,
                "ref wf submit: Arithmetic (id overflow, firstId + NR > MaxVersion)"
            );
            cover!(
                w == crate::tla::TErr::Ok
                    && matches!(&op, Op::Settle { id, completed: true, .. }
                        if matches!(spec::rec(&pre, *id), Some(r) if over(r.recipe, r.units))),
                "ref wf settle(true): doneU saturates (@ + units > DoneCap)"
            );
        }
    }
    if c.part & PART_POST != 0 {
        // PROP: TX-SUBMIT TX-DISPATCH TX-ACK TX-CANCEL TX-SETTLE (post-state on WF states)
        assert!(spec::post_matches(&pre, &p, &x), "TX: exact post-state and frame (WF)");
    }
    if c.part & PART_COV != 0 {
        let w = x.why;
        let id_of = |op: &Op| match op {
            Op::Dispatch { id, .. } | Op::Ack { id, .. } | Op::Cancel { id, .. } | Op::Settle { id, .. } => Some(*id),
            _ => None,
        };
        let pruned = matches!(id_of(&op), Some(i) if i < pre.first_id);
        let f = pre.first_id;
        cover!(res == Err(Error::NotFound) && pruned, "wf: NotFound (pruned id)");
        cover!(res.is_ok() && f > 0 && id_of(&op).is_some(), "wf: Ok on a record at first_id > 0");
        cover!(res == Err(Error::Arithmetic) && w == Why::IdOverflow, "wf submit: Arithmetic (id overflow)");
        cover!(
            res == Err(Error::Bounds) && matches!(op, Op::Cancel { .. }),
            "wf cancel: Bounds (release)"
        );
        cover!(
            res == Err(Error::Arithmetic) && matches!(op, Op::Cancel { .. }) && w != Why::Version,
            "wf cancel: Arithmetic (release)"
        );
        cover!(res == Err(Error::Bounds) && matches!(op, Op::Settle { .. }), "wf settle: Bounds");
        cover!(
            matches!(x.cu, Some((r, v)) if v == u32::MAX && (r as usize) < pre.completed_units.len() && pre.completed_units[r as usize] > u32::MAX - 100),
            "wf settle(true): completed units saturate"
        );
    }
}

// ---------------------------------------------------------------------------
// Configuration and numerics
// ---------------------------------------------------------------------------

/// Strings of 128 and 129 bytes in the raw configurations of `h_init` and
/// `h_validate`: plain mode only. Under Kani both run with the capacity
/// model (24-byte strings): copying and comparing a string of symbolic
/// length up to 129 bytes made `h_init` run out of time, and without the
/// model `h_validate` ran out of 40 GB (RESULTS.md, cost ladder). `validate`
/// reads only the length of a string (`short`: 1 to 128 bytes).
const INIT_LONG: bool = !cfg!(kani);

/// `h_init`: CFG-1. `Pool::new` succeeds iff `validate` does (and the spec's
/// validity), and the initial pool is as specified and satisfies INV. Under
/// Kani `validate` is a stub with a free result (`model::validate`;
/// `h_validate` shows that the real one is the spec's validity), and the
/// INV of the initial pool is `h_init_inv`'s: the raw configuration, the
/// real `validate`, the spec's validity and the invariants in one formula
/// ran out of 40 GB (RESULTS.md, cost ladder).
pub fn init(src: &mut impl Src) {
    let c = src.cfg();
    // Plain mode: half the samples a valid configuration with free initial
    // values (raw ones are rarely valid). Under Kani the raw configurations
    // include every valid one of the domain, and a second builder doubled
    // the heap objects every pointer may refer to.
    let cfg = if cfg!(kani) || src.bool() {
        gen::raw_config(src, &c, INIT_LONG)
    } else {
        gen::init_config(src, &c)
    };
    let now = src.u64();
    #[cfg(kani)]
    crate::model::VALIDATED.store(0, std::sync::atomic::Ordering::SeqCst);
    let r = Pool::new(gen::copy_config(&cfg), now);
    // PROP: CFG-1. `new` is Ok iff `validate` is; with CFG-2 (`h_validate`:
    // `validate` = spec), iff the configuration is valid.
    #[cfg(kani)]
    {
        let v = crate::model::VALIDATED.load(std::sync::atomic::Ordering::SeqCst);
        assert!(v != 0, "CFG-1: new calls validate");
        assert!(r.is_ok() == (v == 1), "CFG-1: new Ok iff validate Ok");
    }
    #[cfg(not(kani))]
    {
        assert!(r.is_ok() == spec::valid(&cfg), "CFG-1: new Ok iff the configuration is valid");
        assert!(r.is_ok() == cfg.validate().is_ok(), "CFG-1: new Ok iff validate Ok");
    }
    match &r {
        Err(e) => assert!(*e == Error::InvalidConfig, "CFG-1: new fails only InvalidConfig"),
        Ok(p) => {
            assert!(
                spec::same_config(&p.config, &cfg)
                    && p.version == 0
                    && p.observed_at == now
                    && !p.paused
                    && p.records.is_empty()
                    && p.first_id == 0,
                "CFG-1: initial pool"
            );
            let mut cu = p.completed_units.len() == cfg.recipes.len();
            for (r, x) in p.completed_units.iter().enumerate() {
                if r < cfg.recipes.len() && *x != 0 {
                    cu = false;
                }
            }
            assert!(cu, "CFG-1: completed_units zero, one per recipe");
            let mut ok = p.values.len() == cfg.metrics.len();
            for (i, m) in cfg.metrics.iter().enumerate() {
                if i < p.values.len() && p.values[i] != m.initial {
                    ok = false;
                }
            }
            assert!(ok, "CFG-1: initial values");
            #[cfg(not(kani))]
            if cfg.metrics.len() <= M {
                assert_inv(p, &Ghost::init(p)); // PROP: CFG-1 (init satisfies INV)
            }
        }
    }
    cover!(r.is_ok(), "init: Ok");
    cover!(r.is_err(), "init: InvalidConfig");
}

/// `h_init_inv`: CFG-1 on valid configurations (up to 2 metrics, 2 terms, 2
/// recipes, free initial values) with the real `validate`: `Pool::new` is
/// `Ok`, and the initial pool satisfies every INV with the initial ghost.
pub fn init_inv(src: &mut impl Src) {
    let c = src.cfg();
    let cfg = gen::init_config(src, &c);
    let now = src.u64();
    // Under Kani `new`'s `Ok` value is built directly (`pool_of`): `h_init`
    // shows that `new` is `Ok` iff `validate` is and builds exactly this
    // pool, and `h_validate` that `validate` is `valid` (asserted here). With
    // the core's `validate` and symbolic shapes in the formula, CBMC did not
    // reach the solver in 20 minutes (RESULTS.md, cost ladder).
    #[cfg(kani)]
    let r: Result<Pool, Error> = {
        assert!(spec::valid(&cfg), "CFG-1: the generated configuration is valid");
        let mut p = gen::pool_of(cfg);
        p.observed_at = now;
        Ok(p)
    };
    #[cfg(not(kani))]
    let r = Pool::new(cfg, now);
    // PROP: CFG-1
    assert!(r.is_ok(), "CFG-1: a valid configuration is accepted");
    if let Ok(p) = &r {
        assert_inv(p, &Ghost::init(p)); // PROP: CFG-1 (init satisfies INV)
    }
    cover!(r.is_ok(), "init inv: Ok");
}

/// `h_validate`: CFG-2. `validate` agrees with the spec on any configuration
/// within the bounds, and an accepted configuration has every reference in
/// range, vectors of length `n`, requirements strictly earlier and distinct,
/// and bounded values.
pub fn validate(src: &mut impl Src) {
    let c = src.cfg();
    // 128- and 129-byte strings: plain mode only (as `h_init`, deviation 23).
    let cfg = gen::raw_config(src, &c, INIT_LONG);
    let v = cfg.validate();
    let ok = v.is_ok();
    // PROP: CFG-2
    assert!(ok == spec::valid(&cfg), "CFG-2: validate = spec");
    if let Err(e) = v {
        assert!(e == Error::InvalidConfig, "CFG-2: only InvalidConfig");
    }
    if ok {
        let n = cfg.metrics.len();
        assert!(n >= 1 && !cfg.terms.is_empty() && !cfg.recipes.is_empty(), "CFG-2: nonempty");
        for r in &cfg.recipes {
            // PROP: CFG-2 (F-4 fixed)
            assert!(!spec::eq_str(&r.actor, &r.executor), "CFG-2: actor != executor");
        }
        for t in &cfg.terms {
            assert!((t.metric as usize) < n, "CFG-2: term metric in range");
            assert!(spec::bounded(t.target as spec::Z) && t.weight >= 1 && t.weight <= 1000, "CFG-2: term values");
        }
        for m in &cfg.metrics {
            assert!(
                spec::bounded(m.min as spec::Z) && spec::bounded(m.max as spec::Z) && m.min <= m.initial && m.initial <= m.max,
                "CFG-2: metric bounds"
            );
        }
        for (i, r) in cfg.recipes.iter().enumerate() {
            assert!(r.reserve.len() == n && r.complete.len() == n, "CFG-2: vectors of length n");
            for x in r.reserve.iter() {
                assert!(spec::bounded(*x as spec::Z), "CFG-2: deltas bounded");
            }
            for x in r.complete.iter() {
                assert!(spec::bounded(*x as spec::Z), "CFG-2: deltas bounded");
            }
            assert!(r.max_units >= 1 && r.max_units <= 100, "CFG-2: max_units");
            for (j, (p, u)) in r.requires.iter().enumerate() {
                assert!((*p as usize) < i && *u >= 1 && *u <= 100, "CFG-2: requires strictly earlier");
                for (q, _) in &r.requires[..j] {
                    assert!(q != p, "CFG-2: requires distinct");
                }
            }
        }
    }
    cover!(ok, "validate: Ok");
    // F-4 fixed: a recipe whose actor is its executor is refused (CFG-2:
    // exactly the spec's validity, which requires actor != executor).
    cover!(
        !ok && cfg.recipes.len() == 1 && spec::eq_str(&cfg.recipes[0].actor, &cfg.recipes[0].executor),
        "validate: InvalidConfig with actor = executor (F-4 fixed)"
    );
    // Q3: the owner may be a recipe's executor (a documented limit).
    cover!(
        ok && !cfg.recipes.is_empty() && spec::eq_str(&cfg.owner, &cfg.recipes[0].executor),
        "validate: Ok with owner = executor (Q3)"
    );
    cover!(ok && cfg.recipes.len() == 3 && cfg.recipes[2].requires.len() == 2, "validate: Ok with two requirements");
    cover!(!ok && cfg.name.is_empty(), "validate: empty name");
    #[cfg(not(kani))]
    cover!(!ok && cfg.name.len() == 129, "validate: name too long");
    cover!(!ok && cfg.metrics.is_empty(), "validate: no metrics");
    cover!(!ok && cfg.terms.is_empty() && !cfg.metrics.is_empty(), "validate: no terms");
    cover!(!ok && cfg.recipes.is_empty() && !cfg.metrics.is_empty() && !cfg.terms.is_empty(), "validate: no recipes");
    cover!(
        !ok && spec::short(&cfg.name)
            && spec::short(&cfg.owner)
            && cfg.metrics.len() == 1
            && cfg.metrics[0].min > cfg.metrics[0].max,
        "validate: min > max"
    );
    cover!(
        !ok && cfg.recipes.len() >= 2 && cfg.recipes[1].requires.len() == 2 && cfg.recipes[1].requires[0].0 == cfg.recipes[1].requires[1].0,
        "validate: duplicate requirement"
    );
    cover!(!ok && cfg.recipes.len() >= 1 && cfg.recipes[0].requires.len() == 1, "validate: requirement not earlier");
}

/// `h_energy`: NUM-1. For a valid configuration and bounded values of
/// length `n`, `energy_terms` and `energy` succeed, each term is exactly
/// `w·r²` (≤ 4e15) and `energy` is their exact sum; other vectors fail Bounds.
pub fn energy(src: &mut impl Src) {
    let c = src.cfg();
    let cfg = gen::config(src, &c);
    let n = cfg.metrics.len();
    // Under Kani, `Pool::new`'s result (CFG-1, `h_init`) is built directly:
    // validating a configuration that is valid by construction costs more
    // than the energy itself (RESULTS.md, cost ladder).
    #[cfg(kani)]
    let p = gen::pool_of(cfg);
    #[cfg(not(kani))]
    let p = Pool::new(cfg, 0).expect("generated config is valid");
    let len = if src.bool() { n } else { src.below(gen::MAX_M as u32 + 2) as usize };
    let mut v = Vec::with_capacity(gen::MAX_M as usize + 1);
    let mut in_range = true;
    for i in 0..gen::MAX_M as usize + 1 {
        if i < len {
            let x = if src.bool() {
                gen::val(src, c.val)
            } else {
                gen::raw_i64(src, c.val)
            };
            if !spec::bounded(x as spec::Z) {
                in_range = false;
            }
            v.push(x);
        }
    }
    let t = p.energy_terms(&v);
    let e = p.energy(&v);
    if len == n && in_range {
        if c.part & PART_TX != 0 {
            // PROP: NUM-1
            assert!(t.is_ok() && e.is_ok(), "NUM-1: energy cannot fail on bounded values");
        }
        let (Ok(t), Ok(e)) = (t, e) else { return };
        if c.part & PART_TX != 0 {
            assert!(t.len() == p.config.terms.len(), "NUM-1: one value per term");
        }
        if c.part & PART_POST != 0 {
            // PROP: NUM-1
            let x = spec::vals(&p.config, &v);
            let mut sum: u128 = 0;
            for (k, tm) in p.config.terms.iter().enumerate() {
                let want = spec::term(tm, x[tm.metric as usize]);
                assert!(want <= 4_000_000_000_000_000, "NUM-1: term <= 1000 * (2e6)^2");
                #[cfg(not(kani))]
                assert!(t[k] as spec::Z == want, "NUM-1: term exact");
                #[cfg(kani)]
                exact_term(t[k], tm, x[tm.metric as usize]);
                sum += t[k] as u128;
            }
            assert!(e as u128 == sum, "NUM-1: energy = exact sum of terms");
        }
        if c.part & PART_COV != 0 {
            cover!(e > 1_000_000_000_000_000 / 1000, "energy: large energy");
            cover!(e == 0, "energy: zero energy");
        }
    } else {
        if c.part & PART_TX != 0 {
            // PROP: NUM-1
            assert!(t == Err(Error::Bounds) && e == Err(Error::Bounds), "NUM-1: other vectors fail Bounds");
        }
        if c.part & PART_COV != 0 {
            cover!(len != n, "energy: wrong length");
            cover!(len == n && !in_range, "energy: value out of range");
        }
    }
}

/// NUM-1 exactness of one term under Kani, without multiplier equivalence
/// (an `i128` product against the code's `u64` product does not finish in 30
/// minutes over the full range, RESULTS.md): the code's term equals `w·r²`
/// modulo 2^64 (a wrapping product, the same circuit), and `w·r² ≤ 4e15 <
/// 2^64` because `r ≤ 2e6` and `w ≤ 1000` (asserted linearly), so equality
/// modulo 2^64 is equality.
#[cfg(kani)]
fn exact_term(got: u64, t: &Term, v: spec::Z) {
    let d = v - t.target as spec::Z;
    let r = match t.shape {
        Shape::Target => {
            if d < 0 {
                -d
            } else {
                d
            }
        }
        Shape::Excess => {
            if d > 0 {
                d
            } else {
                0
            }
        }
        Shape::Deficit => {
            if d < 0 {
                -d
            } else {
                0
            }
        }
    };
    assert!(r >= 0 && r <= 2_000_000 && t.weight <= 1000, "NUM-1: r <= 2e6, w <= 1000");
    let r = r as u64;
    let m = r.wrapping_mul(r).wrapping_mul(t.weight as u64);
    assert!(got == m, "NUM-1: term exact (mod 2^64, and below 2^64)");
}

/// `h_energy_max`: NUM-1 at the largest configuration: 64 terms of weight
/// 1000 at distance 2e6, energy = 64·1000·(2e6)² = 2.56e17, exact.
pub fn energy_max(_src: &mut impl Src) {
    let mut cfg = Config {
        name: "p".into(),
        owner: "o_".into(),
        metrics: vec![Metric {
            name: "m".into(),
            unit: "u".into(),
            initial: 0,
            min: -1_000_000,
            max: 1_000_000,
        }],
        terms: Vec::with_capacity(64),
        recipes: vec![Recipe {
            name: "r".into(),
            actor: "a1".into(),
            executor: "x1".into(),
            reserve: vec![0],
            complete: vec![0],
            max_units: 1,
            requires: vec![],
        }],
        max_age_seconds: 0,
    };
    for k in 0..64 {
        cfg.terms.push(Term {
            metric: 0,
            target: 1_000_000,
            weight: 1000,
            shape: if k % 2 == 0 { Shape::Target } else { Shape::Deficit },
        });
    }
    // Under Kani, `Pool::new`'s result is built directly (as in `energy`):
    // without the capacity model CBMC does not fold the vector lengths, and
    // `validate` unrolled every requirement loop 66 times inside a recipe
    // loop unrolled 66 times (30 min were not enough).
    #[cfg(kani)]
    let p = gen::pool_of(cfg);
    #[cfg(not(kani))]
    let p = Pool::new(cfg, 0).expect("valid");
    // PROP: NUM-1 (64-term worst case)
    let e = p.energy(&[-1_000_000]);
    assert!(e == Ok(256_000_000_000_000_000), "NUM-1: 64-term worst case exact");
    let t = p.energy_terms(&[-1_000_000]).expect("terms");
    assert!(t.len() == 64 && t[63] == 4_000_000_000_000_000, "NUM-1: worst term exact");
    cover!(e.is_ok(), "energy max: Ok");
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// `h_preview`: RD-PREVIEW. `allowed` iff the spec's admission accepts;
/// `reason`, `candidate`, `before`, `after` and the term strings as coded;
/// no mutation; allowed implies submit succeeds with the previewed
/// candidate (v1.1: the cap is in `candidate`, and the id overflow is dead
/// on INV states).
pub fn preview(src: &mut impl Src) {
    let c = src.cfg();
    let Some((p, _g)) = gen::inv_state(src, &c) else {
        return;
    };
    let Op::Submit { caller, req, now } = gen::op(src, &p, SUBMIT, c.val) else {
        return;
    };
    let pre = gen::copy(&p);
    let d = p.preview(caller, &req, now);
    let cfg = &p.config;
    let v = vals_of(&p);
    let raw = match spec::rcp(cfg, req.recipe) {
        Some(r) if req.units <= 100 => spec::add(cfg, &v, &r.reserve, req.units),
        _ => None,
    };
    // `reason` read once into a local array (each read of the heap string is
    // a case split over the string objects; RESULTS.md, cost ladder).
    let rs = spec::Text::<16>::of(if c.part & (PART_TX | PART_COV) != 0 { &d.reason } else { "" });
    if c.part & PART_TX != 0 {
        // PROP: RD-PREVIEW
        assert!(spec::same_state(&p, &pre), "RD-PREVIEW: no mutation");
        let want = spec::candidate(&p, caller, &req, now);
        assert!(d.allowed == want.is_ok(), "RD-PREVIEW: allowed iff candidate Ok");
        let reason = match &want {
            Ok(_) => "Allowed",
            Err((e, _)) => spec::reason(e),
        };
        assert!(rs.eq(reason), "RD-PREVIEW: reason");
        match (&d.candidate, &raw) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                let mut ok = a.len() == spec::n(cfg);
                for i in 0..M {
                    if i < spec::n(cfg) && i < a.len() && a[i] as spec::Z != b[i] {
                        ok = false;
                    }
                }
                assert!(ok, "RD-PREVIEW: candidate values");
            }
            _ => panic!("RD-PREVIEW: candidate presence"),
        }
        if d.allowed {
            let mut q = gen::copy(&p);
            let r = q.submit(caller, req.clone(), now);
            assert!(r.is_ok(), "RD-PREVIEW: allowed implies submit Ok");
            assert!(
                matches!(&d.candidate, Some(v) if spec::eq_i64s(&q.values, v)),
                "RD-PREVIEW: submit reserves the previewed candidate"
            );
        }
    }
    if c.part & PART_POST != 0 {
        // The strings are the decimal numerals of the spec's numbers
        // (`spec::is_dec`: parsed, not formatted; RESULTS.md, cost ladder).
        let e0 = spec::energy(cfg, &v) as u64;
        assert!(spec::is_dec(&d.before, e0), "RD-PREVIEW: before");
        assert!(
            match (&d.after, &raw) {
                (Some(a), Some(x)) => spec::is_dec(a, spec::energy(cfg, x) as u64),
                (None, None) => true,
                _ => false,
            },
            "RD-PREVIEW: after"
        );
        let nt = cfg.terms.len();
        let mut ok = d.terms_before.len() == nt && d.terms_after.len() == if raw.is_some() { nt } else { 0 };
        for (i, t) in cfg.terms.iter().enumerate() {
            if i < d.terms_before.len() && !spec::is_dec(&d.terms_before[i], spec::term(t, v[t.metric as usize]) as u64) {
                ok = false;
            }
            if let Some(x) = &raw {
                if i < d.terms_after.len() && !spec::is_dec(&d.terms_after[i], spec::term(t, x[t.metric as usize]) as u64) {
                    ok = false;
                }
            }
        }
        assert!(ok, "RD-PREVIEW: term strings");
    }
    if c.part & PART_COV != 0 {
        cover!(d.allowed, "preview: allowed");
        cover!(!d.allowed && d.candidate.is_some(), "preview: rejected with a candidate");
        cover!(!d.allowed && d.candidate.is_none(), "preview: rejected without a candidate");
        cover!(rs.eq("Energy"), "preview: Energy");
        cover!(rs.eq("Bounds"), "preview: Bounds");
        cover!(rs.eq("Arithmetic") && p.version >= VERSION_CAP, "preview: Arithmetic at the cap");
    }
}

/// `h_best`: RD-BEST. `best_quantity` returns the admissible quantity in
/// `1..=min(units, 100)` with least energy after reservation, ties to the
/// larger quantity, or `None`; and it is sound (the result is admitted).
///
/// Under Kani the core's `Pool::candidate` is stubbed by a free result
/// (`model::candidate`, deviation 24): the harness shows that
/// `best_quantity` is exactly this search over whatever `candidate` returns
/// (each quantity evaluated once, with the request's other fields), ranked
/// by whatever `energy` (also stubbed) returns on each `Ok` candidate's
/// values, for requests of up to 7 units; `h_preview`, `h_step_submit_tx`
/// and `h_step_submit_post` show that `candidate` is the spec's, and
/// `h_energy` that `energy` is. With the real
/// `candidate` (two admissions of the core and two of the spec, units ≤ 2)
/// the formula had 161.9M clauses and timed out (RESULTS.md, cost ladder).
/// Plain mode runs the real `candidate` with up to 3 units (`max_units` ≤ 3:
/// larger requests only add `InvalidUnits` candidates) and calls `submit`
/// on the result.
const BEST_UNITS: u32 = if cfg!(kani) { 7 } else { 3 };

/// The candidate values of `r` (`None` if it is not admissible): under Kani
/// the stub's recorded result for `r.units`, in plain mode the spec's.
#[cfg(kani)]
fn best_cand(p: &Pool, _caller: &str, r: &Request, _now: u64) -> Option<Vals> {
    use std::sync::atomic::Ordering::SeqCst;
    let k = &crate::model::CAND[r.units as usize];
    if k.calls.load(SeqCst) == 0 || !k.ok.load(SeqCst) {
        return None;
    }
    let n = spec::n(&p.config);
    let mut v = [0; M];
    for i in 0..M {
        if i < n {
            v[i] = k.vals[i].load(SeqCst);
        }
    }
    Some(v)
}

#[cfg(not(kani))]
fn best_cand(p: &Pool, caller: &str, r: &Request, now: u64) -> Option<Vals> {
    spec::candidate(p, caller, r, now).ok()
}

/// The energy of the candidate values `v` of quantity `u` (`None` if it
/// fails): in plain mode the spec's; under Kani the result the stub of
/// `Pool::energy` returned when the core called it on that candidate
/// (`model::energy`, deviation 24; `h_energy`: the real `energy` is `Ok` and
/// the spec's on every bounded vector, NUM-1). `best` asserts below that
/// the core called it exactly once on each `Ok` candidate, with its values.
#[cfg(kani)]
fn best_energy(_p: &Pool, u: u32, _v: &Vals) -> Option<i128> {
    use std::sync::atomic::Ordering::SeqCst;
    let k = &crate::model::ENERGY[u as usize];
    if k.ok.load(SeqCst) {
        Some(k.e.load(SeqCst) as i128)
    } else {
        None
    }
}

#[cfg(not(kani))]
fn best_energy(p: &Pool, _u: u32, v: &Vals) -> Option<i128> {
    Some(spec::energy(&p.config, v) as i128)
}

pub fn best(src: &mut impl Src) {
    let c = src.cfg();
    let Some((p, _g)) = gen::inv_state(src, &c) else {
        return;
    };
    let Op::Submit { caller, req, now } = gen::op(src, &p, SUBMIT, c.val) else {
        return;
    };
    // Cost ladder: the search runs min(units, 100) admissions.
    assume!(req.units <= BEST_UNITS);
    let got = p.best_quantity(caller, &req, now);
    let mut best: Option<(i128, u32)> = None;
    let mut tie = false;
    for u in 1..=BEST_UNITS {
        if u <= req.units {
            let r2 = Request {
                units: u,
                ..req.clone()
            };
            if let Some(e) = best_cand(&p, caller, &r2, now).and_then(|v| best_energy(&p, u, &v)) {
                match best {
                    None => best = Some((e, u)),
                    Some((be, _)) => {
                        if e <= be {
                            tie = e == be;
                            best = Some((e, u));
                        }
                    }
                }
            }
        }
    }
    let want = best.map(|(_, u)| Request {
        units: u,
        ..req.clone()
    });
    // PROP: RD-BEST
    assert!(got == want, "RD-BEST: least energy, ties to more units");
    // Under Kani: `candidate` was called exactly once for each quantity in
    // `1..=units` (units ≤ 7 < 100), with the caller (the same string), the
    // recipe, version and expiry of the request and the same `now`; `energy`
    // exactly once on each `Ok` candidate's values and on nothing else.
    #[cfg(kani)]
    {
        use std::sync::atomic::Ordering::SeqCst;
        assert!(
            crate::model::CAND_OUT.load(SeqCst) == 0,
            "RD-BEST: candidate only for quantities in 1..=units"
        );
        assert!(
            crate::model::ENERGY[0].calls.load(SeqCst) == 0,
            "RD-BEST: energy only after an admissible candidate"
        );
        for u in 1..crate::model::CAND_N {
            let k = &crate::model::CAND[u];
            let calls = k.calls.load(SeqCst);
            assert!(
                calls == (u as u32 <= req.units) as u8,
                "RD-BEST: candidate evaluated once for each quantity in 1..=units"
            );
            let ek = &crate::model::ENERGY[u];
            assert!(
                ek.calls.load(SeqCst) == (calls == 1 && k.ok.load(SeqCst)) as u8,
                "RD-BEST: energy evaluated once for each admissible quantity"
            );
            assert!(
                ek.calls.load(SeqCst) == 0 || ek.args.load(SeqCst),
                "RD-BEST: energy of the candidate's values"
            );
            if calls == 1 {
                assert!(
                    k.caller.load(SeqCst) == caller.as_ptr() as usize
                        && k.caller_len.load(SeqCst) == caller.len()
                        && k.recipe.load(SeqCst) == req.recipe
                        && k.version.load(SeqCst) == req.version
                        && k.expires_at.load(SeqCst) == req.expires_at
                        && k.now.load(SeqCst) == now,
                    "RD-BEST: candidate called with the request's other fields"
                );
            }
        }
    }
    // Soundness: `got` is a request whose candidate is Ok. Under Kani that
    // is the stub's; with `h_preview` (candidate Ok iff the spec's) and
    // `h_step_submit_tx` (submit is the spec's: Ok whenever the candidate
    // is) the core's submit accepts it. Plain mode calls submit.
    #[cfg(not(kani))]
    if let Some(r) = &got {
        let mut q = gen::copy(&p);
        assert!(q.submit(caller, r.clone(), now).is_ok(), "RD-BEST: sound (submit accepts it)");
    }
    cover!(got.is_none(), "best: None");
    cover!(matches!(&got, Some(r) if r.units == req.units), "best: all requested units");
    cover!(matches!(&got, Some(r) if r.units < req.units), "best: fewer units than requested");
    cover!(tie && got.is_some(), "best: a tie at the least energy, to the larger quantity");
}

// ---------------------------------------------------------------------------
// Liveness, wedge, traces, limits, canary
// ---------------------------------------------------------------------------

/// `h_live_all`: LIVE-ALL (F-1, F-2, F-2b and F-5 fixed). From any INV
/// state, with no premise on the base or the version (INV-VERCAP is part of
/// INV), every open record can be resolved by its roles: a Reserved record
/// by a cancel of its actor or the owner, a Dispatched, Active or Unknown
/// record by its executor's settle, either way. v1.0's LIVE-1 needed
/// `B = 0`, version < MAX and settle(completed).
pub fn live_all(src: &mut impl Src) {
    let c = src.cfg();
    let Some((mut p, g)) = gen::inv_state(src, &c) else {
        return;
    };
    assume!(!p.records.is_empty());
    let j = src.below(p.records.len() as u32);
    let id = p.first_id + j;
    let c0 = p.records[j as usize].clone();
    let st = c0.status.clone();
    assume!(spec::open(&st));
    let mut open = 0;
    let mut done_other = false;
    for r in &p.records {
        if spec::open(&r.status) {
            open += 1;
        } else if r.status == Status::Completed {
            done_other = true;
        }
    }
    let healthy = spec::healthy_base(&p.config, &g);
    let cap = p.version >= VERSION_CAP;
    let owner = p.config.owner.clone();
    let r = spec::rcp_of(&p.config, c0.recipe);
    let (actor, ex) = (r.actor.clone(), r.executor.clone());
    let by_owner = src.bool();
    let b = src.bool();
    let res = if st == Status::Reserved {
        p.cancel(if by_owner { &owner } else { &actor }, id, 0)
    } else {
        p.settle(&ex, id, b)
    };
    // PROP: LIVE-ALL
    assert!(res.is_ok(), "LIVE-ALL: every open record can be resolved by its roles");
    assert!(spec::terminal(&p.records[j as usize].status), "LIVE-ALL: the record is terminal");
    let reserved = st == Status::Reserved;
    cover!(reserved && by_owner, "live all: Reserved, cancelled by the owner");
    cover!(reserved && !by_owner, "live all: Reserved, cancelled by the actor");
    cover!(st == Status::Dispatched && b, "live all: Dispatched, completed");
    cover!(st == Status::Dispatched && !b, "live all: Dispatched, released");
    cover!(st == Status::Active && b, "live all: Active, completed");
    cover!(st == Status::Active && !b, "live all: Active, released");
    cover!(st == Status::Unknown && b, "live all: Unknown, completed");
    cover!(st == Status::Unknown && !b, "live all: Unknown, released");
    cover!(!healthy, "live all: a disturbed base (B > 0)");
    cover!(cap, "live all: at or above the cap");
    cover!(open >= 3, "live all: with two other open records");
    cover!(
        st == Status::Dispatched && !b && healthy && done_other,
        "live all: the F-2 shape (a Dispatched position released beside a Completed record, B = 0)"
    );
}

/// The wedge harnesses' initial pool: `Pool::new(cfg, 100)` for a valid
/// configuration. Under Kani the roles are fixed (owner `o_`, every actor
/// `a1`, every executor `x1`) and `new`'s `Ok` value is built directly
/// (CFG-1, `h_init`): with symbolic roles every `==` on an account is a case
/// split over the four accounts, and validating a configuration that is
/// valid by construction is a formula of its own; with both, `h_wedge` ran
/// out of 40 GB (RESULTS.md, cost ladder). Plain mode keeps both.
fn wedge_new(cfg: Config) -> Pool {
    #[cfg(kani)]
    {
        let mut cfg = cfg;
        cfg.owner = ACCTS[0].into();
        for r in cfg.recipes.iter_mut() {
            r.actor = ACCTS[1].into();
            r.executor = ACCTS[2].into();
        }
        let mut p = gen::pool_of(cfg);
        p.observed_at = 100;
        p
    }
    #[cfg(not(kani))]
    Pool::new(cfg, 100).expect("valid")
}

/// `h_wedge`: F-1 fixed, from `Pool::new`. v1.0's script (an observe,
/// then a recovery hedge admitted and dispatched) reached a state where
/// settle both ways failed Bounds and cancel and observe failed Pending.
/// Now, after the same script, one exit call (any of the four) is asserted:
/// settle(completed) Ok, settle(cancelled) Ok, the owner's observe of the
/// current values Ok (the refresh), the owner's cancel Pending (in flight).
/// A hedge the release term cannot cover is refused at admission instead
/// (cover "submit refused with Bounds after a disturbing observe").
pub fn wedge(src: &mut impl Src) {
    let c = src.cfg();
    let cfg = gen::init_config(src, &c);
    let mut p = wedge_new(cfg);
    let owner = p.config.owner.clone();
    let actor = p.config.recipes[0].actor.clone();
    let ex = p.config.recipes[0].executor.clone();
    let n = p.config.metrics.len();
    let mut obs = Vec::with_capacity(gen::MAX_M as usize);
    for _ in 0..n {
        obs.push(gen::val(src, c.val));
    }
    let o1 = p.observe(&owner, obs, 100);
    // B > 0: the observe put some metric outside its bounds.
    let mut disturbed = false;
    for i in 0..M {
        if i < n && i < p.values.len() && spec::pv(&p.config.metrics[i], p.values[i] as Z) > 0 {
            disturbed = true;
        }
    }
    let s = p.submit(
        &actor,
        Request {
            recipe: 0,
            units: 1,
            version: p.version,
            expires_at: 200,
        },
        100,
    );
    let d = p.dispatch(&actor, 0, 100);
    let reached = o1.is_ok() && s == Ok(0) && d.is_ok();
    cover!(reached && disturbed, "wedge: disturbed state, hedge admitted and dispatched");
    cover!(o1.is_ok() && disturbed && s == Err(Error::Bounds), "wedge: submit refused with Bounds after a disturbing observe");
    if reached {
        let k = src.below(4);
        let r = match k {
            0 => p.settle(&ex, 0, true),
            1 => p.settle(&ex, 0, false),
            2 => {
                let vv = gen::copy_vec(&p.values, M + 1);
                p.observe(&owner, vv, 100)
            }
            _ => p.cancel(&owner, 0, 100),
        };
        // PROP: LIVE-ALL REFRESH-1 ASYNC-1 (F-1 fixed)
        if k < 3 {
            assert!(r.is_ok(), "F-1 fixed: settle either way and the refresh are Ok after the hedge is dispatched");
        } else {
            assert!(r == Err(Error::Pending), "ASYNC-1: the owner's cancel of the in-flight hedge is Pending");
        }
        cover!(k == 1 && disturbed, "wedge: the dispatched hedge is released in a disturbed state");
    }
}

/// `h_wedge_healthy`: F-2 fixed. v1.0: a position (recipe 0) and a hedge
/// (recipe 1) are admitted and dispatched, the hedge completes relying on
/// the position's reservation, and then the executor's honest
/// settle(position, cancelled) failed Bounds while cancel and observe
/// failed Pending: the position could never become terminal. v1.1: from
/// the same state the owner's cancel is Pending (in flight), the owner's
/// observe of the current values is Ok (the refresh), and settle(position,
/// cancelled) is Ok. Under Kani from every healthy INV state below the cap
/// with a Dispatched record 0 and a Completed record 1 (the state the
/// script reaches); plain mode runs the script from `Pool::new` on random
/// configurations (the small instance of v1.0 is now refused at the
/// hedge's admission, `f2_small_instance`).
pub fn wedge_healthy(src: &mut impl Src) {
    let c = src.cfg();
    // Under Kani from a healthy INV state (deviation 19): with the script
    // from `Pool::new` every step's outcome is symbolic, so the record
    // vector's length is too from the first submit on, and the nine calls on
    // the core ran out of 40 GB, even on one concrete configuration.
    #[cfg(kani)]
    {
        let Some((mut p, g)) = gen::inv_state(src, &c) else {
            return;
        };
        assume!(spec::healthy_base(&p.config, &g));
        assume!(p.version < VERSION_CAP);
        assume!(p.records.len() == 2);
        assume!(p.records[0].status == Status::Dispatched && p.records[1].status == Status::Completed);
        wedge_healthy_tail(&mut p);
    }
    #[cfg(not(kani))]
    {
        let cfg = gen::init_config(src, &c);
        wedge_healthy_on(cfg);
    }
}

/// The calls of F-2 on a state in which record 0 (the position) is
/// Dispatched, below the cap: the owner's cancel (Pending), the owner's
/// observe of the current values (Ok), the executor's settle(cancelled)
/// (Ok, F-2 fixed). Plain mode also checks the other way out: settle
/// (completed) from the same state is Ok.
fn wedge_healthy_tail(p: &mut Pool) {
    let caller = |s: &str| spec::ACCTS.iter().copied().find(|a| *a == s).unwrap_or("z_");
    let owner = caller(&p.config.owner);
    let x0 = caller(&spec::rcp_of(&p.config, p.records[0].recipe).executor);
    #[cfg(not(kani))]
    {
        let mut q = gen::copy(p);
        assert!(q.settle(x0, 0, true).is_ok(), "LIVE-ALL: settle(completed) of the position is Ok");
    }
    let ca = p.cancel(owner, 0, 100);
    let vv = gen::copy_vec(&p.values, M + 1);
    let ob = p.observe(owner, vv, 100);
    let sf = p.settle(x0, 0, false);
    // PROP: ASYNC-1 REFRESH-1 LIVE-ALL (F-2 fixed)
    assert!(ca == Err(Error::Pending), "ASYNC-1: the owner's cancel of the in-flight position is Pending");
    assert!(ob.is_ok(), "REFRESH-1: the owner restates the current values with the position open");
    assert!(sf.is_ok(), "F-2 fixed: the executor's settle(cancelled) of the position is Ok");
    assert!(p.records[0].status == Status::Cancelled, "F-2 fixed: the position is released");
    cover!(true, "wedge healthy: the position releases (F-2 fixed)");
}

/// The small F-2 instance of v1.0 (plain mode, `f2_small_instance`): one
/// metric in `[0, 4]` from 2, target 4 (weight 1); a position (recipe 0)
/// reserving +2, and a hedge (recipe 1) reserving nothing and completing -3,
/// which is in bounds only with the position's reservation (4 - 3 = 1,
/// while 2 - 3 = -1 is not). v1.1 counts the position's release in the
/// hedge's envelope, so the hedge is refused (Bounds).
pub fn f2_instance() -> Config {
    let recipe = |name: &str, reserve: i64, complete: i64| pool_core::Recipe {
        name: name.into(),
        actor: ACCTS[1].into(),
        executor: ACCTS[2].into(),
        reserve: vec![reserve],
        complete: vec![complete],
        max_units: 1,
        requires: vec![],
    };
    Config {
        name: "p".into(),
        owner: ACCTS[0].into(),
        metrics: vec![pool_core::Metric {
            name: "m".into(),
            unit: "u".into(),
            initial: 2,
            min: 0,
            max: 4,
        }],
        terms: vec![Term {
            metric: 0,
            target: 4,
            weight: 1,
            shape: pool_core::Shape::Target,
        }],
        recipes: vec![recipe("c", 2, 0), recipe("h", 0, -3)],
        max_age_seconds: 0,
    }
}

/// The F-2 script from `Pool::new(cfg)`, each step against the spec; if
/// every step is accepted (the hedge admitted with the position's release
/// counted), the tail. Returns the results of the five script calls.
pub fn wedge_healthy_on(cfg: Config) -> Vec<Res> {
    let mut out = Vec::new();
    if cfg.recipes.len() < 2 {
        return out;
    }
    let mut p = wedge_new(cfg);
    let mut g = Ghost::init(&p);
    let a0 = p.config.recipes[0].actor.clone();
    let (a1, x1) = (p.config.recipes[1].actor.clone(), p.config.recipes[1].executor.clone());
    let caller = |s: &str| spec::ACCTS.iter().copied().find(|a| *a == s).unwrap_or("z_");
    let req = |recipe: u32, v: u32| Request {
        recipe,
        units: 1,
        version: v,
        expires_at: 200,
    };
    let script = [
        Op::Submit { caller: caller(&a0), req: req(0, 0), now: 100 },
        Op::Submit { caller: caller(&a1), req: req(1, 1), now: 100 },
        Op::Dispatch { caller: caller(&a0), id: 0, now: 100 },
        Op::Dispatch { caller: caller(&a1), id: 1, now: 100 },
        Op::Settle { caller: caller(&x1), id: 1, completed: true },
    ];
    let mut reached = true;
    for op in &script {
        let pre = gen::copy(&p);
        let x = spec::expected(&pre, op);
        let res = spec::apply(&mut p, op);
        assert!(res == x.res, "TX: exact result along the F-2 script");
        g = spec::ghost_step(&g, &pre, op, &x);
        assert_inv(&p, &g);
        if res.is_err() {
            reached = false;
        }
        out.push(res);
        if !reached {
            return out;
        }
    }
    // A healthy INV state: no observe, so B = 0.
    assert!(spec::healthy_base(&p.config, &g), "F-2: B = 0 (no observe)");
    cover!(true, "wedge healthy: position and hedge dispatched, hedge completed");
    wedge_healthy_tail(&mut p);
    out
}

/// `h_bmc_from_init` (plain mode): up to `depth` operations of any kind
/// from `Pool::new`. INV (with INV-VER) after every step, and STALE-1 over
/// the trace: accepted requests carry strictly increasing versions, so no
/// accepted request can be accepted again. Under Kani the harnesses run
/// [`bmc_two`] (deviation 20).
pub fn bmc_from_init(src: &mut impl Src, depth: usize) {
    let c = src.cfg();
    let cfg = gen::init_config(src, &c);
    let now = src.u64();
    let mut p = Pool::new(cfg, now).expect("valid");
    let mut g = Ghost::init(&p);
    assert_inv(&p, &g);
    let mut accepted: Vec<Request> = Vec::with_capacity(depth);
    let mut completed = false;
    let mut replay_rejected = false;
    let mut observed_after = false;
    for _ in 0..depth {
        let k = src.below(gen::KINDS);
        let op = if src.bool() && !accepted.is_empty() && k == SUBMIT {
            replay(src, &p, &accepted[0])
        } else {
            gen::op(src, &p, k, c.val)
        };
        let x = spec::expected(&p, &op);
        let h = spec::ghost_step(&g, &p, &op, &x);
        let pre = if c.part & PART_REF != 0 { Some(gen::copy(&p)) } else { None };
        let res = spec::apply(&mut p, &op);
        if let Some(pre) = &pre {
            // PROP: REF-1 REF-2 REF-3 (plain mode: this body is plain-only;
            // up to `depth` records, so an image of 8)
            let (b, nb, k0) = tla_ghost(pre, &g);
            crate::tla::assert_refines::<2, 2, 2, 2, 8>(pre, &b[..nb], k0, &op, &res, &p);
        }
        g = h;
        assert!(res == x.res, "TX: exact result along the trace");
        assert_inv(&p, &g);
        if let Op::Submit { req, .. } = &op {
            let replay = accepted.iter().any(|a| a == req);
            if res.is_ok() {
                // PROP: STALE-1
                assert!(!replay, "STALE-1: an accepted request never replays");
                for a in &accepted {
                    assert!(req.version > a.version, "STALE-1: accepted versions strictly increase");
                }
                accepted.push(req.clone());
            } else if replay {
                replay_rejected = true;
            }
        }
        if res.is_ok() {
            if let Op::Settle { completed: true, .. } = op {
                completed = true;
            }
            if let Op::Observe { .. } = op {
                if completed {
                    observed_after = true;
                }
            }
        }
    }
    cover!(replay_rejected, "bmc: a replayed request is rejected");
    cover!(accepted.len() >= 2, "bmc: two accepted requests");
    cover!(completed, "bmc: a record completes");
    cover!(observed_after, "bmc: observe after a completion");
}

/// A replay of the accepted request `req` by its recipe's actor, at any
/// time.
fn replay(src: &mut impl Src, p: &Pool, req: &Request) -> Op {
    let actor = &spec::rcp_of(&p.config, req.recipe).actor;
    let caller = spec::ACCTS.iter().copied().find(|a| spec::eq_str(a, actor)).unwrap_or("z_");
    Op::Submit {
        caller,
        req: req.clone(),
        now: src.u64(),
    }
}

/// `h_bmc_from_init_*` (deviation 20): two operations from `Pool::new`, the
/// first a submit (`first == SUBMIT`) or a pause or observe (from
/// `Pool::new` there is no record, so every other kind fails `NotFound` or
/// earlier and changes nothing), the second of kind `last` (any if `None`).
/// After the first operation the harness asserts the exact result and the
/// exact post-state (the spec's), then continues from the same state
/// rebuilt with constant shapes: the core's post-state has a record count
/// and value vectors whose lengths depend on the outcome, and every later
/// read of them is a case split (RESULTS.md, cost ladder). After a submit
/// it continues from its `Ok` outcome only: an `Err` leaves the initial
/// pool (asserted), from which the pause/observe harness continues too (a
/// pause by a non-owner fails). INV (with INV-VER from `Init`) after both
/// operations, and STALE-1 across them.
pub fn bmc_two(src: &mut impl Src, first: u32, last: Option<u32>) {
    let c = src.cfg();
    let cfg = gen::init_config(src, &c);
    let now = src.u64();
    // Under Kani `new`'s `Ok` value is built directly (CFG-1: `h_init`,
    // `h_validate`, `h_init_inv`).
    #[cfg(kani)]
    let mut p = {
        let mut p = gen::pool_of(cfg);
        p.observed_at = now;
        p
    };
    #[cfg(not(kani))]
    let mut p = Pool::new(cfg, now).expect("valid");
    let g0 = Ghost::init(&p);
    let k0 = if first == SUBMIT {
        SUBMIT
    } else if src.bool() {
        PAUSE
    } else {
        OBSERVE
    };
    let op0 = gen::op(src, &p, k0, c.val);
    let x0 = spec::expected(&p, &op0);
    let g1 = spec::ghost_step(&g0, &p, &op0, &x0);
    let res0 = spec::apply(&mut p, &op0);
    assert!(res0 == x0.res, "TX: exact result along the trace");
    let n = spec::n(&p.config);
    let mut same = p.values.len() == n
        && p.version == x0.version
        && p.observed_at == x0.observed_at
        && p.paused == x0.paused
        && p.records.len() == x0.push.is_some() as usize;
    for i in 0..M {
        if i < n && i < p.values.len() && p.values[i] as Z != x0.values[i] {
            same = false;
        }
    }
    if let Some(cm) = &x0.push {
        if spec::rec(&p, 0) != Some(cm) {
            same = false;
        }
    }
    let nr = p.config.recipes.len();
    let mut completed_units = Vec::with_capacity(gen::MAX_R as usize + 1);
    for r in 0..gen::MAX_R as usize + 1 {
        if r < nr {
            completed_units.push(0);
        }
    }
    same = same && x0.cu.is_none() && p.first_id == 0 && spec::eq_u32s(&p.completed_units, &completed_units);
    assert!(same, "TX: exact post-state along the trace");
    let req0 = match &op0 {
        Op::Submit { req, .. } => Some(req.clone()),
        _ => None,
    };
    if first == SUBMIT {
        assume!(res0.is_ok());
    }
    let mut values = Vec::with_capacity(M);
    for i in 0..M {
        if i < n {
            values.push(x0.values[i] as i64);
        }
    }
    let mut records = Vec::with_capacity(2);
    if let (true, Some(r)) = (first == SUBMIT, &req0) {
        records.push(Commitment {
            id: 0,
            recipe: r.recipe,
            units: r.units,
            expires_at: r.expires_at,
            status: Status::Reserved,
        });
    }
    let Pool { config, .. } = p;
    let mut p = Pool {
        config,
        values,
        version: x0.version,
        observed_at: x0.observed_at,
        paused: x0.paused,
        records,
        first_id: 0,
        completed_units,
    };
    assert_inv(&p, &g1);
    let k1 = match last {
        Some(k) => k,
        None => src.below(gen::KINDS),
    };
    let op1 = match &req0 {
        Some(r) if k1 == SUBMIT && src.bool() => replay(src, &p, r),
        _ => gen::op(src, &p, k1, c.val),
    };
    let x1 = spec::expected(&p, &op1);
    let g2 = spec::ghost_step(&g1, &p, &op1, &x1);
    let res1 = spec::apply(&mut p, &op1);
    assert!(res1 == x1.res, "TX: exact result along the trace");
    assert_inv(&p, &g2);
    let mut replay_rejected = false;
    let mut two = false;
    if let (Some(r0), Op::Submit { req, .. }) = (&req0, &op1) {
        let replay = req == r0;
        if res1.is_ok() {
            // PROP: STALE-1
            assert!(!replay, "STALE-1: an accepted request never replays");
            assert!(req.version > r0.version, "STALE-1: accepted versions strictly increase");
            two = true;
        } else if replay {
            replay_rejected = true;
        }
    }
    cover!(replay_rejected, "bmc: a replayed request is rejected");
    cover!(two, "bmc: two accepted requests");
}

/// `h_slot_cycle_0`, `h_slot_cycle_2` (the F-3 cycle): one submit/cancel
/// cycle by a recipe's actor from a quiet state (no open record, every
/// metric within `[min, max]`): `Pool::new` (`_0`), or an INV state with 2
/// records (`_2`). If the submit is accepted and the version is at least 2
/// below the cap, the actor's cancel of the new record is accepted, the
/// cycle's only lasting effects are one more record, Cancelled, and the
/// version + 2 (values, `paused`, `observed_at`, `first_id`,
/// `completed_units` and the older records as before), and the same
/// request at the new version is accepted again with the same reserved
/// values and the next id. In v1.0 an actor could repeat the cycle until
/// submit failed `Full` at 256 records, for good (F-3). In v1.1 the cycle
/// consumes no lifetime capacity: at 256 records with a terminal front the
/// submit prunes the front (`h_full_256_prune`: Ok, WIN-2), and `Full`
/// needs an open front (`h_full_256`); plain `f3_slot_exhaustion` runs 300
/// cycles. As in `bmc_two`, each call after the first runs on the previous
/// call's post-state rebuilt with constant shapes, after asserting that the
/// core's post-state is that state (deviation 20).
pub fn slot_cycle(src: &mut impl Src, from_init: bool) {
    let c = src.cfg();
    let p = if from_init {
        let cfg = gen::init_config(src, &c);
        let now = src.u64();
        // Under Kani `new`'s `Ok` value is built directly (CFG-1).
        #[cfg(kani)]
        let p = {
            let mut p = gen::pool_of(cfg);
            p.observed_at = now;
            p
        };
        #[cfg(not(kani))]
        let p = Pool::new(cfg, now).expect("valid");
        p
    } else {
        let Some((p, _g)) = gen::inv_state(src, &c) else {
            return;
        };
        p
    };
    let k = p.records.len();
    let mut quiet = !spec::violating(&p.config, &vals_of(&p));
    for r in &p.records {
        if spec::open(&r.status) {
            quiet = false;
        }
    }
    assume!(quiet);
    // Three effective calls, the last a submit: below the cap (F-5).
    assume!(p.version < VERSION_CAP - 2);
    let op = gen::op(src, &p, SUBMIT, c.val);
    let Op::Submit { caller, req, now } = &op else {
        return;
    };
    let (caller, now) = (*caller, *now);
    let pre = gen::copy(&p);
    let x = spec::expected(&pre, &op);
    let mut p = p;
    let res = spec::apply(&mut p, &op);
    // PROP: TX-SUBMIT (the cycle's first call: exact result and post-state)
    assert!(res == x.res, "F-3: submit is the spec's");
    let Some(cm) = x.push.clone() else {
        // Not accepted: no cycle.
        return;
    };
    assume!(res.is_ok());
    let n = spec::n(&pre.config);
    let mut same = p.values.len() == n
        && p.version == pre.version + 1
        && p.records.len() == k + 1
        && p.records.get(k) == Some(&cm)
        && p.first_id == pre.first_id
        && spec::eq_u32s(&p.completed_units, &pre.completed_units);
    for i in 0..M {
        if i < n && i < p.values.len() && p.values[i] as Z != x.values[i] {
            same = false;
        }
    }
    assert!(same, "F-3: submit's post-state is the spec's");
    // The post-state with constant shapes: the old records, the new one.
    let rebuild = |cfg: Config, vals: &Vals, version: u32, last: Status| {
        let mut values = Vec::with_capacity(M);
        for i in 0..M {
            if i < n {
                values.push(vals[i] as i64);
            }
        }
        // `k` is a constant under Kani (the generators' fixed counts).
        let mut records = Vec::with_capacity(k + 1);
        for j in 0..k {
            records.push(pre.records[j].clone());
        }
        records.push(Commitment {
            status: last,
            ..cm.clone()
        });
        Pool {
            config: cfg,
            values,
            version,
            observed_at: pre.observed_at,
            paused: pre.paused,
            records,
            first_id: pre.first_id,
            completed_units: gen::copy_vec(&pre.completed_units, gen::MAX_R as usize + 1),
        }
    };
    let Pool { config, .. } = p;
    let mut q = rebuild(config, &x.values, x.version, Status::Reserved);
    // The cycle's second call: the same caller (the actor) cancels it.
    let ca = q.cancel(caller, pre.first_id + k as u32, now);
    // PROP: F-3 (the cycle completes from a quiet state)
    assert!(ca == Ok(()), "F-3: the actor's cancel of its fresh record is accepted");
    let v0 = vals_of(&pre);
    let mut back = q.values.len() == n
        && q.version as u64 == pre.version as u64 + 2
        && q.paused == pre.paused
        && q.observed_at == pre.observed_at
        && q.records.len() == k + 1
        && q.records.get(k).map(|r| r.status == Status::Cancelled) == Some(true);
    for i in 0..M {
        if i < n && i < q.values.len() && q.values[i] as Z != v0[i] {
            back = false;
        }
    }
    for j in 0..k {
        if q.records.get(j) != Some(&pre.records[j]) {
            back = false;
        }
    }
    assert!(back, "F-3: the cycle leaves one more Cancelled record, version + 2, nothing else");
    // The cycle's third call: the same request at the new version.
    let Pool { config, .. } = q;
    let mut q = rebuild(config, &v0, pre.version + 2, Status::Cancelled);
    let again = q.submit(
        caller,
        Request {
            version: q.version,
            ..req.clone()
        },
        now,
    );
    let mut repeat = again == Ok(pre.first_id + k as u32 + 1) && q.values.len() == n;
    for i in 0..M {
        if i < n && i < q.values.len() && q.values[i] as Z != x.values[i] {
            repeat = false;
        }
    }
    assert!(repeat, "F-3: the same request is accepted again, with the same values");
    cover!(true, "slot cycle: submit, cancel and submit again by the actor (F-3)");
}

/// A Cancelled record of recipe 0 with id `j`: the records of `full`.
const fn old_record(j: usize) -> Commitment {
    Commitment {
        id: j as u32,
        recipe: 0,
        units: 1,
        expires_at: 0,
        status: Status::Cancelled,
    }
}

/// `K` Cancelled records of recipe 0 with ids `first..first + K`, evaluated
/// by the compiler: one constant array.
const fn old_records<const K: usize>(first: usize) -> [Commitment; K] {
    const C: Commitment = old_record(0);
    let mut a = [C; K];
    let mut j = 0;
    while j < K {
        a[j].id = (first + j) as u32;
        j += 1;
    }
    a
}

/// The 257 records `full` starts from with `first_id = 0` (then truncated to
/// 255 or 256, so the buffer keeps room for a 257th record: the core's push
/// never reallocates, on mutant 21 either).
const OLD_RECORDS: [Commitment; 257] = old_records::<257>(0);

/// The records of `h_full_256_prune`: ids `1..=257` (`first_id = 1`: id 0
/// was pruned already), truncated to 256.
const PRUNE_FIRST: u32 = 1;
const PRUNE_RECORDS: [Commitment; 257] = old_records::<257>(PRUNE_FIRST as usize);

/// Which record-limit harness `full` is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FullAt {
    /// `h_full_255`: 255 records, the front's status free (open or terminal).
    Below,
    /// `h_full_256`: 256 records, an open front: Full.
    OpenFront,
    /// `h_full_256_prune`: 256 records with a terminal front, `first_id = 1`
    /// and `completed_units[0]` free (completions of pruned records).
    Prune,
}

/// `h_full_255`, `h_full_256`, `h_full_256_prune`: the window (WIN-1,
/// WIN-2, WIN-ESC). Submit is exactly as specified at the record limit:
/// below 256 records never Full, whatever the front; at 256 Full iff the
/// front record is open; at 256 with a terminal front admitted when the
/// guards pass, returning `first_id + 256`, the front pruned (the other 255
/// records shifted, unchanged), `first_id + 1`, the pruned id NotFound and
/// `completed_units` kept, so a prerequisite met by a pruned completion
/// still counts (mutant 44). Under Kani the count is concrete: with a
/// symbolic count every push was guarded, the length symbolic from the
/// first record on, and 63 of the 256 pushes took 9 minutes of symbolic
/// execution (RESULTS.md, cost ladder). The records are one constant array
/// moved into the vector (`Vec::from`, then `truncate`), not 255 pushes:
/// with the field-sensitivity option of `run.sh` (each byte of the buffer
/// its own symbol, so that the scans read constants) every push rewrote all
/// 6,168 bytes of the buffer (RESULTS.md, deviation 21, cost ladder 28j).
pub fn full(src: &mut impl Src, at: Option<FullAt>) {
    let c = src.cfg();
    let at = match at {
        Some(a) => a,
        None => [FullAt::Below, FullAt::OpenFront, FullAt::Prune][src.below(3) as usize],
    };
    let cfg = gen::config(src, &c);
    let nm = cfg.metrics.len();
    let nr = cfg.recipes.len();
    let (count, first) = match at {
        FullAt::Below => (255usize, 0u32),
        FullAt::OpenFront => (256, 0),
        FullAt::Prune => (256, PRUNE_FIRST),
    };
    let old = |j: usize| {
        let mut r = old_record(j);
        r.id = first + j as u32;
        r
    };
    let mut records = Vec::from(if first == 0 { OLD_RECORDS } else { PRUNE_RECORDS });
    records.truncate(count);
    // The front's status: a concrete-offset write.
    let front = match at {
        FullAt::Below => gen::status(src),
        FullAt::OpenFront => match src.below(4) {
            0 => Status::Reserved,
            1 => Status::Dispatched,
            2 => Status::Active,
            _ => Status::Unknown,
        },
        FullAt::Prune => Status::Cancelled,
    };
    records[0].status = front.clone();
    let mut base = Vec::with_capacity(gen::MAX_M as usize);
    for _ in 0..nm {
        base.push(gen::val(src, c.val));
    }
    let mut completed_units = Vec::with_capacity(gen::MAX_R as usize + 1);
    for r in 0..gen::MAX_R as usize {
        if r < nr {
            // Free for recipe 0 with the prune (completions of pruned
            // records); otherwise 0 (every retained record is Cancelled).
            completed_units.push(if at != FullAt::Prune || r != 0 {
                0
            } else if src.hint(70) {
                // Plain mode: usually enough for a requirement (1..=100).
                100 + src.below(10)
            } else {
                src.u32()
            });
        }
    }
    let cu0 = if nr > 0 { completed_units[0] } else { 0 };
    let version = src.u32();
    // WIN-1: every record was a submit.
    assume!(version as u64 >= first as u64 + count as u64);
    let mut p = Pool {
        config: cfg,
        values: base,
        version,
        observed_at: src.u64(),
        paused: src.bool(),
        records,
        first_id: first,
        completed_units,
    };
    // The ghost only feeds WIN-1 below (plain mode).
    #[cfg(not(kani))]
    let g = Ghost {
        base: p.values.clone(),
        k0: first,
        eff: None,
        pruned: [0; M],
        done: [0; spec::GR],
    };
    #[cfg(not(kani))]
    assert!(spec::win(&p, &g), "harness: WIN-1 by construction");
    let op = gen::op(src, &p, SUBMIT, c.val);
    let needs0 = match &op {
        Op::Submit { req, .. } => matches!(spec::rcp(&p.config, req.recipe), Some(r) if r.requires.iter().any(|(q, _)| *q == 0)),
        _ => false,
    };
    // The spec's outcome is computed from the pre-state before the call, and
    // the post-state is compared with it and with the known records: no copy
    // of the pool (a copy reads the 256 records back from the heap, and took
    // most of the symbolic execution; RESULTS.md, cost ladder).
    let x = spec::expected(&p, &op);
    // Under Kani the configuration is not compared (deviation 6: no step
    // changes it, `h_step_submit_post`); at this harness's unwind of 258 the
    // copy and the slice comparisons were unrolled about 257 times each.
    #[cfg(not(kani))]
    let cfg0 = gen::copy_config(&p.config);
    #[cfg(not(kani))]
    let h = spec::ghost_step(&g, &p, &op, &x);
    let res = spec::apply(&mut p, &op);
    // PROP: TX-SUBMIT TX-0 TX-VER WIN-1 WIN-2 WIN-ESC at the record limit
    assert!(res == x.res, "TX: exact result at the record limit");
    #[cfg(not(kani))]
    assert!(spec::same_config(&p.config, &cfg0), "TX: configuration unchanged");
    let pruned = res.is_ok() && at == FullAt::Prune;
    let s = pruned as usize;
    let mut ok = p.values.len() == nm
        && p.version == x.version
        && p.observed_at == x.observed_at
        && p.paused == x.paused
        && p.records.len() + s == count + x.push.is_some() as usize
        && p.first_id == first + s as u32
        && p.completed_units.len() == nr
        && (nr == 0 || p.completed_units[0] == cu0);
    for i in 0..M {
        if i < nm && i < p.values.len() && p.values[i] as spec::Z != x.values[i] {
            ok = false;
        }
    }
    // A concrete trip count, not the post-state's length (after the
    // conditional push).
    match at {
        FullAt::Prune => {
            // Either nothing moved (an error), or the other 255 records
            // shifted down by one (the prune).
            if pruned {
                for j in 0..255 {
                    if p.records.get(j) != Some(&old(j + 1)) {
                        ok = false;
                    }
                }
            } else {
                for j in 0..256 {
                    if p.records.get(j) != Some(&old(j)) {
                        ok = false;
                    }
                }
            }
        }
        _ => {
            for j in 0..count {
                let want = if j == 0 {
                    Commitment {
                        status: front.clone(),
                        ..old(0)
                    }
                } else {
                    old(j)
                };
                if p.records.get(j) != Some(&want) {
                    ok = false;
                }
            }
        }
    }
    if let Some(cm) = &x.push {
        if p.records.get(count - s) != Some(cm) {
            ok = false;
        }
    }
    assert!(ok, "TX: exact post-state at the record limit (unchanged on Err: TX-0)");
    match at {
        FullAt::Below => {
            // PROP: WIN-1 (no Full below 256 records, whatever the front)
            assert!(res != Err(Error::Full), "WIN-1: never Full below 256 records");
            cover!(res.is_ok(), "full: Ok at 255 records");
            cover!(res.is_ok() && spec::open(&front), "full: Ok at 255 records with an open front");
        }
        FullAt::OpenFront => {
            // PROP: WIN-ESC (an open front blocks the window: Full once the
            // earlier guards pass, exactly, above)
            assert!(res.is_err() && x.push.is_none(), "WIN-ESC: no admission at 256 records with an open front");
            cover!(res == Err(Error::Full), "full: Full at 256 records with an open front");
        }
        FullAt::Prune => {
            // PROP: WIN-2 WIN-ESC
            assert!(res != Err(Error::Full), "WIN-ESC: a terminal front is pruned, never Full");
            if res.is_ok() {
                assert!(res == Ok(Some(first + 256)), "WIN-2: the new id is first_id + 256");
                assert!(p.record(first) == Err(Error::NotFound), "WIN-2: the pruned id is NotFound");
                assert!(p.record(first + 1).is_ok(), "WIN-2: the next id is retained");
            }
            cover!(res.is_ok(), "full: Ok with prune");
            cover!(res.is_ok() && needs0, "full: Ok with prune, prerequisite met by a pruned completion");
            cover!(res == Err(Error::Prerequisite), "full: Prerequisite with prune (completed_units kept)");
        }
    }
    #[cfg(not(kani))]
    assert!(spec::win(&p, &h), "WIN-1: the window after the step");
}

/// `h_canary`: must FAIL (on this assertion only): the generator and the core
/// together do reach an accepted submit, so the harness setup is not vacuous.
pub fn canary(src: &mut impl Src) {
    let c = src.cfg();
    let Some((mut p, _g)) = gen::inv_state(src, &c) else {
        return;
    };
    let op = gen::op(src, &p, SUBMIT, c.val);
    let res = spec::apply(&mut p, &op);
    assert!(res.is_err(), "canary: no submit is ever accepted");
}

// ---------------------------------------------------------------------------
// Window and cap walks (plain mode)
// ---------------------------------------------------------------------------

/// Plain mode: one step from an INV state with every shared check: the
/// spec's exact outcome and frame (`_tx`, `_post`), LIVE-ALL, every
/// invariant after the step with the ghost's update, and REF-1..3 on the
/// window's image (256 records: the walks fill the window, prune, and meet
/// prerequisites from pruned completions, which the Kani image `TK`, with
/// `first_id = 0`, does not reach). The TLA+ ghost of the post-state from
/// `spec::ghost_step` must be `tla::ghost_after`'s (plan 2.2: the Kani
/// ghost's `pruned` is the TLA+ fold).
#[cfg(not(kani))]
pub fn checked_step(p: &mut Pool, g: &mut Ghost, op: &Op) -> Res {
    let pre = p.clone();
    let x = spec::expected(&pre, op);
    let h = spec::ghost_step(g, &pre, op, &x);
    let res = spec::apply(p, op);
    check_tx(&pre, p, op, &res, &x);
    check_live(&pre, op, &res);
    check_post(&pre, p, op, &res, &x);
    assert_inv(p, &h);
    let (b, nb, k0) = tla_ghost(&pre, g);
    // The image: 8 metrics, recipes and requirements (the fixtures of the
    // F-tests), 64 terms (MAX_TERMS), 256 records.
    crate::tla::assert_refines::<8, 64, 8, 8, 256>(&pre, &b[..nb], k0, op, &res, p); // PROP: REF-1 REF-2 REF-3
    let ga = crate::tla::ghost_after::<M>(&pre, &b[..nb], k0, op, &res, p);
    let (b2, nb2, k2) = tla_ghost(p, &h);
    assert!(
        ga.base[..ga.nb] == b2[..nb2] && ga.k0 == k2,
        "REF ghost: spec::ghost_step's ghost is the TLA+ ghost (plan 2.2)"
    );
    *g = h;
    res
}

#[cfg(not(kani))]
fn acct_of(s: &str) -> &'static str {
    spec::ACCTS.iter().copied().find(|a| *a == s).unwrap_or("z_")
}

/// Plain mode: LIVE-ALL on the open record `id` of `p`: a Reserved record
/// cancelled by its actor or the owner, an in-flight one settled either way
/// by its executor; asserted Ok and terminal (with every check of
/// [`checked_step`]).
#[cfg(not(kani))]
pub fn resolve(src: &mut crate::src::Rng, p: &mut Pool, g: &mut Ghost, id: u32) {
    let completed = src.bool();
    resolve_as(src, p, g, id, completed)
}

/// [`resolve`] with the settle's outcome given.
#[cfg(not(kani))]
pub fn resolve_as(src: &mut crate::src::Rng, p: &mut Pool, g: &mut Ghost, id: u32, completed: bool) {
    let c = p.record(id).expect("resolve: a retained record").clone();
    let r = spec::rcp_of(&p.config, c.recipe);
    let op = if c.status == Status::Reserved {
        let who = if src.bool() { acct_of(&p.config.owner) } else { acct_of(&r.actor) };
        Op::Cancel { caller: who, id, now: p.observed_at }
    } else {
        Op::Settle {
            caller: acct_of(&r.executor),
            id,
            completed,
        }
    };
    let res = checked_step(p, g, &op);
    // PROP: LIVE-ALL
    assert!(res.is_ok(), "LIVE-ALL: {op:?} on {:?} failed {res:?}", c.status);
    assert!(spec::terminal(&p.record(id).expect("retained").status), "LIVE-ALL: the record is terminal");
}

/// Plain mode `window_walk`: `steps` operations of [`gen::window_op`] from
/// `Pool::new` over [`gen::window_config`], long enough to fill the window
/// and prune it many times. Every step: [`checked_step`] (exactness, frame,
/// WIN-1/WIN-2 in `check_post`, LIVE-ALL, the full v1.1 INV including the
/// ghost's `pruned` and `done` branches: INV-LEDGER and INV-CU across
/// prunes). Every 20 steps LIVE-ALL on a random open record (on a copy).
/// Every 120 steps the open records are resolved (mostly released), except
/// those whose id is 7 mod 64 (so the front is sometimes open when the
/// window fills), and the owner observes the initial values again (or
/// refreshes, with such a record open), so that completions do not push the
/// values into a corner where every submit fails Bounds.
#[cfg(not(kani))]
pub fn window_walk(src: &mut crate::src::Rng, steps: usize) {
    let cfg = gen::window_config(src);
    let initial: Vec<i64> = cfg.metrics.iter().map(|m| m.initial).collect();
    let mut p = Pool::new(cfg, 100).expect("valid");
    let mut g = Ghost::init(&p);
    assert_inv(&p, &g);
    for t in 0..steps {
        if t % 120 == 119 {
            let open: Vec<u32> = p
                .records
                .iter()
                .filter(|c| spec::open(&c.status) && c.id % 64 != 7)
                .map(|c| c.id)
                .collect();
            for id in open {
                // Recipe 0 rarely completes (`gen::window_op`).
                let r0 = matches!(spec::rec(&p, id), Some(c) if c.recipe == 0);
                let completed = !r0 && src.hint(20);
                resolve_as(src, &mut p, &mut g, id, completed);
            }
            let vals = if spec::any_open(&p.records) { p.values.clone() } else { initial.clone() };
            let op = Op::Observe {
                caller: acct_of(&p.config.owner),
                vals,
                now: p.observed_at,
            };
            if !p.paused && p.version < VERSION_CAP {
                assert!(checked_step(&mut p, &mut g, &op).is_ok(), "window walk: the owner's observe");
            }
            continue;
        }
        let op = gen::window_op(src, &p);
        let pre = p.clone();
        let res = checked_step(&mut p, &mut g, &op);
        let pruned = p.first_id != pre.first_id;
        cover!(pruned, "window walk: a submit prunes the front");
        cover!(res == Err(Error::Full), "window walk: Full at 256 records with an open front");
        if let Op::Submit { req, .. } = &op {
            let r = spec::rcp_of(&pre.config, req.recipe.min(pre.config.recipes.len() as u32 - 1));
            let by_pruned = r.requires.iter().any(|(q, u)| spec::retained_done(&pre.records, *q) < *u as u64);
            cover!(
                res.is_ok() && req.recipe < pre.config.recipes.len() as u32 && by_pruned,
                "window walk: a prerequisite met by pruned completions"
            );
            cover!(res.is_ok() && pre.first_id > 0, "window walk: a submit with first_id > 0");
        }
        let id = match &op {
            Op::Dispatch { id, .. } | Op::Ack { id, .. } | Op::Cancel { id, .. } | Op::Settle { id, .. } => Some(*id),
            _ => None,
        };
        cover!(
            res == Err(Error::NotFound) && matches!(id, Some(i) if i < pre.first_id),
            "window walk: NotFound on a pruned id"
        );
        cover!(
            res.is_ok() && matches!(op, Op::Observe { .. }) && spec::any_open(&pre.records),
            "window walk: a refresh with records open"
        );
        if t % 20 == 19 {
            let open: Vec<u32> = p.records.iter().filter(|c| spec::open(&c.status)).map(|c| c.id).collect();
            if !open.is_empty() {
                let id = open[src.below(open.len() as u32) as usize];
                let (mut q, mut h) = (p.clone(), g.clone());
                resolve(src, &mut q, &mut h, id);
            }
        }
    }
}

/// Plain mode `cap_walk`: INV-VERCAP and LIVE-ALL from just below the cap.
/// A short window walk from `Pool::new` opens some records; the version is
/// then set to `VERSION_CAP - k` (`k < 24`; INV-VER is no longer tracked,
/// every other invariant still holds: Σ rem ≤ 768); the walk goes on, with
/// [`checked_step`] and VER-CAP (submit, dispatch, set_paused and observe
/// fail at or above the cap); at the end every open record is resolved.
#[cfg(not(kani))]
pub fn cap_walk(src: &mut crate::src::Rng, steps: usize) {
    let cfg = gen::window_config(src);
    let mut p = Pool::new(cfg, 100).expect("valid");
    let mut g = Ghost::init(&p);
    let warm = src.below(40);
    for _ in 0..warm {
        let op = gen::window_op(src, &p);
        let _ = checked_step(&mut p, &mut g, &op);
    }
    p.version = VERSION_CAP - src.below(24);
    g.eff = None;
    assert_inv(&p, &g);
    for _ in 0..steps {
        let op = gen::window_op(src, &p);
        let pre = p.clone();
        let res = checked_step(&mut p, &mut g, &op);
        let starts = matches!(op, Op::Submit { .. } | Op::Dispatch { .. } | Op::Pause { .. } | Op::Observe { .. });
        if pre.version >= VERSION_CAP && starts {
            // PROP: VER-CAP
            assert!(res.is_err() && spec::same_state(&p, &pre), "VER-CAP: nothing starts at or above the cap");
            cover!(res == Err(Error::Arithmetic), "cap walk: refused at the cap");
        }
        cover!(
            res.is_ok() && pre.version >= VERSION_CAP && matches!(op, Op::Settle { .. } | Op::Cancel { .. }),
            "cap walk: a record resolved above the cap"
        );
    }
    // PROP: LIVE-ALL INV-VERCAP: every open record still resolves.
    let open: Vec<u32> = p.records.iter().filter(|c| spec::open(&c.status)).map(|c| c.id).collect();
    for id in open {
        resolve(src, &mut p, &mut g, id);
    }
    assert!(!spec::any_open(&p.records), "LIVE-ALL: every record terminal");
    cover!(p.version > VERSION_CAP, "cap walk: resolved records above the cap");
}
