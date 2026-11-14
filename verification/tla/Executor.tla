------------------------------ MODULE Executor ------------------------------
(***************************************************************************)
(* TLA-EXEC: the synthetic executor's job window against the lock's record *)
(* window, every receipt delivered in any order (EX-6, EX-7, EX-8;         *)
(* threats T-FORGET and T-JOBCAP; OQ-13).                                  *)
(*                                                                         *)
(* Upstream: difflabai/shared-pool-controls                                *)
(* Pin:      57b3f02d18e85149db3c1035431368c4e07a4d8a (merge of PR #4)     *)
(*   executor  crates/executor/src/lib.rs (E: below): WINDOW 6, MAX_JOBS   *)
(*             12, newest 35, execute 62-97, finish 108-122,               *)
(*             confirm_not_executed 131-143, on_settled 153-160, advance   *)
(*             173-177, notify 182-201                                     *)
(*   lock      crates/core/src/lib.rs (C:): MAX_RECORDS 15, window 49-53,  *)
(*             Full 287-289, submit 360-381, index 385-391, dispatch       *)
(*             407-434, cancel 456-475, settle 492-521                     *)
(*   wrapper   crates/contract/src/lib.rs (P:): dispatch 82-91, reconcile  *)
(*             93-112, on_result 113-145, settle 146-152                   *)
(* The notes (.context/shared-pool/v1.1-verify/w9b-notes.md) map every     *)
(* action to these lines.                                                  *)
(*                                                                         *)
(* Lock. Its record window (C:49-53): firstId and the statuses of the      *)
(* retained records, record firstId + k - 1 at recs[k]. Statuses R         *)
(* Reserved, F in flight (Dispatched, Active or Unknown: the executor      *)
(* cannot tell them apart, and settle accepts all three), C Completed, X   *)
(* Cancelled. Values, bounds, energy, time, pause, prerequisites and the   *)
(* version only decide whether submit, dispatch and cancel are allowed;    *)
(* here each is free, so the model has every behaviour of the core and     *)
(* more. A settle passes the bounds and version checks (LIVE-ALL,          *)
(* INV-VERCAP, core layers). Every record's executor is this executor.     *)
(* So this lock never refuses the settle of an in-flight record. The real  *)
(* one can: Unauthorized (another executor's recipe, docs/api.md:42), or   *)
(* Bounds or Arithmetic, which LIVE-ALL and INV-VERCAP exclude in every    *)
(* reachable core state. Such a refusal writes nothing at the lock, and    *)
(* on_settled then writes nothing (E:157), so receipt timing covers it:    *)
(* the receipt is never delivered, or, for a confirmation, it is delivered *)
(* while the record is Reserved (InvalidState) and the record is           *)
(* dispatched afterwards. The equivalence is argued; a one-off run with    *)
(* that refusal added held every invariant (w9b notes).                    *)
(*                                                                         *)
(* LockSideSettle is every way a record closes at the lock without the     *)
(* executor contract's notify: on_result applying an execute result or a   *)
(* reconcile status (P:134-139, the core's settle by the record's          *)
(* executor), and a settle signed with the executor account's own key      *)
(* (docs/api.md:42, the job-cap exit). It may close any in-flight record   *)
(* either way at any time, a superset of what those paths can do. So       *)
(* execute's and status's results are not modelled: they reach only        *)
(* on_result. Acknowledge only moves among D, U and A (all F here).        *)
(*                                                                         *)
(* Executor. Its state field by field: jobs (id |-> status and whether     *)
(* execute or confirm_not_executed made it) and newest. lock and operator  *)
(* are fixed; the checks on them (only lock may execute, only independent  *)
(* operator, #[private]) are the actions' callers. mode is not a           *)
(* variable: set_mode is a separate transaction, so any mode may hold at   *)
(* any execute, and Execute takes it as a parameter ("unknown" stores what *)
(* "normal" stores; only the result differs). recipe and units are left    *)
(* out: only execute's payload check reads them, and its outcome (a panic  *)
(* or the stored status, nothing moves either way) reaches only on_result. *)
(*                                                                         *)
(* Receipts in flight are sets (NEAR runs receipts of different calls in   *)
(* any order): execQ (the lock's execute(id)), setQ (the executor's        *)
(* settle(id, completed) to the lock), cbQ (on_settled(id) after a settle  *)
(* the lock accepted). A refused settle's on_settled writes nothing        *)
(* (E:157), so it runs in the lock's settle step. A rejected call or a     *)
(* panicking receipt reverts, so it is no step.                            *)
(*                                                                         *)
(* Scaled: WINDOW = MAX_RECORDS = W (256), MAX_JOBS = MaxJobs (512). The   *)
(* lock issues ids 0 .. NIds - 1; the operator may name any id in          *)
(* 0 .. MaxId, so an id above NIds - 1 is one the lock never reaches.      *)
(*                                                                         *)
(* Ghosts: made (the ids at or above firstId the executor made a job for), *)
(* refused (the ids of the executor's settles the lock refused), ev (the   *)
(* last step, read only by step properties; the VIEW leaves it out).       *)
(***************************************************************************)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    W,          \* WINDOW (E:6) = MAX_RECORDS (C:15), 256
    MaxJobs,    \* MAX_JOBS (E:12), 512
    NIds,       \* the lock issues ids 0 .. NIds - 1
    MaxId       \* the operator names ids 0 .. MaxId

ASSUME W >= 1 /\ MaxJobs >= 2 * W /\ NIds >= 1 /\ MaxId >= NIds - 1

VARIABLES
    firstId,    \* Pool.first_id
    recs,       \* statuses of Pool.records, oldest first
    jobs,       \* Executor.jobs: id |-> [st, kind]
    newest,     \* Executor.newest
    execQ,      \* ids: execute(id) receipts the lock's dispatch created
    setQ,       \* [id, c]: the executor's settle(id, completed) receipts
    cbQ,        \* ids: on_settled(id) receipts after an accepted settle
    made,       \* ghost: ids >= firstId the executor made a job for
    refused,    \* ghost: ids whose settle from notify the lock refused
    ev          \* ghost: the last step

vars == <<firstId, recs, jobs, newest, execQ, setQ, cbQ, made, refused, ev>>

Ids == 0 .. MaxId
Max(a, b) == IF a >= b THEN a ELSE b

Statuses  == {"R", "F", "C", "X"}
Open      == {"R", "F"}
JobStatus == {"active", "cancelled", "completed"}   \* E:22
Kinds     == {"exec", "conf"}
Modes     == {"normal", "reject", "panic"}

-----------------------------------------------------------------------------
(* Lock state.                                                             *)

NR        == Len(recs)
Retained  == {firstId + k : k \in 0 .. NR - 1}
(* C:385-391 index(id): id - first_id < len, else NotFound.               *)
St(i)     == recs[i - firstId + 1]
Put(i, s) == [recs EXCEPT ![i - firstId + 1] = s]
Done(c)   == IF c THEN "C" ELSE "X"

(* C:492-521 settle(executor, id, completed): the first failing guard,    *)
(* "Noop" for the repeated terminal outcome (Ok, no change), or "Ok".     *)
SettleErr(i, c) ==
    IF i \notin Retained THEN "NotFound"
    ELSE IF (c /\ St(i) = "C") \/ (~c /\ St(i) = "X") THEN "Noop"
    ELSE IF St(i) # "F" THEN "InvalidState"
    ELSE "Ok"

(* Executor state.                                                         *)

(* E:173-177 advance(id): newest = max(newest, id), then keep the jobs    *)
(* with id + WINDOW > newest.                                             *)
Keep(n) == [j \in {x \in DOMAIN jobs : x + W > n} |-> jobs[j]]

(* E:153-160 on_settled(id): private, needs one promise result (E:155),   *)
(* advance(id) unless the settle failed (E:157-159). As <<newest, jobs>>. *)
Settled(i, ok) ==
    IF ok THEN <<Max(newest, i), Keep(Max(newest, i))>> ELSE <<newest, jobs>>

(* The ghost made keeps only ids at or above firstId.                     *)
Made(i) == IF i >= firstId THEN made \cup {i} ELSE made

-----------------------------------------------------------------------------
(* Lock actions.                                                           *)

(* C:360-381 submit: id = first_id + len; at MAX_RECORDS the oldest record *)
(* is pruned if terminal (C:368-371), else Full (C:287-289, no step).      *)
Submit ==
    /\ firstId + NR < NIds
    /\ \/ /\ NR < W
          /\ recs' = Append(recs, "R")
          /\ UNCHANGED <<firstId, made>>
          /\ ev' = [k |-> "submit", id |-> firstId + NR]
       \/ /\ NR = W
          /\ recs[1] \notin Open
          /\ recs' = Append(Tail(recs), "R")
          /\ firstId' = firstId + 1
          /\ made' = made \ {firstId}
          /\ ev' = [k |-> "prune", id |-> firstId]
    /\ UNCHANGED <<jobs, newest, execQ, setQ, cbQ, refused>>

(* C:456-475 cancel by the actor or the owner: only Reserved changes      *)
(* (release, C:476-489).                                                  *)
Cancel(i) ==
    /\ St(i) = "R"
    /\ recs' = Put(i, "X")
    /\ ev' = [k |-> "cancel", id |-> i]
    /\ UNCHANGED <<firstId, jobs, newest, execQ, setQ, cbQ, made, refused>>

(* C:407-434 dispatch, then P:82-91 sends execute(id) to the executor.     *)
Dispatch(i) ==
    /\ St(i) = "R"
    /\ recs' = Put(i, "F")
    /\ execQ' = execQ \cup {i}
    /\ ev' = [k |-> "dispatch", id |-> i]
    /\ UNCHANGED <<firstId, jobs, newest, setQ, cbQ, made, refused>>

(* See the header: on_result (after execute or reconcile) or the executor *)
(* key closes an in-flight record; the executor contract does not change. *)
LockSideSettle(i, c) ==
    /\ SettleErr(i, c) = "Ok"
    /\ recs' = Put(i, Done(c))
    /\ ev' = [k |-> "lock_side_settle", id |-> i]
    /\ UNCHANGED <<firstId, jobs, newest, execQ, setQ, cbQ, made, refused>>

(* The lock runs the executor's settle receipt (P:146-152, C:492-521).    *)
(* Ok or Noop is a success. on_settled gets the result (E:192-199): an    *)
(* accepted one waits in cbQ; a refused one runs in this step, as its     *)
(* write (none, E:157) does not depend on when it runs.                   *)
LockSettle(m) ==
    LET ok == SettleErr(m.id, m.c) \in {"Ok", "Noop"}
    IN /\ setQ' = setQ \ {m}
       /\ recs' = IF SettleErr(m.id, m.c) = "Ok" THEN Put(m.id, Done(m.c))
                  ELSE recs
       /\ IF ok THEN /\ cbQ' = cbQ \cup {m.id}
                     /\ UNCHANGED <<newest, jobs>>
                ELSE /\ newest' = Settled(m.id, FALSE)[1]
                     /\ jobs' = Settled(m.id, FALSE)[2]
                     /\ UNCHANGED cbQ
       /\ refused' = IF ok THEN refused ELSE refused \cup {m.id}
       /\ ev' = [k |-> IF ok THEN "settle_ok" ELSE "settle_refused",
                 id |-> m.id]
       /\ UNCHANGED <<firstId, execQ, made>>

-----------------------------------------------------------------------------
(* Executor actions.                                                       *)

(* E:62-97 execute(id, recipe, units) from the lock (execQ). A stored job *)
(* returns its status or fails the payload check, and moves nothing       *)
(* (E:68-71). A forgotten id returns "cancelled" and stores nothing       *)
(* (E:73-75). Otherwise advance (E:77), then the cap (E:78) and panic     *)
(* mode (E:79): either panics, and the receipt reverts, advance included. *)
(* reject stores "cancelled", the others "active" (E:80-91).              *)
Execute(i, mode) ==
    /\ execQ' = execQ \ {i}
    /\ IF i \in DOMAIN jobs THEN
          /\ ev' = [k |-> "exec_stored", id |-> i]
          /\ UNCHANGED <<jobs, newest, made>>
       ELSE IF i + W <= newest THEN
          /\ ev' = [k |-> "exec_stale", id |-> i]
          /\ UNCHANGED <<jobs, newest, made>>
       ELSE LET n == Max(newest, i)
                full == Cardinality(DOMAIN Keep(n)) >= MaxJobs
            IN IF full \/ mode = "panic" THEN
                  /\ ev' = [k |-> IF full THEN "exec_full" ELSE "exec_panic",
                            id |-> i]
                  /\ UNCHANGED <<jobs, newest, made>>
               ELSE
                  /\ newest' = n
                  /\ jobs' = (i :> [st |-> IF mode = "reject" THEN "cancelled"
                                           ELSE "active",
                                    kind |-> "exec"]) @@ Keep(n)
                  /\ made' = Made(i)
                  /\ ev' = [k |-> "exec_new", id |-> i]
    /\ UNCHANGED <<firstId, recs, setQ, cbQ, refused>>

(* E:108-122 finish(id, completed) by the operator: the job must exist    *)
(* (E:110-114) and be active or already this outcome (E:116-119); its     *)
(* status is written first, then notify sends settle (E:120-121).         *)
Outcome(c) == IF c THEN "completed" ELSE "cancelled"
Finish(i, c) ==
    /\ jobs[i].st \in {"active", Outcome(c)}
    /\ jobs' = [jobs EXCEPT ![i].st = Outcome(c)]
    /\ setQ' = setQ \cup {[id |-> i, c |-> c]}
    /\ ev' = [k |-> "finish", id |-> i]
    /\ UNCHANGED <<firstId, recs, newest, execQ, cbQ, made, refused>>

(* E:131-143 confirm_not_executed(id, recipe, units) by the operator: no  *)
(* job for id (E:133), not stale (E:134), below the cap (E:135); the      *)
(* cancelled job is stored first and kept whatever the lock answers       *)
(* (E:136-141), then notify sends settle(id, false) (E:142).              *)
Confirm(i) ==
    /\ i \notin DOMAIN jobs
    /\ i + W > newest
    /\ Cardinality(DOMAIN jobs) < MaxJobs
    /\ jobs' = (i :> [st |-> "cancelled", kind |-> "conf"]) @@ jobs
    /\ made' = Made(i)
    /\ setQ' = setQ \cup {[id |-> i, c |-> FALSE]}
    /\ ev' = [k |-> "confirm", id |-> i]
    /\ UNCHANGED <<firstId, recs, newest, execQ, cbQ, refused>>

(* E:153-160 on_settled(id) after a settle the lock accepted.             *)
OnSettled(i) ==
    /\ cbQ' = cbQ \ {i}
    /\ newest' = Settled(i, TRUE)[1]
    /\ jobs' = Settled(i, TRUE)[2]
    /\ ev' = [k |-> "on_settled", id |-> i]
    /\ UNCHANGED <<firstId, recs, execQ, setQ, made, refused>>

-----------------------------------------------------------------------------

Init ==
    /\ firstId = 0
    /\ recs = <<>>
    /\ jobs = [i \in {} |-> [st |-> "active", kind |-> "exec"]]
    /\ newest = 0
    /\ execQ = {}
    /\ setQ = {}
    /\ cbQ = {}
    /\ made = {}
    /\ refused = {}
    /\ ev = [k |-> "init", id |-> 0]

Next ==
    \/ Submit
    \/ \E i \in Retained : Cancel(i) \/ Dispatch(i)
    \/ \E i \in Retained, c \in BOOLEAN : LockSideSettle(i, c)
    \/ \E m \in setQ : LockSettle(m)
    \/ \E i \in execQ, mode \in Modes : Execute(i, mode)
    \/ \E i \in DOMAIN jobs, c \in BOOLEAN : Finish(i, c)
    \/ \E i \in Ids : Confirm(i)
    \/ \E i \in cbQ : OnSettled(i)

Spec == Init /\ [][Next]_vars

(* VIEW. A receipt is dead when its delivery can change nothing but its    *)
(* own queue, now and in every later state:                               *)
(*   execute(i) of an id with a job or a forgotten id (E:68-75): the job  *)
(*     stays until forgotten, newest only grows;                          *)
(*   settle(i, c) of a pruned id (NotFound) or of a record closed the     *)
(*     other way (InvalidState): refused for good, and a refused          *)
(*     on_settled writes nothing;                                         *)
(*   on_settled(i) with i <= newest: advance(i) keeps newest, and keeps   *)
(*     every job by InvJobWindow (which the run checks).                  *)
(* No other action reads a queue, and adding a receipt that is there, or  *)
(* dead, gives the same view. So states that differ only in dead receipts *)
(* and in the ghosts ev and refused have the same successors up to those, *)
(* and fingerprinting by View is a bisimulation quotient for every        *)
(* invariant that reads neither (InvVouch reads only live receipts;       *)
(* InvConfSettles is below). TLC checks step properties on every          *)
(* transition it generates, also into a state whose view it has seen. It  *)
(* may not expand a state that differs from an expanded one only in dead  *)
(* receipts, so no step property is about delivering a dead receipt; the  *)
(* witnesses run without the VIEW (run.sh).                               *)
LiveExec == {i \in execQ : i \notin DOMAIN jobs /\ i + W > newest}
DeadSettle(m) ==
    \/ m.id < firstId
    \/ m.id \in Retained /\ St(m.id) \in {"C", "X"} /\ St(m.id) # Done(m.c)
LiveSet  == {m \in setQ : ~DeadSettle(m)}
LiveCb   == {i \in cbQ : i > newest}
(* refused, like ev, is written and never read by an action, so View may  *)
(* leave it out for every invariant but InvConfSettles. That one reads it *)
(* only for jobs above newest (an id that had a job and is at or below    *)
(* newest, or forgotten, never gets back above it), and runs with         *)
(* ViewRefused: W = 2 in run.sh (MC_Executor_W2.cfg). It is the costly    *)
(* one (W = 3: about 4x the views of View).                               *)
Above == {j \in DOMAIN jobs : j > newest}
View == <<firstId, recs, jobs, newest, LiveExec, LiveSet, LiveCb, made>>
ViewRefused == <<View, refused \cap Above>>
(* Cross-check of the quotient: every variable but ev, which only step    *)
(* properties read. At W = 2, NIds = MaxId = 4 it gives the same verdicts *)
(* as ViewRefused (run.sh executor-w2-fullview, opt-in, about 4 min).     *)
ViewFull == <<firstId, recs, jobs, newest, execQ, setQ, cbQ, made, refused>>

-----------------------------------------------------------------------------
(* Invariants (TLA-EXEC).                                                  *)

TypeOK ==
    /\ firstId \in 0 .. NIds
    /\ recs \in Seq(Statuses) /\ NR <= W
    /\ DOMAIN jobs \subseteq Ids
    /\ \A j \in DOMAIN jobs :
          jobs[j].st \in JobStatus /\ jobs[j].kind \in Kinds
    /\ newest \in Ids
    /\ execQ \subseteq Ids
    /\ setQ \subseteq [id : Ids, c : BOOLEAN]
    /\ cbQ \subseteq Ids
    /\ made \subseteq Ids
    /\ refused \subseteq Ids

(* EX-6: at most MAX_JOBS jobs.                                           *)
InvJobCap == Cardinality(DOMAIN jobs) <= MaxJobs

(* EX-6, Lemma J: every job is less than WINDOW behind newest.            *)
InvJobWindow == \A j \in DOMAIN jobs : j + W > newest

(* EX-7: newest is below first_id + WINDOW, so every id the executor has  *)
(* forgotten (id + WINDOW <= newest) is below first_id: pruned. Hence     *)
(* EX-6: execute answers "cancelled" without a job (E:73-75), and         *)
(* confirm_not_executed fails "stale id" (E:134), only for a pruned id.   *)
InvNewest == newest < firstId + W

(* EX-7: the executor never forgets the job of a record the lock retains, *)
(* in flight or not. (An in-flight record whose execute panicked has no   *)
(* job, so the invariant is over the ids the executor made a job for.)    *)
InvRetainedJob == \A i \in Retained \cap made : i \in DOMAIN jobs

(* What will vouch for newest is below first_id + WINDOW while in flight: *)
(* a dispatched id was retained at dispatch, an accepted settle's id at   *)
(* the settle (critique 3.9). Dead receipts vouch for nothing (View).     *)
InvVouch == \A i \in LiveExec \cup LiveCb : i < firstId + W

(* Lemma J (E:7-11, docs/verification-handoff.md EXEC): at most WINDOW    *)
(* jobs at or below newest; every job above it is a confirmation.         *)
InvLemmaJ ==
    /\ Cardinality({j \in DOMAIN jobs : j <= newest}) <= W
    /\ \A j \in Above : jobs[j].kind = "conf"

(* Lemma J, second half: a confirmation above newest was refused by the   *)
(* lock or waits for its settle or its on_settled; none is accepted and   *)
(* then left behind. A pending settle of a pruned or Completed record is  *)
(* dead (View) and refused for good, so it counts as refused. Reads the   *)
(* ghost refused: run with ViewRefused.                                   *)
InvConfSettles ==
    \A j \in Above :
       \/ j \in refused
       \/ j \in LiveCb
       \/ \E m \in LiveSet : m.id = j
       \/ j < firstId
       \/ j \in Retained /\ St(j) = "C"

(* E:7-11, E:129-130: the cap is reached only with at least               *)
(* MAX_JOBS - WINDOW (= WINDOW) jobs above newest.                        *)
InvCapShape == Cardinality(DOMAIN jobs) >= MaxJobs =>
                   Cardinality(Above) >= MaxJobs - W

(* EX-8 / critique 3.9: an accepted settle(id), the one thing besides an  *)
(* execute that raises newest, is for an id the lock retains at that      *)
(* step (checked against the window, apart from how SettleErr decides).   *)
SettleVouch == [][ev'.k = "settle_ok" => ev'.id \in Retained]_vars

(* EX-7 at the moment of forgetting: every job advance drops is pruned.   *)
ForgetOnlyPruned == [][\A j \in DOMAIN jobs \ DOMAIN jobs' : j < firstId]_vars

(* EX-6, EX-8: an execute that makes a job (E:77) and an accepted         *)
(* settle's on_settled (E:157-158) raise newest to at least the id.       *)
AdvanceOnVouch ==
    [][ev'.k \in {"exec_new", "on_settled"} => newest' >= ev'.id]_vars

(* EX-8, the converse (E:31-35): newest never falls, and rises only to    *)
(* the id of an execute that makes a job or of an accepted settle's       *)
(* on_settled. A refused settle, a stored or forgotten id's execute, a    *)
(* panic and the full cap move nothing.                                   *)
NewestOnVouch ==
    [][\/ newest' = newest
       \/ /\ newest' > newest
          /\ ev'.k \in {"exec_new", "on_settled"}
          /\ newest' = ev'.id]_vars

StepExec ==
    SettleVouch /\ ForgetOnlyPruned /\ AdvanceOnVouch /\ NewestOnVouch

-----------------------------------------------------------------------------
(* Witnesses (non-vacuity): the runner requires each to be VIOLATED.       *)

(* The lock prunes twice.                                                 *)
Witness_Pruned == firstId < 2
(* MAX_JOBS jobs.                                                         *)
Witness_Cap == Cardinality(DOMAIN jobs) < MaxJobs
(* T-JOBCAP: an execute the lock sent for a retained record, not stale,   *)
(* that frees no job, fails "full" (docs/api.md:42, E:78).                *)
Witness_JobCapExec ==
    ~(\E i \in execQ :
        /\ i \in Retained /\ i \notin DOMAIN jobs /\ i + W > newest
        /\ Cardinality(DOMAIN Keep(Max(newest, i))) >= MaxJobs)
(* T-JOBCAP: confirm_not_executed of a retained in-flight record fails    *)
(* "full" (E:135).                                                        *)
Witness_JobCapConfirm ==
    ~(\E i \in Retained : St(i) = "F" /\ i \notin DOMAIN jobs /\ i + W > newest
                          /\ Cardinality(DOMAIN jobs) >= MaxJobs)
(* confirm_not_executed of a forgotten id fails "stale id" (E:134).       *)
Witness_ConfirmStale ==
    ~(\E i \in Ids : i \notin DOMAIN jobs /\ i + W <= newest)

WitStep_ExecStale  == [][ev'.k # "exec_stale"]_vars
WitStep_ExecStored == [][ev'.k # "exec_stored"]_vars
WitStep_ExecFull   == [][ev'.k # "exec_full"]_vars
(* An execute that makes a job forgets another (E:77).                    *)
WitStep_ExecForgets ==
    [][~(ev'.k = "exec_new" /\ ~(DOMAIN jobs \subseteq DOMAIN jobs'))]_vars
(* An accepted settle's on_settled raises newest and forgets a job.       *)
WitStep_SettleForgets ==
    [][~(ev'.k = "on_settled" /\ newest' > newest
         /\ ~(DOMAIN jobs \subseteq DOMAIN jobs'))]_vars
(* A refused settle of a confirmation above newest: nothing moves.        *)
WitStep_SettleRefused ==
    [][~(ev'.k = "settle_refused" /\ ev'.id > newest)]_vars
(* The documented exit from the cap (docs/api.md:42): finish(id, false)   *)
(* of a refused job, now accepted, raises newest and frees a slot.        *)
WitStep_CapExit ==
    [][~(/\ ev'.k = "on_settled" /\ ev'.id \in refused
         /\ Cardinality(DOMAIN jobs) >= MaxJobs
         /\ Cardinality(DOMAIN jobs') < MaxJobs)]_vars
=============================================================================
