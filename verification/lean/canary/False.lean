/-
Canary (run.sh canary): this file must FAIL to compile. If it ever
compiles, the checker or its configuration accepts false statements and no
result of this layer can be trusted.
-/
import PoolLean

theorem canary_false : False := by
  first | decide | simp | omega | grind
