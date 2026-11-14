/-
The conformance trace format (`conformance/src/main.rs`): a small parser
for its Prolog terms and the printers the replay compares with. Tools only;
no theorem depends on this file.
-/
import PoolLean.Machine

namespace Pool.Text

open Pool

/-- A Prolog term as the traces write them: atoms, integers, compound terms
and lists. -/
inductive PT where
  | atom (s : String)
  | num (n : Int)
  | app (f : String) (args : List PT)
  | list (xs : List PT)
  deriving Repr, Inhabited

def isIdent (c : Char) : Bool := c.isAlphanum || c == '_'

def takeWhileC (p : Char → Bool) : List Char → List Char × List Char
  | [] => ([], [])
  | c :: cs => if p c then let (a, b) := takeWhileC p cs; (c :: a, b) else ([], c :: cs)

def digitsVal (ds : List Char) : Nat := ds.foldl (fun n d => 10 * n + (d.toNat - '0'.toNat)) 0

mutual
  /-- One term; `fuel` bounds the nesting. -/
  def term : Nat → List Char → Option (PT × List Char)
    | 0, _ => none
    | fuel + 1, cs =>
      match cs with
      | '[' :: ']' :: rest => some (.list [], rest)
      | '[' :: rest =>
        match args fuel rest with
        | some (xs, ']' :: r) => some (.list xs, r)
        | _ => none
      | '-' :: rest =>
        let (ds, r) := takeWhileC Char.isDigit rest
        if ds.isEmpty then none else some (.num (-(digitsVal ds : Int)), r)
      | c :: rest =>
        if c.isDigit then
          let (ds, r) := takeWhileC Char.isDigit (c :: rest)
          some (.num (digitsVal ds), r)
        else if c.isLower then
          let (name, r) := takeWhileC isIdent (c :: rest)
          match r with
          | '(' :: r' =>
            match args fuel r' with
            | some (xs, ')' :: r'') => some (.app (String.ofList name) xs, r'')
            | _ => none
          | _ => some (.atom (String.ofList name), r)
        else none
      | [] => none
  /-- Comma-separated terms; stops before the closing bracket. -/
  def args : Nat → List Char → Option (List PT × List Char)
    | 0, _ => none
    | fuel + 1, cs =>
      match term fuel cs with
      | some (t, ',' :: r) =>
        match args fuel r with
        | some (ts, r') => some (t :: ts, r')
        | none => none
      | some (t, r) => some ([t], r)
      | none => none
end

/-- A whole line `Term.` -/
def parseLine (s : String) : Option PT :=
  let cs := s.toList.filter (· != ' ')
  match term (cs.length + 1) cs with
  | some (t, ['.']) => some t
  | _ => none

/-! ## Decoding -/

def PT.int? : PT → Option Int
  | .num n => some n
  | _ => none

def PT.nat? : PT → Option Nat
  | .num n => if 0 ≤ n then some n.toNat else none
  | _ => none

def PT.atom? : PT → Option String
  | .atom s => some s
  | _ => none

def PT.bool? : PT → Option Bool
  | .atom "true" => some true
  | .atom "false" => some false
  | _ => none

def PT.list? : PT → Option (List PT)
  | .list xs => some xs
  | _ => none

def ints (t : PT) : Option (List Int) := do (← t.list?).mapM PT.int?

def metric? : PT → Option Metric
  | .app "m" [mn, mx, i] => do pure ⟨← mn.int?, ← mx.int?, ← i.int?⟩
  | _ => none

def shape? : PT → Option Shape
  | .atom "target" => some .target
  | .atom "excess" => some .excess
  | .atom "deficit" => some .deficit
  | _ => none

def term? : PT → Option Term
  | .app "t" [m, tg, w, sh] => do pure ⟨← m.nat?, ← tg.int?, ← w.nat?, ← shape? sh⟩
  | _ => none

def req? : PT → Option (Nat × Nat)
  | .app "p" [p, u] => do pure (← p.nat?, ← u.nat?)
  | _ => none

def recipe? : PT → Option (Recipe String)
  | .app "r" [a, x, rs, cs, mu, rq] => do
    pure ⟨← a.atom?, ← x.atom?, ← ints rs, ← ints cs, ← mu.nat?, ← (← rq.list?).mapM req?⟩
  | _ => none

def config? : PT → Option (Cfg String)
  | .app "config" [o, age, ms, ts, rs] => do
    pure ⟨← o.atom?, ← age.nat?, ← (← ms.list?).mapM metric?, ← (← ts.list?).mapM term?,
      ← (← rs.list?).mapM recipe?⟩
  | _ => none

def status? : PT → Option Status
  | .atom "reserved" => some .reserved
  | .atom "dispatched" => some .dispatched
  | .atom "active" => some .active
  | .atom "unknown" => some .unknown
  | .atom "completed" => some .completed
  | .atom "cancelled" => some .cancelled
  | _ => none

/-- What the trace shows of a state (`s/7`): values, version, observed_at,
paused, each retained record's (recipe, units, expires_at, status) (the
window `State.win`: retained record `k` has id `first_id + k`), first_id,
and completed_units per recipe. -/
abbrev Obs :=
  List Int × Nat × Nat × Bool × List (Nat × Nat × Nat × Status) × Nat × List Nat

def crec? : PT → Option (Nat × Nat × Nat × Status)
  | .app "c" [r, u, e, st] => do pure (← r.nat?, ← u.nat?, ← e.nat?, ← status? st)
  | _ => none

def state? : PT → Option Obs
  | .app "s" [vs, ver, obs, p, cs, fid, cu] => do
    pure (← ints vs, ← ver.nat?, ← obs.nat?, ← p.bool?, ← (← cs.list?).mapM crec?,
      ← fid.nat?, ← (← cu.list?).mapM PT.nat?)
  | _ => none

/-- The model's state as the trace shows it: the window of the history, and
`completed_units` as `doneU` (the lifetime count, saturated) per recipe. -/
def obs {A : Type} (cfg : Cfg A) (s : State) : Obs :=
  (s.values, s.version, s.observedAt, s.paused,
   s.win.map (fun c => (c.recipe, c.units, c.expiresAt, c.status)),
   s.firstId, (List.range cfg.recipes.length).map (doneU s))

def op? : PT → Option (Op String)
  | .app "submit" [c, r, u, v, e, n] => do
    pure (.submit (← c.atom?) (← r.nat?) (← u.nat?) (← v.nat?) (← e.nat?) (← n.nat?))
  | .app "dispatch" [c, i, n] => do pure (.dispatch (← c.atom?) (← i.nat?) (← n.nat?))
  | .app "ack" [i, a] => do pure (.ack (← i.nat?) (← a.bool?))
  | .app "cancel" [c, i, n] => do pure (.cancel (← c.atom?) (← i.nat?) (← n.nat?))
  | .app "settle" [c, i, d] => do pure (.settle (← c.atom?) (← i.nat?) (← d.bool?))
  | .app "pause" [c, b] => do pure (.pause (← c.atom?) (← b.bool?))
  | .app "observe" [c, vs, n] => do pure (.observe (← c.atom?) (← ints vs) (← n.nat?))
  | _ => none

/-! ## Printing -/

/-- `err_name`: the error's snake_case name. -/
def errName : Err → String
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

def res? : PT → Option Res
  | .atom "ok" => some .ok
  | .app "ok" [i] => do pure (.okId (← i.nat?))
  | .app "err" [.atom e] =>
    [Err.invalidConfig, .unauthorized, .paused, .stale, .expired, .snapshotExpired,
     .invalidRecipe, .invalidUnits, .prerequisite, .full, .arithmetic, .bounds, .energy,
     .invalidState, .notFound, .pending].find? (errName · == e) |>.map .err
  | _ => none

def resText : Res → String
  | .ok => "ok"
  | .okId i => s!"ok({i})"
  | .err e => s!"err({errName e})"

def statusName : Status → String
  | .reserved => "reserved"
  | .dispatched => "dispatched"
  | .active => "active"
  | .unknown => "unknown"
  | .completed => "completed"
  | .cancelled => "cancelled"

def intsText (v : List Int) : String := "[" ++ ",".intercalate (v.map toString) ++ "]"

def obsText (o : Obs) : String :=
  let (vs, ver, ob, p, cs, fid, cu) := o
  let recs := cs.map fun (r, u, e, st) => s!"c({r},{u},{e},{statusName st})"
  s!"s({intsText vs},{ver},{ob},{p},[{",".intercalate recs}],{fid},[{",".intercalate (cu.map toString)}])"

end Pool.Text
