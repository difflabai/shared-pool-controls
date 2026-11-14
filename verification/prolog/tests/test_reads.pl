%% test_reads.pl -- COV-2: the real core's preview and best_quantity against the model
%%
%% PROP: RD-PREVIEW RD-BEST
%%
%% 1. Differential: every read in conformance/traces/reads.txt (the real core's
%%    preview and best_quantity at states of the conformance traces, for a
%%    request set per state) equals the model's preview_allowed/3, first
%%    failing submit guard, candidate, energies and best_quantity/4, field by
%%    field (verification/reads.pl). A divergence is reported with its trace,
%%    call and request index.
%% 2. Coverage: allowed both ways; every error preview can return from a
%%    reachable state; candidate and best both ways; a best for units above
%%    MAX_UNITS; a best that is not the largest allowed units; an energy tie
%%    broken to the larger units.
%% 3. Vacuity: a read corrupted in each compared field is reported as a
%%    divergence in that field.
%%
%% This is a differential test (testing, not proof): it covers the recorded
%% states and requests only.
%%
%% Run: scryer-prolog test_reads.pl -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module('support/harness').

:- discontiguous(test/2).
:- use_module('../model/pool_machine').
:- use_module('../verification/reads').

run_tests :- run_suite("shared-pool Prolog: COV-2 reads differential", x).

:- dynamic(cached/1).

%% result(-R): reads_run over the checked-in file, once per suite run.
result(R) :-
    (   cached(R0) -> R = R0
    ;   reads_file(F), reads_run(F, R), assertz(cached(R))
    ).

count(Tally, K, N) :- ( memberchk(K-N0, Tally) -> N = N0 ; N = 0 ).

%% first_block(-B): the first state of the file (trace 0's init state).
first_block(B) :-
    reads_file(F),
    catch(fold_reads(F, take_first, none, _), first(B), true),
    nonvar(B).
take_first(B, _, _) :- throw(first(B)).

%%% ========================================================
%%% Differential
%%% ========================================================

test('RD-PREVIEW/RD-BEST differential: every read of the real core equals the model\'s', (
    result(reads(NS, NQ, Divs, Tally)),
    length(Divs, ND),
    note("states ~d, requests ~d, divergences ~d", [NS, NQ, ND]),
    note("classes ~q", [Tally]),
    (   ND > 0 -> forall(( nth1(K, Divs, D), K =< 10 ), note("DIVERGENCE ~q", [D])) ; true ),
    check(states(NS), NS >= 1200), check(requests(NQ), NQ >= 36000),
    check(divergences(ND), ND =:= 0) )).

% Non-vacuity of the differential: the reads reach every outcome class. The
% generator checks the same classes on the real core (REQUIRED_READS).
test('RD reads coverage: allowed both ways, every preview error, best and candidate both ways, ties', (
    result(reads(_, _, _, Tally)),
    forall(member(K, [allowed(true), allowed(false), candidate(some), candidate(none),
                      best(some), best(none), best_capped, best_not_largest, best_tie]),
           ( count(Tally, K, N), note("~q: ~d", [K, N]), check(seen(K), N > 0) )),
    forall(member(R, [allowed, paused, stale, expired, snapshot_expired, full, invalid_recipe,
                      unauthorized, invalid_units, prerequisite, arithmetic, bounds, energy]),
           ( count(Tally, reason(R), N), check(reason_seen(R), N > 0) )),
    findall(R, member(reason(R)-_, Tally), Rs),
    check(no_other_reason(Rs), forall(member(R, Rs),
        memberchk(R, [allowed, paused, stale, expired, snapshot_expired, full, invalid_recipe,
                      unauthorized, invalid_units, prerequisite, arithmetic, bounds, energy]))) )).

%%% ========================================================
%%% Vacuity: corrupted reads are reported in the right field
%%% ========================================================

corrupt(allowed,   q(C, R, U, V, E, N, preview(A, Rs, Ca, B, Af), Bt), q(C, R, U, V, E, N, preview(A1, Rs, Ca, B, Af), Bt)) :-
    ( A == true -> A1 = false ; A1 = true ).
corrupt(reason,    q(C, R, U, V, E, N, preview(A, _, Ca, B, Af), Bt),  q(C, R, U, V, E, N, preview(A, planted, Ca, B, Af), Bt)).
corrupt(candidate, q(C, R, U, V, E, N, preview(A, Rs, _, B, Af), Bt),  q(C, R, U, V, E, N, preview(A, Rs, none, B, Af), Bt)).
corrupt(before,    q(C, R, U, V, E, N, preview(A, Rs, Ca, B, Af), Bt), q(C, R, U, V, E, N, preview(A, Rs, Ca, B1, Af), Bt)) :- B1 is B + 1.
corrupt(after,     q(C, R, U, V, E, N, preview(A, Rs, Ca, B, _), Bt),  q(C, R, U, V, E, N, preview(A, Rs, Ca, B, none), Bt)).
corrupt(best,      q(C, R, U, V, E, N, P, some(K)), q(C, R, U, V, E, N, P, some(K1))) :- K1 is K - 1.

test('RD reads: the unmodified first state agrees; each corrupted field is a divergence there', (
    first_block(B0), B0 = block(T, I, Cfg, S, Qs),
    compare_block(B0, D0, _), check(clean(D0), D0 == []),
    % request 2 of trace 0's init state: a1, recipe 0, 1 unit, allowed, best some(1)
    nth1(2, Qs, Q), note("request 2: ~q", [Q]),
    check(allowed_with_best, Q = q(_, _, _, _, _, _, preview(true, allowed, [_ | _], _, _), some(1))),
    forall(member(F, [allowed, reason, candidate, before, after, best]),
           (   corrupt(F, Q, Q1), nth1(2, Qs, _, Rest), nth1(2, Qs1, Q1, Rest),
               compare_block(block(T, I, Cfg, S, Qs1), D1, _),
               check(reported(F, D1), D1 = [div(T, I, 2, F, _, _, _)]) )) )).
