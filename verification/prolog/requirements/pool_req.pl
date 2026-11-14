%% pool_req.pl -- Requirements of shared-pool-controls, transcribed from the upstream docs
%%
%% PROP: C-1 C-2 C-3 C-4 C-5 C-6
%%
%% Sources (v1.1.0, pin 57b3f02 = upstream PR #4 head 6fcbd56, source-mirror/docs/):
%%   spec.md                    cited as spec:<line>
%%   api.md                     cited as api:<line>
%%   verification-handoff.md    cited as handoff:<line>
%% v1.0.0 text that v1.1.0 deleted is cited as spec@v1.0.0:<line> (likewise
%% api@, handoff@): the findings, the closed divergences and the threats'
%% history. The v1.0.0 -> v1.1.0 line map is .context/shared-pool/v1.1-verify/
%% docmap.tsv; the two low-similarity rows were checked by hand:
%%   spec@v1.0.0:46 -> spec:48. v1.0.0 "A checked inverse can fail if another
%%     commitment depends on its contribution ... resolve dependent work first
%%     ... In the supplied fixtures ... prevent ordinary unsafe reliance on
%%     unexecuted work" is deleted. spec:48 ("Admission refuses any commitment
%%     whose outcomes could make a metric worse than now, so the checked
%%     inverse cannot fail in a reachable state. There is still no forced
%%     administrative release.") replaces it with the opposite claim; the cap
%%     half is spec:50; the fixture sentence's successor is spec:70's last
%%     sentence (risk) and handoff:23.
%%   handoff@v1.0.0:31 -> handoff:34. v1.0.0 "A rejected checked inverse
%%     release leaves liability intact. Liveness is conditional on resolving
%%     dependencies and receiving authoritative evidence." is deleted;
%%     handoff:34 ("... no completion or release can fail the bounds check
%%     (LIVE) ... Liveness is conditional only on authoritative evidence and
%%     gas.") is its successor, without the dependency condition.
%% Every fact carries the line it was transcribed from. Where the docs are
%% silent the fact says so: Basis code_only(Why) means the docs give no
%% condition, name or position, and the code's choice is recorded so that
%% the check can still be exact (and so the gap is visible: C-2 and C-3 count
%% them). doc_condition(Cite) means the docs state the condition but not the
%% error name or the position. Readings the text leaves open are AMBIGUOUS;
%% places where the docs and the code disagree are divergence/4 facts (none
%% at v1.1.0; the v1.0.0 ones are closed_divergence/5).
%%
%% INDEPENDENCE NOTE (ASM-SPEC). Unlike near-approval's M4, the author of this
%% layer had read the code (the shared brief transcribes it). Doc-backed facts
%% are cited to the doc line that states them; nothing marked doc(_) relies on
%% the code. The code-only facts are the code's choices and are labelled.
%%
%% Exported:
%%   requirement(Id, Cite, Text)
%%   method(Name, Kind, Authority, Cite)        Kind: init | view | change | callback
%%       Authority: self | anyone | actor | executor | actor_or_owner | owner
%%   method_core(Method, CoreFn)                the core transition a method drives
%%   guard(OpKind, Order, GuardId, Outcome, Basis)
%%       Outcome = err(Error) | noop (Ok, nothing changes); the first guard whose
%%       condition fails decides; if none fails the effect applies
%%   guard_sequence(OpKind, [Outcome..]), guard_ids(OpKind, [GuardId..])
%%   guard_condition(OpKind, GuardId, [Conjunct..], Cite)
%%       the documented failure condition of a guard whose condition C-2
%%       compares with the code's `if` (record_bound, version_cap)
%%   op_kind(OpKind)
%%   pool_error(Error, Cite)                    error names the docs use
%%   life(OpClass, Flag, From, Outcome, Basis)  lifecycle table
%%       (Outcome to(S) | same | reject(Error))
%%   status(S), terminal(S), open_status(S)
%%   constant(Name, Value, Cite)
%%   fixture(Name, TraceId, Cite), fixture_metric(Fixture, Name, Index, Basis)
%%   fixture_without_claims(Name, Why)          a fixtures::NAMES entry with no claim
%%   fixture_claim(ClaimId, Fixture, Claim, Cite)
%%   known_finding(ClaimId, FindingId)          a claim the code is known to break:
%%                                              C-5 must REPRODUCE the counterexample
%%   finding(Id, Severity, Cite, Text, Status)  the v1.0.0 findings F-1..F-6;
%%       Status = open | fixed(Version, [PropertyId..]) (the properties that
%%       show the fix; strategy.md section 4.5's FR table)
%%   finding(Id, Severity, Cite, Text)          the same without the status
%%   divergence(Id, Cite, DocSays, CodeDoes)    open spec-vs-code disagreements
%%   closed_divergence(Id, OldCite, NewCite, Was, Resolution)
%%   ambiguity(Id, Cite, Text, Reading)
%%   resolved_ambiguity(Id, OldCite, NewCite, Resolution)
%%   decision(Id, Text)

:- module(pool_req, [
    requirement/3,
    method/4,
    method_core/2,
    guard/5,
    guard_sequence/2,
    guard_ids/2,
    guard_condition/4,
    op_kind/1,
    pool_error/2,
    life/5,
    status/1,
    terminal/1,
    open_status/1,
    constant/3,
    fixture/3,
    fixture_metric/4,
    fixture_without_claims/2,
    fixture_claim/4,
    known_finding/2,
    finding/5,
    finding/4,
    divergence/4,
    closed_divergence/5,
    ambiguity/4,
    resolved_ambiguity/4,
    decision/2,
    camel_snake/2
]).

:- use_module(library(lists)).
:- use_module(library(pairs)).

% v1.1.0 has no known finding and no open divergence (both stay queryable).
:- dynamic(known_finding/2).
:- dynamic(divergence/4).

%%% ========================================================
%%% Requirements (one per doc statement that a check or property relies on)
%%% ========================================================

% spec.md, State and identity
requirement('S-STATE', 'spec:7', "Pool = (config, values, version, observed_at, paused, records, first_id, completed_units).").
requirement('S-WINDOW', 'spec:9', "records is a window of at most 256 records, oldest first, with consecutive ids records[k].id = first_id + k; ids never repeat; completed_units[r] counts recipe r's completed units over the pool's life, pruned records included.").
requirement('S-CONFIG', 'spec:11', "Config is immutable: one owner, bounded named metrics, potential terms and recipes; each recipe has one exact actor, one exact executor distinct from the actor, reservation and completion effects, a maximum quantity and earlier-recipe completion prerequisites (recipe_id, minimum_completed_units); the dependency order is acyclic by construction.").
requirement('S-INIT', 'spec:13', "Both initializers are private to their own deployment account.").
requirement('S-PRED', 'spec:13', "The wrapper authorizes by immediate predecessor account, not a JSON actor field and not signer_account_id; calls through an intermediary are attributed to that intermediary.").
requirement('S-PREVIEW-NOAUTH', 'spec:13', "Preview accepts a proposed actor but conveys no authorization.").
requirement('S-NUM', 'spec:15', "Metric values and coefficients lie in [-1,000,000, 1,000,000]; every update is checked; each term's residual is at most 2,000,000 and weight at most 1,000; up to 64 terms sum to at most 256,000,000,000,000,000, below u64::MAX.").
requirement('S-SERIAL', 'spec:15', "Energy is serialized as a decimal string; all exposed metric integers are within JavaScript's exact range.").
requirement('S-EXPIRY', 'spec:15', "Request expiry is an epoch-seconds value.").
% spec.md, Admission
requirement('S-ADM-ORDER', 'spec:21', "Admission checks paused, version equality, expiry strictly after now and at most one day ahead, observation freshness if configured, record bound (Full only when 256 records are retained and the oldest is open), recipe validity, exact caller, quantity range, and completed-unit prerequisites (completed_units).").
requirement('S-ADM-CANDIDATE', 'spec:21', "Candidate values = values + reservation delta * units, with checked arithmetic.").
requirement('S-ADM-HARD', 'spec:23', "A conservative per-metric interval contains every outcome of every open record (it stays open, completes, or releases its reservation, independently of the others) plus the proposed one; no metric's interval violation may exceed its violation before the request or after the reservation; in a healthy state all intervals fit.").
requirement('S-ADM-ENERGY', 'spec:25', "Require V(candidate) <= V(current); in a state already violating hard bounds also require V(candidate + proposed_completion) < V(current).").
requirement('S-ADM-RECOVERY-SCOPE', 'spec:25', "A reservation never changes a metric's violation amount; recovery comes from completion. The recovery rule is an admission rule only; it does not guarantee that execution arrives or recovery completes.").
requirement('S-ADM-EFFECT', 'spec:27', "On success append a Reserved record with id first_id + len(records), replace values, increment version; if 256 records are retained, first drop the oldest, which is terminal, and increment first_id; on rejection return an error with no state change.").
requirement('S-PRUNE', 'spec:27', "A pruned id is NotFound for every call; its history stays in the event log.").
requirement('S-ADM-NOREPLAY', 'spec:27', "Accepted requests cannot replay at the original version; there is no separate client nonce mechanism.").
requirement('S-ADM-WRAPPER', 'spec:27', "The wrapper may emit a rejection event but does not consume a nonce or persist an audit record.").
requirement('S-PREVIEW', 'spec:29', "preview calls the same checker and returns candidate values, score terms and reason; it is advisory.").
requirement('S-BEST', 'spec:29', "best_quantity enumerates positive quantities up to the requested maximum, minimizes energy then favors larger quantity; it only suggests.").
% spec.md, Potential
requirement('S-POTENTIAL', 'spec:33', "Each term references one metric and computes a weighted squared target deviation, excess or deficit; agents cannot supply deltas or invent targets.").
requirement('S-HARD-AXIS', 'spec:35', "Hard constraints are axis-aligned metric bounds.").
% spec.md, Lifecycle
requirement('S-LIFE-DISPATCH', 'spec:39', "Reserved -> Dispatched: exact actor, unpaused, unexpired, fresh risk snapshot; the wrapper creates the promise after the state update.").
requirement('S-LIFE-ACTIVE', 'spec:40', "Dispatched/Unknown -> Active: private acknowledgment says the adapter accepted dispatch.").
requirement('S-LIFE-UNKNOWN', 'spec:41', "Dispatched/Unknown -> Unknown: missing, malformed or failed acknowledgment; retain values. Unknown -> Unknown is an Ok no-op without a version change.").
requirement('S-LIFE-COMPLETE', 'spec:42', "Dispatched/Active/Unknown -> Completed: the recipe executor attests completion; add completion delta, check the envelope of the remaining open records, add the units to completed_units, increment version; no energy comparison.").
requirement('S-LIFE-CANCEL', 'spec:43', "Reserved -> Cancelled: actor or owner cancels before dispatch; subtract reservation delta and check other pending effects. Cancel of a Dispatched, Active or Unknown record returns Pending; of a Completed record, InvalidState.").
requirement('S-LIFE-NONEXEC', 'spec:44', "Dispatched/Active/Unknown -> Cancelled: executor attests definitive nonexecution; same checked inverse update.").
requirement('S-LIFE-IDEMPOTENT', 'spec:45', "Same completion/cancellation repeated: idempotent, Ok without a version change; conflicting terminal evidence: reject with InvalidState.").
requirement('S-LIFE-LATEACK', 'spec:46', "Late acknowledgment after terminal settlement: ignored, Ok without a version change.").
requirement('S-INVERSE', 'spec:48', "Admission refuses any commitment whose outcomes could make a metric worse than now, so the checked inverse cannot fail in a reachable state; there is still no forced administrative release.").
requirement('S-FIXTURE-RELIANCE', 'spec:48; spec:70', "No commitment relies on another's unexecuted work: the checked inverse cannot fail in a reachable state (spec:48), and in the risk fixture no unexecuted position backs a hedge (spec:70). Successor of spec@v1.0.0:46's fixture sentence.").
requirement('S-VERSION-RESERVE', 'spec:50', "Submit (as its last check, so preview reports it), dispatch, set_paused and observe fail Arithmetic once version reaches VERSION_CAP = u32::MAX - 768; acknowledgment, settlement and cancellation keep the plain checked increment and never overflow; at the cap the pool admits nothing new but every open record can still be resolved.").
requirement('S-NOEXPIRY', 'spec:52', "No automatic expiry of a dispatched request; reconciliation queries the fixed executor by immutable id; Unknown is distinct from cancelled.").
requirement('S-ADAPTER', 'spec:54', "The mock adapter binds id to recipe/quantity, dedupes identical dispatch, rejects a different payload; it forgets a job only once its id is 256 or more behind the newest id the lock vouched for; only the lock executes; only the operator finishes/cancels; it stores terminal evidence before notifying the lock.").
% spec.md, External observations and administration
requirement('S-PAUSE', 'spec:58', "set_paused is owner-only and increments version; pausing blocks new admissions and dispatch, not settlement or reconciliation.").
requirement('S-OBSERVE', 'spec:58', "observe is owner-only; it sets new values only when every record is terminal; restating the current values is allowed at any time and renews the snapshot timestamp; it replaces bounded measured values without changing policy; it can create a hard-limit violation.").
requirement('S-NOADMIN', 'spec:60', "No policy upgrades, role mutation, arbitrary funds transfer, dynamic plugins, or reset entry point.").
% spec.md, Scenario semantics
requirement('S-GPU', 'spec:64', "GPU/Friday: coverage and occupancy include reserved and completed bookings; completion does not free historical slots; Friday also consumes dispatch slots atomically.").
requirement('S-STOCK', 'spec:66', "Inventory: positions move at reservation; source availability decreases and transit increases at reservation; receipt moves transit into destination availability; incoming position is never available stock; available_depot + available_A + available_B + transit = 12 across ordinary transfer transitions.").
requirement('S-CASH', 'spec:68', "Hiring: reservation consumes funds or earmark and increases coverage plus escrow; acceptance moves escrow to paid; coverage remains after payment; free + earmark + escrow + paid + protected = 10; review needs completed research >= 2 and implementation >= 3; no worker can authorize its own acceptance.").
requirement('S-RISK', 'spec:70', "Risk: positive loss contributions reserve immediately; hedge benefit occurs only on completion; collateral encumbrance remains until a later measured baseline; no pending hedge offsets are netted into new trade capacity; no unexecuted position backs a hedge.").
% api.md
requirement('A-NONPAYABLE', 'api:5', "All mutating methods are nonpayable, including reconcile.").
requirement('A-METHODS', 'api:7-20', "The method table: names, arguments and authority of new, get_pool, get_commitments, preview, submit, dispatch, reconcile, settle, cancel, set_paused, observe, on_result.").
requirement('A-PAGE', 'api:11', "get_commitments reads a page of at most 50, from id `from`; ids below first_id were pruned.").
requirement('A-SUBMIT-RESULT', 'api:13', "submit returns {decision,id}; no id on rejection.").
requirement('A-ONRESULT', 'api:20', "on_result never panics on its input: a result for an id with no record changes nothing and is logged (reconciled with error NotFound).").
requirement('A-WINDOW', 'api:28', "A submit prunes the oldest retained record when 256 are retained and it is terminal; while it is open submit fails Full, even if the other 255 are terminal; every call on a pruned id fails NotFound, including a repeated executor settle.").
requirement('A-VERSION', 'api:30', "Every effective state transition increments version by exactly 1; calls that change nothing return Ok and leave the version alone: a repeated terminal settlement, a repeated cancel, an acknowledgment that does not change the status, a late acknowledgment.").
requirement('A-VERSION-CAP', 'api:30', "Submit, dispatch, pause and observe stop 768 versions below u32::MAX (Arithmetic); acknowledgment, settlement and cancellation always remain available.").
requirement('A-EXPIRY-MAX', 'api:30', "Maximum expiry is 24 hours ahead.").
requirement('A-REASONS', 'api:32', "Reasons include Unauthorized, Paused, Stale, Expired, SnapshotExpired, InvalidRecipe, InvalidUnits, Prerequisite, Full, Arithmetic, Bounds, Energy.").
requirement('A-AFTER', 'api:32', "after can be present for a rejected request to explain its prospective score; it is not accepted state.").
requirement('A-EXECUTOR', 'api:40', "execute({id,recipe,units}) -> active | cancelled | unknown, only the lock may call; status({id}) also returns completed; any result that is not one JSON string of at most 64 bytes, and any unexpected string, becomes Unknown in the wrapper.").
requirement('A-EXEC-WINDOW', 'api:42', "The synthetic adapter forgets a job once its id is 256 or more behind its newest id; execute of a forgotten id returns cancelled and starts nothing; on_settled raises the newest id only if the lock accepted; at most 512 jobs.").
requirement('A-GAS', 'api:46; api:57', "Attach at least the documented prepaid gas; a call with too little gas fails without changing either contract.").
requirement('A-CONFIG-ROLES', 'api:65', "A recipe's actor and executor must be different accounts; new rejects the config otherwise (InvalidConfig).").
% verification-handoff.md obligations (handoff:11-26) and nuances (handoff:32-39)
requirement('H-CFG', 'handoff:11', "Accepted configuration references are valid and all loops/data are bounded.").
requirement('H-NUM', 'handoff:12', "Valid bounded state/terms cannot overflow energy; all delta overflows fail closed.").
requirement('H-AUTH', 'handoff:13', "Only the recipe actor admits/dispatches, executor settles, owner observes/pauses.").
requirement('H-ADM', 'handoff:14', "Accepted reservation has nonincreasing potential; hard-violation envelopes do not worsen.").
requirement('H-ATOM', 'handoff:15', "All rejected core mutations preserve the full state.").
requirement('H-STALE', 'handoff:16', "A request computed at a prior accepted version cannot replay.").
requirement('H-ASYNC', 'handoff:17', "Unknown outcomes retain reservation until authoritative terminal evidence.").
requirement('H-TERM', 'handoff:18', "Terminal repeated evidence is idempotent; conflicting outcome cannot apply.").
requirement('H-DEP', 'handoff:19', "Prerequisite requires completed units, not acknowledgment or partial completion; pruning keeps the count.").
requirement('H-BIND', 'handoff:20', "Each dispatch id fixes actor, recipe, units and executor.").
requirement('H-STOCK', 'handoff:21', "Inventory fixture conserves available + transit and never spends inbound pipeline.").
requirement('H-CASH', 'handoff:22', "Hiring fixture conserves free + earmark + escrow + paid + protected; payment preserves coverage.").
requirement('H-RISK', 'handoff:23', "Pending negative exposure does not grant new risk headroom, and no hedge relies on an unexecuted position.").
requirement('H-LIVE', 'handoff:24', "Every open record can be resolved.").
requirement('H-WIN', 'handoff:25', "At most 256 retained records with consecutive ids; Full only while the oldest is open; only terminal records are pruned, and a pruned id is NotFound.").
requirement('H-EXEC', 'handoff:26', "The executor never forgets an id whose lock record is retained; at most 256 jobs at or below newest; at most 512 jobs.").
requirement('H-INTERVAL', 'handoff:33', "Hard checks use intervals that contain every outcome of every open record (stays open, completes, or releases its reservation); they should never underapproximate the modeled independent effects.").
requirement('H-LIVENESS', 'handoff:34', "Each metric's interval violation is at most its violation before the request and after the reservation, so in every reachable state no completion or release can fail the bounds check, and with the version reserve the increment cannot fail either; liveness is conditional only on authoritative evidence and gas.").
requirement('H-NOMONO', 'handoff:36', "Energy is not checked on lifecycle reversal or observation.").
requirement('H-VERSION', 'handoff:37', "Version reserve: submit, dispatch, pause and observe stop 768 versions below u32::MAX; an open record needs at most three more increments and there are at most 256 records, so the reserve is exactly enough; every effective transition increments version by exactly 1; pause/observe still advance version.").
requirement('H-WINDOW', 'handoff:38', "The pool keeps its 256 most recent records; a submit into a full window prunes the oldest if terminal and fails Full if open (head-of-line until the executor reports); ids never repeat; a pruned id is NotFound; prerequisites read completed_units.").
requirement('H-JOBCAP', 'handoff:39', "The synthetic executor holds at most 512 jobs; honest use holds 256 jobs plus confirmations in flight; at the cap confirm_not_executed and an execute that frees no job fail full.").

%%% ========================================================
%%% Methods and authority (api.md table api:7-20, spec:13)
%%% ========================================================

method(new,             init,     self,           'api:9 + spec:13').
method(get_pool,        view,     anyone,         'api:10').
method(get_commitments, view,     anyone,         'api:11').
method(preview,         view,     anyone,         'api:12 + spec:13').   % actor is hypothetical
method(submit,          change,   actor,          'api:13').
method(dispatch,        change,   actor,          'api:14 + spec:39').
method(reconcile,       change,   anyone,         'api:15; api:5').
method(settle,          change,   executor,       'api:16 + spec:42,44').
method(cancel,          change,   actor_or_owner, 'api:17 + spec:43').
method(set_paused,      change,   owner,          'api:18 + spec:58').
method(observe,         change,   owner,          'api:19 + spec:58').
method(on_result,       callback, self,           'api:20').
% AMBIGUOUS (amb-reconcile-kind): api:15 makes reconcile callable by anyone and
% api:5 counts it among the mutating methods (nonpayable); it changes nothing
% itself (the private callback applies the result). Recorded as kind change
% with authority anyone; C-1 checks the code grants it no authority.

% The core transition each method drives (api:13-20; spec:39-46). A method may
% drive several (on_result: settle or acknowledge, api:40 + spec:40-44). The
% shell also reads records through the core's &self `record(id)` (dispatch,
% reconcile, on_result); reads are not transitions (C-1 checks they are &self).
method_core(submit,     submit).
method_core(dispatch,   dispatch).
method_core(settle,     settle).
method_core(cancel,     cancel).
method_core(set_paused, set_paused).
method_core(observe,    observe).
method_core(on_result,  settle).
method_core(on_result,  acknowledge).
method_core(preview,    preview).

%%% ========================================================
%%% Errors the docs name
%%% ========================================================
%% api:32 lists submit's reasons (C-2 api32_reasons compares exactly these,
%% cited 'api:32', with the errors reachable in the code's candidate()). v1.1.0
%% also names the lifecycle errors and InvalidConfig.

pool_error('Unauthorized',    'api:32').
pool_error('Paused',          'api:32').
pool_error('Stale',           'api:32').
pool_error('Expired',         'api:32').
pool_error('SnapshotExpired', 'api:32').
pool_error('InvalidRecipe',   'api:32').
pool_error('InvalidUnits',    'api:32').
pool_error('Prerequisite',    'api:32').
pool_error('Full',            'api:32').
pool_error('Arithmetic',      'api:32').
pool_error('Bounds',          'api:32').
pool_error('Energy',          'api:32').
pool_error('NotFound',        'spec:27 "A pruned id is NotFound for every call"; api:20; api:28').
pool_error('Pending',         'spec:43 "A cancel of a Dispatched, Active or Unknown record returns Pending"; api:17').
pool_error('InvalidState',    'spec:43; spec:45 "reject with InvalidState"; api:17; api:42').
pool_error('InvalidConfig',   'api:65 "new rejects the config otherwise (InvalidConfig)"').

%%% ========================================================
%%% Guard tables
%%% ========================================================
%% Op kinds: submit, dispatch, ack (acknowledge), cancel, settle_completed,
%% settle_cancelled (settle(_, _, true|false)), pause (set_paused), observe.
%%
%% v1.1.0 guard ids (pool_machine.pl's holds/4 implements one clause per id):
%%   version_cap      submit 15, dispatch 7, pause 2, observe 4: Arithmetic
%%                    once version >= VERSION_CAP (spec:50); replaces their
%%                    v1.0.0 version_overflow
%%   version_overflow ack 4, cancel 9, settle_* 8: the plain checked increment
%%                    (spec:50 "keep the plain checked increment"); never fires
%%                    from an INV state (INV-VERCAP; C-2 bump_reserve)
%%   not_completed    cancel 4 (new): a Completed record is InvalidState (spec:43)
%%   all_terminal_or_restate  observe 2 (was all_terminal): Pending only for
%%                    new values while a record is open (spec:58)
%%   record_bound     submit 5, new meaning: Full only when 256 records are
%%                    retained and the oldest is open (spec:21)
%%   prerequisites    submit 9, new meaning: completed_units (spec:21)
%%   envelope_arith, bounds: the release-aware interval of spec:23 (every
%%                    outcome of every open record; violation against
%%                    min(before, after the reservation))

op_kind(submit).
op_kind(dispatch).
op_kind(ack).
op_kind(cancel).
op_kind(settle_completed).
op_kind(settle_cancelled).
op_kind(pause).
op_kind(observe).

% --- submit (spec:19-27, api:13, api:28-32)
guard(submit,  1, paused,        err('Paused'),          doc('spec:21 "Check paused"')).
guard(submit,  2, version,       err('Stale'),           doc('spec:21 "version equality"; api:30')).
guard(submit,  3, expiry,        err('Expired'),         doc('spec:21 "expiry strictly after now and at most one day ahead"; api:30')).
guard(submit,  4, freshness,     err('SnapshotExpired'), doc('spec:21 "observation freshness if configured"')).
guard(submit,  5, record_bound,  err('Full'),            doc('spec:21 "record bound (Full only when 256 records are retained and the oldest is open)"; api:28')).
guard(submit,  6, recipe,        err('InvalidRecipe'),   doc('spec:21 "recipe validity"')).
guard(submit,  7, caller,        err('Unauthorized'),    doc('spec:21 "exact caller"; api:13')).
guard(submit,  8, units,         err('InvalidUnits'),    doc('spec:21 "quantity range"')).
guard(submit,  9, prerequisites, err('Prerequisite'),    doc('spec:21 "completed-unit prerequisites (completed_units)"; handoff:19')).
guard(submit, 10, reserve_arith, err('Arithmetic'),      doc('spec:21 "with checked arithmetic"')).
guard(submit, 11, envelope_arith, err('Arithmetic'),     doc_condition('spec:15 "Every update is checked"; spec:23 interval')).
guard(submit, 12, bounds,        err('Bounds'),          doc('spec:23')).
guard(submit, 13, energy,        err('Energy'),          doc('spec:25 "Then require V(candidate) <= V(current)"')).
guard(submit, 14, recovery,      err('Energy'),          doc_condition('spec:25 second sentence; the error name is not stated')).
guard(submit, 15, version_cap,   err('Arithmetic'),      doc('spec:50 "submit (as its last check, so preview reports it) ... fail Arithmetic once version reaches VERSION_CAP"; api:30; handoff:37')).

% --- dispatch (spec:39, api:14)
guard(dispatch, 1, found,       err('NotFound'),        doc('spec:27 "A pruned id is NotFound for every call"; api:28')).
guard(dispatch, 2, caller,      err('Unauthorized'),    doc('spec:39 "exact actor"; api:14')).
guard(dispatch, 3, paused,      err('Paused'),          doc('spec:39 "unpaused"; spec:58')).
guard(dispatch, 4, reserved,    err('InvalidState'),    doc_condition('spec:39 source state Reserved; name and position code-only')).
guard(dispatch, 5, unexpired,   err('Expired'),         doc('spec:39 "unexpired"')).
guard(dispatch, 6, freshness,   err('SnapshotExpired'), doc('spec:39 "fresh risk snapshot"')).
guard(dispatch, 7, version_cap, err('Arithmetic'),      doc('spec:50 "dispatch ... fail Arithmetic once version reaches VERSION_CAP"; api:30')).

% --- acknowledge (spec:40-41, 46; private callback api:20)
guard(ack, 1, found,            err('NotFound'),   doc('spec:27; api:20 "a result for an id with no record changes nothing" (the shell checks first; the core still fails NotFound)')).
guard(ack, 2, awaiting,         noop,              doc('spec:40-41 sources Dispatched/Unknown; spec:46 late ack ignored, Ok without a version change (Reserved and Active+false: amb-ack-other)')).
guard(ack, 3, changes_status,   noop,              doc('spec:41 "Unknown -> Unknown is an Ok no-op without a version change"; api:30 "an acknowledgment that does not change the status"')).
guard(ack, 4, version_overflow, err('Arithmetic'), doc('spec:50 "Acknowledgment ... keep the plain checked increment"; never fires from an INV state (INV-VERCAP)')).

% --- cancel (spec:43, 45, api:17)
guard(cancel, 1, found,            err('NotFound'),     doc('spec:27 "A pruned id is NotFound for every call"; api:28')).
guard(cancel, 2, caller,           err('Unauthorized'), doc('spec:43 "actor or owner"; api:17')).
guard(cancel, 3, not_cancelled,    noop,                doc('spec:45 "Same ... cancellation repeated: idempotent, Ok without a version change"; api:17 "Already Cancelled: Ok, no change"')).
guard(cancel, 4, not_completed,    err('InvalidState'), doc('spec:43 "A cancel of a Completed record returns InvalidState"; api:17')).
guard(cancel, 5, reserved,         err('Pending'),      doc('spec:43 "A cancel of a Dispatched, Active or Unknown record returns Pending"; api:17')).
guard(cancel, 6, release_arith,    err('Arithmetic'),   doc('spec:43 "Subtract reservation delta"; spec:15 checked')).
guard(cancel, 7, envelope_arith,   err('Arithmetic'),   doc_condition('spec:15 "Every update is checked"')).
guard(cancel, 8, bounds,           err('Bounds'),       doc('spec:43 "check other pending effects"; spec:48 (cannot fail in a reachable state)')).
guard(cancel, 9, version_overflow, err('Arithmetic'),   doc('spec:50 "cancellation ... keep the plain checked increment"')).

% --- settle (spec:42, 44, 45, api:16)
guard(settle_completed, 1, found,          err('NotFound'),     doc('spec:27; api:28 "including a repeated executor settle"')).
guard(settle_completed, 2, caller,         err('Unauthorized'), doc('spec:42 "recipe executor"; api:16')).
guard(settle_completed, 3, not_same_terminal, noop,             doc('spec:45 "Same completion ... repeated: idempotent, Ok without a version change"')).
guard(settle_completed, 4, in_flight,      err('InvalidState'), doc('spec:42 sources Dispatched/Active/Unknown; spec:45 "Conflicting terminal evidence: reject with InvalidState"; api:42 Reserved: InvalidState')).
guard(settle_completed, 5, complete_arith, err('Arithmetic'),   doc('spec:42 "add completion delta"; spec:15 checked')).
guard(settle_completed, 6, envelope_arith, err('Arithmetic'),   doc_condition('spec:15 "Every update is checked"')).
guard(settle_completed, 7, bounds,         err('Bounds'),       doc('spec:42 "check the envelope of the remaining open records"; spec:48')).
guard(settle_completed, 8, version_overflow, err('Arithmetic'), doc('spec:50 "settlement ... keep the plain checked increment"')).

guard(settle_cancelled, 1, found,          err('NotFound'),     doc('spec:27; api:28 "including a repeated executor settle"')).
guard(settle_cancelled, 2, caller,         err('Unauthorized'), doc('spec:44 "executor attests"; api:16')).
guard(settle_cancelled, 3, not_same_terminal, noop,             doc('spec:45 "Same ... cancellation repeated: idempotent, Ok without a version change"')).
guard(settle_cancelled, 4, in_flight,      err('InvalidState'), doc('spec:44 sources Dispatched/Active/Unknown; spec:45 "reject with InvalidState"; api:42 "of a record still Reserved (InvalidState)"')).
guard(settle_cancelled, 5, release_arith,  err('Arithmetic'),   doc('spec:44 "same checked inverse update"')).
guard(settle_cancelled, 6, envelope_arith, err('Arithmetic'),   doc_condition('spec:15 "Every update is checked"')).
guard(settle_cancelled, 7, bounds,         err('Bounds'),       doc('spec:44 + spec:43 "check other pending effects"; spec:48')).
guard(settle_cancelled, 8, version_overflow, err('Arithmetic'), doc('spec:50 "settlement ... keep the plain checked increment"')).

% --- set_paused (spec:58, api:18)
guard(pause, 1, owner,       err('Unauthorized'), doc('spec:58 "owner-only"; api:18')).
guard(pause, 2, version_cap, err('Arithmetic'),   doc('spec:50 "set_paused ... fail Arithmetic once version reaches VERSION_CAP"; api:30')).

% --- observe (spec:58, api:19)
guard(observe, 1, owner,                   err('Unauthorized'), doc('spec:58 "owner-only"; api:19')).
guard(observe, 2, all_terminal_or_restate, err('Pending'),      doc_condition('spec:58 "sets new values only when every record is terminal; restating the current values is allowed at any time"; api:19; name code-only')).
guard(observe, 3, bounded_values,          err('Bounds'),       doc_condition('spec:58 "replaces bounded measured values"; length check and name code-only')).
guard(observe, 4, version_cap,             err('Arithmetic'),   doc('spec:50 "observe fail Arithmetic once version reaches VERSION_CAP"; api:30')).
% CODE-ONLY: observe also calls energy(values); with the values bounded (guard 3)
% it cannot fail (NUM-1, spec:15), so it is not a guard here.

%% guard_sequence(+Kind, -Outcomes) / guard_ids(+Kind, -Ids): the specified order.
guard_sequence(Kind, Outcomes) :-
    op_kind(Kind),
    findall(N-O, guard(Kind, N, _, O, _), Ps), keysort(Ps, S), pairs_values(S, Outcomes).
guard_ids(Kind, Ids) :-
    op_kind(Kind),
    findall(N-G, guard(Kind, N, G, _, _), Ps), keysort(Ps, S), pairs_values(S, Ids).

%% guard_condition(Kind, GuardId, Conjuncts, Cite): the guard fails (returns
%% its error) exactly when every conjunct holds. Only the guards whose
%% condition the docs state as a formula over the window or the version are
%% listed; C-2 matches each to a code site with the same extracted condition
%% (code_error_cond/4), and no other guard may stand for such a site.
%%   ge(retained, max_records)  at least max_records records are retained
%%   open(front)                the oldest retained record is open
%%   ge(version, version_cap)   version has reached VERSION_CAP
guard_condition(submit,   record_bound, [ge(retained, max_records), open(front)],
                'spec:21 "Full only when 256 records are retained and the oldest is open"; api:28 "while it is open, submit fails Full"').
guard_condition(submit,   version_cap,  [ge(version, version_cap)],
                'spec:50 "submit (as its last check ...) ... fail Arithmetic once version reaches VERSION_CAP"').
guard_condition(dispatch, version_cap,  [ge(version, version_cap)], 'spec:50 "dispatch ... fail Arithmetic once version reaches VERSION_CAP"').
guard_condition(pause,    version_cap,  [ge(version, version_cap)], 'spec:50 "set_paused ... fail Arithmetic once version reaches VERSION_CAP"').
guard_condition(observe,  version_cap,  [ge(version, version_cap)], 'spec:50 "observe fail Arithmetic once version reaches VERSION_CAP"').

%%% ========================================================
%%% Lifecycle (spec:39-46, api:14-17, api:30)
%%% ========================================================
%% life(OpClass, Flag, From, Outcome, Basis): what a call does to the status of
%% the record it names, assuming every non-status guard passes.
%%   Outcome: to(S)     the status becomes S (S \== From: api:30 makes an Ok
%%                      call that changes nothing a no-op, so a self-loop is `same`)
%%            same      Ok and the status (and everything else) is unchanged
%%            reject(E) the error E
%%   Flag:    none, or the call's boolean (ack accepted, settle completed)

status(reserved).  status(dispatched).  status(active).
status(unknown).   status(completed).   status(cancelled).
terminal(completed).  terminal(cancelled).
open_status(S) :- status(S), \+ terminal(S).

% dispatch: spec:39 Reserved -> Dispatched; nothing else is listed.
life(dispatch, none, reserved, to(dispatched), doc('spec:39; api:14')).
life(dispatch, none, S, reject('InvalidState'), doc_condition('spec:39 only Reserved -> Dispatched; api:14; the name InvalidState is code-only')) :-
    status(S), S \== reserved.
% acknowledge
life(ack, true,  dispatched, to(active),  doc('spec:40')).
life(ack, true,  unknown,    to(active),  doc('spec:40')).
life(ack, false, dispatched, to(unknown), doc('spec:41')).
life(ack, false, unknown,    same,        doc('spec:41 "Unknown -> Unknown is an Ok no-op without a version change"; api:30')).
life(ack, true,  active,     same,        doc('api:30 "an acknowledgment that does not change the status"')).
life(ack, F, S, same, doc('spec:46 "Late acknowledgment after terminal settlement: ignored, Ok without a version change"; api:30')) :-
    member(F, [true, false]), terminal(S).
% AMBIGUOUS (amb-ack-other): the docs list no acknowledgment from Reserved, nor
% a refusal from Active. on_result runs only after dispatch (api:14-15), so
% Reserved cannot occur through the shell; a "unknown" from reconcile can reach
% an Active record.
life(ack, false, active,   same, code_only('docs silent: a refusal for an Active record is ignored')).
life(ack, F, reserved, same, code_only('docs silent: ack of a Reserved record is ignored (unreachable via on_result)')) :- member(F, [true, false]).
% cancel: spec:43, 45, api:17
life(cancel, none, reserved,  to(cancelled), doc('spec:43; api:17')).
life(cancel, none, cancelled, same,          doc('spec:45 idempotent; api:17 "Already Cancelled: Ok, no change"')).
life(cancel, none, S, reject('Pending'), doc('spec:43 "A cancel of a Dispatched, Active or Unknown record returns Pending"; api:17')) :-
    member(S, [dispatched, active, unknown]).
life(cancel, none, completed, reject('InvalidState'), doc('spec:43 "A cancel of a Completed record returns InvalidState"; api:17')).
% settle
life(settle, true,  S, to(completed), doc('spec:42')) :- member(S, [dispatched, active, unknown]).
life(settle, false, S, to(cancelled), doc('spec:44')) :- member(S, [dispatched, active, unknown]).
life(settle, true,  completed, same,  doc('spec:45 idempotent, Ok without a version change')).
life(settle, false, cancelled, same,  doc('spec:45 idempotent, Ok without a version change')).
life(settle, true,  cancelled, reject('InvalidState'), doc('spec:45 "Conflicting terminal evidence: reject with InvalidState"')).
life(settle, false, completed, reject('InvalidState'), doc('spec:45 "Conflicting terminal evidence: reject with InvalidState"')).
life(settle, false, reserved,  reject('InvalidState'), doc('api:42 "The lock refuses a confirmation ... of a record still Reserved (InvalidState)"; spec:44 lists only in-flight sources')).
life(settle, true,  reserved,  reject('InvalidState'), doc_condition('spec:42 lists only Dispatched/Active/Unknown sources; the name is stated for settle(false) only (api:42)')).

%%% ========================================================
%%% Constants
%%% ========================================================

constant(max_value,         1000000,  'spec:15 "[-1,000,000, 1,000,000]"').
constant(max_residual,      2000000,  'spec:15 "residual of at most 2,000,000"').
constant(max_weight,        1000,     'spec:15 "weight <= 1,000"').
constant(max_terms,         64,       'spec:15 "Up to 64 terms"').
constant(max_energy,        256000000000000000, 'spec:15 "sum to at most 256,000,000,000,000,000"').
constant(u64_max,           18446744073709551615, 'spec:15 "safely below u64::MAX"').
constant(js_exact_max,      9007199254740991, 'spec:15 "JavaScript\'s exact range" (2^53 - 1)').
constant(max_records,       256,      'spec:9 "a window of at most 256 records"; api:10; api:28; handoff:38').
constant(page_max,          50,       'api:11 "Read page, max 50"').
constant(expiry_horizon,    86400,    'spec:21 "at most one day ahead"; api:30 "Maximum is 24 hours"').
constant(version_max,       4294967295, 'spec:50 "u32::MAX"; api:30; handoff:37').
constant(version_cap,       4294966527, 'spec:50 "VERSION_CAP = u32::MAX - 768"; api:30 "768 versions below u32::MAX"; handoff:37').
% CODE-ONLY constants (docs silent): MAX_METRICS 32, MAX_RECIPES 32, MAX_UNITS 100,
% name length 128. C-4 compares them with the model's bounds.

%%% ========================================================
%%% Fixtures (spec:62-70, handoff:21-23)
%%% ========================================================
%% fixtures::NAMES (fixtures.rs:47) is [station, gpu, friday, inventory, hiring,
%% risk]. The conformance generator emits the docs' fixtures as traces 1-5
%% chosen BY NAME (trace contract v1.1; CODE-ONLY: conformance/src/main.rs), not
%% in NAMES order, and station as trace 212. C-5 checks every NAMES entry is
%% either a fixture/3 below or a fixture_without_claims/2.

fixture(gpu,       1, 'spec:64').
fixture(friday,    2, 'spec:64').
fixture(inventory, 3, 'spec:66; handoff:21').
fixture(hiring,    4, 'spec:68; handoff:22').
fixture(risk,      5, 'spec:70; handoff:23').

% station (fixtures.rs:58-78) is new in v1.1.0: it is built from gpu
% (extracted fixture_derived(station, gpu, 61)), renames metrics and recipes,
% gives each pair of windows to one ship actor and lowers metrics 3-4 to max 8.
% The docs make no claim about it beyond the demo story.
fixture_without_claims(station,
    "judges-guide.md:11, :26 and README:26 tell the docking demo; spec.md Scenario semantics (spec:62-70) has no station paragraph. Built from gpu (fixtures.rs:58-78; extracted fixture_derived(station, gpu, 61)); conformance trace 212.").

% Metric names the docs use -> index in the fixture's metric vector. The docs
% name the metrics; the order is the fixture code's (CODE-ONLY: fixtures.rs).
fixture_metric(inventory, position_depot,  0, code_only('fixtures.rs:158')).
fixture_metric(inventory, position_a,      1, code_only('fixtures.rs:159')).
fixture_metric(inventory, position_b,      2, code_only('fixtures.rs:160')).
fixture_metric(inventory, available_depot, 3, code_only('fixtures.rs:161')).
fixture_metric(inventory, available_a,     4, code_only('fixtures.rs:162')).
fixture_metric(inventory, available_b,     5, code_only('fixtures.rs:163')).
fixture_metric(inventory, transit,         6, code_only('fixtures.rs:164')).
fixture_metric(hiring, coverage_research,       0, code_only('fixtures.rs:198')).
fixture_metric(hiring, coverage_implementation, 1, code_only('fixtures.rs:199')).
fixture_metric(hiring, coverage_review,         2, code_only('fixtures.rs:200')).
fixture_metric(hiring, free,      3, code_only('fixtures.rs:201')).
fixture_metric(hiring, earmark,   4, code_only('fixtures.rs:202')).
fixture_metric(hiring, escrow,    5, code_only('fixtures.rs:203')).
fixture_metric(hiring, paid,      6, code_only('fixtures.rs:204')).
fixture_metric(hiring, protected, 7, code_only('fixtures.rs:205')).
fixture_metric(risk, selloff_loss,    0, code_only('fixtures.rs:240')).
fixture_metric(risk, funding_loss,    1, code_only('fixtures.rs:241')).
fixture_metric(risk, volatility_loss, 2, code_only('fixtures.rs:242')).
fixture_metric(risk, collateral,      3, code_only('fixtures.rs:243')).
fixture_metric(gpu, booking_1, 3, code_only('fixtures.rs:90')).
fixture_metric(gpu, booking_2, 4, code_only('fixtures.rs:101')).
fixture_metric(friday, booking_1, 3, code_only('fixtures.rs:90')).
fixture_metric(friday, booking_2, 4, code_only('fixtures.rs:101')).
fixture_metric(friday, slots_1,   5, code_only('fixtures.rs:115')).
fixture_metric(friday, slots_2,   6, code_only('fixtures.rs:117')).

% fixture_claim(Id, Fixture, Claim, Cite). Claim terms are interpreted by C-5.
fixture_claim('STOCK-SUM', inventory,
    conserved([available_depot, available_a, available_b, transit], 12),
    'spec:66 "available_depot + available_A + available_B + transit = 12 across ordinary transfer transitions"; handoff:21').
fixture_claim('STOCK-RESERVE', inventory,
    reserve_moves([available_depot, available_a, available_b], decrease, transit, increase),
    'spec:66 "Physical source availability decreases and reserved/in-transit quantity increases"').
fixture_claim('STOCK-RECEIPT', inventory,
    complete_moves(transit, decrease, [available_depot, available_a, available_b], increase),
    'spec:66 "Receipt moves transit into destination availability"').
fixture_claim('STOCK-POSITION', inventory,
    no_complete_effect([position_depot, position_a, position_b]),
    'spec:66 "planning positions move at reservation"').
fixture_claim('STOCK-INBOUND', inventory,
    no_reserve_credit([available_depot, available_a, available_b]),
    'spec:66 "Destination incoming position is never available stock"; handoff:21 "never spends inbound pipeline"').
fixture_claim('CASH-SUM', hiring,
    conserved([free, earmark, escrow, paid, protected], 10),
    'spec:68 "free + earmark + escrow + paid + protected = 10"; handoff:22').
fixture_claim('CASH-COVERAGE', hiring,
    no_complete_effect([coverage_research, coverage_implementation, coverage_review]),
    'spec:68 "Coverage remains after payment"; handoff:22 "payment preserves coverage"').
fixture_claim('CASH-ESCROW', hiring,
    complete_moves(escrow, decrease, [paid], increase),
    'spec:68 "Independent acceptance moves escrow to lifetime spending"').
fixture_claim('CASH-PREREQ', hiring,
    requires(review, [research-2, implementation-3]),
    'spec:68 "Review needs completed research units >=2 and implementation units >=3"').
fixture_claim('CASH-SELF', hiring,
    actor_not_executor,
    'spec:68 "No worker can authorize its own acceptance through the wrapper"').
% The same sentence read as a property of the wrapper (any config), not of the
% fixture's account names: the code must refuse actor = executor. v1.1.0 does
% (lib.rs:130, extracted code_role_checks(validate, [actor-executor])); F-4 fixed.
fixture_claim('CASH-SELF-ENFORCED', hiring,
    validate_rejects(actor-executor),
    'spec:11 "one exact executor account distinct from the actor"; api:65; spec:68 "No worker can authorize its own acceptance through the wrapper"').
fixture_claim('RISK-SIGN', risk,
    reserve_sign([selloff_loss, funding_loss, volatility_loss], nonneg),
    'spec:70 "Positive loss contributions reserve immediately"').
fixture_claim('RISK-HEDGE', risk,
    complete_sign([selloff_loss, funding_loss, volatility_loss], nonpos),
    'spec:70 "hedge benefit occurs only on completion"').
fixture_claim('RISK-COLLATERAL', risk,
    complete_sign([collateral], nonpos),
    'spec:70 "Collateral encumbrance remains until an explicit later measured baseline"').
fixture_claim('RISK-HEADROOM', risk,
    pending_negative_no_headroom,
    'spec:70 "No pending hedge offsets are netted into new trade capacity"; handoff:23').
fixture_claim('GPU-SLOTS', gpu,
    no_complete_effect_all,
    'spec:64 "completion does not free historical slots"').
fixture_claim('FRIDAY-SLOTS', friday,
    no_complete_effect_all,
    'spec:64 "completion does not free historical slots"').
fixture_claim('FRIDAY-ATOMIC', friday,
    paired_reserve([booking_1-slots_1, booking_2-slots_2]),
    'spec:64 "Friday also consumes dispatch slots atomically"').
% spec:48: admission refuses any commitment whose outcomes could make a metric
% worse than now, so a release (cancel before dispatch, or the executor's
% settle(false) after it) never fails Bounds in a reachable state. v1.0.0's
% spec:46 claimed this for the supplied fixtures only; F-2 broke it for risk.
% Checked per fixture by a bounded search (consistency.pl release_search: every
% admissible pair of requests from init, the other record Reserved or
% Completed, both release paths) and on every state of the fixture's own trace
% before its first observe. The general claim (every reachable state) is
% LIVE-ALL / INV-RES (W6, W4, W5).
fixture_claim('GPU-RELEASE', gpu, release_never_blocked,
    'spec:48 "the checked inverse cannot fail in a reachable state"; spec:64').
fixture_claim('FRIDAY-RELEASE', friday, release_never_blocked,
    'spec:48 "the checked inverse cannot fail in a reachable state"; spec:64').
fixture_claim('STOCK-RELEASE', inventory, release_never_blocked,
    'spec:48 "the checked inverse cannot fail in a reachable state"; spec:66').
fixture_claim('CASH-RELEASE', hiring, release_never_blocked,
    'spec:48 "the checked inverse cannot fail in a reachable state"; spec:68').
fixture_claim('RISK-RELEASE', risk, release_never_blocked,
    'spec:48 "the checked inverse cannot fail in a reachable state"; spec:70 "No unexecuted position backs a hedge: a hedge must stay in bounds if the position is later reported unexecuted"; handoff:23').
% recipe names used by requires/2 claims (fixtures.rs:210-235: CODE-ONLY order)
% are resolved by C-5 through fixture_recipe/3 in consistency.pl.

%% known_finding(ClaimId, FindingId): the code breaks this claim (a confirmed
%% open finding). C-5 passes for it only if it REPRODUCES the counterexample;
%% if the counterexample disappears C-5 fails, so the finding is re-examined
%% instead of silently turning into a pass. v1.1.0 has none: F-2
%% (RISK-RELEASE) and F-4 (CASH-SELF-ENFORCED) are fixed, so both claims must
%% now hold outright (C-5 fails on v1.0.0, where they still reproduce).

%%% ========================================================
%%% Findings (v1.0.0 verification; status at v1.1.0)
%%% ========================================================
%% finding(Id, Severity, Cite, Text, Status). Cites are to the v1.0.0 docs the
%% finding was raised against (spec@v1.0.0:N, handoff@v1.0.0:N, api@v1.0.0:N,
%% upstream tag v1.0.0 = 6f61616). Status is open or
%% fixed(Version, Properties): the properties that must hold at Version for the
%% fix to count (C-6 checks they are catalogued and that a fixed finding is
%% not named as a threat's residual risk).

finding('F-1', high, 'spec@v1.0.0:23, 46; handoff@v1.0.0:31',
    "Wedge: in a state disturbed by observe the recovery rule admits a hedge whose completion moves the disturbed metric back out; once the reservation brought it inside, settle(true) and settle(false) fail Bounds, cancel fails Pending, observe fails Pending forever.",
    fixed('v1.1.0', ['LIVE-ALL', 'INV-RES', 'TLA-LIVE-F1', 'LEAN-WIT', 'MC-1'])).
finding('F-2', high, 'spec@v1.0.0:46, 66; handoff:23; handoff@v1.0.0:31',
    "Healthy-state release wedge: in the risk fixture a hedge admitted on the strength of a credit position's reservation blocks the position's release; the executor's honest settle(false) fails Bounds; cancel fails Pending once in flight (Bounds while still Reserved); observe fails Pending. No observe needed.",
    fixed('v1.1.0', ['LIVE-ALL', 'INV-RES', 'TLA-LIVE-F2', 'C-5', 'MC-1'])).
finding('F-3', low, 'handoff@v1.0.0:34; api@v1.0.0:8',
    "Slot exhaustion: one actor exhausts the 256 lifetime records with free submit+cancel cycles; Full is permanent (observe does not prune).",
    fixed('v1.1.0', ['WIN-1', 'WIN-2', 'WIN-ESC', 'WIN-LIVE'])).
finding('F-4', low, 'spec@v1.0.0:9, 50, 64',
    "Config::validate accepts actor = executor, so a principal can attest its own completion; the docs rely on independence.",
    fixed('v1.1.0', ['CFG-2', 'C-5'])).
finding('F-5', low, 'handoff@v1.0.0:34',
    "At version = u32::MAX every transition fails Arithmetic, including settlement of in-flight liabilities and unpausing.",
    fixed('v1.1.0', ['VER-CAP', 'INV-VERCAP', 'C-4'])).
finding('F-6', low, 'spec@v1.0.0:19, 37, 54',
    "With max_age > 0 one long-open record blocks the only snapshot refresh (observe Pending), so admissions and dispatches stop with SnapshotExpired.",
    fixed('v1.1.0', ['OBS-2', 'REFRESH-1'])).

finding(Id, Severity, Cite, Text) :- finding(Id, Severity, Cite, Text, _).

%%% ========================================================
%%% Divergences between the docs and the code
%%% ========================================================
%% divergence(Id, Cite, DocSays, CodeDoes): an OPEN difference at the pin. The
%% model follows the code (MC-2 must agree with the real core) and tags the
%% clause with the divergence id. v1.1.0 has none open.
%%
%% closed_divergence(Id, OldCite, NewCite, Was, Resolution): a v1.0.0
%% divergence and how v1.1.0 resolved it (OldCite to the v1.0.0 docs, NewCite
%% to the docs at the pin).

closed_divergence('div-F1-inverse',
    'spec@v1.0.0:46; handoff@v1.0.0:31', 'spec:48; handoff:34',
    "v1.0.0 docs allowed a checked inverse to fail when another commitment depends on it; F-1 showed both settle outcomes failing Bounds with no other commitment open.",
    "v1.1.0 admission bounds every outcome of every open record against min(before, after the reservation) (spec:23, lib.rs:206-264); the docs now say the checked inverse cannot fail in a reachable state (spec:48; handoff:34). Claim checked by LIVE-ALL, INV-RES, TLA-LIVE-F1.").
closed_divergence('div-F2-fixture-release',
    'spec@v1.0.0:46; spec@v1.0.0:66', 'spec:48; spec:70; handoff:23',
    "v1.0.0 claimed the supplied fixtures prevent unsafe reliance on unexecuted work; F-2 found a risk-fixture hedge blocking a credit position's release.",
    "The release-aware envelope refuses the hedge (x = -reserve*u enters the hull); spec:70 adds 'No unexecuted position backs a hedge'. C-5 RISK-RELEASE must find no blocked release; TLA-LIVE-F2.").
closed_divergence('div-ack-noop-version',
    'spec@v1.0.0:39; api@v1.0.0:26', 'spec:41; api:30',
    "v1.0.0 docs listed Unknown -> Unknown as a transition and said every successful transition changes version; the code returned Ok without a bump for an acknowledgment that changes no status.",
    "Docs fixed, code unchanged: spec:41 'Unknown -> Unknown is an Ok no-op without a version change'; api:30 excludes an acknowledgment that does not change the status (lib.rs:438-451).").
closed_divergence('div-cancel-terminal-name',
    'spec@v1.0.0:43; api@v1.0.0:15', 'spec:43; api:17',
    "v1.0.0 cancel of a Completed record failed Pending, a name suggesting an unresolved commitment.",
    "Code renamed: cancel of a Completed record fails InvalidState (lib.rs:467-469); spec:43 and api:17 say so. Pending is kept for Dispatched/Active/Unknown (lib.rs:470-472).").

%%% ========================================================
%%% Ambiguities and decisions
%%% ========================================================

ambiguity('amb-interval-violation', 'spec:23',
    "\"Admission requires each metric's interval violation to be at most its violation before the request and after the reservation\": the violation amount of an interval is not defined.",
    "Read as max(0, min - low) + max(0, high - max) against min(point violation before, point violation after the reservation) (the code's reading, lib.rs:258-263; check_bounds v > old[i].min(new[i])).").
ambiguity('amb-freshness-boundary', 'spec:21, spec:39',
    "\"observation freshness if configured\" gives no boundary.",
    "Stale iff max_age > 0 and now > observed_at + max_age, saturating (CODE-ONLY, lib.rs:277-283, 422-428).").
ambiguity('amb-recovery-name', 'spec:25',
    "The second energy condition has no error name.",
    "Energy (CODE-ONLY, lib.rs:314-318).").
ambiguity('amb-ack-other', 'spec:40-41, 46; api:30',
    "Acknowledgment of a Reserved record, and a refusal for an Active record, are not described (v1.1.0 settles Unknown -> Unknown and repeated Active).",
    "Ignored: Ok with no change and no version bump (CODE-ONLY, lib.rs:438-440).").
ambiguity('amb-reconcile-kind', 'api:15; api:5',
    "reconcile is open to anyone and api:5 counts it among the mutating calls; it changes nothing itself.",
    "It sends a promise and changes nothing; the callback applies the result (shell lib.rs:93-112). C-1 records it as a change call with no core mutation.").
ambiguity('amb-observe-length', 'spec:58',
    "\"bounded measured values\" does not say what happens to a vector of the wrong length.",
    "Rejected with Bounds (CODE-ONLY, lib.rs:541-543).").
ambiguity('amb-owner-roles', 'spec:11; api:65',
    "spec:11 makes the executor distinct from the actor; it says nothing about the owner, who may also be a recipe's executor.",
    "validate checks actor /= executor only (lib.rs:130). An owner who is also an executor can both pause/observe and attest completion; residual under T-SELF and ASM-OWNER.").

%% resolved_ambiguity(Id, OldCite, NewCite, Resolution): v1.0.0 ambiguities the
%% v1.1.0 docs settle.
resolved_ambiguity('amb-version-transition', 'api@v1.0.0:26', 'api:30',
    "api:30 says which Ok calls change version: every one except an acknowledgment that does not change the status and a repeated terminal settle or cancel.").
resolved_ambiguity('amb-self-executor', 'spec@v1.0.0:9, 50, 64', 'spec:11; api:65',
    "spec:11 requires an executor distinct from the actor and api:65 says new rejects the config otherwise (InvalidConfig); the code enforces it (lib.rs:130).").

decision('model-follows-code', "Where the docs are silent, pool_machine.pl follows the code so MC-2 can demand exact agreement with the real core's traces; every such clause is tagged CODE-ONLY or with its divergence id, and open divergences are findings, not passes.").
decision('c2-unreachable', "C-2 compares guard orders modulo error sites that cannot fire from an INV state: InvalidConfig in add (add_length), NotFound in release after record or index (release_record), a second NotFound from index after record (index_again), energy's Bounds/Arithmetic (energy_terms, energy_sum), the add in submit's recovery check (recovery_add), bump inside bump_new (bump_below_cap), bump in acknowledge/release/settle (bump_reserve, INV-VERCAP), and submit's version and first_id checked adds (submit_version_add after the cap, submit_id_add, WIN-1). Each is listed with its justification in consistency.pl and may appear or not in the compared sequence; every other site must match exactly, in order.").
decision('c5-names', "The conformance generator renames fixture accounts (first actor a1, every later actor a2, executors x1/x2), so actor /= executor is true by construction on trace configs. C-5 checks the self-authorization claim on the fixture's own account names, extracted from fixtures.rs by extract.py, and checks validate rejects actor = executor in the code.").
decision('tla-leg', "C-2's TLA leg compares tla/SharedPool.tla GuardOrder (extracted/tla_facts.pl) with the code path under the c2-unreachable rule; Noop entries are noop. If the TLA file does not exist, the leg is a recorded gap, not a pass.").

%%% ========================================================
%%% Helpers
%%% ========================================================

%% camel_snake(?Camel, ?Snake): 'SnapshotExpired' <-> snapshot_expired.
camel_snake(Camel, Snake) :-
    atom(Camel), !,
    atom_chars(Camel, Cs),
    to_snake(Cs, first, Ss),
    atom_chars(Snake, Ss).
camel_snake(Camel, Snake) :-
    atom(Snake),
    atom_chars(Snake, Ss),
    to_camel(Ss, up, Cs),
    atom_chars(Camel, Cs).

to_snake([], _, []).
to_snake([C | Cs], Pos, Out) :-
    char_code(C, K),
    (   K >= 0'A, K =< 0'Z
    ->  L is K + 32, char_code(LC, L),
        ( Pos == first -> Out = [LC | Rest] ; Out = ['_', LC | Rest] )
    ;   Out = [C | Rest]
    ),
    to_snake(Cs, rest, Rest).

to_camel([], _, []).
to_camel(['_' | Cs], _, Out) :- !, to_camel(Cs, up, Out).
to_camel([C | Cs], Mode, [D | Out]) :-
    (   Mode == up -> char_code(C, K), U is K - 32, char_code(D, U)
    ;   D = C
    ),
    to_camel(Cs, keep, Out).
