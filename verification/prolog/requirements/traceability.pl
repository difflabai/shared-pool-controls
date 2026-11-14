%% traceability.pl -- Property catalogue (strategy.md section 5) -> layers, and
%% requirements -> properties
%%
%% PROP: C-6
%%
%%   property(Id, Layers, Text)   every property ID of the catalogue in
%%                                projects/shared-pool/strategy.md section 5
%%                                (the first cell of each table row there);
%%                                Layers are the layer directories that carry
%%                                it, as in that row's "Layer(s)" cell
%%   prolog_evidence(Id, How)     what THIS layer does for the property
%%   verified_by(Req, Property)   pool_req.pl requirement -> properties
%%
%% C-6 checks: these IDs are exactly strategy.md section 5's IDs (none missing,
%% extra or repeated) and each layer list equals that row's Layer(s) cell, in
%% order; strategy.md is committed, so its absence fails C-6. Also: every
%% property has a known layer; every requirement maps to at least one
%% property; every property named here, in threat_model.pl, or in a
%% prolog_evidence/2 row is in the catalogue.

:- module(traceability, [
    property/3,
    layer/1,
    prolog_evidence/2,
    verified_by/2
]).

:- use_module(library(lists)).

layer(kani).  layer(tla).  layer(lean).  layer(prolog).
layer('near-tests').  layer(sandbox).  layer(conformance).  layer(assumption).

%%% ========================================================
%%% Catalogue
%%% ========================================================

% Config / numeric
property('CFG-1', [kani, lean], "new Ok <=> validate Ok; init satisfies INV").
property('CFG-2', [kani, lean], "validate => references in range, vectors length n, requires strictly earlier and distinct, values bounded").
property('NUM-1', [kani, lean], "valid cfg, bounded values => energy/energy_terms Ok and exact").
property('NUM-2', [kani, lean], "add fails closed exactly when the exact result leaves +-1e6").
property('ROB-1', [kani], "no core panic from any INV state, any arguments").
% Transitions
property('TX-0', [kani, tla, lean, prolog], "every Err leaves the Pool unchanged").
property('TX-SUBMIT', [kani, tla, lean, prolog], "submit: exact result in guard order, exact post-state, frame").
property('TX-DISPATCH', [kani, tla, lean, prolog], "dispatch: exact result in guard order, exact post-state, frame").
property('TX-ACK', [kani, tla, lean, prolog], "acknowledge: exact result in guard order, exact post-state, frame").
property('TX-CANCEL', [kani, tla, lean, prolog], "cancel: exact result in guard order, exact post-state, frame").
property('TX-SETTLE', [kani, tla, lean, prolog], "settle: exact result in guard order, exact post-state, frame").
property('TX-PAUSE', [kani, tla, lean, prolog], "set_paused: exact result in guard order, exact post-state, frame").
property('TX-OBSERVE', [kani, tla, lean, prolog], "observe: exact result in guard order, exact post-state, frame").
property('TX-NOOP', [kani, tla, lean, prolog], "idempotent Ok paths change nothing, version included").
property('TX-VER', [kani, tla, lean, prolog], "every other Ok increments version by exactly 1").
property('VER-CAP', [kani, tla, lean, prolog], "version >= VERSION_CAP => submit, dispatch, set_paused, observe fail Arithmetic; ack, settle, cancel use the plain bump").  % v1.1.0
property('AUTH-1', [kani, tla, lean, prolog], "effective submit/dispatch => actor; cancel => actor or owner; settle => executor; pause/observe => owner").
property('STALE-1', [kani, tla, lean, prolog], "submit Ok => req.version = version; versions never repeat").
property('ADM-1', [kani, tla, lean, prolog], "submit Ok => energy(post) <= energy(pre)").
property('ADM-2', [kani, tla, lean, prolog], "submit Ok in a violating state => energy(post + completion) < energy(pre)").
property('HARD-STEP', [kani, tla, lean, prolog], "every Ok except observe: PV_i(post) <= PV_i(pre)").
property('DEP-1', [kani, tla, lean, prolog], "submit Ok => prerequisites met by Completed units").
property('LIFE-1', [kani, tla, lean, prolog], "status graph, terminal absorbing, append-only records, immutable fields").
property('ASYNC-1', [kani, tla, lean, prolog], "acknowledge never changes values; Active/Unknown keep the reservation").
property('TERM-1', [kani, tla, lean, prolog], "same terminal repeated = Ok no-op; opposite terminal = InvalidState").
property('OBS-1', [kani, tla, lean, prolog], "observe Ok => no open record, config unchanged").
property('PAUSE-1', [kani, tla, lean, prolog], "paused => no submit/dispatch effect; settle/ack/cancel/observe unaffected").
property('OBS-2', [kani, tla, lean, prolog], "observe Ok with an open record changes only observed_at and version; no re-base").  % v1.1.0
property('WIN-2', [kani, tla, lean, prolog], "a pruned id was terminal and is NotFound; ids never reused").  % v1.1.0
% Invariants
property('INV-WF', [tla, lean, kani, prolog], "values length n and bounded; record ids/recipes/units in range; #records <= 256; config never changes").
property('INV-LEDGER', [tla, lean, kani, prolog], "values = base + open/completed reservations + completed completions since the last observe").
property('INV-HARD', [tla, lean, kani, prolog], "envelope over open records computable and EV_i <= B_i").
property('INV-HEALTHY', [tla, lean, kani, prolog], "B = 0 => every subset of open completions lands in bounds").
property('INV-DEP', [tla, lean, kani, prolog], "every record's prerequisites are met by Completed records of lower index").
property('INV-VER', [tla, lean, kani, prolog], "version = number of effective transitions").
property('INV-RES', [tla, lean, kani, prolog], "release-aware envelope computable, endpoints within +-1e6, EV_i = PV_i(values)").  % v1.1.0
property('INV-VERCAP', [tla, lean, kani, prolog], "version + sum over open records of rem(status) <= u32::MAX").  % v1.1.0
property('WIN-1', [tla, lean, kani, prolog], "records[k].id = first_id + k; len <= 256; first_id + len <= version; completed_units = saturated lifetime count").  % v1.1.0
% Reads
property('RD-PREVIEW', [kani, 'near-tests', prolog], "preview.allowed <=> candidate Ok; preview.allowed and version < u32::MAX => submit Ok").
property('RD-BEST', [kani, prolog], "best_quantity sound and minimal as documented").
% Liveness
property('LIVE-1', [lean, tla, prolog], "INV, B = 0, version < u32::MAX => settle(executor, id, true) Ok for every Dispatched/Active/Unknown record").
property('LIVE-2', [tla, prolog], "every dispatched record eventually terminal under executor fairness; only without disturbing observes (F-1)").
property('LIVE-ALL', [lean, tla, kani, prolog], "every open record can be resolved: cancel (actor, owner) of Reserved, settle both ways of in-flight are Ok; no premise").  % v1.1.0
property('WIN-ESC', [lean, tla, kani, prolog], "Full iff 256 retained and the front open; resolving the front makes a submit pass the Full guard").  % v1.1.0
property('REFRESH-1', [lean, tla, kani, prolog], "below the cap, the owner's observe of the current values is Ok in every reachable state").  % v1.1.0
property('WIN-LIVE', [tla], "[](Full => <>~Full) under fairness of resolving the front").  % v1.1.0
% Shell
property('SH-1', ['near-tests'], "caller = predecessor").
property('SH-2', ['near-tests'], "new private + init once").
property('SH-3', ['near-tests'], "on_result private and its status mapping").
property('SH-4', ['near-tests'], "submit = core preview/submit; rejected submit writes nothing").
property('SH-5', ['near-tests'], "core Err => panic with the error string and no state change").
property('SH-6', ['near-tests'], "dispatch promise shape").
property('SH-7', ['near-tests'], "reconcile guard and promise shape").
property('SH-8', ['near-tests'], "Borsh state round trip").
property('SH-9', ['near-tests'], "views; get_commitments paging <= 50").
property('SH-10', ['near-tests'], "nonpayable methods reject deposits").
property('SH-11', ['near-tests'], "wasm exports are the documented methods plus contract_source_metadata; no upgrade, migrate or reset export").
property('SH-12', ['near-tests'], "wasm imports equal the build.sh allowlist; no deploy, key, transfer, stake, account, global-contract or promise_batch import").  % v1.1.0
property('SCN-1', ['near-tests'], "the published scenarios/<name>/trace.json equal the core's run of each fixture").  % v1.1.0
property('EX-1', ['near-tests'], "only lock executes").
property('EX-2', ['near-tests'], "payload binding and dedupe").
property('EX-3', ['near-tests'], "only operator finishes/confirms/sets mode").
property('EX-4', ['near-tests'], "terminal conflict rejected").
property('EX-5', ['near-tests'], "notify calls lock.settle").
property('EX-6', ['near-tests'], "executor window: newest, retention id + 256 > newest, at most 512 jobs, forgotten ids cancelled / stale id").  % v1.1.0
property('EX-7', ['near-tests', sandbox], "the executor never forgets the job of a record the lock retains").  % v1.1.0
property('EX-8', ['near-tests', sandbox], "on_settled private, exactly one promise result, advances only on a successful settle").  % v1.1.0
% Sandbox
property('RT-PRED', [sandbox], "relay attribution").
property('RT-PRIVATE', [sandbox], "new, on_result reject foreign callers").
property('RT-CALLBACK', [sandbox], "execute panic -> Unknown; cancelled -> Cancelled and released; active -> Active").
property('RT-REVERT', [sandbox], "core error leaves state unchanged").
property('RT-RACE', [sandbox], "finish before execute callback; late ack ignored").
property('RT-WEDGE', [sandbox], "F-1 end to end").
property('RES-1', [sandbox], "gas at max-size state < 300 Tgas").
property('RT-WINDOW', [sandbox], "F-3 end to end: window past 256 records through the executor").  % v1.1.0
property('RT-D1', [sandbox], "D-1 chain stays within 256 jobs").  % v1.1.0
% Prolog
property('C-1', [prolog], "authority matrix spec = code").
property('C-2', [prolog], "guard order spec = code = TLA").
property('C-3', [prolog], "lifecycle spec = code = TLA").
property('C-4', [prolog], "constants spec = code").
property('C-5', [prolog], "fixture conservation (STOCK, CASH, RISK, self-authorization)").
property('C-6', [prolog], "traceability").
property('MC-1', [prolog], "small model check").
property('MC-2', [prolog], "trace replay").
% TLA+
property('TLA-SAFE', [tla], "TLC safety").
property('TLA-IND', [tla], "inductive invariant (Apalache, optional)").
property('TLA-LIVE-1', [tla], "LIVE-2 on the completion path").
property('TLA-LIVE-F1', [tla], "F-1 counterexample (must be violated)").
property('TLA-LIVE-F2', [tla], "F-2 counterexample, healthy observations and a fair honest executor (must be violated)").
property('TLA-WIT', [tla], "witnesses").
property('REF-5', [tla], "TLA+ trace replay").
property('TLA-CAP', [tla], "near-cap TLC harness").  % v1.1.0
property('TLA-EXEC', [tla], "executor protocol model: no retained record's job forgotten; accepted confirmations never fill the cap").  % v1.1.0
% Lean
property('LEAN-IND', [lean], "inductive invariant").
property('LEAN-STEP', [lean], "step properties").
property('LEAN-LIVE1', [lean], "LIVE-1 at every size").
property('LEAN-LIVE-ALL', [lean], "LIVE-ALL at every size").  % v1.1.0
property('LEAN-WIT', [lean], "F-1 witness and others").
property('REF-5L', [lean], "Lean trace replay").
property('LEAN-GUARD', [lean], "guard lists = code order").
% Kani links
property('REF-1', [kani], "real core refines the TLA+ model per step within bounds").
property('REF-2', [kani], "real core refines the TLA+ model per step within bounds").
property('REF-3', [kani], "real core refines the TLA+ model per step within bounds").
% Assumptions
property('ASM-PRED', [assumption], "predecessor unforgeable").
property('ASM-PRIVATE', [assumption], "#[private] <=> predecessor = current account").
property('ASM-CALLBACK', [assumption], "callback sees the executor's return or PromiseError").
property('ASM-ATOMIC', [assumption], "a panicking receipt writes nothing").
property('ASM-INTERLEAVE', [assumption], "receipts interleave between dispatch and on_result").
property('ASM-TIME', [assumption], "block timestamp monotone").
property('ASM-EXEC', [assumption], "executor honest").
property('ASM-OWNER', [assumption], "observations truthful").
property('ASM-DEPLOY', [assumption], "no code replacement").
property('ASM-TOOLS', [assumption], "tools sound").
property('ASM-SPEC', [assumption], "specs mean what we intend").
property('ASM-MODEL', [assumption], "Kani small scope").
property('ASM-LEAN-TLA', [assumption], "transcription").
property('ASM-COMPILE', [assumption], "overflow-checks on").
property('ASM-GAS', [assumption], "callers attach the documented prepaid gas").  % v1.1.0

%%% ========================================================
%%% What this layer does for each property it carries
%%% ========================================================
%% Evidence vocabulary (near-approval strategy section 12): check, model-check, test.

prolog_evidence('C-1', check("pool_req method/4 vs extracted caller checks and shell attributes")).
prolog_evidence('C-2', check("pool_req guard/5 vs extracted code paths and TLA GuardOrder")).
prolog_evidence('C-3', check("pool_req life/5 vs extracted status guards, TLA Edges and the traces")).
prolog_evidence('C-4', check("pool_req constant/3 and model_constant/3 vs extracted constants")).
prolog_evidence('C-5', check("fixture claims on trace configs 1-5, their replays, fixtures.rs accounts (station derived from gpu), a two-record release search per fixture; validate rejects actor = executor; no known finding at v1.1.0 (F-2, F-4 fixed)")).
prolog_evidence('C-6', check("this file")).
prolog_evidence('MC-1', 'model-check'("bounded exploration of seven small universes: wedge (F-1), healthy, reliance (F-2), window and window1 (F-3, windows of 2 and 1), cap (F-5, from 4 below VERSION_CAP), f2b (F-2b)")).
prolog_evidence('MC-2', test("replay of every conformance trace through step/4")).
prolog_evidence(P, test("MC-2: checked on every edge/state of the real core's traces; MC-1: on the model's reachable graph")) :-
    member(P, ['TX-0', 'TX-NOOP', 'TX-VER', 'AUTH-1', 'STALE-1', 'ADM-1', 'ADM-2', 'HARD-STEP',
               'DEP-1', 'LIFE-1', 'ASYNC-1', 'TERM-1', 'OBS-1', 'PAUSE-1',
               'INV-WF', 'INV-LEDGER', 'INV-HARD', 'INV-HEALTHY', 'INV-DEP', 'INV-VER', 'LIVE-1',
               'INV-RES', 'INV-VERCAP', 'WIN-1', 'WIN-2', 'OBS-2']).
prolog_evidence(P, test("MC-2: exact result and post-state of every replayed call of this kind")) :-
    member(P, ['TX-SUBMIT', 'TX-DISPATCH', 'TX-ACK', 'TX-CANCEL', 'TX-SETTLE', 'TX-PAUSE', 'TX-OBSERVE']).
prolog_evidence('LIVE-2', 'model-check'("MC-1: no dead or release-blocked record in the wedge, healthy, reliance, cap and f2b universes (v1.0.0: wedge 228 states with a dead in-flight record, F-1; reliance 150 release-blocked states, F-2); the windowed universes check LIVE-ALL instead (ids move)")).
prolog_evidence('RD-PREVIEW', test("COV-2 differential (test_reads): the real core's preview (allowed, reason, candidate, energies) at 1,300 trace states (12 at VERSION_CAP or u32::MAX), 39,627 requests (conformance/traces/reads.txt) equals the model's; model unit tests (preview_allowed <=> submit Ok)")).
prolog_evidence('RD-BEST', test("COV-2 differential (test_reads): the real core's best_quantity on the same 39,627 requests equals the model's best_quantity/4; model unit test against brute force on small cases")).
prolog_evidence(P, test("MC-2: probed at every state of the real core's traces; MC-1: at every reachable state of the model (each open record resolved by its callers, the owner's restatement Ok below the cap, Full exactly with an open front)")) :-
    member(P, ['LIVE-ALL', 'REFRESH-1', 'WIN-ESC']).
prolog_evidence('VER-CAP', 'model-check'("MC-1 cap universe (window 2, from 4 below VERSION_CAP): submit, dispatch, pause and observe fail Arithmetic at the cap, ack, cancel and settle stay Ok past it; MC-2 checks every edge (no trace nears the cap); test_model F-5 at u32::MAX - 768")).

%%% ========================================================
%%% Requirements -> properties
%%% ========================================================

verified_by(Req, P) :- trace_row(Req, Ps), member(P, Ps).

trace_row('S-STATE',       ['INV-WF', 'CFG-1', 'MC-2']).
trace_row('S-WINDOW',      ['WIN-1', 'INV-WF', 'C-4']).
trace_row('S-CONFIG',      ['CFG-1', 'CFG-2', 'INV-WF', 'OBS-1']).
trace_row('S-INIT',        ['SH-2', 'RT-PRIVATE', 'C-1']).
trace_row('S-PRED',        ['SH-1', 'RT-PRED', 'AUTH-1', 'C-1']).
trace_row('S-PREVIEW-NOAUTH', ['RD-PREVIEW', 'C-1']).
trace_row('S-NUM',         ['NUM-1', 'NUM-2', 'C-4', 'INV-WF']).
trace_row('S-SERIAL',      ['C-4', 'SH-8']).
trace_row('S-EXPIRY',      ['TX-SUBMIT', 'C-4']).
trace_row('S-ADM-ORDER',   ['TX-SUBMIT', 'C-2', 'LEAN-GUARD', 'WIN-ESC', 'DEP-1']).
trace_row('S-ADM-CANDIDATE', ['TX-SUBMIT', 'NUM-2']).
trace_row('S-ADM-HARD',    ['HARD-STEP', 'INV-HARD', 'INV-HEALTHY', 'INV-RES']).
trace_row('S-ADM-ENERGY',  ['ADM-1', 'ADM-2']).
trace_row('S-ADM-RECOVERY-SCOPE', ['LIVE-2', 'MC-1', 'TLA-LIVE-F1']).
trace_row('S-ADM-EFFECT',  ['TX-SUBMIT', 'TX-0', 'TX-VER', 'LIFE-1']).
trace_row('S-PRUNE',       ['WIN-2', 'WIN-1', 'C-2']).
trace_row('S-ADM-NOREPLAY', ['STALE-1', 'INV-VER']).
trace_row('S-ADM-WRAPPER', ['SH-4']).
trace_row('S-PREVIEW',     ['RD-PREVIEW']).
trace_row('S-BEST',        ['RD-BEST']).
trace_row('S-POTENTIAL',   ['NUM-1', 'ADM-1']).
trace_row('S-HARD-AXIS',   ['INV-HARD', 'HARD-STEP']).
trace_row('S-LIFE-DISPATCH', ['TX-DISPATCH', 'C-2', 'C-3', 'SH-6', 'PAUSE-1']).
trace_row('S-LIFE-ACTIVE', ['TX-ACK', 'C-3', 'SH-3', 'RT-CALLBACK']).
trace_row('S-LIFE-UNKNOWN', ['TX-ACK', 'ASYNC-1', 'C-3', 'SH-3', 'RT-CALLBACK']).
trace_row('S-LIFE-COMPLETE', ['TX-SETTLE', 'C-3', 'LIVE-1', 'LIVE-ALL', 'WIN-1']).
trace_row('S-LIFE-CANCEL', ['TX-CANCEL', 'C-3', 'AUTH-1', 'TERM-1', 'LIVE-ALL']).
trace_row('S-LIFE-NONEXEC', ['TX-SETTLE', 'C-3', 'RT-CALLBACK', 'LIVE-ALL']).
trace_row('S-LIFE-IDEMPOTENT', ['TERM-1', 'TX-NOOP', 'C-3', 'EX-4']).
trace_row('S-LIFE-LATEACK', ['TX-ACK', 'C-3', 'RT-RACE']).
trace_row('S-INVERSE',     ['LIVE-ALL', 'INV-RES', 'TLA-LIVE-F1', 'RT-WEDGE', 'LEAN-WIT', 'MC-1']).
trace_row('S-FIXTURE-RELIANCE', ['C-5', 'MC-1', 'LIVE-ALL', 'TLA-LIVE-F2']).
trace_row('S-VERSION-RESERVE', ['VER-CAP', 'INV-VERCAP', 'C-2', 'C-4', 'TLA-CAP']).
trace_row('S-NOEXPIRY',    ['LIFE-1', 'SH-7', 'C-3']).
trace_row('S-ADAPTER',     ['EX-1', 'EX-2', 'EX-3', 'EX-4', 'EX-5', 'EX-6', 'EX-7']).
trace_row('S-PAUSE',       ['PAUSE-1', 'TX-PAUSE', 'VER-CAP', 'C-1', 'C-2']).
trace_row('S-OBSERVE',     ['OBS-1', 'OBS-2', 'REFRESH-1', 'TX-OBSERVE', 'C-1', 'C-2']).
% S-NOADMIN (spec:60): C-1 (no role-mutation method in the authority matrix),
% OBS-1 (observe leaves the policy unchanged), SH-11 (no upgrade, migrate or
% reset export), SH-12 (v1.1.0: the wasm imports equal build.sh's allowlist, no
% transfer, deploy, key, stake, account or promise_batch import; v1.0.0's NT-F3
% gap). Code replacement through a deployment-account key is ASM-DEPLOY.
trace_row('S-NOADMIN',     ['C-1', 'OBS-1', 'SH-11', 'SH-12', 'ASM-DEPLOY']).
trace_row('S-GPU',         ['C-5']).
trace_row('S-STOCK',       ['C-5']).
trace_row('S-CASH',        ['C-5', 'DEP-1']).
trace_row('S-RISK',        ['C-5']).
trace_row('A-NONPAYABLE',  ['SH-10', 'C-1']).
trace_row('A-METHODS',     ['C-1', 'SH-1', 'SH-2', 'SH-3']).
trace_row('A-PAGE',        ['SH-9', 'C-4']).
trace_row('A-SUBMIT-RESULT', ['SH-4']).
trace_row('A-ONRESULT',    ['SH-3', 'RT-CALLBACK', 'C-1']).
trace_row('A-WINDOW',      ['WIN-1', 'WIN-2', 'WIN-ESC', 'C-2']).
trace_row('A-VERSION',     ['TX-VER', 'TX-NOOP', 'INV-VER', 'C-3']).
trace_row('A-VERSION-CAP', ['VER-CAP', 'INV-VERCAP', 'LIVE-ALL', 'C-2', 'C-4']).
trace_row('A-EXPIRY-MAX',  ['C-4', 'TX-SUBMIT']).
trace_row('A-REASONS',     ['C-2', 'RD-PREVIEW']).
trace_row('A-AFTER',       ['RD-PREVIEW']).
trace_row('A-EXECUTOR',    ['SH-3', 'RT-CALLBACK', 'EX-1']).
trace_row('A-EXEC-WINDOW', ['EX-6', 'EX-7', 'EX-8', 'TLA-EXEC', 'RT-D1']).
trace_row('A-GAS',         ['RES-1', 'ASM-GAS']).
trace_row('A-CONFIG-ROLES', ['CFG-2', 'C-5']).
trace_row('H-CFG',         ['CFG-1', 'CFG-2']).
trace_row('H-NUM',         ['NUM-1', 'NUM-2', 'ROB-1']).
trace_row('H-AUTH',        ['AUTH-1', 'C-1']).
trace_row('H-ADM',         ['ADM-1', 'ADM-2', 'HARD-STEP']).
trace_row('H-ATOM',        ['TX-0', 'SH-5', 'RT-REVERT']).
trace_row('H-STALE',       ['STALE-1']).
trace_row('H-ASYNC',       ['ASYNC-1', 'INV-LEDGER']).
trace_row('H-TERM',        ['TERM-1']).
trace_row('H-DEP',         ['DEP-1', 'INV-DEP', 'WIN-1']).
trace_row('H-BIND',        ['LIFE-1', 'EX-2', 'SH-6']).
trace_row('H-STOCK',       ['C-5']).
trace_row('H-CASH',        ['C-5']).
trace_row('H-RISK',        ['C-5']).
trace_row('H-LIVE',        ['LIVE-ALL', 'LEAN-LIVE-ALL', 'INV-RES', 'INV-VERCAP', 'C-5']).
trace_row('H-WIN',         ['WIN-1', 'WIN-2', 'WIN-ESC', 'WIN-LIVE']).
trace_row('H-EXEC',        ['EX-6', 'EX-7', 'EX-8', 'TLA-EXEC']).
trace_row('H-INTERVAL',    ['INV-HARD', 'INV-HEALTHY', 'INV-RES']).
trace_row('H-LIVENESS',    ['LIVE-ALL', 'LIVE-1', 'LIVE-2', 'MC-1']).
trace_row('H-NOMONO',      ['TX-SETTLE', 'TX-OBSERVE']).
trace_row('H-VERSION',     ['TX-VER', 'VER-CAP', 'INV-VERCAP', 'INV-WF', 'C-4']).
trace_row('H-WINDOW',      ['WIN-1', 'WIN-2', 'WIN-ESC', 'DEP-1', 'RT-WINDOW']).
trace_row('H-JOBCAP',      ['EX-6', 'TLA-EXEC', 'RT-D1']).
