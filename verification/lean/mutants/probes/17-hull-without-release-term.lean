/-
Property probe for mutant 17-hull-without-release-term
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

-- LIVE-ALL (FR-2): the F-2 shape. With the release term dropped, a hedge is admitted on the
-- strength of an open position, and then cancelling the position fails Bounds.
def f2cfg : Cfg Nat :=
  { owner := 0, maxAge := 300,
    metrics := [⟨0, 100, 60⟩, ⟨0, 80, 40⟩, ⟨0, 60, 30⟩, ⟨0, 40, 40⟩],
    terms := [⟨0, 80, 1, .excess⟩, ⟨1, 60, 1, .excess⟩, ⟨2, 40, 1, .excess⟩],
    recipes := [⟨1, 2, [20, 5, 0, -5], [0, 0, 0, 0], 20, []⟩, ⟨3, 2, [10, 0, 5, -5], [0, 0, 0, 0], 20, []⟩,
                ⟨3, 2, [0, 10, 5, -5], [0, 0, 0, 0], 20, []⟩, ⟨3, 2, [0, 0, 0, -2], [-10, 0, 0, 0], 20, []⟩] }
def s2 := run f2cfg (init f2cfg 100) [.submit 1 0 1 0 200 100, .submit 3 3 8 1 200 100]
example : s2.records.length = 2 ∧ (exec f2cfg s2 (.cancel 1 0 101)).1 = .err .bounds := by decide
