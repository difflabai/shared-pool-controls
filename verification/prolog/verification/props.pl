%% props.pl -- State invariants, step properties and per-state probes, shared by MC-1 and MC-2
%%
%% PROP: MC-1 MC-2 INV-WF INV-LEDGER INV-HARD INV-HEALTHY INV-DEP INV-VER LIVE-1
%% PROP: TX-0 TX-NOOP TX-VER AUTH-1 STALE-1 ADM-1 ADM-2 HARD-STEP DEP-1 LIFE-1
%% PROP: ASYNC-1 TERM-1 OBS-1 PAUSE-1
%% PROP: INV-RES INV-VERCAP WIN-1 WIN-2 WIN-ESC LIVE-ALL VER-CAP OBS-2 REFRESH-1
%%
%% Written from the property catalogue (strategy.md section 5) and the docs
%% it cites (v1.1.0). MC-1 applies them to every edge and state of the model's
%% reachable graph; MC-2 applies them to the REAL core's conformance traces
%% (the post-states are the code's), so a violation there is a finding in the
%% code.
%%
%% Ghost state g(Base, K0, Eff): Base = values at the last re-basing observe
%% (or init), K0 = the id of the first record issued after it, Eff = the
%% version at ghost_init plus the effective transitions since (MC-1's `cap`
%% universe starts near VERSION_CAP).
%%   - An Ok observe re-bases (Base = new values, K0 = first_id + len), except
%%     the refresh (spec:58: the current values restated while a record is
%%     open), which keeps Base and K0 (OBS-2).
%%   - Fold-on-prune: a submit that prunes the front id F0 with K0 = F0 adds
%%     the pruned record's contribution to Base and moves K0 to F0 + 1, so
%%     K0 >= first_id always (checked by INV-LEDGER).
%%
%%   ghost_init(+S, -G)
%%   ghost_step(+G0, +S0, +Op, +Result, +S1, -G1)
%%   state_violation(+S, +G, -V)               enumerates violated invariants
%%   edge_violation(+S0, +Op, +R, +S1, -V)     enumerates violated step properties
%%   probe_violation(+S, +G, -V)               LIVE-1, LIVE-ALL, REFRESH-1, WIN-ESC
%%                                             on a state (they call step/4)
%%   live1_violation(+S, +G, -V)               LIVE-1 alone
%%   allowed_status_change(?From, ?To)          LIFE-1's status graph

:- module(props, [
    ghost_init/2,
    ghost_step/6,
    state_violation/3,
    edge_violation/5,
    probe_violation/3,
    live1_violation/3,
    allowed_status_change/2,
    base_violation/2,
    record_rem/2
]).

:- use_module(library(lists)).
:- use_module(library(iso_ext)).
:- use_module(library(between)).
:- use_module('../requirements/pool_req').
:- use_module('../model/pool_machine').
:- use_module(util).

ghost_init(S, g(V, K, N)) :- pool_values(S, V), pool_next_id(S, K), pool_version(S, N).

ghost_step(g(B, K, E), S0, Op, R, S1, G1) :-
    (   R \= err(_), S1 \== S0
    ->  E1 is E + 1,
        (   Op = observe(_, Vs, _)
        ->  (   pool_values(S0, V0), Vs == V0, has_open(S0)
            ->  G1 = g(B, K, E1)                                  % refresh (OBS-2)
            ;   pool_values(S1, B1), pool_next_id(S1, K1), G1 = g(B1, K1, E1)
            )
        ;   Op = submit(_, _, _, _, _, _)
        ->  pool_first_id(S1, F1),
            (   K < F1
            ->  fold_pruned(S0, K, F1, B, B1), G1 = g(B1, F1, E1)  % fold-on-prune
            ;   G1 = g(B, K, E1)
            )
        ;   G1 = g(B, K, E1)
        )
    ;   G1 = g(B, K, E)
    ).

%% fold_pruned(+S0, +K, +F1, +B, -B1): add the contributions of S0's records
%% with ids in [K, F1) (the ones the submit pruned) to Base.
fold_pruned(S0, K, F1, B, B1) :-
    pool_config(S0, config(_, _, _, _, Rcp)),
    findall(C, ( between(K, F1, Id), Id < F1, record_of(S0, Id, C) ), Cs),
    contribution(Cs, Rcp, B, B1).

has_open(S) :- pool_records(S, Rs), member(c(_, _, _, St), Rs), open_status(St), !.

%% base_violation(+S-G, -B): B_i = PV_i(Base).
base_violation(S-g(Base, _, _), B) :-
    pool_config(S, config(_, _, Ms, _, _)), point_violation(Ms, Base, B).

%% record_rem(+Status, -Rem): the version increments a record can still use
%% (INV-VERCAP): Reserved 3, Dispatched 3, Unknown 2, Active 1, terminal 0.
record_rem(reserved, 3).
record_rem(dispatched, 3).
record_rem(unknown, 2).
record_rem(active, 1).
record_rem(completed, 0).
record_rem(cancelled, 0).

%%% ========================================================
%%% State invariants
%%% ========================================================

state_violation(S, G, V) :-
    (   inv_wf(S, V)
    ;   win1(S, V)
    ;   inv_ledger(S, G, V)
    ;   inv_res(S, V)
    ;   inv_hard(S, G, V)
    ;   inv_dep(S, V)
    ;   inv_ver(S, G, V)
    ;   inv_vercap(S, V)
    ).

% INV-WF: values length n and bounded; record fields in range; one
% completed_units entry per recipe.
inv_wf(S, 'INV-WF'(values)) :-
    pool_config(S, config(_, _, Ms, _, _)), pool_values(S, Vs),
    \+ ( length(Ms, N), length(Vs, N), constant(max_value, M, _),
         forall(member(X, Vs), ( X >= -M, X =< M )) ).
inv_wf(S, 'INV-WF'(record(Id))) :-
    pool_config(S, C), pool_records(S, Rs), pool_first_id(S, F),
    nth0(J, Rs, c(RI, U, _, St)),
    \+ ( recipe_of(C, RI, r(_, _, _, _, MU, _)), U >= 1, U =< MU, status(St) ),
    Id is F + J.
inv_wf(S, 'INV-WF'(completed_units)) :-
    pool_config(S, config(_, _, _, _, Rcp)), pool_done(S, D), u32_max(M),
    \+ ( length(Rcp, N), length(D, N), forall(member(X, D), ( integer(X), X >= 0, X =< M )) ).

% WIN-1: at most window_size records (spec:9), first_id + len <= version
% (spec:27: every submit increments the version), and completed_units[r] is at
% least the retained Completed units of r (capped at u32::MAX), with equality
% while nothing was pruned (first_id = 0). The edge part (WIN-1 in
% edge_violation) makes it the saturated lifetime count by induction.
% (records[k].id = first_id + k holds by construction of record_of/3.)
win1(S, 'WIN-1'(too_many_records(N, W))) :-
    pool_records(S, Rs), length(Rs, N), window_size(W), N > W.
win1(S, 'WIN-1'(next_id_above_version(Next, Ver))) :-
    pool_next_id(S, Next), pool_version(S, Ver), Next > Ver.
win1(S, 'WIN-1'(first_id(F))) :- pool_first_id(S, F), \+ ( integer(F), F >= 0 ).
win1(S, 'WIN-1'(completed_units(R, D, Retained))) :-
    pool_config(S, config(_, _, _, _, Rcp)), pool_done(S, Ds), pool_first_id(S, F),
    nth0(R, Rcp, _), nth0(R, Ds, D),
    retained_completed(S, R, Retained0), u32_max(M), Retained is min(Retained0, M),
    (   D < Retained -> true
    ;   F =:= 0, D =\= Retained
    ).

retained_completed(S, R, N) :-
    pool_records(S, Rs),
    findall(U, member(c(R, U, _, completed), Rs), Us), sum_list(Us, N).

% INV-LEDGER: values = Base + sum over retained ids >= K0 of the record's
% contribution (reserve*u unless Cancelled, plus complete*u if Completed);
% every retained record below K0 is terminal (it was there at a re-basing
% observe, which needs all terminal); first_id <= K0 <= first_id + len.
inv_ledger(S, g(Base, K0, _), 'INV-LEDGER'(Diff)) :-
    pool_config(S, config(_, _, _, _, Rcp)), pool_values(S, Vs),
    findall(C, ( record_id(S, Id, C), Id >= K0 ), Cs),
    contribution(Cs, Rcp, Base, Expect),
    Vs \== Expect,
    Diff = expected(Expect, got(Vs)).
inv_ledger(S, g(_, K0, _), 'INV-LEDGER'(open_before_base(Id))) :-
    record_id(S, Id, c(_, _, _, St)), Id < K0, open_status(St).
inv_ledger(S, g(_, K0, _), 'INV-LEDGER'(ghost_outside_window(K0, F, Next))) :-
    pool_first_id(S, F), pool_next_id(S, Next),
    \+ ( K0 >= F, K0 =< Next ).

%% record_id(+S, -Id, -Rec): the retained records with their ids.
record_id(S, Id, Rec) :-
    pool_records(S, Rs), pool_first_id(S, F),
    nth0(J, Rs, Rec), Id is F + J.

contribution([], _, V, V).
contribution([c(RI, U, _, St) | Cs], Rcp, V0, V) :-
    nth0(RI, Rcp, r(_, _, Res, Com, _, _)),
    ( St \== cancelled -> vadd(V0, Res, U, V1) ; V1 = V0 ),
    ( St == completed  -> vadd(V1, Com, U, V2) ; V2 = V1 ),
    contribution(Cs, Rcp, V2, V).

vadd([], [], _, []).
vadd([A | As], [D | Ds], U, [B | Bs]) :- B is A + D * U, vadd(As, Ds, U, Bs).

% INV-RES: the release-aware envelope over the open records is computable (no
% endpoint leaves +-1e6) and EV_i = PV_i(values) for every metric (upstream
% findings.rs INV-RES). Ghost-free.
inv_res(S, V) :-
    pool_config(S, config(_, _, Ms, _, _)), pool_values(S, Vs),
    (   pool_envelope(S, Vs, none, none, Low, High)
    ->  interval_violation(Ms, Low, High, EV), point_violation(Ms, Vs, PV),
        nth0(I, EV, E), nth0(I, PV, P), E =\= P,
        V = 'INV-RES'(metric(I), ev(E), pv(P))
    ;   V = 'INV-RES'(envelope_overflow)
    ).

% INV-HARD: EV_i <= B_i (a corollary of INV-RES and PV_i(values) <= B_i).
% INV-HEALTHY (corollary): B = 0 => EV = 0 (every combination of outcomes
% lands in bounds). An uncomputable envelope is INV-RES's.
inv_hard(S, G, V) :-
    pool_config(S, config(_, _, Ms, _, _)), pool_values(S, Vs),
    base_violation(S-G, B),
    pool_envelope(S, Vs, none, none, Low, High),
    interval_violation(Ms, Low, High, EV),
    nth0(I, EV, E), nth0(I, B, Bi), E > Bi,
    (   forall(member(X, B), X =:= 0) -> V = 'INV-HEALTHY'(metric(I), ev(E))
    ;   V = 'INV-HARD'(metric(I), ev(E), base(Bi))
    ).

% INV-DEP: every retained record's prerequisites are met by completed_units
% (lifetime, so a pruned prerequisite still counts; handoff:19).
inv_dep(S, 'INV-DEP'(record(Id), p(P, U), completed(N))) :-
    pool_config(S, C),
    record_id(S, Id, c(RI, _, _, _)),
    recipe_of(C, RI, r(_, _, _, _, _, Req)),
    member(p(P, U), Req),
    completed_units(S, P, N), N < U.

% INV-VER: version = version at ghost_init + effective transitions.
inv_ver(S, g(_, _, E), 'INV-VER'(version(N), effective(E))) :- pool_version(S, N), N =\= E.

% INV-VERCAP: version + sum over open records of rem(status) <= u32::MAX, so
% the plain increment of ack, cancel and settle never overflows (spec:50).
inv_vercap(S, 'INV-VERCAP'(version(N), rem(Sum))) :-
    pool_version(S, N), pool_records(S, Rs),
    findall(X, ( member(c(_, _, _, St), Rs), record_rem(St, X) ), Xs), sum_list(Xs, Sum),
    u32_max(M), N + Sum > M.

%%% ========================================================
%%% Probes: properties of a state that call step/4
%%% ========================================================

probe_violation(S, G, V) :-
    (   live1_violation(S, G, V)
    ;   live_all_violation(S, V)
    ;   refresh_violation(S, V)
    ;   win_esc_violation(S, V)
    ).

% LIVE-1: settle(executor, id, true) is Ok for every Dispatched, Active or
% Unknown record. v1.1.0: no premise (a corollary of LIVE-ALL); v1.0.0 needed
% B = 0 and version < u32::MAX.
live1_violation(S, _, 'LIVE-1'(record(Id), Result)) :-
    pool_config(S, C),
    record_id(S, Id, c(RI, _, _, St)), memberchk(St, [dispatched, active, unknown]),
    executor_of(C, RI, X),
    step(S, settle(X, Id, true), Result, _),
    Result \== ok.

% LIVE-ALL (upstream findings.rs assert_live): for every open record, if
% Reserved its cancel by the actor and by the owner is Ok; if in flight the
% executor's settle(true) and settle(false) are Ok; none raises a metric's
% violation. No premise on the base or the version.
live_all_violation(S, 'LIVE-ALL'(record(Id), Op, Why)) :-
    pool_config(S, C), pool_observed_at(S, Now),
    record_id(S, Id, c(RI, _, _, St)), open_status(St),
    (   St == reserved
    ->  owner_of(C, O), actor_of(C, RI, A), member(Ca, [A, O]), Op = cancel(Ca, Id, Now)
    ;   executor_of(C, RI, X), member(B, [true, false]), Op = settle(X, Id, B)
    ),
    step(S, Op, R, S1),
    (   R \== ok -> Why = R
    ;   raised_metric(S, S1, I), Why = raised(metric(I))
    ).

raised_metric(S0, S1, I) :-
    pool_config(S0, config(_, _, Ms, _, _)),
    pool_values(S0, V0), pool_values(S1, V1),
    point_violation(Ms, V0, P0), point_violation(Ms, V1, P1),
    nth0(I, P1, X1), nth0(I, P0, X0), X1 > X0.

% REFRESH-1: below VERSION_CAP the owner's observe of the current values is
% Ok, open records included (spec:58; F-6 fixed).
refresh_violation(S, 'REFRESH-1'(Result)) :-
    pool_version(S, N), version_cap(Cap), N < Cap,
    pool_config(S, C), owner_of(C, O), pool_values(S, Vs), pool_observed_at(S, Now),
    step(S, observe(O, Vs, Now), Result, _),
    Result \== ok.

% WIN-ESC: a valid submit fails Full exactly when window_size records are
% retained and the oldest is open (spec:21); from such a state the front's
% resolver (cancel by its actor if Reserved, the executor's settle(false) if
% in flight) makes it terminal, after which a valid submit does not fail Full.
win_esc_violation(S, V) :-
    pool_paused(S, false),
    probe_submit(S, Op), step(S, Op, R, _),
    (   full_front_open(S)
    ->  R \== err(full), V = 'WIN-ESC'(open_front_not_full(R))
    ;   R == err(full), V = 'WIN-ESC'(full_without_open_front)
    ).
win_esc_violation(S, V) :-
    full_front_open(S),
    pool_config(S, C), pool_first_id(S, F), pool_observed_at(S, Now),
    record_of(S, F, c(RI, _, _, St)),
    (   St == reserved -> actor_of(C, RI, A), Res = cancel(A, F, Now)
    ;   executor_of(C, RI, X), Res = settle(X, F, false)
    ),
    step(S, Res, R1, S1),
    (   R1 \== ok -> V = 'WIN-ESC'(front_not_resolved(Res, R1))
    ;   probe_submit(S1, Op), step(S1, Op, R2, _), R2 == err(full),
        V = 'WIN-ESC'(full_after_resolving(Res))
    ).

full_front_open(S) :-
    pool_records(S, Rs), length(Rs, N), window_size(W), N >= W,
    Rs = [c(_, _, _, St) | _], open_status(St).

%% probe_submit(+S, -Op): one unit of recipe 0 by its actor, at the current
%% version, fresh (now = observed_at) and expiring one second later.
probe_submit(S, submit(A, 0, 1, Ver, Exp, Now)) :-
    pool_config(S, C), actor_of(C, 0, A),
    pool_version(S, Ver), pool_observed_at(S, Now), Exp is Now + 1.

%%% ========================================================
%%% Step properties
%%% ========================================================

edge_violation(S0, Op, R, S1, V) :-
    (   R = err(_), S1 \== S0, V = 'TX-0'(state_changed)
    ;   R == ok, classify(S0, Op, stop(noop)), S1 \== S0, V = 'TX-NOOP'(state_changed)
    ;   is_ok(R), \+ classify(S0, Op, stop(noop)),
        pool_version(S0, N0), pool_version(S1, N1), N1 =\= N0 + 1,
        V = 'TX-VER'(N0, N1)
    ;   is_ok(R), S1 \== S0, \+ authorized(S0, Op), V = 'AUTH-1'(caller)
    ;   is_ok(R), Op = submit(_, _, _, Ver, _, _), pool_version(S0, N0), Ver =\= N0,
        V = 'STALE-1'(request(Ver), version(N0))
    ;   is_ok(R), Op = submit(_, _, _, _, _, _), adm_violation(S0, Op, S1, V)
    ;   is_ok(R), Op \= observe(_, _, _), hard_step_violation(S0, S1, V)
    ;   is_ok(R), Op = submit(_, RI, _, _, _, _), dep_violation(S0, RI, V)
    ;   life_violation(S0, Op, R, S1, V)
    ;   win1_edge_violation(S0, Op, R, S1, V)
    ;   win2_violation(S0, Op, R, S1, V)
    ;   Op = ack(_, _), pool_values(S0, V0), pool_values(S1, V1), V0 \== V1, V = 'ASYNC-1'(values_changed)
    ;   term_violation(S0, Op, R, S1, V)
    ;   obs_violation(S0, Op, R, S1, V)
    ;   is_ok(R), S1 \== S0, pool_config(S0, C0), pool_config(S1, C1), C0 \== C1, V = 'OBS-1'(config_changed)
    ;   pool_paused(S0, true), is_ok(R), S1 \== S0,
        ( Op = submit(_, _, _, _, _, _) ; Op = dispatch(_, _, _) ), V = 'PAUSE-1'(effect_while_paused)
    ;   vercap_violation(S0, Op, R, S1, V)
    ).

is_ok(ok).
is_ok(ok(_)).

% AUTH-1: effective submit/dispatch => actor; cancel => actor or owner; settle =>
% executor; pause/observe => owner. Acknowledge has no caller (private callback).
authorized(S, submit(Ca, RI, _, _, _, _)) :- pool_config(S, C), actor_of(C, RI, A), Ca == A.
authorized(S, dispatch(Ca, Id, _)) :-
    pool_config(S, C), record_of(S, Id, c(RI, _, _, _)), actor_of(C, RI, A), Ca == A.
authorized(S, cancel(Ca, Id, _)) :-
    pool_config(S, C), record_of(S, Id, c(RI, _, _, _)), actor_of(C, RI, A), owner_of(C, O),
    ( Ca == A ; Ca == O ), !.
authorized(S, settle(Ca, Id, _)) :-
    pool_config(S, C), record_of(S, Id, c(RI, _, _, _)), executor_of(C, RI, X), Ca == X.
authorized(S, pause(Ca, _)) :- pool_config(S, C), owner_of(C, O), Ca == O.
authorized(S, observe(Ca, _, _)) :- pool_config(S, C), owner_of(C, O), Ca == O.
authorized(_, ack(_, _)).

% ADM-1: energy(post) <= energy(pre); ADM-2: in a violating state,
% energy(post + completion) < energy(pre).
adm_violation(S0, submit(_, RI, U, _, _, _), S1, V) :-
    pool_config(S0, C), pool_values(S0, V0), pool_values(S1, V1),
    energy(C, V0, E0), energy(C, V1, E1),
    (   E1 > E0, V = 'ADM-1'(E0, E1)
    ;   violating(C, V0), recipe_of(C, RI, r(_, _, _, Com, _, _)),
        vadd(V1, Com, U, VA), energy(C, VA, EA), EA >= E0, V = 'ADM-2'(E0, EA)
    ).

% HARD-STEP: every Ok except observe: PV_i(post) <= PV_i(pre).
hard_step_violation(S0, S1, 'HARD-STEP'(metric(I), P0, P1)) :-
    pool_config(S0, config(_, _, Ms, _, _)),
    pool_values(S0, V0), pool_values(S1, V1),
    point_violation(Ms, V0, PV0), point_violation(Ms, V1, PV1),
    nth0(I, PV1, P1), nth0(I, PV0, P0), P1 > P0.

% DEP-1: submit Ok => prerequisites met by completed_units in the pre-state.
dep_violation(S0, RI, 'DEP-1'(p(P, U), completed(N))) :-
    pool_config(S0, C), recipe_of(C, RI, r(_, _, _, _, _, Req)),
    member(p(P, U), Req), completed_units(S0, P, N), N < U.

% LIFE-1: ids issued in order (one Reserved record per Ok submit, at the next
% id), immutable recipe/units/expires_at, status changes only along the graph,
% at most one record changes per call, terminal absorbing. Records are
% compared by id; a submit may drop the front (WIN-2 checks it was terminal).
allowed_status_change(reserved, dispatched).
allowed_status_change(reserved, cancelled).
allowed_status_change(dispatched, active).
allowed_status_change(dispatched, unknown).
allowed_status_change(unknown, active).
allowed_status_change(S, completed) :- memberchk(S, [dispatched, active, unknown]).
allowed_status_change(S, cancelled) :- memberchk(S, [dispatched, active, unknown]).

life_violation(S0, Op, R, S1, V) :-
    pool_first_id(S0, F0), pool_next_id(S0, X0),
    pool_first_id(S1, F1), pool_next_id(S1, X1),
    (   Op = submit(_, _, _, _, _, _), R = ok(_)
    ->  (   X1 =\= X0 + 1 -> V = 'LIFE-1'(not_appended(X0, X1))
        ;   F1 < F0 -> V = 'LIFE-1'(first_id_decreased(F0, F1))
        ;   \+ record_of(S1, X0, c(_, _, _, reserved)) -> V = 'LIFE-1'(new_not_reserved(X0))
        ;   changed(S0, S1, F1, X0, V)
        )
    ;   (   ( F1 =\= F0 ; X1 =\= X0 ) -> V = 'LIFE-1'(window_changed(F0-X0, F1-X1))
        ;   changed(S0, S1, F0, X0, V)
        )
    ).

%% changed(+S0, +S1, +From, +To, -V): compare the records with ids in [From, To).
changed(S0, S1, From, To, V) :-
    (   between(From, To, Id), Id < To,
        record_of(S0, Id, c(A, B, C, _)), record_of(S1, Id, c(A1, B1, C1, _)),
        ( A \== A1 ; B \== B1 ; C \== C1 ), V = 'LIFE-1'(immutable_field(Id))
    ;   between(From, To, Id), Id < To,
        record_of(S0, Id, c(_, _, _, X)), record_of(S1, Id, c(_, _, _, Y)),
        X \== Y, \+ allowed_status_change(X, Y), V = 'LIFE-1'(transition(Id, X, Y))
    ;   findall(Id, ( between(From, To, Id), Id < To,
                      record_of(S0, Id, c(_, _, _, X)), record_of(S1, Id, c(_, _, _, Y)), X \== Y ), Ids),
        length(Ids, K), K > 1, V = 'LIFE-1'(several_changed(Ids))
    ).

% WIN-1 (edge): an Ok submit returns id first_id + len (spec:27) and stores
% the new record under it; first_id moves by one exactly when the window was
% full; completed_units changes only by an effective settle(true), which adds
% the record's units to its recipe's entry, saturating at u32::MAX (lib.rs:519).
win1_edge_violation(S0, submit(_, RI, U, _, Exp, _), ok(Id), S1, V) :-
    pool_next_id(S0, X0), pool_first_id(S0, F0), pool_first_id(S1, F1),
    pool_records(S0, Rs0), length(Rs0, L0), window_size(W),
    (   Id =\= X0 -> V = 'WIN-1'(submit_id(Id, expected(X0)))
    ;   \+ record_of(S1, Id, c(RI, U, Exp, reserved)) -> V = 'WIN-1'(record_not_at_id(Id))
    ;   ( L0 >= W -> F1 =\= F0 + 1 ; F1 =\= F0 ) -> V = 'WIN-1'(first_id(F0, F1, len(L0)))
    ).
win1_edge_violation(S0, Op, R, S1, V) :-
    pool_done(S0, D0), pool_done(S1, D1),
    (   Op = settle(_, Id, true), R == ok, S1 \== S0
    ->  record_of(S0, Id, c(RI, U, _, _)), u32_max(M),
        nth0(RI, D0, X0, Rest), X1 is min(X0 + U, M), nth0(RI, Expect, X1, Rest),
        D1 \== Expect, V = 'WIN-1'(completed_units(D0, D1, expected(Expect)))
    ;   D1 \== D0, V = 'WIN-1'(completed_units_changed(D0, D1))
    ).

% WIN-2: a pruned record was terminal when the submit pruned it; an id below
% first_id is NotFound for dispatch, acknowledge, cancel and settle, which
% change nothing (spec:27 "A pruned id is NotFound for every call").
win2_violation(S0, submit(_, _, _, _, _, _), ok(_), S1, 'WIN-2'(pruned_open(Id, St))) :-
    pool_first_id(S0, F0), pool_first_id(S1, F1),
    between(F0, F1, Id), Id < F1,
    record_of(S0, Id, c(_, _, _, St)), open_status(St).
win2_violation(S0, Op, R, S1, 'WIN-2'(pruned_id(Id), R)) :-
    op_record_id(Op, Id), pool_first_id(S0, F0), Id < F0,
    \+ ( R == err(not_found), S1 == S0 ).

op_record_id(dispatch(_, Id, _), Id).
op_record_id(ack(Id, _), Id).
op_record_id(cancel(_, Id, _), Id).
op_record_id(settle(_, Id, _), Id).

% TERM-1: by the executor, the same terminal again is an Ok no-op; the opposite
% terminal is InvalidState. By an authorized caller, cancel of a Cancelled
% record is an Ok no-op and cancel of a Completed one is InvalidState (spec:43;
% v1.0.0: Pending).
term_violation(S0, settle(Ca, Id, Done), R, S1, V) :-
    pool_config(S0, C), record_of(S0, Id, c(RI, _, _, St)), executor_of(C, RI, X), Ca == X,
    (   ( Done == true, St == completed ; Done == false, St == cancelled )
    ->  \+ ( R == ok, S1 == S0 ), V = 'TERM-1'(repeat_not_noop(St), R)
    ;   ( Done == true, St == cancelled ; Done == false, St == completed )
    ->  R \== err(invalid_state), V = 'TERM-1'(conflict_not_rejected(St), R)
    ).
term_violation(S0, cancel(Ca, Id, Now), R, S1, V) :-
    authorized(S0, cancel(Ca, Id, Now)),
    record_of(S0, Id, c(_, _, _, St)),
    (   St == cancelled
    ->  \+ ( R == ok, S1 == S0 ), V = 'TERM-1'(cancel_repeat_not_noop, R)
    ;   St == completed
    ->  R \== err(invalid_state), V = 'TERM-1'(cancel_completed_not_rejected, R)
    ).

% OBS-1: an Ok observe with a record open restates the values (the refresh);
% OBS-2: it then changes only observed_at and the version (and the ghost keeps
% its base, see ghost_step/6).
obs_violation(S0, observe(_, Vs, _), ok, S1, V) :-
    has_open(S0), pool_values(S0, V0),
    (   Vs \== V0 -> V = 'OBS-1'(open_record_values_changed)
    ;   S0 = pool(C, X, _, _, P, R, F, D),
        \+ S1 = pool(C, X, _, _, P, R, F, D), V = 'OBS-2'(refresh_changed_more)
    ).

% VER-CAP: at version >= VERSION_CAP, submit, dispatch, set_paused and observe
% are not Ok, and change nothing; one whose other checks all pass fails
% Arithmetic (spec:50: the cap is candidate's last check and bump_new's
% first). "Its other checks pass" is read off the same call one below the cap
% (only the cap and the request-version equality read the version there).
vercap_violation(S0, Op, R, S1, V) :-
    op_guard_kind(Op, Kind), memberchk(Kind, [submit, dispatch, pause, observe]),
    pool_version(S0, N), version_cap(Cap), N >= Cap,
    (   is_ok(R) -> V = 'VER-CAP'(gated_ok(Kind, N))
    ;   S1 \== S0 -> V = 'VER-CAP'(state_changed(Kind))
    ;   R \== err(arithmetic),
        N1 is Cap - 1,
        S0 = pool(C, Vs, _, O, P, Rs, F, D),
        below_cap_op(Op, N, N1, Op1),
        classify(pool(C, Vs, N1, O, P, Rs, F, D), Op1, effect),
        V = 'VER-CAP'(not_arithmetic(Kind, R))
    ).

%% below_cap_op(+Op, +N, +N1, -Op1): the same call against version N1 (a
%% submit at the current version stays current, a stale one stays stale).
below_cap_op(submit(Ca, RI, U, Ver, E, Now), N, N1, submit(Ca, RI, U, Ver1, E, Now)) :- !,
    ( Ver =:= N -> Ver1 = N1 ; Ver1 is N1 + 1 ).
below_cap_op(Op, _, _, Op).
