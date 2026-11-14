%% mc2.pl -- MC-2: replay every conformance trace of the real core through step/4
%%
%% PROP: MC-2
%%
%% For each trace: the model's init_state(Config, Now) must equal the code's
%% init state (a config the code refuses, init_error(E), must be one the model's
%% valid_config/1 refuses too); then for each call, step/4 from the CODE's pre-state must give
%% exactly the code's result and post-state. A disagreement is a divergence,
%% recorded with its trace id and call index (a bug in the model or a finding
%% in the code: never papered over). After a divergence the replay continues
%% from the code's post-state, so one trace can report several.
%%
%% The same pass applies the step properties, invariants and probes (LIVE-1,
%% LIVE-ALL, REFRESH-1, WIN-ESC) of props.pl to the code's own trace
%% (post-states are the code's): a violation there is a property of the real
%% core failing on a concrete input.
%%
%%   mc2_run(+File, -Result)
%%   Result = mc2(Traces, Calls, Divergences, Violations, Hits)
%%     Divergences = [div(Trace, Index, Op, code(R, S), model(R, S))..]
%%     Violations  = [viol(Trace, Index, Op, Property)..]
%%     Hits        = [Kind-Result-Count..] of the calls replayed

:- module(mc2, [mc2_run/2, mc2_replay_trace/3]).

:- use_module(library(lists)).
:- use_module(library(iso_ext)).
:- use_module('../model/pool_machine').
:- use_module(traces).
:- use_module(props).
:- use_module(util).

mc2_run(File, mc2(NT, NC, Divs, Viols, Hits)) :-
    fold_traces(File, replay_one, acc(0, 0, [], [], []), acc(NT, NC, D0, V0, H0)),
    msort_dup(D0, Divs), msort_dup(V0, Viols),
    msort_dup(H0, H1), tally(H1, Hits).

replay_one(T, acc(NT0, NC0, D0, V0, H0), acc(NT, NC, D, V, H)) :-
    mc2_replay_trace(T, r(NC1, D1, V1, H1), _),
    NT is NT0 + 1, NC is NC0 + NC1,
    append(D1, D0, D), append(V1, V0, V), append(H1, H0, H).

%% mc2_replay_trace(+Trace, -r(Calls, Divergences, Violations, Hits), -FinalState)
%% A refused config has no state: FinalState = none.
mc2_replay_trace(trace(N, Cfg, _, init_error(E), Calls), r(0, Divs, [], []), none) :- !,
    (   \+ valid_config(Cfg), Calls == [] -> Divs = []
    ;   Divs = [div(N, 0, init, code(init_error(E), none), model(ok, none))]
    ).
mc2_replay_trace(trace(N, Cfg, Now, Init, Calls), r(NC, Divs, Viols, Hits), Final) :-
    trace_to_pool(Cfg, Init, S0),
    (   init_state(Cfg, Now, M0)
    ->  ( to_trace_state(M0, Init) -> D0 = [] ; to_trace_state(M0, MI), D0 = [div(N, 0, init, code(ok, Init), model(ok, MI))] )
    ;   D0 = [div(N, 0, init, code(ok, Init), model(rejected_config, none))]
    ),
    ghost_init(S0, G0),
    findall(V, ( state_violation(S0, G0, V0) , V = viol(N, 0, init, V0) ), IV),
    calls(Calls, N, 1, Cfg, S0, G0, D0, Divs, IV, Viols, [], Hits, Final),
    length(Calls, NC).

calls([], _, _, _, S, _, D, D, V, V, H, H, S).
calls([call(Op, R, Post) | Cs], N, I, Cfg, S0, G0, D0, D, V0, V, H0, H, Final) :-
    trace_to_pool(Cfg, Post, S1),
    (   catch(step(S0, Op, MR, MS1), E, ( MR = exception(E), MS1 = none ))
    ->  true
    ;   MR = no_result, MS1 = none
    ),
    (   MR == R, MS1 == S1
    ->  D1 = D0
    ;   ( MS1 == none -> MP = none ; to_trace_state(MS1, MP) ),
        D1 = [div(N, I, Op, code(R, Post), model(MR, MP)) | D0]
    ),
    ghost_step(G0, S0, Op, R, S1, G1),
    findall(viol(N, I, Op, P), edge_violation(S0, Op, R, S1, P), EV),
    findall(viol(N, I, Op, P), state_violation(S1, G1, P), SV),
    findall(viol(N, I, Op, P), probe_violation(S1, G1, P), LV),
    append([EV, SV, LV, V0], V1),
    functor(Op, K, _), ( R = err(Er) -> Rk = Er ; Rk = ok ),
    I1 is I + 1,
    calls(Cs, N, I1, Cfg, S1, G1, D1, D, V1, V, [K-Rk | H0], H, Final).
