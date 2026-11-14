/-
LEAN-STEP: every call from every state (no invariant needed) satisfies the
step properties TX-0, TX-NOOP/TX-VER, LIFE-1, AUTH-1, STALE-1, ADM-1, ADM-2,
HARD-STEP, WIN-1/WIN-2's step half (`firstId` grows by at most one, a
pruned record never changes) and ASYNC-1 (`stepProps`); TERM-1 (`term1`,
`term1_cancel`: cancelling a Completed record is `InvalidState`);
STALE-1's no-replay over runs of any length (`stale1_no_replay`).
-/
import PoolLean.Inv
import PoolLean.Guard

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-! ## Results -/

theorem isOk_exec {cfg : Cfg A} {s : State} {op : Op A} :
    (exec cfg s op).1.isOk = true ↔ ∀ e, outcome cfg s op ≠ some (.err e) := by
  cases h : outcome cfg s op with
  | none => simp [exec, h, okRes, Res.isOk]; cases op <;> rfl
  | some o => cases o <;> simp [exec, h, Res.isOk]

/-- Only `ack`, `cancel` and `settle` have idempotent no-op paths. -/
theorem noop_kind {cfg : Cfg A} {s : State} {op : Op A} (h : outcome cfg s op = some .noop) :
    op.isAck = true ∨ (∃ c id now, op = .cancel c id now) ∨ (∃ c id d, op = .settle c id d) := by
  have := Guard.outcome_mem cfg s op h
  cases op with
  | ack => exact Or.inl rfl
  | cancel c id now => exact Or.inr (Or.inl ⟨c, id, now, rfl⟩)
  | settle c id d => exact Or.inr (Or.inr ⟨c, id, d, rfl⟩)
  | _ => simp [Guard.kind, Guard.guardOrder] at this

/-- `submit` succeeds only effectively. -/
theorem submit_ok_none {cfg : Cfg A} {s : State} {c : A} {r u ver exp now : Nat}
    (h : (exec cfg s (.submit c r u ver exp now)).1.isOk = true) :
    outcome cfg s (.submit c r u ver exp now) = none := by
  rw [isOk_exec] at h
  cases ho : outcome cfg s (.submit c r u ver exp now) with
  | none => rfl
  | some o =>
    cases o with
    | err e => exact absurd ho (h e)
    | noop => have := noop_kind ho; simp [Op.isAck] at this

/-! ## AUTH-1 -/

/-- AUTH-1: every `Ok` (effective or no-op) implies the caller's authority. -/
theorem exec_auth {cfg : Cfg A} {s : State} {op : Op A} (h : (exec cfg s op).1.isOk = true) :
    Auth1 cfg s op := by
  rw [isOk_exec] at h
  suffices hs : ¬ Auth1 cfg s op → ∃ e, outcome cfg s op = some (.err e) by
    exact Classical.byContradiction fun hn => by obtain ⟨e, he⟩ := hs hn; exact h e he
  intro hn
  cases op with
  | submit c r u ver exp now =>
    simp only [Auth1] at hn
    simp only [outcome, guards, submitGuards, List.cons_append, firstFail, hn, decide_not,
      decide_false, Bool.not_false, ite_true]
    repeat' split
    all_goals exact ⟨_, rfl⟩
  | dispatch c id now =>
    simp only [Auth1] at hn
    simp only [outcome, guards, dispatchGuards, List.cons_append, List.nil_append, firstFail,
      foundGuard, hn, decide_not, decide_false, Bool.not_false, ite_true]
    repeat' split
    all_goals exact ⟨_, rfl⟩
  | ack id acc => exact absurd trivial hn
  | cancel c id now =>
    simp only [Auth1, not_or] at hn
    simp only [outcome, guards, cancelGuards, List.cons_append, firstFail, hn, foundGuard, ne_eq,
      not_false_eq_true, and_self, decide_true, ite_true]
    repeat' split
    all_goals exact ⟨_, rfl⟩
  | settle c id d =>
    simp only [Auth1] at hn
    simp only [outcome, guards, settleGuards, List.cons_append, firstFail, hn, foundGuard,
      decide_not, decide_false, Bool.not_false, ite_true]
    repeat' split
    all_goals exact ⟨_, rfl⟩
  | pause c b =>
    simp only [Auth1] at hn
    exact ⟨.unauthorized, by simp [outcome, guards, pauseGuards, firstFail, hn]⟩
  | observe c vals now =>
    simp only [Auth1] at hn
    exact ⟨.unauthorized, by simp [outcome, guards, observeGuards, firstFail, hn]⟩

/-! ## LIFE-1 -/

theorem life1_refl (rs : List Rec) : Life1 rs rs := by
  refine ⟨Nat.le_refl _, fun j h _ => ⟨rfl, rfl, rfl, rfl, ?_⟩, fun j h hj => absurd h (by omega)⟩
  cases rs[j].status <;> rfl

theorem life1_set {rs : List Rec} {k : Nat} (hk : k < rs.length) {st : Status}
    (he : edge rs[k].status st = true) : Life1 rs (setStatus rs k st) := by
  refine ⟨by rw [length_setStatus]; exact Nat.le_refl _, fun j h h' => ?_, fun j h' hj => ?_⟩
  · rw [getElem_setStatus hk]
    split
    · rename_i e; subst e; exact ⟨rfl, rfl, rfl, rfl, he⟩
    · refine ⟨rfl, rfl, rfl, rfl, ?_⟩; cases rs[j].status <;> rfl
  · rw [length_setStatus] at h'; omega

theorem life1_append (rs : List Rec) (r u exp : Nat) :
    Life1 rs (rs ++ [⟨rs.length, r, u, exp, .reserved⟩]) := by
  refine ⟨by simp, fun j h h' => ?_, fun j h' hj => ?_⟩
  · rw [List.getElem_append_left h]
    refine ⟨rfl, rfl, rfl, rfl, ?_⟩; cases rs[j].status <;> rfl
  · simp only [List.length_append, List.length_singleton] at h'
    have : j = rs.length := by omega
    subst this
    rw [List.getElem_append_right (Nat.le_refl _)]
    simp

/-- A status update of a retained record leaves every pruned record alone. -/
theorem prune_set {rs : List Rec} {k f : Nat} (hk : k < rs.length) (hf : f ≤ k) (st : Status) :
    ∀ j (h : j < rs.length) (h' : j < (setStatus rs k st).length), j < f →
      (setStatus rs k st)[j] = rs[j] := by
  intro j h h' hj
  rw [getElem_setStatus hk]
  split
  · omega
  · rfl

/-! ## Each effective call -/

section calls

variable {cfg : Cfg A} {s : State}

/-- The frame of a status update for HARD-STEP and ASYNC-1. -/
theorem viol_close {k : Nat} {v : List Int} (henv : boundsFail cfg s v none (some k) = false)
    (i : Nat) (hi : i < cfg.n) : viol cfg v i ≤ viol cfg s.values i :=
  Int.le_trans (viol_le_ev cfg s.records v none (some k) i) ((boundsFail_false.mp henv) i hi).1

theorem sp_none {op : Op A} (ho : outcome cfg s op = none)
    (hver : (effect cfg s op).version = s.version + 1)
    (hlife : Life1 s.records (effect cfg s op).records)
    (hsub : ∀ c rc u ver exp now, op = .submit c rc u ver exp now →
      ver = s.version ∧ energy cfg (effect cfg s op).values ≤ energy cfg s.values ∧
      (violating cfg s.values = true →
        energy cfg (addv (effect cfg s op).values (cfg.recipe rc).complete u) < energy cfg s.values))
    (hhard : op.isObserve = false → ∀ i < cfg.n,
      viol cfg (effect cfg s op).values i ≤ viol cfg s.values i)
    (hprune : s.firstId ≤ (effect cfg s op).firstId ∧ (effect cfg s op).firstId ≤ s.firstId + 1 ∧
      ∀ j (h : j < s.records.length) (h' : j < (effect cfg s op).records.length), j < s.firstId →
        (effect cfg s op).records[j] = s.records[j])
    (hasync : op.isAck = true → (effect cfg s op).values = s.values ∧
      (effect cfg s op).records.length = s.records.length ∧
      ∀ j (h : j < s.records.length) (h' : j < (effect cfg s op).records.length),
        (effect cfg s op).records[j].status.isOpen = s.records[j].status.isOpen ∧
        ((effect cfg s op).records[j].status = .cancelled ↔ s.records[j].status = .cancelled)) :
    StepProps cfg s op (exec cfg s op).1 (exec cfg s op).2 := by
  rw [exec_none ho]
  refine ⟨fun e he => ?_, fun _ => Or.inr hver, ?_, hlife, ?_, ?_, ?_, ?_, hhard, hprune, hasync⟩
  · cases op <;> simp [okRes] at he
  · show s.version ≤ (effect cfg s op).version; omega
  · intro _; exact exec_auth (op := op) (by rw [exec_none ho]; cases op <;> rfl)
  · intro c rc u ver exp now hop _; exact ⟨(hsub c rc u ver exp now hop).1, hver⟩
  · intro c rc u ver exp now hop _; exact (hsub c rc u ver exp now hop).2.1
  · intro c rc u ver exp now hop _; exact (hsub c rc u ver exp now hop).2.2

end calls

/-- LEAN-STEP: every call from every state satisfies the step properties. -/
theorem stepProps (cfg : Cfg A) (s : State) (op : Op A) :
    StepProps cfg s op (exec cfg s op).1 (exec cfg s op).2 := by
  cases ho : outcome cfg s op with
  | some o =>
    have hs : (exec cfg s op).2 = s := by cases o <;> simp [exec, ho]
    have hok : (exec cfg s op).1.isOk = true → o = .noop := by
      cases o with
      | noop => intro; rfl
      | err e => simp [exec, ho, Res.isOk]
    have hsub : ∀ c rc u ver exp now, op = .submit c rc u ver exp now →
        (exec cfg s op).1.isOk = true → False := by
      intro c rc u ver exp now hop h
      subst hop
      rw [submit_ok_none h] at ho; cases ho
    refine ⟨fun _ _ => hs, fun _ => Or.inl hs, by rw [hs]; exact Nat.le_refl _, by rw [hs]; exact life1_refl _,
      exec_auth, fun c rc u ver exp now hop h => (hsub c rc u ver exp now hop h).elim,
      fun c rc u ver exp now hop h => (hsub c rc u ver exp now hop h).elim,
      fun c rc u ver exp now hop h => (hsub c rc u ver exp now hop h).elim,
      fun _ i _ => by rw [hs]; exact Int.le_refl _,
      by rw [hs]; exact ⟨Nat.le_refl _, Nat.le_succ _, fun _ _ _ _ => rfl⟩, fun _ => ?_⟩
    rw [hs]
    exact ⟨rfl, rfl, fun j _ _ => ⟨rfl, Iff.rfl⟩⟩
  | none =>
    cases op with
    | submit c r u ver exp now =>
      have F := submit_ok ho
      refine sp_none ho rfl (life1_append _ _ _ _) ?_ ?_ ?_ (by simp [Op.isAck])
      · intro c' rc u' ver' exp' now' hop
        cases hop
        exact ⟨F.ver, F.adm1, F.adm2⟩
      · intro _ i hi
        exact Int.le_trans (viol_le_ev cfg s.records _ (some (r, u)) none i)
          ((boundsFail_false.mp F.bnd) i hi).1
      · refine ⟨?_, ?_, fun j h h' _ => ?_⟩
        · show s.firstId ≤ (if MAX_RECORDS ≤ s.records.length - s.firstId then s.firstId + 1 else s.firstId)
          split <;> omega
        · show (if MAX_RECORDS ≤ s.records.length - s.firstId then s.firstId + 1 else s.firstId) ≤ s.firstId + 1
          split <;> omega
        · exact List.getElem_append_left h
    | dispatch c id now =>
      have F := dispatch_ok ho
      have hk := F.found
      have hst := F.reserved
      rw [record_eq hk] at hst
      refine sp_none ho rfl (life1_set hk (by rw [hst]; rfl)) (fun _ _ _ _ _ _ h => by cases h)
        (fun _ i _ => Int.le_refl _) ⟨Nat.le_refl _, Nat.le_succ _, prune_set hk F.inWin _⟩
        (by simp [Op.isAck])
    | ack id acc =>
      have F := ack_ok (cfg := cfg) ho
      have hk := F.found
      have hst := F.inflight
      rw [record_eq hk] at hst
      have he : edge s.records[id].status (ackStatus acc) = true := by
        have hne := F.changes
        rw [record_eq hk] at hne
        rcases hst with h | h <;> cases acc <;> simp_all [ackStatus, edge]
      refine sp_none ho rfl (life1_set hk he) (fun _ _ _ _ _ _ h => by cases h)
        (fun _ i _ => Int.le_refl _) ⟨Nat.le_refl _, Nat.le_succ _, prune_set hk F.inWin _⟩
        (fun _ => ⟨rfl, length_setStatus _ _ _, fun j h h' => ?_⟩)
      have h'' : j < (setStatus s.records id (ackStatus acc)).length := h'
      show ((setStatus s.records id (ackStatus acc))[j]'h'').status.isOpen =
          s.records[j].status.isOpen ∧
        (((setStatus s.records id (ackStatus acc))[j]'h'').status = .cancelled ↔
          s.records[j].status = .cancelled)
      rw [getElem_setStatus hk]
      split
      · rename_i e; subst e
        rcases hst with h | h <;> cases acc <;> simp [h, ackStatus, Status.isOpen]
      · exact ⟨rfl, Iff.rfl⟩
    | cancel c id now =>
      have F := cancel_ok ho
      have hk := F.found
      have hst := F.reserved
      rw [record_eq hk] at hst
      refine sp_none ho rfl (life1_set hk (by rw [hst]; rfl)) (fun _ _ _ _ _ _ h => by cases h)
        (fun _ i hi => viol_close F.release.bnd i hi)
        ⟨Nat.le_refl _, Nat.le_succ _, prune_set hk F.inWin _⟩ (by simp [Op.isAck])
    | settle c id done =>
      have F := settle_ok ho
      have hk := F.found
      have hst := F.inflight
      rw [record_eq hk] at hst
      cases done with
      | true =>
        have G := F.complete rfl
        refine sp_none ho rfl (life1_set hk ?_) (fun _ _ _ _ _ _ h => by cases h)
          (fun _ i hi => viol_close G.bnd i hi)
          ⟨Nat.le_refl _, Nat.le_succ _, prune_set hk F.inWin _⟩ (by simp [Op.isAck])
        revert hst; cases s.records[id].status <;> simp [inFlight, edge]
      | false =>
        have G := F.release rfl
        refine sp_none ho rfl (life1_set hk ?_) (fun _ _ _ _ _ _ h => by cases h)
          (fun _ i hi => viol_close G.bnd i hi)
          ⟨Nat.le_refl _, Nat.le_succ _, prune_set hk F.inWin _⟩ (by simp [Op.isAck])
        revert hst; cases s.records[id].status <;> simp [inFlight, edge]
    | pause c b =>
      exact sp_none ho rfl (life1_refl _) (fun _ _ _ _ _ _ h => by cases h)
        (fun _ i _ => Int.le_refl _) ⟨Nat.le_refl _, Nat.le_succ _, fun _ _ _ _ => rfl⟩
        (by simp [Op.isAck])
    | observe c vals now =>
      exact sp_none ho rfl (life1_refl _) (fun _ _ _ _ _ _ h => by cases h)
        (by simp [Op.isObserve]) ⟨Nat.le_refl _, Nat.le_succ _, fun _ _ _ _ => rfl⟩
        (by simp [Op.isAck])

/-- LEAN-STEP over `step`. -/
theorem stepProps_step (cfg : Cfg A) (s : State) (op : Op A) :
    StepProps cfg s op (exec cfg s op).1 (step cfg s op) := stepProps cfg s op

/-! ## TERM-1 -/

/-- TERM-1: by the executor, settling a retained terminal record again with
the same outcome is an `Ok` no-op; with the opposite outcome it is
`InvalidState`. -/
theorem term1 (cfg : Cfg A) (s : State) {id : Nat} (hw : s.firstId ≤ id)
    (hk : id < s.records.length) (done : Bool)
    (hterm : s.records[id].status = .completed ∨ s.records[id].status = .cancelled) :
    exec cfg s (.settle (cfg.recipe s.records[id].recipe).executor id done) =
      (if (done = true ∧ s.records[id].status = .completed) ∨
          (done = false ∧ s.records[id].status = .cancelled)
       then .ok else .err .invalidState, s) := by
  have hr := record_eq hk
  rcases hterm with h | h <;> cases done <;>
    simp [exec, outcome, guards, settleGuards, foundGuard, firstFail, hr, h, hk, inFlight,
      Nat.not_le.mpr hk, Nat.not_lt.mpr hw]

/-- TERM-1 for `cancel` (v1.1.0, lib.rs 467-469): cancelling a retained
cancelled record again is an `Ok` no-op; a completed one is `InvalidState`
(v1.0.0 answered `Pending`). -/
theorem term1_cancel (cfg : Cfg A) (s : State) {id : Nat} (hw : s.firstId ≤ id)
    (hk : id < s.records.length) (now : Nat)
    (hterm : s.records[id].status = .completed ∨ s.records[id].status = .cancelled) :
    exec cfg s (.cancel cfg.owner id now) =
      (if s.records[id].status = .cancelled then .ok else .err .invalidState, s) := by
  have hr := record_eq hk
  rcases hterm with h | h <;>
    simp [exec, outcome, guards, cancelGuards, foundGuard, firstFail, hr, h, hk,
      Nat.not_le.mpr hk, Nat.not_lt.mpr hw]

/-! ## STALE-1 over runs -/

theorem version_mono_run (cfg : Cfg A) (s : State) (ops : List (Op A)) :
    s.version ≤ (run cfg s ops).version := by
  induction ops generalizing s with
  | nil => exact Nat.le_refl _
  | cons op ops ih =>
    exact Nat.le_trans (stepProps cfg s op).verMono (ih (step cfg s op))

/-- STALE-1: once a request is accepted, the same request can never be
accepted again, after any calls whatsoever. -/
theorem stale1_no_replay {cfg : Cfg A} {s : State} {c : A} {r u ver exp now : Nat}
    (h : (exec cfg s (.submit c r u ver exp now)).1.isOk = true) (ops : List (Op A)) (now' : Nat) :
    (exec cfg (run cfg (step cfg s (.submit c r u ver exp now)) ops)
      (.submit c r u ver exp now')).1.isOk = false := by
  have P := stepProps cfg s (.submit c r u ver exp now)
  obtain ⟨hv, hv'⟩ := P.stale1 c r u ver exp now rfl h
  have hm := version_mono_run cfg (step cfg s (.submit c r u ver exp now)) ops
  have hm' : (step cfg s (.submit c r u ver exp now)).version = s.version + 1 := hv'
  cases hx : (exec cfg (run cfg (step cfg s (.submit c r u ver exp now)) ops)
      (.submit c r u ver exp now')).1.isOk with
  | false => rfl
  | true =>
    have := ((stepProps cfg _ _).stale1 c r u ver exp now' rfl hx).1
    omega

end Pool
