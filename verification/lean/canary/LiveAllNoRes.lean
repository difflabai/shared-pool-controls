/-
Canary (run.sh canary): this file must FAIL to compile. LIVE-ALL without
INV-RES is false: the v1.0.0 wedge satisfies every other conjunct of `Inv`
and its executor's settle fails (`Witness.res_needed`,
`Witness.live_all_needs_res`), so no automation may prove it; if it
compiles, the proof of LIVE-ALL no longer rests on INV-RES.
-/
import PoolLean

open Pool in
theorem canary_live_all_no_res {cfg : Cfg Nat} (hV : Valid cfg) {s : State}
    (W : WF cfg s) (L : Ledger cfg s) (P : PvB cfg s) (D : Dep cfg s) (V : Ver s)
    (VC : VerCap s) {id : Nat} (hk : id < s.records.length)
    (hst : inFlight s.records[id].status = true) :
    outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none := by
  first | exact ((live_all hV ⟨W, L, by grind, P, D, V, VC⟩ hk).2 hst).1 | grind | simp_all
