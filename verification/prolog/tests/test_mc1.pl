%% test_mc1.pl -- MC-1: bounded exploration of the model's small universes
%%
%% PROP: MC-1 LIVE-2 LIVE-1 INV-WF INV-LEDGER INV-HARD INV-HEALTHY INV-DEP INV-VER
%% PROP: INV-RES INV-VERCAP WIN-1 WIN-2 WIN-ESC LIVE-ALL VER-CAP OBS-2 REFRESH-1
%%
%% Every universe: every invariant (INV-*, WIN-1), step property (TX-*, AUTH-1,
%% ..., WIN-1/WIN-2 edges, TERM-1, OBS-1/OBS-2, VER-CAP) and probe (LIVE-1,
%% LIVE-ALL, REFRESH-1, WIN-ESC) holds on every reachable state and edge.
%%
%% wedge   (trace 0's config; a disturbing and an in-bounds observation): no
%%         record is certified dead. v1.0.0: 228 states with a dead in-flight
%%         record (F-1); at v1.1.0 the hedge is rejected Bounds in [12,5].
%% healthy (two recipes with a prerequisite, freshness, in-bounds observations):
%%         no open record is dead (LIVE-2 within the bound), none release-blocked.
%% reliance (F-2 scaled down): no release-blocked state. v1.0.0: 150 states
%%         with B = 0 where the position's honest settle(false) failed Bounds.
%% window, window1 (healthy's config with a window of 2 and 1, no truncation;
%%         F-3): submits prune a terminal front, fail Full on an open front,
%%         and admit recipe 1 on a pruned prerequisite; calls on a pruned id
%%         are NotFound. Dead-record analysis does not apply (ids move), LIVE-ALL does.
%% cap     (window 2, VERSION_CAP = u32::MAX - 6, from 4 below the cap; F-5):
%%         gated calls stop with Arithmetic at the cap, ack/cancel/settle stay Ok
%%         past it (INV-VERCAP, LIVE-ALL at the cap); no dead record.
%% f2b     (trace 208's config; F-2b): admission keeps every release term
%%         within +-1e6 (INV-RES endpoint clause); submits fail Arithmetic.
%% Every run is scoped: afterwards window_size/1 is the code's 256 again.
%%
%% Run: scryer-prolog test_mc1.pl -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null
%% (about 2 minutes)

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module('support/harness').

:- discontiguous(test/2).
:- use_module('../model/pool_machine').
:- use_module('../verification/model_check').

run_tests :- run_suite("shared-pool Prolog: MC-1 small model check", x).

report(R) :-
    R = mc1(Name, St, Ed, Tr, violations(Vs), DI, DR, W, hits(H), complete(C), RB, tags(Tg)),
    length(Vs, NV),
    note("~w: ~w ~w ~w violations(~d) ~w ~w complete(~w)", [Name, St, Ed, Tr, NV, DI, DR, C]),
    note("witness: ~q", [W]),
    note("~q", [RB]),
    note("tags: ~q", [Tg]),
    note("results: ~q", [H]),
    (   NV > 0 -> forall(( nth1(K, Vs, V), K =< 10 ), note("VIOLATION ~q", [V])) ; true ).

ok_kinds(H, Ks) :- findall(K, member(K-ok-_, H), Ks0), sort(Ks0, Ks).
tag_count(Tg, T, N) :- ( memberchk(T-N0, Tg) -> N = N0 ; N = 0 ).

%% clean(+R): complete, no violation, the window back at its default.
clean(R) :-
    R = mc1(_, states(NS), _, _, violations(Vs), _, _, _, _, complete(C), _, _),
    check(complete, C == true), check(states(NS), NS > 100),
    check(no_violations, Vs == []),
    check(window_restored, window_size(256)).

no_dead(R) :-
    R = mc1(_, _, _, _, _, dead_inflight(DI), dead_reserved(DR), _, _, _, release_blocked(count(NB), _), _),
    check(no_dead_inflight, DI = count(0, _)), check(no_dead_reserved, DR = count(0, _)),
    check(no_release_blocked, NB =:= 0).

test('MC-1 wedge: properties hold and no record is dead (F-1 fixed; v1.0.0: 228 dead)', (
    mc1_run(wedge, R), report(R), clean(R), no_dead(R),
    R = mc1(_, _, _, _, _, _, _, _, hits(H), _, _, tags(Tg)),
    % the hedge in the disturbed state is now rejected at admission
    tag_count(Tg, submit_rejected(bounds), NBd), check(hedge_rejected, NBd > 0),
    check(f1_shape_rejected, ( mc1_witness(wedge, submit_rejected(bounds), P),
                               P = [observe(o, [12, 5], _) | _], last(P, submit(a1, 0, 1, _, _, _)) )),
    tag_count(Tg, refresh, NRf), check(refresh_reached, NRf > 0),
    ok_kinds(H, Ks), check(ok_kinds(Ks), Ks == [ack, cancel, dispatch, observe, pause, settle, submit]) )).

test('MC-1 healthy: properties hold and no open record is dead (LIVE-2 within the bound)', (
    mc1_run(healthy, R), report(R), clean(R), no_dead(R),
    R = mc1(_, _, _, _, _, _, _, _, hits(H), _, _, _),
    ok_kinds(H, Ks), check(ok_kinds(Ks), Ks == [ack, cancel, dispatch, observe, pause, settle, submit]),
    findall(E, ( member(_-E-_, H), E \== ok ), Es0), sort(Es0, Es),
    note("errors reached: ~q", [Es]),
    forall(member(E, [unauthorized, paused, stale, snapshot_expired, expired, prerequisite, bounds, energy,
                      invalid_state, pending, not_found]),
           check(error_reached(E), memberchk(E, Es))) )).

test('MC-1 reliance: properties hold and no honest release is blocked (F-2 fixed; v1.0.0: 150 blocked)', (
    mc1_run(reliance, R), report(R), clean(R), no_dead(R),
    R = mc1(_, _, _, _, _, _, _, _, hits(H), _, _, tags(Tg)),
    % the hedge is rejected: its release term (+70 on metric 0) is in the envelope
    tag_count(Tg, submit_rejected(bounds), NBd), check(hedge_rejected, NBd > 0),
    check(f2_shape_rejected, ( mc1_witness(reliance, submit_rejected(bounds), P),
                               last(P, submit(a2, 1, 1, _, _, _)) )),
    check(settles_seen, memberchk(settle-ok-_, H)) )).

test('MC-1 window: window of 2, prune, Full on an open front, pruned prerequisite, NotFound (F-3 fixed)', (
    mc1_run(window, R), report(R), clean(R),
    R = mc1(_, _, _, _, _, dead_inflight(DI), _, _, _, _, release_blocked(count(NB), _), tags(Tg)),
    check(dead_not_applicable, DI == not_applicable(pruned)),
    check(no_release_blocked, NB =:= 0),
    forall(member(T, [prune, full_open_front, prereq_after_prune, not_found_pruned, refresh]),
           ( tag_count(Tg, T, N), check(reached(T, N), N > 0) )),
    check(prereq_witness, ( mc1_witness(window, prereq_after_prune, P), last(P, submit(a2, 1, 1, _, _, _)) )) )).

test('MC-1 window1: window of 1, the same witnesses', (
    mc1_run(window1, R), report(R), clean(R),
    R = mc1(_, _, _, _, _, _, _, _, _, _, release_blocked(count(NB), _), tags(Tg)),
    check(no_release_blocked, NB =:= 0),
    forall(member(T, [prune, full_open_front, prereq_after_prune, not_found_pruned]),
           ( tag_count(Tg, T, N), check(reached(T, N), N > 0) )) )).

test('MC-1 cap: gated calls stop at VERSION_CAP, settlement continues past it (F-5 fixed)', (
    mc1_run(cap, R), report(R), clean(R), no_dead(R),
    R = mc1(_, _, _, _, _, _, _, _, _, _, _, tags(Tg)),
    forall(member(K, [submit, dispatch, pause, observe]),
           ( tag_count(Tg, cap_stop(K), N), check(cap_stop(K, N), N > 0) )),
    forall(member(K, [ack, cancel, settle_completed, settle_cancelled]),
           ( tag_count(Tg, cap_plain(K), N), check(cap_plain(K, N), N > 0) )) )).

test('MC-1 f2b: every release term stays within +-1e6 (F-2b fixed)', (
    mc1_run(f2b, R), report(R), clean(R), no_dead(R),
    R = mc1(_, _, _, _, _, _, _, _, _, _, _, tags(Tg)),
    tag_count(Tg, submit_rejected(arithmetic), NA), check(arithmetic_at_admission, NA > 0),
    % recipe 1 at 67 units: its completion term 15000 x 67 leaves +-1e6
    check(f2b_shape, ( mc1_witness(f2b, submit_rejected(arithmetic), P),
                       last(P, submit(a1, 1, 67, _, _, _)) )) )).

test('window_size: the code\'s 256 after the scaled runs (C-2, MC-2 and the reads see 256)', (
    window_size(W), version_cap(Cap), note("window ~d, cap ~d", [W, Cap]),
    check(window, W =:= 256), check(cap, constant_cap(Cap)) )).

constant_cap(Cap) :- u32_max(M), Cap =:= M - 768.

last([X], X) :- !.
last([_ | Xs], X) :- last(Xs, X).
