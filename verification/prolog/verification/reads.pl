%% reads.pl -- COV-2: the real core's reads (preview, best_quantity) against the model
%%
%% PROP: RD-PREVIEW RD-BEST
%%
%% conformance/traces/reads.txt (format: the header of conformance/src/main.rs)
%% records the real core's preview(caller, req, now) and best_quantity(caller,
%% req, now) at states of the conformance traces, for a request set per state.
%% For each request this module computes the model's answer from the same
%% state and compares it field by field:
%%   allowed    preview_allowed/3
%%   reason     `allowed`, or the first failing submit guard (classify/3 on the
%%              submit op, as step/4 walks it). The last guard,
%%              version_cap, is candidate's last check too (v1.1.0,
%%              lib.rs:319-320): at the 12 states of reads.txt at VERSION_CAP
%%              or u32::MAX, a request that passes every other check is
%%              refused Arithmetic.
%%   candidate  values + reserve * units by add_vec/4 when the recipe exists
%%              and units =< MAX_UNITS, else none. preview computes it whether
%%              or not the request is allowed.
%%   before, after  energy/3 of the values and of the candidate (none: none)
%%   best       best_quantity/4
%% A difference is a divergence, div(Trace, Call, K, Field, Request, code(X),
%% model(Y)) with K the request's index in its state: a bug in the model or a
%% finding in the code, never papered over.
%%
%% This is a differential test on the recorded states and requests, not a
%% proof. The per-term energies of preview (terms_before, terms_after) and the
%% recorded request's other fields in best_quantity's result (always the
%% request's own) are not compared.
%%
%%   reads_file(-Path)
%%   fold_reads(+File, :Goal, +A0, -A)
%%       calls Goal(block(Trace, Call, Cfg, State, Qs), A0, A1) per state;
%%       Qs = [q(Caller, Recipe, Units, Version, ExpiresAt, Now, Preview, Best)..]
%%   model_read(+Pool, +Caller, +req(Recipe, Units, Version, ExpiresAt, Now), -Preview, -Best)
%%   compare_block(+Block, -Divs, -Keys)
%%       Keys: the classes of the block's reads (see read_keys/3)
%%   reads_run(+File, -reads(States, Requests, Divs, Tally))
%%       Tally = [Key-Count..] over all reads

:- module(reads, [
    reads_file/1,
    fold_reads/4,
    model_read/5,
    compare_block/3,
    reads_run/2
]).

:- use_module(library(lists)).
:- use_module(library(iso_ext)).
:- use_module('../requirements/pool_req').
:- use_module('../model/pool_machine').
:- use_module(traces).
:- use_module(util).

:- meta_predicate(fold_reads(+, 3, +, -)).

reads_file(Path) :- root_path("../conformance/traces/reads.txt", Path).

fold_reads(File, Goal, A0, A) :-
    setup_call_cleanup(open(File, read, In),
                       fold_(In, Goal, A0, A),
                       close(In)).

fold_(In, Goal, A0, A) :-
    read(In, T),
    (   T == end_of_file -> A = A0
    ;   T = reads(N, I)
    ->  read(In, config(O, MA, Ms, Ts, Rs)),
        read(In, state(S)),
        read_qs(In, Qs),
        call(Goal, block(N, I, config(O, MA, Ms, Ts, Rs), S, Qs), A0, A1),
        fold_(In, Goal, A1, A)
    ;   throw(error(syntax_error(unexpected_term(T)), fold_reads))
    ).

read_qs(In, Qs) :-
    read(In, T),
    (   T == end -> Qs = []
    ;   T = q(_, _, _, _, _, _, _, _) -> Qs = [T | Qs1], read_qs(In, Qs1)
    ;   throw(error(syntax_error(unexpected_term(T)), fold_reads))
    ).

%% model_read(+S, +Caller, +Req, -Preview, -Best): the model's preview and
%% best_quantity, in the recorded shape.
model_read(S, Caller, req(RI, U, V, E, Now), preview(A, R, Cand, Before, After), Best) :-
    Req = req(RI, U, V, E, Now),
    (   preview_allowed(S, Caller, Req) -> A = true ; A = false ),
    classify(S, submit(Caller, RI, U, V, E, Now), Class),
    (   Class == effect -> R = allowed
    ;   Class = stop(err(Err)) -> camel_snake(Err, R)
    ;   R = Class
    ),
    pool_config(S, C), pool_values(S, Vs),
    (   recipe_of(C, RI, r(_, _, Res, _, _, _)),
        model_constant(max_units, MaxU, _), U =< MaxU,
        add_vec(Vs, Res, U, Next)
    ->  Cand = Next
    ;   Cand = none
    ),
    energy(C, Vs, Before),
    (   Cand == none -> After = none ; energy(C, Cand, After) ),
    best_quantity(S, Caller, Req, Best).

%% compare_block(+Block, -Divs, -Keys)
compare_block(block(N, I, Cfg, State, Qs), Divs, Keys) :-
    trace_to_pool(Cfg, State, S),
    compare_qs(Qs, 1, N, I, S, [], Divs, [], Keys0),
    group_keys(Qs, Keys1),
    append(Keys0, Keys1, Keys).

compare_qs([], _, _, _, _, D, D, K, K).
compare_qs([Q | Qs], K, N, I, S, D0, D, Ks0, Ks) :-
    Q = q(C, RI, U, V, E, Now, P, B),
    Req = req(C, RI, U, V, E, Now),
    (   catch(model_read(S, C, req(RI, U, V, E, Now), MP, MB), Ex,
              ( MP = exception(Ex), MB = exception ))
    ->  true
    ;   MP = no_result, MB = no_result
    ),
    (   MP = preview(MA, MR, MC, MBe, MAf), P = preview(A, R, Ca, Be, Af)
    ->  findall(div(N, I, K, F, Req, code(X), model(Y)),
                ( member(F-X-Y, [allowed-A-MA, reason-R-MR, candidate-Ca-MC, before-Be-MBe,
                                 after-Af-MAf, best-B-MB]),
                  X \== Y ),
                D1)
    ;   D1 = [div(N, I, K, model, Req, code(P, B), model(MP, MB))]
    ),
    append(D1, D0, D2),
    read_keys(Q, Ks0, Ks1),
    K1 is K + 1,
    compare_qs(Qs, K1, N, I, S, D2, D, Ks1, Ks).

%% read_keys(+Q, +Ks0, -Ks): one key per class of the code's read.
%%   allowed(true|false), reason(R), candidate(some|none), best(some|none),
%%   best_capped (a best for units > MAX_UNITS)
read_keys(q(_, _, U, _, _, _, preview(A, R, Ca, _, _), B), Ks0, Ks) :-
    ( Ca == none -> CK = candidate(none) ; CK = candidate(some) ),
    ( B == none -> BK = best(none) ; BK = best(some) ),
    model_constant(max_units, MaxU, _),
    (   B = some(_), U > MaxU -> Extra = [best_capped] ; Extra = [] ),
    append([[allowed(A), reason(R), CK, BK], Extra, Ks0], Ks).

%% group_keys(+Qs, -Keys): classes read from several requests of one state
%% that differ only in units (same caller, recipe, version, expiry, now):
%%   best_not_largest  units U1 allowed, and a request with units >= U1 has
%%                     best some(B) with B < U1
%%   best_tie          units U1 < U2 both allowed with equal candidate
%%                     energy, and a request with units >= U2 has best some(U2)
%%                     (an energy tie broken to the larger units)
group_keys(Qs, Keys) :-
    (   member(q(C, RI, U1, V, E, Now, preview(true, _, _, _, _), _), Qs),
        member(q(C, RI, U2, V, E, Now, _, some(B)), Qs),
        U2 >= U1, B < U1
    ->  K1 = [best_not_largest] ; K1 = []
    ),
    (   member(q(C, RI, U1, V, E, Now, preview(true, _, _, _, Af), _), Qs),
        member(q(C, RI, U2, V, E, Now, preview(true, _, _, _, Af), _), Qs),
        U1 < U2,
        member(q(C, RI, U3, V, E, Now, _, some(U2)), Qs),
        U3 >= U2
    ->  K2 = [best_tie] ; K2 = []
    ),
    append(K1, K2, Keys).

reads_run(File, reads(NS, NQ, Divs, Tally)) :-
    fold_reads(File, run_block, acc(0, 0, [], []), acc(NS, NQ, D0, K0)),
    msort_dup(D0, Divs),
    msort_dup(K0, K1), tally(K1, Tally).

run_block(Block, acc(NS0, NQ0, D0, K0), acc(NS, NQ, D, K)) :-
    Block = block(_, _, _, _, Qs),
    compare_block(Block, D1, K1),
    length(Qs, L),
    NS is NS0 + 1, NQ is NQ0 + L,
    append(D1, D0, D), append(K1, K0, K).
