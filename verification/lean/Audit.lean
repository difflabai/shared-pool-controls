/-
Axiom audit (run.sh audit): every headline theorem may depend only on
Lean's standard axioms `propext`, `Classical.choice` and `Quot.sound`. A
`sorry` shows up here as `sorryAx`, a stray `axiom` by its name, a
`native_decide` as `Lean.ofReduceBool`.
-/
import PoolLean

-- LEAN-IND (INV-WF + WIN-1/WIN-2, INV-LEDGER, INV-RES, PV <= B, INV-DEP, INV-VER, INV-VERCAP)
#print axioms Pool.inv_init
#print axioms Pool.inv_step
#print axioms Pool.reach_inv
#print axioms Pool.hard_of_inv
#print axioms Pool.bump_ok
#print axioms Pool.remSum_le
-- LEAN-STEP (LIFE-1, TERM-1, AUTH-1, STALE-1, ADM-1, ADM-2, HARD-STEP, WIN step, ASYNC-1, TX-0)
#print axioms Pool.stepProps
#print axioms Pool.stepProps_step
#print axioms Pool.exec_auth
#print axioms Pool.term1
#print axioms Pool.term1_cancel
#print axioms Pool.stale1_no_replay
#print axioms Pool.version_mono_run
-- DEP-1, OBS-1, OBS-2, REFRESH-1, VER-CAP, snapshot age, PAUSE-1 (Calls.lean)
#print axioms Pool.dep1
#print axioms Pool.dep1_iff
#print axioms Pool.obs1
#print axioms Pool.obs2
#print axioms Pool.refresh_ok
#print axioms Pool.cap_stops
#print axioms Pool.snapshot_fresh
#print axioms Pool.pause1_blocks
#print axioms Pool.pause1_transparent
-- LEAN-LIVE-ALL (LIVE-ALL, res_sub), LEAN-LIVE1 (LIVE-1) and INV-OUTCOMES
#print axioms Pool.sub_box
#print axioms Pool.res_sub
#print axioms Pool.live_all
#print axioms Pool.live_all_reach
#print axioms Pool.no_wedge
#print axioms Pool.live1
#print axioms Pool.live1_exec
#print axioms Pool.live1_reach
#print axioms Pool.healthy
-- WIN-1, WIN-2, WIN-ESC, AUTH-SELF, CFG-2 (Window.lean)
#print axioms Pool.win1
#print axioms Pool.pruned_terminal
#print axioms Pool.pruned_notFound
#print axioms Pool.envS_win
#print axioms Pool.anyOpen_win
#print axioms Pool.full_iff_front_open
#print axioms Pool.submit_not_full_of_front_terminal
#print axioms Pool.win_esc
#print axioms Pool.submit_prunes
#print axioms Pool.valid_actor_ne_executor
#print axioms Pool.valid_iff
-- LEAN-GUARD
#print axioms Pool.Guard.guards_order
#print axioms Pool.Guard.outcome_mem
-- LEAN-WIT
#print axioms Pool.Witness.wcfg_valid
#print axioms Pool.Witness.f1_rejected
#print axioms Pool.Witness.f1_envelope
#print axioms Pool.Witness.recovery_admitted
#print axioms Pool.Witness.res_needed
#print axioms Pool.Witness.live_all_needs_res
#print axioms Pool.Witness.f2_rejected
#print axioms Pool.Witness.self_executor_invalid
#print axioms Pool.Witness.owner_executor_valid
#print axioms Pool.Witness.pause_run
#print axioms Pool.Witness.cap_reach
#print axioms Pool.Witness.cap_instance
#print axioms Pool.Witness.cap_drain
#print axioms Pool.Witness.cap_headroom
#print axioms Pool.Witness.refresh_instance
#print axioms Pool.Witness.live1_instance
#print axioms Pool.Witness.healthy_settle
#print axioms Pool.Witness.cancelled_reach
#print axioms Pool.Witness.stale_instance
#print axioms Pool.Witness.unknown_settle
#print axioms Pool.Witness.cancel_completed_instance
