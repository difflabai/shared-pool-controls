/-
Canary (run.sh canary): this file must FAIL to compile. Not every state
satisfies `Inv` (values that disagree with the ledger, for one), so no
automation may prove it; if it compiles, `Inv` has become trivial.
-/
import PoolLean

open Pool in
theorem canary_every_state_inv (cfg : Cfg Nat) (s : State) : Inv cfg s := by
  first | exact reach_inv (by grind) (by grind) | grind | simp_all
