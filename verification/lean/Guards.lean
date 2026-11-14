/-
pool-lean-guards: LEAN-GUARD, each call's guard order (`Guard.guardOrder`),
one line per call kind, for comparison with the other layers' guard tables
(C-2). A separate executable from `pool-lean` (Main.lean), so that the
replay does not depend on Guard's proofs.
-/
import PoolLean.Guard

open Pool

def main : IO UInt32 := do
  for k in Guard.kinds do
    IO.println s!"lean_guard_order({Guard.kindName k}, [{", ".intercalate ((Guard.guardOrder k).map Guard.outName)}])."
  return 0
