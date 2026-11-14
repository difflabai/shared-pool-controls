------------------------------- MODULE Props -------------------------------
(***************************************************************************)
(* Properties of SharedPool. IDs are the shared-pool catalogue's (brief:   *)
(* "Property catalogue"); each definition is tagged PROP: <ID>.            *)
(*                                                                         *)
(* Two groups of state invariants. InvRel reads version, firstId and k0    *)
(* only relative to each other or through the version cap (false in every  *)
(* state MC_Safety explores), so it is a function of MC_Safety's VIEW.     *)
(* InvAbs reads absolute versions and ids (INV-VERCAP, WIN-1's             *)
(* firstId + NR <= version); harnesses without a VIEW check it (MC_Cap,    *)
(* the liveness configs, TraceCheck).                                      *)
(***************************************************************************)
EXTENDS SharedPool

\* B_i = PV_i(base): the violation of metric m at the last re-basing observe
\* (or new).
B(m) == PV(base, m)
Healthy == \A m \in MIdx : B(m) = 0

\* Units of Completed records of recipe p among the retained records.
WinDone(p) ==
    SumTo([k \in 1 .. NR |->
              IF records[k].recipe = p /\ records[k].status = "Completed"
              THEN records[k].units ELSE 0], NR)

\* rem (strategy 2.2): the most version increments an open record can still
\* take: Reserved 3 (dispatch below the cap, then as Dispatched), Dispatched
\* 3 (ack Unknown, ack Active, settle), Unknown 2, Active 1, terminal 0.
Rem(s) ==
    CASE s = "Reserved" -> 3 [] s = "Dispatched" -> 3 [] s = "Unknown" -> 2
      [] s = "Active" -> 1 [] OTHER -> 0
RemSum == SumTo([k \in 1 .. NR |-> Rem(records[k].status)], NR)

\* The properties restate the core's conditions they rely on rather than
\* reuse the model's operators, so a mutant of the model's operator is
\* checked against the statement: Full (U:287-289), the in-flight
\* statuses (an open record that is not Reserved) and the envelope
\* (HullLo, HullHi below).
FullCond == NR >= MaxRecords /\ Open(records[1].status)
InFlightSpec(s) == s \in {"Dispatched", "Active", "Unknown"}

-----------------------------------------------------------------------------
(* State invariants                                                        *)

\* PROP: INV-WF WIN-1. Shapes and bounds of every field. Ids are implicit
\* (records[k] has id firstId + k - 1), the window holds at most MaxRecords
\* records, completed_units has one entry per recipe, saturates at DoneCap
\* and covers the window's Completed units. Config never changes is StepCfg
\* below; ValidCfg(cfg) is CFG-1's "init satisfies INV". WIN-1's
\* firstId + NR <= version is InvWinVer (absolute).
InvWF ==
    /\ ValidCfg(cfg)
    /\ DOMAIN values = MIdx /\ \A m \in MIdx : Bounded(values[m])
    /\ version \in Nat /\ observedAt \in Nat /\ paused \in BOOLEAN
    /\ firstId \in Nat /\ NR <= MaxRecords
    /\ \A k \in 1 .. NR :
        LET c == records[k] IN
        /\ c.recipe \in 0 .. NRcp - 1
        /\ c.units \in 1 .. Rcp(c.recipe).maxUnits
        /\ c.exp \in Nat
        /\ c.status \in Statuses
    /\ DOMAIN doneU = 1 .. NRcp
    /\ \A p \in 1 .. NRcp : doneU[p] \in 0 .. DoneCap /\ doneU[p] >= Min(DoneCap, WinDone(p - 1))
    /\ DOMAIN base = MIdx /\ k0 \in firstId .. firstId + NR /\ steps \in Nat

\* PROP: WIN-1. submit's id addition cannot overflow (U:363-366).
InvWinVer == firstId + NR <= version

\* PROP: INV-LEDGER. values = base + the contributions of the retained
\* records with id >= k0 (records pruned since are folded into base).
Contribution(m) ==
    SumTo([k \in 1 .. NR |-> IF firstId + k - 1 >= k0 THEN Contrib(records[k], m) ELSE 0], NR)
InvLedger == \A m \in MIdx : values[m] = base[m] + Contribution(m)

\* The release-aware envelope of the open records around the current
\* values (the hull of every outcome: each record stays open, completes,
\* d = complete * u, or releases, x = -reserve * u), restated rather than
\* read from the model's LoTerm/HiTerm, so a mutant of the admission
\* envelope is checked against the statement.
HullTerm(k, m, neg) ==
    LET c == records[k]  rc == Rcp(c.recipe)
        d == rc.complete[m] * c.units  x == -(rc.reserve[m] * c.units)
    IN IF ~Open(c.status) THEN 0
       ELSE IF neg THEN Min(0, Min(d, x)) ELSE Max(0, Max(d, x))
HullLo(m) == values[m] + SumTo([k \in 1 .. NR |-> HullTerm(k, m, TRUE)], NR)
HullHi(m) == values[m] + SumTo([k \in 1 .. NR |-> HullTerm(k, m, FALSE)], NR)
HullEV(m) == Max0(Mn(m) - HullLo(m)) + Max0(HullHi(m) - Mx(m))

\* PROP: INV-RES. The release-aware envelope of the open records is
\* computable (endpoints within +-MAX_VALUE) and each metric's envelope
\* violation equals its current violation.
CheckEnvOk == \A m \in MIdx : Bounded(HullLo(m)) /\ Bounded(HullHi(m))
InvRes == CheckEnvOk /\ \A m \in MIdx : HullEV(m) = PV(values, m)

\* PROP: INV-HARD. The envelope's violation never exceeds the violation at
\* the last re-basing observe (a corollary of INV-RES and HARD-STEP).
InvHard == CheckEnvOk /\ \A m \in MIdx : HullEV(m) <= B(m)

\* PROP: INV-HEALTHY (InvOutcomes). Every combination of outcomes of the
\* open records (complete, release, or none) has no metric's violation
\* above its violation now; with B = 0 every combination lands inside
\* [min, max]. Enumerates 3^|open| combinations, so the model-checking
\* configs check it; TraceCheck's Inv omits it.
OpenIds == {id \in WinIds : Open(Rec(id).status)}
Outcome(f, m) ==
    values[m] + SetSum(OpenIds,
        [id \in OpenIds |->
            CASE f[id] = "complete" -> RecRcp(id).complete[m] * Rec(id).units
              [] f[id] = "release"  -> -(RecRcp(id).reserve[m] * Rec(id).units)
              [] OTHER              -> 0])
InvOutcomes ==
    \A f \in [OpenIds -> {"none", "complete", "release"}] : \A m \in MIdx :
        LET v == Outcome(f, m) IN
        /\ Max0(Mn(m) - v) + Max0(v - Mx(m)) <= PV(values, m)
        /\ Healthy => Mn(m) <= v /\ v <= Mx(m)

\* PROP: INV-DEP. Every retained record's prerequisites are met by the
\* lifetime completed units (completed_units only grows).
InvDep ==
    \A k \in 1 .. NR : \A q \in DOMAIN Rcp(records[k].recipe).requires :
        LET rq == Rcp(records[k].recipe).requires[q] IN doneU[rq[1] + 1] >= rq[2]

\* PROP: INV-VER. version = number of effective transitions (from the
\* initial version).
InvVer == version = steps

\* PROP: INV-VERCAP. version + the open records' remaining increments
\* <= u32::MAX (NO-OVF: ack, settle and cancel never fail Arithmetic).
InvVerCap == RemSum <= MaxVersion - version

\* PROP: TERM-1 (error view). A terminal record: the same terminal again is
\* an Ok no-op, the opposite one InvalidState, also for cancel of a
\* Completed record (U:467).
Term1 ==
    \A id \in WinIds :
        LET x == RecRcp(id).executor IN
        /\ Rec(id).status = "Completed" =>
              /\ SettleErr(x, id, TRUE) = "Noop" /\ SettleErr(x, id, FALSE) = "InvalidState"
              /\ CancelErr(cfg.owner, id, 0) = "InvalidState"
        /\ Rec(id).status = "Cancelled" =>
              /\ SettleErr(x, id, FALSE) = "Noop" /\ SettleErr(x, id, TRUE) = "InvalidState"
              /\ CancelErr(cfg.owner, id, 0) = "Noop"

\* PROP: LIVE-ALL. No premise: every Reserved record can be cancelled by
\* its actor and by the owner, and every in-flight record can be settled
\* by its executor either way.
LiveAll ==
    \A id \in WinIds :
        LET s == Rec(id).status  x == RecRcp(id).executor IN
        /\ s = "Reserved" =>
              CancelErr(RecRcp(id).actor, id, 0) = "Ok" /\ CancelErr(cfg.owner, id, 0) = "Ok"
        /\ InFlightSpec(s) => SettleErr(x, id, TRUE) = "Ok" /\ SettleErr(x, id, FALSE) = "Ok"

\* PROP: LIVE-1 (corollary of LIVE-ALL; v1.0's premises B = 0 and
\* version < u32::MAX dropped). The executor can complete every in-flight
\* record.
Live1 ==
    \A id \in WinIds : InFlightSpec(Rec(id).status) => SettleErr(RecRcp(id).executor, id, TRUE) = "Ok"

\* PROP: LIVE-ALL (NoWedge). Neither wedge of v1.0 exists: F-1 (an
\* in-flight record neither settle can resolve) and F-2 (an in-flight
\* record whose honest settle(false) fails).
NoWedge ==
    \A id \in WinIds :
        LET x == RecRcp(id).executor IN
        InFlightSpec(Rec(id).status) =>
            /\ ~(SettleErr(x, id, TRUE) # "Ok" /\ SettleErr(x, id, FALSE) # "Ok")
            /\ SettleErr(x, id, FALSE) = "Ok"

\* PROP: WIN-ESC. submit fails Full only when the window is full and its
\* oldest record is open, and then that record can be resolved (cancel if
\* Reserved, settle either way if in flight), after which the Full guard
\* passes.
WinEsc ==
    /\ \A r \in 0 .. NRcp - 1 :
          SubmitErr(Rcp(r).actor, r, 1, version, observedAt + 1, observedAt) = "Full" => FullCond
    /\ FullCond =>
          LET f == firstId  x == RecRcp(firstId).executor IN
          /\ Rec(f).status = "Reserved" => CancelErr(cfg.owner, f, 0) = "Ok"
          /\ InFlightSpec(Rec(f).status) => SettleErr(x, f, TRUE) = "Ok" /\ SettleErr(x, f, FALSE) = "Ok"

\* PROP: WIN-2 (error view). A pruned id (the one below the window) is
\* NotFound for every call on a record.
Win2 ==
    firstId > 0 =>
        LET p == firstId - 1 IN
        /\ DispatchErr(Rcp(0).actor, p, observedAt) = "NotFound"
        /\ AckErr(p, TRUE) = "NotFound" /\ CancelErr(cfg.owner, p, 0) = "NotFound"
        /\ SettleErr(Rcp(0).executor, p, TRUE) = "NotFound"

\* PROP: REFRESH-1. Below the cap the owner can always restate the current
\* values (renewing the snapshot), whatever is open.
RefreshAlways == version < VersionCap => ObserveErr(cfg.owner, values, 0) = "Ok"

\* View-safe invariants (MC_Safety); absolute ones; all but INV-HEALTHY
\* (TraceCheck checks this on every core state).
InvRel ==
    /\ InvWF /\ InvLedger /\ InvRes /\ InvHard /\ InvDep /\ InvVer /\ Term1
    /\ LiveAll /\ Live1 /\ NoWedge /\ WinEsc /\ Win2 /\ RefreshAlways
InvAbs == InvVerCap /\ InvWinVer
Inv    == InvRel /\ InvAbs
\* MC_Safety (VIEW): everything view-safe, with INV-HEALTHY.
InvMC  == InvRel /\ InvOutcomes
\* MC_Cap and the liveness configs (no VIEW): everything.
InvCapMC == Inv /\ InvOutcomes

-----------------------------------------------------------------------------
(* Step properties, checked as [][StepProps]_vars. Every effective step    *)
(* changes version, so no effective step is a stutter of vars. L is the    *)
(* call that made the step; L.i is a record id.                            *)

L == last'

\* PROP: INV-WF (config never changes).
StepCfg == cfg' = cfg

\* PROP: TX-VER. Every effective step increments version by exactly 1.
TxVer == version' = version + 1 /\ steps' = steps + 1

\* PROP: VER-CAP. Submit, dispatch, pause and observe take effect only
\* below VERSION_CAP; ack, cancel and settle use the plain bump (TX-VER).
VerCapStep == L.op \in {"submit", "dispatch", "pause", "observe"} => version < VersionCap

\* PROP: LIFE-1 WIN-2. Status graph, terminal absorbing; the window is
\* appended to, or (submit) its terminal oldest record is pruned and a
\* record appended; retained records keep recipe/units/expires_at; new
\* records are Reserved.
Edges ==
    { <<"Reserved", "Dispatched">>, <<"Reserved", "Cancelled">>,
      <<"Dispatched", "Active">>, <<"Dispatched", "Unknown">>, <<"Unknown", "Active">> }
    \cup { <<s, t>> : s \in {"Dispatched", "Active", "Unknown"}, t \in {"Completed", "Cancelled"} }
Life1 ==
    LET drop == firstId' - firstId IN
    /\ drop \in {0, 1}
    /\ drop = 1 => NR = MaxRecords /\ ~Open(records[1].status) /\ L.op = "submit"
    /\ Len(records') \in {NR - drop, NR + 1 - drop}
    /\ \A k \in 1 + drop .. NR :
        LET c == records[k]  d == records'[k - drop] IN
        /\ d.recipe = c.recipe /\ d.units = c.units /\ d.exp = c.exp
        /\ d.status = c.status \/ <<c.status, d.status>> \in Edges
    /\ \A k \in NR + 1 - drop .. Len(records') : records'[k].status = "Reserved"

\* PROP: STALE-1. An accepted request carries the current version, and
\* versions strictly increase, so it cannot be accepted again.
Stale1 == (L.op = "submit" => L.v = version) /\ version' > version

\* PROP: AUTH-1. Who may make each effective call.
Auth1 ==
    /\ L.op \in {"submit", "dispatch"} => L.c = Rcp(L.r).actor
    /\ L.op = "cancel" => L.c \in {Rcp(L.r).actor, cfg.owner}
    /\ L.op = "settle" => L.c = Rcp(L.r).executor
    /\ L.op \in {"pause", "observe"} => L.c = cfg.owner

\* PROP: ADM-1. An accepted reservation does not increase energy.
Adm1 == L.op = "submit" => Energy(values') <= Energy(values)

\* PROP: ADM-2. In a violating state, the reservation plus its completion
\* strictly decreases energy.
Adm2 ==
    (L.op = "submit" /\ Violating) =>
        Energy(AddVec(values', Rcp(L.r).complete, L.u)) < Energy(values)

\* PROP: HARD-STEP. Every effective call but observe: no metric's violation
\* grows.
HardStep == L.op # "observe" => \A m \in MIdx : PV(values', m) <= PV(values, m)

\* PROP: DEP-1. An accepted request's prerequisites are met by the
\* lifetime completed units.
Dep1 ==
    L.op = "submit" =>
        \A q \in DOMAIN Rcp(L.r).requires :
            doneU[Rcp(L.r).requires[q][1] + 1] >= Rcp(L.r).requires[q][2]

\* PROP: WIN-1. completed_units changes only by an effective settle(true),
\* which adds the record's units, saturating.
DoneStep ==
    doneU' = IF L.op = "settle" /\ L.b
             THEN [doneU EXCEPT ![L.r + 1] = Min(@ + L.u, DoneCap)]
             ELSE doneU

\* PROP: ASYNC-1. acknowledge changes only the record's status (between
\* in-flight statuses), never values: Active and Unknown keep the
\* reservation (INV-LEDGER counts them).
Async1 ==
    L.op = "ack" =>
        /\ values' = values /\ observedAt' = observedAt /\ paused' = paused
        /\ firstId' = firstId /\ doneU' = doneU
        /\ Len(records') = NR
        /\ \A k \in 1 .. NR : k # Pos(L.i) => records'[k] = records[k]
        /\ InFlightSpec(records'[Pos(L.i)].status)

\* PROP: OBS-1. observe keeps config and records, and changes the values
\* only with no open record.
Obs1 ==
    L.op = "observe" =>
        /\ (\A k \in 1 .. NR : ~Open(records[k].status)) \/ values' = values
        /\ records' = records /\ firstId' = firstId /\ cfg' = cfg

\* PROP: OBS-2. observe with an open record (a refresh) changes only
\* observed_at and version: values, records, completed_units and the
\* ghost base and k0 stay.
Obs2 ==
    (L.op = "observe" /\ AnyOpen) =>
        /\ values' = values /\ records' = records /\ firstId' = firstId /\ doneU' = doneU
        /\ paused' = paused /\ base' = base /\ k0' = k0

\* PROP: TX-SUBMIT WIN-ESC. The time, window and unit guards of an
\* effective submit held in the pre-state, it got id firstId + NR, and the
\* window was appended to or pruned (the other guards are STALE-1, AUTH-1,
\* DEP-1, ADM-1, ADM-2, HARD-STEP, PAUSE-1 and VER-CAP).
Fresh(now) == cfg.maxAge = 0 \/ now <= observedAt + cfg.maxAge
TxSubmit ==
    L.op = "submit" =>
        LET new == [recipe |-> L.r, units |-> L.u, exp |-> L.exp, status |-> "Reserved"] IN
        /\ L.now < L.exp /\ L.exp <= L.now + Window /\ Fresh(L.now)
        /\ ~FullCond /\ L.r < NRcp /\ L.u \in 1 .. Rcp(L.r).maxUnits
        /\ L.i = firstId + NR
        /\ IF NR >= MaxRecords
           THEN records' = Append(Tail(records), new) /\ firstId' = firstId + 1
           ELSE records' = Append(records, new) /\ firstId' = firstId

\* PROP: TX-DISPATCH. An effective dispatch: the record was retained,
\* Reserved and unexpired, the snapshot fresh, and only its status changed.
TxDispatch ==
    L.op = "dispatch" =>
        /\ Found(L.i) /\ Rec(L.i).status = "Reserved" /\ L.now < Rec(L.i).exp /\ Fresh(L.now)
        /\ records' = [records EXCEPT ![Pos(L.i)].status = "Dispatched"]
        /\ values' = values /\ firstId' = firstId /\ doneU' = doneU

\* PROP: PAUSE-1. Paused: no submit or dispatch takes effect. (That settle,
\* ack, cancel and observe still do is WitStep_Paused*.)
Pause1 == paused => L.op \notin {"submit", "dispatch"}

StepProps ==
    /\ StepCfg /\ TxVer /\ VerCapStep /\ Life1 /\ Stale1 /\ Auth1 /\ Adm1 /\ Adm2
    /\ HardStep /\ Dep1 /\ DoneStep /\ Async1 /\ Obs1 /\ Obs2 /\ TxSubmit /\ TxDispatch
    /\ Pause1
StepProperty == [][StepProps]_vars

-----------------------------------------------------------------------------
(* PROP: TLA-WIT                                                           *)
(* Non-vacuity (TLA-WIT). Each must be reachable, so the runner checks     *)
(* them as invariants or step properties that must be VIOLATED.            *)

\* A record completed.
Witness_Completed == ~\E id \in WinIds : Rec(id).status = "Completed"
\* A record with a prerequisite was admitted (prerequisite met).
Witness_Prerequisite == ~\E id \in WinIds : Len(RecRcp(id).requires) > 0
\* An observe left a metric out of bounds (B # 0).
Witness_Disturbed == Healthy
\* A submit pruned a terminal oldest record (WIN-ESC, WIN-2).
Witness_Prune == firstId = 0
\* Full: the window is full with an open oldest record while a later one
\* is terminal (the Full guard reads the front, not the count).
Witness_FullOpenFront ==
    ~(FullCond /\ \E k \in 2 .. NR : ~Open(records[k].status))
\* cancel of a Completed record is InvalidState (TERM-1).
Witness_CancelCompletedInvalid ==
    ~\E id \in WinIds : CancelErr(cfg.owner, id, 0) = "InvalidState"
\* A submit that v1.0's check (completion-only envelope, compared with the
\* violation now only) would pass is Bounds: the release terms or the
\* candidate's violation decide.
V10Low(cand, r, u, m) ==
    cand[m] + SumTo([k \in 1 .. NR |->
                        LET c == records[k] IN
                        IF Open(c.status) THEN Min0(Rcp(c.recipe).complete[m] * c.units) ELSE 0], NR)
            + Min0(Rcp(r).complete[m] * u)
V10High(cand, r, u, m) ==
    cand[m] + SumTo([k \in 1 .. NR |->
                        LET c == records[k] IN
                        IF Open(c.status) THEN Max0(Rcp(c.recipe).complete[m] * c.units) ELSE 0], NR)
            + Max0(Rcp(r).complete[m] * u)
V10BoundsOk(r, u) ==
    LET cand == AddVec(values, Rcp(r).reserve, u) IN
    \A m \in MIdx :
        Max0(Mn(m) - V10Low(cand, r, u, m)) + Max0(V10High(cand, r, u, m) - Mx(m)) <= PV(values, m)
Witness_SubmitRejectedHull ==
    ~\E r \in 0 .. NRcp - 1, u \in 1 .. MaxU :
        /\ u <= Rcp(r).maxUnits
        /\ SubmitErr(Rcp(r).actor, r, u, version, observedAt + 1, observedAt) = "Bounds"
        /\ V10BoundsOk(r, u)

\* Step witnesses (checked on every transition, VIEW or not).
\* Cancelled after dispatch: settle(false) of an in-flight record.
WitStep_CancelledAfterDispatch ==
    [][~(L.op = "settle" /\ ~L.b /\ InFlightSpec(Rec(L.i).status))]_vars
\* Unknown -> Active (a late positive acknowledgement).
WitStep_UnknownToActive ==
    [][~(L.op = "ack" /\ Rec(L.i).status = "Unknown" /\ L.b)]_vars
\* Recovery admission: a submit accepted in a violating state.
WitStep_Recovery == [][~(L.op = "submit" /\ Violating)]_vars
\* REFRESH-1, OBS-2: an observe took effect with a record open.
WitStep_Refresh == [][~(L.op = "observe" /\ AnyOpen)]_vars
\* A submit was accepted on a prerequisite whose Completed records were all
\* pruned (completed_units, not the window, decides).
WitStep_PrereqAfterPrune ==
    [][~(/\ L.op = "submit"
         /\ Len(Rcp(L.r).requires) > 0
         /\ \A q \in DOMAIN Rcp(L.r).requires : WinDone(Rcp(L.r).requires[q][1]) = 0)]_vars
\* PAUSE-1's second half: settle, ack, cancel and observe still take effect
\* while paused (one witness each).
PausedStep(op) == paused /\ paused' /\ L.op = op
WitStep_PausedSettle  == [][~PausedStep("settle")]_vars
WitStep_PausedAck     == [][~PausedStep("ack")]_vars
WitStep_PausedCancel  == [][~PausedStep("cancel")]_vars
WitStep_PausedObserve == [][~PausedStep("observe")]_vars

\* MC_Cap: the cap reached with a record open (ack, cancel and settle
\* still take effect, LIVE-ALL and INV-VERCAP at the cap).
Witness_AtCap == ~(version >= VersionCap /\ AnyOpen)
\* MC_Cap: a record resolved at or above the cap.
WitStep_ResolveAtCap ==
    [][~(version >= VersionCap /\ L.op \in {"ack", "cancel", "settle"})]_vars

=============================================================================
