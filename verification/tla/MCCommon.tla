------------------------------ MODULE MCCommon ------------------------------
(***************************************************************************)
(* The small universes shared by the TLC models.                           *)
(*                                                                         *)
(* MCConfig (the brief's F-1 wedge with its ranges scaled down; MC_Safety  *)
(* and the F-1 / completion-path liveness checks):                         *)
(*   metrics  m0 in [0, 2] (initial 1), m1 in [-2, 2] (initial 1)          *)
(*   terms    t(1, target 0, weight 1)                                     *)
(*   recipes  r0 = hedge:  actor a1, executor x1, reserve [-1, 0],         *)
(*                         complete [1, -1], max_units 1                   *)
(*            r1 = follow: actor a1, executor x1, reserve [1, -1],         *)
(*                         complete [-1, 0], max_units 2, requires r0 x 1  *)
(*   max_age 1 (SnapshotExpired reachable)                                 *)
(*                                                                         *)
(* CreditHedgeConfig (finding F-2, the supplied `risk` fixture's credit    *)
(* position and selloff hedge scaled down; the MC_Liveness_F2 configs):    *)
(*   metrics  m0 "selloff loss" in [0, 2] (initial 1)                      *)
(*   terms    t(0, target 2, weight 1, excess): no energy inside bounds,   *)
(*            as the fixture's excess targets above the initial losses     *)
(*   recipes  r0 = credit: actor a1, executor x1, reserve [1],             *)
(*                         complete [0], max_units 1                       *)
(*                         (fixture: reserve +20 on loss, complete 0)      *)
(*            r1 = hedge:  actor a1, executor x1, reserve [0],             *)
(*                         complete [-2], max_units 1                      *)
(*                         (fixture: complete -10 x 8 units on loss)       *)
(*   max_age 0                                                             *)
(* From m0 = 1 the hedge alone is Bounds (envelope low 1 - 2 < 0); after   *)
(* the credit's reservation (m0 = 2) it is admitted (low 0): the hedge's   *)
(* completion relies on the credit's reservation. Once the hedge has       *)
(* completed (m0 = 0), releasing the credit (m0 = -1) fails Bounds.        *)
(*                                                                         *)
(* RecoveryConfig (WitStep_Recovery; MC_Recovery.cfg): MCConfig's metrics *)
(* and term, one recipe r0 = hedge: actor a1, executor x1, reserve [0, -1], *)
(* complete [-1, 0], max_units 1. After observe [3, 2] (m0 = 3 > max 2)   *)
(* the hedge is admitted: its envelope keeps m0's violation at 1 and it    *)
(* lowers the energy. In v1.1 no reservation that strictly reduces a       *)
(* violation is admitted (its own release term returns to the current      *)
(* values), and every MCConfig recipe reserves on m0, so MCConfig has no   *)
(* recovery admission.                                                     *)
(*                                                                         *)
(* F2bConfig (finding F-2b, release-arith; MC_F2b.cfg): m0 in [-1e6, 1e6]  *)
(* (initial 0), m1 in [-2, 2] (initial 0); term t(1, target 0); recipes    *)
(* A: reserve [-1e6, 0], complete [0, 0], max_units 1; B: reserve [0, 0],  *)
(* complete [1e6, 0], max_units 2. v1.0 admits A, then B with 2 units      *)
(* (relying on A's reservation), settles B and then cannot release A       *)
(* (Arithmetic). v1.1 counts A's release term (+1e6) in B's envelope.      *)
(*                                                                         *)
(* Accounts o (owner), a1 (actor), x1 (executor), z (stranger).            *)
(***************************************************************************)
EXTENDS Props, TLC

MCConfig ==
    [owner |-> "o", maxAge |-> 1,
     metrics |-> <<[min |-> 0, max |-> 2, init |-> 1], [min |-> -2, max |-> 2, init |-> 1]>>,
     terms |-> <<[metric |-> 1, target |-> 0, weight |-> 1, shape |-> "target"]>>,
     recipes |->
        <<[actor |-> "a1", executor |-> "x1", reserve |-> <<-1, 0>>, complete |-> <<1, -1>>,
           maxUnits |-> 1, requires |-> <<>>],
          [actor |-> "a1", executor |-> "x1", reserve |-> <<1, -1>>, complete |-> <<-1, 0>>,
           maxUnits |-> 2, requires |-> <<<<0, 1>>>>]>>]

ASSUME ValidCfg(MCConfig)

CreditHedgeConfig ==
    [owner |-> "o", maxAge |-> 0,
     metrics |-> <<[min |-> 0, max |-> 2, init |-> 1]>>,
     terms |-> <<[metric |-> 0, target |-> 2, weight |-> 1, shape |-> "excess"]>>,
     recipes |->
        <<[actor |-> "a1", executor |-> "x1", reserve |-> <<1>>, complete |-> <<0>>,
           maxUnits |-> 1, requires |-> <<>>],
          [actor |-> "a1", executor |-> "x1", reserve |-> <<0>>, complete |-> <<-2>>,
           maxUnits |-> 1, requires |-> <<>>]>>]

ASSUME ValidCfg(CreditHedgeConfig)

\* Every vector over {-1, 0, 2, 3} x {-2, 0, 2}: m0 = -1 and m0 = 3 are out
\* of bounds (a disturbing observe); plus a short vector and one beyond
\* MAX_VALUE (both Bounds).
AllObservations ==
    {<<a, b>> : a \in {-1, 0, 2, 3}, b \in {-2, 0, 2}} \cup {<<0>>, <<1000001, 0>>}
\* Observations inside [min, max] only: B stays 0.
HealthyObservations == {<<a, b>> : a \in {0, 2}, b \in {-2, 0, 2}}
\* Plus the disturbing observe that sets up F-1 (m0 = 3 > max 2).
DisturbingObservations == HealthyObservations \cup {<<3, 2>>}
\* CreditHedgeConfig: every in-bounds vector (B stays 0; no disturbing
\* observe, so F-1 cannot occur).
CreditHedgeObservations == {<<a>> : a \in 0 .. 2}

RecoveryConfig ==
    [owner |-> "o", maxAge |-> 1,
     metrics |-> <<[min |-> 0, max |-> 2, init |-> 1], [min |-> -2, max |-> 2, init |-> 1]>>,
     terms |-> <<[metric |-> 1, target |-> 0, weight |-> 1, shape |-> "target"]>>,
     recipes |->
        <<[actor |-> "a1", executor |-> "x1", reserve |-> <<0, -1>>, complete |-> <<-1, 0>>,
           maxUnits |-> 1, requires |-> <<>>]>>]

ASSUME ValidCfg(RecoveryConfig)

F2bConfig ==
    [owner |-> "o", maxAge |-> 0,
     metrics |-> <<[min |-> -1000000, max |-> 1000000, init |-> 0],
                   [min |-> -2, max |-> 2, init |-> 0]>>,
     terms |-> <<[metric |-> 1, target |-> 0, weight |-> 1, shape |-> "target"]>>,
     recipes |->
        <<[actor |-> "a1", executor |-> "x1", reserve |-> <<-1000000, 0>>, complete |-> <<0, 0>>,
           maxUnits |-> 1, requires |-> <<>>],
          [actor |-> "a1", executor |-> "x1", reserve |-> <<0, 0>>, complete |-> <<1000000, 0>>,
           maxUnits |-> 2, requires |-> <<>>]>>]

ASSUME ValidCfg(F2bConfig)

\* F2bConfig: the initial vector only (observe is a refresh or a re-base to it).
F2bObservations == {<<0, 0>>}

\* PROP: TLA-WIT
\* Witnesses of the F2bConfig universe (MC_F2b.cfg). A value at exactly
\* +-MAX_VALUE is reached (A's reservation: add() accepts the bound).
Witness_EdgeValue == ~\E m \in MIdx : values[m] = -MaxValue \/ values[m] = MaxValue
\* F-2b's admission is refused with Arithmetic: with A open and nothing
\* else applied (values = A's reservation), B x 2 (whose completion v1.0
\* admitted on A's reservation) has the hull's high endpoint -1e6 + 1e6
\* (A's release) + 2e6 (B's completion) = 2e6, while its completion terms
\* alone end at 1e6. So only the release term's endpoint check gives
\* Arithmetic here (without it the call fails Bounds: mutants 24, 34).
Witness_F2bRefused ==
    ~(/\ values = <<-1000000, 0>>
      /\ \E id \in WinIds : Rec(id).recipe = 0 /\ Open(Rec(id).status)
      /\ SubmitErr(Rcp(1).actor, 1, 2, version, observedAt + 1, observedAt) = "Arithmetic")

Init == InitFrom(MCConfig, 0)
=============================================================================
