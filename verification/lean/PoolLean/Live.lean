/-
LEAN-LIVE-ALL (LIVE-ALL) and LEAN-LIVE1 at every size, with no premise
beyond `Valid` and `Inv`. In every state satisfying `Inv` (so in every
reachable state), a Reserved record can be cancelled by its actor and by
the owner, and a Dispatched/Active/Unknown record can be settled by its
executor either way: each of these calls passes every guard.

The proof is `res_sub`: closing an open record moves the values by `t`
(its completion `d` or its release `x`), and `min(d, x, 0) ≤ t ≤ max(d, x,
0)`, so the post-call envelope is a sub-box of the current one; INV-RES
(`EV = PV(values)`) then bounds the post-call violation by both the old
and the new values' violation, which is `check_bounds`' test. INV-VERCAP
leaves room for the plain bump. v1.0.0 needed `B = 0` and `version <
u32::MAX` (F-1, F-5); `regression/Live1NoPremise.lean` is that canary,
which compiles now.

Also INV-OUTCOMES (`healthy`): with B = 0, every combination of outcomes
of the open records (each completed, released, or left open) lands inside
[min, max].
-/
import PoolLean.StepProps

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

theorem metric_bounded {cfg : Cfg A} (hV : Valid cfg) {i : Nat} (hi : i < cfg.n) :
    Bounded (cfg.metric i).min ∧ Bounded (cfg.metric i).max := by
  have hm : cfg.metric i = cfg.metrics[i] := by simp [Cfg.metric, Cfg.n] at hi ⊢; simp [hi]
  obtain ⟨h1, h2, -⟩ := hV.metric _ (List.getElem_mem hi)
  rw [hm]; exact ⟨h1, h2⟩

theorem recipe_ok {cfg : Cfg A} (hV : Valid cfg) {k : Nat} (hk : k < cfg.recipes.length) :
    RecipeOk cfg k (cfg.recipe k) := by
  have : cfg.recipe k = cfg.recipes[k] := by simp [Cfg.recipe, hk]
  rw [this]; exact hV.recipe k hk

/-! ## The sub-box -/

section box

variable {cfg : Cfg A} {s : State}

/-- `res_sub`: closing the open record `k` with per-metric delta `t` (between
its low and high hull terms) gives new values `v` whose envelope without
`k` lies inside the current envelope; its endpoints are bounded, and its
violation is at most both the current values' and `v`'s. -/
theorem res_sub (I : Inv cfg s) {k : Nat} (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) {v : List Int} {t : Nat → Int}
    (hv : ∀ i < cfg.n, get v i = get s.values i + t i)
    (ht : ∀ i, loT cfg s.records[k].recipe s.records[k].units i ≤ t i ∧
      t i ≤ hiT cfg s.records[k].recipe s.records[k].units i) :
    ∀ i < cfg.n,
      low cfg s.records s.values none none i ≤ low cfg s.records v none (some k) i ∧
      high cfg s.records v none (some k) i ≤ high cfg s.records s.values none none i ∧
      low cfg s.records v none (some k) i ≤ get v i ∧ get v i ≤ high cfg s.records v none (some k) i ∧
      ev cfg s.records v none (some k) i ≤ viol cfg s.values i ∧
      ev cfg s.records v none (some k) i ≤ viol cfg v i := by
  intro i hi
  have hid : ∀ j (h : j < s.records.length), s.records[j].id = j := fun j h => (I.wf.recs j h).id
  have sl := envS_split (loT cfg) hk hid i
  have sh := envS_split (hiT cfg) hk hid i
  simp only [ho, ite_true] at sl sh
  have e1 := envS_lo_nonpos cfg s.records (some k) i
  have e2 := envS_hi_nonneg cfg s.records (some k) i
  have l0 := loT_nonpos cfg s.records[k].recipe s.records[k].units i
  have h0 := hiT_nonneg cfg s.records[k].recipe s.records[k].units i
  obtain ⟨t1, t2⟩ := ht i
  have hres := I.res.2 i hi
  have B := sub_box (m := (cfg.metric i).min) (M := (cfg.metric i).max) e1 e2 t1 t2 l0 h0
    (by simp only [ev, low, high, extraT, viol, sl, sh] at hres; rw [← hres]; congr 1 <;> congr 1 <;> omega)
  simp only [ev, low, high, extraT, viol, sl, sh, hv i hi]
  omega

/-- The guards `release`/`settle(.., true)` share after the status checks
all pass when closing an open record `k` with an in-hull delta. -/
theorem close_ok (I : Inv cfg s) {k : Nat} (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) {v : List Int} {t : Nat → Int}
    (hvlen : v.length = cfg.n) (hv : ∀ i < cfg.n, get v i = get s.values i + t i)
    (ht : ∀ i, loT cfg s.records[k].recipe s.records[k].units i ≤ t i ∧
      t i ≤ hiT cfg s.records[k].recipe s.records[k].units i) :
    AllB v ∧ envFail cfg s.records v none (some k) = false ∧
      boundsFail cfg s v none (some k) = false := by
  have S := res_sub I hk ho hv ht
  have hb : ∀ i < cfg.n, Bounded (low cfg s.records s.values none none i) ∧
      Bounded (high cfg s.records s.values none none i) := fun i hi =>
    (envFail_false.mp I.res.1) i (by rw [I.wf.vlen]; exact hi)
  refine ⟨allB_of_get fun i hi => ?_, envFail_false.mpr fun i hi => ?_,
    boundsFail_false.mpr fun i hi => ⟨(S i hi).2.2.2.2.1, (S i hi).2.2.2.2.2⟩⟩
  · rw [hvlen] at hi
    obtain ⟨s1, s2, s3, s4, -⟩ := S i hi
    obtain ⟨⟨b1, b2⟩, ⟨b3, b4⟩⟩ := hb i hi
    exact ⟨by omega, by omega⟩
  · rw [hvlen] at hi
    obtain ⟨s1, s2, s3, s4, -⟩ := S i hi
    obtain ⟨⟨b1, b2⟩, ⟨b3, b4⟩⟩ := hb i hi
    exact ⟨⟨by omega, by omega⟩, ⟨by omega, by omega⟩⟩

end box

/-! ## LIVE-ALL -/

section live

variable {cfg : Cfg A} {s : State}

/-- `release(id)` passes every guard for an open record. -/
theorem release_guards_ok (hV : Valid cfg) (I : Inv cfg s) {id : Nat} (hk : id < s.records.length)
    (ho : s.records[id].status.isOpen = true) : NoFire (releaseGuards cfg s id) := by
  have W := I.wf
  have hr := record_eq hk
  have hw := not_pruned_of_open W hk ho
  have RO := recipe_ok hV (W.recs id hk).recipe
  let x : Nat → Int := xlt cfg s.records[id].recipe s.records[id].units
  have hlen : (neg (cfg.recipe s.records[id].recipe).reserve).length = cfg.n := by
    rw [length_neg]; exact RO.reserveLen
  have hvlen : (releaseNext cfg s id).length = cfg.n := by
    simp only [releaseNext, length_addv, hr, hlen, W.vlen]; simp
  have hv : ∀ i < cfg.n, get (releaseNext cfg s id) i = get s.values i + x i := by
    intro i hi
    simp only [releaseNext, hr, x, xlt]
    rw [get_addv (by rw [W.vlen]; exact hi) (by rw [hlen]; exact hi), get_neg, Int.neg_mul]
  have ht : ∀ i, loT cfg s.records[id].recipe s.records[id].units i ≤ x i ∧
      x i ≤ hiT cfg s.records[id].recipe s.records[id].units i := by
    intro i; simp only [loT, hiT, x]; omega
  obtain ⟨c1, c2, c3⟩ := close_ok I hk ho hvlen hv ht
  have hver := bump_ok I.vercap hk ho
  simp only [releaseGuards, addGuards, boundsGuards, verGuard, foundGuard, noFire_append,
    noFire_cons, noFire_nil, and_true, List.cons_append, List.nil_append]
  refine ⟨by simp; omega, by simp [hr, hlen, W.vlen], ?_, c2, c3, by simp; omega, by simp; omega⟩
  simp only [Bool.not_eq_false', decide_eq_true_eq]
  exact c1

/-- `settle(.., true)`'s tail passes every guard for an open record. -/
theorem complete_guards_ok (hV : Valid cfg) (I : Inv cfg s) {id : Nat}
    (hk : id < s.records.length) (ho : s.records[id].status.isOpen = true) :
    NoFire (addGuards s.values (cfg.recipe (s.record id).recipe).complete (s.record id).units ++
      boundsGuards cfg s (completeNext cfg s id) none (some id) ++ [foundGuard s id, verGuard s]) := by
  have W := I.wf
  have hr := record_eq hk
  have hw := not_pruned_of_open W hk ho
  have RO := recipe_ok hV (W.recs id hk).recipe
  let d : Nat → Int := dlt cfg s.records[id].recipe s.records[id].units
  have hlen := RO.completeLen
  have hvlen : (completeNext cfg s id).length = cfg.n := by
    simp only [completeNext, length_addv, hr, hlen, W.vlen]; simp
  have hv : ∀ i < cfg.n, get (completeNext cfg s id) i = get s.values i + d i := by
    intro i hi
    simp only [completeNext, hr, d, dlt]
    exact get_addv (by rw [W.vlen]; exact hi) (by rw [hlen]; exact hi)
  have ht : ∀ i, loT cfg s.records[id].recipe s.records[id].units i ≤ d i ∧
      d i ≤ hiT cfg s.records[id].recipe s.records[id].units i := by
    intro i; simp only [loT, hiT, d]; omega
  obtain ⟨c1, c2, c3⟩ := close_ok I hk ho hvlen hv ht
  have hver := bump_ok I.vercap hk ho
  simp only [addGuards, boundsGuards, verGuard, foundGuard, noFire_append, noFire_cons,
    noFire_nil, and_true, List.cons_append, List.nil_append]
  refine ⟨by simp [hr, hlen, W.vlen], ?_, c2, c3, by simp; omega, by simp; omega⟩
  simp only [Bool.not_eq_false', decide_eq_true_eq]
  exact c1

/-- LEAN-LIVE-ALL (LIVE-ALL): in every `Inv` state of a valid config, a
Reserved record's `cancel` by its actor and by the owner pass every guard,
and an in-flight record's `settle` by its executor passes every guard both
ways. No premise on the baseline or the version. -/
theorem live_all (hV : Valid cfg) (I : Inv cfg s) {id : Nat} (hk : id < s.records.length) :
    (s.records[id].status = .reserved → ∀ now,
      outcome cfg s (.cancel (cfg.recipe s.records[id].recipe).actor id now) = none ∧
      outcome cfg s (.cancel cfg.owner id now) = none) ∧
    (inFlight s.records[id].status = true →
      outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none ∧
      outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id false) = none) := by
  have hr := record_eq hk
  refine ⟨fun hst now => ?_, fun hst => ?_⟩
  · have ho : s.records[id].status.isOpen = true := by rw [hst]; rfl
    have hw := not_pruned_of_open I.wf hk ho
    have R := release_guards_ok hV I hk ho
    constructor <;>
    · simp only [outcome, guards, cancelGuards, firstFail_eq_none, noFire_append, noFire_cons,
        noFire_nil, foundGuard, List.cons_append, List.nil_append, and_true]
      refine ⟨by simp; omega, by simp [hr], by simp [hr, hst], by simp [hr, hst],
        by simp [hr, hst], R⟩
  · have ho : s.records[id].status.isOpen = true := by
      revert hst; cases s.records[id].status <;> simp [inFlight, Status.isOpen]
    have hw := not_pruned_of_open I.wf hk ho
    have hnc : s.records[id].status ≠ .completed := fun h => by rw [h] at ho; cases ho
    have hnx : s.records[id].status ≠ .cancelled := fun h => by rw [h] at ho; cases ho
    constructor
    · simp only [outcome, guards, settleGuards, firstFail_eq_none, noFire_append, noFire_cons,
        noFire_nil, foundGuard, ite_true, List.cons_append, List.nil_append, and_true]
      refine ⟨by simp; omega, by simp [hr], by simp [hr, hnc], by simp [hr, hst], ?_⟩
      have := complete_guards_ok hV I hk ho
      simp only [foundGuard, noFire_append, noFire_cons, noFire_nil, List.cons_append,
        List.nil_append, and_true] at this
      exact this
    · simp only [outcome, guards, settleGuards, firstFail_eq_none, noFire_append, noFire_cons,
        noFire_nil, foundGuard, Bool.false_eq_true, if_false, List.cons_append, List.nil_append,
        and_true]
      exact ⟨by simp; omega, by simp [hr], by simp [hr, hnx], by simp [hr, hst],
        release_guards_ok hV I hk ho⟩

/-- LIVE-ALL from any reachable state of a valid config. -/
theorem live_all_reach (hV : Valid cfg) (h : Reach cfg s) {id : Nat} (hk : id < s.records.length) :
    (s.records[id].status = .reserved → ∀ now,
      outcome cfg s (.cancel (cfg.recipe s.records[id].recipe).actor id now) = none ∧
      outcome cfg s (.cancel cfg.owner id now) = none) ∧
    (inFlight s.records[id].status = true →
      outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none ∧
      outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id false) = none) :=
  live_all hV (reach_inv hV h) hk

/-- No wedge: every open record of a reachable state has a call that closes
it (`cancel` if Reserved, `settle` otherwise), and that call is `Ok`. -/
theorem no_wedge (hV : Valid cfg) (h : Reach cfg s) {id : Nat} (hk : id < s.records.length)
    (ho : s.records[id].status.isOpen = true) :
    ∃ op : Op A, outcome cfg s op = none ∧
      ∃ h' : id < (step cfg s op).records.length, (step cfg s op).records[id].status.isOpen = false := by
  have L := live_all_reach hV h hk
  have closed : ∀ st : Status, st.isOpen = false → ∀ op : Op A, outcome cfg s op = none →
      (effect cfg s op).records = setStatus s.records id st →
      ∃ h' : id < (step cfg s op).records.length, (step cfg s op).records[id].status.isOpen = false := by
    intro st hst op hop he
    simp only [step, exec_none hop]
    have h' : id < (setStatus s.records id st).length := by rw [length_setStatus]; exact hk
    refine ⟨by show id < (effect cfg s op).records.length; rw [he]; exact h', ?_⟩
    show ((effect cfg s op).records[id]'(by rw [he]; exact h')).status.isOpen = false
    simp only [he]
    rw [getElem_setStatus hk]; simp [hst]
  by_cases hres : s.records[id].status = .reserved
  · have hc := (L.1 hres 0).2
    exact ⟨_, hc, closed .cancelled rfl _ hc rfl⟩
  · have hf : inFlight s.records[id].status = true := by
      revert hres ho; cases s.records[id].status <;> simp [inFlight, Status.isOpen]
    have hc := (L.2 hf).1
    exact ⟨_, hc, closed .completed rfl _ hc rfl⟩

end live

/-! ## LIVE-1 (a corollary) -/

/-- LEAN-LIVE1 (LIVE-1): in an `Inv` state, `settle(executor, id, true)`
passes every guard for every Dispatched/Active/Unknown record `id`. v1.0.0
needed `B = 0` and `version < u32::MAX`. -/
theorem live1 {cfg : Cfg A} (hV : Valid cfg) {s : State} (I : Inv cfg s)
    {id : Nat} (hk : id < s.records.length) (hst : inFlight s.records[id].status = true) :
    outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none :=
  ((live_all hV I hk).2 hst).1

/-- LIVE-1 as a run: the settle is `Ok` and completes the record. -/
theorem live1_exec {cfg : Cfg A} (hV : Valid cfg) {s : State} (I : Inv cfg s)
    {id : Nat} (hk : id < s.records.length) (hst : inFlight s.records[id].status = true) :
    (exec cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true)).1 = .ok ∧
      ∃ h : id < (step cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true)).records.length,
        (step cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true)).records[id].status =
          .completed := by
  have ho := live1 hV I hk hst
  refine ⟨by rw [exec_none ho]; rfl, ?_⟩
  simp only [step, exec_none ho]
  have h' : id < (setStatus s.records id .completed).length := by rw [length_setStatus]; exact hk
  refine ⟨h', ?_⟩
  show ((setStatus s.records id .completed)[id]'h').status = .completed
  rw [getElem_setStatus hk]; simp

/-- LIVE-1 from any reachable state of a valid config. -/
theorem live1_reach {cfg : Cfg A} (hV : Valid cfg) {s : State} (h : Reach cfg s)
    {id : Nat} (hk : id < s.records.length) (hst : inFlight s.records[id].status = true) :
    outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none :=
  live1 hV (reach_inv hV h) hk hst

/-! ## INV-OUTCOMES -/

/-- One open record's chosen outcome: completed (`some true`), released
(`some false`) or left open (`none`). -/
def pickT (cfg : Cfg A) (pick : Rec → Option Bool) (c : Rec) (i : Nat) : Int :=
  match pick c with
  | some true => dlt cfg c.recipe c.units i
  | some false => xlt cfg c.recipe c.units i
  | none => 0

/-- With B = 0, the envelope lies inside [min, max] on every metric. -/
theorem env_inside {cfg : Cfg A} {s : State} (I : Inv cfg s) (hB : ∀ i < cfg.n, B cfg s i = 0)
    {i : Nat} (hi : i < cfg.n) :
    (cfg.metric i).min ≤ low cfg s.records s.values none none i ∧
      high cfg s.records s.values none none i ≤ (cfg.metric i).max := by
  have h := (hard_of_inv I).2 i hi
  rw [hB i hi] at h
  simp only [ev] at h
  omega

/-- INV-OUTCOMES (was INV-HEALTHY): with B = 0, applying any combination of
outcomes of the open records (each completed, released or left open) to
the current values lands inside [min, max]. -/
theorem healthy {cfg : Cfg A} {s : State} (I : Inv cfg s) (hB : ∀ i < cfg.n, B cfg s i = 0)
    (pick : Rec → Option Bool) {i : Nat} (hi : i < cfg.n) :
    (cfg.metric i).min ≤ get s.values i +
        sumI (fun c => if c.status.isOpen then pickT cfg pick c i else 0) s.records ∧
      get s.values i + sumI (fun c => if c.status.isOpen then pickT cfg pick c i else 0) s.records
        ≤ (cfg.metric i).max := by
  obtain ⟨h1, h2⟩ := env_inside I hB hi
  have hp : ∀ c : Rec, loT cfg c.recipe c.units i ≤ pickT cfg pick c i ∧
      pickT cfg pick c i ≤ hiT cfg c.recipe c.units i := by
    intro c
    simp only [pickT, loT, hiT]
    split <;> omega
  have hl : envS (loT cfg) s.records none i ≤
      sumI (fun c => if c.status.isOpen then pickT cfg pick c i else 0) s.records := by
    apply sumI_le; intro c _
    simp only [inEnv]
    have := hp c
    by_cases ho : c.status.isOpen <;> simp [ho] <;> omega
  have hh : sumI (fun c => if c.status.isOpen then pickT cfg pick c i else 0) s.records ≤
      envS (hiT cfg) s.records none i := by
    apply sumI_le; intro c _
    simp only [inEnv]
    have := hp c
    by_cases ho : c.status.isOpen <;> simp [ho] <;> omega
  simp only [low, high, extraT] at h1 h2
  omega

end Pool
