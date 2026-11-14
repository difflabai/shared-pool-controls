%% util.pl -- small list helpers Scryer Prolog 0.10.0 does not provide
%% (msort/2 and last/2 are not in its library(lists)).

:- module(util, [msort_dup/2, last_of/2, count_where/3, tally/2]).

:- use_module(library(lists)).
:- use_module(library(pairs)).

%% msort_dup(+List, -Sorted): standard order, duplicates kept (keysort is stable).
msort_dup(L, S) :-
    findall(X-x, member(X, L), P),
    keysort(P, SP),
    pairs_keys(SP, S).

last_of([X], X) :- !.
last_of([_ | Xs], X) :- last_of(Xs, X).

:- meta_predicate(count_where(?, 0, -)).
count_where(T, G, N) :- findall(T, G, L), length(L, N).

%% tally(+Sorted, -Counts): [X-N..] for runs of equal elements of a sorted list.
tally([], []).
tally([X | Xs], [X-N | T]) :- take_same(X, Xs, 1, N, Rest), tally(Rest, T).
take_same(X, [Y | Ys], N0, N, Rest) :- X == Y, !, N1 is N0 + 1, take_same(X, Ys, N1, N, Rest).
take_same(_, Rest, N, N, Rest).
