/-
Per-call properties beyond StepProps, at every size: DEP-1, OBS-1, OBS-2,
the snapshot age bound and PAUSE-1 from every state; REFRESH-1 and VER-CAP
from every `Inv` state.
-/
import PoolLean.Live

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-- `observe` has no no-op path: `Ok` means every guard passed. -/
theorem observe_ok_none {cfg : Cfg A} {s : State} {c : A} {vals : List Int} {now : Nat}
    (h : (exec cfg s (.observe c vals now)).1.isOk = true) :
    outcome cfg s (.observe c vals now) = none := by
  rw [isOk_exec] at h
  cases ho : outcome cfg s (.observe c vals now) with
  | none => rfl
  | some o =>
    cases o with
    | err e => exact absurd ho (h e)
    | noop => have := noop_kind ho; simp [Op.isAck] at this

/-- DEP-1: an accepted `submit` had every prerequisite met: by
`completed_units` (`doneU`, lifetime and saturated) and so by the units of
the Completed records of the whole history, pruned ones included. -/
theorem dep1 {cfg : Cfg A} {s : State} {c : A} {r u ver exp now : Nat}
    (h : (exec cfg s (.submit c r u ver exp now)).1.isOk = true) :
    ∀ pu ∈ (cfg.recipe r).requires, pu.2 ≤ doneU s pu.1 ∧ pu.2 ≤ doneUnits s.records pu.1 := by
  have F := submit_ok (submit_ok_none h)
  have hp := F.prereq
  simp only [prereqFail, List.any_eq_false] at hp
  intro pu hpu
  have h1 : ¬ (doneU s pu.1 < pu.2) := fun h' => hp pu hpu (decide_eq_true h')
  have h2 : doneU s pu.1 ≤ doneUnits s.records pu.1 := Nat.min_le_left _ _
  omega

/-- DEP-1's converse (the prerequisite guard reads the lifetime count, not
the window): a valid recipe whose prerequisites are met by the Completed
records of the history passes the `Prerequisite` guard. -/
theorem dep1_iff {cfg : Cfg A} (hV : Valid cfg) (s : State) {r : Nat}
    (hr : r < cfg.recipes.length) :
    prereqFail (cfg.recipe r) s = false ↔
      ∀ pu ∈ (cfg.recipe r).requires, pu.2 ≤ doneUnits s.records pu.1 := by
  have RO := recipe_ok hV hr
  simp only [prereqFail, List.any_eq_false]
  constructor
  · intro hp pu hpu
    have h1 : ¬ (doneU s pu.1 < pu.2) := fun h' => hp pu hpu (decide_eq_true h')
    have h2 : doneU s pu.1 ≤ doneUnits s.records pu.1 := Nat.min_le_left _ _
    omega
  · intro hp pu hpu hd
    have h1 := hp pu hpu
    have h2 := (RO.req pu hpu).2.2
    have h3 : pu.2 ≤ doneU s pu.1 := by
      simp only [doneU, MAX_UNITS, U32_MAX] at h2 ⊢; omega
    have := of_decide_eq_true hd
    omega

/-- OBS-1: an accepted `observe` found no open record or restated the
current values, and it leaves the records alone (the config is a
parameter of the machine, so it never changes). -/
theorem obs1 {cfg : Cfg A} {s : State} {c : A} {vals : List Int} {now : Nat}
    (h : (exec cfg s (.observe c vals now)).1.isOk = true) :
    (anyOpen s.records = false ∨ vals = s.values) ∧
      (exec cfg s (.observe c vals now)).2.records = s.records ∧
      (exec cfg s (.observe c vals now)).2.values = vals := by
  have ho := observe_ok_none h
  refine ⟨(observe_ok ho).pend.symm, ?_, ?_⟩ <;> rw [exec_none ho] <;> rfl

/-- OBS-2: an accepted `observe` while some record is open (a refresh)
leaves the values and the records unchanged and does not re-base. -/
theorem obs2 {cfg : Cfg A} {s : State} {c : A} {vals : List Int} {now : Nat}
    (h : (exec cfg s (.observe c vals now)).1.isOk = true) (hopen : anyOpen s.records = true) :
    (exec cfg s (.observe c vals now)).2.values = s.values ∧
      (exec cfg s (.observe c vals now)).2.records = s.records ∧
      (exec cfg s (.observe c vals now)).2.base = s.base ∧
      (exec cfg s (.observe c vals now)).2.k0 = s.k0 := by
  have ho := observe_ok_none h
  have hv : vals = s.values := by
    rcases (observe_ok ho).pend with h | h
    · exact h
    · rw [h] at hopen; cases hopen
  have hr : refresh s vals = true := by simp [refresh, hv, hopen]
  rw [exec_none ho]
  exact ⟨hv, rfl, by simp [effect, hr], by simp [effect, hr]⟩

/-- REFRESH-1: below the cap, the owner's `observe` restating the current
values is `Ok` in every `Inv` state, open records or not (v1.0.0 answered
`Pending` while any record was open, F-6). -/
theorem refresh_ok {cfg : Cfg A} {s : State} (I : Inv cfg s) (hv : s.version < VERSION_CAP)
    (now : Nat) : outcome cfg s (.observe cfg.owner s.values now) = none := by
  have W := I.wf
  simp only [outcome, guards, observeGuards, bumpNewGuards, capGuard, verGuard, energyBad,
    firstFail_eq_none, noFire_cons, noFire_nil, List.cons_append, List.nil_append, and_true]
  simp only [VERSION_CAP, MAX_RECORDS, U32_MAX] at hv
  refine ⟨by simp, by simp, by simp [W.vbnd], by simp [W.vlen, W.vbnd],
    decide_eq_false (by simp only [VERSION_CAP, MAX_RECORDS, U32_MAX]; omega),
    decide_eq_false (by simp only [U32_MAX]; omega)⟩

/-- VER-CAP: from `VERSION_CAP` on, no `submit`, `dispatch`, `set_paused` or
`observe` passes its guards: each needs `version < VERSION_CAP`. -/
theorem cap_stops {cfg : Cfg A} {s : State} (hcap : VERSION_CAP ≤ s.version) (op : Op A)
    (hop : (∃ c r u ver exp now, op = .submit c r u ver exp now) ∨ (∃ c id now, op = .dispatch c id now) ∨
      (∃ c b, op = .pause c b) ∨ (∃ c vals now, op = .observe c vals now)) :
    outcome cfg s op ≠ none := by
  intro h
  rcases hop with ⟨c, r, u, ver, exp, now, rfl⟩ | ⟨c, id, now, rfl⟩ | ⟨c, b, rfl⟩ | ⟨c, vals, now, rfl⟩
  · have := (submit_ok h).cap; omega
  · have := (dispatch_ok h).cap; omega
  · have := (pause_ok h).2.1; omega
  · have := (observe_ok h).cap; omega

/-- The snapshot's age bound is inclusive (lib.rs 277-284, 422-429):
`SnapshotExpired` needs `max_age > 0` and `now` strictly past
`observed_at + max_age` (saturating), so at that instant the snapshot is
still fresh. -/
theorem snapshot_fresh {cfg : Cfg A} {s : State} {now : Nat}
    (h : cfg.maxAge = 0 ∨ now ≤ satAdd s.observedAt cfg.maxAge) :
    snapshotOld cfg s now = false := by
  simp only [snapshotOld, decide_eq_false_iff_not, not_and, Nat.not_lt]
  omega

/-- PAUSE-1, first half: while paused, `submit` and `dispatch` change nothing. -/
theorem pause1_blocks {cfg : Cfg A} {s : State} (hp : s.paused = true) :
    (∀ c r u ver exp now, step cfg s (.submit c r u ver exp now) = s) ∧
      (∀ c id now, step cfg s (.dispatch c id now) = s) := by
  constructor
  · intro c r u ver exp now
    have : outcome cfg s (.submit c r u ver exp now) = some (.err .paused) := by
      simp [outcome, guards, submitGuards, firstFail, hp]
    simp only [step, exec_err this]
  · intro c id now
    rcases exec_cases cfg s (.dispatch c id now) with h | ⟨h, -⟩
    · exact h
    · have := (dispatch_ok h).notPaused; rw [hp] at this; cases this

/-- PAUSE-1, second half: `ack`, `cancel`, `settle` and `observe` do not
read the pause flag: from a state that differs only in `paused` they give
the same result and the same post-state (up to `paused`). -/
theorem pause1_transparent (cfg : Cfg A) (s : State) (b : Bool) (op : Op A)
    (hop : (∃ id acc, op = .ack id acc) ∨ (∃ c id now, op = .cancel c id now) ∨
      (∃ c id d, op = .settle c id d) ∨ (∃ c vals now, op = .observe c vals now)) :
    (exec cfg { s with paused := b } op).1 = (exec cfg s op).1 ∧
      (exec cfg { s with paused := b } op).2 = { (exec cfg s op).2 with paused := b } := by
  have hg : outcome cfg { s with paused := b } op = outcome cfg s op := by
    rcases hop with ⟨id, acc, rfl⟩ | ⟨c, id, now, rfl⟩ | ⟨c, id, d, rfl⟩ | ⟨c, vals, now, rfl⟩ <;> rfl
  cases ho : outcome cfg s op with
  | none =>
    have ho' : outcome cfg { s with paused := b } op = none := by rw [hg, ho]
    rw [exec_none ho, exec_none ho']
    rcases hop with ⟨id, acc, rfl⟩ | ⟨c, id, now, rfl⟩ | ⟨c, id, d, rfl⟩ | ⟨c, vals, now, rfl⟩
    · exact ⟨rfl, rfl⟩
    · exact ⟨rfl, rfl⟩
    · cases d <;> exact ⟨rfl, rfl⟩
    · exact ⟨rfl, rfl⟩
  | some o =>
    have ho' : outcome cfg { s with paused := b } op = some o := by rw [hg, ho]
    cases o with
    | err e => rw [exec_err ho, exec_err ho']; exact ⟨rfl, rfl⟩
    | noop => rw [exec_noop ho, exec_noop ho']; exact ⟨rfl, rfl⟩

end Pool
