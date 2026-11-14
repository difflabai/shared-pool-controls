/-
LEAN-IND: `Inv` (INV-WF with WIN-1/WIN-2, INV-LEDGER, INV-RES, `PV ≤ B`,
INV-DEP, INV-VER, INV-VERCAP) holds initially and is preserved by every
call, for every config `validate` accepts and states of every size (any
number of metrics, terms, recipes and records, any window position);
hence it holds in every reachable state (`reach_inv`). INV-HARD and NO-OVF
are corollaries (`hard_of_inv`, `bump_ok`).
-/
import PoolLean.Facts

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-- The shape of every effective post-state that changes values and records
but not the window's start. -/
def upd (s : State) (v : List Int) (recs : List Rec) : State :=
  { s with values := v, records := recs, version := s.version + 1, eff := s.eff + 1 }

/-! ## Remaining bumps (INV-VERCAP) -/

theorem rem_le (st : Status) : rem st ≤ 3 := by cases st <;> simp [rem]

theorem rem_closed {st : Status} (h : st.isOpen = false) : rem st = 0 := by
  cases st <;> simp_all [rem, Status.isOpen]

theorem rem_pos {st : Status} (h : st.isOpen = true) : 0 < rem st := by
  cases st <;> simp_all [rem, Status.isOpen]

theorem remSum_append (l : List Rec) (c : Rec) : remSum (l ++ [c]) = remSum l + rem c.status := by
  simp [remSum, sumN_append]

theorem remSum_setStatus {l : List Rec} {k : Nat} (hk : k < l.length) (st : Status) :
    remSum (setStatus l k st) + rem l[k].status = remSum l + rem st := by
  simp only [remSum, setStatus, getD_rec hk]
  exact sumN_set (fun c => rem c.status) l k _ hk

/-- Only the window holds open records, and it holds at most `MAX_RECORDS`. -/
theorem remSum_le {cfg : Cfg A} {s : State} (W : WF cfg s) : remSum s.records ≤ 3 * MAX_RECORDS := by
  have h1 : remSum (s.records.take s.firstId) = 0 := by
    apply sumN_eq_zero
    intro x hx
    obtain ⟨j, hj, rfl⟩ := List.mem_iff_getElem.mp hx
    rw [List.length_take] at hj
    simp only [List.getElem_take]
    exact rem_closed ((W.recs j (by omega)).pruned (by omega))
  have h2 : remSum (s.records.drop s.firstId) ≤ 3 * (s.records.drop s.firstId).length :=
    sumN_le_mul fun x _ => rem_le x.status
  have h3 : remSum s.records = remSum (s.records.take s.firstId) + remSum (s.records.drop s.firstId) := by
    simp only [remSum]; rw [← sumN_append, List.take_append_drop]
  rw [List.length_drop] at h2
  have := W.win
  have : 3 * (s.records.length - s.firstId) ≤ 3 * MAX_RECORDS := Nat.mul_le_mul_left 3 this
  omega

/-- A capped bump (`bump_new`) re-establishes INV-VERCAP. -/
theorem vercap_cap {cfg : Cfg A} {s : State} (W : WF cfg s) (h : s.version ≤ VERSION_CAP) :
    VerCap s := by
  have := remSum_le W
  simp only [VerCap, VERSION_CAP, MAX_RECORDS, U32_MAX] at *; omega

/-- NO-OVF: while a record is open, the plain bump cannot fail. -/
theorem bump_ok {s : State} (VC : VerCap s) {k : Nat} (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) : s.version < U32_MAX := by
  have h1 : rem s.records[k].status ≤ remSum s.records :=
    le_sumN (f := fun c => rem c.status) (List.getElem_mem hk)
  have := rem_pos ho
  simp only [VerCap] at VC; omega

/-! ## init -/

theorem inv_init {cfg : Cfg A} (hV : Valid cfg) (now : Nat) : Inv cfg (init cfg now) := by
  have hlen : (cfg.metrics.map (·.initial)).length = cfg.n := by simp [Cfg.n]
  have hb : AllB (cfg.metrics.map (·.initial)) := by
    intro x hx
    simp only [List.mem_map] at hx
    obtain ⟨m, hm, rfl⟩ := hx
    obtain ⟨h1, h2, _, h4, h5⟩ := hV.metric m hm
    simp only [Bounded] at *; omega
  refine ⟨⟨hlen, hb, hlen, hb, by simp [init], by simp [init, MAX_RECORDS], by simp [init],
    by simp [init], by simp [init]⟩, ?_, ⟨?_, ?_⟩, ?_, ?_, ?_, ?_⟩
  · intro i _; simp [init, ledger]
  · rw [envFail_false]; intro i hi
    simp only [init, low, high, envS, extraT, sumI_nil, Int.add_zero]
    have := allB_get hb hi
    exact ⟨this, this⟩
  · intro i _; simp [init, ev, low, high, envS, extraT, viol]
  · intro i _; simp [init, B]
  · intro j hj; simp [init] at hj
  · simp [init, Ver, U32_MAX]
  · simp [init, VerCap, remSum, U32_MAX]

/-! ## Generic: a status update of record `k` -/

section generic

variable {cfg : Cfg A} {s : State} {k : Nat} {st : Status}

theorem not_pruned_of_open (W : WF cfg s) (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) : s.firstId ≤ k := by
  refine Nat.le_of_not_lt fun h => ?_
  have := (W.recs k hk).pruned h; rw [ho] at this; cases this

theorem wf_set (W : WF cfg s) (hk : k < s.records.length) (ho : s.records[k].status.isOpen = true)
    {v : List Int} (hv : v.length = cfg.n) (hvb : AllB v) :
    WF cfg (upd s v (setStatus s.records k st)) := by
  refine ⟨hv, hvb, W.blen, W.bbnd, ?_, ?_, ?_, ?_, ?_⟩
  · simp only [upd, length_setStatus]; exact W.fle
  · simp only [upd, length_setStatus]; exact W.win
  · simp only [upd, length_setStatus]; exact W.k0le
  · simp only [upd, length_setStatus]; have := W.lenVer; omega
  · intro j hj
    simp only [upd] at hj ⊢
    have hj' : j < s.records.length := by rwa [length_setStatus] at hj
    have R := W.recs j hj'
    rw [getElem_setStatus hk]
    split
    · rename_i h
      subst h
      have hk0 : ¬ k < s.k0 := fun h => by have := R.old h; rw [ho] at this; cases this
      have hkf : ¬ k < s.firstId := fun h => by have := R.pruned h; rw [ho] at this; cases this
      exact ⟨R.id, R.recipe, R.unitsPos, R.unitsLe, fun h => absurd h hk0, fun h => absurd h hkf⟩
    · exact ⟨R.id, R.recipe, R.unitsPos, R.unitsLe, R.old, R.pruned⟩

theorem k0_le_of_open (W : WF cfg s) (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) : s.k0 ≤ s.records[k].id := by
  have R := W.recs k hk
  rw [R.id]
  refine Nat.le_of_not_lt fun h => ?_
  have := R.old h; rw [ho] at this; cases this

theorem ledger_set (W : WF cfg s) (L : Ledger cfg s) (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) {v : List Int}
    (hv : ∀ i < cfg.n, get v i = get s.values i +
      (contrib cfg i { s.records[k] with status := st } - contrib cfg i s.records[k])) :
    Ledger cfg (upd s v (setStatus s.records k st)) := by
  intro i hi
  simp only [upd]
  rw [ledger_setStatus cfg hk st s.k0 i (k0_le_of_open W hk ho), hv i hi, L i hi]
  omega

/-- INV-RES after closing record `k` (completed or cancelled): the check
`check_bounds(v, None, Some(k))` that the call passed is exactly the
post-state's envelope, and it gives `EV ≤ PV(v)`; `PV ≤ EV` always. The
new values' violation is also at most the old one's (HARD-STEP). -/
theorem res_close (W : WF cfg s) (hk : k < s.records.length) (hst : st.isOpen = false)
    {v : List Int} (henv : envFail cfg s.records v none (some k) = false)
    (hbnd : boundsFail cfg s v none (some k) = false) :
    InvRes cfg (upd s v (setStatus s.records k st)) ∧
      ∀ i < cfg.n, viol cfg v i ≤ viol cfg s.values i := by
  have hid : ∀ j (h : j < s.records.length), s.records[j].id = j := fun j h => (W.recs j h).id
  have hl : ∀ i, low cfg (setStatus s.records k st) v none none i = low cfg s.records v none (some k) i := by
    intro i; simp only [low, envS_close _ hk hid hst]
  have hh : ∀ i, high cfg (setStatus s.records k st) v none none i = high cfg s.records v none (some k) i := by
    intro i; simp only [high, envS_close _ hk hid hst]
  refine ⟨⟨?_, ?_⟩, ?_⟩
  · show envFail cfg (setStatus s.records k st) v none none = false
    rw [envFail_congr hl hh]; exact henv
  · intro i hi
    show ev cfg (setStatus s.records k st) v none none i = viol cfg v i
    rw [ev_congr hl hh]
    have h1 := (boundsFail_false.mp hbnd) i hi
    have h2 := viol_le_ev cfg s.records v none (some k) i
    omega
  · intro i hi
    have h1 := (boundsFail_false.mp hbnd) i hi
    have h2 := viol_le_ev cfg s.records v none (some k) i
    omega

/-- INV-RES after moving record `k` between open statuses. -/
theorem res_open (R : InvRes cfg s) (hk : k < s.records.length)
    (ho : s.records[k].status.isOpen = true) (hst : st.isOpen = true) :
    InvRes cfg (upd s s.values (setStatus s.records k st)) := by
  have hl : ∀ i, low cfg (setStatus s.records k st) s.values none none i = low cfg s.records s.values none none i := by
    intro i; simp only [low, envS_open _ hk ho hst]
  have hh : ∀ i, high cfg (setStatus s.records k st) s.values none none i = high cfg s.records s.values none none i := by
    intro i; simp only [high, envS_open _ hk ho hst]
  constructor
  · show envFail cfg (setStatus s.records k st) s.values none none = false
    rw [envFail_congr hl hh]; exact R.1
  · intro i hi
    show ev cfg (setStatus s.records k st) s.values none none i = viol cfg s.values i
    rw [ev_congr hl hh]; exact R.2 i hi

theorem dep_set (D : Dep cfg s) (hk : k < s.records.length)
    (hnc : s.records[k].status ≠ .completed) {v : List Int} :
    Dep cfg (upd s v (setStatus s.records k st)) := by
  intro j hj pu hpu
  simp only [upd] at hj hpu ⊢
  have hj' : j < s.records.length := by rwa [length_setStatus] at hj
  have e : (setStatus s.records k st)[j].recipe = s.records[j].recipe := by
    rw [getElem_setStatus hk]; split
    · rename_i h; subst h; rfl
    · rfl
  rw [e] at hpu
  exact Nat.le_trans (D j hj' pu hpu) (doneUnits_setStatus_ge hk hnc st j pu.1)

theorem ver_upd (V : Ver s) (h : s.version < U32_MAX) (v : List Int) (recs : List Rec) :
    Ver (upd s v recs) := by
  simp only [Ver, upd] at *; omega

/-- INV-VERCAP after a plain bump that lowers record `k`'s remaining bumps. -/
theorem vercap_set (VC : VerCap s) (hk : k < s.records.length)
    (hlt : rem st < rem s.records[k].status) (v : List Int) :
    VerCap (upd s v (setStatus s.records k st)) := by
  have := remSum_setStatus hk st
  simp only [VerCap, upd] at *; omega

/-- A status update of an open record, with the new values' ledger delta. -/
theorem inv_set (I : Inv cfg s) (hk : k < s.records.length) (ho : s.records[k].status.isOpen = true)
    (hver : s.version < U32_MAX) {v : List Int} (hv : v.length = cfg.n) (hvb : AllB v)
    (hl : ∀ i < cfg.n, get v i = get s.values i +
      (contrib cfg i { s.records[k] with status := st } - contrib cfg i s.records[k]))
    (hr : InvRes cfg (upd s v (setStatus s.records k st)))
    (hp : ∀ i < cfg.n, viol cfg v i ≤ viol cfg s.values i)
    (hc : VerCap (upd s v (setStatus s.records k st))) :
    Inv cfg (upd s v (setStatus s.records k st)) :=
  ⟨wf_set I.wf hk ho hv hvb, ledger_set I.wf I.ledger hk ho hl, hr,
   fun i hi => Int.le_trans (hp i hi) (I.pvb i hi),
   dep_set I.dep hk (fun h => by rw [h] at ho; cases ho), ver_upd I.ver hver v _, hc⟩

end generic

/-! ## Each call -/

section calls

variable {cfg : Cfg A} {s : State}

theorem inv_dispatch {c : A} {id now : Nat} (I : Inv cfg s)
    (h : outcome cfg s (.dispatch c id now) = none) :
    Inv cfg { effect cfg s (.dispatch c id now) with eff := s.eff + 1 } := by
  have F := dispatch_ok h
  have hk := F.found
  have hst := F.reserved
  rw [record_eq F.found] at hst
  have ho : s.records[id].status.isOpen = true := by rw [hst]; rfl
  show Inv cfg (upd s s.values (setStatus s.records id .dispatched))
  exact inv_set I F.found ho F.noOverflow I.wf.vlen I.wf.vbnd
    (fun i _ => by simp [contrib, hst]) (res_open I.res F.found ho rfl) (fun i _ => Int.le_refl _)
    (vercap_cap (wf_set I.wf F.found ho I.wf.vlen I.wf.vbnd) (by simp only [upd]; have := F.cap; omega))

theorem inv_ack {id : Nat} {acc : Bool} (I : Inv cfg s) (h : outcome cfg s (.ack id acc) = none) :
    Inv cfg { effect cfg s (.ack id acc) with eff := s.eff + 1 } := by
  have F := ack_ok (cfg := cfg) h
  have hk := F.found
  have hst := F.inflight
  have hch := F.changes
  rw [record_eq F.found] at hst hch
  have ho : s.records[id].status.isOpen = true := by rcases hst with h | h <;> rw [h] <;> rfl
  have hst' : (ackStatus acc).isOpen = true := by cases acc <;> rfl
  have hc : ∀ i, contrib cfg i { s.records[id] with status := ackStatus acc } =
      contrib cfg i s.records[id] := by
    intro i
    rcases hst with h | h <;> cases acc <;> simp [contrib, h, ackStatus]
  have hlt : rem (ackStatus acc) < rem s.records[id].status := by
    rcases hst with h | h <;> cases acc <;> simp_all [rem, ackStatus]
  show Inv cfg (upd s s.values (setStatus s.records id (ackStatus acc)))
  exact inv_set I F.found ho F.noOverflow I.wf.vlen I.wf.vbnd
    (fun i _ => by rw [hc]; omega) (res_open I.res F.found ho hst') (fun i _ => Int.le_refl _)
    (vercap_set I.vercap F.found hlt _)

/-- `release(id)` from an open record. -/
theorem inv_release {id : Nat} (I : Inv cfg s) (hk : id < s.records.length)
    (ho : s.records[id].status.isOpen = true) (F : ReleaseOk cfg s id) :
    Inv cfg (release cfg s id |> fun s' => { s' with eff := s.eff + 1 }) := by
  have hlen := F.len
  rw [record_eq hk] at hlen
  have hnc : s.records[id].status ≠ .cancelled := fun h => by rw [h] at ho; cases ho
  have hnd : s.records[id].status ≠ .completed := fun h => by rw [h] at ho; cases ho
  show Inv cfg (upd s (releaseNext cfg s id) (setStatus s.records id .cancelled))
  have RC := res_close (st := .cancelled) I.wf hk rfl F.env F.bnd
  refine inv_set I hk ho F.noOverflow ?_ F.nextB ?_ RC.1 RC.2
    (vercap_set (st := .cancelled) I.vercap hk (rem_pos ho) _)
  · simp only [releaseNext, length_addv, length_neg, record_eq hk, ← hlen, I.wf.vlen]; simp
  · intro i hi
    have hi1 : i < s.values.length := by rw [I.wf.vlen]; exact hi
    simp only [releaseNext, record_eq hk]
    rw [get_addv hi1 (by rw [length_neg, ← hlen]; exact hi1), get_neg]
    simp [contrib, hnc, hnd, Int.neg_mul]

theorem inv_cancel {c : A} {id now : Nat} (I : Inv cfg s)
    (h : outcome cfg s (.cancel c id now) = none) :
    Inv cfg { effect cfg s (.cancel c id now) with eff := s.eff + 1 } := by
  have F := cancel_ok h
  have hk := F.found
  have hst := F.reserved
  rw [record_eq F.found] at hst
  exact inv_release I F.found (by rw [hst]; rfl) F.release

theorem inv_settle {c : A} {id : Nat} {done : Bool} (I : Inv cfg s)
    (h : outcome cfg s (.settle c id done) = none) :
    Inv cfg { effect cfg s (.settle c id done) with eff := s.eff + 1 } := by
  have F := settle_ok h
  have hk := F.found
  have hst := F.inflight
  rw [record_eq F.found] at hst
  have ho : s.records[id].status.isOpen = true := by
    revert hst; cases s.records[id].status <;> simp [inFlight, Status.isOpen]
  have hnc : s.records[id].status ≠ .cancelled := fun h => by rw [h] at ho; cases ho
  have hnd : s.records[id].status ≠ .completed := fun h => by rw [h] at ho; cases ho
  cases done with
  | false => exact inv_release I F.found ho (F.release rfl)
  | true =>
    have G := F.complete rfl
    have hlen := G.len
    rw [record_eq F.found] at hlen
    show Inv cfg (upd s (completeNext cfg s id) (setStatus s.records id .completed))
    have RC := res_close (st := .completed) I.wf F.found rfl G.env G.bnd
    refine inv_set I F.found ho G.noOverflow ?_ G.nextB ?_ RC.1 RC.2
      (vercap_set (st := .completed) I.vercap F.found (rem_pos ho) _)
    · simp only [completeNext, length_addv, record_eq F.found, ← hlen, I.wf.vlen]; simp
    · intro i hi
      have hi1 : i < s.values.length := by rw [I.wf.vlen]; exact hi
      simp only [completeNext, record_eq F.found]
      rw [get_addv hi1 (by rw [← hlen]; exact hi1)]
      simp [contrib, hnc, hnd]
      omega

/-- `submit`: the record is appended to the history; if 256 records were
retained, the oldest (terminal, since the Full guard passed) is pruned. -/
theorem inv_submit {c : A} {r u ver exp now : Nat} (I : Inv cfg s)
    (h : outcome cfg s (.submit c r u ver exp now) = none) :
    Inv cfg { effect cfg s (.submit c r u ver exp now) with eff := s.eff + 1 } := by
  have F := submit_ok h
  have W := I.wf
  let new : Rec := ⟨s.records.length, r, u, exp, .reserved⟩
  let next := addv s.values (cfg.recipe r).reserve u
  let fid := if MAX_RECORDS ≤ s.records.length - s.firstId then s.firstId + 1 else s.firstId
  have hnext : next.length = cfg.n := by
    simp only [next, length_addv, ← F.len, W.vlen]; simp
  show Inv cfg { s with records := s.records ++ [new], firstId := fid, values := next,
                        version := s.version + 1, eff := s.eff + 1 }
  have hl : ∀ i, low cfg (s.records ++ [new]) next none none i = low cfg s.records next (some (r, u)) none i := by
    intro i; simp only [low, new, envS_append_new, extraT]; omega
  have hh : ∀ i, high cfg (s.records ++ [new]) next none none i = high cfg s.records next (some (r, u)) none i := by
    intro i; simp only [high, new, envS_append_new, extraT]; omega
  -- the Full guard passed: if 256 records are retained, the front is terminal
  have hfront : MAX_RECORDS ≤ s.records.length - s.firstId → ∃ h : s.firstId < s.records.length,
      s.records[s.firstId].status.isOpen = false := by
    intro hm
    have hlt : s.firstId < s.records.length := by simp only [MAX_RECORDS] at hm; omega
    refine ⟨hlt, ?_⟩
    have := F.full
    simp only [fullFires, Bool.and_eq_false_iff, decide_eq_false_iff_not, record_eq hlt] at this
    rcases this with h | h
    · exact absurd hm h
    · exact h
  have hfid1 : s.firstId ≤ fid := by simp only [fid]; split <;> omega
  have hfid2 : fid ≤ s.records.length := by
    simp only [fid]; split
    · rename_i hm; simp only [MAX_RECORDS] at hm; omega
    · exact W.fle
  have W' : WF cfg { s with records := s.records ++ [new], firstId := fid, values := next,
                             version := s.version + 1, eff := s.eff + 1 } := by
    refine ⟨hnext, F.nextB, W.blen, W.bbnd, ?_, ?_, ?_, ?_, ?_⟩
    · show fid ≤ (s.records ++ [new]).length
      simp only [List.length_append, List.length_singleton]; omega
    · show (s.records ++ [new]).length - fid ≤ MAX_RECORDS
      simp only [List.length_append, List.length_singleton, fid]
      have := W.win; split <;> omega
    · show s.k0 ≤ (s.records ++ [new]).length
      simp only [List.length_append, List.length_singleton]; have := W.k0le; omega
    · show (s.records ++ [new]).length ≤ s.version + 1
      simp only [List.length_append, List.length_singleton]; have := W.lenVer; omega
    · intro j hj
      simp only [List.length_append, List.length_singleton] at hj ⊢
      by_cases hjl : j < s.records.length
      · have R := W.recs j hjl
        rw [List.getElem_append_left hjl]
        refine ⟨R.id, R.recipe, R.unitsPos, R.unitsLe, R.old, fun hj' => ?_⟩
        by_cases hjf : j < s.firstId
        · exact R.pruned hjf
        · simp only [fid] at hj'
          split at hj'
          · rename_i hm
            obtain ⟨_, hf⟩ := hfront hm
            have : j = s.firstId := by omega
            subst this; exact hf
          · omega
      · have hj' : j = s.records.length := by omega
        subst hj'
        rw [List.getElem_append_right (Nat.le_refl _)]
        simp only [Nat.sub_self, List.getElem_cons_zero, new]
        exact ⟨rfl, F.recipe, F.unitsPos, F.unitsLe, fun h => absurd W.k0le (by omega),
          fun h => absurd hfid2 (by omega)⟩
  have hres : envFail cfg (s.records ++ [new]) next none none = false ∧
      ∀ i < cfg.n, ev cfg (s.records ++ [new]) next none none i = viol cfg next i ∧
        viol cfg next i ≤ viol cfg s.values i := by
    refine ⟨by rw [envFail_congr hl hh]; exact F.env, fun i hi => ?_⟩
    rw [ev_congr hl hh]
    have h1 := (boundsFail_false.mp F.bnd) i hi
    have h2 := viol_le_ev cfg s.records next (some (r, u)) none i
    simp only [next] at h2 ⊢
    omega
  refine ⟨W', ?_, ⟨hres.1, fun i hi => (hres.2 i hi).1⟩, ?_, ?_, ?_, ?_⟩
  · intro i hi
    show get next i = get s.base i + ledger cfg (s.records ++ [new]) s.k0 i
    simp only [next]
    have hi1 : i < s.values.length := by rw [W.vlen]; exact hi
    rw [ledger_append_new cfg s.records s.k0 r u exp i W.k0le,
      get_addv hi1 (by rw [← F.len]; exact hi1), I.ledger i hi]
    omega
  · intro i hi
    show viol cfg next i ≤ viol cfg s.base i
    exact Int.le_trans (hres.2 i hi).2 (I.pvb i hi)
  · intro j hj pu hpu
    simp only [List.length_append, List.length_singleton] at hj hpu ⊢
    by_cases hjl : j < s.records.length
    · rw [List.getElem_append_left hjl] at hpu
      rw [List.take_append_of_le_length (Nat.le_of_lt hjl)]
      exact I.dep j hjl pu hpu
    · have hj' : j = s.records.length := by omega
      subst hj'
      rw [List.getElem_append_right (Nat.le_refl _)] at hpu
      simp only [Nat.sub_self, List.getElem_cons_zero, new] at hpu
      rw [List.take_append_of_le_length (Nat.le_refl _), List.take_length]
      have := F.prereq
      simp only [prereqFail, doneU, List.any_eq_false, decide_eq_true_eq, Nat.not_lt] at this
      have h2 : ¬ (min (doneUnits s.records pu.1) U32_MAX < pu.2) := fun h' =>
        this pu hpu (decide_eq_true h')
      have h3 := Nat.le_of_not_lt h2
      exact Nat.le_trans h3 (Nat.min_le_left _ _)
  · show s.version + 1 = s.eff + 1 ∧ s.version + 1 ≤ U32_MAX
    have := I.ver.1; have := F.noOverflow; omega
  · exact vercap_cap W' (by show s.version + 1 ≤ VERSION_CAP; have := F.cap; omega)

theorem inv_pause {c : A} {b : Bool} (I : Inv cfg s) (h : outcome cfg s (.pause c b) = none) :
    Inv cfg { effect cfg s (.pause c b) with eff := s.eff + 1 } := by
  have F := pause_ok h
  obtain ⟨W, L, R, P, D, V, VC⟩ := I
  show Inv cfg { s with paused := b, version := s.version + 1, eff := s.eff + 1 }
  have W' : WF cfg { s with paused := b, version := s.version + 1, eff := s.eff + 1 } :=
    ⟨W.vlen, W.vbnd, W.blen, W.bbnd, W.fle, W.win, W.k0le,
      by show s.records.length ≤ s.version + 1; have := W.lenVer; omega, fun j hj => W.recs j hj⟩
  exact ⟨W', L, R, P, D,
    by show s.version + 1 = s.eff + 1 ∧ s.version + 1 ≤ U32_MAX
       have := V.1; have := F.2.2; omega,
    vercap_cap W' (by show s.version + 1 ≤ VERSION_CAP; have := F.2.1; omega)⟩

/-- `observe`: a refresh (the values restated while some record is open)
keeps the ghosts; otherwise no record is open and the call re-bases. -/
theorem inv_observe {c : A} {vals : List Int} {now : Nat} (I : Inv cfg s)
    (h : outcome cfg s (.observe c vals now) = none) :
    Inv cfg { effect cfg s (.observe c vals now) with eff := s.eff + 1 } := by
  have F := observe_ok h
  obtain ⟨W, L, R, P, D, V, VC⟩ := I
  by_cases hr : refresh s vals = true
  · have hv : vals = s.values := by simp only [refresh, Bool.and_eq_true, decide_eq_true_eq] at hr; exact hr.1
    subst hv
    show Inv cfg { s with values := s.values, observedAt := now, version := s.version + 1,
                          base := if refresh s s.values then s.base else s.values,
                          k0 := if refresh s s.values then s.k0 else s.records.length,
                          eff := s.eff + 1 }
    simp only [hr, if_true]
    have W' : WF cfg { s with observedAt := now, version := s.version + 1, eff := s.eff + 1 } :=
      ⟨W.vlen, W.vbnd, W.blen, W.bbnd, W.fle, W.win, W.k0le,
        by show s.records.length ≤ s.version + 1; have := W.lenVer; omega, fun j hj => W.recs j hj⟩
    exact ⟨W', L, R, P, D,
      by show s.version + 1 = s.eff + 1 ∧ s.version + 1 ≤ U32_MAX
         have := V.1; have := F.noOverflow; omega,
      vercap_cap W' (by show s.version + 1 ≤ VERSION_CAP; have := F.cap; omega)⟩
  · have hclosed' : anyOpen s.records = false := by
      rcases F.pend with hv | hv
      · subst hv; simp only [refresh, decide_true, Bool.true_and, Bool.not_eq_true] at hr; exact hr
      · exact hv
    have hr' : refresh s vals = false := by simpa using hr
    show Inv cfg { s with values := vals, observedAt := now, version := s.version + 1,
                          base := if refresh s vals then s.base else vals,
                          k0 := if refresh s vals then s.k0 else s.records.length,
                          eff := s.eff + 1 }
    simp only [hr', Bool.false_eq_true, if_false]
    have hlen : vals.length = cfg.n := by rw [F.len, W.vlen]
    have hcl : ∀ x ∈ s.records, x.status.isOpen = false := by
      simpa [anyOpen] using hclosed'
    have hclosed : ∀ j (hj : j < s.records.length), s.records[j].status.isOpen = false :=
      fun j hj => hcl _ (List.getElem_mem hj)
    have henv : ∀ (t : Nat → Nat → Nat → Int) i, envS t s.records none i = 0 :=
      fun t i => envS_closed t hclosed' none i
    have W' : WF cfg { s with values := vals, observedAt := now, version := s.version + 1,
                              base := vals, k0 := s.records.length, eff := s.eff + 1 } :=
      ⟨hlen, F.valsB, hlen, F.valsB, W.fle, W.win, Nat.le_refl _,
        by show s.records.length ≤ s.version + 1; have := W.lenVer; omega,
        fun j hj => by
          have R := W.recs j hj
          exact ⟨R.id, R.recipe, R.unitsPos, R.unitsLe, fun _ => hclosed j hj, R.pruned⟩⟩
    refine ⟨W', ?_, ⟨?_, ?_⟩, ?_, D, ?_, ?_⟩
    · intro i _
      show get vals i = get vals i + ledger cfg s.records s.records.length i
      have : ledger cfg s.records s.records.length i = 0 := by
        apply Eq.trans (sumI_congr _) (sumI_zero s.records)
        intro x hx
        obtain ⟨j, hj, rfl⟩ := List.mem_iff_getElem.mp hx
        have := (W.recs j hj).id
        simp only [this]; simp; omega
      omega
    · show envFail cfg s.records vals none none = false
      rw [envFail_false]; intro i hi
      simp only [low, high, henv, extraT, Int.add_zero]
      have := allB_get F.valsB hi
      exact ⟨this, this⟩
    · intro i _
      show ev cfg s.records vals none none i = viol cfg vals i
      simp only [ev, low, high, henv, extraT, Int.add_zero, viol]
    · intro i _
      show viol cfg vals i ≤ viol cfg vals i
      exact Int.le_refl _
    · show s.version + 1 = s.eff + 1 ∧ s.version + 1 ≤ U32_MAX
      have := V.1; have := F.noOverflow; omega
    · exact vercap_cap W' (by show s.version + 1 ≤ VERSION_CAP; have := F.cap; omega)

end calls

/-! ## Every call, every reachable state -/

theorem inv_effect {cfg : Cfg A} {s : State} {op : Op A} (I : Inv cfg s)
    (h : outcome cfg s op = none) : Inv cfg { effect cfg s op with eff := s.eff + 1 } := by
  cases op with
  | submit c r u ver exp now => exact inv_submit I h
  | dispatch c id now => exact inv_dispatch I h
  | ack id acc => exact inv_ack I h
  | cancel c id now => exact inv_cancel I h
  | settle c id done => exact inv_settle I h
  | pause c b => exact inv_pause I h
  | observe c vals now => exact inv_observe I h

/-- LEAN-IND: every call, accepted or not, preserves `Inv`. -/
theorem inv_step {cfg : Cfg A} {s : State} (I : Inv cfg s) (op : Op A) : Inv cfg (step cfg s op) := by
  rcases exec_cases cfg s op with h | ⟨h1, h2⟩
  · simp only [step, h]; exact I
  · simp only [step, h2]; exact inv_effect I h1

/-- LEAN-IND: `Inv` holds in every reachable state of every valid config. -/
theorem reach_inv {cfg : Cfg A} (hV : Valid cfg) {s : State} (h : Reach cfg s) : Inv cfg s := by
  induction h with
  | init now => exact inv_init hV now
  | step op _ ih => exact inv_step ih op

/-- INV-HARD, now a corollary: `EV = PV(values) ≤ B`. -/
theorem hard_of_inv {cfg : Cfg A} {s : State} (I : Inv cfg s) : Hard cfg s :=
  ⟨I.res.1, fun i hi => by rw [I.res.2 i hi]; exact I.pvb i hi⟩

end Pool
