/-
LEAN-WIT: concrete witnesses, all checked by `decide` / `rfl` in the kernel
(never `native_decide`). Each v1.1.0 finding fixed, on a concrete config
(the configs are the candidate corpus's traces; accounts o = 0, a1 = 1,
x1 = 2, a2 = 3):

* F-1 (trace 0): after `observe [12, 5]` the F-1 hedge `submit` is
  rejected `Bounds` (`f1_rejected`): the hull counts the hedge's release,
  which would leave metric 0 at 12 (v1.0.0 admitted it, and the pool
  wedged: `f1_forever` on the v1.0.0 tag). The recovery hedge of trace 214,
  whose completion lowers metric 0, is admitted and settles
  (`recovery_admitted`).
* F-2 (trace 209): the risk config's hedge, admitted on v1.0.0 on the
  strength of the open position, is rejected `Bounds` (`f2_rejected`).
* F-4 (trace 210): a recipe whose actor is its executor fails `validate`
  (`self_executor_invalid`); the owner may still execute (trace 213,
  `owner_executor_valid`).
* F-5 (`cap_instance`): a reachable state at `VERSION_CAP` (4294966527)
  with a Dispatched and a Reserved record, reached from `init` by a run of
  `VERSION_CAP` calls (`cap_reach`, by induction, not evaluation): new work
  (`submit`, `dispatch`, `set_paused`, `observe`) fails `Arithmetic`, and
  every open record is still resolved `Ok` (`cap_drain`).
* F-6 (trace 211): with records open and the snapshot expired, the owner's
  refresh is `Ok`, keeps the values and records, and unblocks `dispatch`
  (`refresh_instance`); a changed value is still `Pending`.
* Non-vacuity: a valid config and a reachable state with an in-flight
  record where LIVE-1 applies (and its conclusion is observed concretely),
  and reachable Completed and Cancelled records.
-/
import PoolLean.Window

namespace Pool.Witness
open Pool

theorem reach_run {A : Type} [DecidableEq A] [Inhabited A] {cfg : Cfg A} {s : State}
    (h : Reach cfg s) (ops : List (Op A)) : Reach cfg (run cfg s ops) := by
  induction ops generalizing s with
  | nil => exact h
  | cons op ops ih => exact ih (Reach.step op h)

theorem run_cons {A : Type} [DecidableEq A] [Inhabited A] (cfg : Cfg A) (s : State) (op : Op A)
    (ops : List (Op A)) : run cfg s (op :: ops) = run cfg (step cfg s op) ops := rfl

theorem run_append {A : Type} [DecidableEq A] [Inhabited A] (cfg : Cfg A) (s : State)
    (l₁ l₂ : List (Op A)) : run cfg s (l₁ ++ l₂) = run cfg (run cfg s l₁) l₂ := by
  simp [run, List.foldl_append]

/-! ## F-1 -/

/-- The F-1 config: m(0,10,5), m(-100,100,5); t(1,0,1,target);
r(a1,x1,[-2,0],[2,-5],1,[]). -/
def wcfg : Cfg Nat :=
  { owner := 0, maxAge := 0,
    metrics := [⟨0, 10, 5⟩, ⟨-100, 100, 5⟩],
    terms := [⟨1, 0, 1, .target⟩],
    recipes := [⟨1, 2, [-2, 0], [2, -5], 1, []⟩] }

theorem wcfg_valid : Valid wcfg := (valid_iff wcfg).mp (by decide)

/-- After the owner observes [12, 5] (metric 0 above its max of 10)... -/
def f1obs : State := run wcfg (init wcfg 100) [.observe 0 [12, 5] 100]

/-- ... the F-1 hedge is rejected `Bounds`: its release would leave metric 0
at 12 (violation 2), above the candidate's 0. On v1.0.0 it was admitted
and the pool wedged (trace 0). -/
theorem f1_rejected :
    (exec wcfg (init wcfg 100) (.observe 0 [12, 5] 100)).1 = .ok ∧
      exec wcfg f1obs (.submit 1 0 1 1 200 100) = (.err .bounds, f1obs) := by decide

/-- The envelope that rejects it: low [10, 0], high [12, 5]; EV₀ = 2 while
the candidate's own violation is 0. -/
theorem f1_envelope :
    low wcfg f1obs.records [10, 5] (some (0, 1)) none 0 = 10 ∧
      high wcfg f1obs.records [10, 5] (some (0, 1)) none 0 = 12 ∧
      ev wcfg f1obs.records [10, 5] (some (0, 1)) none 0 = 2 ∧ viol wcfg [10, 5] 0 = 0 := by
  decide

/-- The recovery config of trace 214: r(a1,x1,[0,-1],[-3,0],1,[]). -/
def rcfg : Cfg Nat :=
  { owner := 0, maxAge := 0,
    metrics := [⟨0, 10, 5⟩, ⟨-100, 100, 5⟩],
    terms := [⟨1, 0, 1, .target⟩],
    recipes := [⟨1, 2, [0, -1], [-3, 0], 1, []⟩] }

theorem rcfg_valid : Valid rcfg := (valid_iff rcfg).mp (by decide)

def rops : List (Op Nat) :=
  [.observe 0 [12, 5] 100, .submit 1 0 1 1 200 100, .dispatch 1 0 100, .ack 0 true,
   .settle 2 0 true]

/-- F-1 recovery (trace 214): a hedge whose completion lowers metric 0 is
admitted in the disturbed state, and settles, bringing metric 0 to 9. -/
theorem recovery_admitted :
    (exec rcfg (run rcfg (init rcfg 100) (rops.take 1)) (.submit 1 0 1 1 200 100)).1 = .okId 0 ∧
      (exec rcfg (run rcfg (init rcfg 100) (rops.take 4)) (.settle 2 0 true)).1 = .ok ∧
      (run rcfg (init rcfg 100) rops).values = [9, 4] ∧
      (run rcfg (init rcfg 100) rops).records = [⟨0, 0, 1, 200, .completed⟩] := by decide

/-- The v1.0.0 wedge, as a v1.1.0 state: record 0 Active, values [10, 5]
after the observation [12, 5]. v1.1.0 never reaches it (`f1_rejected`). -/
def w10 : State :=
  { values := [10, 5], version := 4, observedAt := 100, paused := false,
    records := [⟨0, 0, 1, 200, .active⟩], firstId := 0, base := [12, 5], k0 := 0, eff := 4 }

/-- INV-RES is what LIVE-ALL needs: the wedge satisfies every other
conjunct of `Inv`, yet both of the executor's settles fail `Bounds`. So
the canary `canary/LiveAllNoRes.lean` states a false theorem. -/
theorem res_needed :
    WF wcfg w10 ∧ Ledger wcfg w10 ∧ PvB wcfg w10 ∧ Dep wcfg w10 ∧ Ver w10 ∧ VerCap w10 ∧
      ¬ InvRes wcfg w10 ∧
      exec wcfg w10 (.settle 2 0 true) = (.err .bounds, w10) ∧
      exec wcfg w10 (.settle 2 0 false) = (.err .bounds, w10) := by
  refine ⟨⟨by decide, by decide, by decide, by decide, by decide, by decide, by decide, by decide,
    fun j hj => ?_⟩, by unfold Ledger; decide, by unfold PvB; decide, fun j hj pu hpu => ?_,
    by unfold Ver; decide, by unfold VerCap; decide, by unfold InvRes; decide, by decide,
    by decide⟩
  · have : j = 0 := by simp [w10] at hj; omega
    subst this
    exact ⟨rfl, by simp [w10, wcfg], by simp [w10], by simp [w10, wcfg, Cfg.recipe],
      fun h => absurd h (by simp [w10]), fun h => absurd h (by simp [w10])⟩
  · have : j = 0 := by simp [w10] at hj; omega
    subst this
    simp [w10, wcfg, Cfg.recipe] at hpu

theorem live_all_needs_res : ¬ ∀ s : State, WF wcfg s → Ledger wcfg s → PvB wcfg s →
    Dep wcfg s → Ver s → VerCap s → ∀ id (hk : id < s.records.length),
      inFlight s.records[id].status = true →
      outcome wcfg s (.settle (wcfg.recipe s.records[id].recipe).executor id true) = none := by
  intro h
  obtain ⟨W, L, P, D, V, VC, -, hs, -⟩ := res_needed
  have := h w10 W L P D V VC 0 (by decide) (by decide)
  have e : (wcfg.recipe (w10.records[0]'(by decide)).recipe).executor = 2 := rfl
  rw [e] at this
  rw [exec_none this] at hs
  cases hs

/-! ## F-2 -/

/-- The risk config of trace 209 (a2 = 3). -/
def f2cfg : Cfg Nat :=
  { owner := 0, maxAge := 300,
    metrics := [⟨0, 100, 60⟩, ⟨0, 80, 40⟩, ⟨0, 60, 30⟩, ⟨0, 40, 40⟩],
    terms := [⟨0, 80, 1, .excess⟩, ⟨1, 60, 1, .excess⟩, ⟨2, 40, 1, .excess⟩],
    recipes := [⟨1, 2, [20, 5, 0, -5], [0, 0, 0, 0], 20, []⟩,
                ⟨3, 2, [10, 0, 5, -5], [0, 0, 0, 0], 20, []⟩,
                ⟨3, 2, [0, 10, 5, -5], [0, 0, 0, 0], 20, []⟩,
                ⟨3, 2, [0, 0, 0, -2], [-10, 0, 0, 0], 20, []⟩] }

theorem f2cfg_valid : Valid f2cfg := (valid_iff f2cfg).mp (by decide)

def f2open : State := run f2cfg (init f2cfg 100) [.submit 1 0 1 0 200 100]

/-- F-2 (trace 209): with position 0 open, the hedge (recipe 3, 8 units) is
rejected `Bounds`: releasing position 0 would take metric 0 to 60 - 80 =
-20 below its min of 0. v1.0.0 admitted it on the position's strength. -/
theorem f2_rejected :
    (exec f2cfg (init f2cfg 100) (.submit 1 0 1 0 200 100)).1 = .okId 0 ∧
      exec f2cfg f2open (.submit 3 3 8 1 200 100) = (.err .bounds, f2open) := by decide

/-! ## F-4 -/

/-- Trace 210: recipe 0's actor a1 is its executor (and recipe 1's is the
owner). -/
def scfg : Cfg Nat :=
  { owner := 0, maxAge := 0,
    metrics := [⟨0, 10, 5⟩],
    terms := [⟨0, 0, 1, .excess⟩],
    recipes := [⟨1, 1, [-1], [0], 2, []⟩, ⟨0, 0, [-1], [0], 2, []⟩] }

/-- F-4: `validate` refuses a recipe whose actor is its own executor. -/
theorem self_executor_invalid : validate scfg = false ∧ ¬ Valid scfg := by
  refine ⟨by decide, fun h => ?_⟩
  have := (valid_iff scfg).mpr h
  revert this; decide

/-- Trace 213: the owner may be a recipe's executor. -/
def ocfg : Cfg Nat :=
  { owner := 0, maxAge := 0,
    metrics := [⟨0, 10, 5⟩],
    terms := [⟨0, 0, 1, .excess⟩],
    recipes := [⟨1, 0, [-1], [0], 2, []⟩] }

theorem owner_executor_valid : Valid ocfg := (valid_iff ocfg).mp (by decide)

/-! ## F-5 -/

/-- A config whose recipe moves nothing: m(0,100,50); t(0,50,1,target);
r(a1,x1,[0],[0],1,[]). -/
def ccfg : Cfg Nat :=
  { owner := 0, maxAge := 0,
    metrics := [⟨0, 100, 50⟩],
    terms := [⟨0, 50, 1, .target⟩],
    recipes := [⟨1, 2, [0], [0], 1, []⟩] }

theorem ccfg_valid : Valid ccfg := (valid_iff ccfg).mp (by decide)

def cpre : List (Op Nat) := [.submit 1 0 1 0 200 100, .submit 1 0 1 1 200 100, .dispatch 1 0 101]

/-- Record 0 Dispatched, record 1 Reserved, at version 3. -/
def s3 : State :=
  { values := [50], version := 3, observedAt := 100, paused := false,
    records := [⟨0, 0, 1, 200, .dispatched⟩, ⟨1, 0, 1, 200, .reserved⟩], firstId := 0,
    base := [50], k0 := 0, eff := 3 }

theorem cpre_eq : run ccfg (init ccfg 100) cpre = s3 := by decide

/-- Below the cap, the owner's `set_paused(false)` from an unpaused state
only bumps the version. -/
theorem pause_run (s : State) (hp : s.paused = false) (n : Nat)
    (h : s.version + n ≤ VERSION_CAP) :
    run ccfg s (List.replicate n (.pause 0 false)) =
      { s with version := s.version + n, eff := s.eff + n } := by
  induction n generalizing s with
  | zero => cases s; simp at hp ⊢; simp [run, hp]
  | succ n ih =>
    have hv : s.version < VERSION_CAP := by omega
    have ho : outcome ccfg s (.pause 0 false) = none := by
      simp only [outcome, guards, pauseGuards, bumpNewGuards, capGuard, verGuard,
        firstFail_eq_none, noFire_cons, noFire_nil, List.cons_append, List.nil_append, and_true]
      refine ⟨by decide, decide_eq_false (by omega),
        decide_eq_false (by simp only [VERSION_CAP, MAX_RECORDS] at hv ⊢; omega)⟩
    rw [List.replicate_succ, run_cons]
    have hs : step ccfg s (.pause 0 false) =
        { s with version := s.version + 1, eff := s.eff + 1 } := by
      simp only [step, exec_none ho, effect]
      cases s; simp at hp ⊢; exact hp
    rw [hs, ih { s with version := s.version + 1, eff := s.eff + 1 } hp
      (by show s.version + 1 + n ≤ VERSION_CAP; omega)]
    simp only [Nat.add_assoc, Nat.add_comm 1 n]

/-- `cpre`, then `VERSION_CAP - 3` pauses. -/
def capOps : List (Op Nat) := cpre ++ List.replicate (VERSION_CAP - 3) (.pause 0 false)

/-- The state at the cap. -/
def scap : State := { s3 with version := VERSION_CAP, eff := VERSION_CAP }

theorem cap_eq : run ccfg (init ccfg 100) capOps = scap := by
  rw [capOps, run_append, cpre_eq, pause_run s3 rfl _ (by simp [s3, VERSION_CAP, MAX_RECORDS, U32_MAX])]
  simp [scap, s3, VERSION_CAP, MAX_RECORDS, U32_MAX]

theorem cap_reach : Reach ccfg scap := cap_eq ▸ reach_run (Reach.init 100) capOps

/-- F-5 at the cap: new work fails `Arithmetic` (`submit` with the current
version, `dispatch` of the Reserved record, `set_paused`, `observe` of the
current values), while the open records are resolved `Ok`: the executor
settles the Dispatched one either way, the actor cancels the Reserved one,
and `acknowledge` moves the Dispatched one. -/
theorem cap_instance :
    exec ccfg scap (.submit 1 0 1 VERSION_CAP 200 100) = (.err .arithmetic, scap) ∧
      exec ccfg scap (.dispatch 1 1 101) = (.err .arithmetic, scap) ∧
      exec ccfg scap (.pause 0 true) = (.err .arithmetic, scap) ∧
      exec ccfg scap (.observe 0 [50] 102) = (.err .arithmetic, scap) ∧
      (exec ccfg scap (.settle 2 0 true)).1 = .ok ∧
      (exec ccfg scap (.settle 2 0 false)).1 = .ok ∧
      (exec ccfg scap (.cancel 1 1 102)).1 = .ok ∧
      (exec ccfg scap (.ack 0 true)).1 = .ok := by
  simp only [scap, s3, VERSION_CAP, MAX_RECORDS, U32_MAX]
  decide

/-- ... and both open records can be resolved in turn, past the cap. -/
theorem cap_drain :
    run ccfg scap [.ack 0 true, .settle 2 0 true, .cancel 1 1 102] =
      { scap with version := VERSION_CAP + 3, eff := VERSION_CAP + 3,
                  records := [⟨0, 0, 1, 200, .completed⟩, ⟨1, 0, 1, 200, .cancelled⟩] } := by
  simp only [scap, s3, VERSION_CAP, MAX_RECORDS, U32_MAX]
  decide

/-- INV-VERCAP is tight enough: at the cap the headroom left is 762. -/
theorem cap_headroom : scap.version + remSum scap.records + 762 = U32_MAX := by
  simp only [scap, s3, VERSION_CAP, MAX_RECORDS, U32_MAX]
  decide

/-! ## F-6 -/

/-- Trace 211's config: m(0,10,5); t(0,0,1,excess); two recipes
r(a1,x1,[-1],[0],2,[]); max_age 50. -/
def fcfg : Cfg Nat :=
  { owner := 0, maxAge := 50,
    metrics := [⟨0, 10, 5⟩],
    terms := [⟨0, 0, 1, .excess⟩],
    recipes := [⟨1, 2, [-1], [0], 2, []⟩, ⟨1, 2, [-1], [0], 2, []⟩] }

theorem fcfg_valid : Valid fcfg := (valid_iff fcfg).mp (by decide)

def fops : List (Op Nat) :=
  [.submit 1 0 1 0 1000 100, .dispatch 1 0 100, .ack 0 true, .submit 1 1 1 3 1000 120]

/-- Record 0 Active, record 1 Reserved, values [3], observed at 100. -/
def fstate : State := run fcfg (init fcfg 100) fops

/-- F-6 (trace 211): at time 151 the snapshot has expired, so `dispatch`
fails; a changed observation is `Pending` (records are open); restating
the values is `Ok`, keeps values and records, and after it the `dispatch`
is `Ok`. v1.0.0 answered `Pending` to the refresh too. -/
theorem refresh_instance :
    (exec fcfg fstate (.dispatch 1 1 151)).1 = .err .snapshotExpired ∧
      (exec fcfg fstate (.observe 0 [4] 151)).1 = .err .pending ∧
      (exec fcfg fstate (.observe 0 [3] 151)).1 = .ok ∧
      (step fcfg fstate (.observe 0 [3] 151)).values = fstate.values ∧
      (step fcfg fstate (.observe 0 [3] 151)).records = fstate.records ∧
      (exec fcfg (step fcfg fstate (.observe 0 [3] 151)) (.dispatch 1 1 152)).1 = .ok := by
  decide

/-! ## Non-vacuity: the healthy path -/

/-- From `init`: submit 1 unit, dispatch it. -/
def hops : List (Op Nat) := [.submit 1 0 1 0 200 100, .dispatch 1 0 101]

def hstate : State := run wcfg (init wcfg 100) hops

theorem hstate_eq : hstate =
    { values := [3, 5], version := 2, observedAt := 100, paused := false,
      records := [⟨0, 0, 1, 200, .dispatched⟩], firstId := 0, base := [5, 5], k0 := 0,
      eff := 2 } := by decide

theorem hstate_reach : Reach wcfg hstate := reach_run (Reach.init 100) hops

/-- LIVE-1 applies on a reachable state with an in-flight record. -/
theorem live1_premises : ∃ h : 0 < hstate.records.length, inFlight hstate.records[0].status = true := by
  rw [hstate_eq]; exact ⟨by decide, by decide⟩

/-- `live1` applied to it, and the concrete settle agreeing. -/
theorem live1_instance : outcome wcfg hstate (.settle 2 0 true) = none := by
  have ⟨hk, hst⟩ := live1_premises
  have := live1_reach wcfg_valid hstate_reach hk hst
  have e : (wcfg.recipe (hstate.records[0]'hk).recipe).executor = 2 := by
    simp only [hstate_eq]; rfl
  rw [e] at this
  exact this

theorem healthy_settle :
    exec wcfg hstate (.settle 2 0 true) =
      (.ok, { values := [5, 0], version := 3, observedAt := 100, paused := false,
              records := [⟨0, 0, 1, 200, .completed⟩], firstId := 0, base := [5, 5], k0 := 0,
              eff := 3 }) := by
  decide

/-- A Cancelled record is reachable too (submit, then the actor cancels). -/
theorem cancelled_reach :
    run wcfg (init wcfg 100) [.submit 1 0 1 0 200 100, .cancel 1 0 102] =
      { values := [5, 5], version := 2, observedAt := 100, paused := false,
        records := [⟨0, 0, 1, 200, .cancelled⟩], firstId := 0, base := [5, 5], k0 := 0,
        eff := 2 } := by decide

/-- STALE-1 concretely: a submit carrying an old version fails `Stale`. -/
theorem stale_instance : (exec wcfg hstate (.submit 1 0 1 0 300 102)).1 = .err .stale := by decide

/-- A healthy dispatched record whose execution the executor reports unknown,
then settles: the Unknown branch of LIVE-1. -/
theorem unknown_settle :
    (exec wcfg (run wcfg hstate [.ack 0 false]) (.settle 2 0 true)).1 = .ok ∧
      (run wcfg hstate [.ack 0 false]).records = [⟨0, 0, 1, 200, .unknown⟩] := by decide

/-- TERM-1 concretely: cancelling the completed record is `InvalidState`
(v1.0.0: `Pending`). -/
theorem cancel_completed_instance :
    (exec wcfg (step wcfg hstate (.settle 2 0 true)) (.cancel 0 0 102)).1 = .err .invalidState := by
  decide

end Pool.Witness
