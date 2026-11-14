# shared-pool sandbox layer: results (v1.1.0)

Runtime evidence on the real wasm. `pool_contract.wasm` and `pool_executor.wasm`
are built from the pinned source mirror (57b3f02, clean; tree identical to the
v1.1.0 PR head 6fcbd56) by the documented build, upstream `scripts/build.sh`.
They run in a local near-sandbox 2.13.4 through near-workspaces 0.23.0, the
versions upstream `crates/sandbox` pins. Evidence class: **test** (near-approval
strategy §12). Every test also keeps a host-side `pool_core::Pool` in step with
the chain. That is the real core, used as an oracle. After each step the test
compares the whole `get_pool` state with it, so the tests cover the shell and
core together, not the shell alone.

The v1.0.0 record (two builds, findings SB-1 and F-1 reproduced) is in git at
a072a2d and in PR #31; this file replaces it.

Runner: `projects/shared-pool/sandbox/run.sh`. Exit 0 means every check passed.
It runs these steps in order:

1. `scripts/check-lock.py`. The sandbox `Cargo.lock` was seeded from the mirror's
   lockfile, and all 342 shared registry packages match it.
2. `scripts/build-wasm.sh`: runs upstream `scripts/build.sh` in the mirror
   (with `CARGO_TARGET_DIR`, `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` cleared,
   so nothing outside build.sh changes the build). build.sh sets
   `RUSTFLAGS="--cfg near"`, checks both import sections against its reviewed
   list, and writes `artifacts/build.json`. The script then requires
   `dirty: false`, `source_commit` = the mirror's HEAD and `rustflags` =
   `--cfg near`, copies both wasm files to `target/pool/wasm32-unknown-unknown/release/`,
   and requires the copies to hash to build.json's sha256. One variant,
   `upstream`: v1.0.0's second variant `cfg-near` is what build.sh itself now does.
3. `scripts/build-res.sh`, which builds the helper contract `relay/`.
4. `scripts/sandbox-bin.sh`, which supplies a sha256-checked near-sandbox binary.
5. `tests/runtime.rs` (13 tests), then `tests/resources.rs` (RES-1, 2 tests).
   `SANDBOX_SKIP_RES=1` skips RES-1.

Each test writes its observations to `results/upstream/<ID>.json`.

## Artifacts under test (`results/wasm.json`)

rustc 1.98.1 (48a229cea 2026-09-01); `cargo build -p pool-contract -p
pool-executor --target wasm32-unknown-unknown --release --locked` with
`RUSTFLAGS="--cfg near"`; the mirror's release profile (opt-level z, LTO,
panic=abort, overflow-checks=true, codegen-units=1). Cargo.lock sha256
`09a1439436039ddeaf4129577fd4de5c21c84c42b0651ddb493079922754e83e`.

| wasm | sha256 | NEAR code hash | bytes | imports |
|---|---|---|---|---|
| pool_contract | `e6383aabed62dcf694ac2f97da4520a019b18c9db637b1ee2afc7180c697e2ea` | `GVgVzY6yh26Ut22AavEu7pSpPm7B11oEsLoMUXckRr8Z` | 377358 | 18 |
| pool_executor | `b92f146919be69f36f517edc10f1e04e6ee96430ba0d745e4422132615634e83` | `DTt4aM2wPfNuNie9kqTSHmnL4K8CVxnLGhQfuWvB3Ypr` | 223068 | 16 |

These equal build.json's hashes, the hashes near-tests' SURF-3(a) records, and
the hashes upstream's RES-1 figures (docs/api.md) were measured on.

## Properties

| ID | test (`PROP:` tag) | outcome | what it shows |
|---|---|---|---|
| RT-PRED | `rt_pred_relay_is_the_caller` | pass | [RT-PRED](#rt-pred) |
| RT-PRIVATE, EX-8 (sandbox half) | `rt_private_new_and_on_result` | pass | [RT-PRIVATE](#rt-private) |
| RT-CALLBACK | `rt_callback_status_mapping`, `rt_callback_non_string_executor_results` | pass (NT-F4 fixed) | [RT-CALLBACK](#rt-callback) |
| RT-REVERT | `rt_revert_core_errors_write_nothing` | pass (NT-F2, F-5 fixed) | [RT-REVERT](#rt-revert) |
| RT-RACE | `rt_race_settle_before_callback`, `rt_race_executor_key_settles_while_callback_in_flight`, `rt_race_operator_finish_vs_callback` | pass (the operator-`finish` form cannot occur on one shard) | [RT-RACE](#rt-race) |
| RT-WEDGE | `rt_wedge_f1_refused_and_cure_settles`, `rt_wedge_f2_honest_release_after_the_hedge` | pass (F-1, F-2 fixed) | [RT-WEDGE](#rt-wedge) |
| RT-WINDOW, EX-7 (sandbox half) | `rt_window_300_records_through_the_executor` | pass (F-3 fixed) | [RT-WINDOW](#rt-window) |
| RT-D1, EX-7 and EX-8 (sandbox halves) | `rt_d1_confirmed_failed_executes_keep_256_jobs` | pass | [RT-D1](#rt-d1) |
| RES-1 | `res_1_gas_at_max_state`, `res_1_min_prepaid_gas_and_executor_cap` | pass | [RES-1](#res-1) |
| (SB-1, inverted) | `sb1_documented_build_reports_panic_messages` | pass (SB-1 = NT-F1 fixed) | [SB-1](#sb-1-fixed) |

Every failure any test expects must read `Smart contract panicked: <msg>`
(`expect_panic` in `tests/common/mod.rs`); a bare `unreachable` trap fails the
test. Core errors are matched exactly (`NotFound`, `Pending`, ...); Rust
`assert!`/`expect` messages, which near-sdk prefixes with `panicked at
<file>:<line>:`, are matched by containment.

### RT-PRED

Unchanged from v1.0.0 and passing on the v1.1.0 wasm:

- An actor signs `relay.forward(submit)`. The pool attributes the call to the
  relay: the decision event's `actor` is the relay, the result is
  `Unauthorized`, and storage is byte-identical.
- The same request sent directly by the actor is accepted.
- When the relay is a recipe's actor, a forward signed by an unrelated account
  is accepted as the relay's submit.
- `dispatch`, `cancel`, `settle` (even when signed by the executor's key),
  `set_paused` and `observe` (signed by the owner) are all rejected
  `Unauthorized` when they go through the relay. None of them writes anything.

### RT-PRIVATE

- `get_pool` before `new` fails "The contract is not initialized".
- `pool.new` fails "Method new is private" for the owner, the actor, an
  unrelated account and the executor, and through the relay when the
  transaction is signed with the pool's own key. `exec.new` behaves the same
  way. A second `new` by the contract's own account fails "The contract has
  already been initialized". Storage is unchanged in every case.
- `on_result` fails "Method on_result is private" for four foreign callers and
  for the relay.
- **Changed in v1.1.0.** On the pool's own key, `on_result` is now **Ok**: the
  wrapper reads the raw promise results, a transaction carries none, so the
  count is not 1 and the result is "unknown". Observed: on an Unknown record
  the event is `{id, outcome: "unknown", error: null}` and storage is
  unchanged; on an **Active** record the same event, and the record stays
  Active with storage unchanged; for an id never issued (999) the event carries
  `error: "NotFound"`. Only the deployment account's own key can send this, so
  it falls under ASM-PRIVATE.
- **EX-8, sandbox half.** The executor's `on_settled({id: 600})` fails "Method
  on_settled is private" from the owner, the actor, an unrelated account, the
  lock account and the relay. From the executor's own key it passes the
  private check and fails `not a settle callback` (the promise-result count is
  0, not 1). The executor's raw storage is unchanged in every case. RT-D1
  covers the "advances only on an accepted settle" half.

### RT-CALLBACK

Each row is executor mode → result of `execute` → effect on the record.

- **normal** → "active" → Active. The reservation is kept; the executor holds
  the job as "active".
- **reject** → "cancelled" → Cancelled through `settle(executor, false)`. The
  reservation is released exactly.
- **unknown** → the executor stores the job and returns "unknown" → Unknown.
  `cancel` then fails Pending. `reconcile`, signed by an unrelated account,
  reads "active" → Active; repeating it is a no-op.
- **panic** → `execute` fails "injected failure" → the callback has no
  successful result → Unknown. The dispatch transaction itself succeeds and the
  reservation is kept. `reconcile` reads "unknown", a no-op.
  `confirm_not_executed` releases it: receipts executor `[ok, ok]` (the call,
  then `on_settled`), lock `[ok]` (`settle`).
- `finish` has three receipts in v1.1.0, in this order: `finish`, the lock's
  `settle`, then the executor's `on_settled`. The test checks the executor's
  jobs and `newest` after each step: final jobs `[(0, completed), (1,
  cancelled), (2, cancelled), (3, cancelled)]`, `newest` 3.
- **NT-F4, fixed** (`rt_callback_non_string_executor_results`, record
  `RT-CALLBACK-nonstring`). Upstream's hand-assembled executor wasm
  (`integration.rs` `non_string_executor`, ported with its MIT attribution;
  sha256 `7608bb67…6ec2`) returns the JSON number `5` from `execute` and a
  70-byte JSON string from `status`. Both map to "unknown": the dispatch
  succeeds and the record goes Unknown; the reconcile is a no-op with storage
  unchanged. The executor account's own `settle(0, false)` then releases it.

### RT-REVERT

Ten core errors through the wasm all panic with the error name. For each one
the pool's raw storage is byte-identical before and after, and the transaction
emits no event:

- Unauthorized, NotFound and InvalidState on `dispatch` and `settle`
- Unauthorized and Pending on `cancel`
- Unauthorized on `set_paused`
- Pending on `observe`

The same holds for: Paused on `dispatch`; Bounds on `observe` with three bad
inputs. Rejected submits (Paused, Stale, InvalidUnits, Unauthorized) do not
panic: they return a decision, emit one `decision` event and write nothing.

New in v1.1.0:

- `cancel` of a Completed record fails InvalidState (conflicting terminal
  evidence), storage unchanged.
- **NT-F2, fixed.** `reconcile` with 1 yoctoNEAR attached fails "Method
  reconcile doesn't accept deposit"; no receipt reaches the executor and
  storage is unchanged.
- **F-5, fixed.** The pool's `version` is patched to `VERSION_CAP` =
  u32::MAX − 768 = 4294966527 (reaching it takes about 2^32 transitions), with
  one Reserved and one Active record. `set_paused`, `observe` (restating the
  current values) and `dispatch` each fail Arithmetic, with no event, no
  executor receipt and storage byte-identical; the core gives the same error.
  `preview` returns `allowed: false, reason: Arithmetic`, equal to the core
  (v1.0.0's preview said allowed and submit panicked), and `submit` returns that
  rejected decision without a panic. The open records still resolve: `cancel`
  of the Reserved one and the operator's `finish(true)` of the Active one are
  both Ok (version CAP + 2). With every record terminal, `observe` still fails
  Arithmetic.
- A pruned id's `dispatch` gives NotFound with storage unchanged (RT-WINDOW).

### RT-RACE

1. **Settle before the callback (deterministic).** The executor is the helper
   contract `relay/`, which runs `settle` from inside `execute`. On one shard
   `settle` lands first, every time, and the late "active" ack is a no-op: the
   version moves by +2 (dispatch, settle), where the callback-first order would
   give +3. This holds for `settle(true)` and `settle(false)`, and for a stale
   `reconcile` read (+1). Reconciling a terminal record fails InvalidState.
2. **Timed race with the upstream executor contract.** The executor account's
   own key signs `settle` 0–400 ms after `dispatch` is broadcast. Every serial
   order the core allows was observed (settle before dispatch: InvalidState;
   dispatch, settle, callback: the late ack ignored; dispatch, callback,
   settle). The test fails if the late-ack order never occurs.
3. **Timed race with the operator's `finish`.** Two outcomes only: "finish
   before execute" (`finish` fails "unknown job", one receipt; a later `finish`
   succeeds), or "callback first, then finish → settle → on_settled". For every
   accepted `finish` the test checks the three receipts and that `on_settled`
   raised the executor's `newest` to the id. `finish` returns the lock's
   `settle` value and `on_settled` runs as a separate callback, so the async
   outcome is re-polled until `on_settled` has run (`complete` in common/;
   near-workspaces' own await can return before it). On one shard the notify
   cannot overtake the callback: a gap, not a failure.

### RT-WEDGE

Both v1.0.0 wedges are fixed on the wasm (LIVE-2 holds on these paths).

- **F-1** (`rt_wedge_f1_refused_and_cure_settles`). The brief's configuration:
  `observe [12, 5]` (metric 0 above its max 10), then the hedge
  `r(actor, exec, [-2,0], [2,-5], 1, [])`. v1.1.0 refuses the submit `Bounds`
  (preview and core agree; storage unchanged), so nothing is dispatched
  (`dispatch(0)` fails NotFound, the executor has no job) and `observe [5, 5]`
  is Ok. The cure-by-completion hedge of upstream `findings.rs:175`
  (`r(actor, exec, [0,-1], [-3,0], 1, [])`) is admitted at `[12, 5]`
  (values `[12, 4]`), dispatched (Active), and settles through the operator's
  `finish(true)` (receipts `[ok, ok]` / `[ok]`): Completed, values `[9, 4]`.
  `finish(false)` afterwards fails "terminal outcome conflict" and changes
  nothing; `observe [5, 5]` is Ok.
- **F-2** (`rt_wedge_f2_honest_release_after_the_hedge`, record
  `RT-WEDGE-F2`). Upstream's `risk` fixture (`fixtures.rs`) with credit, equity
  and macro accounts, in two forms: the position Reserved, and the position
  dispatched with the executor panicking (Unknown). The hedge is sized as if
  the position were released: 8 and 7 units are refused Bounds, 6 (the core's
  `best_quantity`) is admitted. The position's honest release (`cancel` when
  Reserved, `confirm_not_executed` when Unknown) is Ok in both forms, nothing
  wedges, and `observe [60, 40, 30, 40]` is Ok after.

### RT-WINDOW

F-3, fixed, end to end (port of upstream `integration.rs`
`window_of_300_records_through_the_executor`, with formalver's core comparison
after every step).

- 300 records go through the executor: ids 0–9 and 290–299 in normal mode
  (dispatched, Active, then the operator's `finish(true)`: Completed), the rest
  in reject mode (`execute` returns "cancelled" and `on_result` releases the
  record: Cancelled).
- Final pool: `first_id` 44, 256 records (Cancelled 246, Completed 10),
  `completed_units [20]`, version 920. The executor holds exactly ids 44..=299
  (256 jobs); the most jobs ever held was 256.
- EX-7 is checked after every transaction (620 checks): every id the executor
  has seen and the lock still retains is still a job; `get_jobs` equals the
  executor's borsh state; at most 256 jobs; every job satisfies
  `id + 256 > newest`.
- Checkpoints at records 0, 50, 100, 150, 200, 250, 255, 256, 257, 299 record
  `first_id`, the record count and the exact job-id range.
- Pruned id 0: `dispatch`, `cancel`, the executor key's `settle` and
  `reconcile` each fail NotFound with storage unchanged; the executor's
  `finish(0)` fails "unknown job", `confirm_not_executed(0)` fails "stale id",
  and `status(0)` returns "unknown".
- `get_commitments` pages across `first_id`: `(0, 50)` and `(44, 50)` give
  44..=93, `(290, 50)` gives 290..=299, `(300, 50)` is empty, and `(100, 100)`
  gives 100..=149 (the limit is capped at 50).

### RT-D1

D-1 (owner decision Q4) on the real runtime: port of upstream
`integration.rs` `confirmed_failed_executes_keep_the_executor_at_256_jobs`.

- **Deviation from upstream:** the executor's operator is a separate account,
  not the owner, so the roles stay distinct; the owner's `set_mode` fails "only
  independent operator".
- The executor panics on every `execute`. For each of 300 records: `dispatch`
  (receipts executor `[fail]`, lock `[ok, ok]`; the record goes Unknown), then
  the operator's `confirm_not_executed` (executor `[ok, ok]`, lock `[ok]`; the
  record is Cancelled and `on_settled` raises `newest` to the id).
- At upstream's ten checkpoints (confirmations 0, 50, 100, 150, 200, 250, 255,
  256, 257, 299) the exact job-id list is asserted. The executor never holds
  more than 256 jobs; EX-7 is checked after every transaction (606 checks).
- Then: id 43 (pruned): `confirm_not_executed` fails "stale id" and `finish`
  "unknown job", storage unchanged. A refused confirmation of id 10000:
  executor `[ok, ok]`, lock `[fail]` (NotFound), the job is kept and `newest`
  stays 299; the lock's storage is unchanged (`on_settled` takes the refused
  branch: EX-8). `on_settled` from foreign accounts fails "Method on_settled is
  private". Id 300 confirmed while Reserved is refused InvalidState; its later
  `dispatch` gives "cancelled" → Cancelled; the operator's `finish(300, false)`
  is accepted and forgets job 44. Id 301 goes end to end (Completed).
- Final pool: `first_id` 46, 256 records (Cancelled 255, Completed 1),
  `completed_units [1]`.

### SB-1 (fixed)

`sb1_documented_build_reports_panic_messages` (inverted from v1.0.0's
`sb1_upstream_build_loses_panic_messages`). On build.sh's wasm no failure is a
bare trap; each carries its message: `Method new is private`, `Unauthorized`,
`NotFound`, `Bounds`, `Method on_result is private`, `only independent
operator`, `Method on_settled is private`, `Pending` (cancel and observe), and
`Method reconcile doesn't accept deposit` (NT-F2). v1.0.0's record of the
finding (every failure "WebAssembly trap: An `unreachable` opcode was
executed.") is `results/upstream/SB-1.json` at a072a2d.

### RES-1

Method: upstream `crates/sandbox/tests/res1.rs` at the pin (which upstream
adapted from this layer's v1.0.0 test), ported back with formalver's two-test
split. Acceptance thresholds: upstream `docs/api.md:44-59`, the gas table.
Upstream's own figures (the table's, and upstream's `artifacts/res1.json`
from that run, kept as `.context/shared-pool/v1.1/res1-q4.json`) are compared
as an informational cross-check: gas is deterministic for an identical wasm
and protocol version, and upstream measured the hashes above. **Every
documented figure, and every figure in upstream's res1.json, was reproduced
exactly.**

The state is built with real transactions only (no state patching):

- 32 metrics with 128-byte names and units, 64 terms, 32 recipes with
  128-byte names, 64-byte actor and executor ids, full 32-entry effect
  vectors, recipe *i* requiring every *j < i*;
- (A) 31 Completed prerequisites (recipes 0–30 × 1), then recipe-31
  reservations of 100 units up to 255 records;
- (B) the 256th submit (no prune), then 31 more submits that each prune a
  Completed front record, so the pool retains 256 open records and the next
  submit is rejected `Full` (nothing written; core and chain agree);
- (C) `set_paused` both ways, `observe` restating the values, `cancel` of the
  oldest record and one more submit: ids 32..=287 retained, all open.

At 256 open records: `STATE` is 45,749 bytes of Borsh, the account's
`storage_usage` 423,334 bytes, and `get_pool` returns 53,401 bytes of JSON
(upstream's res1.json: the same three numbers).

**Test 1, `res_1_gas_at_max_state`** (`results/upstream/RES-1.json`), every
call at 300 Tgas:

| call | receipts (Tgas) | tx total |
|---|---|---|
| submit #256 (224 open records, no prune) | pool 14.02 | 14.33 |
| submit #257 (prunes id 0) | pool 14.05 | 14.36 |
| submit #287 (255 open records, prunes the last Completed record) | pool **14.81** | 15.13 |
| submit #288 (rejected Full) | pool 8.10 | 8.41 |
| preview at 255 records (as a transaction) | pool 6.70 | 7.01 |
| get_pool (as a transaction) | pool 9.85 | 10.15 |
| get_commitments(0, 1000) → ids 31..80 | pool 3.85 | 4.16 |
| set_paused(true) / (false) | pool 7.59 / 7.59 | 7.89 / 7.90 |
| observe (restating the values) | pool 7.71 | 8.02 |
| cancel (release; 255 other open records) | pool 10.87 | 11.18 |
| dispatch | pool 8.29, on_result 7.63, execute 1.43 | 18.10 |
| reconcile (Active, a no-op) | pool 4.15, on_result 7.63, status 1.23 | 13.76 |
| dispatch, execute returns "cancelled", on_result releases | pool 8.29, on_result **10.92**, execute 1.35 | 21.31 |
| dispatch, execute fails, on_result marks Unknown | pool 8.29, on_result 7.62, execute 1.29 (fails) | 17.95 |
| executor finish(true) | finish 2.12, on_settled 1.37, settle **10.87** | 15.11 |
| executor confirm_not_executed | 2.12, on_settled 1.37, settle 10.86 | 15.11 |
| settle(true) / settle(false) by the executor key | pool 10.84 / 10.83 | 11.15 / 11.14 |

- The worst pool receipt is submit #287 at 14.81 Tgas (docs: "largest receipt
  14.81 Tgas (a `submit` with 255 open records)"), 15.19 under the documented
  30 Tgas attach for `submit`, `cancel`, `settle`, `observe` and `set_paused`.
  Every pool receipt in both tests is under 30 Tgas (asserted). The largest
  receipt of those five calls is at least 10 Tgas under their attach
  (asserted in both tests, docs/api.md:50's rule for that row: 15.19 here,
  19.16 in test 2, whose largest is a settle by the executor key at 10.84).
- Submit gas grows with the open records the envelope sums: 7.33 Tgas at 1
  record, 10.30 at 128, 14.02 at 256 (no prune), 14.81 with 256 open after a
  prune (`submit_gas_by_submit` in the record).

**Test 2, `res_1_min_prepaid_gas_and_executor_cap`** (`results/upstream/RES-1-gas.json`):
the least whole-Tgas prepaid gas, counting up from 40 Tgas.

| call | state | fails at | least that succeeds | docs least | docs attach | margin to attach |
|---|---|---|---|---|---|---|
| `dispatch` | 256 records, all open; executor 31 jobs | 40–69 | **70** | 70 | 100 | 30 |
| `reconcile` | 256 records, the record Active; executor 3 jobs | 40–45 | **46** | 46 | 60 | 14 |
| `confirm_not_executed` | executor 254 jobs (255 after); the record Unknown | 40–54 | **55** | 55 | 70 | 15 |
| `finish` | executor 256 jobs, the last job | 40–54 | **55** | 55 | 70 | 15 |
| `finish` | executor 512 jobs (its cap) | 40–55 | **56** | 56 | 70 | 14 |

- Each of the 84 failing attempts fails in its first receipt with the
  runtime's "Exceeded the prepaid gas.". For each one the test checks: no
  receipt runs on the other contract; no log and no event; the raw storage of
  both the pool and the executor is byte-identical; `get_pool` still equals
  the core, and the record keeps its status.
- After failed attempts at 40 and 69 Tgas on a second record, a 300 Tgas
  `dispatch` succeeds; a third record succeeds again at exactly 70 Tgas. The
  `dispatch` receipt burns 8.29 Tgas with 70 or with 300 Tgas prepaid: the
  gas attached to promises is fixed.
- Each least amount is at least 10 Tgas below its documented attach
  (asserted); each equals the documented least (recorded, `matches_docs`).
- **Promise receipts** (docs/api.md's second bullet), all under their
  attached gas (asserted): `on_result` at most 10.92 of 30 (a release),
  `execute` 2.46 of 30 (the 256th job), `status` 1.76 of 10 (256 jobs), the
  lock's `settle` after `finish` or `confirm_not_executed` at most 10.87 of
  40, `on_settled` 2.47 of 10.
- **The executor at its cap (512 jobs).** Reached as upstream does: the
  operator confirms the 256 unassigned ids 289..=544; each adds a "cancelled"
  job, the lock's `settle` fails NotFound (afterwards the pool still equals
  the core) and `on_settled` takes the refused branch (`newest` stays 287). The confirmation
  from 511 to 512 jobs burns 4.39 Tgas; one more fails "full" on the executor
  and never reaches the lock. At 512 jobs `status` burns 2.29 of 10,
  `finish` 4.39 with `on_settled` 3.68 of 10 (3.66 after a refused settle), and
  `execute` of a new id (288, after the front record 32 is finished and pruned)
  3.65 of 30: it forgets job 32 and adds 288. All equal docs/api.md.
- **`on_settled` under 6 Tgas** (upstream's `ON_SETTLED_LIMIT`, asserted for
  every measured one, including all 256 refused ones): largest 3.68 Tgas,
  6.32 Tgas of margin to the 10 attached.
- The executor's `STATE` is 5,745 bytes of Borsh at 256 jobs and 12,145 at
  512 (upstream's res1.json: the same).

## What is NOT shown

- **Multiple shards and congestion.** The sandbox is a single node with a
  single shard. On mainnet, receipt order across shards, and under congestion
  or delayed receipts, can differ. The late-ack order was produced
  deterministically with the helper executor and by timing with the executor
  key, not with the operator's `finish`, which cannot overtake its callback on
  one shard.
- **Other protocol versions.** Gas is near-sandbox 2.13.4's protocol
  configuration. Other protocol versions price differently.
- **Proof of the worst case.** RES-1's maximum state is a constructed worst
  case, not a proven one. The least prepaid gas is measured at that state only,
  in whole Tgas, for `dispatch`, `reconcile` (256 open records),
  `confirm_not_executed` (254 jobs) and `finish` (256 and 512 jobs). The
  methods that create no promise (`submit`, `cancel`, `settle`, `observe`,
  `set_paused`) were run at 300 Tgas only; their largest receipt is compared
  with the documented 30 Tgas attach.
- **Out of scope here (near-tests' mocked runtime covers them):** expiry by
  block time (`Expired`, `SnapshotExpired`) and deposits on methods other than
  `reconcile`.
- **Storage staking.** The 50 NEAR pool and executor accounts held their
  largest states; storage staking limits were not probed.
- **Live deployment.** Nothing here touches testnet or mainnet, deployment
  locks, or code replacement (ASM-DEPLOY).

## Run times (this machine, macOS arm64)

On 2026-11-14, `run.sh` with warm caches took 1041 s in total and exited 0.
build.sh's `cargo build` was a no-op on the mirror's cached target; its import
check and build-wasm.sh's hash checks still ran. The steps took:

- `check-lock`, `build-wasm`, `build-res`, `sandbox-bin`: a few seconds.
- `tests/runtime.rs`: 13 tests in 509.5 s with 3 test threads, each test in
  its own sandbox. RT-D1 (about 500 s) and RT-WINDOW (about 320 s) set the
  pace: each sends over 600 transactions and reads both contracts' state after
  every one (EX-7).
- `tests/resources.rs` (RES-1): 2 tests in parallel in 523.4 s, each building
  the 256-record state with about 360 real transactions (A-C took 156 s in
  `res_1_gas_at_max_state`, `state.build_seconds`; the whole test 163 s);
  `res_1_min_prepaid_gas_and_executor_cap` sets the pace (about 970
  transactions in all, 84 of them low-gas attempts). Upstream's single test took 469
  and 524 s (fix-1.md).

`SANDBOX_SKIP_RES=1` leaves about 8.5 minutes. The relay helper is
`sandbox_relay.wasm` sha256 `15756aac597357de320d8973ba88f050c46f4aa4250c92d83c138c8e07b6150c`.
