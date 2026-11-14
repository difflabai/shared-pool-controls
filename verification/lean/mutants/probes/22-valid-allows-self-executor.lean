/-
Property probe for mutant 22-valid-allows-self-executor
(run.sh mutants): must COMPILE with the mutant applied and FAIL to compile
on the unmutated machine. It states, on a concrete state, the behaviour
the fix removed, so a kill by the probe is a kill by the named property,
not by an exactness check.
-/
import PoolLean.Machine
open Pool
def wcfg : Cfg Nat :=
  { owner := 0, maxAge := 0, metrics := [⟨0, 10, 5⟩, ⟨-100, 100, 5⟩], terms := [⟨1, 0, 1, .target⟩],
    recipes := [⟨1, 2, [-2, 0], [2, -5], 1, []⟩] }
def pcfg : Cfg Nat :=
  { owner := 0, maxAge := 0, metrics := [⟨0, 10, 5⟩], terms := [⟨0, 5, 1, .target⟩],
    recipes := [⟨1, 2, [0], [0], 1, []⟩, ⟨1, 2, [0], [0], 1, [(0, 1)]⟩] }
def mk (recs : List Rec) (fid ver : Nat) : State :=
  { values := [5], version := ver, observedAt := 100, paused := false, records := recs,
    firstId := fid, base := [5], k0 := 0, eff := ver }
def cancelled256 : List Rec := (List.range 256).map (fun j => ⟨j, 0, 1, 200, .cancelled⟩)

-- CFG-2 / AUTH-SELF (FR-5): validate accepts a recipe whose actor is its executor (trace 210).
def scfg : Cfg Nat :=
  { owner := 0, maxAge := 0, metrics := [⟨0, 10, 5⟩], terms := [⟨0, 0, 1, .excess⟩],
    recipes := [⟨1, 1, [-1], [0], 2, []⟩, ⟨0, 0, [-1], [0], 2, []⟩] }
example : validate scfg = true := by decide
