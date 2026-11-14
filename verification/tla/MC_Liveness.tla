---------------------------- MODULE MC_Liveness ----------------------------
(***************************************************************************)
(* LIVE-2: every dispatched record eventually ends (Completed or           *)
(* Cancelled), under weak fairness of the executor. What "fairness of the  *)
(* executor" means decided the answer at v1.0.0 (findings F-1, F-2), so    *)
(* each config fixes it. At v1.1.0 every one holds (LIVE-ALL: both settles *)
(* of an in-flight record are Ok in every reachable state):                *)
(*                                                                         *)
(*   TLA-LIVE-1  the completion path (MC_Liveness.cfg, universe MCConfig;  *)
(*               MC_Liveness_F2Done.cfg, universe CreditHedgeConfig):      *)
(*               observes stay inside [min, max] and the executor is       *)
(*               weakly fair to COMPLETING each record (SettleDone).       *)
(*               MC_Liveness_Unfair.cfg: without fairness LIVE-2 fails     *)
(*               (non-vacuity).                                            *)
(*   TLA-LIVE-F1 (MC_Liveness_F1.cfg) disturbing observes are allowed, and *)
(*               the executor is weakly fair to settling each record       *)
(*               either way (SettleAny): LIVE-2 holds (v1.0.0: violated by *)
(*               the F-1 wedge).                                           *)
(*   TLA-LIVE-F2 (MC_Liveness_F2.cfg, universe CreditHedgeConfig) only     *)
(*               in-bounds observes, the executor is weakly fair to        *)
(*               reporting NONEXECUTION (SettleFalse: the honest           *)
(*               settle(false) of ASM-EXEC) and actors/owner are weakly    *)
(*               fair to cancelling Reserved records: LIVE-2 holds         *)
(*               (v1.0.0: violated by the F-2 wedge; at v1.1.0 the hedge   *)
(*               that relied on the credit's reservation is refused at     *)
(*               submit).                                                  *)
(*   WIN-LIVE    (MC_Liveness_Win.cfg, MaxRecords 1; also                  *)
(*               MC_Liveness_Win2.cfg, MaxRecords 2) a Full window is      *)
(*               eventually not Full, under weak fairness of resolving its *)
(*               oldest record (ResolveFront: cancel or settle either      *)
(*               way).                                                     *)
(*                                                                         *)
(* The behaviours are MC_Cap's (Pool::new at VersionCap - H): calls that   *)
(* start work (submit, dispatch, pause, observe) stop at the cap, so every *)
(* behaviour has finitely many effective steps (then stutters) and TLC's   *)
(* liveness check is sound without a VIEW or CONSTRAINT. At most H records *)
(* are ever submitted, so their ids are 0 .. H - 1; a pruned id counts as  *)
(* terminal (WIN-2: only a terminal record is pruned).                     *)
(* Scaled (C-5): MaxRecords 2 (real 256; WIN-LIVE at 1 and 2), H 4; 1-2    *)
(* metrics, 2 recipes.                                                     *)
(***************************************************************************)
EXTENDS MC_Cap

RecIds == 0 .. H - 1
InFlightRec(id) == Found(id) /\ InFlight(Rec(id).status)
TerminalRec(id) == id < firstId \/ (Found(id) /\ ~Open(Rec(id).status))

\* The executor completes record id / reports its nonexecution / settles it
\* either way; some caller cancels it (only a Reserved record can be).
SettleDone(id)  == Found(id) /\ \E x \in Accounts : Settle(x, id, TRUE)
SettleFalse(id) == Found(id) /\ \E x \in Accounts : Settle(x, id, FALSE)
SettleAny(id)   == Found(id) /\ \E x \in Accounts, b \in BOOLEAN : Settle(x, id, b)
CancelAny(id)   == Found(id) /\ \E c \in Accounts, now \in Times : Cancel(c, id, now)

\* PROP: TLA-LIVE-1
\* (the completion path)
LiveSpec   == CapInit /\ [][Next]_vars /\ \A id \in RecIds : WF_vars(SettleDone(id))
UnfairSpec == CapSpec
\* PROP: TLA-LIVE-F1
F1Spec     == CapInit /\ [][Next]_vars /\ \A id \in RecIds : WF_vars(SettleAny(id))
\* PROP: TLA-LIVE-F2
F2Spec     == CapInit /\ [][Next]_vars
                      /\ \A id \in RecIds : WF_vars(SettleFalse(id)) /\ WF_vars(CancelAny(id))

\* PROP: LIVE-2
Live2 == \A id \in RecIds : [](InFlightRec(id) => <>TerminalRec(id))

\* Resolving the window's oldest record: cancel it or settle it either way.
ResolveFront == NR > 0 /\ (CancelAny(firstId) \/ SettleAny(firstId))
\* PROP: WIN-LIVE
WinSpec == CapInit /\ [][Next]_vars /\ WF_vars(ResolveFront)
\* PROP: WIN-LIVE
WinLive == [](WinFull => <>~WinFull)
=============================================================================
