----------------------------- MODULE SharedPool -----------------------------
(***************************************************************************)
(* pool-core of difflabai/shared-pool-controls as a state machine over     *)
(* every call sequence (near-approval strategy.md section 17 pattern).     *)
(*                                                                         *)
(* Upstream: difflabai/shared-pool-controls crates/core/src/lib.rs (U:)    *)
(* Pin:      57b3f02 (v1.1.0, the merge of PR #4; tree = PR head 6fcbd56). *)
(*           v1.0.0 (6f61616) is modelled at the shared-pool-v1.0.0 tag.   *)
(*                                                                         *)
(* The variables are the Pool struct field by field:                       *)
(*                                                                         *)
(*   cfg         Config (owner, max_age_seconds, metrics, terms, recipes). *)
(*               A variable, not a constant, so one TLC run can replay     *)
(*               traces of many configurations (REF-5); no action writes   *)
(*               it (INV-WF, StepCfg).                                      *)
(*   values      Pool.values, a sequence over the metrics (1-based: the    *)
(*               code's metric i is values[i + 1])                         *)
(*   version     Pool.version                                              *)
(*   observedAt  Pool.observed_at                                          *)
(*   paused      Pool.paused                                               *)
(*   records     Pool.records, the window (at most MaxRecords records,     *)
(*               oldest first); records[k] has id firstId + k - 1, so ids  *)
(*               are implicit (WIN-1) and Rec(id) = records[Pos(id)]       *)
(*   firstId     Pool.first_id                                             *)
(*   doneU       Pool.completed_units (doneU[p + 1] for recipe p),         *)
(*               saturating at DoneCap (u32::MAX in the core)              *)
(*                                                                         *)
(* Names and units of metrics and recipes are not modelled: the core only  *)
(* validates their lengths. Ghost (not in the Pool): base and k0 are the   *)
(* values and the first id not yet counted at the last re-basing observe   *)
(* (or new); a pruned record that was counted is folded into base (k0 is   *)
(* an id; strategy 2.2). steps counts effective transitions (INV-VER),     *)
(* last is the most recent effective call (a history variable read only by *)
(* step properties).                                                       *)
(*                                                                         *)
(* Each entry point has an error operator giving the first failing guard   *)
(* in the core's order, "Noop" for the core's idempotent Ok(()) paths      *)
(* (TX-NOOP), or "Ok", and an action enabled exactly when it is "Ok". A    *)
(* rejected or idempotent call writes nothing (TX-0), so it is a           *)
(* stuttering step and not part of Next.                                   *)
(*                                                                         *)
(* Numbers are exact integers. The core's i64/u64 arithmetic cannot wrap   *)
(* on these paths (NUM-1, NUM-2: values stay within +-1e6, units <= 100,   *)
(* weights <= 1000), and every bound check is transcribed. Three constants *)
(* are scaled in the TLC configs (C-5): MaxRecords (MAX_RECORDS = 256),    *)
(* MaxVersion (u32::MAX does not fit TLC's 32-bit integers; the configs    *)
(* use 2^31 - 1) and DoneCap (u32::MAX; 1 in the small universes, where    *)
(* every prerequisite asks for 1 unit, so the saturated count decides      *)
(* every prerequisite as the exact one does). VersionCap = MaxVersion -    *)
(* 3 * MaxRecords is VERSION_CAP's definition (U:24) with both scaled.     *)
(***************************************************************************)
EXTENDS Integers, Sequences, FiniteSets

CONSTANTS
    \* Every account that calls: owner, actors, executors and strangers.
    Accounts,
    \* Argument domains of Next (model checking only; TraceCheck replays
    \* the recorded arguments): block times, expiry offsets added to now,
    \* and the vectors the owner may observe.
    Times,
    ExpOffsets,
    Observations,
    \* MAX_RECORDS (256 in the core): the window's size.
    MaxRecords,
    \* u32::MAX: bump() fails when version = MaxVersion.
    MaxVersion,
    \* u32::MAX: completed_units saturates here.
    DoneCap

MaxValue  == 1000000    \* MAX_VALUE
MaxMetric == 32         \* MAX_METRICS
MaxRecipe == 32         \* MAX_RECIPES
MaxTerm   == 64         \* MAX_TERMS
MaxWeight == 1000       \* MAX_WEIGHT
MaxUnit   == 100        \* MAX_UNITS
Window    == 86400      \* submit: expires_at <= now + 86400
\* VERSION_CAP (U:24): submit, dispatch, set_paused and observe fail
\* Arithmetic from here on (bump_new, U:401-406).
VersionCap == MaxVersion - 3 * MaxRecords

Statuses == {"Reserved", "Dispatched", "Active", "Unknown", "Completed", "Cancelled"}
Shapes   == {"target", "excess", "deficit"}

VARIABLES
    cfg, values, version, observedAt, paused, records, firstId, doneU,
    base, k0, steps, last

pool  == <<cfg, values, version, observedAt, paused, records, firstId, doneU>>
vars  == <<cfg, values, version, observedAt, paused, records, firstId, doneU,
           base, k0, steps, last>>

-----------------------------------------------------------------------------
(* Arithmetic helpers                                                      *)

Max0(x) == IF x > 0 THEN x ELSE 0
Min0(x) == IF x < 0 THEN x ELSE 0
Min(x, y) == IF x < y THEN x ELSE y
Max(x, y) == IF x > y THEN x ELSE y
Abs(x)  == IF x < 0 THEN -x ELSE x
\* bounded(): -MAX_VALUE ..= MAX_VALUE.
Bounded(v) == -MaxValue <= v /\ v <= MaxValue

\* Sum of f[1] .. f[k], as a recursive function: TLC re-evaluates a RECURSIVE
\* operator's lazy argument f (0.3 s a step at 256 records; 5 ms this way).
SumTo(f, k) == LET S[j \in 0 .. k] == IF j = 0 THEN 0 ELSE f[j] + S[j - 1] IN S[k]

RECURSIVE SetSum(_, _)
\* Sum of g[x] over a finite set S.
SetSum(S, g) == IF S = {} THEN 0 ELSE LET x == CHOOSE y \in S : TRUE IN g[x] + SetSum(S \ {x}, g)

-----------------------------------------------------------------------------
(* Shape of the configuration and the window                               *)

NM   == Len(cfg.metrics)
MIdx == 1 .. NM
NR   == Len(records)
NRcp == Len(cfg.recipes)
\* index() (U:385-391): the record with this id is retained iff
\* firstId <= id < firstId + NR; it is records[Pos(id)].
Pos(id)    == id - firstId + 1
Found(id)  == firstId <= id /\ id < firstId + NR
Rec(id)    == records[Pos(id)]
\* The ids of the retained records.
WinIds     == firstId .. firstId + NR - 1
Rcp(r)     == cfg.recipes[r + 1]
RecRcp(id) == Rcp(Rec(id).recipe)
Mn(m) == cfg.metrics[m].min
Mx(m) == cfg.metrics[m].max
\* The largest max_units of any recipe (argument domain of Next).
MaxU == LET us == {cfg.recipes[k].maxUnits : k \in 1 .. NRcp} IN CHOOSE u \in us : \A w \in us : w <= u

Open(s)     == s \notin {"Completed", "Cancelled"}
InFlight(s) == s \in {"Dispatched", "Active", "Unknown"}
AnyOpen     == \E k \in 1 .. NR : Open(records[k].status)
\* candidate()'s Full (U:287-289): the window is full and its oldest
\* record, which a submit would prune, is open.
WinFull == NR >= MaxRecords /\ Open(records[1].status)

\* Config::validate (CFG-2), names omitted. requires is a sequence of
\* <<prerequisite recipe, units>>. v1.1 adds actor # executor (U:130).
ValidCfg(c) ==
    LET n == Len(c.metrics) IN
    /\ n >= 1 /\ n <= MaxMetric
    /\ Len(c.recipes) >= 1 /\ Len(c.recipes) <= MaxRecipe
    /\ Len(c.terms) >= 1 /\ Len(c.terms) <= MaxTerm
    /\ c.maxAge >= 0
    /\ \A m \in 1 .. n:
        LET x == c.metrics[m] IN
        /\ Bounded(x.min) /\ Bounded(x.max) /\ x.min <= x.max
        /\ x.min <= x.init /\ x.init <= x.max
    /\ \A k \in 1 .. Len(c.terms):
        LET t == c.terms[k] IN
        /\ t.metric \in 0 .. n - 1 /\ Bounded(t.target)
        /\ t.weight \in 1 .. MaxWeight /\ t.shape \in Shapes
    /\ \A k \in 1 .. Len(c.recipes):
        LET r == c.recipes[k] IN
        /\ r.actor # r.executor
        /\ Len(r.reserve) = n /\ Len(r.complete) = n
        /\ r.maxUnits \in 1 .. MaxUnit
        /\ Len(r.requires) <= MaxRecipe
        /\ \A q \in DOMAIN r.requires:
              r.requires[q][1] \in 0 .. k - 2 /\ r.requires[q][2] \in 1 .. MaxUnit
        /\ \A m \in 1 .. n: Bounded(r.reserve[m]) /\ Bounded(r.complete[m])
        /\ \A q1, q2 \in DOMAIN r.requires: q1 # q2 => r.requires[q1][1] # r.requires[q2][1]

-----------------------------------------------------------------------------
(* The core's private functions                                            *)

\* violations(values)[i]: PV_i(v) = max(0, min_i - v_i) + max(0, v_i - max_i).
PV(vals, m) == Max0(Mn(m) - vals[m]) + Max0(vals[m] - Mx(m))
Violating == \E m \in MIdx : PV(values, m) > 0

\* add(base, d, u): the exact result, and whether some component leaves
\* +-MAX_VALUE (Arithmetic). Lengths are equal in every WF state.
AddVec(b, d, u)   == [m \in DOMAIN b |-> b[m] + d[m] * u]
AddFails(b, d, u) == \E m \in DOMAIN b : ~Bounded(b[m] + d[m] * u)
Neg(d) == [m \in DOMAIN d |-> -d[m]]

\* envelope()'s terms for one record or the extra (U:216-228): on metric m,
\* d = complete * u (it completes) and x = -reserve * u (its reservation is
\* released by cancel or settle(false)); low adds min(d, x, 0), high adds
\* max(d, x, 0).
LoTerm(rc, u, m) == Min0(Min(rc.complete[m] * u, -(rc.reserve[m] * u)))
HiTerm(rc, u, m) == Max0(Max(rc.complete[m] * u, -(rc.reserve[m] * u)))
\* Sum of the low (neg) or high terms of the open records other than the one
\* with id skip (-1 for None; no id is negative).
OpenSum(m, skip, neg) ==
    SumTo([k \in 1 .. NR |->
              LET c == records[k] IN
              IF Open(c.status) /\ firstId + k - 1 # skip
              THEN (IF neg THEN LoTerm(Rcp(c.recipe), c.units, m)
                    ELSE HiTerm(Rcp(c.recipe), c.units, m))
              ELSE 0], NR)
\* The extra (recipe er, eu units) of candidate(); eu = 0 means None.
ExtraPart(er, eu, m, neg) ==
    IF eu = 0 THEN 0
    ELSE IF neg THEN LoTerm(Rcp(er), eu, m) ELSE HiTerm(Rcp(er), eu, m)
Low(cand, er, eu, skip, m)  == cand[m] + OpenSum(m, skip, TRUE) + ExtraPart(er, eu, m, TRUE)
High(cand, er, eu, skip, m) == cand[m] + OpenSum(m, skip, FALSE) + ExtraPart(er, eu, m, FALSE)
\* EV_i: the envelope's violation of metric m.
EV(cand, er, eu, skip, m) ==
    Max0(Mn(m) - Low(cand, er, eu, skip, m)) + Max0(High(cand, er, eu, skip, m) - Mx(m))

\* check_bounds(candidate, extra, skip) (U:250-266). The envelope's partial
\* sums are monotone from a bounded start (each low term <= 0, each high
\* term >= 0), so it fails (Arithmetic) exactly when a final low or high
\* leaves +-MAX_VALUE; then Bounds if some metric's envelope violation
\* exceeds the smaller of its violation now and at the candidate (U:261).
CheckBounds(cand, er, eu, skip) ==
    LET lo == [m \in MIdx |-> Low(cand, er, eu, skip, m)]
        hi == [m \in MIdx |-> High(cand, er, eu, skip, m)]
    IN IF \E m \in MIdx : ~Bounded(lo[m]) \/ ~Bounded(hi[m]) THEN "Arithmetic"
       ELSE IF \E m \in MIdx :
                 Max0(Mn(m) - lo[m]) + Max0(hi[m] - Mx(m)) > Min(PV(values, m), PV(cand, m))
            THEN "Bounds"
       ELSE "Ok"

\* energy(): sum of weight * r^2 over the terms. Exact (NUM-1).
TermR(vals, t) ==
    LET d == vals[t.metric + 1] - t.target IN
    CASE t.shape = "target"  -> Abs(d)
      [] t.shape = "excess"  -> Max0(d)
      [] t.shape = "deficit" -> Max0(-d)
Energy(vals) ==
    SumTo([k \in 1 .. Len(cfg.terms) |->
              LET r == TermR(vals, cfg.terms[k]) IN cfg.terms[k].weight * r * r],
          Len(cfg.terms))

\* bump(): version.checked_add(1) (U:395-398).
VerErr == IF version >= MaxVersion THEN "Arithmetic" ELSE "Ok"
\* bump_new() (U:401-406): the cap, then bump (which cannot fail below it).
CapErr == IF version >= VersionCap THEN "Arithmetic" ELSE VerErr

SnapExpired(now) == cfg.maxAge > 0 /\ now > observedAt + cfg.maxAge

-----------------------------------------------------------------------------
(* Guards, in the core's order. GuardOrder lists each entry point's exits  *)
(* in order: error codes, and "Noop" where an idempotent Ok(()) returns    *)
(* early. Arithmetic appears once per checked operation (add, envelope,    *)
(* version cap or bump). C-2 compares it with the Prolog guard table. The  *)
(* last Arithmetic of submit, dispatch, pause and observe is the version   *)
(* cap (U:319, 402); the sites C-2 lets a model omit as unreachable from   *)
(* an INV state (submit's checked version and id additions, U:362-366,    *)
(* WIN-1; bump after the cap) are not listed.                              *)

GuardOrder == [
    submit   |-> <<"Paused", "Stale", "Expired", "SnapshotExpired", "Full",
                   "InvalidRecipe", "Unauthorized", "InvalidUnits", "Prerequisite",
                   "Arithmetic", "Arithmetic", "Bounds", "Energy", "Arithmetic",
                   "Energy", "Arithmetic">>,
    dispatch |-> <<"NotFound", "Unauthorized", "Paused", "InvalidState", "Expired",
                   "SnapshotExpired", "Arithmetic">>,
    ack      |-> <<"NotFound", "Noop", "Noop", "Arithmetic">>,
    cancel   |-> <<"NotFound", "Unauthorized", "Noop", "InvalidState", "Pending",
                   "Arithmetic", "Arithmetic", "Bounds", "Arithmetic">>,
    settle   |-> <<"NotFound", "Unauthorized", "Noop", "InvalidState",
                   "Arithmetic", "Arithmetic", "Bounds", "Arithmetic">>,
    pause    |-> <<"Unauthorized", "Arithmetic">>,
    observe  |-> <<"Unauthorized", "Pending", "Bounds", "Arithmetic">>
]

\* candidate() (U:267-323) then submit's additions (U:360-366).
SubmitErr(c, r, u, v, exp, now) ==
    IF paused THEN "Paused"
    ELSE IF v # version THEN "Stale"
    ELSE IF exp <= now \/ exp > now + Window THEN "Expired"
    ELSE IF SnapExpired(now) THEN "SnapshotExpired"
    ELSE IF WinFull THEN "Full"
    ELSE IF r >= NRcp THEN "InvalidRecipe"
    ELSE IF c # Rcp(r).actor THEN "Unauthorized"
    ELSE IF u = 0 \/ u > Rcp(r).maxUnits THEN "InvalidUnits"
    ELSE IF \E q \in DOMAIN Rcp(r).requires :
                doneU[Rcp(r).requires[q][1] + 1] < Rcp(r).requires[q][2]
         THEN "Prerequisite"
    ELSE IF AddFails(values, Rcp(r).reserve, u) THEN "Arithmetic"
    ELSE LET next == AddVec(values, Rcp(r).reserve, u)
             cb   == CheckBounds(next, r, u, -1)
             e0   == Energy(values)
         IN IF cb # "Ok" THEN cb
            ELSE IF Energy(next) > e0 THEN "Energy"
            ELSE IF Violating /\ AddFails(next, Rcp(r).complete, u) THEN "Arithmetic"
            ELSE IF Violating /\ Energy(AddVec(next, Rcp(r).complete, u)) >= e0
                 THEN "Energy"
            ELSE IF CapErr # "Ok" THEN "Arithmetic"
            \* submit's id = first_id + len (U:363-366): unreachable (WIN-1).
            ELSE IF firstId + NR > MaxVersion THEN "Arithmetic"
            ELSE "Ok"

DispatchErr(c, id, now) ==
    IF ~Found(id) THEN "NotFound"
    ELSE IF c # RecRcp(id).actor THEN "Unauthorized"
    ELSE IF paused THEN "Paused"
    ELSE IF Rec(id).status # "Reserved" THEN "InvalidState"
    ELSE IF now >= Rec(id).exp THEN "Expired"
    ELSE IF SnapExpired(now) THEN "SnapshotExpired"
    ELSE CapErr

AckTo(acc) == IF acc THEN "Active" ELSE "Unknown"

\* acknowledge(id, accepted): no caller in the core (the shell's private
\* callback is the only caller, SH-3). The plain bump.
AckErr(id, acc) ==
    IF ~Found(id) THEN "NotFound"
    ELSE IF Rec(id).status \notin {"Dispatched", "Unknown"} THEN "Noop"
    ELSE IF Rec(id).status = AckTo(acc) THEN "Noop"
    ELSE VerErr

\* release(id) (U:476-489): undo the reservation; the envelope skips the
\* record. The plain bump.
ReleaseErr(id) ==
    LET d == Neg(RecRcp(id).reserve)
        u == Rec(id).units
    IN IF AddFails(values, d, u) THEN "Arithmetic"
       ELSE LET cb == CheckBounds(AddVec(values, d, u), 0, 0, id) IN
            IF cb # "Ok" THEN cb ELSE VerErr

\* cancel(caller, id, now) (U:456-475): now is unused by the core. A
\* Completed record is InvalidState (U:467), before Pending.
CancelErr(c, id, now) ==
    IF ~Found(id) THEN "NotFound"
    ELSE IF c # RecRcp(id).actor /\ c # cfg.owner THEN "Unauthorized"
    ELSE IF Rec(id).status = "Cancelled" THEN "Noop"
    ELSE IF Rec(id).status = "Completed" THEN "InvalidState"
    ELSE IF Rec(id).status # "Reserved" THEN "Pending"
    ELSE ReleaseErr(id)

SettleErr(c, id, done) ==
    IF ~Found(id) THEN "NotFound"
    ELSE IF c # RecRcp(id).executor THEN "Unauthorized"
    ELSE IF (done /\ Rec(id).status = "Completed") \/ (~done /\ Rec(id).status = "Cancelled")
         THEN "Noop"
    ELSE IF ~InFlight(Rec(id).status) THEN "InvalidState"
    ELSE IF ~done THEN ReleaseErr(id)
    ELSE LET d == RecRcp(id).complete
             u == Rec(id).units
         IN IF AddFails(values, d, u) THEN "Arithmetic"
            ELSE LET cb == CheckBounds(AddVec(values, d, u), 0, 0, id) IN
                 IF cb # "Ok" THEN cb ELSE VerErr

PauseErr(c, b) ==
    IF c # cfg.owner THEN "Unauthorized" ELSE CapErr

\* observe (U:534-549): Pending only if the values change while a record is
\* open (restating the current values, a refresh, is Ok). energy(values)?
\* cannot fail for a bounded vector of the right length (NUM-1), so it has
\* no exit here.
ObserveErr(c, vals, now) ==
    IF c # cfg.owner THEN "Unauthorized"
    ELSE IF vals # values /\ AnyOpen THEN "Pending"
    ELSE IF Len(vals) # NM \/ \E k \in DOMAIN vals : ~Bounded(vals[k]) THEN "Bounds"
    ELSE CapErr

-----------------------------------------------------------------------------
(* Actions: the effect of an "Ok" call                                     *)

Initial(c) == [m \in 1 .. Len(c.metrics) |-> c.metrics[m].init]

\* Pool::new(c, t0) for a configuration that validates (CFG-1), at version
\* v0 (0 for new; MC_Cap starts near the cap, INV-VER counts from v0).
InitAt(c, t0, v0) ==
    /\ cfg = c
    /\ values = Initial(c)
    /\ version = v0
    /\ observedAt = t0
    /\ paused = FALSE
    /\ records = <<>>
    /\ firstId = 0
    /\ doneU = [p \in 1 .. Len(c.recipes) |-> 0]
    /\ base = Initial(c)
    /\ k0 = 0
    /\ steps = v0
    /\ last = [op |-> "new", c |-> c.owner, i |-> 0, r |-> 0, u |-> 0, v |-> 0, b |-> FALSE,
               now |-> t0, exp |-> 0]
InitFrom(c, t0) == InitAt(c, t0, 0)

\* Ghost bookkeeping of an effective call: its arguments (i is the record
\* id; r and u are the record's recipe and units for calls on a record).
Step(op, c, i, r, u, v, b, now, exp) ==
    /\ steps' = steps + 1
    /\ last' = [op |-> op, c |-> c, i |-> i, r |-> r, u |-> u, v |-> v, b |-> b,
                now |-> now, exp |-> exp]

\* A record's contribution to values since its admission (the ledger's
\* term; ghost): its reservation unless Cancelled, plus its completion.
Contrib(c, m) ==
    IF c.status = "Cancelled" THEN 0
    ELSE Rcp(c.recipe).reserve[m] * c.units
         + (IF c.status = "Completed" THEN Rcp(c.recipe).complete[m] * c.units ELSE 0)

SetStatus(id, s) == records' = [records EXCEPT ![Pos(id)].status = s]

\* submit (U:360-383): a full window prunes its (terminal) oldest record
\* and advances first_id; the new record gets id firstId + NR (before the
\* prune). Ghost: a pruned record the ledger counted (k0 <= its id) is
\* folded into base.
Submit(c, r, u, v, exp, now) ==
    /\ SubmitErr(c, r, u, v, exp, now) = "Ok"
    /\ LET new   == [recipe |-> r, units |-> u, exp |-> exp, status |-> "Reserved"]
           prune == NR >= MaxRecords
       IN /\ records' = IF prune THEN Append(Tail(records), new) ELSE Append(records, new)
          /\ firstId' = IF prune THEN firstId + 1 ELSE firstId
          /\ IF prune /\ k0 <= firstId
             THEN /\ base' = [m \in MIdx |-> base[m] + Contrib(records[1], m)]
                  /\ k0' = firstId + 1
             ELSE UNCHANGED <<base, k0>>
    /\ values' = AddVec(values, Rcp(r).reserve, u)
    /\ version' = version + 1
    /\ UNCHANGED <<cfg, observedAt, paused, doneU>>
    /\ Step("submit", c, firstId + NR, r, u, v, FALSE, now, exp)

Dispatch(c, id, now) ==
    /\ DispatchErr(c, id, now) = "Ok"
    /\ SetStatus(id, "Dispatched")
    /\ version' = version + 1
    /\ UNCHANGED <<cfg, values, observedAt, paused, firstId, doneU, base, k0>>
    /\ Step("dispatch", c, id, Rec(id).recipe, Rec(id).units, 0, FALSE, now, 0)

Ack(id, acc) ==
    /\ AckErr(id, acc) = "Ok"
    /\ SetStatus(id, AckTo(acc))
    /\ version' = version + 1
    /\ UNCHANGED <<cfg, values, observedAt, paused, firstId, doneU, base, k0>>
    /\ Step("ack", cfg.owner, id, Rec(id).recipe, Rec(id).units, 0, acc, 0, 0)

\* release(id)'s effect.
Release(id) ==
    /\ values' = AddVec(values, Neg(RecRcp(id).reserve), Rec(id).units)
    /\ SetStatus(id, "Cancelled")
    /\ version' = version + 1
    /\ UNCHANGED <<cfg, observedAt, paused, firstId, doneU, base, k0>>

Cancel(c, id, now) ==
    /\ CancelErr(c, id, now) = "Ok"
    /\ Release(id)
    /\ Step("cancel", c, id, Rec(id).recipe, Rec(id).units, 0, FALSE, now, 0)

\* settle(true) also counts the units in completed_units, saturating (U:519).
Settle(c, id, done) ==
    /\ SettleErr(c, id, done) = "Ok"
    /\ IF done
       THEN /\ values' = AddVec(values, RecRcp(id).complete, Rec(id).units)
            /\ SetStatus(id, "Completed")
            /\ doneU' = [doneU EXCEPT ![Rec(id).recipe + 1] = Min(@ + Rec(id).units, DoneCap)]
            /\ version' = version + 1
            /\ UNCHANGED <<cfg, observedAt, paused, firstId, base, k0>>
       ELSE Release(id)
    /\ Step("settle", c, id, Rec(id).recipe, Rec(id).units, 0, done, 0, 0)

SetPaused(c, b) ==
    /\ PauseErr(c, b) = "Ok"
    /\ paused' = b
    /\ version' = version + 1
    /\ UNCHANGED <<cfg, values, observedAt, records, firstId, doneU, base, k0>>
    /\ Step("pause", c, 0, 0, 0, 0, b, 0, 0)

\* Ghost (strategy 2.2): a refresh (the current values while a record is
\* open) keeps base and k0; any other observe re-bases (k0 = the next id).
Observe(c, vals, now) ==
    /\ ObserveErr(c, vals, now) = "Ok"
    /\ values' = vals
    /\ observedAt' = now
    /\ version' = version + 1
    /\ IF vals = values /\ AnyOpen
       THEN UNCHANGED <<base, k0>>
       ELSE base' = vals /\ k0' = firstId + NR
    /\ UNCHANGED <<cfg, paused, records, firstId, doneU>>
    /\ Step("observe", c, 0, 0, 0, 0, FALSE, now, 0)

-----------------------------------------------------------------------------
(* Every call any party can make, over the argument domains. Record ids    *)
(* range from one pruned id (if any) to one past the window, recipes one   *)
(* past the end, units one past max_units, request versions over the       *)
(* current one and one other (any other value behaves the same: the core   *)
(* compares it only for equality), so every guard stays reachable for the  *)
(* step properties.                                                        *)

IdDom == (IF firstId > 0 THEN firstId - 1 ELSE 0) .. firstId + NR
OtherVersion == IF version < MaxVersion THEN version + 1 ELSE version - 1

SubmitNext  == \E c \in Accounts, r \in 0 .. NRcp, u \in 0 .. MaxU + 1,
                  v \in {version, OtherVersion}, now \in Times, d \in ExpOffsets :
                  Submit(c, r, u, v, now + d, now)
DispatchNext == \E c \in Accounts, id \in IdDom, now \in Times : Dispatch(c, id, now)
AckNext     == \E id \in IdDom, b \in BOOLEAN : Ack(id, b)
CancelNext  == \E c \in Accounts, id \in IdDom, now \in Times : Cancel(c, id, now)
SettleNext  == \E c \in Accounts, id \in IdDom, b \in BOOLEAN : Settle(c, id, b)
PauseNext   == \E c \in Accounts, b \in BOOLEAN : SetPaused(c, b)
ObserveNext == \E c \in Accounts, vals \in Observations, now \in Times : Observe(c, vals, now)

Next ==
    \/ SubmitNext \/ DispatchNext \/ AckNext \/ CancelNext
    \/ SettleNext \/ PauseNext \/ ObserveNext

=============================================================================
