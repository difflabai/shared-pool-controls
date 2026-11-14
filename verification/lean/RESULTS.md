# shared-pool: Lean layer results (v1.1.0)

Runner: `projects/shared-pool/lean/run.sh [build|audit|canary|regression|replay|mutants]`
(default: all; exit 0 = every check passes). Lean 4.34.1, core library only
(no Mathlib). Pin: shared-pool-controls `57b3f02d` (the PR #4 merge commit,
v1.1.0; its tree equals the PR head `6fcbd566`), `crates/core/src/lib.rs`.
The v1.0.0 results (pin `a4bb5c3c`) are in this file's history.

## What is proved (every size, kernel-checked)

`PoolLean/Machine.lean` transcribes the v1.1.0 core: each call is its guard
list in the code's order (first firing guard = outcome; errors and no-ops
stutter), then `effect`. Values are `Int`, counters `Nat`, accounts a type
parameter `A` with decidable equality, so every theorem holds for every
number of metrics, recipes, terms, records and accounts, any config
`validate` accepts (`Valid`, CFG-2's checks, actor ≠ executor included),
any times and any argument values.

History encoding (strategy section 18.1): `records` is the whole history
(record `j` has id `j`) and `firstId` is the code's `first_id`; the code's
window is `State.win = records.drop firstId`. The lookups read only window
quantities (`firstId ≤ id < records.length`), and the envelope and
observe's "some record is open" equal their window values because a
pruned record is terminal (`envS_win`, `anyOpen_win`). `completed_units[r]`
is `doneU s r = min (Σ Completed units of r over the history) u32::MAX`.
`VERSION_CAP = u32::MAX − 3·256`: `bump_new` (`capGuard`, then the plain
bump) gates submit, dispatch, set_paused and observe; ack, cancel and
settle use the plain `checked_add` (`verGuard`). Ghosts: `base`/`k0`
(values and next id at the last re-basing `observe` or `new`; a refresh,
the same values while a record is open, keeps them) and `eff`.

`Inv` (Props.lean) = INV-WF (with WIN-1, WIN-2) ∧ INV-LEDGER ∧ INV-RES
(the envelope is computable and EV_i = PV_i(values)) ∧ PV_i(values) ≤ B_i ∧
INV-DEP ∧ INV-VER ∧ INV-VERCAP (`version + Σ rem ≤ u32::MAX`, rem Reserved
3, Dispatched 3, Unknown 2, Active 1, terminal 0). INV-HARD is the
corollary `hard_of_inv`.

| ID | Theorem(s) | Statement |
|----|-----------|-----------|
| LEAN-IND | `inv_init`, `inv_step`, `reach_inv`, `hard_of_inv`, `bump_ok`, `remSum_le` (Inv.lean) | `Valid cfg ⇒ Inv (init cfg now)`; `Inv s ⇒ Inv (step s op)` for EVERY op (no validity premise on the step); so every reachable state satisfies `Inv`. `hard_of_inv`: INV-HARD. `bump_ok`: with an open record, `version < u32::MAX` (NO-OVF of the plain bump). `remSum_le`: Σ rem ≤ 3·256 |
| LEAN-LIVE-ALL | `res_sub`, `close_ok`, `live_all`, `live_all_reach`, `no_wedge` (Live.lean) | LIVE-ALL with no premise beyond `Valid` and `Inv`: in every reachable state a Reserved record's `cancel` by its actor and by the owner, and an in-flight record's `settle(executor, id, b)` for both b, pass every guard; `no_wedge`: every open record of every reachable state is made terminal by one `Ok` call. `res_sub`: closing an open record shrinks the envelope to a sub-box containing the new values, so its violation is at most both the old and the new values' (the `check_bounds` test) |
| LEAN-LIVE1 | `live1`, `live1_exec`, `live1_reach` (Live.lean) | LIVE-1, now a corollary of LIVE-ALL: `Valid`, `Inv` ⇒ `settle(executor, id, true)` passes every guard and completes each in-flight record. The v1.0.0 premises (B = 0, `version < u32::MAX`) are gone |
| INV-HEALTHY | `healthy` (Live.lean) | INV-OUTCOMES: `Inv` ∧ B = 0 ⇒ for every choice of outcome per open record (completed, released or left open), values + the chosen deltas lie in [min, max] on every metric |
| LEAN-STEP | `stepProps`, `stepProps_step`, `exec_auth`, `term1`, `term1_cancel`, `stale1_no_replay`, `version_mono_run` (StepProps.lean) | From every state (no `Inv` premise): TX-0, TX-NOOP/TX-VER, LIFE-1 (append-only, frame, status graph, fresh = Reserved with id = index), the prune step (`firstId` grows by at most 1, pruned records unchanged), AUTH-1, STALE-1, ADM-1, ADM-2, HARD-STEP, ASYNC-1; TERM-1 for settle and cancel (cancelling a Completed record is `InvalidState`) |
| (also) | `dep1`, `dep1_iff`, `obs1`, `obs2`, `refresh_ok`, `cap_stops`, `snapshot_fresh`, `pause1_blocks`, `pause1_transparent` (Calls.lean) | DEP-1 (an accepted submit's prerequisites are met by `completed_units`, hence by the history's Completed units) and its converse `dep1_iff` (the guard reads the lifetime count: pruned completions count); OBS-1 and OBS-2 (an `Ok` observe with a record open changes neither values, records, `base` nor `k0`); REFRESH-1 (`refresh_ok`: below the cap, the owner's observe of the current values is `Ok` in every `Inv` state); VER-CAP (`cap_stops`: from `VERSION_CAP` submit, dispatch, set_paused and observe all fail); the snapshot age bound is inclusive; PAUSE-1 |
| WIN-1, WIN-2, WIN-ESC | `win1`, `pruned_terminal`, `pruned_notFound`, `envS_win`, `anyOpen_win`, `full_iff_front_open`, `submit_not_full_of_front_terminal`, `win_esc`, `submit_prunes` (Window.lean) | WIN-1: at most 256 retained, ids `firstId + k`, `firstId + len = records.length ≤ version` (so the id's `checked_add`, lib.rs 363-366, never fails). WIN-2: a pruned record is terminal and every call naming it is `NotFound`. WIN-ESC: `Full` iff 256 retained and the front open; from any reachable `Full` state one `Ok` call resolves the front and the Full guard then passes; a submit at 256 retained prunes exactly the oldest and returns id `firstId + 256` |
| CFG-2 | `valid_actor_ne_executor`, `valid_iff` (Window.lean) | `validate` decides `Valid` (`valid_iff`); a valid recipe's actor is not its executor (AUTH-SELF; the owner may execute) |
| LEAN-GUARD | `Guard.guards_order`, `Guard.outcome_mem` (Guard.lean) | the outcome codes of every call's guard list are exactly `guardOrder` (written from lib.rs at the pin with line numbers; `pool-lean-guards`, Guards.lean, prints it), and every outcome is one of them. Checked against the code: each entry against its lib.rs line at 57b3f02, and each call's list against its `code_events` sequence in `prolog/extracted/code_facts.pl` with the helper calls expanded (the consecutive `Arithmetic` sites inside `add` and `envelope` are one entry each; `energy`'s `Arithmetic` sites are absent because the model's energy is exact, NUM-1). Compared with W3's guard table (guard/5), which is coarser: it has one row per check the spec names and leaves out the sites that cannot fire in an `Inv` state (decision `c2-unreachable`). With those sites dropped from `guardOrder` (the `InvalidConfig` length checks in `add`, the second `NotFound` lookups in `release` and `index`, the `energy_terms` `Bounds` checks, the `add` in submit's recovery check, the plain bump inside `bump_new`, submit's version and id `checked_add`), the two lists are equal for all eight call kinds (a script over `pool-lean-guards` and guard/5; not part of `run.sh`) |
| LEAN-WIT | Witness.lean | see below, all by `decide`/`rfl` in the kernel (the cap state by induction) |
| REF-5L | `pool-lean replay` (Main.lean) | the committed v1.1.0 corpus: 216 traces (1 refused at init), 8,865 calls (2,565 `Ok`), every result and post-state equal; see "Replay" below |

### LEAN-WIT (Witness.lean)

Configs are the v1.1.0 corpus's traces; accounts o = 0, a1 = 1, x1 = 2,
a2 = 3.

* F-1 (trace 0): `wcfg_valid`; `f1_rejected`: after `observe [12,5]` the
  F-1 hedge is rejected `Bounds` (v1.0.0 admitted it and wedged);
  `f1_envelope`: the hull that rejects it (low 10, high 12, EV₀ = 2 against
  the candidate's PV₀ = 0). `recovery_admitted` (trace 214): the recovery
  hedge, whose completion lowers metric 0, is admitted and settles to
  [9, 4].
* INV-RES is needed: `res_needed`: the v1.0.0 wedge state satisfies WF,
  Ledger, PV ≤ B, Dep, Ver and VerCap but not INV-RES, and both of its
  executor's settles fail `Bounds`; `live_all_needs_res` refutes LIVE-ALL
  without INV-RES (the statement of the canary `LiveAllNoRes`).
* F-2 (trace 209): `f2cfg_valid`, `f2_rejected`: the hedge admitted on
  v1.0.0 on the strength of an open position is rejected `Bounds`.
* F-4 (traces 210, 213): `self_executor_invalid` (`validate` = false and
  ¬ `Valid` when an actor is its own executor); `owner_executor_valid`.
* F-5: `cap_reach`: a state at `VERSION_CAP` = 4294966527 with a
  Dispatched and a Reserved record is reachable from `init` (`pause_run`,
  by induction); `cap_instance`: there submit, dispatch, set_paused and
  observe fail `Arithmetic` while both settles, the cancel and the ack are
  `Ok`; `cap_drain`: both records are resolved past the cap;
  `cap_headroom`: 762 of INV-VERCAP's headroom is left.
* F-6 (trace 211): `refresh_instance`: with records open and the snapshot
  expired, a changed observation is `Pending`, the owner's refresh is
  `Ok`, keeps values and records, and unblocks `dispatch`.
* Non-vacuity of the healthy side: `hstate_reach`, `live1_premises`,
  `live1_instance` (`live1` applied on a reachable state), `healthy_settle`,
  `unknown_settle`, `cancelled_reach`, `stale_instance`, and
  `cancel_completed_instance` (TERM-1: cancelling a Completed record is
  `InvalidState`).

## Vacuity guards and hygiene

* Axiom audit (`Audit.lean`): 65/65 headline theorems depend only on
  `propext`, `Classical.choice`, `Quot.sound` (several on fewer). 189
  theorems in all.
* Canaries, each must FAIL to compile, and does: `canary/False.lean`
  (`False`), `canary/NotInv.lean` (every state satisfies `Inv`),
  `canary/LiveAllNoRes.lean` (LIVE-ALL with INV-RES dropped from `Inv`,
  false by `Witness.live_all_needs_res`).
* Regression, must COMPILE, and does: `regression/Live1NoPremise.lean`.
  It is the v1.0.0 canary `canary/Live1NoPremise.lean` with its statement
  unchanged (LIVE-1 without B = 0): it failed to compile on v1.0.0 (F-1,
  F-5; re-checked on `a072a2d`) and is `live1` on v1.1.0.
* Hygiene scan (comments stripped): no `sorry`, `admit`, `axiom`,
  `native_decide`, `implemented_by`, `extern`, `unsafe`, `opaque` in
  PoolLean/, Main.lean, Guards.lean, Audit.lean, canary/, regression/. The build prints
  no `declaration uses 'sorry'`.

## Replay (REF-5L)

`run.sh replay` builds `pool-lean` (Main.lean: Machine and Text, not Guard)
and replays `conformance/traces/core.txt`,
the v1.1.0 corpus (216 traces, 8,865 calls, 2,565 `Ok`; states `s/7`),
committed together with the parser (`Text.lean`, `Main.lean`: states with
`first_id` and `completed_units`, `init_error`). Every result and
post-state is equal (values, version, observed_at, paused, the retained
records, first_id, completed_units); `validate` accepts every `init`
config and refuses trace 210's (`init_error(invalid_config)`). The old
parser cannot read this corpus and the new one cannot read the v1.0.0
corpus (`s/5`), so the two were committed together.

## Mutants

26 hand-written mutants of Machine.lean (`mutants/*.patch`, applied with
`patch -p1` in a copy of this directory): the 16 v1.0.0 mutants
re-anchored (01, 04 and 08 re-targeted), and 17-26 that each restore one
v1.0.0 behaviour (fix-revert ids FR-1..FR-8 in each patch header).
`run.sh mutants` runs the replay (only when the unmutated machine
replays the corpus, which it does on the committed v1.1.0 corpus), the
full `lake build`,
and, for 17-26, a property probe (`mutants/probes/`: a concrete state on
which the named property fails; it must fail to compile unmutated and
compile on the mutant). All 26 are killed by the build; none survives.

| Mutant | FR | Build: first module (theorem) | Probe | Replay |
|--------|----|------|-------|--------|
| 01-refresh-accepts-changed-values (Pending dropped; re-target of observe-pending-dropped) | | Facts (`observe_ok`) | | killed |
| 02-envelope-skip-ignored | | Lemmas (`envS_split`, `envS_close`) | | killed |
| 03-bounds-slack (EV ≤ min + 1 tolerated) | | Lemmas (`boundsFail_false`) | | killed |
| 04-cap-off-by-one (`>` for `>=` at the cap; re-target) | | Facts (`submit_ok`, `dispatch_ok`, `pause_ok`, `observe_ok`) | | survives (no trace reaches the cap) |
| 05-release-adds-reserve | | Facts (`release_ok`) | | killed |
| 06-cancel-any-caller | | Facts (`cancel_ok`) | | killed |
| 07-ack-status-swapped | | Witness (`unknown_settle`) | | killed |
| 08-submit-id-ignores-first-id (id = window index; re-target) | | Inv (`inv_submit`: record id = index) | | survives (the trace format shows no record ids) |
| 09-guard-order-paused-stale | | Guard (`guards_order`), Facts (`submit_ok`) | | killed (trace 3) |
| 10-prereq-ignores-units | | Inv (`inv_submit`: INV-DEP) | | survives |
| 11-unknown-not-settleable | | Live (`no_wedge`) | | killed |
| 12-adm1-energy-check-dropped | | Facts (`submit_ok`) | | killed |
| 13-observe-keeps-k0 | | Inv (`inv_observe`) | | survives (ghost) |
| 14-snapshot-age-inclusive | | Calls (`snapshot_fresh`) | | killed |
| 15-settle-complete-uses-reserve | | Facts (`settle_ok`) | | killed |
| 16-dispatch-ignores-pause | | Facts (`dispatch_ok`) | | killed |
| 17-hull-without-release-term | FR-2 | Live (`release_guards_ok`, `healthy`) | LIVE-ALL: F-2 shape, the position's cancel fails `Bounds` | killed (trace 30) |
| 18-bounds-against-old-only (`v > old`) | FR-1 | Lemmas (`boundsFail_false`) | LIVE-ALL: the F-1 wedge is reachable, both settles fail | killed (trace 0) |
| 19-full-ignores-front | FR-3 | Inv (`inv_submit`) | WIN-ESC: Full with a terminal front | killed (trace 207) |
| 20-prune-keeps-first-id | FR-3 | Inv (`inv_submit`: WIN-1 bound) | WIN-1: 257 retained | killed (trace 207) |
| 21-prereq-over-window | FR-4 | Inv (`inv_submit`) | DEP-1 (`dep1_iff`): a pruned completion no longer counts | killed (trace 207) |
| 22-valid-allows-self-executor | FR-5 | Window (`valid_actor_ne_executor`) | CFG-2: trace 210's config validates | killed (trace 210) |
| 23-dispatch-plain-bump | FR-6 | Guard (`guards_order`), Facts (`dispatch_ok`: the cap) | VER-CAP: dispatch passes at the cap | survives (no trace reaches the cap) |
| 24-refresh-rejected (Pending whenever a record is open) | FR-7 | Facts (`observe_ok`) | REFRESH-1: the refresh is `Pending` | killed (trace 27) |
| 25-cancel-completed-pending | FR-8 | Guard (`guards_order`), Facts (`cancel_ok`) | TERM-1: cancel of Completed is `Pending` | killed (trace 4) |
| 26-refresh-rebases | FR-7 | Inv (`inv_observe`) | OBS-2: an accepted refresh re-bases `k0` | survives (ghost) |

"Build: first module" is where lake stops (it does not build a failed
module's dependents); the theorems are those whose proofs break there.
Several first breaks are proof shape rather than a false statement: 19
(Inv still holds when Full fires more often), 21 (INV-DEP still holds when
the guard counts fewer units), 24 and 25 (the Facts restatements still
hold); the probes show the named property itself fails on each. The
replay column is `run.sh mutants` at 6b120fd (2026-11-14): 09 diverges
first in trace 3, 25 in trace 4, and 17, 18, 19-21, 22 and 24 at the
traces named. `pool-lean` imports only Machine and Text, so it builds on
every mutant here, including 09, 23 and 25, whose `guards_order` (Guard)
breaks. Before 6b120fd it also imported Guard (for the guard printer, now
`pool-lean-guards`), and `run.sh` printed "killed by replay" for those
three when `pool-lean` did not build. That was a build failure, not a
trace divergence. `run.sh` now prints "no replay (pool-lean does not
build)" in that case. The column is the same as the first scratch
measurement, which was taken with Guard dropped from Main (23 survives
the replay).

## Run times (darwin arm64, this workspace)

| Check | Time |
|-------|------|
| `run.sh build` (+ hygiene scan) | 10 s (every module rebuilt; 3 s at 6b120fd, where only Main and Guards rebuilt) |
| `run.sh audit` (65 `#print axioms`) | 1 s |
| `run.sh canary` (3 files) | 1 s |
| `run.sh regression` (1 file) | under 1 s |
| `run.sh replay` | 1 s (a7781c7, the v1.1.0 corpus; 3 s in the scratch copy) |
| `run.sh mutants` (26 mutants, 10 probes) | 178 s at 6b120fd (174 s at a7781c7), one mutant at a time, each with the replay (135 s before the corpus was committed, when the unmutated replay failed and the per-mutant replay was skipped) |
| whole run, all six checks | 185 s at 6b120fd (182 s at a7781c7; 148 s before the corpus was committed) |

## What is NOT shown here

* Transcription fidelity is not a proof: Machine.lean is a hand
  transcription of lib.rs. REF-5L (the v1.1.0 corpus) checks it against
  real-core traces, which is evidence, not proof, of equivalence. The
  history encoding is a representation choice; `envS_win`, `anyOpen_win`
  and `win1` show the guards' quantities equal the window's, and the
  replay compares the window, `first_id` and `completed_units` with the
  code's.
* Numeric representation (stated in Machine.lean's header): the model
  computes exactly in `Int`/`Nat`; that the code's i64/u64 arithmetic
  never overflows on these paths is NUM-1/NUM-2 (Kani's job). `energy` is
  the exact Σ w·r²; the code's `energy` Arithmetic error is not modelled.
  The envelope's fail-on-first partial sum is modelled by the final
  low/high sums (argued in the header), not proved against a step-by-step
  model. The u32 `version` (both bumps) and the u32 saturation of
  `completed_units` are modelled exactly.
* Times, units and versions are unbounded `Nat` in the model (the code's
  u64/u32 types bound the inputs). `max_age`'s u32 type and all names and
  strings are not modelled.
* Panics (ROB-1) are not modelled: out-of-range reads give defaults. `Inv`
  shows every index the guards read is in range in every reachable state.
* RD-PREVIEW and RD-BEST are not modelled. NUM-1, NUM-2, CFG-1 and the
  shell (SH-*, EX-*, RT-*) are other layers' work.
* LIVE-2 and WIN-LIVE (eventual termination under fairness) are temporal
  and not stated in Lean; LIVE-ALL and WIN-ESC are their per-state
  enabling halves.
* TX-SUBMIT..TX-OBSERVE (exact results and post-states) are the model's
  definitions plus REF-5L, not separate theorems.

## Findings

* F-1, F-2, F-4, F-5 and F-6 are fixed in the model at every size: LIVE-ALL
  holds in every reachable state with no premise, the F-1 and F-2 hedges
  are rejected, `validate` refuses an actor that executes, new work stops
  at `VERSION_CAP` while every open record can still be resolved, and the
  owner can always refresh the snapshot. F-3's window lock is gone
  (WIN-ESC). Cancelling a Completed record is `InvalidState` (TERM-1).
* v1.0.0 evidence: on `a072a2d` (v1.0.0's Lean layer) the canary
  `Live1NoPremise` fails to compile and `Witness.f1_forever` (the wedge is
  permanent) is proved; the fix-revert mutants 17-26 restore each v1.0.0
  behaviour and are killed.
* No disagreement between the model and the candidate v1.1.0 corpus.
