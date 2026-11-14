/-
What a call that passes every guard knows: one structure per entry point,
read off the guard list (`outcome = none`).
-/
import PoolLean.Lemmas

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-- The simp set that turns `NoFire` of a guard list into its conditions. -/
macro "guard_simp" " at " h:ident : tactic => `(tactic| (
  simp only [outcome, guards, submitGuards, dispatchGuards, ackGuards, cancelGuards,
    settleGuards, releaseGuards, pauseGuards, observeGuards, addGuards, boundsGuards, verGuard,
    capGuard, bumpNewGuards, foundGuard, firstFail_eq_none, List.map, noFire_append, noFire_cons, noFire_nil,
    List.cons_append, List.nil_append, and_true] at $h:ident
  simp only [decide_eq_false_iff_not, Bool.not_eq_false', decide_eq_true_eq,
    Bool.and_eq_false_imp, not_or, Classical.not_not, ne_eq, Nat.not_lt, Nat.not_le,
    and_assoc, not_and] at $h:ident))

structure SubmitOk (cfg : Cfg A) (s : State) (c : A) (r u ver exp now : Nat) : Prop where
  notPaused : s.paused = false
  ver : ver = s.version
  expiry : now < exp ∧ exp ≤ satAdd now DAY
  snapshot : snapshotOld cfg s now = false
  full : fullFires s = false
  recipe : r < cfg.recipes.length
  auth : c = (cfg.recipe r).actor
  unitsPos : 0 < u
  unitsLe : u ≤ (cfg.recipe r).maxUnits
  prereq : prereqFail (cfg.recipe r) s = false
  len : s.values.length = (cfg.recipe r).reserve.length
  nextB : AllB (addv s.values (cfg.recipe r).reserve u)
  env : envFail cfg s.records (addv s.values (cfg.recipe r).reserve u) (some (r, u)) none = false
  bnd : boundsFail cfg s (addv s.values (cfg.recipe r).reserve u) (some (r, u)) none = false
  adm1 : energy cfg (addv s.values (cfg.recipe r).reserve u) ≤ energy cfg s.values
  adm2 : violating cfg s.values = true →
    energy cfg (addv (addv s.values (cfg.recipe r).reserve u) (cfg.recipe r).complete u) <
      energy cfg s.values
  cap : s.version < VERSION_CAP
  noOverflow : s.version < U32_MAX
  idOk : s.records.length ≤ U32_MAX

theorem submit_ok {cfg : Cfg A} {s : State} {c : A} {r u ver exp now : Nat}
    (h : outcome cfg s (.submit c r u ver exp now) = none) : SubmitOk cfg s c r u ver exp now := by
  guard_simp at h
  obtain ⟨h1, h2, h3, h3', h4, h5, h6, h7, h8, h8', h9, h10, h11, h12, h13, -, -, h16, -, -, -, h20,
    h21, h22, h23⟩ := h
  exact ⟨h1, h2, ⟨h3, h3'⟩, h4, h5, h6, h7, by omega, h8', h9, h10, h11,
    h12, h13, h16, h20, h21, h22, h23⟩

/-- `release(id)` passes its guards. -/
structure ReleaseOk (cfg : Cfg A) (s : State) (id : Nat) : Prop where
  len : s.values.length = (cfg.recipe (s.record id).recipe).reserve.length
  nextB : AllB (releaseNext cfg s id)
  env : envFail cfg s.records (releaseNext cfg s id) none (some id) = false
  bnd : boundsFail cfg s (releaseNext cfg s id) none (some id) = false
  noOverflow : s.version < U32_MAX

theorem release_ok {cfg : Cfg A} {s : State} {id : Nat} (h : NoFire (releaseGuards cfg s id)) :
    ReleaseOk cfg s id := by
  simp only [releaseGuards, addGuards, boundsGuards, verGuard, foundGuard, noFire_append,
    noFire_cons, noFire_nil, and_true, List.cons_append, List.nil_append] at h
  simp only [decide_eq_false_iff_not, Bool.not_eq_false', decide_eq_true_eq, not_or,
    Classical.not_not, ne_eq, Nat.not_lt, Nat.not_le, and_assoc, length_neg] at h
  obtain ⟨-, -, h1, h2, h3, h4, -, -, h5⟩ := h
  exact ⟨h1, h2, h3, h4, h5⟩

/-- `settle(.., true)` passes its last guards. -/
structure CompleteOk (cfg : Cfg A) (s : State) (id : Nat) : Prop where
  len : s.values.length = (cfg.recipe (s.record id).recipe).complete.length
  nextB : AllB (completeNext cfg s id)
  env : envFail cfg s.records (completeNext cfg s id) none (some id) = false
  bnd : boundsFail cfg s (completeNext cfg s id) none (some id) = false
  noOverflow : s.version < U32_MAX

structure DispatchOk (cfg : Cfg A) (s : State) (c : A) (id now : Nat) : Prop where
  inWin : s.firstId ≤ id
  found : id < s.records.length
  auth : c = (cfg.recipe (s.record id).recipe).actor
  notPaused : s.paused = false
  reserved : (s.record id).status = .reserved
  live : now < (s.record id).expiresAt
  snapshot : snapshotOld cfg s now = false
  cap : s.version < VERSION_CAP
  noOverflow : s.version < U32_MAX

theorem dispatch_ok {cfg : Cfg A} {s : State} {c : A} {id now : Nat}
    (h : outcome cfg s (.dispatch c id now) = none) : DispatchOk cfg s c id now := by
  guard_simp at h
  obtain ⟨h0, h1, h2, h3, h4, h5, h6, -, -, h7, h8⟩ := h
  exact ⟨h0, h1, h2, h3, h4, h5, h6, h7, h8⟩

structure AckOk (s : State) (id : Nat) (acc : Bool) : Prop where
  inWin : s.firstId ≤ id
  found : id < s.records.length
  inflight : (s.record id).status = .dispatched ∨ (s.record id).status = .unknown
  changes : (s.record id).status ≠ ackStatus acc
  noOverflow : s.version < U32_MAX

theorem ack_ok {cfg : Cfg A} {s : State} {id : Nat} {acc : Bool}
    (h : outcome cfg s (.ack id acc) = none) : AckOk s id acc := by
  guard_simp at h
  obtain ⟨h0, h1, h2, -, -, h3, h4⟩ := h
  refine ⟨h0, h1, ?_, h3, h4⟩
  by_cases hd : (s.record id).status = .dispatched
  · exact Or.inl hd
  · exact Or.inr (h2 hd)

structure CancelOk (cfg : Cfg A) (s : State) (c : A) (id : Nat) : Prop where
  inWin : s.firstId ≤ id
  found : id < s.records.length
  auth : c = (cfg.recipe (s.record id).recipe).actor ∨ c = cfg.owner
  reserved : (s.record id).status = .reserved
  release : ReleaseOk cfg s id

theorem cancel_ok {cfg : Cfg A} {s : State} {c : A} {id now : Nat}
    (h : outcome cfg s (.cancel c id now) = none) : CancelOk cfg s c id := by
  simp only [outcome, guards, cancelGuards, foundGuard, firstFail_eq_none, noFire_append,
    noFire_cons, noFire_nil, List.cons_append, List.nil_append, and_true] at h
  obtain ⟨h1, h2, h3, h3', h4, h5⟩ := h
  simp only [decide_eq_false_iff_not, ne_eq, Nat.not_le, Nat.not_lt, not_or, not_and,
    Classical.not_not] at h1 h2 h3 h3' h4
  refine ⟨h1.1, h1.2, ?_, h4, release_ok h5⟩
  by_cases ha : c = (cfg.recipe (s.record id).recipe).actor
  · exact Or.inl ha
  · exact Or.inr (h2 ha)

structure SettleOk (cfg : Cfg A) (s : State) (c : A) (id : Nat) (done : Bool) : Prop where
  inWin : s.firstId ≤ id
  found : id < s.records.length
  auth : c = (cfg.recipe (s.record id).recipe).executor
  inflight : inFlight (s.record id).status = true
  complete : done = true → CompleteOk cfg s id
  release : done = false → ReleaseOk cfg s id

theorem settle_ok {cfg : Cfg A} {s : State} {c : A} {id : Nat} {done : Bool}
    (h : outcome cfg s (.settle c id done) = none) : SettleOk cfg s c id done := by
  simp only [outcome, guards, settleGuards, foundGuard, firstFail_eq_none, noFire_append,
    noFire_cons, noFire_nil, List.cons_append, List.nil_append, and_true] at h
  obtain ⟨h1, h2, h3, h4, h5⟩ := h
  simp only [decide_eq_false_iff_not, ne_eq, Nat.not_le, Nat.not_lt, not_or, Classical.not_not,
    Bool.not_eq_false'] at h1 h2 h4
  refine ⟨h1.1, h1.2, h2, h4, fun hd => ?_, fun hd => ?_⟩
  · subst hd
    simp only [if_true, addGuards, boundsGuards, verGuard, foundGuard, noFire_append,
      noFire_cons, noFire_nil, and_true] at h5
    simp only [decide_eq_false_iff_not, Bool.not_eq_false', decide_eq_true_eq, ne_eq,
      Classical.not_not, Nat.not_le, Nat.not_lt, not_or, and_assoc] at h5
    obtain ⟨g1, g2, g3, g4, -, -, g5⟩ := h5
    exact ⟨g1, g2, g3, g4, g5⟩
  · subst hd
    simp only [Bool.false_eq_true, if_false] at h5
    exact release_ok h5

theorem pause_ok {cfg : Cfg A} {s : State} {c : A} {b : Bool}
    (h : outcome cfg s (.pause c b) = none) :
    c = cfg.owner ∧ s.version < VERSION_CAP ∧ s.version < U32_MAX := by
  guard_simp at h
  exact h

structure ObserveOk (cfg : Cfg A) (s : State) (c : A) (vals : List Int) : Prop where
  auth : c = cfg.owner
  /-- lib.rs 538: the values are restated, or no record is open -/
  pend : vals = s.values ∨ anyOpen s.records = false
  len : vals.length = s.values.length
  valsB : AllB vals
  energyOk : energyBad cfg vals = false
  cap : s.version < VERSION_CAP
  noOverflow : s.version < U32_MAX

theorem observe_ok {cfg : Cfg A} {s : State} {c : A} {vals : List Int} {now : Nat}
    (h : outcome cfg s (.observe c vals now) = none) : ObserveOk cfg s c vals := by
  guard_simp at h
  simp only [Bool.or_eq_false_iff, decide_eq_false_iff_not, Classical.not_not,
    Bool.not_eq_false', decide_eq_true_eq] at h
  obtain ⟨h1, h2, ⟨h3, h4⟩, h5, h6, h7⟩ := h
  refine ⟨h1, ?_, h3, h4, h5, h6, h7⟩
  by_cases hv : vals = s.values
  · exact Or.inl hv
  · exact Or.inr (h2 hv)

end Pool
