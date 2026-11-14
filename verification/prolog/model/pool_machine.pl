%% pool_machine.pl -- Reference semantics step/4 of pool-core (shared-pool-controls v1.1.0)
%%
%% PROP: MC-1 MC-2 C-5 VER-CAP WIN-2 WIN-ESC
%%
%% Written from docs/spec.md and docs/api.md (v1.1.0, cited per clause). The
%% guard ORDER is not restated here: step/4 walks guard/5 of
%% requirements/pool_req.pl, so the model's behaviour is the requirement table,
%% and MC-2 checks it against the real core. Where the docs are silent the
%% clause follows the code and is tagged CODE-ONLY. v1.1.0 has no open
%% spec/code divergence (pool_req.pl closed_divergence/5 lists v1.0.0's).
%%
%% Values are exact integers; u32/u64 widths appear only where the code can
%% reach them (version, completed_units saturation, saturating time sums).
%%
%% Config (conformance trace format):
%%   config(Owner, MaxAge, Metrics, Terms, Recipes)
%%     Metrics = [m(Min, Max, Initial)..]
%%     Terms   = [t(Metric, Target, Weight, Shape)..]   Shape: target | excess | deficit
%%     Recipes = [r(Actor, Executor, Reserve, Complete, MaxUnits, [p(Recipe, Units)..])..]
%% State (the trace contract's s/7 after the config, spec:7):
%%   pool(Config, Values, Version, ObservedAt, Paused, Records, FirstId, Done)
%%     Records = [c(Recipe, Units, ExpiresAt, Status)..], oldest first; record j
%%               has id FirstId + j (spec:9 "records[k].id = first_id + k")
%%     Done    = [completed_units of recipe r..] (spec:9; saturating u32, lib.rs:519)
%% Ops (trace format):
%%   submit(Caller, Recipe, Units, Version, ExpiresAt, Now)
%%   dispatch(Caller, Id, Now)    ack(Id, Accepted)        cancel(Caller, Id, Now)
%%   settle(Caller, Id, Completed) pause(Caller, Paused)   observe(Caller, Values, Now)
%% Result: ok | ok(Id) | err(snake_case_error)
%%
%% Window size. The code's window is MAX_RECORDS = 256 (constant(max_records)).
%% MC-1 explores scaled universes with a window of 1 or 2: window_size/1 is
%% that parameter and VERSION_CAP scales with it (version_cap/1 = u32::MAX -
%% 3 * window, lib.rs:24). It is 256 unless a goal runs under
%% with_window_size/2, which restores it on exit, failure or exception
%% (setup_call_cleanup), so a scaled MC-1 run cannot leak into C-2, MC-2 or the
%% reads (tests/test_mc1.pl checks the default after a scaled run).

:- module(pool_machine, [
    valid_config/1,
    init_state/3,
    step/4,
    classify/3,
    op_guard_kind/2,
    to_trace_state/2,
    pool_config/2,
    pool_values/2,
    pool_version/2,
    pool_observed_at/2,
    pool_records/2,
    pool_paused/2,
    pool_first_id/2,
    pool_done/2,
    pool_next_id/2,
    recipe_of/3,
    record_of/3,
    actor_of/3,
    executor_of/3,
    owner_of/2,
    add_vec/4,
    sub_vec/4,
    envelope/7,
    envelope_opts/8,
    pool_envelope/6,
    hull_term/3,
    point_violation/3,
    interval_violation/4,
    violating/2,
    energy/3,
    completed_units/3,
    admissible/3,
    admissible_opts/4,
    preview_allowed/3,
    best_quantity/4,
    u32_max/1,
    u64_max/1,
    model_constant/3,
    window_size/1,
    with_window_size/2,
    version_cap/1
]).

:- use_module(library(lists)).
:- use_module(library(between)).
:- use_module(library(iso_ext)).
:- use_module('../requirements/pool_req').

:- discontiguous(holds/4).
:- dynamic(window_override/1).
:- meta_predicate(with_window_size(+, 0)).

u32_max(M) :- model_constant(version_width, W, _), M is 2^W - 1.
u64_max(M) :- model_constant(time_width, W, _), M is 2^W - 1.

%% model_constant(Name, Value, Basis): the bounds this model uses that the docs
%% do not state (CODE-ONLY). C-4 compares each with extracted/code_facts.pl.
model_constant(max_metrics, 32,  code_only('docs silent; core MAX_METRICS')).
model_constant(max_recipes, 32,  code_only('docs silent; core MAX_RECIPES')).
model_constant(max_units,   100, code_only('docs silent; core MAX_UNITS')).
model_constant(max_requires, 32, code_only('docs silent; validate bounds requires by MAX_RECIPES')).
model_constant(version_width, 32, code_only('spec:50 names u32::MAX; the width u32 is the code\'s')).
model_constant(time_width,    64, code_only('saturating u64 time sums (expiry, freshness)')).

%% window_size(-W): the record window (spec:9 "at most 256 records"); a scaled
%% MC-1 universe sets it with with_window_size/2.
window_size(W) :- ( window_override(W0) -> W = W0 ; constant(max_records, W, _) ).

%% with_window_size(+W, :Goal): run Goal once with window W; the previous
%% window is back after Goal succeeds, fails or throws.
with_window_size(W, Goal) :-
    integer(W), W >= 1,
    setup_call_cleanup(asserta(window_override(W)), once(Goal), retract(window_override(W))).

%% version_cap(-Cap): spec:50 "VERSION_CAP = u32::MAX - 768" = u32::MAX - 3 *
%% the window (lib.rs:24 `u32::MAX - 3 * MAX_RECORDS`). At the default window
%% this is constant(version_cap) (test_model checks it).
version_cap(Cap) :- window_size(W), u32_max(M), Cap is M - 3 * W.

max_value(V) :- constant(max_value, V, _).

bounded(X) :- max_value(M), X >= -M, X =< M.

sat_add64(A, B, C) :- u64_max(U), C is min(A + B, U).

%%% ========================================================
%%% Accessors
%%% ========================================================

pool_config(pool(C, _, _, _, _, _, _, _), C).
pool_values(pool(_, V, _, _, _, _, _, _), V).
pool_version(pool(_, _, N, _, _, _, _, _), N).
pool_observed_at(pool(_, _, _, O, _, _, _, _), O).
pool_paused(pool(_, _, _, _, P, _, _, _), P).
pool_records(pool(_, _, _, _, _, R, _, _), R).
pool_first_id(pool(_, _, _, _, _, _, F, _), F).
pool_done(pool(_, _, _, _, _, _, _, D), D).
%% pool_next_id(+S, -Next): first_id + len(records), the id the next submit gets.
pool_next_id(pool(_, _, _, _, _, R, F, _), N) :- length(R, L), N is F + L.

owner_of(config(O, _, _, _, _), O).
recipe_of(config(_, _, _, _, Rs), I, R) :- integer(I), I >= 0, nth0(I, Rs, R).
actor_of(Cfg, I, A) :- recipe_of(Cfg, I, r(A, _, _, _, _, _)).
executor_of(Cfg, I, X) :- recipe_of(Cfg, I, r(_, X, _, _, _, _)).

%% record_of(+S, +Id, -Rec): the retained record with this id (spec:9, 27; a
%% pruned id, below first_id, or one not issued yet has none: NotFound).
record_of(pool(_, _, _, _, _, Rs, F, _), Id, Rec) :-
    integer(Id), Id >= F, K is Id - F, nth0(K, Rs, Rec).

to_trace_state(pool(_, V, N, O, P, R, F, D), s(V, N, O, P, R, F, D)).

%%% ========================================================
%%% Configuration (spec:11, spec:15; api:65; handoff:11)
%%% ========================================================
%% Numeric part of Config::validate. The code also checks name/unit/owner
%% string lengths (1..128): CODE-ONLY and not representable in trace configs.

valid_config(config(Owner, MaxAge, Ms, Ts, Rs)) :-
    atom(Owner), integer(MaxAge), MaxAge >= 0,
    model_constant(max_metrics, MaxM, _), model_constant(max_recipes, MaxR, _),
    length(Ms, N), N >= 1, N =< MaxM,                   % CODE-ONLY: MAX_METRICS
    length(Rs, NR), NR >= 1, NR =< MaxR,                 % CODE-ONLY: MAX_RECIPES
    length(Ts, NT), NT >= 1, constant(max_terms, MT, _), NT =< MT,   % spec:15
    forall(member(m(Lo, Hi, I), Ms),
           ( bounded(Lo), bounded(Hi), Lo =< Hi, I >= Lo, I =< Hi )),      % spec:15
    constant(max_weight, MW, _),
    forall(member(t(K, Tg, W, Sh), Ts),
           ( integer(K), K >= 0, K < N, bounded(Tg), W >= 1, W =< MW,        % spec:15, 33
             member(Sh, [target, excess, deficit]) )),
    valid_recipes(Rs, 0, N).

valid_recipes([], _, _).
valid_recipes([r(A, X, Res, Com, MU, Req) | Rs], I, N) :-
    % spec:11 "one exact executor account distinct from the actor"; api:65
    % (v1.1.0 lib.rs:130 `r.actor == r.executor` is InvalidConfig; F-4 fixed).
    % The owner may be a recipe's actor or executor (amb-owner-roles).
    atom(A), atom(X), A \== X,
    length(Res, N), length(Com, N),                      % spec:11 effects span all metrics
    forall(member(D, Res), bounded(D)), forall(member(D, Com), bounded(D)),  % spec:15
    model_constant(max_units, MaxU, _), model_constant(max_requires, MaxQ, _),
    MU >= 1, MU =< MaxU,                                 % CODE-ONLY: MAX_UNITS
    length(Req, NQ), NQ =< MaxQ,
    forall(member(p(P, U), Req), ( P >= 0, P < I, U >= 1, U =< MaxU )),  % spec:11 earlier recipes
    findall(P, member(p(P, _), Req), Ps), sort(Ps, Ss), length(Ss, NQ),  % CODE-ONLY: distinct ids
    I1 is I + 1,
    valid_recipes(Rs, I1, N).

%% init_state(+Config, +Now, -Pool): Pool::new (spec:7; handoff:25 "Config validates
%% initial values within bounds"). Version 0, observed_at = Now, unpaused, no
%% records, first_id 0, completed_units 0 per recipe (lib.rs:154-167).
init_state(Cfg, Now, pool(Cfg, Vs, 0, Now, false, [], 0, Done)) :-
    valid_config(Cfg),
    Cfg = config(_, _, Ms, _, Rs),
    findall(I, member(m(_, _, I), Ms), Vs),
    findall(0, member(_, Rs), Done).

%%% ========================================================
%%% Arithmetic (spec:15, 21, 23, 25)
%%% ========================================================

%% add_vec(+Base, +Delta, +Units, -Out): checked, fails if any component leaves
%% [-1e6, 1e6] (spec:15 "Every update is checked"; NUM-2).
add_vec([], [], _, []).
add_vec([B | Bs], [D | Ds], U, [V | Vs]) :-
    V is B + D * U, bounded(V),
    add_vec(Bs, Ds, U, Vs).

%% sub_vec(+Base, +Delta, +Units, -Out): the checked inverse (spec:43-44).
sub_vec(Bs, Ds, U, Vs) :- findall(N, ( member(D, Ds), N is -D ), Ns), add_vec(Bs, Ns, U, Vs).

%% point_violation(+Metrics, +Values, -PV): PV_i = max(0, min-v) + max(0, v-max).
point_violation([], [], []).
point_violation([m(Lo, Hi, _) | Ms], [V | Vs], [P | Ps]) :-
    P is max(0, Lo - V) + max(0, V - Hi),
    point_violation(Ms, Vs, Ps).

%% interval_violation(+Metrics, +Low, +High, -EV)   (AMBIGUOUS amb-interval-violation)
interval_violation([], [], [], []).
interval_violation([m(Lo, Hi, _) | Ms], [L | Ls], [H | Hs], [E | Es]) :-
    E is max(0, Lo - L) + max(0, H - Hi),
    interval_violation(Ms, Ls, Hs, Es).

violating(Cfg, Values) :-
    Cfg = config(_, _, Ms, _, _),
    point_violation(Ms, Values, PV),
    member(P, PV), P > 0, !.

%% The release-aware envelope (spec:23): per metric an interval containing
%% every outcome of every open record, plus the proposed one. Each record may
%% stay open, complete (add d = complete*units) or be released (add
%% x = -reserve*units), independently: low += min(d, x, 0), high += max(d, x, 0)
%% (lib.rs:206-244). Extra = none | extra(Recipe, Units); Skip = none | Id (the
%% record being settled or cancelled is left out). Fails (Arithmetic) if an
%% endpoint leaves [-1e6, 1e6]. CODE-ONLY: the code checks after every record;
%% low only falls and high only rises, so that is the same as checking the
%% final endpoints.
%%
%% envelope/7 and envelope_opts/8 take a record list whose first record has id
%% 0 (C-5's callers pass Skip = none); pool_envelope/6 takes the pool's window.
envelope(Cfg, Recs, Values, Extra, Skip, Low, High) :-
    envelope_w(Cfg, 0, Recs, Values, Extra, Skip, [], Low, High).

%% envelope_opts/8: Opts = [ignore_pending_negative] replaces each OPEN
%% record's completion delta d by max(0, d) (used only by C-5's RISK-HEADROOM
%% check: high = max(d, x, 0) is then unchanged).
envelope_opts(Cfg, Recs, Values, Extra, Skip, Opts, Low, High) :-
    envelope_w(Cfg, 0, Recs, Values, Extra, Skip, Opts, Low, High).

pool_envelope(pool(C, _, _, _, _, Recs, F, _), Values, Extra, Skip, Low, High) :-
    envelope_w(C, F, Recs, Values, Extra, Skip, [], Low, High).

envelope_w(Cfg, F, Recs, Values, Extra, Skip, Opts, Low, High) :-
    Cfg = config(_, _, _, _, Rs),
    open_terms(Recs, F, Rs, Skip, Ts0),
    (   memberchk(ignore_pending_negative, Opts)
    ->  findall(h(P, X), ( member(h(D, X), Ts0), maplist(pos_part, D, P) ), Ts1)
    ;   Ts1 = Ts0
    ),
    (   Extra = extra(RI, U)
    ->  nth0(RI, Rs, R), hull_term(R, U, T), append(Ts1, [T], Ts)
    ;   Ts = Ts1
    ),
    sum_hull(Ts, Values, Low, High),
    maplist(bounded, Low), maplist(bounded, High).

%% open_terms(+Records, +Id, +Recipes, +Skip, -Terms): h(D, X) per open record
%% other than Skip; Id is the first record's id.
open_terms([], _, _, _, []).
open_terms([c(RI, U, _, St) | Cs], Id, Rs, Skip, Out) :-
    Id1 is Id + 1,
    (   open_status(St), Skip \== Id
    ->  nth0(RI, Rs, R), hull_term(R, U, T), Out = [T | Out1]
    ;   Out = Out1
    ),
    open_terms(Cs, Id1, Rs, Skip, Out1).

%% hull_term(+Recipe, +Units, -h(D, X)): completion delta D = complete*units
%% and release delta X = -reserve*units (lib.rs:216-222).
hull_term(r(_, _, Res, Com, _, _), U, h(D, X)) :-
    scale(Com, U, D), scale(Res, U, R), maplist(neg, R, X).

scale(Ds, U, Xs) :- findall(X, ( member(D, Ds), X is D * U ), Xs).
pos_part(X, Y) :- Y is max(0, X).
neg(X, Y) :- Y is -X.

sum_hull([], V, V, V).
sum_hull([h(D, X) | Ts], V, Low, High) :-
    sum_hull(Ts, V, L0, H0),
    add_hull(D, X, L0, H0, Low, High).
add_hull([], [], [], [], [], []).
add_hull([D | Ds], [X | Xs], [L | Ls], [H | Hs], [L1 | Ls1], [H1 | Hs1]) :-
    L1 is L + min(0, min(D, X)), H1 is H + max(0, max(D, X)),
    add_hull(Ds, Xs, Ls, Hs, Ls1, Hs1).

%% hard_ok(+Cfg, +Current, +Candidate, +Low, +High): spec:23 "No metric's
%% interval violation may exceed its violation before the request or its
%% violation after the reservation" (lib.rs:258-263 `v > old[i].min(new[i])`).
%% Current = the pool's values before the call; Candidate = the values the call
%% would set (submit: after the reservation; release: after the inverse;
%% settle(true): after the completion).
hard_ok(Cfg, Current, Cand, Low, High) :-
    Cfg = config(_, _, Ms, _, _),
    point_violation(Ms, Current, PV),
    point_violation(Ms, Cand, PN),
    interval_violation(Ms, Low, High, EV),
    \+ ( nth0(I, EV, E), nth0(I, PV, P), nth0(I, PN, Q), E > min(P, Q) ).

%% energy(+Cfg, +Values, -E): spec:33, sum of w * r^2 with r the target
%% deviation |v - t|, excess max(0, v - t) or deficit max(0, t - v).
energy(config(_, _, _, Ts, _), Vs, E) :- energy_(Ts, Vs, 0, E).
energy_([], _, E, E).
energy_([t(K, Tg, W, Sh) | Ts], Vs, E0, E) :-
    nth0(K, Vs, V), residual(Sh, V, Tg, R),
    E1 is E0 + W * R * R,
    energy_(Ts, Vs, E1, E).
residual(target,  V, T, R) :- R is abs(V - T).
residual(excess,  V, T, R) :- R is max(0, V - T).
residual(deficit, V, T, R) :- R is max(0, T - V).

%% completed_units(+S, +Recipe, -N): completed_units[recipe], the lifetime
%% Completed units, pruned records included (spec:9; handoff:19).
completed_units(S, P, N) :- pool_done(S, D), nth0(P, D, N).

%%% ========================================================
%%% step/4
%%% ========================================================

op_guard_kind(submit(_, _, _, _, _, _), submit).
op_guard_kind(dispatch(_, _, _),        dispatch).
op_guard_kind(ack(_, _),                ack).
op_guard_kind(cancel(_, _, _),          cancel).
op_guard_kind(settle(_, _, true),       settle_completed).
op_guard_kind(settle(_, _, false),      settle_cancelled).
op_guard_kind(pause(_, _),              pause).
op_guard_kind(observe(_, _, _),         observe).

%% step(+S0, +Op, -Result, -S1). A rejected or no-op call leaves the state
%% unchanged (spec:27 "On rejection return an error with no state change";
%% handoff:15; spec:41, 45-46 Ok without a version change).
step(S0, Op, Result, S1) :-
    op_guard_kind(Op, Kind),
    guard_ids(Kind, Ids),
    (   first_stop(Ids, Kind, S0, Op, Outcome)
    ->  S1 = S0,
        (   Outcome = err(E) -> camel_snake(E, Sn), Result = err(Sn)
        ;   Result = ok
        )
    ;   effect(Kind, S0, Op, Result, S1)
    ).

%% classify(+S0, +Op, -Class): stop(err(Error)) | stop(noop) | effect -- which
%% guard decides the call (used by the step properties: TX-NOOP, TX-VER).
classify(S0, Op, Class) :-
    op_guard_kind(Op, Kind),
    guard_ids(Kind, Ids),
    (   first_stop(Ids, Kind, S0, Op, Outcome) -> Class = stop(Outcome) ; Class = effect ).

first_stop([G | Gs], Kind, S, Op, Outcome) :-
    (   holds(Kind, G, S, Op) -> first_stop(Gs, Kind, S, Op, Outcome)
    ;   once(guard(Kind, _, G, Outcome, _))
    ).

%% holds(+Kind, +GuardId, +S, +Op): the guard passes. A guard may assume that
%% every guard before it in guard/5 order passed. An id with no clause fails,
%% so its guard fires: every guard/5 id needs a clause here.

% --- shared
% spec:50: ack, cancel and settle keep the plain checked increment (bump).
holds(_, version_overflow, S, _) :- pool_version(S, N), u32_max(M), N < M.
% spec:50: submit (last), dispatch, set_paused, observe stop at VERSION_CAP (bump_new).
holds(_, version_cap, S, _) :- pool_version(S, N), version_cap(Cap), N < Cap.
% spec:27 "A pruned id is NotFound for every call"; api:28.
holds(_, found, S, Op) :- op_id(Op, Id), record_of(S, Id, _).

% --- submit (spec:21-27)
holds(submit, paused, S, _) :- pool_paused(S, false).
holds(submit, version, S, submit(_, _, _, V, _, _)) :- pool_version(S, N), V =:= N.
holds(submit, expiry, _, submit(_, _, _, _, Exp, Now)) :-
    constant(expiry_horizon, H, _),
    sat_add64(Now, H, Max),                 % CODE-ONLY: saturating u64 sum
    Exp > Now, Exp =< Max.
holds(submit, freshness, S, submit(_, _, _, _, _, Now)) :- fresh(S, Now).
% spec:21 "Full only when 256 records are retained and the oldest is open"
holds(submit, record_bound, S, _) :-
    pool_records(S, Rs), length(Rs, N), window_size(W),
    (   N < W -> true
    ;   Rs = [c(_, _, _, St) | _], terminal(St)
    ).
holds(submit, recipe, S, submit(_, RI, _, _, _, _)) :- pool_config(S, C), recipe_of(C, RI, _).
holds(submit, caller, S, submit(Caller, RI, _, _, _, _)) :- pool_config(S, C), actor_of(C, RI, A), Caller == A.
holds(submit, units, S, submit(_, RI, U, _, _, _)) :-
    pool_config(S, C), recipe_of(C, RI, r(_, _, _, _, MU, _)), U >= 1, U =< MU.
% spec:21 "completed-unit prerequisites (completed_units)"; handoff:19
holds(submit, prerequisites, S, submit(_, RI, _, _, _, _)) :-
    pool_config(S, C), recipe_of(C, RI, r(_, _, _, _, _, Req)),
    forall(member(p(P, U), Req), ( completed_units(S, P, N), N >= U )).
holds(submit, reserve_arith, S, Op) :- submit_next(S, Op, _).
holds(submit, envelope_arith, S, Op) :- submit_envelope(S, Op, _, _).
holds(submit, bounds, S, Op) :-
    submit_envelope(S, Op, Low, High), submit_next(S, Op, Next),
    pool_config(S, C), pool_values(S, V),
    hard_ok(C, V, Next, Low, High).
holds(submit, energy, S, Op) :-
    submit_next(S, Op, Next), pool_config(S, C), pool_values(S, V),
    energy(C, Next, E1), energy(C, V, E0), E1 =< E0.
holds(submit, recovery, S, Op) :-
    pool_config(S, C), pool_values(S, V),
    (   \+ violating(C, V) -> true
    ;   Op = submit(_, RI, U, _, _, _),
        submit_next(S, Op, Next), recipe_of(C, RI, r(_, _, _, Com, _, _)),
        (   add_vec(Next, Com, U, After) -> true
        ;   % CODE-ONLY: the code's add here returns Arithmetic, which cannot
            % happen once the envelope (guard envelope_arith) passed; surfaced loudly.
            throw(unreachable(recovery_add, S, Op))
        ),
        energy(C, After, EA), energy(C, V, E0), EA < E0
    ).

submit_next(S, submit(_, RI, U, _, _, _), Next) :-
    pool_config(S, C), recipe_of(C, RI, r(_, _, Res, _, _, _)), pool_values(S, V),
    add_vec(V, Res, U, Next).
submit_envelope(S, Op, Low, High) :-
    Op = submit(_, RI, U, _, _, _),
    submit_next(S, Op, Next),
    pool_envelope(S, Next, extra(RI, U), none, Low, High).

fresh(S, Now) :-
    S = pool(config(_, MaxAge, _, _, _), _, _, Obs, _, _, _, _),
    (   MaxAge =:= 0 -> true
    ;   sat_add64(Obs, MaxAge, Lim), Now =< Lim
    ).

% --- dispatch (spec:39)
holds(dispatch, caller, S, dispatch(Caller, Id, _)) :-
    record_of(S, Id, c(RI, _, _, _)), pool_config(S, C), actor_of(C, RI, A), Caller == A.
holds(dispatch, paused, S, _) :- pool_paused(S, false).
holds(dispatch, reserved, S, dispatch(_, Id, _)) :- record_of(S, Id, c(_, _, _, reserved)).
holds(dispatch, unexpired, S, dispatch(_, Id, Now)) :- record_of(S, Id, c(_, _, Exp, _)), Now < Exp.
holds(dispatch, freshness, S, dispatch(_, _, Now)) :- fresh(S, Now).

% --- acknowledge (spec:40-41, 46)
holds(ack, awaiting, S, ack(Id, _)) :-
    record_of(S, Id, c(_, _, _, St)), memberchk(St, [dispatched, unknown]).
% spec:41 "Unknown -> Unknown is an Ok no-op without a version change"; api:30:
% an acknowledgment that would not change the status changes nothing.
holds(ack, changes_status, S, ack(Id, Acc)) :-
    record_of(S, Id, c(_, _, _, St)), ack_next(Acc, Next), St \== Next.
ack_next(true, active).
ack_next(false, unknown).

% --- cancel (spec:43, 45; api:17)
holds(cancel, caller, S, cancel(Caller, Id, _)) :-
    record_of(S, Id, c(RI, _, _, _)), pool_config(S, C),
    actor_of(C, RI, A), owner_of(C, O), ( Caller == A ; Caller == O ), !.
holds(cancel, not_cancelled, S, cancel(_, Id, _)) :- record_of(S, Id, c(_, _, _, St)), St \== cancelled.
% spec:43 "A cancel of a Completed record returns InvalidState" (v1.0.0: Pending)
holds(cancel, not_completed, S, cancel(_, Id, _)) :- record_of(S, Id, c(_, _, _, St)), St \== completed.
holds(cancel, reserved, S, cancel(_, Id, _)) :- record_of(S, Id, c(_, _, _, reserved)).
holds(Kind, release_arith, S, Op) :- release_kind(Kind), op_id(Op, Id), release_next(S, Id, _).
holds(Kind, envelope_arith, S, Op) :-
    terminal_kind(Kind), op_id(Op, Id), terminal_next(Kind, S, Id, Next),
    pool_envelope(S, Next, none, Id, _, _).
holds(Kind, bounds, S, Op) :-
    terminal_kind(Kind), op_id(Op, Id), terminal_next(Kind, S, Id, Next),
    pool_config(S, C), pool_values(S, V),
    pool_envelope(S, Next, none, Id, Low, High),
    hard_ok(C, V, Next, Low, High).

% --- settle (spec:42, 44, 45)
holds(Kind, caller, S, settle(Caller, Id, _)) :-
    settle_kind(Kind),
    record_of(S, Id, c(RI, _, _, _)), pool_config(S, C), executor_of(C, RI, X), Caller == X.
holds(Kind, not_same_terminal, S, settle(_, Id, Done)) :-
    settle_kind(Kind),
    record_of(S, Id, c(_, _, _, St)),
    \+ ( Done == true, St == completed ),
    \+ ( Done == false, St == cancelled ).
holds(Kind, in_flight, S, settle(_, Id, _)) :-
    settle_kind(Kind),
    record_of(S, Id, c(_, _, _, St)), memberchk(St, [dispatched, active, unknown]).
holds(settle_completed, complete_arith, S, settle(_, Id, _)) :- complete_next(S, Id, _).

% --- set_paused (spec:58)
holds(pause, owner, S, pause(Caller, _)) :- pool_config(S, C), owner_of(C, O), Caller == O.

% --- observe (spec:58; api:19)
holds(observe, owner, S, observe(Caller, _, _)) :- pool_config(S, C), owner_of(C, O), Caller == O.
% spec:58 "sets new values only when every record is terminal; restating the
% current values is allowed at any time" (lib.rs:538: Pending iff the values
% differ and some record is open; v1.0.0: any open record).
holds(observe, all_terminal_or_restate, S, observe(_, Vs, _)) :-
    pool_values(S, V0),
    (   Vs == V0 -> true
    ;   pool_records(S, Rs), \+ ( member(c(_, _, _, St), Rs), open_status(St) )
    ).
holds(observe, bounded_values, S, observe(_, Vs, _)) :-
    pool_values(S, V0), length(V0, N), length(Vs, N),          % CODE-ONLY: length (amb-observe-length)
    maplist(bounded, Vs).

release_kind(cancel).
release_kind(settle_cancelled).
terminal_kind(cancel).
terminal_kind(settle_cancelled).
terminal_kind(settle_completed).
settle_kind(settle_completed).
settle_kind(settle_cancelled).

op_id(dispatch(_, Id, _), Id).
op_id(ack(Id, _), Id).
op_id(cancel(_, Id, _), Id).
op_id(settle(_, Id, _), Id).

release_next(S, Id, Next) :-
    record_of(S, Id, c(RI, U, _, _)), pool_config(S, C), recipe_of(C, RI, r(_, _, Res, _, _, _)),
    pool_values(S, V), sub_vec(V, Res, U, Next).
complete_next(S, Id, Next) :-
    record_of(S, Id, c(RI, U, _, _)), pool_config(S, C), recipe_of(C, RI, r(_, _, _, Com, _, _)),
    pool_values(S, V), add_vec(V, Com, U, Next).
terminal_next(settle_completed, S, Id, Next) :- !, complete_next(S, Id, Next).
terminal_next(_, S, Id, Next) :- release_next(S, Id, Next).

%%% Effects of calls whose guards all pass

% spec:27 "append a Reserved record with id first_id + len(records), replace
% values, increment version. If 256 records are retained, first drop the
% oldest, which is terminal, and increment first_id."
effect(submit, S0, Op, ok(Id), S1) :-
    Op = submit(_, RI, U, _, Exp, _),
    submit_next(S0, Op, Next),
    S0 = pool(C, _, N, Obs, P, Recs, F, D),
    length(Recs, L), Id is F + L,
    window_size(W),
    (   L >= W
    ->  Recs = [_ | Kept], F1 is F + 1          % record_bound passed: the front is terminal
    ;   Kept = Recs, F1 = F
    ),
    append(Kept, [c(RI, U, Exp, reserved)], Recs1),
    N1 is N + 1,
    S1 = pool(C, Next, N1, Obs, P, Recs1, F1, D).
% spec:39 Reserved -> Dispatched (the shell then creates the promise)
effect(dispatch, S0, dispatch(_, Id, _), ok, S1) :- set_status(S0, Id, dispatched, S1).
% spec:40-41; "retain values" (ASYNC-1)
effect(ack, S0, ack(Id, Acc), ok, S1) :- ack_next(Acc, St), set_status(S0, Id, St, S1).
% spec:43-44 checked inverse: subtract the reservation, Cancelled
effect(Kind, S0, Op, ok, S1) :-
    release_kind(Kind), op_id(Op, Id),
    release_next(S0, Id, Next),
    set_status(S0, Id, cancelled, S0a),
    S0a = pool(C, _, N, O, P, R, F, D), S1 = pool(C, Next, N, O, P, R, F, D).
% spec:42 add completion delta, Completed, add the units to completed_units
% (saturating u32, lib.rs:519)
effect(settle_completed, S0, settle(_, Id, _), ok, S1) :-
    complete_next(S0, Id, Next),
    record_of(S0, Id, c(RI, U, _, _)),
    set_status(S0, Id, completed, S0a),
    S0a = pool(C, _, N, O, P, R, F, D0),
    u32_max(M), nth0(RI, D0, X0, Rest), X1 is min(X0 + U, M), nth0(RI, D1, X1, Rest),
    S1 = pool(C, Next, N, O, P, R, F, D1).
% spec:58 set_paused increments version (also when the flag does not change: CODE-ONLY)
effect(pause, pool(C, V, N, O, _, R, F, D), pause(_, B), ok, pool(C, V, N1, O, B, R, F, D)) :- N1 is N + 1.
% spec:58 replaces values and refreshes the snapshot time (a restatement of the
% current values changes only observed_at and version); spec:50 increments version
effect(observe, pool(C, _, N, _, P, R, F, D), observe(_, Vs, Now), ok, pool(C, Vs, N1, Now, P, R, F, D)) :-
    N1 is N + 1.

set_status(pool(C, V, N, O, P, R0, F, D), Id, St, pool(C, V, N1, O, P, R1, F, D)) :-
    N1 is N + 1, K is Id - F,
    nth0(K, R0, c(RI, U, E, _), Rest),
    nth0(K, R1, c(RI, U, E, St), Rest).

%%% ========================================================
%%% Reads (spec:29; api:10, 30-32)
%%% ========================================================

%% admissible(+S, +Caller, +Req): candidate(caller, req, now) is Ok, i.e. every
%% submit guard passes (spec:29 "preview calls the same checker"). The version
%% cap is candidate's last check (spec:50 "so preview reports it"), so it is
%% included. Req = req(Recipe, Units, Version, ExpiresAt, Now).
admissible(S, Caller, Req) :- admissible_opts(S, Caller, Req, []).

%% admissible_opts/4 with Opts = [ignore_pending_negative]: the same check with
%% the open records' negative completion parts dropped from the envelope (C-5
%% RISK-HEADROOM compares the two).
admissible_opts(S, Caller, req(RI, U, V, Exp, Now), Opts) :-
    Op = submit(Caller, RI, U, V, Exp, Now),
    guard_ids(submit, Ids),
    forall(member(G, Ids), holds_opts(G, S, Op, Opts)).

holds_opts(G, S, Op, Opts) :-
    (   memberchk(ignore_pending_negative, Opts), memberchk(G, [envelope_arith, bounds])
    ->  Op = submit(_, RI, U, _, _, _),
        submit_next(S, Op, Next),
        S = pool(C, _, _, _, _, Recs, F, _),
        envelope_w(C, F, Recs, Next, extra(RI, U), none, Opts, Low, High),
        (   G == bounds -> pool_values(S, V0), hard_ok(C, V0, Next, Low, High) ; true )
    ;   holds(submit, G, S, Op)
    ).

preview_allowed(S, Caller, Req) :- admissible(S, Caller, Req).

%% best_quantity(+S, +Caller, +Req, -Best): spec:29 enumerate units 1..min(units, 100)
%% (CODE-ONLY cap MAX_UNITS), minimise energy of the candidate, ties -> larger units.
%% Best = none | some(Units).
best_quantity(S, Caller, req(RI, U, V, Exp, Now), Best) :-
    model_constant(max_units, MaxU, _), Hi is min(U, MaxU),
    pool_config(S, C),
    findall(E-Neg, ( between(1, Hi, K),
                     admissible(S, Caller, req(RI, K, V, Exp, Now)),
                     submit_next(S, submit(Caller, RI, K, V, Exp, Now), Next),
                     energy(C, Next, E), Neg is -K ), Cands),
    (   Cands == [] -> Best = none
    ;   keysort_first(Cands, _-NegB), K0 is -NegB, Best = some(K0)
    ).

%% keysort_first(+Pairs, -Least): the least E-Neg pair in standard order
%% (smallest energy, then smallest -Units = largest units).
keysort_first([P | Ps], Least) :- keysort_first_(Ps, P, Least).
keysort_first_([], L, L).
keysort_first_([P | Ps], L0, L) :- ( P @< L0 -> keysort_first_(Ps, P, L) ; keysort_first_(Ps, L0, L) ).
