//! `#[kani::proof]` wrappers: each runs a `checks` body with symbolic values.
//! Bounds and value domains per harness are in RESULTS.md ("Bounds", "Cost
//! ladder").

use crate::checks::{self, PART_COV, PART_INV, PART_POST, PART_REF, PART_TX};
use crate::src::{Cfg, Dom, Shape, Sym, Ver};

/// Step harnesses: 1 metric, 1 term, 2 recipes, exactly 2 records (fixed:
/// a symbolic count doubles the formula, and each further metric, term or
/// recipe is a multiple again; RESULTS.md, cost ladder); values small or near
/// ±1e6; deltas in -3..=3; max_units ≤ 3; weights in {1, 2, 1000};
/// versions, times and expiries free; record ids, request recipes and units
/// over a range that reaches every guard outcome.
const STEP: Cfg = Cfg {
    val: Dom::Anchored(4),
    delta: Dom::Small(3),
    max_units: 3,
    any_weight: false,
    shape: SHAPE,
    ver: Ver::Free,
    part: PART_TX | PART_POST | PART_INV | PART_COV,
};

const SHAPE: Shape = Shape {
    metrics: Some(1),
    terms: Some(1),
    recipes: Some(2),
    records: Some(2),
};

const fn with_part(c: Cfg, part: u8) -> Cfg {
    Cfg { part, ..c }
}

const fn with_records(c: Cfg, k: u8) -> Cfg {
    Cfg {
        shape: Shape {
            records: Some(k),
            ..c.shape
        },
        ..c
    }
}

/// CFG-1 over raw configurations (strings of 0 or 2 bytes; the 128/129-byte
/// boundary of `short` is `h_validate`'s), `validate` free (deviation 23).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::stub(pool_core::Config::validate, crate::model::validate)]
#[kani::unwind(4)]
fn h_init() {
    checks::init(&mut Sym(STEP));
}

/// CFG-1: the initial pool of a valid configuration of the step shape (1
/// metric, 1 term, 2 recipes) satisfies INV (deviation 23).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_init_inv() {
    checks::init_inv(&mut Sym(with_records(STEP, 0)));
}

/// CFG-1: as `h_init_inv`, with 2 metrics, 2 terms and 2 recipes.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_init_inv_2() {
    checks::init_inv(&mut Sym(Cfg {
        shape: Shape {
            metrics: Some(2),
            terms: Some(2),
            recipes: Some(2),
            records: Some(0),
        },
        ..STEP
    }));
}

/// CFG-2 over raw configurations (strings of 0 or 2 bytes, deviation 23).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(4)]
fn h_validate() {
    checks::validate(&mut Sym(STEP));
}

/// NUM-1 (contract): any bounded values, targets and weights (2 metrics, 2
/// terms): `Ok` with one term per term, `Bounds` otherwise.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_energy() {
    checks::energy(&mut Sym(Cfg {
        val: Dom::Full,
        any_weight: true,
        shape: Shape {
            metrics: Some(2),
            terms: Some(2),
            ..SHAPE
        },
        part: PART_TX | PART_COV,
        ..STEP
    }));
}

/// NUM-1 (exactness): each term is `w·r²` and the energy their sum, for
/// values and targets near 0 and ±1e6 and any weight in 1..=1000.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_energy_exact() {
    checks::energy(&mut Sym(Cfg {
        any_weight: true,
        shape: Shape {
            metrics: Some(2),
            terms: Some(2),
            ..SHAPE
        },
        part: PART_POST,
        ..STEP
    }));
}

/// NUM-1: the 64-term worst case.
#[kani::proof]
#[kani::unwind(66)]
fn h_energy_max() {
    checks::energy_max(&mut Sym(STEP));
}

/// A step harness in four parts: `_tx` exact result, `_post` exact
/// post-state, `_inv` the invariants after the step, `_cov` the covers. Each
/// part is its own CBMC run over the same symbolic step.
macro_rules! step_parts {
    ($body:path, $tx:ident, $post:ident, $inv:ident, $cov:ident) => {
        #[kani::proof]
        #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
        #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
        #[kani::unwind(6)]
        fn $tx() {
            $body(&mut Sym(with_part(STEP, PART_TX)));
        }
        #[kani::proof]
        #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
        #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
        #[kani::unwind(6)]
        fn $post() {
            $body(&mut Sym(with_part(STEP, PART_POST)));
        }
        #[kani::proof]
        #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
        #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
        #[kani::unwind(6)]
        fn $inv() {
            $body(&mut Sym(with_part(STEP, PART_INV)));
        }
        #[kani::proof]
        #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
        #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
        #[kani::unwind(6)]
        fn $cov() {
            $body(&mut Sym(with_part(STEP, PART_COV)));
        }
    };
}

step_parts!(checks::step_submit, h_step_submit_tx, h_step_submit_post, h_step_submit_inv, h_step_submit_cov);
step_parts!(checks::step_dispatch, h_step_dispatch_tx, h_step_dispatch_post, h_step_dispatch_inv, h_step_dispatch_cov);
step_parts!(checks::step_ack, h_step_ack_tx, h_step_ack_post, h_step_ack_inv, h_step_ack_cov);
step_parts!(checks::step_cancel, h_step_cancel_tx, h_step_cancel_post, h_step_cancel_inv, h_step_cancel_cov);
step_parts!(checks::step_settle, h_step_settle_tx, h_step_settle_post, h_step_settle_inv, h_step_settle_cov);
step_parts!(checks::step_pause, h_step_pause_tx, h_step_pause_post, h_step_pause_inv, h_step_pause_cov);
step_parts!(checks::step_observe, h_step_observe_tx, h_step_observe_post, h_step_observe_inv, h_step_observe_cov);

/// REF-1..3 (refinement of `tla/SharedPool.tla`, `tla.rs`), one harness
/// per operation kind: the `h_step` pre-state and call (`checks::step_ref`:
/// the same generators, `STEP` and its bounds), the real core applied, then
/// `tla::assert_refines` on the image `TK` (capacity 2 metrics, 2 terms, 2
/// recipes, 2 requirements, 4 records; `first_id = 0`, so no prune: the
/// window's REF is plain mode's, the walks and the traces): REF-1 the
/// core's result is the TLA+ error operator's (`Ok` exactly when the action
/// is enabled or the call is the operator's "Noop"), REF-2 on "Ok"
/// `alpha(post)` is the action's post-state, REF-3 every other call
/// stutters. One cover per value of the operator that the step domain
/// reaches, and both ghost branches of `Observe` (`checks::ref_covers`, the
/// `h_ref` family).
macro_rules! step_ref {
    ($($h:ident = $k:ident);* $(;)?) => {
        $(
            // PROP: REF-1 REF-2 REF-3
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(6)]
            fn $h() {
                checks::step_ref(&mut Sym(with_part(STEP, PART_REF)), crate::gen::$k);
            }
        )*
    };
}

step_ref!(
    h_ref_submit = SUBMIT;
    h_ref_dispatch = DISPATCH;
    h_ref_ack = ACK;
    h_ref_cancel = CANCEL;
    h_ref_settle = SETTLE;
    h_ref_pause = PAUSE;
    h_ref_observe = OBSERVE;
);

/// The `_inv` part from INV states with 0, 1 or 3 records (`_inv` itself
/// has 2). With `h_init_inv` (the initial state) they close the induction
/// for every state with at most 3 records: a step from `k` records ends with
/// `k` or `k + 1`, and each `k` in `0..=3` is a pre-state of every kind.
macro_rules! step_inv_at {
    ($($h:ident = $body:path, $k:literal);* $(;)?) => {
        $(
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(6)]
            fn $h() {
                $body(&mut Sym(with_records(with_part(STEP, PART_INV), $k)));
            }
        )*
    };
}

step_inv_at!(
    h_step_submit_inv_0 = checks::step_submit, 0;
    h_step_submit_inv_1 = checks::step_submit, 1;
    h_step_submit_inv_3 = checks::step_submit, 3;
    h_step_dispatch_inv_0 = checks::step_dispatch, 0;
    h_step_dispatch_inv_1 = checks::step_dispatch, 1;
    h_step_dispatch_inv_3 = checks::step_dispatch, 3;
    h_step_ack_inv_0 = checks::step_ack, 0;
    h_step_ack_inv_1 = checks::step_ack, 1;
    h_step_ack_inv_3 = checks::step_ack, 3;
    h_step_cancel_inv_0 = checks::step_cancel, 0;
    h_step_cancel_inv_1 = checks::step_cancel, 1;
    h_step_cancel_inv_3 = checks::step_cancel, 3;
    h_step_settle_inv_0 = checks::step_settle, 0;
    h_step_settle_inv_1 = checks::step_settle, 1;
    h_step_settle_inv_3 = checks::step_settle, 3;
    h_step_pause_inv_0 = checks::step_pause, 0;
    h_step_pause_inv_1 = checks::step_pause, 1;
    h_step_pause_inv_3 = checks::step_pause, 3;
    h_step_observe_inv_0 = checks::step_observe, 0;
    h_step_observe_inv_1 = checks::step_observe, 1;
    h_step_observe_inv_3 = checks::step_observe, 3;
);

/// VER-CAP, INV-VERCAP and LIVE-ALL at `version >= VERSION_CAP` from INV
/// states, one harness per operation kind (the covers of all seven are one
/// family, `h_version_cap`).
macro_rules! version_cap {
    ($($h:ident = $k:ident),*) => {
        $(
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(6)]
            fn $h() {
                checks::version_cap(
                    &mut Sym(Cfg {
                        ver: Ver::AtCap,
                        ..STEP
                    }),
                    Some(crate::gen::$k),
                );
            }
        )*
    };
}

version_cap!(
    h_version_cap_submit = SUBMIT,
    h_version_cap_dispatch = DISPATCH,
    h_version_cap_ack = ACK,
    h_version_cap_cancel = CANCEL,
    h_version_cap_settle = SETTLE,
    h_version_cap_pause = PAUSE,
    h_version_cap_observe = OBSERVE
);

/// TX-VER at `version = u32::MAX` (no wrap) on WF states, for the three
/// kinds that use the plain increment (one cover family, `h_version_max`).
macro_rules! version_max {
    ($($h:ident = $k:ident),*) => {
        $(
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(6)]
            fn $h() {
                checks::version_max(
                    &mut Sym(Cfg {
                        ver: Ver::Max,
                        ..STEP
                    }),
                    Some(crate::gen::$k),
                );
            }
        )*
    };
}

version_max!(
    h_version_max_ack = ACK,
    h_version_max_cancel = CANCEL,
    h_version_max_settle = SETTLE
);

/// WF states (K-D2): `first_id`, `completed_units`, the values and the
/// version free; up to 2 records. TX, the post-state and the covers in one
/// harness per kind (one cover family, `h_wf`).
macro_rules! wf_step {
    ($($h:ident = $k:ident),*) => {
        $(
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(6)]
            fn $h() {
                checks::wf_step(&mut Sym(with_part(STEP, PART_TX | PART_POST | PART_COV)), Some(crate::gen::$k));
            }
        )*
    };
}

wf_step!(
    h_wf_submit = SUBMIT,
    h_wf_dispatch = DISPATCH,
    h_wf_ack = ACK,
    h_wf_cancel = CANCEL,
    h_wf_settle = SETTLE
);

#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_preview() {
    checks::preview(&mut Sym(with_part(STEP, PART_TX | PART_COV)));
}

/// RD-PREVIEW's strings (`before`, `after`, `terms_*`).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(24)]
fn h_preview_post() {
    checks::preview(&mut Sym(with_part(STEP, PART_POST)));
}

/// RD-BEST: `best_quantity` is the documented search over `candidate` and
/// `energy` (both stubbed by free results, deviation 24), requests of up to
/// 7 units; the state's records do not matter to it (none).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::stub(pool_core::Pool::candidate, crate::model::candidate)]
#[kani::stub(pool_core::Pool::energy, crate::model::energy)]
#[kani::unwind(9)]
fn h_best() {
    checks::best(&mut Sym(with_records(STEP, 0)));
}

/// LIVE-ALL with 3 records (two others may be open), no premise on the
/// base or the version.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_live_all() {
    checks::live_all(&mut Sym(with_records(STEP, 3)));
}

/// F-1 fixed, from `Pool::new` (values in -4..=4 suffice for the shape):
/// one exit call after the hedge is dispatched.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_wedge() {
    checks::wedge(&mut Sym(Cfg {
        val: Dom::Small(4),
        ..STEP
    }));
}

/// F-2 fixed, in every healthy INV state (`B = 0`, the step shape, below
/// the cap) whose record 0 is Dispatched and record 1 Completed (deviation
/// 19; plain mode: the script from `Pool::new`).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_wedge_healthy() {
    checks::wedge_healthy(&mut Sym(Cfg {
        val: Dom::Small(4),
        ..STEP
    }));
}

/// The configuration of `h_bmc_from_init_*`.
const BMC: Cfg = Cfg {
    val: Dom::Small(3),
    delta: Dom::Small(2),
    max_units: 2,
    ..STEP
};

/// Two operations from `Pool::new`: a pause or an observe, then any
/// operation (deviation 20).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(7)]
fn h_bmc_from_init_po() {
    checks::bmc_two(&mut Sym(BMC), crate::gen::PAUSE, None);
}

/// Two operations from `Pool::new`: a submit, then one of kind `$k`, one
/// harness per kind (one cover family, `h_bmc_from_init`; deviation 20).
macro_rules! bmc_from_init {
    ($($h:ident = $k:ident),*) => {
        $(
            #[kani::proof]
            #[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
            #[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
            #[kani::unwind(7)]
            fn $h() {
                checks::bmc_two(&mut Sym(BMC), crate::gen::SUBMIT, Some(crate::gen::$k));
            }
        )*
    };
}

bmc_from_init!(
    h_bmc_from_init_s_submit = SUBMIT,
    h_bmc_from_init_s_dispatch = DISPATCH,
    h_bmc_from_init_s_ack = ACK,
    h_bmc_from_init_s_cancel = CANCEL,
    h_bmc_from_init_s_settle = SETTLE,
    h_bmc_from_init_s_pause = PAUSE,
    h_bmc_from_init_s_observe = OBSERVE
);

/// F-3: a submit/cancel cycle by the actor from `Pool::new`, then the same
/// request accepted again (one cover family, `h_slot_cycle`).
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_slot_cycle_0() {
    checks::slot_cycle(&mut Sym(with_records(STEP, 0)), true);
}

/// F-3: the same cycle from a quiet INV state with 2 records.
#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_slot_cycle_2() {
    checks::slot_cycle(&mut Sym(STEP), false);
}

/// The window: 255 records, the front's status free (never Full).
#[kani::proof]
#[kani::unwind(258)]
fn h_full_255() {
    checks::full(&mut Sym(STEP), Some(checks::FullAt::Below));
}

/// The window: 256 records with an open front (`Full`).
#[kani::proof]
#[kani::unwind(258)]
fn h_full_256() {
    checks::full(&mut Sym(STEP), Some(checks::FullAt::OpenFront));
}

/// The window: 256 records with a terminal front, `first_id = 1`: the
/// submit prunes the front (WIN-2), and `completed_units` keeps the
/// completions of pruned records (mutant 44).
#[kani::proof]
#[kani::unwind(258)]
fn h_full_256_prune() {
    checks::full(&mut Sym(STEP), Some(checks::FullAt::Prune));
}

#[kani::proof]
#[kani::stub(std::vec::Vec::with_capacity_in, crate::model::with_capacity_in)]
#[kani::stub(std::vec::Vec::reserve, crate::model::reserve)]
#[kani::unwind(6)]
fn h_canary() {
    checks::canary(&mut Sym(STEP));
}
