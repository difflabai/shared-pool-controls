/-
Shared lemmas: guard lists, vectors, sums over records, the envelope and the
ledger under each kind of record update.
-/
import PoolLean.Props

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-! ## Guard lists -/

/-- No guard of the list fires. -/
def NoFire : List Guard → Prop
  | [] => True
  | g :: gs => g.2 = false ∧ NoFire gs

@[simp] theorem noFire_nil : NoFire [] ↔ True := Iff.rfl
@[simp] theorem noFire_cons (g : Guard) (gs : List Guard) :
    NoFire (g :: gs) ↔ g.2 = false ∧ NoFire gs := Iff.rfl

@[simp] theorem noFire_append (a b : List Guard) : NoFire (a ++ b) ↔ NoFire a ∧ NoFire b := by
  induction a with
  | nil => simp
  | cons g a ih => simp [ih, and_assoc]

theorem firstFail_eq_none (gs : List Guard) : firstFail gs = none ↔ NoFire gs := by
  induction gs with
  | nil => simp [firstFail]
  | cons g gs ih => obtain ⟨o, b⟩ := g; cases b <;> simp [firstFail, ih]

/-- A list in which some guard fires stops the call. -/
theorem firstFail_ne_none {gs : List Guard} {g : Guard} (hg : g ∈ gs) (hb : g.2 = true) :
    firstFail gs ≠ none := by
  rw [Ne, firstFail_eq_none]
  intro h
  induction gs with
  | nil => simp at hg
  | cons g' gs ih =>
    rcases List.mem_cons.mp hg with rfl | hg
    · simp [hb] at h
    · exact ih hg h.2

/-! ## exec -/

theorem exec_none {cfg : Cfg A} {s : State} {op : Op A} (h : outcome cfg s op = none) :
    exec cfg s op = (okRes s op, { effect cfg s op with eff := s.eff + 1 }) := by
  simp [exec, h]

theorem exec_err {cfg : Cfg A} {s : State} {op : Op A} {e : Err}
    (h : outcome cfg s op = some (.err e)) : exec cfg s op = (.err e, s) := by
  simp [exec, h]

theorem exec_noop {cfg : Cfg A} {s : State} {op : Op A} (h : outcome cfg s op = some .noop) :
    exec cfg s op = (.ok, s) := by
  simp [exec, h]

/-- Every call either leaves the state alone or applies `effect`. -/
theorem exec_cases (cfg : Cfg A) (s : State) (op : Op A) :
    (exec cfg s op).2 = s ∨
      (outcome cfg s op = none ∧ exec cfg s op = (okRes s op, { effect cfg s op with eff := s.eff + 1 })) := by
  cases h : outcome cfg s op with
  | none => exact Or.inr ⟨rfl, exec_none h⟩
  | some o =>
    cases o with
    | err e => left; rw [exec_err h]
    | noop => left; rw [exec_noop h]

/-! ## Vectors -/

theorem get_eq {v : List Int} {i : Nat} (h : i < v.length) : get v i = v[i] := by
  simp [get, h]

theorem get_of_le {v : List Int} {i : Nat} (h : v.length ≤ i) : get v i = 0 := by
  simp [get, List.getElem?_eq_none h]

theorem length_addv (b d : List Int) (u : Nat) : (addv b d u).length = min b.length d.length := by
  simp [addv]

theorem get_addv {b d : List Int} {u i : Nat} (hb : i < b.length) (hd : i < d.length) :
    get (addv b d u) i = get b i + get d i * (u : Int) := by
  have h : i < (addv b d u).length := by rw [length_addv]; omega
  rw [get_eq h, get_eq hb, get_eq hd]
  simp [addv]

theorem get_neg (d : List Int) (i : Nat) : get (neg d) i = - get d i := by
  by_cases h : i < d.length
  · have h' : i < (neg d).length := by simpa [neg] using h
    rw [get_eq h', get_eq h]; simp [neg]
  · have h' : (neg d).length ≤ i := by simp [neg]; omega
    rw [get_of_le h', get_of_le (by omega)]; simp

theorem length_neg (d : List Int) : (neg d).length = d.length := by simp [neg]

theorem allB_get {v : List Int} (h : AllB v) {i : Nat} (hi : i < v.length) : Bounded (get v i) := by
  rw [get_eq hi]; exact h _ (List.getElem_mem hi)

theorem allB_of_get {v : List Int} (h : ∀ i < v.length, Bounded (get v i)) : AllB v := by
  intro x hx
  obtain ⟨i, hi, rfl⟩ := List.mem_iff_getElem.mp hx
  have := h i hi
  rwa [get_eq hi] at this

theorem anyLt_eq_false {n : Nat} {p : Nat → Bool} : anyLt n p = false ↔ ∀ i < n, p i = false := by
  simp [anyLt, List.any_eq_false, List.mem_range]

theorem anyLt_eq_true {n : Nat} {p : Nat → Bool} : anyLt n p = true ↔ ∃ i < n, p i = true := by
  simp [anyLt, List.any_eq_true, List.mem_range]

/-! ## Sums -/

section sums
variable {α : Type}

@[simp] theorem sumI_nil (f : α → Int) : sumI f [] = 0 := rfl
@[simp] theorem sumI_cons (f : α → Int) (a : α) (l : List α) : sumI f (a :: l) = f a + sumI f l := rfl

theorem sumI_append (f : α → Int) (l₁ l₂ : List α) : sumI f (l₁ ++ l₂) = sumI f l₁ + sumI f l₂ := by
  induction l₁ with
  | nil => simp
  | cons a l ih => simp [ih]; omega

theorem sumI_set (f : α → Int) (l : List α) (k : Nat) (x : α) (hk : k < l.length) :
    sumI f (l.set k x) = sumI f l - f l[k] + f x := by
  induction l generalizing k with
  | nil => simp at hk
  | cons a l ih =>
    cases k with
    | zero => simp; omega
    | succ k =>
      simp only [List.set_cons_succ, sumI_cons, List.getElem_cons_succ]
      rw [ih k (by simpa using hk)]; omega

theorem sumI_le {f g : α → Int} {l : List α} (h : ∀ x ∈ l, f x ≤ g x) : sumI f l ≤ sumI g l := by
  induction l with
  | nil => simp
  | cons a l ih =>
    simp only [sumI_cons]
    have h1 := h a (by simp)
    have h2 := ih (fun x hx => h x (by simp [hx]))
    omega

theorem sumI_zero (l : List α) : sumI (fun _ => (0 : Int)) l = 0 := by
  induction l with
  | nil => rfl
  | cons a l ih => simp [ih]

theorem sumI_nonpos {f : α → Int} {l : List α} (h : ∀ x ∈ l, f x ≤ 0) : sumI f l ≤ 0 := by
  have := sumI_le (g := fun _ => 0) h
  rw [sumI_zero] at this; exact this

theorem sumI_nonneg {f : α → Int} {l : List α} (h : ∀ x ∈ l, 0 ≤ f x) : 0 ≤ sumI f l := by
  have := sumI_le (f := fun _ => 0) h
  rw [sumI_zero] at this; exact this

theorem sumI_congr {f g : α → Int} {l : List α} (h : ∀ x ∈ l, f x = g x) : sumI f l = sumI g l :=
  Int.le_antisymm (sumI_le fun x hx => Int.le_of_eq (h x hx))
    (sumI_le fun x hx => Int.le_of_eq (h x hx).symm)

theorem sumI_sub (f g : α → Int) (l : List α) : sumI (fun x => f x - g x) l = sumI f l - sumI g l := by
  induction l with
  | nil => simp
  | cons a l ih => simp [ih]; omega

@[simp] theorem sumN_nil (f : α → Nat) : sumN f [] = 0 := rfl
@[simp] theorem sumN_cons (f : α → Nat) (a : α) (l : List α) : sumN f (a :: l) = f a + sumN f l := rfl

theorem sumN_set (f : α → Nat) (l : List α) (k : Nat) (x : α) (hk : k < l.length) :
    sumN f (l.set k x) + f l[k] = sumN f l + f x := by
  induction l generalizing k with
  | nil => simp at hk
  | cons a l ih =>
    cases k with
    | zero => simp; omega
    | succ k =>
      simp only [List.set_cons_succ, sumN_cons, List.getElem_cons_succ]
      have := ih k (by simpa using hk); omega

theorem sumN_append (f : α → Nat) (l₁ l₂ : List α) : sumN f (l₁ ++ l₂) = sumN f l₁ + sumN f l₂ := by
  induction l₁ with
  | nil => simp
  | cons a l ih => simp [ih]; omega

theorem sumN_le_mul {f : α → Nat} {c : Nat} {l : List α} (h : ∀ x ∈ l, f x ≤ c) :
    sumN f l ≤ c * l.length := by
  induction l with
  | nil => simp
  | cons a l ih =>
    simp only [sumN_cons, List.length_cons, Nat.mul_succ]
    have := h a (by simp); have := ih (fun x hx => h x (by simp [hx])); omega

theorem sumN_eq_zero {f : α → Nat} {l : List α} (h : ∀ x ∈ l, f x = 0) : sumN f l = 0 := by
  have := sumN_le_mul (c := 0) (fun x hx => Nat.le_of_eq (h x hx)); omega

theorem le_sumN {f : α → Nat} {l : List α} {x : α} (hx : x ∈ l) : f x ≤ sumN f l := by
  induction l with
  | nil => simp at hx
  | cons a l ih =>
    simp only [sumN_cons]
    rcases List.mem_cons.mp hx with rfl | hx
    · omega
    · have := ih hx; omega

/-- Updating an entry that counts zero never lowers a sum. -/
theorem sumN_set_ge (f : α → Nat) (l : List α) (k : Nat) (x : α)
    (h0 : ∀ h : k < l.length, f l[k] = 0) : sumN f l ≤ sumN f (l.set k x) := by
  by_cases hk : k < l.length
  · have := sumN_set f l k x hk; have := h0 hk; omega
  · rw [List.set_eq_of_length_le (by omega)]; exact Nat.le_refl _

end sums

/-- With every record at its own index (`ids`), the only record whose id is
`k` is `l[k]`. -/
theorem sumI_id (g : Rec → Int) (l : List Rec) (off k : Nat)
    (hid : ∀ j (h : j < l.length), l[j].id = off + j) (hk1 : off ≤ k) (hk2 : k < off + l.length) :
    sumI (fun c => if c.id = k then g c else 0) l = g (l[k - off]'(by omega)) := by
  induction l generalizing off with
  | nil => simp at hk2; omega
  | cons a l ih =>
    simp only [sumI_cons]
    have ha : a.id = off := by
      have := hid 0 (by simp); rw [List.getElem_cons_zero] at this; omega
    by_cases hko : k = off
    · subst hko
      have hrest : sumI (fun c => if c.id = k then g c else 0) l = 0 := by
        apply Eq.trans (sumI_congr (g := fun _ => 0) _) (sumI_zero l)
        · intro x hx
          obtain ⟨j, hj, rfl⟩ := List.mem_iff_getElem.mp hx
          have := hid (j + 1) (by simp; omega)
          simp at this
          simp [this]
      simp [ha, hrest]
    · have h1 : (if a.id = k then g a else 0) = 0 := by simp [ha]; omega
      rw [h1, ih (off + 1) (fun j hj => by have := hid (j + 1) (by simp; omega); simp at this; omega)
        (by omega) (by simp at hk2; omega)]
      have : k - off = (k - (off + 1)) + 1 := by omega
      simp [this]

/-! ## Records -/

theorem getD_rec {l : List Rec} {k : Nat} (hk : k < l.length) : l.getD k default = l[k] := by
  simp [hk]

theorem record_eq {s : State} {k : Nat} (hk : k < s.records.length) : s.record k = s.records[k] := by
  simp [State.record, hk]

theorem length_setStatus (l : List Rec) (k : Nat) (st : Status) : (setStatus l k st).length = l.length := by
  simp [setStatus]

theorem getElem_setStatus {l : List Rec} {k : Nat} (hk : k < l.length) (st : Status) (j : Nat)
    (hj : j < (setStatus l k st).length) :
    (setStatus l k st)[j] = if k = j then { l[k] with status := st } else l[j]'(by simpa [length_setStatus] using hj) := by
  simp only [setStatus, List.getElem_set, getD_rec hk]

/-- `sumI` over a status update of record `k`. -/
theorem sumI_setStatus (f : Rec → Int) {l : List Rec} {k : Nat} (hk : k < l.length) (st : Status) :
    sumI f (setStatus l k st) = sumI f l - f l[k] + f { l[k] with status := st } := by
  simp only [setStatus, getD_rec hk]; exact sumI_set f l k _ hk

/-! ## The envelope -/

theorem envS_append_new (t : Nat → Nat → Nat → Int) (recs : List Rec) (r u exp i : Nat) :
    envS t (recs ++ [⟨recs.length, r, u, exp, .reserved⟩]) none i = envS t recs none i + t r u i := by
  simp [envS, sumI_append, inEnv, Status.isOpen]

/-- Removing record `k` from the envelope (the code's `skip`). -/
theorem envS_split (t : Nat → Nat → Nat → Int) {recs : List Rec} {k : Nat} (hk : k < recs.length)
    (hid : ∀ j (h : j < recs.length), recs[j].id = j) (i : Nat) :
    envS t recs none i = envS t recs (some k) i +
      (if recs[k].status.isOpen then t recs[k].recipe recs[k].units i else 0) := by
  let T : Rec → Int := fun c => if c.status.isOpen then t c.recipe c.units i else 0
  have h1 : envS t recs (some k) i =
      sumI (fun c => T c - (if c.id = k then T c else 0)) recs := by
    apply sumI_congr; intro c _
    by_cases h : c.id = k
    · simp [T, inEnv, h]
    · have h' : k ≠ c.id := fun e => h e.symm
      simp [T, inEnv, h, h']
  have h2 : envS t recs none i = sumI T recs := by
    apply sumI_congr; intro c _; simp [T, inEnv]
  have h3 := sumI_id T recs 0 k (by simpa using hid) (by omega) (by omega)
  rw [h1, h2, sumI_sub, h3]
  simp [T]

/-- Closing record `k` (completed or cancelled) is skipping it. -/
theorem envS_close (t : Nat → Nat → Nat → Int) {recs : List Rec} {k : Nat} (hk : k < recs.length)
    (hid : ∀ j (h : j < recs.length), recs[j].id = j) {st : Status} (hst : st.isOpen = false)
    (i : Nat) : envS t (setStatus recs k st) none i = envS t recs (some k) i := by
  have := envS_split t hk hid i
  simp only [envS] at this ⊢
  rw [sumI_setStatus _ hk]
  simp [inEnv, hst] at this ⊢
  omega

/-- Moving record `k` between open statuses leaves the envelope alone. -/
theorem envS_open (t : Nat → Nat → Nat → Int) {recs : List Rec} {k : Nat} (hk : k < recs.length)
    {st : Status} (h1 : recs[k].status.isOpen = true) (h2 : st.isOpen = true) (i : Nat) :
    envS t (setStatus recs k st) none i = envS t recs none i := by
  simp only [envS]
  rw [sumI_setStatus _ hk]
  simp [inEnv, h1, h2]

/-- With no record open, the envelope is empty. -/
theorem envS_closed (t : Nat → Nat → Nat → Int) {recs : List Rec} (h : anyOpen recs = false)
    (skip : Option Nat) (i : Nat) : envS t recs skip i = 0 := by
  simp only [anyOpen, List.any_eq_false, Bool.not_eq_true] at h
  simp only [envS]
  rw [sumI_congr (g := fun _ => 0) (fun c hc => by simp [inEnv, h c hc]), sumI_zero]

theorem loT_nonpos (cfg : Cfg A) (r u i : Nat) : loT cfg r u i ≤ 0 := by
  simp only [loT]; omega

theorem hiT_nonneg (cfg : Cfg A) (r u i : Nat) : 0 ≤ hiT cfg r u i := by
  simp only [hiT]; omega

theorem envS_lo_nonpos (cfg : Cfg A) (recs : List Rec) (skip : Option Nat) (i : Nat) :
    envS (loT cfg) recs skip i ≤ 0 :=
  sumI_nonpos fun c _ => by have := loT_nonpos cfg c.recipe c.units i; split <;> omega

theorem envS_hi_nonneg (cfg : Cfg A) (recs : List Rec) (skip : Option Nat) (i : Nat) :
    0 ≤ envS (hiT cfg) recs skip i :=
  sumI_nonneg fun c _ => by have := hiT_nonneg cfg c.recipe c.units i; split <;> omega

theorem extraT_lo_nonpos (cfg : Cfg A) (e : Option (Nat × Nat)) (i : Nat) :
    extraT (loT cfg) e i ≤ 0 := by
  cases e with
  | none => simp [extraT]
  | some p => obtain ⟨r, u⟩ := p; exact loT_nonpos cfg r u i

theorem extraT_hi_nonneg (cfg : Cfg A) (e : Option (Nat × Nat)) (i : Nat) :
    0 ≤ extraT (hiT cfg) e i := by
  cases e with
  | none => simp [extraT]
  | some p => obtain ⟨r, u⟩ := p; exact hiT_nonneg cfg r u i

theorem low_le (cfg : Cfg A) (recs : List Rec) (v : List Int) (e : Option (Nat × Nat))
    (skip : Option Nat) (i : Nat) : low cfg recs v e skip i ≤ get v i := by
  have := envS_lo_nonpos cfg recs skip i
  have := extraT_lo_nonpos cfg e i
  simp only [low]; omega

theorem le_high (cfg : Cfg A) (recs : List Rec) (v : List Int) (e : Option (Nat × Nat))
    (skip : Option Nat) (i : Nat) : get v i ≤ high cfg recs v e skip i := by
  have := envS_hi_nonneg cfg recs skip i
  have := extraT_hi_nonneg cfg e i
  simp only [high]; omega

/-- PV ≤ EV: the envelope contains the candidate. -/
theorem viol_le_ev (cfg : Cfg A) (recs : List Rec) (v : List Int) (e : Option (Nat × Nat))
    (skip : Option Nat) (i : Nat) : viol cfg v i ≤ ev cfg recs v e skip i := by
  have := low_le cfg recs v e skip i
  have := le_high cfg recs v e skip i
  simp only [viol, ev]; omega

theorem viol_nonneg (cfg : Cfg A) (v : List Int) (i : Nat) : 0 ≤ viol cfg v i := by
  simp only [viol]; omega

/-- `envFail` and `ev` read the envelope only through `low` and `high`. -/
theorem envFail_congr {cfg : Cfg A} {r1 r2 : List Rec} {v : List Int} {e1 e2 : Option (Nat × Nat)}
    {k1 k2 : Option Nat} (hl : ∀ i, low cfg r1 v e1 k1 i = low cfg r2 v e2 k2 i)
    (hh : ∀ i, high cfg r1 v e1 k1 i = high cfg r2 v e2 k2 i) :
    envFail cfg r1 v e1 k1 = envFail cfg r2 v e2 k2 := by
  simp only [envFail, hl, hh]

theorem ev_congr {cfg : Cfg A} {r1 r2 : List Rec} {v : List Int} {e1 e2 : Option (Nat × Nat)}
    {k1 k2 : Option Nat} (hl : ∀ i, low cfg r1 v e1 k1 i = low cfg r2 v e2 k2 i)
    (hh : ∀ i, high cfg r1 v e1 k1 i = high cfg r2 v e2 k2 i) (i : Nat) :
    ev cfg r1 v e1 k1 i = ev cfg r2 v e2 k2 i := by
  simp only [ev, hl, hh]

theorem envFail_false {cfg : Cfg A} {recs : List Rec} {v : List Int} {e : Option (Nat × Nat)}
    {k : Option Nat} : envFail cfg recs v e k = false ↔
      ∀ i < v.length, Bounded (low cfg recs v e k i) ∧ Bounded (high cfg recs v e k i) := by
  simp [envFail, anyLt_eq_false]

/-- `check_bounds` passes: on every metric the envelope's violation is at
most both the current values' and the candidate's. -/
theorem boundsFail_false {cfg : Cfg A} {s : State} {v : List Int} {e : Option (Nat × Nat)}
    {k : Option Nat} : boundsFail cfg s v e k = false ↔
      ∀ i < cfg.n, ev cfg s.records v e k i ≤ viol cfg s.values i ∧
        ev cfg s.records v e k i ≤ viol cfg v i := by
  simp only [boundsFail, anyLt_eq_false, decide_eq_false_iff_not, Int.not_lt]
  constructor
  · intro h i hi; have := h i hi; omega
  · intro h i hi; have := h i hi; omega

/-! ## The sub-box (`res_sub`'s arithmetic) -/

/-- One metric of `res_sub`. The pre-state's envelope is `[v + L + lk, v + H
+ hk]`: `L ≤ 0 ≤ H` the other open records' terms and `lk = min(d, x, 0) ≤ 0
≤ hk = max(d, x, 0)` record `k`'s. Closing `k` with delta `t ∈ {d, x}` (so
`lk ≤ t ≤ hk`) moves the values to `v + t` and the envelope to `[v + t + L,
v + t + H]`. If the pre-state's envelope violation equals the values' own
(INV-RES), the post envelope lies inside the pre one, and its violation is
at most both the old and the new values' violation. -/
theorem sub_box {m M v L H lk hk t : Int} (hL : L ≤ 0) (hH : 0 ≤ H) (hlk : lk ≤ t)
    (hhk : t ≤ hk) (hlk0 : lk ≤ 0) (hhk0 : 0 ≤ hk)
    (hres : max 0 (m - (v + L + lk)) + max 0 (v + H + hk - M) = max 0 (m - v) + max 0 (v - M)) :
    v + L + lk ≤ v + t + L ∧ v + t + H ≤ v + H + hk ∧
    max 0 (m - (v + t + L)) + max 0 (v + t + H - M) ≤ max 0 (m - v) + max 0 (v - M) ∧
    max 0 (m - (v + t + L)) + max 0 (v + t + H - M) ≤ max 0 (m - (v + t)) + max 0 (v + t - M) := by
  omega

/-! ## The ledger -/

theorem ledger_append_new (cfg : Cfg A) (recs : List Rec) (k0 r u exp i : Nat)
    (hk : k0 ≤ recs.length) :
    ledger cfg (recs ++ [⟨recs.length, r, u, exp, .reserved⟩]) k0 i =
      ledger cfg recs k0 i + get (cfg.recipe r).reserve i * (u : Int) := by
  simp [ledger, sumI_append, contrib, hk]

theorem ledger_setStatus (cfg : Cfg A) {recs : List Rec} {k : Nat} (hk : k < recs.length)
    (st : Status) (k0 i : Nat) (hk0 : k0 ≤ recs[k].id) :
    ledger cfg (setStatus recs k st) k0 i =
      ledger cfg recs k0 i - contrib cfg i recs[k] + contrib cfg i { recs[k] with status := st } := by
  simp only [ledger]
  rw [sumI_setStatus _ hk]
  simp [hk0]

/-! ## Prerequisites -/

theorem doneUnits_append (l : List Rec) (c : Rec) (p : Nat) :
    doneUnits (l ++ [c]) p = doneUnits l p + doneUnits [c] p := by
  simp only [doneUnits]
  induction l with
  | nil => simp
  | cons a l ih => simp [ih]; omega

/-- Completing a record never lowers a completed count; any other update
of a record that is not completed leaves it as large. -/
theorem doneUnits_setStatus_ge {l : List Rec} {k : Nat} (hk : k < l.length)
    (hnc : l[k].status ≠ .completed) (st : Status) (j p : Nat) :
    doneUnits (l.take j) p ≤ doneUnits ((setStatus l k st).take j) p := by
  simp only [doneUnits, setStatus, List.take_set, getD_rec hk]
  apply sumN_set_ge
  intro h
  simp [doneF, List.getElem_take, hnc]

end Pool
