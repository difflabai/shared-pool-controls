%% traces.pl -- Reading the conformance traces (projects/shared-pool/conformance/traces/core.txt)
%%
%% PROP: MC-2 C-3 C-5
%%
%% Format (conformance/src/main.rs header): one Prolog term per line,
%%   trace(N). config(..). init(Now, State). call(Op, Result, State).. end.
%% or, for a config the code refuses (v1.1.0: Pool::new returns an error),
%%   trace(N). config(..). init_error(E). end.
%% State is s(Values, Version, ObservedAt, Paused, Records, FirstId, DoneUnits)
%% (v1.1.0: the retained window starts at id FirstId; DoneUnits are the
%% completed_units counters per recipe). The traces are generated from the
%% real pool-core, so every result and post-state in them is the code's.
%%
%%   trace_file(-Path)              the checked-in trace file (found from the cwd)
%%   prolog_root(-Root)             the prolog/ directory, as a char list path
%%   fold_traces(+File, :Goal, +A0, -A)
%%       calls Goal(trace(N, Cfg, Now, Init, Calls), A0, A1) for each trace in
%%       order; Calls = [call(Op, Result, State)..]. A refused config gives
%%       Now = none, Init = init_error(E), Calls = []. One trace is in memory at a time.
%%   trace_to_pool(+Cfg, +s(..), -Pool)

:- module(traces, [
    trace_file/1,
    prolog_root/1,
    root_path/2,
    fold_traces/4,
    trace_to_pool/3
]).

:- use_module(library(lists)).
:- use_module(library(files)).
:- use_module(library(iso_ext)).

:- meta_predicate(fold_traces(+, 3, +, -)).

prolog_root(Root) :-
    (   file_exists("requirements/pool_req.pl") -> Root = "."
    ;   file_exists("../requirements/pool_req.pl") -> Root = ".."
    ;   file_exists("projects/shared-pool/prolog/requirements/pool_req.pl")
    ->  Root = "projects/shared-pool/prolog"
    ;   throw(error(existence_error(directory, prolog_root), traces))
    ).

root_path(Rel, Path) :-
    prolog_root(Root),
    append(Root, [/ | Rel], Path).

trace_file(Path) :- root_path("../conformance/traces/core.txt", Path).

trace_to_pool(Cfg, s(V, N, O, P, R, F, D), pool(Cfg, V, N, O, P, R, F, D)).

fold_traces(File, Goal, A0, A) :-
    setup_call_cleanup(open(File, read, In),
                       fold_(In, Goal, A0, A),
                       close(In)).

fold_(In, Goal, A0, A) :-
    read(In, T),
    (   T == end_of_file -> A = A0
    ;   T = trace(N)
    ->  read(In, config(O, MA, Ms, Ts, Rs)),
        read(In, I),
        (   I = init(Now, Init) -> read_calls(In, Calls)
        ;   I = init_error(E) -> Now = none, Init = init_error(E), read_calls(In, Calls)
        ;   throw(error(syntax_error(unexpected_term(I)), fold_traces))
        ),
        call(Goal, trace(N, config(O, MA, Ms, Ts, Rs), Now, Init, Calls), A0, A1),
        fold_(In, Goal, A1, A)
    ;   throw(error(syntax_error(unexpected_term(T)), fold_traces))
    ).

read_calls(In, Calls) :-
    read(In, T),
    (   T == end -> Calls = []
    ;   T = call(Op, R, S) -> Calls = [call(Op, R, S) | Cs], read_calls(In, Cs)
    ;   throw(error(syntax_error(unexpected_term(T)), fold_traces))
    ).
