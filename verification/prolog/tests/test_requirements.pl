%% test_requirements.pl -- C-1..C-6: docs = code = TLA+, fixtures, traceability
%%
%% PROP: C-1 C-2 C-3 C-4 C-5 C-6
%%
%% Run (from prolog/tests or prolog/):
%%   scryer-prolog test_requirements.pl -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null

:- use_module(library(lists)).
:- use_module(library(format)).
:- use_module(library(iso_ext)).
:- use_module('support/harness').

:- discontiguous(test/2).
:- use_module('../requirements/pool_req').
:- use_module('../requirements/traceability').
:- use_module('../extracted/code_facts').
:- use_module('../extracted/tla_facts').
:- use_module('../verification/consistency').

run_tests :- run_suite("shared-pool Prolog: requirement checks C-1..C-6", x).

c_ok(Id) :-
    c_check(Id, R),
    (   R = ok(Info) -> note("~w ok: ~q", [Id, Info])
    ;   R = fail(Ps, Info),
        note("~w FAILED (~q)", [Id, Info]),
        forall(member(P, Ps), note("  problem: ~q", [P])),
        fail
    ).

test('C-1 authority matrix: api.md/spec:11 = shell attributes + core caller checks', c_ok('C-1')).
test('C-2 guard order: spec = code paths = TLA+ GuardOrder', c_ok('C-2')).
test('C-3 lifecycle: spec = code = TLA+ Edges = LIFE-1, and the real traces', c_ok('C-3')).
test('C-4 constants: docs = code, model bounds = code', c_ok('C-4')).
test('C-5 fixture conservation on trace configs 1-5', c_ok('C-5')).
test('C-6 traceability: catalogue = strategy.md section 5, requirements, threats, assumptions', c_ok('C-6')).

% C-6 is not vacuous: strategy.md section 5 is read, and every catalogue ID and
% every layer list is compared with it.
test('C-6 compares every catalogue ID and layer list with strategy.md section 5', (
    c_check('C-6', R), ( R = ok(Info) ; R = fail(_, Info) ),
    memberchk(properties(NP), Info), memberchk(strategy_rows(NS), Info),
    memberchk(layers_equal(NL), Info),
    note("properties ~d, strategy.md section 5 rows ~d, equal layer lists ~d", [NP, NS, NL]),
    check(nonempty, NP > 0), check(rows(NS), NS =:= NP), check(layers(NL), NL =:= NP) )).

% ... and the comparison reports each kind of disagreement: a property/3 row
% missing from the catalogue (what mutant r01 does), a section 5 row missing,
% a repeated row, a layer list that differs (mutant r02) and an empty section.
test('C-6 reports a dropped, an added, a repeated and a relabelled ID, and an empty section', (
    strategy_path(SP), strategy_catalogue(SP, Rows, SPs),
    check(read_clean(SPs), SPs == []),
    findall(Id-Ls, property(Id, Ls, _), Cat),
    check(agree, catalogue_problems(Rows, Cat, [])),
    Cat = [C1-CL1 | CatRest],
    catalogue_problems(Rows, CatRest, P1), note("property ~w dropped: ~q", [C1, P1]),
    check(dropped_property, P1 = [strategy_id_not_in_catalogue(C1, line(_))]),
    Rows = [row(N1, R1, RL1) | RowsRest],
    catalogue_problems(RowsRest, Cat, P2), note("row ~w dropped: ~q", [R1, P2]),
    check(dropped_row, P2 == [catalogue_id_not_in_strategy(R1)]),
    append(Rows, [row(9999, R1, RL1)], Rows3),
    catalogue_problems(Rows3, Cat, P3), note("row ~w repeated: ~q", [R1, P3]),
    check(repeated_row, P3 == [strategy_duplicate_id(R1, lines([N1, 9999]))]),
    catalogue_problems(Rows, [C1-[prolog | CL1] | CatRest], P4), note("layers of ~w changed: ~q", [C1, P4]),
    check(relabelled, P4 = [layers_differ(C1, line(_), strategy(_), catalogue([prolog | CL1]))]),
    catalogue_problems([], Cat, P5),
    check(empty, memberchk(strategy_no_ids, P5)) )).

% The section 5 reader on small documents: only section 5's tables count,
% header and separator rows are skipped, an escaped bar stays in its cell, and
% a missing section, a non-ID first cell, a short row, a table with another
% header and a table without a separator are each reported with their line.
test('C-6 strategy.md reader: section 5 only, headers skipped, malformed rows reported', (
    Doc = "# T\n## 4. Before\n| ID | S | Layer(s) |\n|---|---|---|\n| X-1 | s | tla |\n## 5. Catalogue\n\nText.\n\n### 5.1 Sub\n\n| ID | Statement | Layer(s) | Evidence |\n|---|---|---|---|\n| A-1 | a \\| b | kani, lean | e |\n| B-2 | b | near-tests, prolog | e |\n\n| ID | S | Layer(s) |\n|:--|---|---|\n| C-3 | c | assumption |\n## 6. After\n| ID | S | Layer(s) |\n|---|---|---|\n| Z-9 | z | tla |\n",
    catalogue_rows(Doc, Rows, Ps), note("rows ~q, problems ~q", [Rows, Ps]),
    check(rows, Rows == [row(14, 'A-1', [kani, lean]), row(15, 'B-2', ['near-tests', prolog]),
                         row(19, 'C-3', [assumption])]),
    check(no_problems, Ps == []),
    catalogue_rows("# T\n## 6. X\n| ID | S | Layer(s) |\n|---|---|---|\n| A-1 | s | tla |\n", R2, P2),
    check(no_section, ( R2 == [], P2 == [strategy_section_5_missing] )),
    Bad = "## 5. C\n| ID | S | Layer(s) |\n|---|---|---|\n| `A-1` | s | tla |\n| B-2 | s |\n\n| Name | Layer(s) |\n|---|---|\n| C-3 | tla |\n\n| D-4 | s | tla |\n",
    catalogue_rows(Bad, R3, P3), note("malformed: rows ~q, problems ~q", [R3, P3]),
    check(malformed_rows, R3 == [row(5, 'B-2', none)]),
    check(malformed_problems, P3 == [strategy_row_not_a_bare_id(line(4), '`A-1`'),
                                     strategy_row_without_layers(line(5), 'B-2', cells(2)),
                                     strategy_table_header(line(7), ['Name', 'Layer(s)']),
                                     strategy_table_without_separator(line(11))]) )).

% C-2 is not vacuous: every op kind's alignment uses reachable items of the code,
% and the comparison would see a swapped pair.
test('C-2 alignment covers every op kind and rejects a swapped spec', (
    forall(op_kind(K), (
        consistency:kind_code(K, Fn, Fl), code_path(Fn, Fl, Items), spec_outcomes(K, S),
        check(aligned(K), once(c2_align(S, Items, _))),
        length(S, N), check(nonempty(K), N > 0),
        (   S = [A, B | Rest], A \== B
        ->  check(swap_detected(K), \+ c2_align([B, A | Rest], Items, _))
        ;   true
        ) )) )).

% C-2 compares guard sites, not only outcome names: each guard with a
% documented condition (guard_condition/4: the record bound, the version cap)
% stands for a code site with the same extracted `if` condition. Observe with
% a plain bump() in place of bump_new() (an Arithmetic site without the cap
% test; mutant c12) still lines up by outcome names and is rejected; so is a
% Full test without the open-front conjunct (mutant c11), and a cap-check
% site cannot stand for a guard without that condition.
test('C-2 compares the record bound and version cap conditions with the code', (
    findall(K-G-Fn, ( op_kind(K), consistency:kind_code(K, F, Fl), code_path(F, Fl, Is),
                      spec_outcomes(K, S), once(c2_align(S, Is, A)),
                      member(g(G, _, C)-[Fn-_ | _], A), C \== none ), Ms),
    note("conditioned guards and their code sites: ~q", [Ms]),
    check(matched, Ms == [submit-record_bound-candidate, submit-version_cap-candidate,
                          dispatch-version_cap-bump_new, pause-version_cap-bump_new,
                          observe-version_cap-bump_new]),
    consistency:kind_code(observe, OF, OFl), code_path(OF, OFl, OIs), spec_outcomes(observe, OS),
    append(Pre, [it('Arithmetic', [bump_new-_, observe-LB]) | _], OIs),
    code_events(bump, [e('Arithmetic', LBump)]),
    append(Pre, [it('Arithmetic', [bump-LBump, observe-LB])], Plain),
    check(plain_bump_names_align, once(consistency:c2_align(names, OS, Plain, _))),
    check(plain_bump_rejected, \+ c2_align(OS, Plain, _)),
    consistency:spec_cond(submit, record_bound, FC), note("record_bound condition: ~q", [FC]),
    check(full_needs_open_front, \+ consistency:same_conds(FC, [ge(retained, 'MAX_RECORDS')])),
    code_error_cond(bump_new, LC, 'Arithmetic', _),
    check(cap_site_reserved, \+ consistency:site_ok(none, 'Arithmetic', [bump_new-LC])) )).

% C-3: the code derivation is total and agrees with the spec on every row. The
% v1.0.0 difference (Unknown -> Unknown on a negative acknowledgment) is closed:
% spec:41 now names it an Ok no-op (div-ack-noop-version is a closed divergence).
test('C-3 static spec/code differences: none (div-ack-noop-version closed)', (
    findall(Op-F-S, ( consistency:life_domain(Op, F, S), life(Op, F, S, SO, _),
                      code_life(Op, F, S, CO), SO \== CO ), Ds),
    note("differences: ~q", [Ds]),
    check(no_difference, Ds == []),
    check(ack_noop_doc, life(ack, false, unknown, same, doc(_))),
    check(cancel_completed, life(cancel, none, completed, reject('InvalidState'), doc(_))),
    check(closed, closed_divergence('div-ack-noop-version', _, _, _, _)) )).

% The TLA+ leg is live (the file exists at this pin) or explicitly a gap.
test('C-2/C-3 TLA+ leg status is recorded', (
    tla_source(Src), note("tla/SharedPool.tla: ~w", [Src]),
    (   Src == present
    ->  findall(Op, tla_guard_order(Op, _), Ops), length(Ops, NO), check(tla_ops(NO), NO >= 7),
        findall(x, tla_status_edge(_, _), Es), length(Es, NE), check(tla_edges(NE), NE > 0)
    ;   true
    ) )).

% No divergence is open at the pin; each v1.0.0 divergence is recorded as closed
% with its v1.0.0 citation, its v1.1.0 citation and both texts.
test('divergences: none open; the v1.0.0 ones are closed with old and new citations', (
    findall(D, divergence(D, _, _, _), Open), note("open divergences: ~q", [Open]),
    check(none_open, Open == []),
    findall(D, closed_divergence(D, _, _, _, _), Ds), note("closed divergences: ~q", [Ds]),
    check(closed_four, Ds == ['div-F1-inverse', 'div-F2-fixture-release',
                              'div-ack-noop-version', 'div-cancel-terminal-name']),
    forall(closed_divergence(D, Old, New, Was, Res),
           check(cites(D), ( atom(Old), atom(New), atom_chars(Old, OC), append(_, ['@', v, '1', '.', '0', '.', '0' | _], OC),
                             atom_chars(New, NC), \+ append(_, ['@' | _], NC),
                             Was = [_ | _], Res = [_ | _] ))) )).

% The version reserve constant is extracted with its value (u32::MAX - 768,
% evaluated by extract.py) and agrees with the docs (C-4 compares them too).
test('VERSION_CAP is extracted as u32::MAX - 768 and matches the docs', (
    check(extracted, code_constant('VERSION_CAP', 4294966527, 24)),
    check(docs, constant(version_cap, 4294966527, _)),
    constant(version_max, VM, _), constant(max_records, MR, _),
    check(reserve, 4294966527 =:= VM - 3 * MR),
    guard_sequence(submit, SO), append(_, [Last], SO), note("submit's last guard: ~q", [Last]),
    check(cap_last_in_submit, guard(submit, _, version_cap, Last, _)) )).

% C-5's trace releases address a record by id = FirstId + j (record j of an
% s/7 state; spec:9, spec:27), not by its list index: in a pruned window
% (FirstId > 0) the index is not the id. A v1.0.0 s/5 state starts at 0.
% (The release itself is the model's step/4 at that id: an end-to-end test on
% a pruned window needs the v1.1 model's pool with first_id, W6.)
test('C-5 trace releases use id = first_id + index in a pruned window', (
    St7 = s([0], 9, 0, false, [c(0, 1, 10, completed), c(0, 1, 10, reserved),
                               c(1, 2, 10, cancelled), c(1, 2, 10, dispatched)], 5, [1, 0]),
    findall(Id-RI-S, consistency:trace_open_record(St7, Id, RI, S), O7),
    note("open records of a window starting at id 5: ~q", [O7]),
    check(pruned, O7 == [6-0-reserved, 8-1-dispatched]),
    St5 = s([0], 2, 0, false, [c(0, 1, 10, reserved), c(0, 1, 10, completed)]),
    findall(Id-S, consistency:trace_open_record(St5, Id, _, S), O5),
    check(unpruned, O5 == [0-reserved]) )).

% End to end on the v1.1.0 model (pool/8): C-5's release of an open record of
% a pruned window reaches the model's step/4 at id first_id + index, and the
% list index alone would name a pruned id (NotFound).
test('C-5 trace releases in a pruned window are Ok on the model at id = first_id + index', (
    Cfg = config(o, 0, [m(0, 10, 5)], [t(0, 5, 1, target)],
                 [r(a1, x1, [0], [0], 5, []), r(a2, x2, [0], [0], 5, [])]),
    S = pool(Cfg, [5], 9, 100, false, [c(0, 1, 1000, completed), c(0, 1, 1000, reserved),
                                       c(1, 2, 1000, cancelled), c(1, 2, 1000, dispatched)], 5, [1, 0]),
    pool_machine:to_trace_state(S, St7),
    findall(R, consistency:trace_release(Cfg, St7, S, 100, R), Rels),
    note("releases ~q", [Rels]),
    check(released, Rels == [rel(6, reserved, ok), rel(8, dispatched, ok)]),
    check(index_is_pruned, pool_machine:step(S, cancel(a1, 1, 100), err(not_found), _)) )).

% Extracted fixture accounts (fixtures.rs) behind CASH-SELF.
test('C-5 CASH-SELF uses the fixtures.rs account names, not the renamed trace atoms', (
    fixture_executor(X, _), fixture_actors(hiring, As),
    note("hiring actors ~q, executor ~q", [As, X]),
    As = [_ | _], \+ memberchk(X, As) )).

% C-5 at v1.1.0 reproduces no finding: F-2 (RISK-RELEASE) and F-4
% (CASH-SELF-ENFORCED) are fixed, so both claims are checked outright and no
% known_finding/2 remains. All six v1.0.0 findings are fixed('v1.1.0', Props).
test('C-5 reproduces no finding; F-1..F-6 are fixed at v1.1.0', (
    c_check('C-5', R), ( R = ok(Info) ; R = fail(_, Info) ),
    memberchk(findings(Fs), Info),
    note("findings reproduced: ~q", [Fs]),
    check(none_reproduced, Fs == []),
    check(no_known_finding, \+ known_finding(_, _)),
    memberchk(claims(Cs), Info),
    check(cash_self_enforced, memberchk('CASH-SELF-ENFORCED'-[code_role_checks(RC)], Cs)),
    check(validate_rejects_self, memberchk(actor-executor, RC)),
    findall(F, finding(F, _, _, _), All), note("findings recorded: ~q", [All]),
    check(six_findings, All == ['F-1', 'F-2', 'F-3', 'F-4', 'F-5', 'F-6']),
    forall(member(F, All), check(fixed(F), finding(F, _, _, _, fixed('v1.1.0', [_ | _])))) )).
