/-
LEAN-GUARD: the guard list of every call has exactly the code's order of
outcomes (`guardOrder`, read off lib.rs), and every outcome a call can have
is the first guard of that list that fires. `pool-lean guards` prints
`guardOrder` for comparison with the other layers' guard tables (C-2).
-/
import PoolLean.Machine

namespace Pool.Guard

open Pool

variable {A : Type} [DecidableEq A] [Inhabited A]

/-- Call kinds; `settle` has one guard list per value of `completed`. -/
inductive Kind where
  | submit
  | dispatch
  | ack
  | cancel
  | settleDone
  | settleUndone
  | pause
  | observe
  deriving DecidableEq, Repr

def kinds : List Kind :=
  [.submit, .dispatch, .ack, .cancel, .settleDone, .settleUndone, .pause, .observe]

def kind : Op A → Kind
  | .submit .. => .submit
  | .dispatch .. => .dispatch
  | .ack .. => .ack
  | .cancel .. => .cancel
  | .settle _ _ true => .settleDone
  | .settle _ _ false => .settleUndone
  | .pause .. => .pause
  | .observe .. => .observe

open Out Err in
/-- The code's guard order, call by call (lib.rs v1.1.0 line of each check,
at the pin 57b3f02). Lean keeps the checks that cannot fire in a reachable
state (the second `index`, the plain bump after the cap, the id add) so the
list is the code's, check for check. -/
def guardOrder : Kind → List Out
  -- candidate 267-322, submit 360-366
  | .submit => [err paused, err stale, err expired, err snapshotExpired,
                err full,                                -- 287-289: 256 retained, front open
                err invalidRecipe, err unauthorized, err invalidUnits,
                err prerequisite,                        -- 301-306: completed_units
                err invalidConfig, err arithmetic,       -- add (307)
                err arithmetic, err bounds,              -- check_bounds (308)
                err bounds, err bounds, err Err.energy,  -- energy before/after (309-313)
                err invalidConfig, err arithmetic,       -- recovery add (315)
                err bounds, err Err.energy,              -- recovery energy (315-317)
                err arithmetic,                          -- VERSION_CAP (319-321)
                err arithmetic,                          -- version checked_add (362)
                err arithmetic]                          -- id checked_add (363-366)
  -- 407-431
  | .dispatch => [err notFound, err unauthorized, err paused, err invalidState, err expired,
                  err snapshotExpired,
                  err notFound,                          -- index (430)
                  err arithmetic, err arithmetic]        -- bump_new (431 -> 401-405)
  -- 436-452
  | .ack => [err notFound, noop,
             err notFound,                               -- index (448)
             noop, err arithmetic]                       -- bump (452)
  -- 456-474, then release 476-485
  | .cancel => [err notFound, err unauthorized, noop,
                err invalidState,                        -- Completed (467-469)
                err pending,                             -- 470-472
                err notFound, err invalidConfig, err arithmetic, err arithmetic, err bounds,
                err notFound, err arithmetic]            -- index (484), bump (485)
  -- 492-515
  | .settleDone => [err notFound, err unauthorized, noop, err invalidState,
                    err invalidConfig, err arithmetic,   -- add (512)
                    err arithmetic, err bounds,          -- check_bounds (513)
                    err notFound, err arithmetic]        -- index (514), bump (515)
  -- 492-510, then release 476-485
  | .settleUndone => [err notFound, err unauthorized, noop, err invalidState,
                      err notFound, err invalidConfig, err arithmetic, err arithmetic, err bounds,
                      err notFound, err arithmetic]
  -- 522-526
  | .pause => [err unauthorized, err arithmetic, err arithmetic]
  -- 534-545
  | .observe => [err unauthorized,
                 err pending,                            -- 538-540: changed values, some open
                 err bounds, err bounds,                 -- 541-544
                 err arithmetic, err arithmetic]         -- bump_new (545)

/-- LEAN-GUARD: every call's guard list has exactly `guardOrder`'s outcomes, in order. -/
theorem guards_order (cfg : Cfg A) (s : State) (op : Op A) :
    (guards cfg s op).map Prod.fst = guardOrder (kind op) := by
  cases op with
  | settle c id done => cases done <;> rfl
  | _ => rfl

theorem firstFail_mem {gs : List Guard} {o : Out} (h : firstFail gs = some o) :
    o ∈ gs.map Prod.fst := by
  induction gs with
  | nil => simp [firstFail] at h
  | cons g gs ih =>
    obtain ⟨o', b⟩ := g
    cases b
    · simp only [firstFail] at h; exact List.mem_cons_of_mem _ (ih (by simpa using h))
    · simp only [firstFail, if_true] at h; cases h; simp

/-- Every outcome a call can have is in its kind's `guardOrder`. -/
theorem outcome_mem (cfg : Cfg A) (s : State) (op : Op A) {o : Out}
    (h : outcome cfg s op = some o) : o ∈ guardOrder (kind op) := by
  rw [← guards_order cfg s op]; exact firstFail_mem h

def outName : Out → String
  | .noop => "noop"
  | .err e => match e with
    | .invalidConfig => "invalid_config"
    | .unauthorized => "unauthorized"
    | .paused => "paused"
    | .stale => "stale"
    | .expired => "expired"
    | .snapshotExpired => "snapshot_expired"
    | .invalidRecipe => "invalid_recipe"
    | .invalidUnits => "invalid_units"
    | .prerequisite => "prerequisite"
    | .full => "full"
    | .arithmetic => "arithmetic"
    | .bounds => "bounds"
    | .energy => "energy"
    | .invalidState => "invalid_state"
    | .notFound => "not_found"
    | .pending => "pending"

def kindName : Kind → String
  | .submit => "submit"
  | .dispatch => "dispatch"
  | .ack => "ack"
  | .cancel => "cancel"
  | .settleDone => "settle_completed"
  | .settleUndone => "settle_cancelled"
  | .pause => "pause"
  | .observe => "observe"

end Pool.Guard
