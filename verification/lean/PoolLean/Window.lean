/-
The record window (v1.1.0, F-3) and the config check (F-4), at every size:

* WIN-1 (`win1`): at most `MAX_RECORDS` records retained, their ids
  `firstId, firstId + 1, ...`, and the next id `firstId + len` at most the
  version (so the id's `checked_add`, lib.rs 363-366, never fails).
* WIN-2 (`pruned_terminal`, `pruned_notFound`): a pruned record is
  terminal, and every call naming its id is `NotFound`.
* The guards read only the window (`envS_win`, `anyOpen_win`): the
  history encoding computes the code's quantities.
* WIN-ESC (`full_iff_front_open`, `submit_not_full_of_front_terminal`,
  `win_esc`): `Full` fires only with 256 retained and the front open, and
  one `Ok` call (LIVE-ALL) resolves the front, after which the guard
  passes. `submit_prunes`: a submit at 256 retained prunes the oldest.
* AUTH-SELF / CFG-2 (`valid_actor_ne_executor`, `valid_iff`): `validate`
  decides `Valid`, which rejects a recipe whose actor is its executor.
-/
import PoolLean.Calls

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

section window

variable {cfg : Cfg A} {s : State}

theorem win_length (s : State) : s.win.length = s.records.length - s.firstId := by
  simp [State.win]

/-- WIN-1. -/
theorem win1 (I : Inv cfg s) :
    s.win.length ≤ MAX_RECORDS ∧
      (∀ k (h : k < s.win.length), s.win[k].id = s.firstId + k) ∧
      s.firstId + s.win.length = s.records.length ∧ s.records.length ≤ s.version := by
  have W := I.wf
  refine ⟨by rw [win_length]; exact W.win, fun k h => ?_, by rw [win_length]; have := W.fle; omega,
    W.lenVer⟩
  rw [win_length] at h
  simp only [State.win, List.getElem_drop]
  exact (W.recs _ (by omega)).id

/-- WIN-2: pruned records are terminal. -/
theorem pruned_terminal (I : Inv cfg s) {j : Nat} (hj : j < s.firstId) (h : j < s.records.length) :
    s.records[j].status.isOpen = false :=
  (I.wf.recs j h).pruned hj

/-- WIN-2: every call naming a pruned id is `NotFound` (from every state). -/
theorem pruned_notFound (cfg : Cfg A) (s : State) {j : Nat} (hj : j < s.firstId) (c : A)
    (now : Nat) (b : Bool) :
    outcome cfg s (.dispatch c j now) = some (.err .notFound) ∧
      outcome cfg s (.ack j b) = some (.err .notFound) ∧
      outcome cfg s (.cancel c j now) = some (.err .notFound) ∧
      outcome cfg s (.settle c j b) = some (.err .notFound) := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;>
    simp [outcome, guards, dispatchGuards, ackGuards, cancelGuards, settleGuards, foundGuard,
      firstFail, hj]

theorem sumI_take_closed (W : WF cfg s) (f : Rec → Int) (hf : ∀ c : Rec, c.status.isOpen = false → f c = 0) :
    sumI f (s.records.take s.firstId) = 0 := by
  apply Eq.trans (sumI_congr _) (sumI_zero _)
  intro x hx
  obtain ⟨j, hj, rfl⟩ := List.mem_iff_getElem.mp hx
  rw [List.length_take] at hj
  simp only [List.getElem_take]
  exact hf _ ((W.recs j (by omega)).pruned (by omega))

/-- The envelope over the history is the envelope over the window. -/
theorem envS_win (W : WF cfg s) (t : Nat → Nat → Nat → Int) (skip : Option Nat) (i : Nat) :
    envS t s.records skip i = envS t s.win skip i := by
  have h := sumI_append (fun c => if inEnv skip c then t c.recipe c.units i else 0)
    (s.records.take s.firstId) (s.records.drop s.firstId)
  rw [List.take_append_drop] at h
  simp only [envS, State.win]
  rw [h, sumI_take_closed W _ (fun c hc => by simp [inEnv, hc])]
  omega

/-- observe's "some record is open" over the history is the window's. -/
theorem anyOpen_win (W : WF cfg s) : anyOpen s.records = anyOpen s.win := by
  have h : s.records = s.records.take s.firstId ++ s.win := (List.take_append_drop _ _).symm
  have ht : anyOpen (s.records.take s.firstId) = false := by
    simp only [anyOpen, List.any_eq_false, Bool.not_eq_true]
    intro x hx
    obtain ⟨j, hj, rfl⟩ := List.mem_iff_getElem.mp hx
    rw [List.length_take] at hj
    simp only [List.getElem_take]
    exact (W.recs j (by omega)).pruned (by omega)
  conv => lhs; rw [h]
  simp only [anyOpen, List.any_append] at ht ⊢
  rw [ht]; simp

/-- WIN-ESC: the Full guard fires iff 256 records are retained and the
oldest one is open. -/
theorem full_iff_front_open (I : Inv cfg s) :
    fullFires s = true ↔ s.win.length = MAX_RECORDS ∧
      ∃ h : s.firstId < s.records.length, s.records[s.firstId].status.isOpen = true := by
  have W := I.wf
  have hw := W.win
  rw [win_length]
  simp only [fullFires, Bool.and_eq_true, decide_eq_true_eq]
  constructor
  · rintro ⟨h1, h2⟩
    have hlt : s.firstId < s.records.length := by simp only [MAX_RECORDS] at h1; omega
    rw [record_eq hlt] at h2
    exact ⟨by omega, hlt, h2⟩
  · rintro ⟨h1, hlt, h2⟩
    rw [record_eq hlt]
    exact ⟨by omega, h2⟩

/-- WIN-ESC: with the oldest retained record terminal (or none retained),
the Full guard passes (from every state). -/
theorem submit_not_full_of_front_terminal (s : State)
    (h : ∀ h : s.firstId < s.records.length, s.records[s.firstId].status.isOpen = false) :
    fullFires s = false := by
  simp only [fullFires, Bool.and_eq_false_iff, decide_eq_false_iff_not]
  by_cases hlt : s.firstId < s.records.length
  · right; rw [record_eq hlt]; exact h hlt
  · left; simp only [MAX_RECORDS]; omega

/-- WIN-ESC: from any reachable state in which `submit` would be `Full`, one
`Ok` call resolves the oldest record (its actor or the owner cancels a
Reserved one, its executor settles an in-flight one), and after it the
Full guard passes. -/
theorem win_esc (hV : Valid cfg) (h : Reach cfg s) (hf : fullFires s = true) :
    ∃ op : Op A, outcome cfg s op = none ∧ (step cfg s op).firstId = s.firstId ∧
      fullFires (step cfg s op) = false := by
  have I := reach_inv hV h
  obtain ⟨-, hlt, ho⟩ := (full_iff_front_open I).mp hf
  have L := live_all hV I hlt
  have closes : ∀ op : Op A, outcome cfg s op = none → ∀ st : Status, st.isOpen = false →
      (effect cfg s op).records = setStatus s.records s.firstId st →
      (effect cfg s op).firstId = s.firstId →
      (step cfg s op).firstId = s.firstId ∧ fullFires (step cfg s op) = false := by
    intro op hop st hst he hfid
    simp only [step, exec_none hop]
    refine ⟨hfid, submit_not_full_of_front_terminal _ fun h' => ?_⟩
    have h'' : s.firstId < (setStatus s.records s.firstId st).length := by
      rw [length_setStatus]; exact hlt
    show ((effect cfg s op).records[(effect cfg s op).firstId]'h').status.isOpen = false
    simp only [he, hfid]
    rw [getElem_setStatus hlt]; simp [hst]
  by_cases hres : s.records[s.firstId].status = .reserved
  · have hc := (L.1 hres 0).2
    exact ⟨_, hc, closes _ hc .cancelled rfl rfl rfl⟩
  · have hfl : inFlight s.records[s.firstId].status = true := by
      revert hres ho; cases s.records[s.firstId].status <;> simp [inFlight, Status.isOpen]
    have hc := (L.2 hfl).1
    exact ⟨_, hc, closes _ hc .completed rfl rfl rfl⟩

/-- WIN-1 step: a submit accepted with 256 records retained prunes exactly
the oldest: `firstId` grows by one, 256 stay retained, and the new record's
id is `first_id + 256`. -/
theorem submit_prunes (I : Inv cfg s) {c : A} {r u ver exp now : Nat}
    (h : outcome cfg s (.submit c r u ver exp now) = none) (hfull : s.win.length = MAX_RECORDS) :
    (exec cfg s (.submit c r u ver exp now)).1 = .okId (s.firstId + MAX_RECORDS) ∧
      (step cfg s (.submit c r u ver exp now)).firstId = s.firstId + 1 ∧
      (step cfg s (.submit c r u ver exp now)).win.length = MAX_RECORDS := by
  have W := I.wf
  rw [win_length] at hfull
  have hle : MAX_RECORDS ≤ s.records.length - s.firstId := by omega
  simp only [step, exec_none h, okRes]
  refine ⟨by simp only [MAX_RECORDS] at hfull ⊢; congr 1; have := W.fle; omega, ?_, ?_⟩
  · show (if MAX_RECORDS ≤ s.records.length - s.firstId then s.firstId + 1 else s.firstId) = _
    simp [hle]
  · rw [win_length]
    show (s.records ++ _).length - (if MAX_RECORDS ≤ s.records.length - s.firstId then s.firstId + 1 else s.firstId) = _
    simp only [hle, if_true, List.length_append, List.length_singleton]
    omega

end window

/-! ## AUTH-SELF and `validate` -/

/-- AUTH-SELF (CFG-2, v1.1.0 lib.rs 130): no valid recipe's actor is its
executor. (The owner may be an executor.) -/
theorem valid_actor_ne_executor {cfg : Cfg A} (hV : Valid cfg) {k : Nat}
    (hk : k < cfg.recipes.length) : (cfg.recipe k).actor ≠ (cfg.recipe k).executor :=
  (recipe_ok hV hk).actorNe

theorem recipeOkB_iff (cfg : Cfg A) (k : Nat) (r : Recipe A) :
    recipeOkB cfg k r = true ↔ RecipeOk cfg k r := by
  simp only [recipeOkB, Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true]
  constructor
  · rintro ⟨⟨⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩, h9⟩, h10⟩
    exact ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9, h10⟩
  · rintro ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9, h10⟩
    exact ⟨⟨⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩, h9⟩, h10⟩

/-- CFG-2: `validate` (the replay's test of `Pool::new`) decides `Valid`. -/
theorem valid_iff (cfg : Cfg A) : validate cfg = true ↔ Valid cfg := by
  have hrec : (∀ k ∈ List.range cfg.recipes.length, recipeOkB cfg k (cfg.recipe k) = true) ↔
      ∀ k (h : k < cfg.recipes.length), RecipeOk cfg k cfg.recipes[k] := by
    constructor
    · intro h k hk
      have := (recipeOkB_iff cfg k _).mp (h k (List.mem_range.mpr hk))
      have e : cfg.recipe k = cfg.recipes[k] := by simp [Cfg.recipe, hk]
      rwa [e] at this
    · intro h k hk
      have hk' := List.mem_range.mp hk
      have e : cfg.recipe k = cfg.recipes[k] := by simp [Cfg.recipe, hk']
      rw [e]; exact (recipeOkB_iff cfg k _).mpr (h k hk')
  simp only [validate, Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true]
  rw [hrec]
  constructor
  · rintro ⟨⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩, h9⟩
    exact ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9⟩
  · rintro ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9⟩
    exact ⟨⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩, h9⟩

end Pool
