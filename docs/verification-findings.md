# Verification findings for v1.0.0

This page lists the problems an outside verification effort found in
v1.0.0 of the contract.

**Status.** v1.1.0 fixes every finding below. Each finding has a
**Status** line with the fixing commit and the tests that check the fix;
[CHANGELOG.md](../CHANGELOG.md) lists the changes. The formal results cited
here apply to v1.0.0. v1.1.0 has not been formally re-verified: its fixes
are checked by tests in this repository only.

**Reproducers and regression tests.** In v1.0.0 each core finding had a
reproducer test in `crates/core/src/findings.rs` (commit `94f27ff`) that
passed and asserted the faulty result. Each v1.1 fix rewrote its
reproducer into a regression test of the fixed behaviour. The **Reproducer**
paragraphs below name the v1.0.0 tests; the **Status** lines name the v1.1.0
tests. The shell findings point to the tests that show them in the
verification project.

**Scope.** The verification ran on commit `a4bb5c3`, the head of PR #1.
`crates/core/src/lib.rs`, `crates/contract` and `crates/executor` are
byte-identical between `a4bb5c3` and v1.0.0 (`80fb89e`). The rest of the
diff is package versions, the `station` scenario, fixture and tests, the
demo, docs, UI, `scripts/smoke.py` and the Pages workflow.
Every finding below applies to v1.0.0.

**Line references.** `spec:N`, `api:N` and `handoff:N` are line N of
`docs/spec.md`, `docs/api.md` and `docs/verification-handoff.md` in v1.0.0.

**Formal evidence.** In [difflabai/formalver](https://github.com/difflabai/formalver),
draft [PR #31](https://github.com/difflabai/formalver/pull/31), under
[`projects/shared-pool/`](https://github.com/difflabai/formalver/tree/b9c495b8fa47e395d662a348f1cc7e352018aea5/projects/shared-pool)
at commit `b9c495b`. Paths in this page are relative to that directory.
Section 4 of its `strategy.md` is the full record. The **Evidence**
paragraphs below cite results as that section reports them. Only the
reproducer tests in `crates/core/src/findings.rs` run in this repository.
The TLC, Lean, Prolog, shell and sandbox results were not re-run for this
page.

## How the findings were found

The formalver project checked the code with several independent methods:

- **TLA+.** A model of the core, checked with TLC for safety and liveness
  in small universes.
- **Lean 4.** A transcription of the core, with proofs about the
  transcription. They are not proofs about the Rust code.
- **Prolog.** A reference model with consistency checks against the docs,
  an exhaustive search of small state spaces (MC-1) and a trace replay
  (MC-2).
- **Conformance traces.** 212 call sequences generated from the real
  `pool-core`. TLA+, Lean, Prolog and the shell tests replay each one and
  get the same results and post-states.
- **Host and wasm tests of the NEAR shell.** The `#[near]` methods called as
  Rust functions, and both wasm builds (`scripts/build.sh`, and the same
  with `--cfg near`) run in an interpreter over near-vm-runner's mocked
  runtime.
- **Sandbox.** Both wasm files on a local near-workspaces chain, compared
  with the host core.
- **Kani** (bounded model checking of the Rust core) is in progress. It has
  produced no results yet.

Nothing here is a proof about the Rust core or the NEAR shell. The Lean
results hold for the transcription, and the TLC and MC-1 results hold
within their bounds. The rest are tests. The findings do not depend on any
of that: each one is a concrete call sequence that gives the stated result
on the real code.

## Summary

Severity rates the harm to a property the docs promise.

| ID | Severity | Effect | v1.0.0 reproducer | Status |
|---|---|---|---|---|
| F-1 | high | After an `observe` breaks a bound, an admitted recovery hedge can be left with no way to settle, and the pool cannot be observed again (shown for the trace-0 configuration). | `f1_disturbed_state_hedge_can_never_settle` | fixed in v1.1.0 (`59e404f`) |
| F-2 | high | In the supplied `risk` fixture, with every metric in bounds, a position that a completed hedge relied on cannot be released by `settle(false)` or `cancel`. | `f2_risk_position_cannot_be_released_after_hedge_completes`, `f2_risk_reserved_position_is_stuck_after_hedge_completes` | fixed in v1.1.0 (`59e404f`) |
| F-2b | high (part of F-2) | A release is not bounded by the envelope, so it can fail `Arithmetic`. | `f2b_release_outside_value_range_fails_arithmetic` | fixed in v1.1.0 (`59e404f`) |
| F-3 | low | One actor can use all 256 lifetime records with free submit-and-cancel cycles. Every later `submit` fails `Full`. | `f3_submit_cancel_cycles_exhaust_lifetime_records` | fixed in v1.1.0 (`1f01d43`) |
| F-4 | low | `validate` accepts a recipe whose actor is its executor, or the owner. That account settles its own work. | `f4_validate_accepts_actor_or_owner_as_executor` | fixed in v1.1.0 (`dc774f8`) |
| F-5 | low | At version u32::MAX every transition fails `Arithmetic`, including settlement and unpausing. | `f5_version_max_freezes_settlement_and_every_transition`, `f5_paused_pool_at_version_max_cannot_unpause` | fixed in v1.1.0 (`da43188`) |
| F-6 | low | With `max_age_seconds > 0`, one long-open record blocks `observe`, so `submit` and `dispatch` fail `SnapshotExpired` until it settles. | `f6_open_record_blocks_the_snapshot_refresh` | fixed in v1.1.0 (`3eaf1cb`) |
| NT-F1 | medium (diagnostics, not safety) | The documented build omits `--cfg near`. Every on-chain failure is a bare trap with no message. | formalver `near-tests`, `sandbox` | fixed in v1.1.0 (`effd174`) |
| NT-F2 | low | `reconcile` accepts and keeps an attached deposit. | formalver `near-tests` | fixed in v1.1.0 (`1d441c5`) |
| NT-F3 | informational | The wasm imports 15 host functions for accounts, deploys, keys, transfers and staking. | formalver `near-tests` | fixed in v1.1.0 (`3598902`) |
| NT-F4 | low | A promise result that is not a JSON string makes the callback panic. The record does not become Unknown. | formalver `near-tests` | fixed in v1.1.0 (`1d441c5`) |

## Core findings

### F-1 (high): disturbed-state hedge wedge

**Status.** Fixed in v1.1.0 (`59e404f`). Admission now counts every
outcome of every open record (stay open, complete, release), and compares
each metric's interval violation with the smaller of its violation before
the request and after the reservation. In this configuration the hedge is
refused (`Bounds`), and every open record can always be resolved.
Regression tests: `f1_hedge_that_cures_by_reservation_is_refused`,
`f1_recovery_by_completion_is_admitted_and_live`, `live_all_on_random_runs`.
Formal re-verification pending.

**What happens.** An `observe` puts metric 0 at 12, above its max of 10.
The recovery rule (spec:21, spec:23) admits a hedge that reserves −2 on
metric 0 and completes +2. Admission compares the envelope's violation (2)
with the current one (2), so it passes. The completion lowers the potential
(metric 1 goes from 5 to 0), so the strict-decrease rule passes too. Once
the hedge is reserved, metric 0 is 10 and the pool is healthy. Every later
check compares with a violation of 0:

- `settle(true)` fails `Bounds`: the completion takes metric 0 to 12.
- `settle(false)` fails `Bounds`: the release takes metric 0 back to 12.
- `cancel` fails `Pending` for the actor and the owner, because the record
  was dispatched.
- `observe` fails `Pending`, because a record is open (spec:54).

In this configuration no later call helps: the record never becomes
terminal, and the pool can never be observed again. In other
configurations a later admission that lowers metric 0 can let
`settle(true)` pass, as in F-2. On a local chain the executor commits "completed", and its
notification to the pool fails `Bounds`. The executor then refuses to change
its answer, so the two contracts disagree permanently.

**Why it matters.** spec:46 says a checked inverse "can fail if another
commitment depends on its contribution" and to "resolve dependent work
first". Here no other commitment exists, so there is nothing to resolve.
handoff:31 makes liveness conditional on "resolving dependencies and
receiving authoritative evidence". The executor's evidence arrives, and
neither outcome can be applied.

**Reproducer.** `f1_disturbed_state_hedge_can_never_settle`; formalver
trace 0.

```text
config   metric 0 in [0, 10] initial 5, metric 1 in [-100, 100] initial 5
         term    metric 1, target 0, weight 1, Target
         recipe  0: actor H, executor X, reserve [-2, 0], complete [2, -5], max_units 1
observe(owner, [12, 5], 100)                     Ok        metric 0 is 2 over its max
submit(H, {recipe 0, units 1, version 1}, 100)   Ok(0)     values [10, 5]
dispatch(H, 0, 101); acknowledge(0, true)        Ok        record 0 Active
settle(X, 0, true)                               Bounds    would give [12, 0]
settle(X, 0, false)                              Bounds    would give [12, 5]
cancel(owner, 0, 102); cancel(H, 0, 102)         Pending
observe(owner, [5, 5], 103)                      Pending
```

**Evidence.**
- Lean: `f1_forever` and `f1_never_terminal` in
  `lean/PoolLean/Witness.lean`. For this configuration, every later call
  sequence leaves the record Active and every `observe` failing. This is a
  result about the Lean transcription.
- TLA+: `Witness_Wedge` (`tla/Props.tla`) reaches the wedge at depth 4
  under `MC_Safety`.
  TLA-LIVE-F1 (`tla/MC_Liveness_F1.cfg`) violates LIVE-2.
  `MC_Liveness_F1Shape.cfg` shows that, in its small universe, every LIVE-2
  counterexample is this wedge.
- Prolog: `prolog/tests/test_model.pl` "F-1 witness". MC-1 universe `wedge`
  has 228 dead states.
- Shell: `near-tests/tests/wedge.rs`, from Dispatched, Unknown and Active,
  on the host and both wasm builds.
- Sandbox: `rt_wedge_f1_end_to_end` in `sandbox/tests/runtime.rs`
  (RT-WEDGE in `sandbox/RESULTS.md`).

### F-2 (high): healthy-state release wedge in the `risk` fixture

**Status.** Fixed in v1.1.0 (`59e404f`), by the same change as F-1. With
the position open, 8- and 7-unit hedges are refused and 6 units are
admitted. After the hedge completes, the position can still be released by
`settle(false)` or `cancel`. Regression tests:
`f2_hedge_cannot_rely_on_an_unexecuted_position`,
`f2_reserved_position_can_always_be_cancelled`. Formal re-verification
pending.

**What happens.** There is no `observe`, and every metric is in bounds. A
credit position (recipe 0, 1 unit) reserves +20 selloff loss: 60 becomes
80. A selloff hedge (recipe 3, 8 units, −10 selloff loss per unit on
completion) is admitted only because that +20 is counted: the low end of the
envelope is 80 − 80 = 0. Without the position, the same hedge fails
`Bounds` (60 − 80 < 0). Both are dispatched. The hedge completes, and
selloff loss is 0. The position's trade never ran, so the executor's honest
report is `settle(false)`. Releasing the +20 would give 0 − 20 < 0:

- `settle(false)` fails `Bounds`.
- `cancel` fails `Pending` for the actor and the owner.
- `observe` fails `Pending`.

`settle(true)`, a report of a trade that did not happen, succeeds. While the
snapshot is fresh, a new admission that adds 20 selloff loss (2 equity
units) also lets the release pass, at the cost of new, unrelated exposure.
After the snapshot expires (observed at 100, `max_age_seconds` 300),
`submit` fails `SnapshotExpired`. Then `settle(true)` is the only call that
ends the record.

If the position was never dispatched, the same check makes `cancel` fail
`Bounds`. With a 7-unit hedge, selloff loss ends at 10 and the release would
give −10. Once the request has expired and the snapshot is stale, `dispatch`
fails `Expired`, `submit` fails `SnapshotExpired`, `settle` fails
`InvalidState` and `observe` fails `Pending`. No call changes the record or
the values again; `set_paused` only changes the pause flag and the version.
formalver's strategy.md notes that the same check blocks `cancel` of a
Reserved position. The expiry step is not in formalver; this repository's
`f2_risk_reserved_position_is_stuck_after_hedge_completes` shows it.

**Why it matters.** spec:46: "In the supplied fixtures, physical
availability and risk-credit timing prevent ordinary unsafe reliance on
unexecuted work." Here a hedge in a supplied fixture relies on an
unexecuted position. spec:46 also says "resolve dependent work first". The
dependent hedge is resolved (Completed), and the release still fails.
api:14 says `settle(false)` means "definitive nonexecution". The honest
report is refused, and the false one is accepted. handoff:23 (RISK) is the
related obligation. It is not contradicted: the hedge's headroom comes from
the position's pending reservation, not from pending negative exposure.

**Reproducer.** `f2_risk_position_cannot_be_released_after_hedge_completes`;
formalver trace 209.

```text
fixture  risk: selloff, funding, volatility, collateral = [60, 40, 30, 40]
         max_age 300, observed at 100
submit(credit, {recipe 0, units 1}, 100)      Ok(0)     values [80, 45, 30, 35]
submit(macro, {recipe 3, units 8}, 100)       Ok(1)     values [80, 45, 30, 19]
dispatch and acknowledge(true) for 0 and 1    Ok        both Active
settle(executor, 1, true)                     Ok        selloff loss 0
settle(executor, 0, false)                    Bounds    release would give -20
cancel(owner, 0); cancel(credit, 0)           Pending
observe(owner, [60, 40, 30, 40], 102)         Pending
submit(equity, {recipe 1, units 2}, 401)      SnapshotExpired
settle(executor, 0, true)                     Ok
```

**Evidence.**
- Trace 209 (`f2-risk`, from `f2_risk()` in `conformance/src/main.rs`),
  reported equal under TLC, Lean, Prolog (MC-2) and the shell.
- Prolog: `prolog/tests/test_model.pl` "F-2 witness" (7-unit hedge). The
  C-5 release search (`prolog/verification/consistency.pl`) finds 20
  counterexamples in `risk` and none in `gpu`, `friday`, `inventory` or
  `hiring`. MC-1 universe `reliance` has 150 release-blocked healthy states.
- TLA+: TLA-LIVE-F2 (`tla/MC_Liveness_F2.cfg`, with weak fairness of the
  honest `settle(false)` and of `cancel`) violates LIVE-2.
  `MC_Liveness_F2Shape.cfg` shows that, in its small universe, every LIVE-2
  counterexample is this wedge.

### F-2b: a release can leave the value range

**Status.** Fixed in v1.1.0 (`59e404f`). The release is inside the
admission interval, so b x 100 fails `Arithmetic` at admission and b x 66
is admitted; after b completes, a can be cancelled. Regression test:
`f2b_release_stays_in_the_value_range`. Formal re-verification pending.

**What happens.** A release subtracts the reservation
(`values - reserve * units`), and nothing bounds that result in advance. If
a sibling's completion relied on a large reservation, the release can leave
±1,000,000, and `add` fails `Arithmetic`. `cancel` (actor and owner) and the
honest `settle(false)` fail `Arithmetic`, `observe` fails `Pending`, and
`settle(true)` succeeds. The TLA+ layer found it.

**Why it matters.** As F-2, with a different error.

**Reproducer.** `f2b_release_outside_value_range_fails_arithmetic`;
formalver trace 208.

```text
config   metric 0 in [-1,000,000, 1,000,000] initial 0
         metric 1 in [-10, 10] initial 0; term metric 1, target 0, Target
         recipe 0 (a): reserve [-1,000,000, 0], complete [0, 0], max_units 1
         recipe 1 (b): reserve [0, 0], complete [15,000, 0], max_units 100
submit(A, a x 1)                                  Ok(0)        metric 0 = -1,000,000
submit(A, b x 100)                                Ok(1)        envelope high 500,000
dispatch, acknowledge(true), settle(X, 1, true)   Ok           metric 0 = 500,000
cancel(A, 0); cancel(owner, 0)                    Arithmetic   release would give 1,500,000
dispatch, acknowledge(true), settle(X, 0, false)  Arithmetic
observe(owner, [500,000, 0])                      Pending
settle(X, 0, true)                                Ok
```

**Evidence.** Trace 208 (`release-arith`, from `release_arith()` in
`conformance/src/main.rs`), reported equal under TLC, Lean, Prolog and the
shell.

### F-3 (low): lifetime record exhaustion

**Status.** Fixed in v1.1.0 (`1f01d43`). The pool keeps a window of the
256 most recent records. A submit prunes the oldest record when it is
terminal and fails `Full` only while it is open; that record can always be
closed (F-1's fix). Prerequisites count completed units over the pool's
life. The executor forgets jobs 256 ids behind the newest it has executed.
Regression tests: `f3_window_frees_terminal_records_and_keeps_ids`,
`f3_prerequisites_survive_pruning`, `window_on_random_runs`, the contract
and executor `f3_*` tests, and the sandbox test
`window_of_300_records_through_the_executor`. Formal re-verification
pending.

**What happens.** Records are never deleted (spec:54: `observe` does not
delete history), and `submit` fails `Full` at 256 records. A submit followed
by a cancel costs only gas, so one actor can use all 256 records. After
that, every `submit` by every actor fails `Full`, also after an `observe`.
Nothing frees a record.

**Why it matters.** handoff:34: "256 lifetime records bound ordinary
growth." They do, but the bound is also a lifetime budget that one actor can
spend for everyone. api:8 states the limit without this consequence.

**Reproducer.** `f3_submit_cancel_cycles_exhaust_lifetime_records`;
formalver trace 207 (a smaller config).

```text
fixture  gpu
256 x { submit(research, {recipe 0, units 1}) Ok(id); cancel(research, id) Ok }
submit(evaluation, {recipe 2, units 1})       Full
observe(owner, values)                        Ok
submit(evaluation, {recipe 2, units 1})       Full
```

**Evidence.**
- Trace 207 (`full`, 515 calls), reported replayed by TLC (it also kills
  TLA+ mutants 13 and 25), Lean (REF-5L), Prolog (MC-2) and
  `near-tests/tests/traces.rs`.
- Sandbox: RES-1 (`sandbox/tests/resources.rs`). Submit #257 fails `Full`
  on chain.

### F-4 (low): a principal can attest its own work

**Status.** Fixed in v1.1.0 (`dc774f8`). `validate` rejects a recipe whose
actor is its executor (`InvalidConfig`), so both mutations below are
refused. The owner may still be a recipe's actor, or its executor, when the
other role is a different account. Regression test:
`f4_actor_cannot_be_its_own_executor`. Formal re-verification pending.

**What happens.** `Config::validate` does not compare a recipe's actor with
its executor or with the owner. A recipe with actor = executor, or with
actor = executor = owner, validates. That account can then submit, dispatch
and settle its own work as completed.

**Why it matters.** spec:64 (hiring): "No worker can authorize its own
acceptance through the wrapper." spec:50 calls the account that finishes
jobs "the independent operator". The wrapper's checks work as written; the
configuration lets the roles coincide.

**Reproducer.** `f4_validate_accepts_actor_or_owner_as_executor`; formalver
trace 210 (a smaller config).

```text
fixture  hiring, with recipe 0 executor := research (its actor)
                 and recipe 1 actor := executor := owner
validate()                                                         Ok
research: submit(recipe 0, 2 units), dispatch, acknowledge(true)   Ok
settle(research, 0, true)                                          Ok
owner: submit(recipe 1, 3 units), dispatch, acknowledge(true)      Ok
settle(owner, 1, true)                                             Ok
paid lifetime = 5, with no independent acceptance
```

**Evidence.**
- Trace 210 (`f4-self`), reported equal under TLC, Lean, Prolog and the
  shell.
- Prolog: `prolog/tests/test_model.pl` "F-4 witness".
  `prolog/extract.py` records that `validate` has no role checks.

### F-5 (low): version saturation freezes every transition

**Status.** Fixed in v1.1.0 (`da43188`). Submit, dispatch, `set_paused`
and `observe` fail `Arithmetic` from `VERSION_CAP` (u32::MAX - 768).
Acknowledgment, settlement and cancellation still bump the version by 1 and
always remain available: an open record needs at most three more bumps,
and at most 256 records are open. At the cap a paused pool stays paused,
and its open records still settle. Regression tests:
`f5_open_records_resolve_above_the_version_cap`,
`f5_paused_pool_at_the_cap_still_settles`, `f5_reserve_is_exactly_enough`.
Formal re-verification pending.

**What happens.** Every effective transition adds 1 to `version` with
`checked_add`. At `version = u32::MAX` every transition fails `Arithmetic`:
`settle` both ways, `acknowledge`, `dispatch`, `cancel`, `set_paused`
(including `set_paused(false)` on a paused pool) and `observe` when no
record is open. With an open record, `observe` fails `Pending` first. State
is unchanged, and in-flight records can never settle.

Reaching it takes 2³² − 1 = 4,294,967,295 effective transitions. From
reading the code, every bump other than `set_paused` and `observe` belongs
to a record: at most one submit, one dispatch, two acknowledgments and one
settle or cancel each, so at most 1,280 in all. The rest would have to come
from the owner's `set_paused` and `observe`.

**Why it matters.** handoff:34 documents "Version overflow is an explicit
rejection", which is what happens. The effect on in-flight records is not
documented. spec:54 says pausing does not block "settlement or
reconciliation"; at u32::MAX both are blocked.

**Reproducer.** `f5_version_max_freezes_settlement_and_every_transition`
and `f5_paused_pool_at_version_max_cannot_unpause`. Pool fields are public,
so the tests set the version.

```text
fixture  gpu; record 0 Dispatched (research), record 1 Reserved (evaluation)
version := u32::MAX
settle(executor, 0, true); settle(executor, 0, false)   Arithmetic
acknowledge(0, true); acknowledge(0, false)             Arithmetic
dispatch(evaluation, 1); cancel(evaluation, 1)          Arithmetic
set_paused(owner, true)                                 Arithmetic
observe(owner, values)                                  Pending

fixture  gpu; set_paused(owner, true); version := u32::MAX
set_paused(owner, false)                                Arithmetic
observe(owner, [0, 0, 0, 0, 0])                         Arithmetic
```

**Evidence.**
- No conformance trace: the highest version in the traces is 513.
- Prolog: `prolog/tests/test_model.pl` "F-5 witness" (paused, version
  u32::MAX, record 0 Dispatched).
- Lean: the proofs kill `lean/mutants/04-version-overflow-off-by-one.patch`.
- Sandbox: RT-REVERT (`rt_revert_core_errors_write_nothing` in
  `sandbox/tests/runtime.rs`) writes version u32::MAX into the on-chain
  state. `set_paused` and `submit` fail, and storage is unchanged.

### F-6 (low): an open record blocks the only snapshot refresh

**Status.** Fixed in v1.1.0 (`3eaf1cb`). The owner can restate the current
values at any time, which renews the snapshot timestamp. New values still
need every record terminal. Regression test:
`f6_owner_refreshes_the_snapshot_with_records_open`. Formal re-verification
pending.

**What happens.** With `max_age_seconds > 0`, `submit` and `dispatch` need
a snapshot no older than `max_age_seconds` (spec:19, spec:37). Only
`observe` refreshes it, and `observe` needs every record terminal (spec:54).
If one record stays open past `max_age_seconds`, nobody can submit or
dispatch, and the owner cannot refresh the snapshot, until the executor
settles that record. Reserved records can be cancelled. Dispatched records
wait for the executor; they do not expire (spec:48).

**Why it matters.** The docs state each rule on its own (spec:19, 37, 54).
They do not state the combination: one slow job stops admissions for every
actor.

**Reproducer.** `f6_open_record_blocks_the_snapshot_refresh`; formalver
trace 211 (a smaller config, `max_age_seconds` 50).

```text
fixture  risk: max_age 300, observed at 100
submit(credit, recipe 0 x 1, 100); dispatch; acknowledge(true)   record 0 Active
submit(macro, recipe 3 x 1, 100)                                 record 1 Reserved
at now = 401:
submit(equity, recipe 1 x 1)       SnapshotExpired
dispatch(macro, 1)                 SnapshotExpired
observe(owner, [60, 40, 30, 40])   Pending
cancel(macro, 1)                   Ok
observe(owner, [60, 40, 30, 40])   Pending    record 0 is still Active
settle(executor, 0, true)          Ok
observe(owner, [60, 40, 30, 40])   Ok
submit(equity, recipe 1 x 1)       Ok
```

**Evidence.**
- Trace 211 (`f6-stale`), reported equal under TLC, Lean, Prolog and the
  shell.
- Prolog: `prolog/tests/test_model.pl` "F-6 witness", on the risk fixture.

## Shell, build and runtime findings

v1.0.0's tests did not reproduce these. The contract's unit tests call the
`#[near]` methods as plain Rust functions, and near-sdk generates the
deposit checks and the `#[callback_result]` decoding only for wasm32. The
formalver tests named below run the wasm. v1.1.0 adds build, unit and
sandbox checks for each fix; the formalver tests were not re-run.

### NT-F1 (medium; diagnostics, not safety): the documented build drops every panic message

**Status.** Fixed in v1.1.0 (`effd174`). `scripts/build.sh` sets
`RUSTFLAGS="--cfg near"` and fails if a Wasm does not import
`env.panic_utf8`. The sandbox helper `fail` asserts the error text, so a
build without the flag fails the sandbox suite.

**What happens.** api:40 says to build with `scripts/build.sh`, which runs
plain `cargo build --target wasm32-unknown-unknown`. Without `--cfg near`,
near-sdk 5.29.1 compiles its off-chain fallbacks: `env::panic_str` becomes
`eprintln!` plus `panic!()`, and the wasm does not import `panic_utf8`.
Every on-chain failure is a bare `unreachable` trap. State still reverts,
but a caller cannot tell `Pending` from `Bounds` from `Unauthorized`.
`cargo near build`, or `RUSTFLAGS="--cfg near"`, keeps the messages.

**Why it matters.** `must` in `crates/contract/src/lib.rs` turns every core
error into `env::panic_str`, so the error names are the contract's error
interface, and api:28 lists them. The upstream sandbox helper `fail`
(`crates/sandbox/tests/integration.rs:36`) only asserts `is_failure()`, so
it does not notice.

**Evidence.**
- `nt_f1_upstream_build_drops_every_panic_message` in
  `near-tests/tests/shell.rs`.
- Sandbox: `sb1_upstream_build_loses_panic_messages` in
  `sandbox/tests/runtime.rs` (SB-1 in `sandbox/RESULTS.md`).

### NT-F2 (low): `reconcile` keeps an attached deposit

**Status.** Fixed in v1.1.0 (`1d441c5`). `reconcile` rejects any deposit
("Method reconcile doesn't accept deposit"). Tests:
`nt_f2_reconcile_rejects_deposit`, and a 1 yoctoNEAR `reconcile` in the
sandbox suite.

**What happens.** `reconcile` takes `&self`, so near-sdk generates no
nonpayable check for it. A call with an attached deposit succeeds. Its two
outgoing calls carry 0, so the contract keeps the deposit.

**Why it matters.** api:3: "All mutating methods are nonpayable."

**Evidence.** `nt_f2_reconcile_accepts_and_keeps_an_attached_deposit` in
`near-tests/tests/shell.rs` (both wasm builds, 1 NEAR attached).

### NT-F3 (informational): host imports the contracts never use

**Status.** Fixed in v1.1.0 (`3598902`). Both contracts create promises
with `promise_create`, `promise_then` and `promise_return` instead of
near-sdk's `Promise`. The 15 imports are gone: the contract imports 18 host
functions and the executor 13. `scripts/build.sh` fails if a Wasm imports a
host function for any action other than a function call, or any import
that is not on its list of those 18 and 13.

**What happens.** Both contracts, in both builds, import 15 host functions
that create or delete accounts, deploy code (including global contracts),
add or delete keys (including gas keys), transfer or stake. near-sdk's
generic `Promise` code links them. The source uses only `function_call`
actions, and no wasm call in the formalver suite (more than 50,000) reached
one of them. The import list alone cannot rule them out; a reviewer has to
read the code.

**Why it matters.** No doc line is broken. It weakens the evidence that the
deployed code cannot move funds or change keys (spec:56 leaves deployment
authority to a separate check).

**Evidence.** `wasm-surface.expected` lists every import and marks these 15
`forbidden`. `near-tests/run.sh surface`: the SURF-2 check fails on exactly
these imports, as expected.

### NT-F4 (low): a non-string promise result does not become Unknown

**Status.** Fixed in v1.1.0 (`1d441c5`). `on_result` reads the raw
promise result. Anything but one JSON string of at most 64 bytes becomes
Unknown, and a result for an id with no record is logged instead of
panicking. Tests: `nt_f4_non_string_result_becomes_unknown`,
`nt_f4_result_for_unknown_id_is_logged`.

**What happens.** `on_result` takes
`#[callback_result] result: Result<String, PromiseError>`. If the executor
returns a value that is not a JSON string (unquoted `completed`, empty,
`null`, `42`, an array or an object), near-sdk cannot decode it and the
callback panics. The record stays as it was, for example Dispatched,
instead of becoming Unknown. `reconcile` can retry. The supplied executor
always returns strings.

**Why it matters.** api:34: "Runtime failures and unexpected strings become
Unknown in the wrapper." Strings do. Other JSON values do not.

**Evidence.** `sh3_non_string_promise_results_fail_the_callback` in
`near-tests/tests/shell.rs` (six payloads, both wasm builds).

## Docs vs code

| ID | Docs | Code | Suggested resolution |
|---|---|---|---|
| `div-ack-noop-version` | spec:39 lists "Dispatched/Unknown -> Unknown". api:26: "Every successful state transition changes version". | `acknowledge` returns `Ok` without a version bump when the status would not change. The code comment says why: `reconcile` is permissionless, so a bump on a no-op would let anyone invalidate every in-flight request. The existing test `repeated_unknown_reconciliation_does_not_change_version` pins it. | The code is right. Change the docs. |
| `div-cancel-terminal-name` | spec:41-43 and api:15: cancel applies only to undispatched Reserved records. | Cancelling a Completed or dispatched record fails `Pending`, a name that suggests an open commitment. `div_cancel_of_completed_or_dispatched_record_says_pending` pins it. | Return `InvalidState`, as `dispatch` and `settle` do for a record in the wrong status, or document `Pending`. |

**Status.** Both resolved in v1.1.0 (`138ef98`). `div-ack-noop-version`:
spec and api now say that a call that changes nothing, such as an
acknowledgment that does not change the status, returns `Ok` without a
version change. `div-cancel-terminal-name`: `cancel` of a Completed record
fails `InvalidState`; Dispatched, Active and Unknown records still fail
`Pending`. Test: `cancel_names_conflicting_and_in_flight_states`.

## Measured minimum prepaid gas

docs/api.md gives no prepaid gas per method. The formalver sandbox measured
the least prepaid gas that succeeds at a maximum-size state: 32 metrics, 64
terms, 32 recipes and 256 records (225 of them open), 45,613 bytes of Borsh
state. It counted up one whole Tgas at a time from 40 Tgas.

| Call | Fails at | Least that succeeds | Gas the call attaches to its promises |
|---|---|---|---|
| `dispatch` | 40 to 69 Tgas | 70 Tgas | `execute` 30 + `on_result` 30 |
| `reconcile` | 40 to 45 Tgas | 46 Tgas | `status` 10 + `on_result` 30 |
| executor `finish` | 40 to 42 Tgas | 43 Tgas | notify 40 |

- Both builds give the same minimums.
- A failing attempt changes nothing: no receipt runs on the other contract,
  and the storage of both contracts is byte-identical afterwards.
- At 70 Tgas, `dispatch` has under 1 Tgas spare. A documented figure needs
  headroom.
- The `dispatch` and `reconcile` sweeps ran with the executor at 31 and 34
  jobs. Only `finish` was swept with the executor at its 256-job cap.
- The worst single pool receipt at that state is `submit` #256, at 12.56
  Tgas.

Evidence: `res_1_gas_at_max_state` and `res_1_min_prepaid_gas_and_executor_cap`
in `sandbox/tests/resources.rs`; RES-1 in `sandbox/RESULTS.md`; OQ-9 in
`strategy.md`.

**Status.** v1.1.0 adds a gas table to [docs/api.md](api.md#gas)
(`e47f1e5`). It was measured on the v1.1 Wasm with a scratch test adapted
from the RES-1 tests, at the same configuration with 256 retained records,
all open. The minimums for `dispatch` (70 Tgas), `reconcile` (46) and
executor `finish` (43) are the same as above; `confirm_not_executed` also
needs 43. The largest single receipt is a `submit` with 255 open records,
at 14.81 Tgas.

## Observation (not a finding)

At `version = u32::MAX`, `preview` returns `allowed: true` for a request
that `submit` then rejects with `Arithmetic`. `submit` checks the version
bump after the checker that `preview` shares. The contract's `submit` runs
`preview` first and then `must(submit)`, so on chain the call panics
instead of returning a rejected decision.
`f5_version_max_freezes_settlement_and_every_transition` pinned it.

**Status.** Closed in v1.1.0 (`da43188`). The version check is now part of
the admission that `preview` shares, so `preview` reports `Arithmetic` from
`VERSION_CAP` on. `f5_open_records_resolve_above_the_version_cap` checks
it.
