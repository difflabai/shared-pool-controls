%% consistency.pl -- Requirement checks C-1..C-6: the docs (pool_req.pl) against
%% the code (extracted/code_facts.pl), the TLA+ model (extracted/tla_facts.pl),
%% the conformance traces and the property catalogue (strategy.md section 5).
%%
%% PROP: C-1 C-2 C-3 C-4 C-5 C-6
%%
%%   c_ids(-Ids)
%%   c_check(+Id, -Result)     Result = ok(Info) | fail(Problems, Info)
%%
%% Every check collects ALL its problems (never stops at the first) so a
%% failure report is complete. Info carries counts that show the check was
%% not vacuous (how many rows were compared).

:- module(consistency, [
    c_ids/1,
    c_check/2,
    code_path/3,
    spec_outcomes/2,
    c2_align/3,
    unreachable/2,
    unreachable_rule/3,
    code_life/4,
    strategy_path/1,
    strategy_catalogue/3,
    catalogue_rows/3,
    catalogue_problems/3
]).

:- use_module(library(lists)).
:- use_module(library(iso_ext)).
:- use_module(library(pairs)).
:- use_module(library(between)).
:- use_module(library(files)).
:- use_module(library(pio)).
:- use_module(library(dcgs)).
:- use_module('../requirements/pool_req').
:- use_module('../requirements/traceability').
:- use_module('../requirements/threat_model').
:- use_module('../extracted/code_facts').
:- use_module('../extracted/tla_facts').
:- use_module('../model/pool_machine').
:- use_module(traces).
:- use_module(props).
:- use_module(util).

c_ids(['C-1', 'C-2', 'C-3', 'C-4', 'C-5', 'C-6']).

:- discontiguous(c_problems/3).

c_check(Id, R) :-
    c_problems(Id, Ps0, Info),
    sort(Ps0, Ps),
    (   Ps == [] -> R = ok(Info) ; R = fail(Ps, Info) ).

%%% ========================================================
%%% C-1  Authority matrix: api.md table + spec:11 = shell attributes + core caller checks
%%% ========================================================

c_problems('C-1', Ps, [methods(NM), shell_methods(NS), core_caller_checks(NC), shell_core_calls(NK),
                        core_reads(NR)]) :-
    findall(P, c1_problem(P), Ps),
    count_where(x, method(_, _, _, _), NM),
    count_where(x, shell_method(_, _, _, _), NS),
    count_where(x, code_caller_check(_, _, _, _, _), NC),
    count_where(x, shell_core_call(_, _, _, _), NK),
    count_where(x, ( shell_core_call(_, F, _, _), \+ code_fn(F, _, mut) ), NR).

auth_roles(actor, [actor]).
auth_roles(executor, [executor]).
auth_roles(actor_or_owner, [actor, owner]).
auth_roles(owner, [owner]).

% api:7-20: the documented methods are exactly the shell's #[near] methods.
c1_problem(missing_shell_method(M)) :- method(M, _, _, _), \+ shell_method(M, _, _, _).
c1_problem(undocumented_shell_method(M)) :- shell_method(M, _, _, _), \+ method(M, _, _, _).
% api:5: all mutating methods are nonpayable (near-sdk default; #[payable] opts out).
c1_problem(payable(M)) :- shell_method(M, _, Attrs, _), memberchk(payable, Attrs).
% api:9, api:20, spec:13: initializer and callback are #[private]; views take &self.
c1_problem(shape(M, Kind, Attrs, Recv)) :-
    method(M, Kind, _, _), shell_method(M, _, Attrs, Recv),
    \+ kind_shape(Kind, M, Attrs, Recv).
% spec:13: the wrapper authorizes by immediate predecessor.
c1_problem(caller_helper(H)) :- shell_helper(caller, H, _), H \== predecessor_account_id.
c1_problem(no_caller_helper) :- \+ shell_helper(caller, _, _).
% Every core function the shell calls exists (Pool::clone is derived, not core code).
c1_problem(core_call_unknown(M, F)) :-
    shell_core_call(M, F, _, _), F \== clone, \+ code_fn(F, _, _).
% A caller-checked method passes the predecessor to every mutating core call it
% makes (v1.1.0: dispatch, reconcile and on_result also read `record(id)`, a
% &self lookup that takes no caller; reads carry no authority) ...
c1_problem(caller_source(M, F, Src)) :-
    method(M, change, Auth, _), auth_roles(Auth, _),
    shell_core_call(M, F, Src, _), code_fn(F, _, mut), Src \== predecessor.
c1_problem(no_core_call(M)) :-
    method(M, change, Auth, _), auth_roles(Auth, _), \+ shell_core_call(M, _, _, _).
% ... and the core transition it drives checks exactly the documented roles
% (one conjunctive check: caller /= each role => Unauthorized).
c1_problem(core_roles(M, F, expected(Exp), found(Found))) :-
    method(M, change, Auth, _), auth_roles(Auth, Exp0), sort(Exp0, Exp),
    method_core(M, F),
    findall(Rs-Mode, ( caller_check_reachable(F, _, Rs0, Mode), sort(Rs0, Rs) ), Found0),
    sort(Found0, Found),
    Found \== [Exp-and].
c1_problem(core_error(F, E)) :- code_caller_check(F, _, _, _, E), E \== 'Unauthorized'.
% api:20, spec:40-44: the private callback drives settle and acknowledge only
% (and reads the record), settle with the recipe's own executor (authenticity:
% #[private], ASM-CALLBACK).
c1_problem(callback_core(M, F)) :-
    method(M, callback, _, _), shell_core_call(M, F, _, _), code_fn(F, _, mut), \+ method_core(M, F).
c1_problem(callback_core_missing(M, F)) :-
    method(M, callback, _, _), method_core(M, F), \+ shell_core_call(M, F, _, _).
c1_problem(callback_settle_caller(Src)) :-
    method(M, callback, _, _), shell_core_call(M, settle, Src, _), Src \== recipe_executor.
% acknowledge has no caller of its own: no core check, called only from private methods.
c1_problem(acknowledge_has_caller_check) :- caller_check_reachable(acknowledge, _, _, _).
c1_problem(acknowledge_from_public(M)) :-
    shell_core_call(M, acknowledge, _, _), shell_method(M, _, A, _), \+ memberchk(private, A).
% No mutating core transition is reachable except through its documented method.
c1_problem(undocumented_core_call(M, F)) :-
    shell_core_call(M, F, _, _), code_fn(F, _, mut), \+ method_core(M, F).
c1_problem(documented_core_call_missing(M, F)) :-
    method_core(M, F), \+ shell_core_call(M, F, _, _).
% spec:13: preview conveys no authorization: the core preview cannot mutate.
c1_problem(preview_mutates(R)) :- code_fn(preview, _, R), R \== ref.
c1_problem(preview_not_view) :- \+ method(preview, view, _, _).

kind_shape(init, _, A, _) :- memberchk(init, A), memberchk(private, A).
kind_shape(callback, _, A, mut) :- memberchk(private, A), \+ memberchk(init, A).
kind_shape(view, _, A, ref) :- \+ memberchk(private, A), \+ memberchk(init, A).
% amb-reconcile-kind: reconcile is open to anyone and changes nothing itself
% (it may read the record; it calls no mutating core function).
kind_shape(change, reconcile, [], ref) :-
    \+ ( shell_core_call(reconcile, F, _, _), code_fn(F, _, mut) ).
kind_shape(change, M, [], mut) :- M \== reconcile.

%% caller_check_reachable(+Fn, -Where, -Roles, -Mode): a caller check in Fn or in a
%% function it calls (core functions do not recurse).
caller_check_reachable(F, G-L, Rs, Mode) :-
    findall(G0, reach(F, G0), Gs0), sort(Gs0, Gs),
    member(G, Gs),
    code_caller_check(G, L, Mode, Rs, _).

reach(F, F).
reach(F, G) :-
    code_events(F, Es), member(E, Es),
    ( E = c(H, _) ; E = t(H, _) ),
    reach(H, G).

%%% ========================================================
%%% C-2  Guard order: spec (guard/5) = code paths = TLA+ GuardOrder
%%% ========================================================
%% The code path of an op kind is the core function's event list (errors,
%% no-op returns, calls) with calls expanded in place. In a leaf function
%% (no calls) adjacent repeats of one error collapse: they are one check over
%% several components. Error sites that cannot fire from an INV state are
%% marked by unreachable/2 (decision c2-unreachable); they may be skipped or
%% matched, every other site must match the specification in order. The
%% specification leg also compares sites, not only outcome names: a guard with
%% a documented condition (guard_condition/4: the record bound, the version
%% cap) matches only a site whose extracted `if` condition (code_error_cond/4)
%% is the same, and no other guard matches such a site. So observe's plain
%% bump() in place of bump_new() (an Arithmetic site without the cap test), or
%% a Full test without the open-front conjunct, fails C-2 although the outcome
%% names still line up. The TLA+ leg has outcome names only.

kind_code(submit,           submit,      none).
kind_code(dispatch,         dispatch,    none).
kind_code(ack,              acknowledge, none).
kind_code(cancel,           cancel,      none).
kind_code(settle_completed, settle,      true).
kind_code(settle_cancelled, settle,      false).
kind_code(pause,            set_paused,  none).
kind_code(observe,          observe,     none).

tla_kind(submit, submit).  tla_kind(dispatch, dispatch).  tla_kind(ack, ack).
tla_kind(cancel, cancel).  tla_kind(settle_completed, settle).
tla_kind(settle_cancelled, settle).  tla_kind(pause, pause).  tla_kind(observe, observe).

%% code_path(+Fn, +Flag, -Items): Items = [it(Outcome, Frames)..], Outcome an
%% error name or noop, Frames = [Fn-Line | caller frames] innermost first.
code_path(Fn, Flag, Items) :- expand(Fn, Flag, [], Items).

expand(Fn, Flag, Stack, Items) :-
    code_events(Fn, Es0),
    (   leaf(Es0) -> collapse(Es0, Es) ; Es = Es0 ),
    walk(Es, Fn, Flag, Stack, Items).

leaf(Es) :- \+ member(c(_, _), Es), \+ member(t(_, _), Es).

collapse([], []).
collapse([E], [E]) :- !.
collapse([e(X, L), e(Y, _) | Es], Out) :- X == Y, !, collapse([e(X, L) | Es], Out).
collapse([E | Es], [E | Out]) :- collapse(Es, Out).

walk([], _, _, _, []).
walk([E | Es], Fn, Flag, Stack, Items) :-
    (   E = e(Err, L)
    ->  Items = [it(Err, [Fn-L | Stack]) | Rest], walk(Es, Fn, Flag, Stack, Rest)
    ;   E = noop(L)
    ->  Items = [it(noop, [Fn-L | Stack]) | Rest], walk(Es, Fn, Flag, Stack, Rest)
    ;   E = c(G, L)
    ->  expand(G, none, [Fn-L | Stack], Sub), append(Sub, Rest, Items),
        walk(Es, Fn, Flag, Stack, Rest)
    ;   E = t(G, L)
    ->  (   tail_taken(Fn, G, Flag)
        ->  expand(G, none, [Fn-L | Stack], Items)          % the tail call returns
        ;   walk(Es, Fn, Flag, Stack, Items)
        )
    ).

%% tail_taken(+Fn, +Callee, +Flag): an unguarded tail call is always taken; a
%% guarded one (settle: `if !completed { return self.release(id) }`) when its
%% flag condition holds for the op's boolean.
tail_taken(Fn, G, Flag) :-
    (   code_status_guard(Fn, _, Cond, tail(G)) -> flag_cond(Cond, Flag) ; true ).

flag_cond(flag(_), F) :- !, F == true.
flag_cond(not(C), F) :- !, \+ flag_cond(C, F).
flag_cond(and(A, B), F) :- !, flag_cond(A, F), flag_cond(B, F).
flag_cond(or(A, B), F) :- !, ( flag_cond(A, F) -> true ; flag_cond(B, F) ).
flag_cond(C, _) :- throw(error(domain_error(flag_condition, C), tail_taken)).

%% unreachable(+Item, -Rule): an error site that cannot fire from an INV state.
unreachable(it('InvalidConfig', [add-_ | _]), add_length).
% v1.0.0 frames [record-_, release-_]; v1.1.0 record() calls index() (lib.rs:393).
unreachable(it('NotFound', [record-_, release-_ | _]), release_record).
unreachable(it('NotFound', [index-_, record-_, release-_ | _]), release_record).
% v1.1.0: a direct index(id)? in a function that already found the id through
% record(id)? earlier in its own body (dispatch, acknowledge, release, settle).
unreachable(it('NotFound', [index-_, F-L | _]), index_again) :-
    F \== record, code_events(F, Es), member(c(record, L0), Es), L0 < L.
unreachable(it(E, [energy_terms-_ | _]), energy_terms) :- memberchk(E, ['Bounds', 'Arithmetic']).
unreachable(it('Arithmetic', [energy-_ | _]), energy_sum).
unreachable(it('Arithmetic', [add-_, candidate-L | _]), recovery_add) :-
    code_events(candidate, Es), member(c(check_bounds, LB), Es), L > LB.
% v1.1.0: bump() inside bump_new(), after bump_new's own cap check (an
% Arithmetic site whose extracted condition is version >= VERSION_CAP).
unreachable(it('Arithmetic', [bump-_, bump_new-L | _]), bump_below_cap) :-
    cap_check(bump_new, LC), LC < L.
% v1.1.0: the plain checked increment of acknowledge, release and settle.
unreachable(it('Arithmetic', [bump-_, F-_ | _]), bump_reserve) :-
    memberchk(F, [acknowledge, release, settle]).
% v1.1.0: submit's own checked additions (lib.rs:362, 363-366), after a call
% to candidate, whose last check is the cap check (straight-line: every Ok
% return of candidate passed it, and nothing in between bumps the version).
unreachable(it('Arithmetic', [submit-L | _]), submit_version_add) :-
    code_checked_add(submit, L, version), cap_checked_before(submit, L).
unreachable(it('Arithmetic', [submit-L | _]), submit_id_add) :-
    code_checked_add(submit, L, first_id), cap_checked_before(submit, L).

%% cap_check(?Fn, ?Line): an Arithmetic site of Fn whose `if` condition is the
%% documented version cap (the same condition guard_condition/4 gives
%% version_cap).
cap_check(Fn, L) :-
    code_error_cond(Fn, L, 'Arithmetic', CC),
    once(spec_cond(_, version_cap, SC)), same_conds(SC, CC).
cap_checked_before(Fn, L) :-
    code_events(Fn, Es), member(c(G, LG), Es), LG < L, code_events(G, GEs),
    append(_, [e('Arithmetic', LC)], GEs), cap_check(G, LC).

%% unreachable_rule(Rule, Why, DischargedBy)
unreachable_rule(add_length, "add's length check: every vector has length n in a valid config and an INV state", ['CFG-2', 'INV-WF']).
unreachable_rule(release_record, "release looks the record up again after cancel/settle found it", []).
unreachable_rule(index_again, "the second lookup of an id that record(id) found earlier in the same call; nothing in between removes records", []).
unreachable_rule(energy_terms, "spec:15: residual <= 2e6, weight <= 1000, <= 64 terms; bounded values cannot overflow", ['NUM-1']).
unreachable_rule(energy_sum, "spec:15: the sum of <= 64 terms of <= 4e15 fits u64", ['NUM-1']).
unreachable_rule(recovery_add, "next + complete*units lies inside the envelope that already passed (spec:23: the interval contains the proposed completion)", ['INV-HARD']).
unreachable_rule(bump_below_cap, "bump_new fails Arithmetic at version >= VERSION_CAP before calling bump, and VERSION_CAP < u32::MAX (C-4 version_reserve), so the increment fits", ['VER-CAP', 'C-4']).
unreachable_rule(bump_reserve, "spec:50: acknowledgment, settlement and cancellation keep the plain checked increment; version + sum of rem(status) over open records <= u32::MAX, and the call's record is open, so the increment fits", ['INV-VERCAP']).
unreachable_rule(submit_version_add, "candidate's last check failed Arithmetic at version >= VERSION_CAP (spec:50), so version + 1 fits", ['VER-CAP', 'C-4']).
unreachable_rule(submit_id_add, "first_id + len(records) <= version < VERSION_CAP (spec:9, spec:27)", ['WIN-1']).

%% spec_outcomes(+Kind, -Items): guard/5 order, as g(GuardId, Outcome, Cond):
%% err(E) -> E, noop -> noop; Cond the guard's documented condition
%% (guard_condition/4, doc constants renamed to the code's) or none.
spec_outcomes(K, Outs) :-
    guard_ids(K, Gs), guard_sequence(K, Os), maplist(spec_item(K), Gs, Os, Outs).
spec_item(K, G, O, g(G, X, C)) :-
    spec_out(O, X), ( spec_cond(K, G, C0) -> C = C0 ; C = none ).
spec_out(err(E), E).
spec_out(noop, noop).
g_out(g(_, X, _), X).

%% spec_cond(?Kind, ?Guard, -Conjuncts): guard_condition/4 with each doc
%% constant replaced by its code name (doc_code_constant/2, as in C-4).
spec_cond(K, G, C) :- guard_condition(K, G, C0, _), maplist(code_conj, C0, C).
code_conj(T, T1) :- T =.. [F, A, D], doc_code_constant(D, Code), !, T1 =.. [F, A, Code].
code_conj(T, T).
same_conds(A, B) :- msort_dup(A, A1), msort_dup(B, B1), A1 == B1.

tla_out('Noop', noop) :- !.
tla_out(E, E).

%% c2_align(+Expected, +Items, -Alignment): every reachable item matches the next
%% expected item in order; an unreachable item may match or be skipped.
%% Expected items are outcome names (the TLA+ leg) or g/3 (the specification:
%% site-checked). c2_align/4's first argument is `sites` or `names` (outcome
%% names only, used to tell a condition mismatch from an order mismatch).
c2_align(S, Items, A) :- c2_align(sites, S, Items, A).

c2_align(_, [], [], []).
c2_align(M, S, [It | Cs], [skip(R) | A]) :- unreachable(It, R), c2_align(M, S, Cs, A).
c2_align(M, [X | S], [it(Y, F) | Cs], [X-F | A]) :- c2_match(M, X, Y, F), c2_align(M, S, Cs, A).

c2_match(_, X, Y, _) :- atom(X), !, X == Y.
c2_match(names, g(_, X, _), Y, _) :- X == Y.
c2_match(sites, g(_, X, C), Y, F) :- X == Y, site_ok(C, X, F).

%% site_ok(+SpecCond, +Outcome, +Frames): a guard with a documented condition
%% matches only a site whose extracted condition is the same; a guard without
%% one does not match a site whose condition some guard documents.
site_ok(none, X, [Fn-L | _]) :- !,
    \+ ( code_error_cond(Fn, L, X, CC), spec_cond(_, _, SC), same_conds(SC, CC) ).
site_ok(SC, X, [Fn-L | _]) :- code_error_cond(Fn, L, X, CC), same_conds(SC, CC).

c_problems('C-2', Ps, Info) :-
    findall(P, c2_problem(P), Ps),
    findall(K-N, ( op_kind(K), guard_sequence(K, Gs), length(Gs, N) ), Sizes),
    findall(B, ( guard(_, _, _, _, B0), functor(B0, B, _) ), Bs), msort_dup(Bs, Bs1), tally(Bs1, Basis),
    findall(R, ( op_kind(K), kind_code(K, F, Fl), code_path(F, Fl, Is), member(It, Is), unreachable(It, R) ), Rs0),
    sort(Rs0, Rs),
    (   tla_source(present) -> Tla = tla_leg(compared) ; Tla = tla_leg(gap_no_tla_file) ),
    findall(K-G-Fn-L, ( op_kind(K), kind_code(K, F, Fl), code_path(F, Fl, Is), spec_outcomes(K, S),
                        once(c2_align(S, Is, A)), member(g(G, _, C)-[Fn-L | _], A), C \== none ), Conds),
    length(Conds, NConds),
    Info = [guards_per_kind(Sizes), basis(Basis), unreachable_rules_used(Rs),
            conditions_compared(NConds), Tla].

%% The specification leg: an order (or outcome) difference is spec_vs_code;
%% if the outcome names line up but a documented condition does not, the
%% problem names each guard whose site has another condition (code(none): the
%% site has no extracted `if` condition, e.g. a plain checked increment).
c2_problem(P) :-
    op_kind(K), kind_code(K, Fn, Flag),
    code_path(Fn, Flag, Items), spec_outcomes(K, S),
    \+ once(c2_align(S, Items, _)),
    (   once(c2_align(names, S, Items, A))
    ->  findall(G-spec(C)-site(SFn-L)-code(CC),
                ( member(g(G, X, C)-[SFn-L | Up], A), \+ site_ok(C, X, [SFn-L | Up]),
                  ( code_error_cond(SFn, L, X, CC0) -> CC = CC0 ; CC = none ) ), Ds),
        P = condition_vs_code(K, Ds)
    ;   maplist(g_out, S, Names), reachable_outcomes(Items, Reach),
        P = spec_vs_code(K, spec(Names), code(Reach))
    ).
c2_problem(tla_vs_code(K, tla(T), code(Reach))) :-
    tla_source(present),
    op_kind(K), tla_kind(K, Op), tla_guard_order(Op, T0), maplist(tla_out, T0, T),
    kind_code(K, Fn, Flag), code_path(Fn, Flag, Items),
    \+ once(c2_align(names, T, Items, _)),
    reachable_outcomes(Items, Reach).
c2_problem(guard_condition_unknown(K, G)) :-
    guard_condition(K, G, _, _), \+ guard(K, _, G, _, _).
c2_problem(tla_missing(Op)) :-
    tla_source(present), op_kind(K), tla_kind(K, Op), \+ tla_guard_order(Op, _).
c2_problem(tla_unknown_op(Op)) :-
    tla_guard_order(Op, _), \+ tla_kind(_, Op).
% api:32 lists submit's reasons: exactly the errors reachable in candidate.
c2_problem(api32_reasons(doc(D), code(C))) :-
    findall(E, pool_error(E, 'api:32'), D0), sort(D0, D),
    code_path(candidate, none, Items),
    findall(E, ( member(it(E, F), Items), E \== noop, \+ unreachable(it(E, F), _) ), C0), sort(C0, C),
    D \== C.
% Every Error variant of the code is named by the docs (v1.1.0 names NotFound,
% Pending, InvalidState and InvalidConfig too: pool_error/2).
c2_problem(error_unaccounted(V)) :-
    code_enum('Error', Vs, _), member(V, Vs), \+ pool_error(V, _).
c2_problem(error_unknown_to_code(E)) :-
    guard(_, _, _, err(E), _), code_enum('Error', Vs, _), \+ memberchk(E, Vs).
c2_problem(unreachable_rule_undocumented(R)) :-
    op_kind(K), kind_code(K, F, Fl), code_path(F, Fl, Is), member(It, Is), unreachable(It, R),
    \+ unreachable_rule(R, _, _).

reachable_outcomes(Items, Outs) :-
    findall(E, ( member(It, Items), It = it(E, _), \+ unreachable(It, _) ), Outs).


%%% ========================================================
%%% C-3  Lifecycle: spec life/5 = code status guards = TLA+ Edges = LIFE-1, and the traces
%%% ========================================================

life_fn(dispatch, dispatch).
life_fn(ack, acknowledge).
life_fn(cancel, cancel).
life_fn(settle, settle).
life_flags(dispatch, [none]).
life_flags(cancel, [none]).
life_flags(ack, [true, false]).
life_flags(settle, [true, false]).

life_domain(Op, F, S) :- life_fn(Op, _), life_flags(Op, Fs), member(F, Fs), status(S).

%% code_life(+Op, +Flag, +From, -Outcome): the code's status guards in source
%% order; the first that fires decides; otherwise the function's status write
%% (or that of its unguarded tail call).
code_life(Op, Flag, From, Out) :-
    life_fn(Op, Fn),
    findall(L-g(C, O), code_status_guard(Fn, L, C, O), Gs0), keysort(Gs0, Gs1), pairs_values(Gs1, Gs),
    next_status(Fn, Flag, Next),
    (   first_fired(Gs, Flag, From, Next, O)
    ->  outcome_of(O, Out)
    ;   default_write(Fn, Next, Out)
    ).

first_fired([g(C, O) | Gs], Flag, From, Next, Out) :-
    (   eval(C, Flag, From, Next) -> Out = O ; first_fired(Gs, Flag, From, Next, Out) ).

outcome_of(err(E), reject(E)).
outcome_of(noop, same).
outcome_of(tail(G), to(S)) :- code_status_write(G, _, S).

default_write(Fn, Next, to(S)) :-
    (   code_status_write(Fn, _, W)
    ->  ( W == next -> S = Next ; S = W )
    ;   code_events(Fn, Es), member(t(G, _), Es), \+ code_status_guard(Fn, _, _, tail(G))
    ->  code_status_write(G, _, S)
    ).

next_status(Fn, Flag, Next) :-
    (   code_status_next(Fn, _, _, T, F) -> ( Flag == true -> Next = T ; Next = F ) ; Next = none ).

eval(eq(next), _, From, Next) :- !, From == Next.
eval(eq(S), _, From, _) :- !, From == S.
eval(ne(S), _, From, _) :- !, From \== S.
eval(in(L), _, From, _) :- !, memberchk(From, L).
eval(not(C), F, From, N) :- !, \+ eval(C, F, From, N).
eval(and(A, B), F, From, N) :- !, eval(A, F, From, N), eval(B, F, From, N).
eval(or(A, B), F, From, N) :- !, ( eval(A, F, From, N) -> true ; eval(B, F, From, N) ).
eval(flag(_), F, _, _) :- !, F == true.
eval(C, _, _, _) :- throw(error(domain_error(status_condition, C), code_life)).

c_problems('C-3', Ps, Info) :-
    findall(P, c3_static_problem(P), P1),
    c3_trace(P2, Obs),
    append(P1, P2, Ps),
    count_where(x, life_domain(_, _, _), NDom),
    findall(Op-F-S, ( life_domain(Op, F, S), life(Op, F, S, SO, _), code_life(Op, F, S, CO), SO \== CO ), Div),
    length(Div, NDiv),
    findall(Op-F-S, ( life_domain(Op, F, S), life(Op, F, S, _, code_only(_)) ), CodeOnly),
    spec_edges(SE), length(SE, NE),
    findall(Op-F-S, member(Op-F-S-_, Obs), Seen0), sort(Seen0, Seen), length(Seen, NObs),
    (   tla_source(present) -> Tla = tla_leg(compared) ; Tla = tla_leg(gap_no_tla_file) ),
    Info = [domain(NDom), edges(NE), divergences(NDiv), code_only_rows(CodeOnly),
            trace_domain_coverage(NObs, NDom), Tla].

% The table is total and deterministic over the domain.
c3_static_problem(spec_missing(Op, F, S)) :- life_domain(Op, F, S), \+ life(Op, F, S, _, _).
c3_static_problem(spec_ambiguous(Op, F, S, Os)) :-
    life_domain(Op, F, S), findall(O, life(Op, F, S, O, _), Os), Os = [_, _ | _].
c3_static_problem(code_undetermined(Op, F, S)) :- life_domain(Op, F, S), \+ code_life(Op, F, S, _).
% Spec = code per (op, flag, status), rejects with their error names. v1.1.0:
% no exception (div-ack-noop-version and div-cancel-terminal-name are closed:
% spec:41, 43 and api:30 now state what the code does).
c3_static_problem(mismatch(Op, F, S, spec(SO), code(CO))) :-
    life_domain(Op, F, S), life(Op, F, S, SO, _), code_life(Op, F, S, CO),
    SO \== CO.
% A self-loop listed as a transition would need a version bump on an Ok call that
% changes nothing; api:30 forbids it, so life/5 must say `same` instead.
c3_static_problem(listed_self_loop(Op, F, S)) :- life_domain(Op, F, S), life(Op, F, S, to(S), _).
% Status graph: spec = code = props LIFE-1 = TLA+ Props.tla Edges.
c3_static_problem(edges(spec(SE), code(CE))) :- spec_edges(SE), code_edges(CE), SE \== CE.
c3_static_problem(edges(spec(SE), life1(PE))) :- spec_edges(SE), props_edges(PE), SE \== PE.
c3_static_problem(edges(spec(SE), tla(TE))) :-
    tla_source(present), spec_edges(SE),
    findall(A-B, tla_status_edge(A, B), TE0), sort(TE0, TE), SE \== TE.
% spec:25: a new record is Reserved.
c3_static_problem(submit_status(S)) :- code_status_write(submit, _, S), S \== reserved.
c3_static_problem(submit_status_missing) :- \+ code_status_write(submit, _, _).

spec_edges(E) :- findall(A-B, ( life(_, _, A, to(B), _), A \== B ), E0), sort(E0, E).
code_edges(E) :- findall(A-B, ( life_domain(Op, F, A), code_life(Op, F, A, to(B)), A \== B ), E0), sort(E0, E).
props_edges(E) :- findall(A-B, ( status(A), status(B), allowed_status_change(A, B) ), E0), sort(E0, E).

%% c3_trace(-Problems, -Observed): the lifecycle the REAL core shows in the
%% conformance traces. A call on an existing record that returns Ok shows
%% to(S) or same; a status error (dispatch/settle InvalidState, cancel Pending
%% or InvalidState) shows reject(Error); any other error comes from a
%% non-status guard and is skipped. A trace without an initial state (v1.1.0
%% `init_error`, as traces.pl delivers it) has no calls to observe.
c3_trace(Ps, Obs) :-
    trace_file(File),
    fold_traces(File, c3_one, c3([], []), c3(Ps0, Obs0)),
    sort(Obs0, Obs),
    (   Obs == [] -> Ps = [trace_leg_vacuous | Ps0] ; Ps = Ps0 ).

c3_one(trace(N, Cfg, _, Init, Calls), c3(P0, O0), c3(P, O)) :-
    (   Init = init_error(_) -> P = P0, O = O0
    ;   trace_to_pool(Cfg, Init, S0) -> c3_calls(Calls, N, 1, Cfg, S0, P0, P, O0, O)
    ;   P = [trace_state_unreadable(N) | P0], O = O0
    ).

c3_calls([], _, _, _, _, P, P, O, O).
c3_calls([call(Op, R, Post) | Cs], N, I, Cfg, S0, P0, P, O0, O) :-
    trace_to_pool(Cfg, Post, S1),
    (   life_call(Op, Class, Flag, Id), record_of(S0, Id, c(_, _, _, From)),
        observed(Class, R, S1, Id, From, Ob)
    ->  life(Class, Flag, From, Spec, _),
        O1 = [Class-Flag-From-Ob | O0],
        (   Spec == Ob -> P1 = P0
        ;   P1 = [trace_mismatch(N, I, Op, From, spec(Spec), observed(Ob)) | P0]
        )
    ;   P1 = P0, O1 = O0
    ),
    I1 is I + 1,
    c3_calls(Cs, N, I1, Cfg, S1, P1, P, O1, O).

life_call(dispatch(_, Id, _), dispatch, none, Id).
life_call(ack(Id, F), ack, F, Id).
life_call(cancel(_, Id, _), cancel, none, Id).
life_call(settle(_, Id, F), settle, F, Id).

observed(_, R, S1, Id, From, Ob) :-
    ( R == ok ; R = ok(_) ), !,
    record_of(S1, Id, c(_, _, _, To)),
    ( To == From -> Ob = same ; Ob = to(To) ).
observed(Class, err(E), _, _, _, reject(C)) :- status_error(Class, E), camel_snake(C, E).

status_error(dispatch, invalid_state).
status_error(cancel, pending).
status_error(cancel, invalid_state).
status_error(settle, invalid_state).

%%% ========================================================
%%% C-4  Constants: docs = code; model's code-only bounds = code
%%% ========================================================

c_problems('C-4', Ps, [compared(NC), derived(ND), docs_silent(CO)]) :-
    findall(P, c4_problem(P), Ps),
    count_where(x, doc_code_constant(_, _), NC0),
    count_where(x, model_code_constant(_, _), NM),
    NC is NC0 + NM,
    ND = 5,
    findall(C-V, ( code_constant(C, V, _), \+ doc_code_constant(_, C) ), CO).
% (CO: code constants the docs do not state; each is compared with model_constant/3.)

doc_code_constant(max_value, 'MAX_VALUE').
doc_code_constant(max_terms, 'MAX_TERMS').
doc_code_constant(max_weight, 'MAX_WEIGHT').
doc_code_constant(max_records, 'MAX_RECORDS').
doc_code_constant(version_cap, 'VERSION_CAP').        % v1.1.0, spec:50

model_code_constant(max_metrics, 'MAX_METRICS').
model_code_constant(max_recipes, 'MAX_RECIPES').
model_code_constant(max_units, 'MAX_UNITS').
model_code_constant(max_requires, 'MAX_RECIPES').   % lib.rs:135 bounds requires.len() by MAX_RECIPES

c4_problem(doc_vs_code(N, doc(V), code(C, W))) :-
    doc_code_constant(N, C), constant(N, V, _),
    (   code_constant(C, W, _) -> V =\= W ; W = missing ).
c4_problem(model_vs_code(N, model(V), code(C, W))) :-
    model_code_constant(N, C), model_constant(N, V, _),
    (   code_constant(C, W, _) -> V =\= W ; W = missing ).
% spec:21 / api:30: one day = 86400 s, a literal in candidate.
c4_problem(expiry_horizon(doc(V), code(Ls))) :-
    constant(expiry_horizon, V, _),
    findall(X, code_literal(candidate, X, _), Ls),
    \+ memberchk(V, Ls).
% api:11: page max 50, a literal in the shell's get_commitments.
c4_problem(page_max(doc(V), code(Ls))) :-
    constant(page_max, V, _),
    findall(X, shell_literal(get_commitments, X, _), Ls),
    \+ memberchk(V, Ls).
% spec:50, api:30, handoff:37: the version is a u32 (the cap is "u32::MAX - 768"):
% the code's version fields are u32.
c4_problem(version_width(S, F, T)) :-
    member(S-F, ['Pool'-version, 'Request'-version]),
    \+ code_field(S, F, u32), T = not_u32.
c4_problem(version_max(doc(V), model(M))) :-
    constant(version_max, V, _), u32_max(M), V =\= M.
% spec:50, handoff:37: the reserve is three increments per retained record, so
% VERSION_CAP = u32::MAX - 3 * MAX_RECORDS (derived), and it is below u32::MAX
% (C-2's bump_below_cap and submit_version_add rely on that).
c4_problem(version_reserve(doc(C), derived(D))) :-
    constant(version_cap, C, _), constant(version_max, M, _), constant(max_records, R, _),
    D is M - 3 * R, C =\= D.
c4_problem(version_cap_not_below_max(C, M)) :-
    constant(version_cap, C, _), constant(version_max, M, _), \+ C < M.
% spec:9 window fields (v1.1.0). Their widths are CODE-ONLY: first_id is a u32
% id; completed_units is one u32 per recipe, saturating (lib.rs:519; the trace
% contract's min(u32::MAX, ..)).
c4_problem(window_field(S, F, T)) :-
    member(S-F-T, ['Pool'-first_id-u32, 'Pool'-completed_units-vec(u32), 'Commitment'-id-u32]),
    \+ code_field(S, F, T).
c4_problem(model_version_width(W)) :- model_constant(version_width, W, _), W =\= 32.
% Time sums saturate at u64::MAX: the code's time fields are u64.
c4_problem(time_width(S, F)) :-
    member(S-F, ['Pool'-observed_at, 'Request'-expires_at, 'Commitment'-expires_at]),
    \+ code_field(S, F, u64).
c4_problem(model_time_width(W)) :- model_constant(time_width, W, _), W =\= 64.
% spec:13 derived bounds.
c4_problem(max_residual(doc(R), derived(D))) :-
    constant(max_residual, R, _), constant(max_value, V, _), D is 2 * V, R =\= D.
c4_problem(max_energy(doc(E), derived(D))) :-
    constant(max_energy, E, _), constant(max_terms, T, _), constant(max_weight, W, _),
    constant(max_residual, R, _), D is T * W * R * R, E =\= D.
c4_problem(energy_not_below_u64(E, U)) :-
    constant(max_energy, E, _), constant(u64_max, U, _), \+ E < U.
c4_problem(u64_mismatch(doc(U), model(M))) :- constant(u64_max, U, _), u64_max(M), U =\= M.
c4_problem(js_range(V, J)) :-
    constant(max_value, V, _), constant(js_exact_max, J, _), \+ V =< J.
% Every code constant is compared or listed (docs silent: CODE-ONLY).
c4_problem(code_constant_unaccounted(C)) :-
    code_constant(C, _, _), \+ doc_code_constant(_, C), \+ model_code_constant(_, C).

%%% ========================================================
%%% C-5  Fixture conservation on trace configs 1-5 (the fixtures)
%%% ========================================================

%% Known findings (pool_req known_finding/2): the claim's counterexample must be
%% REPRODUCED; it is then listed under findings(..) in Info, not as a problem.
%% If it no longer reproduces, that is a problem (re-examine the finding).
%% v1.1.0: there is none (F-2 and F-4 are fixed); every claim must hold.
c_problems('C-5', Ps, Info) :-
    fixture_traces(Ts0),
    findall(release(F, St, Cx, TSt, TCx),
            ( fixture(F, _, _), fixture_claim(_, F, release_never_blocked, _),
              member(F-T, Ts0), release_results(T, St, Cx, TSt, TCx) ), Rel),
    append(Ts0, Rel, Ts),
    findall(P, c5_problem(Ts, P), Ps),
    findall(Id-Res, ( fixture_claim(Id, F, C, _), claim_info(Ts, F, C, Res) ), Res0),
    findall(Id-Fd-Why, ( known_finding(Id, Fd), fixture_claim(Id, F, C, _),
                         once(claim_problem(Ts, F, C, Why)) ), Fs),
    Info = [claims(Res0), findings(Fs)].

%% fixture_traces(-Ts): [Name-trace(..)] for traces 1..5, read once (the v1.1.0
%% trace contract keeps those numbers: the generator picks the fixtures by name).
fixture_traces(Ts) :-
    trace_file(File),
    catch(fold_traces(File, keep_fixture, [], _), done(Acc), true),
    findall(Name-T, ( member(T, Acc), T = trace(N, _, _, _, _), fixture(Name, N, _) ), Ts).

keep_fixture(T, A0, A) :-
    T = trace(N, _, _, _, _),
    (   N > 5 -> throw(done(A0))
    ;   N >= 1 -> A = [T | A0]
    ;   A = A0
    ).

fx_cfg(Ts, F, Cfg) :- member(F-trace(_, Cfg, _, _, _), Ts).
fx_trace(Ts, F, T) :- member(F-T, Ts).
fx_idx(F, Name, I) :- fixture_metric(F, Name, I, _).
recipes_of(config(_, _, _, _, Rs), Rs).

% v1.1.0 (conformance/src/main.rs, trace contract): traces 1-5 are the fixtures
% chosen by name, no longer fixtures::NAMES[N-1] (NAMES now starts with the
% station, trace 212). Every fixture/3 is in NAMES, and every NAMES entry is a
% fixture/3 or a recorded fixture without claims.
c5_problem(_, fixture_not_in_code(F)) :-
    fixture(F, _, _), fixture_names(Ns, _), \+ memberchk(F, Ns).
c5_problem(_, fixture_unaccounted(F)) :-
    fixture_names(Ns, _), member(F, Ns), \+ fixture(F, _, _), \+ fixture_without_claims(F, _).
c5_problem(_, fixture_trace_number_repeated(N)) :-
    fixture(F, N, _), fixture(G, N, _), F @< G.
% The release search must admit something, or release_never_blocked is vacuous.
c5_problem(Ts, release_search_vacuous(F)) :-
    member(release(F, [pairs_admitted(0) | _], _, _, _), Ts).
c5_problem(Ts, fixture_trace_missing(F)) :- fixture(F, _, _), \+ fx_cfg(Ts, F, _).
c5_problem(Ts, metric_index(F, Name, I)) :-
    fixture_metric(F, Name, I, _), fx_cfg(Ts, F, config(_, _, Ms, _, _)),
    length(Ms, N), \+ ( I >= 0, I < N ).
c5_problem(Ts, claim(Id, F, Why)) :-
    fixture_claim(Id, F, C, _), \+ known_finding(Id, _),
    claim_problem(Ts, F, C, Why).
c5_problem(Ts, known_finding_not_reproduced(Id, Fd)) :-
    known_finding(Id, Fd), fixture_claim(Id, F, C, _),
    \+ claim_problem(Ts, F, C, _).
c5_problem(_, known_finding_unknown_claim(Id)) :- known_finding(Id, _), \+ fixture_claim(Id, _, _, _).
c5_problem(_, known_finding_undefined(Id, Fd)) :- known_finding(Id, Fd), \+ finding(Fd, _, _, _).
% Account renaming (conformance main.rs:219-230): consistent with fixtures.rs order.
c5_problem(Ts, renaming(F, expected(E), trace(A))) :-
    fixture_recipe_actors(F, Names), fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    findall(X, member(r(X, _, _, _, _, _), Rs), A),
    rename_actors(Names, [], E),
    A \== E.

rename_actors([], _, []).
rename_actors([N | Ns], Seen, [A | As]) :-
    (   member(N-A0, Seen) -> A = A0, Seen1 = Seen
    ;   ( Seen == [] -> A = a1 ; A = a2 ), Seen1 = [N-A | Seen]
    ),
    rename_actors(Ns, Seen1, As).

eff(reserve, r(_, _, Res, _, _, _), Res).
eff(complete, r(_, _, _, Com, _, _), Com).

coef(F, Kind, R, Name, V) :- fx_idx(F, Name, I), eff(Kind, R, E), nth0(I, E, V).

sum_named(F, Names, Vec, S) :-
    findall(X, ( member(Nm, Names), fx_idx(F, Nm, I), nth0(I, Vec, X) ), Xs), sum_list(Xs, S).

%% claim_problem(+Ts, +Fixture, +Claim, -Why): enumerates what is wrong.
claim_problem(Ts, F, conserved(Names, Total), Why) :-
    fx_trace(Ts, F, trace(_, Cfg, _, Init, Calls)), st_parts(Init, V0, _, _, _),
    recipes_of(Cfg, Rs),
    (   sum_named(F, Names, V0, S0), S0 =\= Total, Why = initial_sum(S0)
    ;   nth0(J, Rs, R), member(K, [reserve, complete]), eff(K, R, E),
        sum_named(F, Names, E, S), S =\= 0, Why = recipe_not_conserving(J, K, S)
    ;   % dynamic: every call other than observe keeps the sum (the real core's trace)
        conserve_walk(Calls, F, Names, V0, 1, Why)
    ).
claim_problem(Ts, F, reserve_moves(Srcs, decrease, T, increase), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    (   nth0(J, Rs, R), coef(F, reserve, R, T, X), X < 0, Why = transit_decreases(J)
    ;   nth0(J, Rs, R), member(Sn, Srcs), coef(F, reserve, R, Sn, X), X > 0, Why = source_increases(J, Sn)
    ;   \+ ( member(R, Rs), coef(F, reserve, R, T, X), X > 0 ), Why = vacuous_no_transfer
    ).
claim_problem(Ts, F, complete_moves(T, decrease, Ds, increase), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    (   nth0(J, Rs, R), coef(F, complete, R, T, X), X > 0, Why = source_increases(J, T)
    ;   nth0(J, Rs, R), member(D, Ds), coef(F, complete, R, D, X), X < 0, Why = destination_decreases(J, D)
    ;   \+ ( member(R, Rs), coef(F, complete, R, T, X), X < 0 ), Why = vacuous_no_completion_move
    ).
claim_problem(Ts, F, no_complete_effect(Names), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, R), member(Nm, Names), coef(F, complete, R, Nm, X), X =\= 0,
    Why = completion_moves(J, Nm, X).
claim_problem(Ts, F, no_reserve_credit(Names), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, R), member(Nm, Names), coef(F, reserve, R, Nm, X), X > 0,
    Why = reservation_credits(J, Nm, X).
claim_problem(Ts, F, requires(Who, Pre), Why) :-
    (   \+ fixture_recipe_actors(F, _) -> Why = no_recipe_order_extracted
    ;   fixture_recipe_actors(F, Names), fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
        (   \+ nth0(_, Names, Who) -> Why = no_recipe(Who)
        ;   nth0(I, Names, Who), nth0(I, Rs, r(_, _, _, _, _, Req)),
            findall(p(J, U), ( member(Nm-U, Pre), nth0(J, Names, Nm) ), Exp0), sort(Exp0, Exp),
            sort(Req, Got),
            Exp \== Got, Why = requires(expected(Exp), config(Got))
        )
    ).
claim_problem(Ts, F, actor_not_executor, Why) :-
    (   fixture_executor(X, _), fixture_actors(F, As), memberchk(X, As), Why = executor_is_actor(X)
    ;   \+ fixture_actors(F, _), Why = no_actors_extracted
    ;   fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs), nth0(J, Rs, r(A, X, _, _, _, _)), A == X,
        Why = trace_recipe_self(J)
    ).
claim_problem(Ts, F, reserve_sign(Names, nonneg), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, R), member(Nm, Names), coef(F, reserve, R, Nm, X), X < 0,
    Why = negative_reservation(J, Nm, X).
claim_problem(Ts, F, complete_sign(Names, nonpos), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, R), member(Nm, Names), coef(F, complete, R, Nm, X), X > 0,
    Why = positive_completion(J, Nm, X).
claim_problem(Ts, F, no_complete_effect_all, Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, r(_, _, _, Com, _, _)), member(X, Com), X =\= 0,
    Why = completion_effect(J).
claim_problem(Ts, F, paired_reserve(Pairs), Why) :-
    fx_cfg(Ts, F, Cfg), recipes_of(Cfg, Rs),
    nth0(J, Rs, R), member(B-S, Pairs),
    coef(F, reserve, R, B, XB), coef(F, reserve, R, S, XS), XB =\= XS,
    Why = unpaired(J, B-XB, S-XS).
claim_problem(Ts, F, pending_negative_no_headroom, Why) :-
    fx_trace(Ts, F, T),
    headroom_problem(T, Why).
claim_problem(Ts, F, release_never_blocked, Why) :-
    (   \+ member(release(F, _, _, _, _), Ts) -> Why = no_release_search
    ;   member(release(F, _, Cx, _, TCx), Ts),
        (   Cx = [C | _], length(Cx, N), Why = release_blocked(count(N), first(C))
        ;   TCx = [C | _], length(TCx, N), Why = release_blocked_in_trace(count(N), first(C))
        )
    ).
% CASH-SELF-ENFORCED: Config::validate compares the two roles (extracted;
% v1.1.0 lib.rs:130 `r.actor == r.executor` => InvalidConfig).
claim_problem(_, _, validate_rejects(A-B), Why) :-
    code_role_checks(validate, Ps), \+ memberchk(A-B, Ps),
    Why = validate_accepts(A = B, code_role_checks(validate, Ps)).

%% release_results(+Trace, -Stats, -Cex, -TraceStats, -TraceCex)
%% (a) Bounded search from the fixture's init state (the code's, from the
%%     trace): every admissible request (recipe, units 1..max) by its actor,
%%     then every admissible second request; then for each of the two records
%%     X, with the other record Y left Reserved or dispatched and settled
%%     completed by its executor: release X by its actor's cancel, and by
%%     dispatch + its executor's settle(false). A release that is not Ok is a
%%     counterexample (v1.1.0 spec:48 "the checked inverse cannot fail in a
%%     reachable state", spec:70 for risk; at v1.0.0 the claim was
%%     spec@v1.0.0:46's last sentence, broken in risk: F-2).
%% (b) Every state of the fixture's own trace before its first observe (so the
%%     base is the fixture's in-bounds initial values): every open record's
%%     release (cancel if Reserved, settle(false) by the executor if in flight)
%%     must be Ok.
release_results(trace(_, Cfg, Now, Init, Calls), [pairs_admitted(NP), releases_tried(NA)], Cx,
                [trace_states(NS), trace_releases(NT)], TCx) :-
    trace_to_pool(Cfg, Init, S0),
    Exp is Now + 100,
    findall(S2-p(I, U, J, V),
            ( fx_req(Cfg, I, U, A1), step(S0, submit(A1, I, U, 0, Exp, Now), ok(0), S1),
              fx_req(Cfg, J, V, A2), step(S1, submit(A2, J, V, 1, Exp, Now), ok(1), S2) ), Pairs),
    length(Pairs, NP),
    findall(rel(Pr, release(X), other(YSt), via(How))-R,
            ( member(S2-Pr, Pairs), release_attempt(Cfg, S2, Now, X, YSt, How, R) ), Atts),
    length(Atts, NA),
    findall(A-R, ( member(A-R, Atts), R \== ok ), Cx),
    states_before_observe(Init, Calls, Sts),
    length(Sts, NS),
    findall(K-rel(Id, St0, R), ( nth0(K, Sts, St), trace_to_pool(Cfg, St, S), trace_release(Cfg, St, S, Now, rel(Id, St0, R)) ), TAtts),
    length(TAtts, NT),
    findall(state(K)-Rel, ( member(K-Rel, TAtts), Rel = rel(_, _, R), R \== ok ), TCx).

fx_req(Cfg, I, U, A) :-
    recipes_of(Cfg, Rs), length(Rs, N), N1 is N - 1,
    between(0, N1, I), nth0(I, Rs, r(A, _, _, _, MU, _)), between(1, MU, U).

release_attempt(Cfg, S2, Now, X, YSt, How, R) :-
    member(X-Y, [0-1, 1-0]),
    record_of(S2, X, c(RX, _, _, _)), record_of(S2, Y, c(RY, _, _, _)),
    actor_of(Cfg, RX, AX), executor_of(Cfg, RX, XX),
    actor_of(Cfg, RY, AY), executor_of(Cfg, RY, XY),
    (   YSt = reserved, S3 = S2
    ;   YSt = completed, step(S2, dispatch(AY, Y, Now), ok, S2a), step(S2a, settle(XY, Y, true), ok, S3)
    ),
    (   How = cancel, step(S3, cancel(AX, X, Now), R, _)
    ;   How = settle_false, step(S3, dispatch(AX, X, Now), ok, S4), step(S4, settle(XX, X, false), R, _)
    ).

%% states_before_observe(+Init, +Calls, -States): the init state and the
%% post-states of the calls before the first Ok observe.
states_before_observe(Init, Calls, [Init | Sts]) :- before_obs(Calls, Sts).
before_obs([], []).
before_obs([call(Op, R, St) | Cs], Out) :-
    (   Op = observe(_, _, _), R == ok -> Out = []
    ;   Out = [St | Out1], before_obs(Cs, Out1)
    ).

%% trace_release(+Cfg, +TraceState, +Pool, +Now, -rel(Id, Status, Result)):
%% an open record of the trace state and its release in the model (Pool is
%% trace_to_pool of the same state): cancel by the actor if Reserved, else
%% settle(false) by the executor.
trace_release(Cfg, TSt, S, Now, rel(Id, St, R)) :-
    trace_open_record(TSt, Id, RI, St),
    (   St == reserved -> actor_of(Cfg, RI, A), step(S, cancel(A, Id, Now), R, _)
    ;   executor_of(Cfg, RI, X), step(S, settle(X, Id, false), R, _)
    ).

%% trace_open_record(+TraceState, -Id, -Recipe, -Status): an open record of
%% the trace state with its id. Record j of the state has id FirstId + j
%% (spec:9, spec:27: the window keeps the most recent records; s/7's sixth
%% position); a v1.0.0 s/5 state has no pruned prefix (FirstId 0). The list
%% index alone is the id only while FirstId = 0.
trace_open_record(TSt, Id, RI, St) :-
    st_parts(TSt, _, _, _, Rs), st_first_id(TSt, F),
    nth0(J, Rs, c(RI, _, _, St)), open_status(St), Id is F + J.

%% conserve_walk: every non-observe call keeps sum(Names); report the first that does not.
conserve_walk([call(Op, _, St) | Cs], F, Names, V0, I, Why) :-
    st_parts(St, V1, _, _, _),
    sum_named(F, Names, V0, S0), sum_named(F, Names, V1, S1),
    (   Op \= observe(_, _, _), S0 =\= S1 -> Why = not_conserved_by_call(I, Op, S0, S1)
    ;   I1 is I + 1, conserve_walk(Cs, F, Names, V1, I1, Why)
    ).

%% st_parts(+TraceState, -Values, -Version, -ObservedAt, -Records): the fields
%% C-5 reads, from v1.0.0's s/5 or v1.1.0's s/7 (the trace contract adds
%% FirstId and DoneUnits after the five unchanged positions).
st_parts(s(V, N, O, _, R), V, N, O, R).
st_parts(s(V, N, O, _, R, _, _), V, N, O, R).
st_first_id(s(_, _, _, _, _), 0).
st_first_id(s(_, _, _, _, _, F, _), F).

%% RISK-HEADROOM (spec:70, handoff:23): in every state of the risk trace and for
%% every (recipe, units), (a) the envelope's high endpoint is the same with and
%% without the open records' negative completion parts (pending hedges add
%% nothing to capacity), and (b) a request admissible with them is admissible
%% without them.
headroom_problem(trace(_, Cfg, _, Init, Calls), Why) :-
    findall(St, ( St = Init ; member(call(_, _, St), Calls) ), Sts),
    recipes_of(Cfg, Rs), length(Rs, NR), NR1 is NR - 1,
    member(St, Sts), trace_to_pool(Cfg, St, S),
    st_parts(St, V, Ver, Obs, Recs),
    between(0, NR1, RI), nth0(RI, Rs, r(A, _, _, _, MU, _)),
    between(1, MU, U),
    recipe_of(Cfg, RI, r(_, _, Res, _, _, _)),
    (   add_vec(V, Res, U, Next),
        envelope_opts(Cfg, Recs, Next, extra(RI, U), none, [], _, H1),
        envelope_opts(Cfg, Recs, Next, extra(RI, U), none, [ignore_pending_negative], _, H2),
        H1 \== H2,
        Why = high_endpoint_netted(St, RI, U)
    ;   Now = Obs, Exp is Now + 1,
        admissible(S, A, req(RI, U, Ver, Exp, Now)),
        \+ admissible_opts(S, A, req(RI, U, Ver, Exp, Now), [ignore_pending_negative]),
        Why = headroom_from_pending(St, RI, U)
    ).

%% claim_info: non-vacuity counts per claim.
claim_info(Ts, F, conserved(Names, _), [non_observe_calls(N), first_observe_at(K)]) :- !,
    fx_trace(Ts, F, trace(_, _, _, _, Calls)),
    count_where(x, ( member(call(C, _, _), Calls), C \= observe(_, _, _) ), N),
    (   nth1(K, Calls, call(observe(_, _, _), ok, _)) -> true ; K = none ),
    Names = [_ | _].
claim_info(Ts, F, pending_negative_no_headroom, [states(NS), states_with_pending_negative(NP), admissible_requests(NA)]) :- !,
    fx_trace(Ts, F, trace(_, Cfg, _, Init, Calls)),
    findall(St, ( St = Init ; member(call(_, _, St), Calls) ), Sts), length(Sts, NS),
    count_where(x, ( member(St, Sts), st_parts(St, _, _, _, Recs), pending_negative(Cfg, Recs) ), NP),
    recipes_of(Cfg, Rs), length(Rs, NR), NR1 is NR - 1,
    count_where(x, ( member(St, Sts), trace_to_pool(Cfg, St, S), st_parts(St, _, Ver, Obs, _),
                          between(0, NR1, RI), nth0(RI, Rs, r(A, _, _, _, MU, _)), between(1, MU, U),
                          Exp is Obs + 1, admissible(S, A, req(RI, U, Ver, Exp, Obs)) ), NA).
claim_info(Ts, F, release_never_blocked, [St, TSt, blocked(NC), trace_blocked(NT)]) :- !,
    member(release(F, St, Cx, TSt, TCx), Ts), length(Cx, NC), length(TCx, NT).
claim_info(_, _, validate_rejects(_), [code_role_checks(Ps)]) :- !, code_role_checks(validate, Ps).
claim_info(_, _, _, static).

pending_negative(Cfg, Recs) :-
    member(c(RI, _, _, St), Recs), open_status(St),
    recipe_of(Cfg, RI, r(_, _, _, Com, _, _)), member(X, Com), X < 0, !.

%%% ========================================================
%%% C-6  Traceability
%%% ========================================================
%% The property catalogue is projects/shared-pool/strategy.md section 5, which
%% is committed. Its IDs are the first cell of every markdown table row between
%% the heading "## 5." and the next "## " heading (each table's header and
%% separator rows skipped), and its "Layer(s)" column is the property's layer
%% list. C-6 requires those IDs to be exactly traceability.pl's property/3 IDs
%% (none missing, none extra, none repeated on either side) and each row's
%% layer list to equal property/3's, in the same order. A missing strategy.md,
%% a missing section 5 or an empty ID set is a C-6 failure, never a skip.

c_problems('C-6', Ps, Info) :-
    findall(P, c6_problem(P), Ps0),
    strategy_path(SP),
    (   file_exists(SP)
    ->  strategy_catalogue(SP, Rows, SPs)
    ;   atom_chars(SA, SP), Rows = [], SPs = [strategy_file_missing(SA)]
    ),
    findall(CId-CLs, property(CId, CLs, _), Cat),
    catalogue_problems(Rows, Cat, CPs),
    append([SPs, CPs, Ps0], Ps),
    length(Rows, NS),
    count_where(x, ( member(row(_, RId, RLs), Rows), memberchk(RId-RLs, Cat) ), NL),
    count_where(x, property(_, _, _), NP),
    count_where(x, requirement(_, _, _), NR),
    count_where(x, threat(_, _, _, _), NT),
    count_where(x, finding(_, _, _, _, open), NFO),
    count_where(x, finding(_, _, _, _, fixed(_, _)), NFX),
    count_where(x, divergence(_, _, _, _), NDO),
    count_where(x, closed_divergence(_, _, _, _, _), NDC),
    findall(L-N, ( layer(L), count_where(x, ( property(_, Ls, _), memberchk(L, Ls) ), N) ), PerLayer),
    Info = [properties(NP), strategy_rows(NS), layers_equal(NL), requirements(NR), threats(NT),
            findings(open(NFO), fixed(NFX)), divergences(open(NDO), closed(NDC)), per_layer(PerLayer)].

c6_problem(no_layer(Id)) :- property(Id, [], _).
c6_problem(unknown_layer(Id, L)) :- property(Id, Ls, _), member(L, Ls), \+ layer(L).
c6_problem(duplicate_property(Id)) :-
    findall(I, property(I, _, _), Is), msort_dup(Is, S), append(_, [Id, Id | _], S).
c6_problem(requirement_untraced(R)) :- requirement(R, _, _), \+ verified_by(R, _).
c6_problem(trace_row_unknown_requirement(R)) :- verified_by(R, _), \+ requirement(R, _, _).
c6_problem(trace_row_unknown_property(R, P)) :- verified_by(R, P), \+ property(P, _, _).
c6_problem(threat_unmitigated(T)) :- threat(T, _, _, _), \+ mitigated_by(T, _).
c6_problem(threat_unknown_property(T, P)) :- mitigated_by(T, P), \+ property(P, _, _).
c6_problem(mitigation_unknown_threat(T)) :- mitigated_by(T, _), \+ threat(T, _, _, _).
c6_problem(assumption_not_in_catalogue(A)) :- assumption(A, _, _), \+ property(A, [assumption], _).
c6_problem(catalogue_assumption_undefined(A)) :- property(A, [assumption], _), \+ assumption(A, _, _).
c6_problem(prolog_property_without_evidence(P)) :-
    property(P, Ls, _), memberchk(prolog, Ls), \+ prolog_evidence(P, _).
c6_problem(evidence_for_non_prolog_property(P)) :-
    prolog_evidence(P, _), \+ ( property(P, Ls, _), memberchk(prolog, Ls) ).
c6_problem(divergence_untraced(D)) :-
    divergence(D, Cite, _, _), \+ atom(Cite).
c6_problem(closed_divergence_untraced(D)) :-
    closed_divergence(D, Old, New, _, _), \+ ( atom(Old), atom(New) ).
% Every OPEN finding is named in some threat's residual (threat_model.pl).
c6_problem(finding_without_threat(F)) :-
    finding(F, _, _, _, open), \+ named_in_residual(F).
% A fixed finding names the catalogue properties that show the fix (at least
% one); none of them may still list it as a residual.
c6_problem(fixed_finding_without_evidence(F)) :- finding(F, _, _, _, fixed(_, [])).
c6_problem(fixed_finding_unknown_property(F, P)) :-
    finding(F, _, _, _, fixed(_, Ps)), member(P, Ps), \+ property(P, _, _).
c6_problem(fixed_finding_still_residual(F)) :-
    finding(F, _, _, _, fixed(_, _)), named_in_residual(F).
c6_problem(finding_status(F, S)) :-
    finding(F, _, _, _, S), \+ ( S == open ; S = fixed(V, Ps), atom(V), ground(Ps), length(Ps, _) ).

%% named_in_residual(+F): F (e.g. 'F-2', not 'F-2b' or 'F-21') occurs in a residual.
named_in_residual(F) :-
    atom_chars(F, FC),
    threat(_, _, _, Res), Res \== none, append(_, Tail, Res), append(FC, After, Tail),
    \+ ( After = [D | _], char_code(D, K),
         ( K >= 0'0, K =< 0'9 ; K >= 0'a, K =< 0'z ) ), !.

strategy_path(P) :- root_path("../strategy.md", P).

%% catalogue_problems(+Rows, +Catalogue, -Problems): Rows = [row(Line, Id,
%% Layers)] from strategy.md section 5; Catalogue = [Id-Layers] from
%% property/3. Problems = [] exactly when the two agree and Rows is not empty.
catalogue_problems(Rows, Cat, Ps) :-
    findall(Id, member(row(_, Id, _), Rows), SIds0), sort(SIds0, SIds),
    findall(Id, member(Id-_, Cat), CIds0), sort(CIds0, CIds),
    findall(P, catalogue_problem(Rows, Cat, SIds, CIds, P), Ps1),
    (   Rows == [] -> Ps = [strategy_no_ids | Ps1] ; Ps = Ps1 ).

catalogue_problem(_, _, SIds, CIds, catalogue_id_not_in_strategy(Id)) :-
    member(Id, CIds), \+ memberchk(Id, SIds).
catalogue_problem(Rows, _, _, CIds, strategy_id_not_in_catalogue(Id, line(N))) :-
    member(row(N, Id, _), Rows), \+ memberchk(Id, CIds).
catalogue_problem(Rows, _, SIds, _, strategy_duplicate_id(Id, lines(Ns))) :-
    member(Id, SIds), findall(N, member(row(N, Id, _), Rows), Ns), Ns = [_, _ | _].
catalogue_problem(Rows, Cat, _, _, layers_differ(Id, line(N), strategy(SLs), catalogue(CLs))) :-
    member(row(N, Id, SLs), Rows), memberchk(Id-CLs, Cat), SLs \== CLs.

%% strategy_catalogue(+File, -Rows, -Problems): catalogue_rows/3 of the file.
strategy_catalogue(File, Rows, Ps) :-
    phrase_from_file(seq(Cs), File),
    catalogue_rows(Cs, Rows, Ps).

%% catalogue_rows(+Chars, -Rows, -Problems): Rows = [row(Line, Id, Layers)] in
%% file order, one per body row of every table in section 5 ("## 5." up to the
%% next "## " heading). A table is a run of lines that start with '|'. Its
%% first row is the header (first cell "ID", one cell "Layer(s)"); its second
%% is the separator (cells of '-' and ':'). Problems name the line of a table
%% without a separator or with another header, a row whose first cell is not a
%% bare ID, and a row with no Layer(s) cell (its Layers are `none`).
catalogue_rows(Cs, Rows, Ps) :-
    text_lines(Cs, 1, Ls),
    (   append(_, [_-H | After], Ls), append("## 5.", _, H)
    ->  section_lines(After, Sec),
        table_blocks(Sec, Blocks),
        blocks_rows(Blocks, Rows, Ps)
    ;   Rows = [], Ps = [strategy_section_5_missing]
    ).

text_lines([], _, []) :- !.
text_lines(Cs, N, [N-L | Ls]) :-
    line_chars(Cs, L, Rest), N1 is N + 1, text_lines(Rest, N1, Ls).

line_chars([], [], []).
line_chars([C | Cs], L, Rest) :-
    (   C == '\n' -> L = [], Rest = Cs ; L = [C | L1], line_chars(Cs, L1, Rest) ).

section_lines([], []).
section_lines([N-L | Ls], Sec) :-
    (   append("## ", _, L) -> Sec = [] ; Sec = [N-L | Sec1], section_lines(Ls, Sec1) ).

table_blocks([], []).
table_blocks([N-L | Ls], Bs) :-
    (   L = [B | _], bar(B)
    ->  take_table(Ls, More, Rest), Bs = [[N-L | More] | Bs1], table_blocks(Rest, Bs1)
    ;   table_blocks(Ls, Bs)
    ).

take_table([N-L | Ls], [N-L | More], Rest) :- L = [B | _], bar(B), !, take_table(Ls, More, Rest).
take_table(Rest, [], Rest).

blocks_rows([], [], []).
blocks_rows([B | Bs], Rows, Ps) :-
    block_rows(B, Rows1, Ps1),
    blocks_rows(Bs, Rows2, Ps2),
    append(Rows1, Rows2, Rows), append(Ps1, Ps2, Ps).

block_rows([HN-H, _-S | Body], Rows, Ps) :-
    row_cells(S, SC), separator_row(SC), !,
    row_cells(H, HC),
    (   HC = ["ID" | _], nth0(LI, HC, "Layer(s)")
    ->  body_rows(Body, LI, Rows, Ps)
    ;   maplist(atom_chars, HA, HC), Rows = [], Ps = [strategy_table_header(line(HN), HA)]
    ).
block_rows([N-_ | _], [], [strategy_table_without_separator(line(N))]).

body_rows([], _, [], []).
body_rows([N-L | Ls], LI, Rows, Ps) :-
    row_cells(L, Cells),
    Cells = [IdC | _],
    (   bare_id(IdC)
    ->  atom_chars(Id, IdC),
        (   nth0(LI, Cells, LC)
        ->  layer_list(LC, Lay), Ps1 = []
        ;   Lay = none, length(Cells, K), Ps1 = [strategy_row_without_layers(line(N), Id, cells(K))]
        ),
        Rows = [row(N, Id, Lay) | Rows1]
    ;   atom_chars(A, IdC), Rows = Rows1, Ps1 = [strategy_row_not_a_bare_id(line(N), A)]
    ),
    body_rows(Ls, LI, Rows1, Ps2),
    append(Ps1, Ps2, Ps).

%% row_cells(+Line, -Cells): the trimmed cells of "| a | b |"; "\|" inside a
%% cell is not a separator.
row_cells(L, Cells) :-
    trim(L, L1),
    (   L1 = [B | L2], bar(B) -> true ; L2 = L1 ),
    (   append(L3, [B2], L2), bar(B2), \+ ( append(_, [E], L3), backslash(E) ) -> true ; L3 = L2 ),
    split_cells(L3, Cells0),
    maplist(trim, Cells0, Cells).

split_cells(Cs, [Cell | Cells]) :-
    cell_chars(Cs, Cell, Rest),
    (   Rest = [_ | Rest1] -> split_cells(Rest1, Cells) ; Cells = [] ).

cell_chars([], [], []).
cell_chars([C | Cs], Cell, Rest) :-
    (   bar(C) -> Cell = [], Rest = [C | Cs]
    ;   backslash(C), Cs = [B | Cs1], bar(B) -> Cell = [C, B | Cell1], cell_chars(Cs1, Cell1, Rest)
    ;   Cell = [C | Cell1], cell_chars(Cs, Cell1, Rest)
    ).

% '|' and '\\' as char codes: library(dcgs) makes '|' an operator, which the
% reader then rejects as an operand before '->'.
bar(C) :- char_code(C, 124).
backslash(C) :- char_code(C, 92).

separator_row(Cells) :-
    Cells = [_ | _],
    \+ ( member(C, Cells), \+ separator_cell(C) ).
separator_cell(C) :- memberchk('-', C), \+ ( member(X, C), \+ memberchk(X, "-:") ).

%% layer_list(+Cell, -Layers): "kani, near-tests, prolog" -> [kani, 'near-tests', prolog].
layer_list([], []) :- !.
layer_list(Cell, Ls) :-
    split_on(',', Cell, Parts), maplist(trim, Parts, Ts), maplist(atom_chars, Ls, Ts).

split_on(Sep, Cs, [P | Ps]) :-
    (   append(P0, [Sep | Rest], Cs) -> P = P0, split_on(Sep, Rest, Ps) ; P = Cs, Ps = [] ).

%% bare_id(+Chars): an ID such as C-1, ASM-PRED or TLA-LIVE-F1: an upper-case
%% letter, then upper-case letters, digits and hyphens, with at least one
%% hyphen and not ending in one.
bare_id([C | Cs]) :-
    upper(C), \+ ( member(X, Cs), \+ id_char(X) ),
    memberchk('-', Cs), last_of([C | Cs], Last), Last \== ('-').

id_char(C) :- char_code(C, K), ( K >= 0'A, K =< 0'Z ; K >= 0'0, K =< 0'9 ; K =:= 45 ).
upper(C) :- char_code(C, K), K >= 0'A, K =< 0'Z.

trim(Cs, T) :- drop_ws(Cs, C1), reverse(C1, R1), drop_ws(R1, R2), reverse(R2, T).
drop_ws([C | Cs], T) :- ws(C), !, drop_ws(Cs, T).
drop_ws(Cs, Cs).
ws(' ').  ws('\t').  ws('\r').
