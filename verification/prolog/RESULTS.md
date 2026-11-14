# shared-pool: Prolog layer results

Pin: difflabai/shared-pool-controls v1.1.0, merge commit
`57b3f02d18e85149db3c1035431368c4e07a4d8a` (upstream PR #4; its tree is the
PR head `6fcbd56`). Docs transcribed: `docs/spec.md`, `docs/api.md`,
`docs/verification-handoff.md` at that pin, cited as `spec:<line>`,
`api:<line>`, `handoff:<line>`; v1.0.0 text that v1.1.0 deleted is cited as
`spec@v1.0.0:<line>`. Tools: Scryer Prolog 0.10.0 (`scryer-prolog`, every
suite run as
`-g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null`)
and python3 for `extract.py` (run with `-I`, stdlib only). Property
catalogue: `projects/shared-pool/strategy.md` §5 (118 IDs), which C-6 reads.
Run: `projects/shared-pool/prolog/run.sh` (optional `PROLOG_LOGS=<dir>`).
Last full run: 2026-11-14, one uninterrupted run of `run.sh` (exit 0) at
the integration commit `a7781c7` (the v1.1.0 corpus and the trace reader
committed together): every step passes and all 47 mutants are killed, in
1,777 s, about 30 minutes (suites 182 s, mutants 1,554 s summed, most of it
the MC-1 mutants), on a machine shared with other jobs. The first such run,
in a scratch copy at `51d4ca2` with the same corpus and reader, took
1,735 s.

Conformance traces (v1.1.0 corpus, generated from the real core by
`conformance/`): `core.txt`, sha256
`dd24852eadda4fab5ad890035dad3ccf2344704ca18a090625dc2e71a2de4268` (216
traces, 8,865 calls; trace 210 is a refused config, `init_error`), and
`reads.txt`, sha256
`9733bc8f23b4cab34bdcbe75e83d7e36cad647d84d488e52e333e0b147700103` (39,627
requests at 1,300 states). The MC-2 and COV-2 numbers below are on this
corpus with the s/7 trace reader (`verification/traces.pl`, `mc2.pl`,
`reads.pl`), committed together with it (the corpus swap): the v1.0.0
corpus (s/5) does not convert to `pool/8`.

Evidence vocabulary (near-approval strategy §12): this layer gives `check`
(C-1..C-6), `model-check` (MC-1, LIVE-2, VER-CAP) and `test` (the MC-2
replay, the step properties, invariants and probes checked on the real core's
traces, the COV-2 reads differential and the step/4 unit tests).

## Layout

| File | Contents |
|---|---|
| `requirements/pool_req.pl` | 79 `requirement/3` facts (one per doc statement used); the api.md method table and authority (`method/4`, `method_core/2`); the guard tables in the order the docs list them (`guard/5`, 57 rows, each `doc` or `doc_condition`; `guard_condition/4` for the Full and version-cap conditions); the lifecycle table (`life/5`); constants (including `version_cap` = u32::MAX - 768); the five fixtures with 23 claims (`fixture_claim/4`); `finding/4` (F-1..F-6, each `fixed('v1.1.0', Props)`); 4 `closed_divergence/4` and 7 `ambiguity/4` facts. |
| `requirements/threat_model.pl` | 6 adversaries, 25 threats with mitigating properties and residuals, and the 15 ASM-* assumptions of strategy.md §5. |
| `requirements/traceability.pl` | The 118-ID catalogue of strategy.md §5 with each ID's layer list, `prolog_evidence/2` for the 48 IDs this layer carries, and 79 requirement → property rows. |
| `model/pool_machine.pl` | `step/4`, written from the docs, over `pool(Config, Values, Version, ObservedAt, Paused, Records, FirstId, Done)`: `Records` is the retained window (record j has id `FirstId + j`), `Done` the per-recipe `completed_units` (saturating u32). `step` walks the guard table of `pool_req.pl` in order, so the model's behaviour *is* the requirement table. The envelope is the hull of each open record's completion `d = complete*u` and release `x = -reserve*u` (`low += min(d, x, 0)`, `high += max(d, x, 0)`), every endpoint within ±1e6 or `Arithmetic`; `Bounds` compares the envelope's violation with `min(PV(values), PV(candidate))`. `window_size/1` (256) and `version_cap/1` (u32::MAX - 3 × window) can be scaled down only inside `with_window_size/2` (MC-1). 13 places are tagged `CODE-ONLY` where the docs are silent. Reads: `admissible/3`, `preview_allowed/3` (every submit guard, the cap included), `best_quantity/4`. |
| `extract.py` → `extracted/code_facts.pl` | Generated from `crates/core/src/lib.rs`: constants (VERSION_CAP as an expression), enums, field widths, per-function error sites and their window/version conditions, fallible calls, early `Ok` returns, caller checks, status guards and writes, and role comparisons in `Config::validate`. Also from `crates/contract/src/lib.rs` and `crates/core/src/fixtures.rs`. The parser is narrow and fails loudly on constructs it does not know. |
| `extract.py` → `extracted/tla_facts.pl` | `GuardOrder` from `tla/SharedPool.tla` and the LIFE-1 `Edges` from `tla/Props.tla`. |
| `verification/consistency.pl` | C-1..C-6. Every problem is collected, and each check reports non-vacuity counts. |
| `verification/model_check.pl` | MC-1: breadth-first exploration of seven universes through `step/4`; dead records; release-blocked states; edge tags (prune, Full on an open front, prerequisite after a prune, NotFound on a pruned id, refresh, the cap) with the shortest witness of each. |
| `verification/props.pl` | Invariants (INV-WF, WIN-1, INV-LEDGER, INV-RES, INV-HARD, INV-HEALTHY, INV-DEP, INV-VER, INV-VERCAP), step properties (TX-0, TX-NOOP, TX-VER, AUTH-1, STALE-1, ADM-1, ADM-2, HARD-STEP, DEP-1, LIFE-1, WIN-1, WIN-2, ASYNC-1, TERM-1, OBS-1, OBS-2, PAUSE-1, VER-CAP) and probes (LIVE-1, LIVE-ALL, REFRESH-1, WIN-ESC). The INV-LEDGER ghost `g(Base, K0, Eff)` folds pruned records into the base and keeps it on a refresh. MC-1 and MC-2 both use them. |
| `verification/{mc2,traces,util}.pl` | The MC-2 replay and the trace reader (s/7 states and `init_error` traces). |
| `verification/reads.pl` | COV-2: reads `reads.txt`, rebuilds each state and compares the real core's reads with the model's, field by field. |
| `tests/test_{requirements,model,mc1,mc2,reads}.pl` | The suites (`run_tests`), via `tests/support/harness.pl`. |
| `mutants/{model,requirements,code,reads}/*.patch` | 32 model mutants (m21-m32 restore one v1.0.0 behaviour each), 2 requirements mutants, 11 code mutants and 2 reads mutants, each with `# expect: <suite> <test prefix>` lines. |
| `run.sh` | The runner. |

`run.sh` steps:
1. `extract.py --check`: regeneration gives no diff.
2. Each suite must exit 0 with no loader `Warning:` in its log.
3. Canaries: `test_requirements`, `test_model` and `test_mc2` each run with `SHARED_POOL_PROLOG_CANARY=1` and must exit 1 with only the canary test failing. `test_mc1` and `test_reads` have no canary run; their mutants show they exit non-zero on a failing test.
4. PROP tags: every catalogue ID with the prolog layer is tagged in a `.pl/.py/.sh` file here, and no tag is unknown.
5. Mutants: each patch is applied to a scratch copy; every `# expect:` suite must exit non-zero and print `FAIL <prefix>`.

## Results

| Check | ID | Result | Size / time |
|---|---|---|---|
| `extract.py --check` | C-1..C-4 inputs | extracted facts regenerate with no diff | |
| Authority: api.md table + spec:11 = the shell's `#[near]` attributes, receivers and caller sources + the core's caller checks | C-1 | pass | 12 methods, 12 shell methods, 6 core caller checks, 17 shell → core calls (7 reads) |
| Guard order: `guard/5` (docs order) = each entry point's code path = TLA+ `GuardOrder`; the documented Full and version-cap conditions = the code's | C-2 | pass (the TLA leg reads the v1.1.0 `GuardOrder`, whose cancel row gains InvalidState) | 8 op kinds, 57 guards (49 doc, 8 doc_condition; submit 15, dispatch 7, ack 4, cancel 9, settle 8 + 8, pause 2, observe 4); 10 unreachable-site rules; 5 conditions compared |
| Lifecycle: `life/5` = the code's status guards = TLA+ `Edges` = the props LIFE-1 graph = the status changes seen in the real traces | C-3 | pass, no divergence (v1.0.0's `div-ack-noop-version` and `div-cancel-terminal-name` closed) | 36 rows, 11 edges, 3 code-only rows (ack of Reserved both ways, ack(false) of Active); all 36 rows observed in the traces |
| Constants: docs = code; derived bounds; the model's code-only bounds = code | C-4 | pass | 9 compared (VERSION_CAP = 4294966527), 5 derived |
| Fixture claims on trace configs 1-5 | C-5 | pass, no finding reproduced (F-2, F-4 fixed) | 23 claims; release search 1,308 / 1,308 / 36 / 28 / 81 admitted pairs (gpu / friday / inventory / hiring / risk), 10,464 / 10,464 / 288 / 224 / 648 releases tried, 0 blocked; on trace states 179 / 167 / 67 / 26 / 9 releases, 0 blocked; RISK-HEADROOM 46 states, 464 admissible requests |
| Traceability | C-6 | pass | 118 properties = 118 §5 rows, equal layer lists; 79 requirements, 25 threats, findings open 0 / fixed 6, divergences open 0 / closed 4 |
| MC-1 `wedge` (trace 0's config, observations [12, 5] and [5, 5]) | MC-1, LIVE-2, LIVE-ALL | no violation; no dead or release-blocked record (v1.0.0: 228 states with a dead in-flight record, F-1); the F-1 hedge is rejected `Bounds` in [12, 5] (166 edges, shortest witness `observe(o, [12, 5])`, `submit(a1, 0, 1)`) | 4,672 states, 149,584 edges, complete; 4,320 refresh edges |
| MC-1 `healthy` (2 recipes, a prerequisite, freshness) | MC-1, LIVE-2 | no violation; no dead or release-blocked record | 5,032 states, 134,752 edges, complete; 11 error kinds reached (with `not_found`) |
| MC-1 `reliance` (F-2, scaled down) | MC-1, LIVE-2 | no violation; no release-blocked state (v1.0.0: 150 with B = 0); the hedge is rejected `Bounds` (44 edges) | 454 states, 10,810 edges, complete |
| MC-1 `window` (healthy's config, window 2, no truncation; F-3) | MC-1, WIN-1, WIN-2, WIN-ESC, LIVE-ALL | no violation; prune 2,280 edges, Full on an open front 3,344, prerequisite admitted after its record was pruned 353, NotFound on the pruned id 20,090, refresh 3,536 | 4,754 states, 160,976 edges, complete |
| MC-1 `window1` (window 1) | same | no violation; prune 928, Full on an open front 2,120, prerequisite after prune 168, NotFound on the pruned id 8,450 | 2,186 states, 65,070 edges, complete |
| MC-1 `cap` (window 2, VERSION_CAP = u32::MAX - 6, from 4 below it; F-5) | MC-1, VER-CAP, INV-VERCAP, LIVE-ALL | no violation; no dead record; at the cap submit (768 edges), dispatch (158), set_paused (2,968) and observe (3,592) stop with `Arithmetic`, while ack (276), cancel (540) and settle (392 + 392) stay Ok past it | 1,834 states, 52,770 edges, complete; the key keeps the version |
| MC-1 `f2b` (trace 208's config; F-2b) | MC-1, INV-RES | no violation; no dead or release-blocked record; a submit whose envelope would leave ±1e6 fails `Arithmetic` at admission (147 edges, 16 of them only through a release term; shortest: recipe 1 × 67 from [0, 0]) | 1,776 states, 43,240 edges, complete |
| MC-1 suite total (8 tests, including "the window is 256 again after the scaled runs") | | | 111 s |
| MC-2: every trace replayed through `step/4` from the code's pre-state; result and post-state compared exactly; a refused config must be refused by `valid_config/1`; invariants, step properties and probes checked on the code's own states | MC-2, TX-*, INV-*, WIN-*, OBS-*, LIVE-1, LIVE-ALL, REFRESH-1, WIN-ESC | 0 divergences, 0 violations | 216 traces, 8,865 calls (2,565 Ok); every op kind Ok and all 15 reachable error kinds (`full` 1, `arithmetic` 3, `not_found` 2,869 calls); traces 0, 208-215 checked for their fixed shapes; suite (23 tests) 14.0 s |
| step/4 unit tests (doc-cited), including the F-1..F-6 "fixed" tests (each keeps its v1.0.0 outcome in a comment), the RD-PREVIEW/RD-BEST model checks and the checkers' vacuity tests | TX-*, RD-*, WIN-*, VER-CAP | 34 pass | 1.4 s |
| COV-2 reads differential: at each state of `reads.txt` the real core's `preview` and `best_quantity` equal the model's | RD-PREVIEW, RD-BEST | 0 divergences | 1,300 states (12 at VERSION_CAP or u32::MAX), 39,627 requests: 1,785 allowed, 37,842 rejected (paused 4,919, stale 1,998, expired 3,384, snapshot_expired 815, full 26, invalid_recipe 2,202, unauthorized 5,505, invalid_units 13,484, prerequisite 642, arithmetic 61, bounds 3,712, energy 1,094); best some 6,233, none 33,394 (1,912 above MAX_UNITS); suite (3 tests) 41.7 s |
| Requirements suite (C-1..C-6 and their non-vacuity tests) | C-* | 19 tests | 13.9 s |
| Planted faults (`test_mc2`): each step-property checker fires on a corrupted real call and stays silent on the real one (the v1.0.0 fourteen plus TERM-1 on cancel(Completed), OBS-2, WIN-1, WIN-2); in `test_model` the INV-*, WIN-1, edge and probe checkers fire on corrupted or v1.0.0 states | vacuity | pass | |
| PROP tags | | every prolog-layer catalogue ID is tagged; no unknown tag | 48 of 118 |
| Mutants | | 47 of 47 killed | 1,554 s summed |

MC-1 bounds. The exploration state is pool + clock + ghost. Time is monotone
(ASM-TIME) over each universe's time set: wedge {100, 200}, healthy, window1
and cap {100, 150, 250}, reliance {100, 250}, window and f2b {100}. Callers
are each recipe's actor and executor, the owner and a stranger `z`. Submits
are generated at the current version and at a stale one, with units up to the
recipe's maximum (f2b: 1, 33, 34, 66, 67 for recipe 1). Observation vectors
are fixed per universe (wedge includes the disturbing [12, 5]); the owner's
restatement of the current values is generated whenever a record is open.
Calls name every retained id, the pruned id `first_id - 1` and the unissued
id `first_id + len`.

Record caps are 3 (wedge) and 2 (healthy, reliance, f2b). A state where a
further submit would have been Ok is marked truncated and never certified
dead. `window`, `window1` and `cap` are not truncated: they run under
`with_window_size(W)` (W = 2 or 1), prune at W records and fail Full only
with an open front; ids are keyed relative to `first_id`, and
`completed_units` are capped at the largest prerequisite. The dead-record
analysis names records by index, so on a graph with a prune it reports
`not_applicable(pruned)` and LIVE-ALL (checked on every state) carries the
liveness evidence. The visited key drops the version (bisimilar below the
cap; INV-VER is checked per edge) except in `cap`. Exploration stops at the
first violation. After every run the window is 256 again (`test_mc1` checks
it), so C-2, MC-2 and the reads see the code's window and cap.

## Findings: F-1..F-6 fixed at v1.1.0

At v1.1.0 this layer reproduces none of the six v1.0.0 findings: C-5 reports
`findings([])` and every `finding/4` is `fixed('v1.1.0', Props)`. The v1.0.0
evidence (MC-1 228 dead and 150 blocked, C-5's 20 counterexamples, the
witness tests) stays in PR #31 and the tag `shared-pool-v1.0.0`. Each fix has
three kinds of regression evidence: (a) the same check passes here and fails on
v1.0.0; (b) a revert mutant that restores the v1.0.0 behaviour inside this
model is killed (m21-m32, c10-c12); (c) the differential replay of the two
corpora (below).

| Finding | PASS at v1.1.0 (this layer) | FAIL on v1.0.0 | Revert mutants |
|---|---|---|---|
| F-1 wedge | `test_model` "F-1 fixed" (the hedge in [12, 5] fails `Bounds`; from [5, 5] it is admitted and settles both ways); MC-1 `wedge` 0 dead; trace 0 (submit `Bounds`), traces 214-215 (the recovery hedge admitted and settled each way); INV-RES and LIVE-ALL on every MC-1 and MC-2 state | v1.0.0 model: the submit is `ok(0)`; MC-1 `wedge` 228 dead | m21, m22 |
| F-2 release blocked | `test_model` "F-2 fixed" (hedge × 7 `Bounds`, × 6 Ok, the position's honest `settle(false)` Ok at [0, 40, 30, 28]); MC-1 `reliance` 0 blocked; C-5 RISK-RELEASE 0 blocked; trace 209 | v1.0.0 model: hedge × 7 `ok(1)`; MC-1 150 blocked; C-5 20 counterexamples | m21 |
| F-2b release `Arithmetic` | `test_model` "F-2b fixed" (recipe 1 × 100 fails `Arithmetic` at admission, × 66 is Ok and its release too); MC-1 `f2b`; trace 208 | v1.0.0 model: `ok(1)` | m21 (MC-1 `f2b`: LIVE-ALL, a release fails `Arithmetic`); m23 (the error kind only, see below) |
| F-3 slot exhaustion | `test_model` "F-3 fixed" (256 cycles, then `ok(256)` with `first_id` 1 and `NotFound` on id 0; Full only while the front is open; a pruned prerequisite still counts); MC-1 `window`, `window1`; trace 207 (`ok(256)`, `ok(257)` on a pruned prerequisite, `Full` with an open front, `NotFound` on id 0) | v1.0.0 model: `err(full)` after 256 cycles | m24, m25, m26, c11 |
| F-4 self-authorization | `valid_config/1` rejects actor = executor (`test_model` "F-4 fixed"); trace 210 is refused by the code and by the model; C-5 CASH-SELF-ENFORCED; trace 213 (the owner as executor settles) | v1.0.0 `valid_config` accepts it | c10 |
| F-5 version saturation | `test_model` "F-5 fixed" at the real VERSION_CAP 4294966527 (submit, dispatch, set_paused, observe `Arithmetic`; ack, settle, cancel Ok); MC-1 `cap` (INV-VERCAP, VER-CAP, LIVE-ALL at the cap); COV-2 preview at the 12 reads states at VERSION_CAP or u32::MAX | v1.0.0 model: submit at that version `ok(1)`; at u32::MAX every call `Arithmetic` | m27, m28, c12 |
| F-6 stale snapshot | `test_model` "F-6 fixed" and the observe test (a restatement with records open is Ok and changes only `observed_at` and version; any changed metric is `Pending`); MC-1 REFRESH-1 and OBS-2 on every state and edge; trace 211 and 7 more traces | v1.0.0 model: `err(pending)` | m29, m30, m31 |
| div-cancel | `test_model` cancel test (Completed: `InvalidState`); TERM-1 on every edge; 18 traces | v1.0.0 model: `err(pending)` | m32 |

**Differential replay (c).** The v1.0.0 corpus (212 traces, s/5 states
rewritten as s/7 with `first_id` 0 and `completed_units` summed from the
records) through this model diverges on exactly 44 traces, each at the call
the fix changes: cancel of a Completed record `Pending` → `InvalidState` 18
(4, 15, 16, 23, 26, 41, 42, 47, 53, 61, 84, 85, 134, 170, 171, 183, 187, 200);
observe `Pending` → Ok 8 (27, 69, 73, 86, 100, 153, 163, 211); submit
`Energy` → `Bounds` 6 (32, 91, 94, 138, 164, 186); submit Ok → `Bounds` 9 (0,
30, 38, 79, 144, 154, 156, 181, 209) and Ok → `Arithmetic` 1 (208); `Full` →
`ok(256)` 1 (207); a config accepted → refused 1 (210). On the v1.0.0 code's
own states the new properties fire: INV-RES on 10 traces (the 10 submits
above), LIVE-ALL on 9, LIVE-1 on 5 (0, 79, 154, 208, 209) and TERM-1 on 19.
The v1.1.0 corpus through the v1.0.0 model gives the same 44 (210 through
`valid_config/1`, which accepts its config; the v1.0.0 reader has no
`init_error`). Traces 212-215 agree with both models.

### Docs vs code

No divergence is open at v1.1.0. v1.0.0's four are `closed_divergence/4` with
the old and the new citation: `div-F1-inverse`, `div-F2-fixture-release`,
`div-ack-noop-version` (api.md now says an acknowledgment that would not
change the status changes nothing) and `div-cancel-terminal-name` (spec:43:
cancelling a Completed record is `InvalidState`).

The model follows the code's reading of each remaining ambiguity, recorded in
`pool_req.pl` and tagged `CODE-ONLY` in the model where it applies:
`amb-interval-violation`, `amb-freshness-boundary`, `amb-recovery-name`,
`amb-ack-other`, `amb-reconcile-kind`, `amb-observe-length` and
`amb-owner-roles` (the owner may be a recipe's executor; trace 213).

## Mutants

All 47 are killed: 32 model, 2 requirements, 11 code and 2 reads mutants. "Killed" means every `# expect:` suite exits non-zero and prints `FAIL <prefix>` (for `# expect: generator <message>`: the generator exits non-zero and prints the message). m21-m32 each restore one v1.0.0 behaviour in the v1.1.0 model (FR ids as in strategy.md §4.5); c10-c12 do the same in the code (W3).

| Mutant | What it changes | Killed by |
|---|---|---|
| m01-hard-ok-nonstrict | ADM-1/HARD-STEP: a call that keeps a violation equal to min(before, after) is rejected (E >= min instead of E > min) | test_model (submit: each documented reason); test_mc2 (MC-2: all traces replay) |
| m02-ack-noop-dropped | div-ack-noop-version: a repeated acknowledgment bumps version | test_model (acknowledge); test_mc2 (MC-2: all traces replay) |
| m03-freshness-strict | TX-SUBMIT/TX-DISPATCH freshness: the snapshot is stale at exactly observed_at + max_age | test_model (submit: expiry and freshness boundaries); test_mc2 (MC-2: all traces replay) |
| m04-expiry-horizon-strict | ADM: expires_at = now + 86400 is rejected | test_model (submit: expiry and freshness boundaries); test_model (submit: saturating u64 expiry) |
| m05-recovery-nonstrict | ADM-2: the recovery check accepts a completion that does not lower energy | test_mc2 (MC-2: all traces replay) |
| m06-prerequisite-strict | DEP-1: a prerequisite needs more than the required completed units | test_model (DEP-1/CASH-PREREQ); test_mc2 (MC-2: all traces replay) |
| m07-envelope-skip-dropped | the record being settled or cancelled is counted twice in the envelope | test_model (STOCK); test_mc2 (MC-2: all traces replay) |
| m08-dispatch-expiry-inclusive | dispatch at exactly expires_at is accepted | test_model (dispatch: actor, unpaused, Reserved) |
| m09-energy-strict | ADM-1: a reservation that keeps energy equal is rejected | test_model (F-1 fixed: from a healthy observe); test_mc2 (MC-2: all traces replay) |
| m10-cancel-owner-dropped | AUTH-1: the owner can no longer cancel | test_model (cancel: actor or owner); test_mc2 (MC-2: all traces replay) |
| m11-settle-caller-actor | AUTH-1: settle checks the recipe actor instead of the executor | test_model (TERM-1: same terminal); test_mc2 (MC-2: all traces replay) |
| m12-observe-pending-dropped | OBS-1: observe sets new values with records open (no Pending at all) | test_model (observe: owner, all terminal or a restatement); test_model (F-6 fixed); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 wedge) |
| m13-in-flight-reserved | LIFE-1: settle accepts a Reserved record | test_mc2 (MC-2: all traces replay) |
| m14-status-change-no-bump | TX-VER: a status change does not advance version | test_model (acknowledge); test_mc2 (MC-2: all traces replay) |
| m15-release-adds | TERM-1: cancellation adds the reservation instead of subtracting it | test_model (cancel: actor or owner); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 healthy) |
| m16-guard-order-paused-stale | C-2: the specified submit order checks Stale before Paused | test_requirements (C-2 guard order); test_model (submit: the first failing guard); test_mc2 (MC-2: all traces replay) |
| m17-release-no-bounds | F-2 hidden: a release (cancel, settle(false)) skips the envelope bounds check. At v1.1.0 no reachable release fails Bounds (INV-RES), so only the hand-built v1.0.0 wedge state kills it | test_model (LIVE-1, LIVE-ALL and INV-RES fire) |
| m18-best-tie-smaller | RD-BEST: an energy tie in best_quantity goes to the smaller units | test_model (RD-BEST (model)); test_reads (RD-PREVIEW/RD-BEST differential) |
| m19-preview-ignores-paused | RD-PREVIEW: preview_allowed skips the paused check (submit still has it) | test_reads (RD-PREVIEW/RD-BEST differential) |
| m20-best-request-version | RD-BEST: best_quantity checks each quantity at the pool's version, not the request's | test_reads (RD-PREVIEW/RD-BEST differential) |
| m21-hull-no-release-term | INV-RES/LIVE-ALL: the envelope has no release term (v1.0.0 completion-only hull) (FR-2; F-1, F-2) | test_model (F-2 fixed); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 reliance) |
| m22-comparator-current-only | INV-RES/HARD-STEP: the bounds comparator is the violation before the call only (v1.0.0 E > PV(values)) (FR-1; F-1) | test_model (F-1 fixed: in the disturbed state); test_mc2 (MC-2 F-1 fixed); test_mc1 (MC-1 wedge) |
| m23-release-endpoint-unbounded | envelope Arithmetic: only the completion endpoints are held to +-1e6, not the release terms; Bounds, over the same hull, still refuses those submits (FR-2; F-2b) | test_model (F-2b fixed); test_mc2 (MC-2: all traces replay) |
| m24-full-ignores-front | WIN-ESC: Full whenever 256 records are retained, whatever the front's status (v1.0.0) (FR-3; F-3) | test_model (F-3 fixed: 256 submit+cancel cycles); test_model (F-3 fixed: Full only while); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 window) |
| m25-prune-keeps-first-id | WIN-1: a pruning submit drops the front but does not advance first_id (FR-3; F-3) | test_model (F-3 fixed: 256 submit+cancel cycles); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 window) |
| m26-prerequisite-over-window | DEP-1: prerequisites count the retained Completed records, not completed_units (v1.0.0) (FR-4; F-3) | test_model (F-3 fixed: completed_units counts a pruned prerequisite); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 window) |
| m27-plain-bump-gated | VER-CAP: dispatch, set_paused and observe check the plain u32::MAX increment, not VERSION_CAP (FR-6; F-5) | test_model (F-5 fixed); test_mc1 (MC-1 cap) |
| m28-submit-no-cap | VER-CAP/INV-VERCAP: submit, and so preview, is not gated by VERSION_CAP (FR-6; F-5) | test_model (F-5 fixed); test_mc1 (MC-1 cap); test_reads (RD-PREVIEW/RD-BEST differential) |
| m29-refresh-rejected | REFRESH-1: restating the current values with a record open is Pending (v1.0.0) (FR-7; F-6) | test_model (F-6 fixed); test_model (observe: owner, all terminal or a restatement); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 wedge) |
| m30-refresh-first-metric | OBS-1/OBS-2: the restatement test compares the first metric only, so changed values pass as a refresh (FR-7; F-6) | test_model (observe: owner, all terminal or a restatement); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 window) |
| m31-ghost-refresh-rebases | INV-LEDGER ghost: every Ok observe re-bases, a refresh with records open included (v1.0.0 ghost) (FR-7; F-6) | test_model (ghost: fold-on-prune); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 wedge) |
| m32-cancel-completed-pending | TERM-1: a cancel of a Completed record reaches the reserved guard and is Pending (v1.0.0) (FR-8; div-cancel) | test_model (cancel: actor or owner); test_mc2 (MC-2: all traces replay); test_mc1 (MC-1 healthy) |
| r01-property-dropped | C-6: traceability.pl loses the TLA-IND row, so the catalogue no longer equals strategy.md section 5 (nothing else names TLA-IND) | test_requirements (C-6 traceability); test_requirements (C-6 compares every catalogue ID) |
| r02-layer-changed | C-6: NUM-2's layer list gains tla, so it no longer equals its Layer(s) cell in strategy.md section 5 | test_requirements (C-6 traceability); test_requirements (C-6 compares every catalogue ID) |
| c01-submit-stale-before-paused | C-2: core checks Stale before Paused in candidate | test_requirements (C-2 guard order) |
| c02-dispatch-caller-executor | C-1: dispatch authorizes the executor instead of the actor | test_requirements (C-1 authority) |
| c03-settle-unknown-dropped | C-3: settle no longer accepts an Unknown record | test_requirements (C-3 lifecycle) |
| c04-max-records-255 | C-4: MAX_RECORDS differs from api.md's 256 | test_requirements (C-4 constants) |
| c05-settle-private | C-1: settle becomes #[private] (the executor can no longer call it) | test_requirements (C-1 authority) |
| c06-pause-no-bump | C-2: set_paused no longer advances version (its Arithmetic outcome disappears) | test_requirements (C-2 guard order) |
| c07-cancel-owner-dropped | C-1: the owner can no longer cancel | test_requirements (C-1 authority) |
| c08-fixture-self-executor | C-5 CASH-SELF: a hiring recipe's actor is the executor | test_requirements (C-5 fixture conservation); test_requirements (C-5 CASH-SELF) |
| c10-validate-accepts-self-executor | F-4 revert (FR-5): Config::validate no longer rejects actor = executor (v1.0.0 behaviour); C-5 CASH-SELF-ENFORCED must fail. Replaces c09, whose change (validate rejects actor = executor) is the v1.1.0 code. | test_requirements (C-5 fixture conservation); test_requirements (C-5 reproduces no finding) |
| c11-full-ignores-front | v1.1.0 record bound (F-3 / FR-3): submit fails Full whenever 256 records are retained, even if the oldest is terminal (spec:21, api:28: Full only while the oldest is open). The outcome order is unchanged; C-2 compares the extracted condition with guard_condition(submit, record_bound, ...). | test_requirements (C-2 guard order) |
| c12-observe-plain-bump | F-5 revert (FR-6) in observe: bump() in place of bump_new(), so observe no longer stops at VERSION_CAP (spec:50, api:30). The outcome names still line up (Unauthorized, Pending, Bounds, Arithmetic); C-2 requires the version_cap guard to be a site with the cap condition. | test_requirements (C-2 guard order) |
| d01-best-tie-smaller | RD-BEST (code side): the real core's best_quantity breaks an energy tie to the smaller units; reads.txt is regenerated from this core | test_reads (RD-PREVIEW/RD-BEST differential) |
| d02-preview-allows-paused | RD-PREVIEW (code side): the real core's preview allows a request that candidate rejects as Paused; submit still rejects it | the generator ("preview and submit disagree") |

The model mutants were run on a copy of this directory with the v1.1.0
corpus and its trace reader; every suite was run for each mutant, and the
table lists the `# expect:` lines (most mutants also fail other tests).
The reads mutants rebuild the generator from the mutated core: d01 leaves
`core.txt` unchanged and changes `reads.txt`, where `test_reads` finds 1,645
divergences; d02 stops the generator with 4,919 preview/submit disagreements
(every `Paused` request).
No property can see two of the mutants on a reachable state. m17 (a release
skips the bounds check): under
INV-RES a release or completion never fails `Bounds`, so only the hand-built
v1.0.0 wedge state in `test_model` kills it. m23 (the ±1e6 check covers the
completion endpoints only): `Bounds` compares the same hull, release terms
included, with the metric's bounds, which lie inside ±1e6, and values are
always inside ±1e6; so an endpoint past ±1e6 always exceeds the current
violation, and the submit is still refused, as `Bounds` instead of
`Arithmetic`. MC-1 `f2b` reaches the same states and edges under m23 (147
`Arithmetic` refusals become 131 `Arithmetic` and 16 `Bounds`, no
violation); the replay of trace 208 and `test_model` F-2b kill it. The
full revert of the release term, m21, is killed by MC-1 `f2b` (LIVE-ALL).
m26 (prerequisites counted over the retained window) only refuses more, so
no safety property can see it; it is killed by MC-1 `window`'s reachability
check (no prerequisite is admitted after its record was pruned), by
`test_model` F-3 and by trace 207's exact replay (`ok(257)`).

## What is not shown

- **Independence (ASM-SPEC).** The author of this layer had read the code.
  Doc-backed facts cite their doc line and code-only facts are labelled, but no
  independent reviewer has checked the transcription.
- **Bounds.** MC-1 explores seven small universes (at most 3 records or a
  window of 1-2, 1-3 times, fixed observation vectors, and in `cap` the last 4
  versions below a scaled cap). The real window (256) and cap (u32::MAX - 768)
  appear only in the unit tests and the traces. C-5's release search covers two
  records from each fixture's init state and the trace states.
- **MC-2 coverage.** The 8,865 trace calls reach every op kind's Ok and all 15
  reachable error kinds, but `full` only once (trace 207) and `arithmetic` 3
  times; no trace comes near VERSION_CAP (only the reads have states there),
  so VER-CAP and INV-VERCAP on the real core rest on Kani, the unit tests and
  MC-1 `cap`.
- **Reads.** COV-2 tests the real core's `preview` and `best_quantity` on the
  recorded requests only (including 12 states at VERSION_CAP or u32::MAX,
  where a request that passes every other check is refused `Arithmetic`):
  other states and request values, the
  per-term energies and the `name` and `unit` strings are not compared.
- **C-2 unreachable sites.** The alignment skips ten kinds of code error site
  that cannot fire from an INV state; each rule names what discharges it (see
  `consistency.pl`), and this layer assumes them.
- **TLA leg.** C-2 and C-3 compare `GuardOrder` and `Edges` as written in the
  TLA+ files; that the TLA+ operators follow them is the TLA layer's REF-5.
- **Extraction (ASM-TOOLS).** `extract.py` is a narrow parser over this pin's
  sources; code mutants c01-c12 show that it sees the changes they make.
- **Shell and strings.** Runtime shell behaviour belongs to near-tests and
  sandbox; name, unit and owner length validation is not modelled.
- **MC-1 canary.** `test_mc1` has no canary run. Its non-vacuity comes from the
  counts and witnesses it asserts (the hedge rejected, prunes, Full on an open
  front, a pruned prerequisite, NotFound, refreshes, each gated call stopped at
  the cap) and from the mutants that it kills.
