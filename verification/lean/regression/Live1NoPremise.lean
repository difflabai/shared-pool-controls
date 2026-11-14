/-
Regression (run.sh regression): this file must COMPILE. It is the v1.0.0
canary `canary/Live1NoPremise.lean`, which had to FAIL to compile there:
LIVE-1 without its premise B = 0 was false on v1.0.0 (F-1: the wedge is a
reachable `Inv` state in which the executor's settle fails, and F-5's
`u32::MAX` was the other premise). On v1.1.0 it is a theorem (`live1`, a
corollary of LIVE-ALL, which needs neither premise): the statement below is
byte-for-byte the v1.0.0 canary's, only the proof changed. FAIL on v1.0.0,
PASS on v1.1.0 (strategy F-1, F-5).
-/
import PoolLean

open Pool in
theorem canary_live1_unconditional {cfg : Cfg Nat} (hV : Valid cfg) {s : State} (I : Inv cfg s)
    (hv : s.version < U32_MAX) {id : Nat} (hk : id < s.records.length)
    (hst : inFlight s.records[id].status = true) :
    outcome cfg s (.settle (cfg.recipe s.records[id].recipe).executor id true) = none := by
  exact live1 hV I hk hst
