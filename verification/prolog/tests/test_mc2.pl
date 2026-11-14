%% test_mc2.pl -- MC-2: every conformance trace of the real core through step/4
%%
%% PROP: MC-2 TX-0 TX-NOOP TX-VER AUTH-1 STALE-1 ADM-1 ADM-2 HARD-STEP DEP-1 LIFE-1
%% PROP: ASYNC-1 TERM-1 OBS-1 PAUSE-1 INV-WF INV-LEDGER INV-HARD INV-HEALTHY INV-DEP INV-VER LIVE-1
%% PROP: INV-RES INV-VERCAP WIN-1 WIN-2 WIN-ESC LIVE-ALL OBS-2 REFRESH-1
%% PROP: TX-SUBMIT TX-DISPATCH TX-ACK TX-CANCEL TX-SETTLE TX-PAUSE TX-OBSERVE
%%
%% 1. Replay: exact agreement on every result and post-state (divergences are
%%    reported with trace id and call index), and the step properties,
%%    invariants and probes hold on the real core's own traces.
%% 2. Vacuity guards: a corrupted trace is detected as a divergence; each step
%%    property checker fires on a real call with one planted fault and is silent
%%    on the unmodified call.
%%
%% Run: scryer-prolog test_mc2.pl -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module('support/harness').

:- discontiguous(test/2).
:- use_module('../requirements/pool_req').
:- use_module('../model/pool_machine').
:- use_module('../verification/traces').
:- use_module('../verification/props').
:- use_module('../verification/mc2').

run_tests :- run_suite("shared-pool Prolog: MC-2 trace replay", x).

%% load_trace(+N, -Trace): trace N of the checked-in file.
load_trace(N, T) :-
    trace_file(F),
    catch(fold_traces(F, find_trace(N), none, _), found(T), true),
    nonvar(T).
find_trace(N, T, A, A) :- T = trace(M, _, _, _, _), ( M =:= N -> throw(found(T)) ; true ).

%% call_at(+N, +I, -S0, -Op, -R, -S1): the I-th call of trace N (1-based).
call_at(N, I, S0, Op, R, S1) :-
    load_trace(N, trace(_, Cfg, _, Init, Calls)),
    nth1(I, Calls, call(Op, R, Post)),
    (   I =:= 1 -> Pre = Init ; I0 is I - 1, nth1(I0, Calls, call(_, _, Pre)) ),
    trace_to_pool(Cfg, Pre, S0), trace_to_pool(Cfg, Post, S1).

viols(S0, Op, R, S1, Vs) :- findall(V, edge_violation(S0, Op, R, S1, V), Vs).

%% planted(+Label, +RealTuple, +Planted, +Expected): the real call is clean; the
%% planted one is reported under Expected (a property name).
planted(Label, t(S0, Op, R, S1), t(P0, POp, PR, P1), Exp) :-
    viols(S0, Op, R, S1, V0), check(real_clean(Label, V0), V0 == []),
    viols(P0, POp, PR, P1, Vs),
    note("~w: ~q", [Label, Vs]),
    check(fires(Label, Exp), ( member(V, Vs), functor(V, Exp, _) )).

set_version(pool(C, V, _, O, P, R, F, D), N, pool(C, V, N, O, P, R, F, D)).
set_values(pool(C, _, N, O, P, R, F, D), V, pool(C, V, N, O, P, R, F, D)).
set_records(pool(C, V, N, O, P, _, F, D), R, pool(C, V, N, O, P, R, F, D)).
set_paused(pool(C, V, N, O, _, R, F, D), P, pool(C, V, N, O, P, R, F, D)).

%%% ========================================================
%%% Replay
%%% ========================================================

test('MC-2: all traces replay with exact agreement and no property violation', (
    trace_file(F),
    mc2_run(F, mc2(NT, NC, Divs, Viols, Hits)),
    length(Divs, ND), length(Viols, NV),
    note("traces ~d, calls ~d, divergences ~d, violations ~d", [NT, NC, ND, NV]),
    note("results seen: ~q", [Hits]),
    (   ND > 0 -> forall(( nth1(K, Divs, D), K =< 10 ), note("DIVERGENCE ~q", [D])) ; true ),
    (   NV > 0 -> forall(( nth1(K, Viols, V), K =< 10 ), note("VIOLATION ~q", [V])) ; true ),
    check(traces(NT), NT >= 216), check(calls(NC), NC >= 8000),
    check(divergences(ND), ND =:= 0), check(violations(NV), NV =:= 0) )).

% Non-vacuity of the replay: every op kind succeeds somewhere and the traces
% reach every error the code can return from an INV state. Full and
% Arithmetic come from the generator's appended traces 206 (arith: add and
% envelope at the 1e6 value bound), 207 (full: 256 records, a pruned front,
% Full on an open front, NotFound on a pruned id) and 208 (F-2b: the
% release term out of +-1e6 fails Arithmetic at admission).
test('MC-2 coverage: every op kind Ok and every reachable error', (
    trace_file(F),
    mc2_run(F, mc2(_, _, _, _, Hits)),
    findall(K, member(K-ok-_, Hits), Ks0), sort(Ks0, Ks),
    findall(E, ( member(_-E-_, Hits), E \== ok ), Es0), sort(Es0, Es),
    note("ok kinds ~q", [Ks]), note("errors ~q", [Es]),
    check(kinds, Ks == [ack, cancel, dispatch, observe, pause, settle, submit]),
    forall(member(E, [unauthorized, paused, stale, expired, snapshot_expired, invalid_recipe,
                      invalid_units, prerequisite, bounds, energy, invalid_state, not_found, pending,
                      full, arithmetic]),
           check(error_seen(E), memberchk(E, Es))) )).

% v1.0.0: trace 0 was the brief's wedge (the hedge admitted in [12,5], then
% settle both ways Bounds and observe Pending). v1.1.0 rejects the hedge at
% admission (spec:23 hull, min comparator), so no record exists; traces 214
% and 215 are the recovery-shaped hedge from a disturbed state, admitted and
% settled each way.
test('MC-2 F-1 fixed: trace 0 rejects the hedge; traces 214-215 settle the recovery hedge each way', (
    trace_file(F), fold_traces(F, collect_traces([0, 214, 215]), [], Ts),
    forall(member(N, [0, 214, 215]), (
        check(present(N), memberchk(N-T, Ts)),
        mc2_replay_trace(T, r(NC, Divs, Viols, _), Final),
        note("trace ~d: ~d calls, divergences ~q, violations ~q, final ~q", [N, NC, Divs, Viols, Final]),
        check(exact(N), ( Divs == [], Viols == [] )) )),
    memberchk(0-T0, Ts), results_of(T0, R0),
    check(f1_rejected, append(_, [observe(o, [12, 5], 100)-ok, submit(a1, 0, 1, 1, 200, 100)-err(bounds) | _], R0)),
    check(f1_no_record, memberchk(settle(x1, 0, true)-err(not_found), R0)),
    check(f1_observe_ok, memberchk(observe(o, [5, 5], 103)-ok, R0)),
    memberchk(214-T214, Ts), results_of(T214, R214), memberchk(215-T215, Ts), results_of(T215, R215),
    check(f1_recovery_true, ( memberchk(observe(o, [12, 5], _)-ok, R214), memberchk(settle(x1, 0, true)-ok, R214) )),
    check(f1_recovery_false, ( memberchk(observe(o, [12, 5], _)-ok, R215), memberchk(settle(x1, 0, false)-ok, R215) )) )).

% F-2, F-2b, F-4, F-6 and F-8 on the real core: traces 208 (f2b-arith), 209
% (f2-risk), 210 (f4-self), 211 (f6-stale), 212 (random over the new paths)
% and 213 (owner as executor) replay exactly, and each has its finding's
% fixed shape. RESULTS.md cites these traces next to the step/4 witnesses.
collect_traces(Ns, T, A, A1) :-
    T = trace(M, _, _, _, _), ( memberchk(M, Ns) -> A1 = [M-T | A] ; A1 = A ).
results_of(trace(_, _, _, _, Calls), ORs) :- findall(Op-R, member(call(Op, R, _), Calls), ORs).

test('MC-2 F-2, F-2b, F-4, F-6, F-8 fixed: traces 208-213 replay exactly, each with its fixed shape', (
    trace_file(F), fold_traces(F, collect_traces([5, 208, 209, 210, 211, 212, 213]), [], Ts),
    forall(member(N, [208, 209, 210, 211, 212, 213]), (
        check(present(N), memberchk(N-T, Ts)),
        mc2_replay_trace(T, r(NC, Divs, Viols, _), _),
        note("trace ~d: ~d calls, divergences ~q, violations ~q", [N, NC, Divs, Viols]),
        check(exact(N), ( Divs == [], Viols == [] )) )),
    % F-2b: recipe 1 at 100 units has a release term of 1.5e6: Arithmetic at
    % admission, so record 0 stays cancellable and settles false.
    memberchk(208-T208, Ts), results_of(T208, R208),
    check(f2b_arith, append(_, [submit(a1, 0, 1, _, _, _)-ok(0), submit(a1, 1, 100, _, _, _)-err(arithmetic) | _], R208)),
    check(f2b_release, ( memberchk(cancel(a1, 0, _)-ok, R208), memberchk(settle(x1, 0, false)-ok, R208) )),
    % F-2: the supplied risk fixture (trace 5's config). The hedge beside the
    % open position is rejected Bounds (its release is in the envelope); the
    % position's honest settle(false) is Ok in bounds, cancel and observe Ok.
    memberchk(5-trace(_, Cfg5, _, _, _), Ts), memberchk(209-T209, Ts),
    T209 = trace(_, Cfg209, _, _, C209), results_of(T209, R209),
    check(f2_risk_fixture, Cfg209 == Cfg5),
    check(f2_hedge_rejected, append(_, [submit(a1, 0, 1, _, _, _)-ok(0), submit(a2, 3, 8, _, _, _)-err(bounds) | _], R209)),
    check(f2_release_ok, ( nth1(K, C209, call(settle(x1, 0, false), ok, S2)),
                           S2 = s(V2, _, _, _, _, _, _), \+ violating(Cfg209, V2) )),
    check(f2_after, ( nth1(J, C209, call(observe(o, _, _), ok, _)), J > K )),
    % F-4: a recipe whose actor is its executor is refused at construction
    % (v1.0.0: accepted, each settled its own record).
    memberchk(210-T210, Ts), T210 = trace(_, Cfg210, _, I210, C210),
    check(f4_refused, ( I210 = init_error(invalid_config), C210 == [], \+ valid_config(Cfg210) )),
    check(f4_self, Cfg210 = config(O, _, _, _, [r(A, A, _, _, _, _), r(O, O, _, _, _, _)])),
    % F-8 (trace 213): the owner as a recipe's executor settles it.
    memberchk(213-T213, Ts), T213 = trace(_, config(O3, _, _, _, [r(A3, O3, _, _, _, _)]), _, _, _), results_of(T213, R213),
    check(owner_executor, ( memberchk(settle(O3, 0, true)-ok, R213), memberchk(settle(A3, 0, true)-err(unauthorized), R213) )),
    % F-6: max_age 50; with record 0 open, submit and dispatch fail
    % SnapshotExpired; the owner's restatement of the current values is Ok
    % with records open (v1.0.0: Pending) and new work proceeds.
    memberchk(211-T211, Ts), T211 = trace(_, config(_, 50, _, _, _), _, _, C211), results_of(T211, R211),
    check(f6_stale, ( memberchk(submit(_, _, _, _, _, _)-err(snapshot_expired), R211),
                      memberchk(dispatch(_, _, _)-err(snapshot_expired), R211) )),
    check(f6_refresh, ( nth1(KR, C211, call(observe(o, X, T7), ok, s(X, _, T7, _, Rs7, _, _))),
                        memberchk(c(_, _, _, active), Rs7),
                        KP is KR - 1, nth1(KP, C211, call(_, _, s(X, _, _, _, _, _, _))) )),
    check(f6_unblocked, memberchk(cancel(a1, 1, _)-ok, R211)) )).

test('MC-2 detects a corrupted trace (result and post-state), with trace id and index', (
    load_trace(0, trace(N, Cfg, Now, Init, Calls)),
    nth1(5, Calls, call(Op, _, Post), Rest),
    nth1(5, Calls1, call(Op, ok, Post), Rest),
    mc2_replay_trace(trace(N, Cfg, Now, Init, Calls1), r(_, D1, _, _), _),
    note("result corrupted: ~q", [D1]),
    check(result_div, D1 = [div(0, 5, settle(x1, 0, true), code(ok, _), model(err(not_found), _))]),
    Post = s(V, Ver, O, P, R, Fi, Dn), Ver1 is Ver + 1,
    nth1(3, Calls, call(Op3, R3, _), Rest3),
    nth1(3, Calls2, call(Op3, R3, s(V, Ver1, O, P, R, Fi, Dn)), Rest3),
    mc2_replay_trace(trace(N, Cfg, Now, Init, Calls2), r(_, D2, _, _), _),
    check(state_div, member(div(0, 3, _, _, _), D2)) )).

%%% ========================================================
%%% Planted faults: each property checker fires (vacuity guard)
%%% ========================================================
%% Trace 4 (hiring): 1 submit err(energy); 2 submit ok(0); 3 submit ok(1);
%% 4 submit err(prerequisite); 5 dispatch ok; 11 settle(x1,0,true) ok;
%% 12 pause(o,true) ok; 14 ack(1,true) ok (a Reserved record: ignored).

test('TX-0 fires when an error changes state', (
    call_at(4, 1, S0, Op, R, S1), pool_version(S1, N), N1 is N + 1, set_version(S1, N1, P1),
    planted('TX-0', t(S0, Op, R, S1), t(S0, Op, R, P1), 'TX-0') )).

test('TX-NOOP fires when an ignored acknowledgment bumps version', (
    call_at(4, 14, S0, Op, R, S1), check(is_noop, S1 == S0),
    pool_version(S1, N), N1 is N + 1, set_version(S1, N1, P1),
    planted('TX-NOOP', t(S0, Op, R, S1), t(S0, Op, R, P1), 'TX-NOOP') )).

test('TX-VER fires on a double version bump', (
    call_at(4, 2, S0, Op, R, S1), pool_version(S1, N), N1 is N + 1, set_version(S1, N1, P1),
    planted('TX-VER', t(S0, Op, R, S1), t(S0, Op, R, P1), 'TX-VER') )).

test('AUTH-1 fires on an effective submit by a non-actor', (
    call_at(4, 2, S0, Op, R, S1), Op = submit(_, RI, U, V, E, Now),
    planted('AUTH-1', t(S0, Op, R, S1), t(S0, submit(z, RI, U, V, E, Now), R, S1), 'AUTH-1') )).

test('STALE-1 fires on an accepted request at another version', (
    call_at(4, 2, S0, Op, R, S1), Op = submit(A, RI, U, _, E, Now),
    planted('STALE-1', t(S0, Op, R, S1), t(S0, submit(A, RI, U, 7, E, Now), R, S1), 'STALE-1') )).

test('ADM-1 fires when an admitted reservation raises energy', (
    call_at(4, 2, S0, Op, R, S1), pool_values(S1, [_ | Vt]), set_values(S1, [9 | Vt], P1),
    planted('ADM-1', t(S0, Op, R, S1), t(S0, Op, R, P1), 'ADM-1') )).

test('HARD-STEP fires when an Ok call moves a metric out of bounds', (
    call_at(4, 2, S0, Op, R, S1), pool_values(S1, [A, B, C, _ | Vt]), set_values(S1, [A, B, C, -1 | Vt], P1),
    planted('HARD-STEP', t(S0, Op, R, S1), t(S0, Op, R, P1), 'HARD-STEP') )).

test('DEP-1 fires when a submit without prerequisites is admitted', (
    call_at(4, 4, S0, Op, R, S1), Op = submit(_, RI, U, _, E, _),
    pool_records(S0, Rs), append(Rs, [c(RI, U, E, reserved)], Rs1), set_records(S0, Rs1, Q),
    pool_version(S0, N), N1 is N + 1, set_version(Q, N1, P1), length(Rs, Id),
    planted('DEP-1', t(S0, Op, R, S1), t(S0, Op, ok(Id), P1), 'DEP-1') )).

test('LIFE-1 fires on a Reserved -> Completed jump', (
    call_at(4, 5, S0, Op, R, S1), pool_records(S1, [c(A, B, C, _) | Rt]), set_records(S1, [c(A, B, C, completed) | Rt], P1),
    planted('LIFE-1', t(S0, Op, R, S1), t(S0, Op, R, P1), 'LIFE-1') )).

test('ASYNC-1 fires when an acknowledgment changes values', (
    call_at(4, 5, _, _, _, D), step(D, ack(0, true), ok, D1),
    pool_values(D1, [X | Vt]), X1 is X + 1, set_values(D1, [X1 | Vt], P1),
    planted('ASYNC-1', t(D, ack(0, true), ok, D1), t(D, ack(0, true), ok, P1), 'ASYNC-1') )).

test('TERM-1 fires when a repeated completion is rejected', (
    call_at(4, 11, _, _, _, C), step(C, settle(x1, 0, true), R, C1), check(noop, ( R == ok, C1 == C )),
    planted('TERM-1', t(C, settle(x1, 0, true), ok, C), t(C, settle(x1, 0, true), err(invalid_state), C), 'TERM-1') )).

test('OBS-1 fires on an observe accepted with open records', (
    call_at(4, 3, _, _, _, S), step(S, observe(o, [1, 1, 1, 1, 1, 1, 1, 3], 150), R, S1), check(pending, R == err(pending)),
    S = pool(Cf, _, N, _, P, Rs, Fi, Dn), N1 is N + 1, P1 = pool(Cf, [1, 1, 1, 1, 1, 1, 1, 3], N1, 150, P, Rs, Fi, Dn),
    planted('OBS-1', t(S, observe(o, [1, 1, 1, 1, 1, 1, 1, 3], 150), R, S1),
                     t(S, observe(o, [1, 1, 1, 1, 1, 1, 1, 3], 150), ok, P1), 'OBS-1') )).

test('PAUSE-1 fires on a dispatch effective while paused', (
    call_at(4, 12, _, _, _, S), check(paused, pool_paused(S, true)),
    step(S, dispatch(a2, 1, 120), R, S1), check(rejected, R == err(paused)),
    pool_records(S, [C0, c(A, B, E, reserved)]), pool_version(S, N), N1 is N + 1,
    set_records(S, [C0, c(A, B, E, dispatched)], Q), set_version(Q, N1, P1),
    planted('PAUSE-1', t(S, dispatch(a2, 1, 120), R, S1), t(S, dispatch(a2, 1, 120), ok, P1), 'PAUSE-1') )).

test('ADM-2 fires when a recovery reservation does not recover', (
    load_trace(5, trace(_, Cfg, _, _, _)),
    init_state(Cfg, 100, R0), step(R0, observe(o, [110, 40, 30, 40], 100), ok, R1),
    Op = submit(a2, 3, 1, 1, 200, 100), step(R1, Op, ok(0), R2),
    set_values(R2, [130, 40, 30, 38], P2),
    planted('ADM-2', t(R1, Op, ok(0), R2), t(R1, Op, ok(0), P2), 'ADM-2') )).

%% v1.1.0 properties on the appended traces: trace 4 call 26 cancels a
%% Completed record (InvalidState), trace 211 call 7 is the owner's refresh,
%% trace 207 call 514 prunes the front (id 256, first_id 1) and call 519
%% names the pruned id 0 (NotFound).

test('TERM-1 fires when cancelling a Completed record is accepted (v1.1.0 InvalidState)', (
    call_at(4, 26, S0, Op, R, S1), check(op, Op = cancel(a2, 1, _)), check(rejected, R == err(invalid_state)),
    planted('TERM-1', t(S0, Op, R, S1), t(S0, Op, ok, S0), 'TERM-1') )).

test('OBS-2 fires when a refresh changes more than observed_at and version', (
    call_at(211, 7, S0, Op, R, S1), check(refresh, ( Op = observe(o, V, _), pool_values(S0, V), R == ok )),
    set_paused(S1, true, P1),
    planted('OBS-2', t(S0, Op, R, S1), t(S0, Op, R, P1), 'OBS-2') )).

test('WIN-1 fires when a pruning submit returns another id', (
    call_at(207, 514, S0, Op, R, S1), check(pruned, ( R == ok(256), pool_first_id(S1, 1) )),
    planted('WIN-1', t(S0, Op, R, S1), t(S0, Op, ok(255), S1), 'WIN-1') )).

test('WIN-2 fires when a call on a pruned id is not NotFound', (
    call_at(207, 519, S0, Op, R, S1), check(not_found, ( Op = dispatch(_, 0, _), R == err(not_found) )),
    planted('WIN-2', t(S0, Op, R, S1), t(S0, Op, err(invalid_state), S1), 'WIN-2') )).
