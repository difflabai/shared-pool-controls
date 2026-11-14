%% harness.pl -- the test runner shared by the tests/test_*.pl files
%%
%% It lives in tests/support/ so tools/scripts/run-prolog.sh, which runs every
%% tests/*.pl, sees only test files.
%%
%% Each test file defines test(Name, Goal) clauses and a title, and calls
%% run_suite(Title, Module). run_suite prints PASS/FAIL per test and halts:
%% 0 if every test passed, 1 if any failed, 2 on an uncaught exception. It
%% never returns, so a caller's trailing goal cannot mask a failure.
%%
%% Canary: with SHARED_POOL_PROLOG_CANARY=1 in the environment one deliberately
%% failing test runs in every suite, so the runner can show that a failure makes
%% the file exit non-zero (run.sh requires that).

:- module(harness, [run_suite/2, check/2, note/2]).

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module(library(os)).

:- dynamic(failures/1).
:- dynamic(passes/1).
failures(0).
passes(0).

:- meta_predicate(run_suite(+, :)).

run_suite(Title, M:_) :-
    format("=== ~s ===~n", [Title]),
    catch(run_all(M), E, ( format("ERROR: uncaught exception ~q~n", [E]), halt(2) )),
    failures(F), passes(P),
    (   F =:= 0
    ->  format("~n=== ALL ~d TESTS PASSED ===~n", [P]), halt(0)
    ;   format("~n=== ~d TEST(S) FAILED, ~d passed ===~n", [F, P]), halt(1)
    ).

run_all(M) :-
    findall(N-G, M:test(N, G), Ts),
    forall(member(N-G, Ts), run_one(N, M:G)),
    (   getenv("SHARED_POOL_PROLOG_CANARY", "1")
    ->  run_one(canary_must_fail, fail)
    ;   true
    ).

run_one(Name, Goal) :-
    (   catch(call(Goal), E, ( format("  exception in ~w: ~q~n", [Name, E]), fail ))
    ->  format("PASS ~w~n", [Name]), bump(passes)
    ;   format("FAIL ~w~n", [Name]), bump(failures)
    ).

bump(K) :-
    G =.. [K, N], retract(G), N1 is N + 1, G1 =.. [K, N1], assertz(G1).

%% check(+Label, :Goal): Goal must succeed; on failure print Label (used inside tests
%% to say WHICH sub-condition failed).
:- meta_predicate(check(+, 0)).
check(Label, Goal) :-
    (   call(Goal) -> true
    ;   format("    check failed: ~q~n", [Label]), fail
    ).

%% note(+Fmt, +Args): indented information line.
note(Fmt, Args) :- format("    ", []), format(Fmt, Args), nl.
