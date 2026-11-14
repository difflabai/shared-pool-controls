/-
The properties (strategy catalogue IDs): the invariant `Inv` (INV-WF with
WIN-1/WIN-2, INV-LEDGER, INV-RES, INV-DEP, INV-VER, INV-VERCAP, and
`PV(values) ≤ B`, from which INV-HARD follows) and the step properties.
-/
import PoolLean.Machine

namespace Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-! ## The invariant -/

/-- One record's well-formedness (INV-WF), `j` its index in the history. -/
structure RecOk (cfg : Cfg A) (k0 fid : Nat) (j : Nat) (c : Rec) : Prop where
  id : c.id = j
  recipe : c.recipe < cfg.recipes.length
  unitsPos : 0 < c.units
  unitsLe : c.units ≤ (cfg.recipe c.recipe).maxUnits
  /-- records before the last re-basing `observe` are terminal -/
  old : j < k0 → c.status.isOpen = false
  /-- WIN-2: pruned records are terminal -/
  pruned : j < fid → c.status.isOpen = false

/-- INV-WF with WIN-1: values of length `n` and bounded, `records[j].id = j`
(the window's ids are `firstId, firstId + 1, ...`), recipe in range, `1 ≤
units ≤ max_units`; `firstId ≤ records.length`, at most `MAX_RECORDS`
records retained, and `firstId + len = records.length ≤ version` (so the
id's `checked_add`, lib.rs 363-366, cannot fail); the ghost baseline is a
bounded vector of length `n`; every record before `k0` and (WIN-2) every
pruned record is terminal. (The config never changes: it is a parameter of
the machine.) -/
structure WF (cfg : Cfg A) (s : State) : Prop where
  vlen : s.values.length = cfg.n
  vbnd : AllB s.values
  blen : s.base.length = cfg.n
  bbnd : AllB s.base
  fle : s.firstId ≤ s.records.length
  win : s.records.length - s.firstId ≤ MAX_RECORDS
  k0le : s.k0 ≤ s.records.length
  lenVer : s.records.length ≤ s.version
  recs : ∀ j (h : j < s.records.length), RecOk cfg s.k0 s.firstId j s.records[j]

/-- A record's effect on metric `i`: its reservation unless cancelled, plus
its completion if completed. -/
def contrib (cfg : Cfg A) (i : Nat) (c : Rec) : Int :=
  (if c.status = .cancelled then 0 else get (cfg.recipe c.recipe).reserve i * (c.units : Int)) +
  (if c.status = .completed then get (cfg.recipe c.recipe).complete i * (c.units : Int) else 0)

/-- `Σ_{j ≥ k0} contrib(records[j])` (ids are positions, INV-WF). -/
def ledger (cfg : Cfg A) (recs : List Rec) (k0 i : Nat) : Int :=
  sumI (fun c => if k0 ≤ c.id then contrib cfg i c else 0) recs

/-- INV-LEDGER: `values = base + Σ_{j ≥ k0, ¬Cancelled} reserve_j·u_j +
Σ_{j ≥ k0, Completed} complete_j·u_j`, over the history (pruned records
included; a refresh does not re-base). -/
def Ledger (cfg : Cfg A) (s : State) : Prop :=
  ∀ i < cfg.n, get s.values i = get s.base i + ledger cfg s.records s.k0 i

/-- B_i = PV_i(base). -/
def B (cfg : Cfg A) (s : State) (i : Nat) : Int := viol cfg s.base i

/-- INV-RES: the envelope over the open records is computable (its
endpoints within ±1e6), and on every metric its violation equals the
values' own: `EV_i(values) = PV_i(values)`. Ghost-free. -/
def InvRes (cfg : Cfg A) (s : State) : Prop :=
  envFail cfg s.records s.values none none = false ∧
    ∀ i < cfg.n, ev cfg s.records s.values none none i = viol cfg s.values i

/-- No metric's violation exceeds the baseline's: `PV_i(values) ≤ B_i`
(HARD-STEP accumulated since the last re-basing `observe`). -/
def PvB (cfg : Cfg A) (s : State) : Prop := ∀ i < cfg.n, viol cfg s.values i ≤ B cfg s i

/-- INV-HARD (v1.0's invariant, now a corollary `hard_of_inv`): the
envelope is computable and its violation is at most the baseline's. -/
def Hard (cfg : Cfg A) (s : State) : Prop :=
  envFail cfg s.records s.values none none = false ∧
    ∀ i < cfg.n, ev cfg s.records s.values none none i ≤ B cfg s i

/-- INV-DEP: every record's prerequisites are met by completed records of
lower index (the history: pruned records count, as `completed_units`
keeps lifetime counts). -/
def Dep (cfg : Cfg A) (s : State) : Prop :=
  ∀ j (h : j < s.records.length), ∀ pu ∈ (cfg.recipe s.records[j].recipe).requires,
    pu.2 ≤ doneUnits (s.records.take j) pu.1

/-- INV-VER: the version counts the effective transitions, and never passed
`u32::MAX`. -/
def Ver (s : State) : Prop := s.version = s.eff ∧ s.version ≤ U32_MAX

/-- The plain bumps an open record may still need: `cancel` (Reserved), or
`ack`, `ack`, `settle` (Dispatched), `ack`, `settle` (Unknown), `settle`
(Active). `dispatch` is a capped bump and needs no headroom. -/
def rem : Status → Nat
  | .reserved => 3
  | .dispatched => 3
  | .unknown => 2
  | .active => 1
  | _ => 0

def remSum (recs : List Rec) : Nat := sumN (fun c => rem c.status) recs

/-- INV-VERCAP: the version leaves room for every plain bump the open
records may still need. NO-OVF (`bump_ok`) is a corollary. -/
def VerCap (s : State) : Prop := s.version + remSum s.records ≤ U32_MAX

structure Inv (cfg : Cfg A) (s : State) : Prop where
  wf : WF cfg s
  ledger : Ledger cfg s
  res : InvRes cfg s
  pvb : PvB cfg s
  dep : Dep cfg s
  ver : Ver s
  vercap : VerCap s

/-! ## Step properties -/

/-- LIFE-1's status graph (reflexive: a step may leave a status alone). -/
def edge : Status → Status → Bool
  | .reserved, .dispatched => true
  | .reserved, .cancelled => true
  | .dispatched, .active => true
  | .dispatched, .unknown => true
  | .unknown, .active => true
  | .dispatched, .completed => true
  | .active, .completed => true
  | .unknown, .completed => true
  | .dispatched, .cancelled => true
  | .active, .cancelled => true
  | .unknown, .cancelled => true
  | a, b => a == b

/-- LIFE-1: records are append-only, a new record is `Reserved` with its
index as id, `id`/`recipe`/`units`/`expires_at` never change, and statuses
move only along `edge` (so `Completed` and `Cancelled` are absorbing). -/
structure Life1 (rs rs' : List Rec) : Prop where
  grow : rs.length ≤ rs'.length
  frame : ∀ j (h : j < rs.length) (h' : j < rs'.length),
    rs'[j].id = rs[j].id ∧ rs'[j].recipe = rs[j].recipe ∧ rs'[j].units = rs[j].units ∧
    rs'[j].expiresAt = rs[j].expiresAt ∧ edge rs[j].status rs'[j].status = true
  fresh : ∀ j (h' : j < rs'.length), rs.length ≤ j → rs'[j].status = .reserved ∧ rs'[j].id = j

/-- AUTH-1, per call: an `Ok` result (effective or no-op) implies the caller's
authority. `ack` has no caller (the shell makes it private). -/
def Auth1 (cfg : Cfg A) (s : State) : Op A → Prop
  | .submit c r _ _ _ _ => c = (cfg.recipe r).actor
  | .dispatch c id _ => c = (cfg.recipe (s.record id).recipe).actor
  | .ack _ _ => True
  | .cancel c id _ => c = (cfg.recipe (s.record id).recipe).actor ∨ c = cfg.owner
  | .settle c id _ => c = (cfg.recipe (s.record id).recipe).executor
  | .pause c _ => c = cfg.owner
  | .observe c _ _ => c = cfg.owner

def Res.isOk : Res → Bool
  | .err _ => false
  | _ => true

def Op.isObserve : Op A → Bool
  | .observe .. => true
  | _ => false

def Op.isAck : Op A → Bool
  | .ack .. => true
  | _ => false

/-- The step properties of one call `op` from `s`, with result `r` and
post-state `s'`. -/
structure StepProps (cfg : Cfg A) (s : State) (op : Op A) (r : Res) (s' : State) : Prop where
  /-- TX-0: an error leaves the pool unchanged -/
  tx0 : ∀ e, r = .err e → s' = s
  /-- TX-VER / TX-NOOP: an `Ok` either changes nothing (version included) or
  bumps the version by exactly one -/
  txver : r.isOk = true → s' = s ∨ s'.version = s.version + 1
  /-- the version never decreases -/
  verMono : s.version ≤ s'.version
  life1 : Life1 s.records s'.records
  auth1 : r.isOk = true → Auth1 cfg s op
  /-- STALE-1: an accepted request named the current version -/
  stale1 : ∀ c rc u ver exp now, op = .submit c rc u ver exp now → r.isOk = true →
    ver = s.version ∧ s'.version = s.version + 1
  /-- ADM-1 -/
  adm1 : ∀ c rc u ver exp now, op = .submit c rc u ver exp now → r.isOk = true →
    energy cfg s'.values ≤ energy cfg s.values
  /-- ADM-2: in a violating state, the reservation plus its completion
  strictly lowers the energy -/
  adm2 : ∀ c rc u ver exp now, op = .submit c rc u ver exp now → r.isOk = true →
    violating cfg s.values = true →
    energy cfg (addv s'.values (cfg.recipe rc).complete u) < energy cfg s.values
  /-- HARD-STEP: no call but `observe` raises any metric's violation -/
  hardStep : op.isObserve = false → ∀ i < cfg.n, viol cfg s'.values i ≤ viol cfg s.values i
  /-- WIN-1/WIN-2: `firstId` grows by at most one, and a pruned record
  never changes -/
  prune : s.firstId ≤ s'.firstId ∧ s'.firstId ≤ s.firstId + 1 ∧
    ∀ j (h : j < s.records.length) (h' : j < s'.records.length), j < s.firstId →
      s'.records[j] = s.records[j]
  /-- ASYNC-1: `acknowledge` never changes values and keeps every record's
  reservation (open stays open, nothing is cancelled) -/
  async1 : op.isAck = true → s'.values = s.values ∧ s'.records.length = s.records.length ∧
    ∀ j (h : j < s.records.length) (h' : j < s'.records.length),
      s'.records[j].status.isOpen = s.records[j].status.isOpen ∧
      (s'.records[j].status = .cancelled ↔ s.records[j].status = .cancelled)

end Pool
