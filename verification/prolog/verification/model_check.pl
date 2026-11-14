%% model_check.pl -- MC-1: bounded exploration of small universes through step/4
%%
%% PROP: MC-1 LIVE-2 LIVE-1 INV-WF INV-LEDGER INV-HARD INV-HEALTHY INV-DEP INV-VER
%% PROP: TX-0 TX-NOOP TX-VER AUTH-1 STALE-1 ADM-1 ADM-2 HARD-STEP DEP-1 LIFE-1
%% PROP: ASYNC-1 TERM-1 OBS-1 PAUSE-1
%% PROP: INV-RES INV-VERCAP WIN-1 WIN-2 WIN-ESC LIVE-ALL VER-CAP OBS-2 REFRESH-1
%%
%% A universe fixes a config, a finite set of times, observation vectors, a
%% window and the callers (the recipe's actors and executors, the owner, and
%% a stranger z). From init, every generated call is applied with step/4
%% (breadth first). Every edge is checked with props:edge_violation/5 and every
%% new state with props:state_violation/3 and props:probe_violation/3 (LIVE-1,
%% LIVE-ALL, REFRESH-1, WIN-ESC).
%%
%% Window. Each universe runs under pool_machine:with_window_size/2 with its
%% window(W) (default: the code's 256), so the model prunes and fails Full at
%% W records and VERSION_CAP is u32::MAX - 3W. The window is restored when the
%% run ends (tests/test_mc1.pl checks).
%%
%% Exploration state n(Pool, Clock, Ghost). Time is monotone (ASM-TIME): a call
%% uses a time >= Clock from the universe's list. The visited key drops the
%% version and the ghost's effect counter: guards read the version only through
%% equality with the request (submits are generated with the current version and
%% with a stale one) and through VERSION_CAP, which these universes do not come
%% near, so states that differ only in version are bisimilar. INV-VER is
%% therefore checked per edge (TX-VER: +1 on every effective call, 0
%% otherwise), which is the induction step for it. The `cap` universe starts
%% near VERSION_CAP and keeps the version in its key (every behaviour there is
%% finite: gated calls stop at the cap, and INV-VERCAP bounds the rest).
%% The key also abstracts the window position: ids are kept relative to
%% first_id (calls name records by first_id + k, and the pruned id first_id - 1
%% when there is one), with min(first_id, 1); completed_units are capped at the
%% largest prerequisite of the config (prerequisites only compare with it).
%%
%% Truncation (universes with max_records(M)): at M records no submit is
%% generated; a state where some submit would have been Ok is marked truncated.
%% Such a universe never prunes. A universe without max_records generates
%% submits always and prunes at its window.
%%
%% Dead records (LIVE-2): record J is dead in state S if J is open in S and no
%% state reachable from S has J terminal. A state that can reach a truncated
%% state is never certified dead (the cut-off part might help). The analysis
%% names records by index, so it runs only on graphs where nothing was pruned
%% (first_id = 0 in every state); otherwise it reports not_applicable(pruned),
%% and LIVE-ALL, checked on every state, is the liveness evidence.
%%
%% Release blocked (F-2): a state whose ghost base is in bounds (B = 0, no
%% disturbing observe) in which the executor's settle(false) on an in-flight
%% record fails. Such a record still has settle(true), so it is not dead by
%% reachability; under ASM-EXEC (settle(false) only on definitive
%% nonexecution) it is the record the executor can never resolve honestly.
%%
%% Exploration stops at the first property violation (a violation fails the
%% run anyway; without the stop a mutated model can explode to max_states).
%%
%%   universe(Name, u(Config, Options))
%%   mc1_run(+Name, -Result)
%%   Result = mc1(Name, states(N), edges(E), truncated(T), violations(Vs),
%%                dead_inflight(D), dead_reserved(R), witness(Path), hits(H), complete(B),
%%                release_blocked(count(K), witness(Path2)), tags(Tags))
%%   Tags = [Tag-Count..]: the window, refresh and cap situations the run met
%%   (edge_tag/5), each with a witness path in the run's `witness_paths` (mc1_witness/3).

:- module(model_check, [universe/2, mc1_run/2, mc1_witness/3, edge_tag/5]).

:- use_module(library(lists)).
:- use_module(library(iso_ext)).
:- use_module(library(assoc)).
:- use_module(library(between)).
:- use_module(library(pairs)).
:- use_module('../requirements/pool_req').
:- use_module('../model/pool_machine').
:- use_module(props).
:- use_module(util).

:- dynamic(witness_path/3).

%%% ========================================================
%%% Universes
%%% ========================================================

% F-1 (brief; conformance trace 0): one recipe whose completion pushes metric 0
% up by 2 and whose reservation pulls it down by 2. Observations: a disturbing
% one (metric 0 at 12 > max 10) and an in-bounds one. v1.1.0: the hedge's
% completion is in the envelope, so in [12, 5] the submit fails Bounds.
universe(wedge, u(config(o, 0, [m(0, 10, 5), m(-100, 100, 5)], [t(1, 0, 1, target)],
                         [r(a1, x1, [-2, 0], [2, -5], 1, [])]),
                  [times([100, 200]), observations([[12, 5], [5, 5]]),
                   max_records(3), expiry(100), max_states(60000)])).
% Healthy: two recipes (the second requires a completed unit of the first),
% two actors and executors, freshness configured, only in-bounds observations.
universe(healthy, u(config(o, 100, [m(0, 10, 4), m(0, 6, 2)],
                          [t(0, 5, 1, target), t(1, 3, 2, deficit)],
                          [r(a1, x1, [1, 1], [1, -1], 2, []),
                           r(a2, x2, [2, 0], [-1, 1], 1, [p(0, 1)])]),
                    [times([100, 150, 250]), observations([[4, 2], [8, 5]]),
                     max_records(2), expiry(120), max_states(60000)])).

% F-2 scaled down (brief; risk fixture, docs/spec.md:66): r0 is a credit
% position (reserves +20 selloff loss, -5 collateral, no completion effect),
% r1 a hedge whose completion takes 70 off the selloff loss. At v1.0.0 the
% hedge was admissible while the position's reservation was counted (60 - 70
% < 0); once it completed, releasing the position put the loss at -10:
% Bounds. v1.1.0's envelope holds the position's release, so the hedge is not
% admitted beside it. Only in-bounds observations, so B = 0 throughout. Times
% 250 > expiry 200: an undispatched position dies.
universe(reliance, u(config(o, 0, [m(0, 100, 60), m(0, 40, 40)], [t(0, 80, 1, excess)],
                           [r(a1, x1, [20, -5], [0, 0], 1, []),
                            r(a2, x1, [0, -2], [-70, 0], 1, [])]),
                     [times([100, 250]), observations([[60, 40]]),
                      max_records(2), expiry(100), max_states(60000)])).

% F-3 scaled (spec:9, 21, 27): healthy's config with a window of 2 and no
% truncation, so submits prune a terminal front, fail Full on an open front,
% and recipe 1 is admitted on a pruned prerequisite (completed_units). Calls
% also name the pruned id first_id - 1 and the unissued id first_id + len.
universe(window, u(config(o, 100, [m(0, 10, 4), m(0, 6, 2)],
                         [t(0, 5, 1, target), t(1, 3, 2, deficit)],
                         [r(a1, x1, [1, 1], [1, -1], 2, []),
                          r(a2, x2, [2, 0], [-1, 1], 1, [p(0, 1)])]),
                   [times([100]), observations([[2, 1], [4, 2]]),
                    window(2), expiry(120), max_states(200000)])).
% The same with a window of 1: every submit after the first prunes or is Full.
universe(window1, u(config(o, 100, [m(0, 10, 4), m(0, 6, 2)],
                          [t(0, 5, 1, target), t(1, 3, 2, deficit)],
                          [r(a1, x1, [1, 1], [1, -1], 2, []),
                           r(a2, x2, [2, 0], [-1, 1], 1, [p(0, 1)])]),
                    [times([100, 150, 250]), observations([[4, 2], [8, 5]]),
                     window(1), expiry(120), max_states(200000)])).

% F-5 scaled (spec:50): healthy's config, window 2 (VERSION_CAP = u32::MAX - 6),
% starting 4 versions below the cap. The start is reachable: from Pool::new,
% set_paused(false) is Ok at every version below the cap and changes nothing
% but the version. The key keeps the version.
universe(cap, u(config(o, 100, [m(0, 10, 4), m(0, 6, 2)],
                      [t(0, 5, 1, target), t(1, 3, 2, deficit)],
                      [r(a1, x1, [1, 1], [1, -1], 2, []),
                       r(a2, x2, [2, 0], [-1, 1], 1, [p(0, 1)])]),
                [times([100, 150, 250]), observations([[4, 2], [8, 5]]),
                 window(2), below_cap(4), expiry(120), max_states(200000)])).

% F-2b (conformance trace 208): a reservation of -1e6 on metric 0 and a
% recipe completing +15,000 per unit. v1.0.0 admitted a completion that left
% the first record's release outside +-1e6 (cancel and settle(false) failed
% Arithmetic); v1.1.0's envelope bounds the release term. Units sampled
% around the boundaries (33/34 after observing 500,000; 66/67 from 0).
universe(f2b, u(config(o, 0, [m(-1000000, 1000000, 0), m(-10, 10, 0)], [t(1, 0, 1, target)],
                      [r(a1, x1, [-1000000, 0], [0, 0], 1, []),
                       r(a1, x1, [0, 0], [15000, 0], 100, [])]),
                [times([100]), observations([[0, 0], [500000, 0]]),
                 units(1, [33, 34, 66, 67]),
                 max_records(2), expiry(150), max_states(60000)])).

opt(Opts, Key) :- memberchk(Key, Opts).

universe_window(Opts, W) :- ( opt(Opts, window(W)) -> true ; constant(max_records, W, _) ).

%%% ========================================================
%%% Calls generated in a state
%%% ========================================================

ops(u(Cfg, Opts), n(S, Clock, _), Ops, AtCap) :-
    opt(Opts, times(Ts)), opt(Opts, observations(Obs)), opt(Opts, expiry(Ex)),
    findall(T, ( member(T, Ts), T >= Clock ), Nows),
    pool_version(S, Ver), pool_records(S, Recs), length(Recs, NRec),
    pool_first_id(S, F), pool_values(S, Cur),
    Cfg = config(Owner, _, _, _, Rs), length(Rs, NR), NR1 is NR - 1,
    findall(Op,
            ( member(RI, [0]), Op = submit(z, RI, 1, Ver, E, Clock), E is Clock + Ex
            ; Op = submit(A, 0, 1, V1, E, Clock), actor_of(Cfg, 0, A), V1 is Ver + 1, E is Clock + Ex
            ; between(0, NR1, RI), actor_of(Cfg, RI, A), unit_choice(Cfg, Opts, RI, U),
              member(Now, Nows), E is Now + Ex,
              Op = submit(A, RI, U, Ver, E, Now)
            ), Subs),
    (   opt(Opts, max_records(MaxR)), NRec >= MaxR -> AtCap = Subs, SubOps = [] ; AtCap = [], SubOps = Subs ),
    Last is F + NRec - 1,
    findall(Op,
            ( between(F, Last, Id), record_of(S, Id, c(RI, _, _, _)),
              actor_of(Cfg, RI, A), executor_of(Cfg, RI, X),
              (   member(Now, Nows), Op = dispatch(A, Id, Now)
              ;   member(B, [true, false]), Op = ack(Id, B)
              ;   member(Now, Nows), Op = cancel(A, Id, Now)
              ;   Op = cancel(Owner, Id, Clock)
              ;   member(B, [true, false]), Op = settle(X, Id, B)
              ;   Id =:= F, ( Op = dispatch(z, Id, Clock) ; Op = cancel(z, Id, Clock) ; Op = settle(z, Id, true) )
              )
            ), RecOps),
    findall(Op, absent_id_op(Cfg, S, Clock, Op), AbsOps),
    findall(Op,
            ( member(B, [true, false]), Op = pause(Owner, B)
            ; Op = pause(z, true)
            ; member(Vs, Obs), member(Now, Nows), Op = observe(Owner, Vs, Now)
            ; \+ memberchk(Cur, Obs), has_open_record(S), member(Now, Nows),
              Op = observe(Owner, Cur, Now)                                       % the refresh
            ; Obs = [Vs | _], Op = observe(z, Vs, Clock)
            ), AdmOps),
    append([SubOps, RecOps, AbsOps, AdmOps], Ops).

has_open_record(S) :- pool_records(S, Rs), member(c(_, _, _, St), Rs), open_status(St), !.

%% unit_choice: every unit count up to the recipe's max, or the universe's sample.
unit_choice(Cfg, Opts, RI, U) :-
    (   opt(Opts, units(RI, Us)) -> member(U, Us)
    ;   recipe_of(Cfg, RI, r(_, _, _, _, MU, _)), between(1, MU, U)
    ).

%% absent_id_op: calls on the pruned id first_id - 1 (WIN-2) and on the next,
%% unissued id, by the callers that would be authorized for recipe 0.
absent_id_op(Cfg, S, Clock, Op) :-
    pool_first_id(S, F), pool_next_id(S, Next),
    actor_of(Cfg, 0, A), executor_of(Cfg, 0, X), owner_of(Cfg, O),
    (   F >= 1, Id is F - 1,
        ( Op = dispatch(A, Id, Clock) ; Op = ack(Id, true) ; Op = cancel(O, Id, Clock)
        ; Op = settle(X, Id, true) ; Op = settle(X, Id, false) )
    ;   Op = dispatch(A, Next, Clock)
    ).

op_now(submit(_, _, _, _, _, N), N).
op_now(dispatch(_, _, N), N).
op_now(cancel(_, _, N), N).
op_now(observe(_, _, N), N).

key(KeepVersion, DoneCap, n(pool(C, V, N, O, P, R, F, D), Clock, g(B, K, _)),
    k(C, V, Ver, O, P, R, Clock, B, KRel, F1, DC)) :-
    ( KeepVersion == true -> Ver = N ; Ver = x ),
    KRel is K - F, F1 is min(F, 1),
    findall(X, ( member(X0, D), X is min(X0, DoneCap) ), DC).

done_cap(config(_, _, _, _, Rs), Cap) :-
    findall(U, ( member(r(_, _, _, _, _, Req), Rs), member(p(_, U), Req) ), Us),
    foldl_max(Us, 0, Cap).

foldl_max([], M, M).
foldl_max([X | Xs], M0, M) :- M1 is max(M0, X), foldl_max(Xs, M1, M).

%%% ========================================================
%%% Exploration
%%% ========================================================

mc1_run(Name, Result) :-
    universe(Name, u(_, Opts)),
    universe_window(Opts, W),
    with_window_size(W, mc1_run_(Name, Result)).

mc1_run_(Name, mc1(Name, states(NS), edges(NE), truncated(NT), violations(Vs),
                   dead_inflight(DI), dead_reserved(DR), witness(Wit), hits(Hits), complete(Done),
                   release_blocked(count(NB), witness(WB)), tags(Tags))) :-
    universe(Name, U),
    U = u(Cfg, Opts),
    opt(Opts, times([T0 | _])),
    init_state(Cfg, T0, Init),
    start_state(Opts, Init, S0),
    ghost_init(S0, G0),
    N0 = n(S0, T0, G0),
    ( opt(Opts, below_cap(_)) -> KV = true ; KV = false ),
    done_cap(Cfg, DCap),
    key(KV, DCap, N0, K0),
    list_to_assoc([K0-0], Vis0),
    list_to_assoc([0-N0], Nodes0),
    findall(V, ( state_violation(S0, G0, V) ; probe_violation(S0, G0, V) ), IV0),
    findall(viol(0, init, V), member(V, IV0), IV),
    opt(Opts, max_states(Max)),
    explore(0, U, KV-DCap, Max, x(1, Vis0, Nodes0, [], IV, [], [], [], []),
            x(NS, _, Nodes, Edges, Vs0, Par, Trunc, H0, T0s)),
    (   NS >= Max -> Done = false
    ;   Vs0 \== [] -> Done = stopped_at_violation
    ;   Done = true
    ),
    length(Edges, NE), length(Trunc, NT),
    sort(Vs0, Vs),
    msort_dup(H0, H1), tally(H1, Hits),
    findall(T, member(T-_, T0s), Ts0), msort_dup(Ts0, Ts1), tally(Ts1, Tags),
    retractall(witness_path(Name, _, _)),
    list_to_assoc(Par, ParA),
    reverse(T0s, T0r),
    forall(member(T-_, Tags),
           ( once(member(T-(P-Op), T0r)), path_to(P, ParA, Pa0), append(Pa0, [Op], Pa),
             assertz(witness_path(Name, T, Pa)) )),
    (   pruned_somewhere(NS, Nodes)
    ->  DI = not_applicable(pruned), DR = not_applicable(pruned), Wit = none
    ;   dead_records(NS, Nodes, Edges, Trunc, Dead),
        findall(Id-J, ( member(Id-J-St, Dead), memberchk(St, [dispatched, active, unknown]) ), DI0),
        findall(Id-J, member(Id-J-reserved, Dead), DR0),
        summarize(DI0, DI), summarize(DR0, DR),
        (   DI0 = [Id1-J1 | _] -> path_to(Id1, ParA, P1), Wit = dead(record(J1), Id1, P1)
        ;   DR0 = [Id2-J2 | _] -> path_to(Id2, ParA, P2), Wit = dead(record(J2), Id2, P2)
        ;   Wit = none
        )
    ),
    findall(Id-J, ( between(0, NS, Id), Id < NS, get_assoc(Id, Nodes, n(S, _, G)),
                    release_blocked(S, G, J) ), RB),
    length(RB, NB),
    (   RB = [IdB-JB | _] -> path_to(IdB, ParA, PB), WB = blocked(record(JB), IdB, PB) ; WB = none ).

%% mc1_witness(+Universe, ?Tag, -Path): the path (from init) of the first edge
%% of the last run of Universe that met Tag; the path ends with that call.
mc1_witness(Name, Tag, Path) :- witness_path(Name, Tag, Path).

%% start_state(+Opts, +Init, -S0): Init, or Init at version VERSION_CAP - H.
start_state(Opts, Init, S0) :-
    (   opt(Opts, below_cap(H))
    ->  version_cap(Cap), N is Cap - H,
        Init = pool(C, V, _, O, P, R, F, D), S0 = pool(C, V, N, O, P, R, F, D)
    ;   S0 = Init
    ).

pruned_somewhere(NS, Nodes) :-
    between(0, NS, I), I < NS, get_assoc(I, Nodes, n(S, _, _)),
    pool_first_id(S, F), F > 0, !.

%% release_blocked(+S, +Ghost, -Id): B = 0 and the executor's settle(Id, false)
%% on the in-flight record Id is rejected.
release_blocked(S, G, Id) :-
    base_violation(S-G, B), forall(member(X, B), X =:= 0),
    pool_config(S, C), pool_records(S, Rs), pool_first_id(S, F),
    nth0(J, Rs, c(RI, _, _, St)), memberchk(St, [dispatched, active, unknown]),
    Id is F + J,
    executor_of(C, RI, Ex),
    step(S, settle(Ex, Id, false), R, _), R \== ok.

summarize(L, count(N, records(Js))) :-
    length(L, N), findall(J, member(_-J, L), Js0), sort(Js0, Js).

explore(P, U, KO, Max, X0, X) :-
    X0 = x(N, _, _, _, Vs, _, _, _, _),
    (   ( P >= N ; N >= Max ; Vs \== [] ) -> X = X0
    ;   X0 = x(_, _, Nodes, _, _, _, _, _, _),
        get_assoc(P, Nodes, Node),
        ops(U, Node, Ops, AtCap),
        expand_ops(Ops, P, KO, Node, X0, X1),
        mark_truncated(AtCap, P, Node, X1, X2),
        P1 is P + 1,
        explore(P1, U, KO, Max, X2, X)
    ).

mark_truncated([], _, _, X, X).
mark_truncated([Op | Ops], P, n(S, _, _), X0, X) :-
    (   catch(step(S, Op, R, _), _, fail), R = ok(_)
    ->  X0 = x(N, Vi, No, E, V, Pa, T, H, Tg), X = x(N, Vi, No, E, V, Pa, [P | T], H, Tg)
    ;   mark_truncated(Ops, P, n(S, _, _), X0, X)
    ).

expand_ops([], _, _, _, X, X).
expand_ops([Op | Ops], P, KO, Node, X0, X) :-
    KO = KV-DCap,
    Node = n(S0, Clock, G0),
    X0 = x(N, Vis, Nodes, Es, Vs, Par, Tr, Hs, Tgs),
    (   catch(step(S0, Op, R, S1), E, ( R = exception(E), S1 = S0 ))
    ->  true
    ;   R = no_result, S1 = S0
    ),
    findall(viol(P, Op, V),
            (   R = exception(Ex), V = exception(Ex)
            ;   R == no_result, V = no_result
            ;   edge_violation(S0, Op, R, S1, V)
            ), EVs),
    ghost_step(G0, S0, Op, R, S1, G1),
    ( op_now(Op, Now) -> Clock1 is max(Clock, Now) ; Clock1 = Clock ),
    N1 = n(S1, Clock1, G1),
    key(KV, DCap, N1, K1),
    functor(Op, F, _), ( R = err(Er) -> Rk = Er ; R = ok(_) -> Rk = ok ; Rk = R ),
    (   get_assoc(K1, Vis, Id)
    ->  Vis1 = Vis, Nodes1 = Nodes, Par1 = Par, NN = N, SVs = []
    ;   Id = N, NN is N + 1,
        put_assoc(K1, Vis, Id, Vis1), put_assoc(Id, Nodes, N1, Nodes1),
        Par1 = [Id-(P-Op) | Par],
        findall(viol(Id, Op, V), ( state_violation(S1, G1, V) ; probe_violation(S1, G1, V) ), SVs)
    ),
    findall(T-(P-Op), edge_tag(S0, Op, R, S1, T), NewTags),
    append(NewTags, Tgs, Tgs1),
    append([EVs, SVs, Vs], Vs1),
    X1 = x(NN, Vis1, Nodes1, [P-Id | Es], Vs1, Par1, Tr, [F-Rk | Hs], Tgs1),
    expand_ops(Ops, P, KO, Node, X1, X).

%% edge_tag(+S0, +Op, +R, +S1, -Tag): the window, refresh and cap situations
%% (the witnesses the window, cap and f2b universes must reach).
edge_tag(S0, submit(_, _, _, _, _, _), ok(_), S1, prune) :-
    pool_first_id(S0, F0), pool_first_id(S1, F1), F1 > F0.
edge_tag(_, submit(_, _, _, _, _, _), err(full), _, full_open_front).
edge_tag(S0, submit(_, RI, _, _, _, _), ok(_), _, prereq_after_prune) :-
    pool_config(S0, C), recipe_of(C, RI, r(_, _, _, _, _, Req)),
    member(p(P, U), Req),
    pool_records(S0, Rs), findall(X, member(c(P, X, _, completed), Rs), Xs), sum_list(Xs, N),
    N < U, !.
edge_tag(S0, Op, err(not_found), _, not_found_pruned) :-
    op_id_of(Op, Id), pool_first_id(S0, F), Id < F.
edge_tag(S0, observe(_, Vs, _), ok, _, refresh) :-
    pool_values(S0, Vs), pool_records(S0, Rs), member(c(_, _, _, St), Rs), open_status(St), !.
edge_tag(S0, Op, err(arithmetic), _, cap_stop(K)) :-
    op_guard_kind(Op, K), memberchk(K, [submit, dispatch, pause, observe]),
    pool_version(S0, N), version_cap(Cap), N >= Cap.
edge_tag(S0, Op, ok, S1, cap_plain(K)) :-
    op_guard_kind(Op, K), memberchk(K, [ack, cancel, settle_completed, settle_cancelled]),
    S1 \== S0, pool_version(S0, N), version_cap(Cap), N >= Cap.
edge_tag(_, submit(_, _, _, _, _, _), err(E), _, submit_rejected(E)) :-
    memberchk(E, [arithmetic, bounds]).

op_id_of(dispatch(_, Id, _), Id).
op_id_of(ack(Id, _), Id).
op_id_of(cancel(_, Id, _), Id).
op_id_of(settle(_, Id, _), Id).

%%% ========================================================
%%% Dead records
%%% ========================================================

dead_records(NS, Nodes, Edges, Trunc, Dead) :-
    reverse_adjacency(Edges, Rev),
    findall(J, ( between(0, NS, I), I < NS, get_assoc(I, Nodes, n(S, _, _)),
                 pool_records(S, Rs), nth0(J, Rs, _) ), Js0),
    sort(Js0, Js),
    findall(Id-J-St,
            ( member(J, Js),
              findall(I, ( between(0, NS, I), I < NS, get_assoc(I, Nodes, n(S, _, _)),
                           pool_records(S, Rs), nth0(J, Rs, c(_, _, _, St0)), terminal(St0) ), Tg0),
              append(Tg0, Trunc, Tg),
              backward(Tg, Rev, Can),
              between(0, NS, Id), Id < NS, get_assoc(Id, Nodes, n(S, _, _)),
              pool_records(S, Rs), nth0(J, Rs, c(_, _, _, St)), open_status(St),
              \+ get_assoc(Id, Can, _)
            ), Dead0),
    sort(Dead0, Dead).

reverse_adjacency(Edges, Rev) :-
    empty_assoc(E0),
    foldl_rev(Edges, E0, Rev).
foldl_rev([], A, A).
foldl_rev([F-T | Es], A0, A) :-
    (   get_assoc(T, A0, L) -> put_assoc(T, A0, [F | L], A1) ; put_assoc(T, A0, [F], A1) ),
    foldl_rev(Es, A1, A).

backward(Targets, Rev, Can) :-
    empty_assoc(C0),
    bw(Targets, Rev, C0, Can).
bw([], _, C, C).
bw([I | Is], Rev, C0, C) :-
    (   get_assoc(I, C0, _) -> bw(Is, Rev, C0, C)
    ;   put_assoc(I, C0, t, C1),
        ( get_assoc(I, Rev, Ps) -> append(Ps, Is, Is1) ; Is1 = Is ),
        bw(Is1, Rev, C1, C)
    ).

path_to(0, _, []) :- !.
path_to(Id, Par, Path) :-
    get_assoc(Id, Par, P-Op),
    path_to(P, Par, Path0),
    append(Path0, [Op], Path).
