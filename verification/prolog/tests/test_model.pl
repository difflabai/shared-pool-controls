%% test_model.pl -- unit tests of step/4 (the reference semantics)
%%
%% PROP: TX-SUBMIT TX-DISPATCH TX-ACK TX-CANCEL TX-SETTLE TX-PAUSE TX-OBSERVE
%% PROP: TERM-1 PAUSE-1 DEP-1 ADM-2 RD-PREVIEW RD-BEST LIVE-1 MC-1
%%
%% Each test cites the doc line it exercises. The F-1..F-6 "fixed" tests show
%% each v1.0.0 finding fixed at v1.1.0 and keep the v1.0.0 outcome in a
%% comment; F-1 replays the brief's witness (conformance trace 0) call by call.
%% On the v1.1.0 corpus the model agrees with the real core on all 8,865
%% trace calls (MC-2), including the core's reproducers: trace 0 (F-1), 207
%% (F-3), 208 (F-2b), 209 (F-2), 210 (F-4), 211 (F-6) and 212-215; test_mc2
%% checks the shapes of 0 and 208-215.
%%
%% Run: scryer-prolog test_model.pl -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module(library(between)).
:- use_module('support/harness').

:- discontiguous(test/2).
:- use_module('../requirements/pool_req').
:- use_module('../model/pool_machine').
:- use_module('../verification/props').

run_tests :- run_suite("shared-pool Prolog: step/4 unit tests", x).

%%% Configs (trace configs 0, 3, 4, 5 of conformance/traces/core.txt; healthy is MC-1's;
%%% zero has no effects, so only the window and the version decide)
cfg(wedge, config(o, 0, [m(0, 10, 5), m(-100, 100, 5)], [t(1, 0, 1, target)],
                  [r(a1, x1, [-2, 0], [2, -5], 1, [])])).
cfg(inventory, config(o, 0, [m(4,20,6),m(0,20,6),m(0,20,0),m(4,20,6),m(0,20,6),m(0,20,0),m(0,20,0)],
                      [t(0,4,1,target),t(1,4,1,target),t(2,4,1,target)],
                      [r(a1,x1,[-1,1,0,-1,0,0,1],[0,0,0,0,1,0,-1],20,[]),
                       r(a2,x1,[-1,0,1,-1,0,0,1],[0,0,0,0,0,1,-1],20,[]),
                       r(a1,x1,[0,-1,1,0,-1,0,1],[0,0,0,0,0,1,-1],20,[])])).
cfg(hiring, config(o, 0, [m(0,10,0),m(0,10,0),m(0,10,0),m(0,10,5),m(0,2,2),m(0,7,0),m(0,7,0),m(3,3,3)],
                   [t(0,2,1,target),t(1,3,1,target),t(2,2,1,target)],
                   [r(a1,x1,[1,0,0,-1,0,1,0,0],[0,0,0,0,0,-1,1,0],20,[]),
                    r(a2,x1,[0,1,0,-1,0,1,0,0],[0,0,0,0,0,-1,1,0],20,[]),
                    r(a2,x1,[0,0,1,0,-1,1,0,0],[0,0,0,0,0,-1,1,0],20,[p(0,2),p(1,3)])])).
cfg(healthy, config(o, 100, [m(0, 10, 4), m(0, 6, 2)], [t(0, 5, 1, target), t(1, 3, 2, deficit)],
                    [r(a1, x1, [1, 1], [1, -1], 2, []), r(a2, x2, [2, 0], [-1, 1], 1, [p(0, 1)])])).
cfg(zero, config(o, 0, [m(0, 10, 5)], [t(0, 5, 1, target)], [r(a1, x1, [0], [0], 1, [])])).
cfg(risk, config(o, 300, [m(0,100,60),m(0,80,40),m(0,60,30),m(0,40,40)],
                 [t(0,80,1,excess),t(1,60,1,excess),t(2,40,1,excess)],
                 [r(a1,x1,[20,5,0,-5],[0,0,0,0],20,[]),r(a2,x1,[10,0,5,-5],[0,0,0,0],20,[]),
                  r(a2,x1,[0,10,5,-5],[0,0,0,0],20,[]),r(a2,x1,[0,0,0,-2],[-10,0,0,0],20,[])])).

init(Name, S) :- cfg(Name, C), init_state(C, 100, S).

%% run(+S0, +[Op-Expected..], -S): every call gives Expected; an error leaves S unchanged.
run(S, [], S).
run(S0, [Op-Exp | Ps], S) :-
    step(S0, Op, R, S1),
    (   R = Exp -> true
    ;   note("~q: expected ~q, got ~q", [Op, Exp, R]), fail
    ),
    (   R = err(_) -> check(unchanged(Op), S1 == S0) ; true ),
    run(S1, Ps, S).

status_of(S, Id, St) :- record_of(S, Id, c(_, _, _, St)).
version(S, N) :- pool_version(S, N).

%%% ========================================================
%%% F-1..F-6 fixed (v1.1.0). Each test keeps the v1.0.0 outcome in a comment;
%%% tests/test_mc1.pl and the conformance traces 0, 207-211, 214, 215 carry
%%% the same regressions on the real core.
%%% ========================================================

% v1.0.0 (trace 0): observe [12,5] Ok; submit Ok(0); dispatch; ack(true);
% settle(true) and settle(false) Bounds; cancel and observe Pending: wedged.
test('F-1 fixed: in the disturbed state the hedge is rejected Bounds (spec:23 hull, min comparator)', (
    init(wedge, S0),
    run(S0, [observe(o, [12, 5], 100) - ok,
             submit(a1, 0, 1, 1, 200, 100) - err(bounds),    % v1.0.0: ok(0), then wedged
             observe(o, [5, 5], 101) - ok], S),
    pool_records(S, Rs), check(no_record, Rs == []) )).

test('F-1 fixed: from a healthy observe the same hedge is admitted and settles either way', (
    init(wedge, S0),
    % [5,5]: the reservation pulls metric 0 to 3, the hull [3,5] fits;
    % the energy (term on metric 1) does not change.
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok], S1),
    run(S1, [settle(x1, 0, true) - ok], S2), status_of(S2, 0, completed),
    run(S1, [settle(x1, 0, false) - ok], S3), status_of(S3, 0, cancelled),
    run(S1, [ack(0, true) - ok, settle(x1, 0, false) - ok], _) )).

% v1.0.0: the hedge x7 was ok(1) (80 - 70 >= 0, the position's release not
% counted); after the hedge completed, settle(x1, 0, false) failed Bounds and
% cancel/observe Pending (F-2).
test('F-2 fixed: risk fixture, the hedge beside an open position is held to its release (spec:23, 46)', (
    init(risk, R0),
    run(R0, [submit(a1, 0, 1, 0, 200, 100) - ok(0),          % credit position: selloff loss 60 -> 80
             submit(a2, 3, 7, 1, 200, 100) - err(bounds),    % 80 - 20 (release) - 70 < 0
             submit(a2, 3, 6, 1, 200, 100) - ok(1),          % 80 - 20 - 60 = 0
             dispatch(a1, 0, 101) - ok, dispatch(a2, 1, 101) - ok,
             settle(x1, 1, true) - ok,                        % hedge fills: selloff loss 20
             settle(x1, 0, false) - ok], S),                  % the honest release: 20 - 20 = 0
    pool_values(S, V), note("released at ~q", [V]), V == [0, 40, 30, 28],
    cfg(risk, RC), check(healthy, \+ violating(RC, V)) )).

% v1.0.0 (trace 208): B was ok(1); after B completed, cancel and settle(false)
% of A failed Arithmetic (values - reserve*units left +-1e6).
test('F-2b fixed: trace 208, the release term is bounded, so B fails Arithmetic at admission (spec:23)', (
    Cfg = config(o, 0, [m(-1000000, 1000000, 0), m(-10, 10, 0)], [t(1, 0, 1, target)],
                 [r(a1, x1, [-1000000, 0], [0, 0], 1, []), r(a1, x1, [0, 0], [15000, 0], 100, [])]),
    init_state(Cfg, 100, S0),
    % the release term of A (+1e6) plus B's completion (+1.5e6) leave +1e6:
    % Arithmetic (not Bounds; the code's envelope fails before check_bounds)
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0),
             submit(a1, 1, 100, 1, 200, 100) - err(arithmetic),   % v1.0.0: ok(1)
             submit(a1, 1, 66, 1, 200, 100) - ok(1),               % high = 0 + 990,000
             dispatch(a1, 1, 101) - ok, settle(x1, 1, true) - ok,
             cancel(a1, 0, 102) - ok], S),
    pool_values(S, V), note("after the release ~q", [V]), V == [990000, 0] )).

% v1.0.0: after 256 free submit+cancel cycles every submit failed Full, even
% after an observe (trace 207: full).
test('F-3 fixed: 256 submit+cancel cycles, then a submit prunes the terminal front and gets id 256 (spec:21, 27)', (
    init(wedge, S0),
    cycles(256, S0, S),
    pool_records(S, Rs), length(Rs, NRec), check(full_size, NRec =:= 256),
    pool_version(S, N),
    run(S, [submit(a1, 0, 1, N, 200, 100) - ok(256)], S1),      % v1.0.0: err(full)
    pool_first_id(S1, F1), pool_records(S1, Rs1), length(Rs1, L1),
    note("first_id ~d, ~d retained", [F1, L1]),
    check(pruned_one, F1 =:= 1), check(window, L1 =:= 256),
    check(id_256_reserved, status_of(S1, 256, reserved)),
    % WIN-2: the pruned id 0 is NotFound for every call, which changes nothing
    run(S1, [dispatch(a1, 0, 101) - err(not_found), ack(0, true) - err(not_found),
             cancel(a1, 0, 101) - err(not_found), cancel(o, 0, 101) - err(not_found),
             settle(x1, 0, true) - err(not_found), settle(x1, 0, false) - err(not_found),
             dispatch(a1, 257, 101) - err(not_found)], _) )).

cycles(0, S, S) :- !.
cycles(K, S0, S) :-
    pool_version(S0, N), pool_next_id(S0, Id),
    run(S0, [submit(a1, 0, 1, N, 200, 100) - ok(Id), cancel(a1, Id, 100) - ok], S1),
    K1 is K - 1, cycles(K1, S1, S).

test('F-3 fixed: Full only while the oldest of 256 retained records is open (spec:21, WIN-ESC)', (
    Cfg = config(o, 0, [m(0, 10, 5)], [t(0, 5, 1, target)], [r(a1, x1, [0], [0], 1, [])]),
    init_state(Cfg, 100, S0),
    open_submits(256, S0, S),
    run(S, [submit(a1, 0, 1, 256, 200, 100) - err(full),
            cancel(o, 1, 100) - ok,                           % a later record: still Full
            submit(a1, 0, 1, 257, 200, 100) - err(full),
            cancel(a1, 0, 100) - ok,                          % the front: one slot frees
            submit(a1, 0, 1, 258, 200, 100) - ok(256)], S1),
    pool_first_id(S1, F1), check(front_pruned, F1 =:= 1),
    % the new front (id 1) is Cancelled, so the next submit prunes it; then id 2 is open
    run(S1, [submit(a1, 0, 1, 259, 200, 100) - ok(257),
             submit(a1, 0, 1, 260, 200, 100) - err(full)], S2),
    pool_first_id(S2, F2), check(front_open, ( F2 =:= 2, status_of(S2, 2, reserved) )) )).

open_submits(0, S, S) :- !.
open_submits(K, S0, S) :-
    pool_version(S0, N), pool_next_id(S0, Id),
    run(S0, [submit(a1, 0, 1, N, 200, 100) - ok(Id)], S1),
    K1 is K - 1, open_submits(K1, S1, S).

% v1.0.0 scanned the retained records for Completed units: with the
% prerequisite pruned, recipe 1 failed Prerequisite (FR-4; trace 207
% submit:prereq_after_prune).
test('F-3 fixed: completed_units counts a pruned prerequisite (spec:9, 21; window of 2)', (
    with_window_size(2, (
        cfg(healthy, C), init_state(C, 100, S0),
        run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok,
                 settle(x1, 0, true) - ok,                    % completed_units[0] = 1
                 observe(o, [4, 2], 102) - ok,
                 submit(a1, 0, 1, 4, 200, 102) - ok(1), cancel(a1, 1, 102) - ok,
                 submit(a1, 0, 1, 6, 200, 102) - ok(2), cancel(a1, 2, 102) - ok], S1),
        pool_first_id(S1, F1), check(completed_record_pruned, F1 =:= 1),
        check(lifetime_count, completed_units(S1, 0, 1)),
        run(S1, [submit(a2, 1, 1, 8, 200, 102) - ok(3)], S2),   % v1.0.0: err(prerequisite)
        pool_records(S2, Rs2), note("retained ~q", [Rs2]),
        check(no_completed_prerequisite_retained, \+ member(c(0, _, _, completed), Rs2)) )) )).

% v1.0.0: valid_config accepted actor = executor and the principal settled its
% own work (F-4).
test('F-4 fixed: a config with actor = executor is invalid (spec:11; CFG-2)', (
    Cfg = config(o, 0, [m(0, 10, 5), m(-100, 100, 5)], [t(1, 0, 1, target)],
                 [r(a1, a1, [-2, 0], [2, -5], 1, [])]),
    check(rejected, \+ valid_config(Cfg)),
    check(no_pool, \+ init_state(Cfg, 100, _)),
    % the owner may still be a recipe's executor (amb-owner-roles)
    check(owner_executor, valid_config(config(o, 0, [m(0, 10, 5)], [t(0, 5, 1, target)],
                                              [r(a1, o, [0], [0], 1, [])]))) )).

% v1.0.0: at version u32::MAX settle, ack and set_paused all failed
% Arithmetic, so the open record could never become terminal (F-5).
test('F-5 fixed: at VERSION_CAP = u32::MAX - 768 new work stops and settlement continues (spec:50)', (
    version_cap(Cap), u32_max(M), note("VERSION_CAP ~d", [Cap]),
    check(real_cap, Cap =:= M - 768),
    check(documented_cap, constant(version_cap, Cap, _)),
    cfg(zero, C),
    S = pool(C, [5], Cap, 100, false, [c(0, 1, 200, dispatched), c(0, 1, 200, reserved)], 0, [0]),
    check(inv_vercap_holds, \+ state_violation(S, g([5], 0, Cap), 'INV-VERCAP'(_, _))),
    run(S, [submit(a1, 0, 1, Cap, 200, 100) - err(arithmetic),     % candidate's last check
            submit(a1, 0, 1, 7, 200, 100) - err(stale),             % an earlier check decides first
            dispatch(a1, 1, 101) - err(arithmetic),
            pause(o, true) - err(arithmetic),
            observe(o, [5], 101) - err(arithmetic),                  % the refresh too
            ack(0, true) - ok,                                       % plain increment: Cap + 1
            settle(x1, 0, true) - ok,
            cancel(a1, 1, 101) - ok], S1),
    pool_version(S1, N1), note("version after settling ~d", [N1]), check(three_bumps, N1 =:= Cap + 3),
    check(all_terminal, ( pool_records(S1, Rs), forall(member(c(_, _, _, St), Rs), terminal(St)) )),
    Cm1 is Cap - 1,
    run(pool(C, [5], Cm1, 100, false, [], 0, [0]), [pause(o, false) - ok, pause(o, false) - err(arithmetic)], _) )).

% v1.0.0: with a record in flight every observe failed Pending, so a stale
% snapshot (max_age) stopped dispatch and admission until it settled (F-6).
test('F-6 fixed: the owner restates the current values with a record open; only observed_at and version change (spec:58)', (
    init(risk, R0),                                            % max_age 300, observed_at 100
    run(R0, [submit(a2, 3, 1, 0, 1000, 100) - ok(0), submit(a2, 3, 1, 1, 1000, 100) - ok(1),
             dispatch(a2, 0, 101) - ok,
             dispatch(a2, 1, 401) - err(snapshot_expired),
             submit(a2, 3, 1, 3, 1000, 401) - err(snapshot_expired),
             observe(o, [60, 40, 30, 40], 401) - err(pending)], S1), % new values while open: Pending
    pool_values(S1, V1), note("current values ~q", [V1]),
    run(S1, [observe(o, V1, 401) - ok], S2),                  % v1.0.0: err(pending)
    S1 = pool(C, V1, N1, _, P, Rs, F, D), N2 is N1 + 1,
    check(refresh_only, S2 == pool(C, V1, N2, 401, P, Rs, F, D)),
    run(S2, [dispatch(a2, 1, 402) - ok, submit(a2, 3, 1, 5, 1000, 402) - ok(2)], _) )).

%%% ========================================================
%%% Admission (spec:19-25, api:26-28)
%%% ========================================================

test('submit: the first failing guard decides (paused before stale, expiry, caller)', (
    init(wedge, S0),
    run(S0, [pause(o, true) - ok, submit(z, 7, 0, 99, 1, 100) - err(paused)], _) )).

test('submit: each documented reason is reachable (api:28)', (
    init(hiring, H0),
    run(H0, [submit(a1, 0, 1, 1, 200, 100) - err(stale),
             submit(a1, 0, 1, 0, 100, 100) - err(expired),          % expires_at <= now
             submit(a1, 0, 1, 0, 86501, 100) - err(expired),        % > now + 86400
             submit(a1, 3, 1, 0, 200, 100) - err(invalid_recipe),
             submit(a2, 0, 1, 0, 200, 100) - err(unauthorized),
             submit(a1, 0, 0, 0, 200, 100) - err(invalid_units),
             submit(a1, 0, 21, 0, 200, 100) - err(invalid_units),
             submit(a2, 2, 1, 0, 200, 100) - err(prerequisite),
             submit(a1, 0, 6, 0, 200, 100) - err(bounds),           % free 5 - 6 < 0
             submit(a1, 0, 5, 0, 200, 100) - err(energy)], _),     % coverage 5 >> target 2
    init(risk, R0),
    run(R0, [submit(a1, 0, 1, 0, 500, 401) - err(snapshot_expired)], _) )).

test('submit: expiry and freshness boundaries (spec:19; CODE-ONLY boundaries)', (
    init(risk, R0),
    run(R0, [submit(a2, 3, 1, 0, 86500, 100) - ok(0)], _),          % now + 86400 accepted
    run(R0, [submit(a2, 3, 1, 0, 101, 100) - ok(0)], _),            % now + 1 accepted
    run(R0, [submit(a2, 3, 1, 0, 500, 400) - ok(0)], _),            % now = observed_at + max_age
    run(R0, [submit(a2, 3, 1, 0, 500, 401) - err(snapshot_expired)], _) )).

test('submit: saturating u64 expiry horizon (CODE-ONLY)', (
    init(wedge, S0), u64_max(U), Now is U - 10,
    step(S0, submit(a1, 0, 1, 0, U, Now), R, _), note("result ~q", [R]),
    R \== err(expired) )).

test('version at u32::MAX (unreachable: INV-VERCAP) is Arithmetic for the plain increment too (spec:50)', (
    init(wedge, S0), S0 = pool(C, _, _, O, P, _, F, D), u32_max(M),
    S1 = pool(C, [3, 5], M, O, P, [c(0, 1, 200, dispatched)], F, D),
    run(S1, [pause(o, true) - err(arithmetic), submit(a1, 0, 1, M, 200, 100) - err(arithmetic),
             pause(z, true) - err(unauthorized), ack(0, true) - err(arithmetic),
             settle(x1, 0, true) - err(arithmetic)], _),
    check(inv_vercap_flags_it, state_violation(S1, g([5, 5], 0, M), 'INV-VERCAP'(_, _))) )).

test('DEP-1/CASH-PREREQ: review needs Completed research >= 2 and implementation >= 3 (spec:64)', (
    init(hiring, H0),
    run(H0, [submit(a1, 0, 2, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok, ack(0, true) - ok,
             submit(a2, 1, 3, 3, 200, 101) - ok(1), dispatch(a2, 1, 102) - ok, settle(x1, 1, true) - ok,
             % research only Active (acknowledged, not completed): handoff:19
             submit(a2, 2, 1, 6, 200, 102) - err(prerequisite),
             settle(x1, 0, true) - ok,
             submit(a2, 2, 1, 7, 200, 103) - ok(2)], _) )).

test('ADM-2 recovery hedge admitted in a disturbed state; a loss trade is not (spec:23, spec:66)', (
    init(risk, R0),
    run(R0, [observe(o, [110, 40, 30, 40], 100) - ok,
             submit(a1, 0, 1, 1, 200, 100) - err(bounds),
             submit(a2, 3, 1, 1, 200, 100) - ok(0)], S),
    pool_values(S, V), note("after hedge reservation ~q", [V]),
    V == [110, 40, 30, 38] )).                                     % benefit only on completion

test('STOCK: transit is never available before receipt; the sum stays 12 (spec:62)', (
    init(inventory, I0),
    % "Depot to A" would raise the position energy (Energy); "Depot to B" lowers it
    run(I0, [submit(a1, 0, 2, 0, 200, 100) - err(energy), submit(a2, 1, 2, 0, 200, 100) - ok(0)], I1),
    pool_values(I1, V1), V1 = [_, _, P1, D1, A1, B1, T1], check(sum1, D1 + A1 + B1 + T1 =:= 12),
    check(position_moves_at_reservation, P1 =:= 2),
    check(no_credit_at_reservation, B1 =:= 0),
    run(I1, [dispatch(a2, 0, 101) - ok, settle(x1, 0, true) - ok], I2),
    pool_values(I2, V2), V2 = [_, _, _, D2, A2, B2, T2], check(sum2, D2 + A2 + B2 + T2 =:= 12),
    check(received, B2 =:= 2) )).

%%% ========================================================
%%% Lifecycle (spec:37-44, api:12-15)
%%% ========================================================

test('acknowledge: Dispatched->Unknown->Active; repeated status is Ok without a version bump (div-ack-noop-version)', (
    init(wedge, S0),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok, ack(0, false) - ok], S1),
    status_of(S1, 0, unknown), version(S1, N1),
    run(S1, [ack(0, false) - ok], S2), check(same_state, S2 == S1), version(S2, N1),
    run(S2, [ack(0, true) - ok], S3), status_of(S3, 0, active), version(S3, N3), check(bump, N3 =:= N1 + 1),
    run(S3, [ack(0, true) - ok, ack(0, false) - ok], S4), check(active_ignores, S4 == S3),
    run(S0, [ack(0, true) - err(not_found)], _) )).

test('TERM-1: same terminal again is Ok no-op; the opposite is InvalidState (spec:43)', (
    init(wedge, S0),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok,
             settle(x1, 0, true) - ok], S1),
    run(S1, [settle(x1, 0, true) - ok], S2), check(noop, S2 == S1),
    run(S1, [settle(x1, 0, false) - err(invalid_state), settle(a1, 0, true) - err(unauthorized),
             ack(0, true) - ok], S3),
    check(late_ack_ignored, S3 == S1) )).

test('cancel: actor or owner before dispatch; in flight Pending; Completed InvalidState (spec:43; api:17)', (
    init(wedge, S0),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), cancel(z, 0, 101) - err(unauthorized),
             cancel(o, 0, 101) - ok, cancel(a1, 0, 102) - ok,             % idempotent
             submit(a1, 0, 1, 2, 300, 102) - ok(1), dispatch(a1, 1, 103) - ok,
             cancel(a1, 1, 104) - err(pending), settle(x1, 1, true) - ok,
             cancel(a1, 1, 105) - err(invalid_state),                    % v1.0.0: err(pending)
             cancel(o, 1, 105) - err(invalid_state),
             cancel(z, 1, 105) - err(unauthorized)], S),
    status_of(S, 0, cancelled), status_of(S, 1, completed) )).

test('dispatch: actor, unpaused, Reserved, unexpired, fresh (spec:37)', (
    init(risk, R0),
    run(R0, [submit(a2, 3, 1, 0, 200, 100) - ok(0),
             dispatch(a1, 0, 101) - err(unauthorized),
             pause(o, true) - ok, dispatch(a2, 0, 101) - err(paused), pause(o, false) - ok,
             dispatch(a2, 0, 200) - err(expired),
             dispatch(a2, 9, 101) - err(not_found)], R1),
    run(R1, [dispatch(a2, 0, 199) - ok, dispatch(a2, 0, 199) - err(invalid_state)], _),
    run(R0, [submit(a2, 3, 1, 0, 1000, 100) - ok(0), dispatch(a2, 0, 401) - err(snapshot_expired)], _) )).

test('PAUSE-1: pausing blocks admission and dispatch, not settlement, ack, cancel or observe (spec:54)', (
    init(wedge, S0),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), submit(a1, 0, 1, 1, 200, 100) - ok(1),
             dispatch(a1, 0, 101) - ok, pause(o, true) - ok,
             submit(a1, 0, 1, 4, 200, 101) - err(paused), dispatch(a1, 1, 101) - err(paused),
             ack(0, true) - ok, settle(x1, 0, true) - ok, cancel(a1, 1, 102) - ok,
             observe(o, [5, 5], 103) - ok, pause(o, true) - ok], S),
    version(S, N), note("version ~d (pause to the same value still bumps: CODE-ONLY)", [N]), N =:= 9 )).

test('observe: owner, all terminal or a restatement, bounded values of the right length (spec:58)', (
    init(wedge, S0),
    run(S0, [observe(a1, [5, 5], 100) - err(unauthorized),
             observe(o, [5], 100) - err(bounds), observe(o, [5, 5, 5], 100) - err(bounds),
             observe(o, [1000001, 5], 100) - err(bounds),
             observe(o, [-1000000, 1000000], 150) - ok], S),
    pool_values(S, V), V == [-1000000, 1000000],
    S = pool(_, _, _, 150, _, _, _, _),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0),
             observe(o, [5, 5], 101) - err(pending),                     % new values, a record open
             observe(o, [3, 6], 101) - err(pending),                     % only the second differs
             observe(o, [3, 5], 102) - ok,                               % the refresh
             observe(a1, [3, 5], 102) - err(unauthorized)], R),
    check(refreshed, ( pool_observed_at(R, 102), pool_values(R, [3, 5]), pool_version(R, 2) )) )).

%%% ========================================================
%%% Reads (spec:27) on the model. test_reads.pl compares the real core's
%%% preview and best_quantity with the model (COV-2).
%%% ========================================================

test('RD-PREVIEW (model): preview_allowed <=> submit Ok, over a request grid', (
    init(risk, R0),
    run(R0, [submit(a2, 3, 1, 0, 200, 100) - ok(0)], R1),
    forall(( member(S, [R0, R1]), member(A, [a1, a2, z]), between(0, 4, RI), between(0, 21, U) ),
           ( pool_version(S, V),
             (   preview_allowed(S, A, req(RI, U, V, 200, 100))
             ->  check(allowed_submits(A, RI, U), step(S, submit(A, RI, U, V, 200, 100), ok(_), _))
             ;   check(rejected_rejects(A, RI, U), step(S, submit(A, RI, U, V, 200, 100), err(_), _))
             ) )),
    % unconditional at v1.1.0: the cap is candidate's last check, so at
    % VERSION_CAP preview refuses exactly as submit does (lib.rs:319-320)
    version_cap(Cap), cfg(zero, Z), SC = pool(Z, [5], Cap, 100, false, [], 0, [0]),
    check(cap_preview, \+ preview_allowed(SC, a1, req(0, 1, Cap, 200, 100))),
    check(cap_submit, step(SC, submit(a1, 0, 1, Cap, 200, 100), err(arithmetic), _)) )).

test('RD-BEST (model): best_quantity = brute-force minimum energy, ties to larger units', (
    forall(( member(F, [hiring, risk, inventory]), init(F, S), cfg(F, C),
             C = config(_, _, _, _, Rs), length(Rs, NR), NR1 is NR - 1,
             between(0, NR1, RI), actor_of(C, RI, A), member(U, [1, 3, 20, 150]) ),
           ( best_quantity(S, A, req(RI, U, 0, 200, 100), B),
             Hi is min(U, 100),
             findall(E-K, ( between(1, Hi, K), admissible(S, A, req(RI, K, 0, 200, 100)),
                            step(S, submit(A, RI, K, 0, 200, 100), ok(_), S1),
                            pool_values(S1, V1), energy(C, V1, E) ), Cs),
             (   Cs == [] -> check(none(F, RI, U), B == none)
             ;   findall(E, member(E-_, Cs), Es), min_list(Es, Emin),
                 findall(K, member(Emin-K, Cs), Ks), max_list(Ks, Kb),
                 check(best(F, RI, U, B, Kb), B == some(Kb))
             ) )) )).

%%% ========================================================
%%% Properties on hand-built states (the checkers can fire)
%%% ========================================================

test('LIVE-1, LIVE-ALL and INV-RES fire on v1.0.0\'s F-1 wedge state (unreachable at v1.1.0)', (
    init(wedge, W0), W0 = pool(C, _, _, O, P, _, _, D),
    % trace 0 at v1.0.0: observe [12,5], the hedge reserved and Active at [10,5]
    S = pool(C, [10, 5], 4, O, P, [c(0, 1, 200, active)], 0, D), G = g([12, 5], 0, 4),
    check(live1, live1_violation(S, G, 'LIVE-1'(_, err(bounds)))),
    findall(Op-W, probe_violation(S, G, 'LIVE-ALL'(_, Op, W)), LA), note("LIVE-ALL ~q", [LA]),
    check(live_all_both_ways, LA = [settle(x1, 0, true)-err(bounds), settle(x1, 0, false)-err(bounds)]),
    check(inv_res, state_violation(S, G, 'INV-RES'(metric(0), ev(2), pv(0)))),
    % the refresh still works there (REFRESH-1): only the settlement is stuck
    check(refresh_ok, \+ probe_violation(S, G, 'REFRESH-1'(_))) )).

test('WIN-ESC: Full exactly with an open front; resolving the front frees a slot (window of 2)', (
    with_window_size(2, (
        cfg(zero, C), init_state(C, 100, S0),
        run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), submit(a1, 0, 1, 1, 200, 100) - ok(1),
                 submit(a1, 0, 1, 2, 200, 100) - err(full)], S),
        ghost_init(S0, G0),
        check(no_win_esc, \+ probe_violation(S, G0, 'WIN-ESC'(_))),
        run(S, [dispatch(a1, 0, 100) - ok, settle(x1, 0, false) - ok,
                submit(a1, 0, 1, 4, 200, 100) - ok(2)], S2),
        check(pruned, pool_first_id(S2, 1)) )) )).

test('edge checkers fire on wrong transitions (TERM-1, WIN-1, WIN-2, VER-CAP, OBS-1, OBS-2)', (
    cfg(zero, C), init_state(C, 100, S0),
    run(S0, [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 100) - ok,
             settle(x1, 0, true) - ok, submit(a1, 0, 1, 3, 200, 100) - ok(1)], S),
    % TERM-1: cancel of a Completed record answered Pending (v1.0.0's outcome)
    check(term1, edge_violation(S, cancel(a1, 0, 100), err(pending), S, 'TERM-1'(_, _))),
    % WIN-1: a submit that returns a used id
    run(S, [submit(a1, 0, 1, 4, 200, 100) - ok(2)], S3),
    check(win1, edge_violation(S, submit(a1, 0, 1, 4, 200, 100), ok(1), S3, 'WIN-1'(_))),
    % WIN-2: an Ok call on a pruned id
    S = pool(C, V, N, O, P, [_, R1], _, D), Sp = pool(C, V, N, O, P, [R1], 1, D),
    check(win2, edge_violation(Sp, dispatch(a1, 0, 100), ok, Sp, 'WIN-2'(_, _))),
    % VER-CAP: set_paused Ok at the cap; observe failing Pending instead of Arithmetic is fine only if it is Pending
    version_cap(Cap), Sc = pool(C, V, Cap, O, P, [R1], 1, D), Cap1 is Cap + 1,
    check(vercap_ok, edge_violation(Sc, pause(o, true), ok, pool(C, V, Cap1, O, true, [R1], 1, D), 'VER-CAP'(_))),
    check(vercap_reason, edge_violation(Sc, pause(o, true), err(unauthorized), Sc, 'VER-CAP'(_))),
    % OBS-1: an Ok observe that changes the values with a record open; OBS-2: a refresh that also pauses
    N1 is N + 1,
    check(obs1, edge_violation(S, observe(o, [4], 100), ok, pool(C, [4], N1, 100, P, [_, R1], 0, D), 'OBS-1'(_))),
    S = pool(_, _, _, _, _, Rs, _, _),
    check(obs2, edge_violation(S, observe(o, V, 100), ok, pool(C, V, N1, 100, true, Rs, 0, D), 'OBS-2'(_))) )).

test('INV checkers fire on corrupted states (INV-LEDGER, INV-HEALTHY, INV-HARD, INV-WF, INV-DEP, INV-VER, WIN-1)', (
    init(hiring, H0),
    run(H0, [submit(a1, 0, 2, 0, 200, 100) - ok(0)], H1),
    ghost_init(H0, G0), ghost_step(G0, H0, submit(a1, 0, 2, 0, 200, 100), ok(0), H1, G1),
    check(clean, \+ state_violation(H1, G1, _)),
    H1 = pool(C, V, N, O, P, R, F, D),
    V = [V0 | Vt], V0b is V0 + 1,
    check(ledger, state_violation(pool(C, [V0b | Vt], N, O, P, R, F, D), G1, 'INV-LEDGER'(_))),
    check(ghost_window, state_violation(H1, g(V, 5, 1), 'INV-LEDGER'(ghost_outside_window(_, _, _)))),
    check(wf, state_violation(pool(C, V, N, O, P, [c(0, 0, 200, reserved)], F, D), G1, 'INV-WF'(_))),
    check(wf_done, state_violation(pool(C, V, N, O, P, R, F, [0]), G1, 'INV-WF'(completed_units))),
    check(dep, state_violation(pool(C, V, N, O, P, [c(2, 1, 200, reserved)], F, D), G1, 'INV-DEP'(_, _, _))),
    check(ver, state_violation(pool(C, V, 7, O, P, R, F, D), G1, 'INV-VER'(_, _))),
    check(win1_next_id, state_violation(pool(C, V, N, O, P, R, 3, D), G1, 'WIN-1'(next_id_above_version(_, _)))),
    check(win1_done, state_violation(pool(C, V, N, O, P, [c(0, 2, 200, completed)], F, D), G1,
                                     'WIN-1'(completed_units(0, 0, 2)))),
    init(wedge, W0),
    W0 = pool(WC, _, _, WO, WP, _, _, WD),
    % v1.0.0's F-1 state (record in flight at [10,5]) breaks INV-RES whatever the base;
    % with an in-bounds base it breaks INV-HEALTHY too
    check(healthy, state_violation(pool(WC, [10, 5], 1, WO, WP, [c(0, 1, 200, dispatched)], 0, WD),
                                   g([10, 5], 0, 1), 'INV-HEALTHY'(_, _))),
    % INV-HARD with B /= 0: a loss trade open next to a hedge in a disturbed risk state
    init(risk, R0), R0 = pool(RC, _, _, RO, RP, _, _, RD),
    check(hard, state_violation(pool(RC, [130, 45, 30, 33], 2, RO, RP, [c(3, 1, 200, dispatched), c(0, 1, 200, reserved)], 0, RD),
                                g([110, 40, 30, 40], 0, 2), 'INV-HARD'(_, _, _))) )).

test('ghost: fold-on-prune keeps INV-LEDGER, and a refresh keeps the base (window of 2)', (
    with_window_size(2, (
        % energy reads metric 1 only, which no recipe moves: every submit is admitted
        C = config(o, 0, [m(0, 100, 50), m(0, 100, 50)], [t(1, 50, 1, target)], [r(a1, x1, [1, 0], [1, 0], 1, [])]),
        init_state(C, 100, S0), ghost_init(S0, G0),
        Ops = [submit(a1, 0, 1, 0, 200, 100) - ok(0), dispatch(a1, 0, 101) - ok,
               settle(x1, 0, true) - ok,                                  % [52,50]
               submit(a1, 0, 1, 3, 200, 101) - ok(1),                     % [53,50]
               observe(o, [53, 50], 102) - ok,                            % refresh: base kept
               cancel(a1, 1, 102) - ok,
               submit(a1, 0, 1, 6, 200, 102) - ok(2)],                    % prunes the Completed id 0
        walk(Ops, S0, G0, S, G),
        note("final ghost ~q", [G]),
        check(pruned, pool_first_id(S, 1)),
        G = g(B, K, E), check(base_folded, ( B == [52, 50], K =:= 1, E =:= 7 )) )) )).

walk([], S, G, S, G).
walk([Op-Exp | Ps], S0, G0, S, G) :-
    run(S0, [Op-Exp], S1), step(S0, Op, R, S1),
    ghost_step(G0, S0, Op, R, S1, G1),
    findall(V, ( state_violation(S1, G1, V) ; edge_violation(S0, Op, R, S1, V) ), Vs),
    check(clean(Op, Vs), Vs == []),
    walk(Ps, S1, G1, S, G).

test('window_size: 256 and VERSION_CAP = constant(version_cap) by default; with_window_size is scoped', (
    window_size(W), version_cap(Cap), note("window ~d, cap ~d", [W, Cap]),
    check(default_window, ( W =:= 256, constant(max_records, 256, _) )),
    check(default_cap, constant(version_cap, Cap, _)),
    with_window_size(2, ( window_size(2), version_cap(C2), u32_max(M), C2 =:= M - 6 )),
    check(after_success, window_size(256)),
    check(after_failure, ( \+ with_window_size(3, fail), window_size(256) )),
    catch(with_window_size(1, throw(stop)), stop, true),
    check(after_exception, window_size(256)) )).

test('init_state: version 0, observed_at = now, unpaused, no records, first_id 0, no completed units (CFG-1 model side)', (
    forall(cfg(_, C), ( init_state(C, 100, S), S = pool(C, V, 0, 100, false, [], 0, D),
                        C = config(_, _, Ms, _, Rs), findall(I, member(m(_, _, I), Ms), V),
                        findall(0, member(_, Rs), D) )) )).

test('valid_config rejects out-of-range configs (CFG-2 model side)', (
    cfg(wedge, config(O, A, Ms, Ts, Rs)),
    check(ok, valid_config(config(O, A, Ms, Ts, Rs))),
    check(term_metric, \+ valid_config(config(O, A, Ms, [t(2, 0, 1, target)], Rs))),
    check(weight, \+ valid_config(config(O, A, Ms, [t(1, 0, 1001, target)], Rs))),
    check(effect_len, \+ valid_config(config(O, A, Ms, Ts, [r(a1, x1, [1], [1, 1], 1, [])]))),
    check(units, \+ valid_config(config(O, A, Ms, Ts, [r(a1, x1, [1, 1], [1, 1], 101, [])]))),
    check(self_prereq, \+ valid_config(config(O, A, Ms, Ts, [r(a1, x1, [1, 1], [1, 1], 1, [p(0, 1)])]))),
    check(self_executor, \+ valid_config(config(O, A, Ms, Ts, [r(a1, a1, [1, 1], [1, 1], 1, [])]))),
    check(initial, \+ valid_config(config(O, A, [m(0, 10, 11), m(0, 1, 0)], Ts, Rs))) )).

min_list([X | Xs], M) :- foldl_min(Xs, X, M).
foldl_min([], M, M).
foldl_min([X | Xs], M0, M) :- M1 is min(X, M0), foldl_min(Xs, M1, M).
max_list([X | Xs], M) :- foldl_max(Xs, X, M).
foldl_max([], M, M).
foldl_max([X | Xs], M0, M) :- M1 is max(X, M0), foldl_max(Xs, M1, M).
