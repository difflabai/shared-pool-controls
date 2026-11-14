%% threat_model.pl -- Adversaries, threats and trust assumptions for shared-pool-controls
%%
%% PROP: C-6
%%
%% The threats are transcribed from the upstream docs (spec.md, api.md,
%% verification-handoff.md at pin 57b3f02 = v1.1.0; each cites its line), and
%% the assumptions are the ASM-* rows of strategy.md section 5.14. A residual
%% names only open findings (C-6: a finding fixed at the pin may not stay a
%% residual); the v1.0.0 findings F-1..F-6 are fixed in v1.1.0 (pool_req.pl).
%% mitigated_by/2 names the property IDs (strategy.md section 5) that address
%% a threat; C-6 checks that every one of them is a known property and that
%% every threat has a mitigation or a recorded residual.
%%
%%   adversary(Id, Capabilities)
%%   threat(Id, Cite, Threat, Residual)        Residual = none | Text
%%   mitigated_by(Threat, PropertyOrAssumption)
%%   accepted_risk(Threat, Text)
%%   assumption(Id, Text, Level)               Level = high | med | low | policy
%%   assumption_checked_by(Asm, Check)         a property or tool that tests it

:- module(threat_model, [
    adversary/2,
    threat/4,
    mitigated_by/2,
    accepted_risk/2,
    assumption/3,
    assumption_checked_by/2
]).

%%% ========================================================
%%% Adversaries
%%% ========================================================

adversary('ADV-ACTOR', "A recipe actor (or anyone holding its key): calls submit/dispatch/cancel with any arguments, any number of times, in any order; reads the whole state (get_pool).").
adversary('ADV-ANY', "Any NEAR account: calls every public method, including reconcile (api:15) and preview with any proposed actor (api:12).").
adversary('ADV-RELAY', "A contract or relayer between a key holder and the pool: its calls are attributed to it, not to the signer (spec:13).").
adversary('ADV-EXEC', "The recipe executor account: may attest completion or nonexecution; trusted for the truth of its attestation (ASM-EXEC), not for anything else. Distinct from the recipe's actor (spec:11; api:65).").
adversary('ADV-OWNER', "The owner: pause/observe/cancel; trusted for the truth of observations (ASM-OWNER), not assumed powerless (spec:60). May also be a recipe's executor (amb-owner-roles).").
adversary('ADV-DEPLOY', "Holder of a full-access key on the deployment account: can replace code (spec:60); out of scope (ASM-DEPLOY).").

%%% ========================================================
%%% Threats -> mitigating properties
%%% ========================================================

threat('T-ATTRIB', 'spec:13', "A call is attributed to an actor that did not make it (JSON actor field, signer instead of predecessor, preview identity).", none).
threat('T-INIT', 'spec:13; api:9', "An unrelated caller initializes the pool, or the pool is re-initialized.", none).
threat('T-FORGE', 'api:20; api:40', "Execution evidence is forged through a public callback or a non-executor settle.", none).
threat('T-REPLAY', 'spec:27; handoff:16', "An accepted request is replayed (no nonce).", none).
threat('T-OVERCOMMIT', 'spec:23; handoff:14', "An admission lets some subset of outstanding outcomes (completions or releases) drive a metric past its hard bound, or worsens a disturbed metric.", none).
threat('T-ENERGY', 'spec:25; handoff:14', "An admission increases the potential.", none).
threat('T-OVERFLOW', 'spec:15; spec:50; handoff:12; handoff:37', "Integer overflow in values, envelopes, energy, version or first_id; or version exhaustion blocks the resolution of open records.", none).
threat('T-PARTIAL', 'spec:27; handoff:15', "A rejected call leaves part of its effect in state.", none).
threat('T-DOUBLE', 'spec:45; handoff:18', "A commitment is settled twice, or both completed and cancelled.", none).
threat('T-UNKNOWN', 'spec:41, 52; handoff:17', "An unknown or failed acknowledgment releases the reservation while the job may have run.", "ASM-EXEC: settle(false) is only as true as the executor").
threat('T-PREREQ', 'spec:21; spec:68; handoff:19', "Dependent work is admitted before its prerequisites are completed, or pruning erases the completed count.", none).
threat('T-SELF', 'spec:11; spec:68; api:65', "A worker authorizes its own acceptance.", "The owner may also be a recipe's executor: validate checks actor /= executor only (lib.rs:130; amb-owner-roles); trusted under ASM-OWNER").
threat('T-PAUSE', 'spec:58', "Admission or dispatch takes effect while paused; or pausing blocks settlement.", none).
threat('T-OBS', 'spec:58', "observe runs with unresolved commitments, changes policy or deletes history.", "ASM-OWNER: the observed values themselves are trusted").
threat('T-BIND', 'spec:54; api:40; handoff:20', "The executor runs a different recipe/quantity than the reservation, or a duplicate dispatch runs twice.", none).
threat('T-RECONCILE', 'api:15; spec:41; api:30', "Anyone calls reconcile repeatedly to churn version (griefing pending requests into Stale).", none).
threat('T-DEPOSIT', 'api:5', "Attached deposits are accepted by a mutating method (reconcile included).", none).
threat('T-GROWTH', 'spec:9; api:10-11; handoff:38', "Unbounded storage or gas growth.", none).
threat('T-SLOTS', 'spec:9, 21, 27; api:28; handoff:38', "An actor fills the record window so nobody can be admitted again.", "Head-of-line: a dispatched oldest record keeps submit at Full until its executor reports (api:28; handoff:38; ASM-EXEC)").
threat('T-FRESHNESS', 'spec:21, 39, 58; api:19', "With max_age > 0, an open record blocks the only snapshot refresh, so admissions and dispatches stop with SnapshotExpired.", none).
threat('T-WEDGE', 'spec:48; handoff:24; handoff:34', "A commitment can never become terminal and the pool can never be observed again.", "Liveness is conditional on authoritative evidence and gas (ASM-EXEC, ASM-GAS; handoff:34); there is no forced administrative release (spec:48)").
threat('T-PRUNE', 'spec:9, 27; api:28; handoff:25', "An open record is pruned, an id is reused, or a call on a pruned id (a late callback, a repeated executor settle) acts on another record.", none).
threat('T-FORGET', 'spec:54; api:42; handoff:26', "The executor forgets the job of a record the lock still retains, so a later execute or status reports cancelled although the job ran.", none).
threat('T-JOBCAP', 'api:42; handoff:39', "The executor's 512-job cap fills, so confirmations and new executes fail and submit stays Full.", "Only refused confirmations (operator error: unassigned, pruned or too-early ids) fill it; the exits are a settle signed by the executor's own key or an accepted finish(false) (api:42; handoff:39; ASM-EXEC)").
threat('T-REDEPLOY', 'spec:60', "Code is replaced through a deployment-account full-access key.", "Out of scope (ASM-DEPLOY)").

mitigated_by('T-ATTRIB', 'AUTH-1').  mitigated_by('T-ATTRIB', 'SH-1').  mitigated_by('T-ATTRIB', 'RT-PRED').
mitigated_by('T-ATTRIB', 'C-1').     mitigated_by('T-ATTRIB', 'ASM-PRED').
mitigated_by('T-INIT', 'SH-2').      mitigated_by('T-INIT', 'RT-PRIVATE'). mitigated_by('T-INIT', 'C-1').
mitigated_by('T-INIT', 'ASM-PRIVATE').
mitigated_by('T-FORGE', 'SH-3').     mitigated_by('T-FORGE', 'RT-PRIVATE'). mitigated_by('T-FORGE', 'AUTH-1').
mitigated_by('T-FORGE', 'C-1').      mitigated_by('T-FORGE', 'ASM-CALLBACK').
mitigated_by('T-REPLAY', 'STALE-1'). mitigated_by('T-REPLAY', 'TX-VER'). mitigated_by('T-REPLAY', 'INV-VER').
mitigated_by('T-OVERCOMMIT', 'HARD-STEP'). mitigated_by('T-OVERCOMMIT', 'INV-HARD').
mitigated_by('T-OVERCOMMIT', 'INV-HEALTHY'). mitigated_by('T-OVERCOMMIT', 'INV-RES'). mitigated_by('T-OVERCOMMIT', 'TX-SUBMIT').
mitigated_by('T-ENERGY', 'ADM-1').   mitigated_by('T-ENERGY', 'ADM-2').
mitigated_by('T-OVERFLOW', 'NUM-1'). mitigated_by('T-OVERFLOW', 'NUM-2'). mitigated_by('T-OVERFLOW', 'ROB-1').
mitigated_by('T-OVERFLOW', 'VER-CAP'). mitigated_by('T-OVERFLOW', 'INV-VERCAP'). mitigated_by('T-OVERFLOW', 'TLA-CAP').
mitigated_by('T-OVERFLOW', 'C-2').   mitigated_by('T-OVERFLOW', 'C-4').   mitigated_by('T-OVERFLOW', 'ASM-COMPILE').
mitigated_by('T-PARTIAL', 'TX-0').   mitigated_by('T-PARTIAL', 'SH-5'). mitigated_by('T-PARTIAL', 'RT-REVERT').
mitigated_by('T-PARTIAL', 'ASM-ATOMIC').
mitigated_by('T-DOUBLE', 'TERM-1').  mitigated_by('T-DOUBLE', 'LIFE-1'). mitigated_by('T-DOUBLE', 'EX-4').
mitigated_by('T-DOUBLE', 'C-3').
mitigated_by('T-UNKNOWN', 'ASYNC-1'). mitigated_by('T-UNKNOWN', 'RT-CALLBACK'). mitigated_by('T-UNKNOWN', 'SH-3').
mitigated_by('T-UNKNOWN', 'INV-LEDGER').
mitigated_by('T-PREREQ', 'DEP-1').   mitigated_by('T-PREREQ', 'INV-DEP'). mitigated_by('T-PREREQ', 'WIN-1').
mitigated_by('T-PREREQ', 'C-5').
mitigated_by('T-SELF', 'CFG-2').     mitigated_by('T-SELF', 'C-5').       mitigated_by('T-SELF', 'AUTH-1').
mitigated_by('T-SELF', 'ASM-OWNER').
mitigated_by('T-PAUSE', 'PAUSE-1').  mitigated_by('T-PAUSE', 'VER-CAP'). mitigated_by('T-PAUSE', 'C-2').
mitigated_by('T-OBS', 'OBS-1').      mitigated_by('T-OBS', 'OBS-2').     mitigated_by('T-OBS', 'INV-WF').
mitigated_by('T-BIND', 'EX-2').      mitigated_by('T-BIND', 'SH-6'). mitigated_by('T-BIND', 'LIFE-1').
mitigated_by('T-BIND', 'EX-1').
mitigated_by('T-RECONCILE', 'SH-7'). mitigated_by('T-RECONCILE', 'TX-NOOP'). mitigated_by('T-RECONCILE', 'C-3').
mitigated_by('T-DEPOSIT', 'SH-10').  mitigated_by('T-DEPOSIT', 'C-1').
mitigated_by('T-GROWTH', 'INV-WF').  mitigated_by('T-GROWTH', 'WIN-1'). mitigated_by('T-GROWTH', 'RES-1').
mitigated_by('T-GROWTH', 'SH-9').    mitigated_by('T-GROWTH', 'C-4').
mitigated_by('T-SLOTS', 'WIN-1').    mitigated_by('T-SLOTS', 'WIN-2').    mitigated_by('T-SLOTS', 'WIN-ESC').
mitigated_by('T-SLOTS', 'WIN-LIVE'). mitigated_by('T-SLOTS', 'LIVE-ALL'). mitigated_by('T-SLOTS', 'RT-WINDOW').
mitigated_by('T-SLOTS', 'INV-WF').   mitigated_by('T-SLOTS', 'TX-SUBMIT'). mitigated_by('T-SLOTS', 'C-4').
mitigated_by('T-FRESHNESS', 'OBS-2'). mitigated_by('T-FRESHNESS', 'REFRESH-1'). mitigated_by('T-FRESHNESS', 'TX-SUBMIT').
mitigated_by('T-FRESHNESS', 'TX-DISPATCH'). mitigated_by('T-FRESHNESS', 'OBS-1').
mitigated_by('T-WEDGE', 'LIVE-ALL'). mitigated_by('T-WEDGE', 'LEAN-LIVE-ALL'). mitigated_by('T-WEDGE', 'INV-RES').
mitigated_by('T-WEDGE', 'INV-VERCAP'). mitigated_by('T-WEDGE', 'LIVE-1'). mitigated_by('T-WEDGE', 'LIVE-2').
mitigated_by('T-WEDGE', 'TLA-LIVE-F1'). mitigated_by('T-WEDGE', 'TLA-LIVE-F2'). mitigated_by('T-WEDGE', 'LEAN-WIT').
mitigated_by('T-WEDGE', 'RT-WEDGE'). mitigated_by('T-WEDGE', 'MC-1').    mitigated_by('T-WEDGE', 'C-5').
mitigated_by('T-WEDGE', 'ASM-EXEC'). mitigated_by('T-WEDGE', 'ASM-GAS').
mitigated_by('T-PRUNE', 'WIN-1').    mitigated_by('T-PRUNE', 'WIN-2').    mitigated_by('T-PRUNE', 'SH-3').
mitigated_by('T-PRUNE', 'SH-7').     mitigated_by('T-PRUNE', 'C-2').
mitigated_by('T-FORGET', 'EX-6').    mitigated_by('T-FORGET', 'EX-7').    mitigated_by('T-FORGET', 'EX-8').
mitigated_by('T-FORGET', 'RT-PRIVATE'). mitigated_by('T-FORGET', 'TLA-EXEC').
mitigated_by('T-JOBCAP', 'EX-6').    mitigated_by('T-JOBCAP', 'RES-1').   mitigated_by('T-JOBCAP', 'TLA-EXEC').
mitigated_by('T-JOBCAP', 'RT-D1').   mitigated_by('T-JOBCAP', 'ASM-EXEC').
mitigated_by('T-REDEPLOY', 'SH-11'). mitigated_by('T-REDEPLOY', 'SH-12'). mitigated_by('T-REDEPLOY', 'ASM-DEPLOY').

accepted_risk('T-REDEPLOY', "spec:60: deployment upgrade authority is verified separately.").
accepted_risk('T-UNKNOWN', "spec:52: an executed but unacknowledged job remains a liability until authoritative evidence arrives.").
accepted_risk('T-OBS', "spec:58: observe is a trusted external fact assertion and may create a hard-limit violation.").
accepted_risk('T-WEDGE', "handoff:34: liveness is conditional only on authoritative evidence and gas; spec:48: no forced administrative release. The v1.0.0 wedges (F-1, F-2) are fixed in v1.1.0 and re-checked here (LIVE-ALL, INV-RES).").
accepted_risk('T-SLOTS', "api:28, handoff:38: until the executor reports, submit stays Full (head-of-line). The v1.0.0 permanent Full (F-3) is fixed by pruning terminal records.").
accepted_risk('T-SELF', "spec:11 and api:65 require only actor /= executor; an owner-executor is trusted like any owner (ASM-OWNER).").
accepted_risk('T-JOBCAP', "api:42, handoff:39: a production adapter needs its own retention rule; the synthetic executor's cap is reached only through refused confirmations.").

%%% ========================================================
%%% Assumptions (strategy.md section 5.14, ASM-*)
%%% ========================================================

assumption('ASM-PRED', "The predecessor account id is unforgeable.", high).
assumption('ASM-PRIVATE', "#[private] admits exactly predecessor = current account.", high).
assumption('ASM-CALLBACK', "A callback sees the executor's return value or PromiseError.", high).
assumption('ASM-ATOMIC', "A panicking receipt writes nothing.", high).
assumption('ASM-INTERLEAVE', "Other receipts can run between dispatch and on_result; the models treat ack/settle as independent steps.", med).
assumption('ASM-TIME', "The block timestamp is monotone.", high).
assumption('ASM-EXEC', "The executor is honest: settle(false) only on definitive nonexecution.", policy).
assumption('ASM-OWNER', "Owner observations are truthful.", policy).
assumption('ASM-DEPLOY', "No code replacement; the deployment lock is out of scope.", policy).
assumption('ASM-TOOLS', "Scryer Prolog, TLC, Lean, Kani and the extractors are sound for what is used.", med).
assumption('ASM-SPEC', "The specs (and this layer's transcription of the docs) mean what we intend; needs an independent reviewer. The author of this layer had read the code.", low).
assumption('ASM-MODEL', "Kani's small scope (and MC-1's small universe) is representative.", med).
assumption('ASM-LEAN-TLA', "The Lean and TLA+ models transcribe the code's semantics.", med).
assumption('ASM-COMPILE', "The release profile has overflow-checks = true.", high).
assumption('ASM-GAS', "Callers attach at least the prepaid gas api.md documents; a receipt that runs out of gas changes nothing (ASM-ATOMIC) and can be retried.", med).

assumption_checked_by('ASM-PRED', 'RT-PRED').      assumption_checked_by('ASM-PRED', 'SH-1').
assumption_checked_by('ASM-PRIVATE', 'RT-PRIVATE'). assumption_checked_by('ASM-PRIVATE', 'SH-2').
assumption_checked_by('ASM-CALLBACK', 'RT-CALLBACK').
assumption_checked_by('ASM-ATOMIC', 'RT-REVERT'). assumption_checked_by('ASM-ATOMIC', 'RES-1').
assumption_checked_by('ASM-INTERLEAVE', 'RT-RACE').
assumption_checked_by('ASM-TOOLS', 'canaries and mutants in every runner').
assumption_checked_by('ASM-SPEC', 'status.toml [review] spec_reviewer (empty)').
assumption_checked_by('ASM-LEAN-TLA', 'REF-1').    assumption_checked_by('ASM-LEAN-TLA', 'REF-2').
assumption_checked_by('ASM-LEAN-TLA', 'REF-3').    assumption_checked_by('ASM-LEAN-TLA', 'MC-2').
assumption_checked_by('ASM-LEAN-TLA', 'REF-5').    assumption_checked_by('ASM-LEAN-TLA', 'REF-5L').
assumption_checked_by('ASM-LEAN-TLA', 'C-2').
assumption_checked_by('ASM-COMPILE', 'tools/scripts/check-pin.sh').
assumption_checked_by('ASM-GAS', 'RES-1').
