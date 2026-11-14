# shared-pool near-tests: results

Shell layer of the shared-pool verification (near-approval strategy.md
section 9 pattern). Target: difflabai/shared-pool-controls v1.1.0, the
`../source-mirror` gitlink at `57b3f02d18e85149db3c1035431368c4e07a4d8a`
(the merge of upstream PR #4; its tree equals the PR head `6fcbd56`).
Sources (sha256): contract `crates/contract/src/lib.rs` `3810d18d…c03c`,
executor `crates/executor/src/lib.rs` `a2d63842…9198`, core
`crates/core/src/lib.rs` `c3bb825b…ba48`, `scripts/build.sh` `429b63ea…1f3e`.
Toolchain 1.98.1 (the mirror's `rust-toolchain.toml`), near-sdk 5.29.1
(`unit-testing`), mocked runtime = near-vm-runner 0.37.4 `VMLogic`, wasmi
2.0.0.

The v1.0.0 record of this layer (four findings NT-F1..NT-F4, the F-1
wedge reproduced through the shell, 21 mutants) is in git history at
`a072a2d`. v1.1.0 fixes all of them; their witnesses are inverted here
into regression tests, and the historical mutants s00a/s00b (below) show
that those tests fail on the v1.0.0 contract and executor.

Run: `./run.sh test | surface | mutants` (or `./run.sh` for all three).
`test` and the default `all` first run `check-tags.py`, the tag check
(below); any problem it reports fails the run.

## How the shell is exercised

- **Host path**: the `#[near]` methods called as Rust functions. Every call
  starts from storage (`state_read`, Borsh) and ends with `state_write` on
  success; a panic discards every write and receipt.
- **Wasm path**: the real wasm of the documented build, run by the wasmi
  interpreter with every `env` import forwarded to the same mocked
  `VMLogic`. The documented build is upstream `scripts/build.sh`:
  `RUSTFLAGS="--cfg near"`, `cargo build --target wasm32-unknown-unknown
  --release --locked` from the mirror root (`Build::Near`). `./run.sh
  surface` (SURF-3(a)) runs build.sh itself and checks that its
  `artifacts/build.json` hashes are these files. The no-cfg build
  (`Build::Plain`) is only the NT-F1 negative control. This covers the code
  near-sdk generates only for wasm32: `#[init]`, `#[private]`, the
  nonpayable deposit checks and JSON argument/return encoding. Any import
  outside the shell's set traps as a forbidden host call; every wasm call
  asserts none happened.
- **Receipt router** (`Net`): pool and executor wasm side by side; outgoing
  receipts run FIFO, a callback gets its dependency's result, and the
  transaction's result follows `promise_return`. Used for
  `dispatch -> execute -> on_result`, `reconcile -> status -> on_result`,
  `finish/confirm_not_executed -> settle -> on_settled`.
- Expected values come from `pool_core::Pool` (same pinned crate), from a
  reference model of the executor written from docs/api.md ("Executor
  protocol"), and from docs/api.md, never from the contract's output.
- `require!(c, m)` is `assert!` on the host test build (debug assertions)
  and `env::panic_str` in the release wasm: reconcile's deposit check is a
  Rust panic on the host path and a guest panic on the wasm path, with the
  same message (`common::on_host`).
- `Cargo.lock`: `tests/lockfile.rs` checks the packages of the code under
  test resolve exactly as in the mirror's lockfile; the only extra packages
  are wasmi and its dependencies (harness only).

## Test suite (`./run.sh test`, 38 tests)

| File | Properties | What |
|---|---|---|
| `tests/shell.rs` (15) | SH-1..SH-7, SH-9, SH-10, RD-PREVIEW (shell half), LIVE-ALL at the shell | exact scenarios on the host path and the documented build. The v1.0.0 finding witnesses are inverted: `nt_f1_documented_build_reports_every_panic_message` (SH-5), `nt_f2_reconcile_rejects_an_attached_deposit` (SH-10), `sh3_non_string_promise_results_become_unknown` (SH-3; upstream's 16 raw-result cases). F-3 through the shell: `sh9_views_and_paging_by_id_through_the_window` (300 submits, first_id 44, pages by id), `sh9_pruned_ids_are_not_found_and_pages_are_by_id` (every id-taking method on a pruned id) |
| `tests/store_diff.rs` (3) | SH-8 (+SH-1, SH-3..SH-7, SH-9, RD-PREVIEW, LIVE-ALL) | 2,000 x 40 random calls on the host path and 300 x 40 on the documented build, Borsh reload every call; results, panic messages, events, receipts, stored bytes and views; required coverage. `sh8_store_diff_window`: 4 host + 1 wasm sequences of 1,500 submit-heavy calls past the 256-record window (prunes, pruned ids, ids `first_id + k`) |
| `tests/traces.rs` (1) | SH-8 (+SH-3, SH-4, SH-5), LIVE-2, LIVE-ALL | the conformance traces (`../conformance/traces/core.txt`: 216 traces, 8,865 calls) replayed through the shell on both paths, host and the documented build (Near). States `s/7` (first_id and completed_units compared); trace 210 is `init_error(invalid_config)`: `Pool::new` refuses its config and the shell's `new` fails, writing nothing; an `ack` NotFound is an Ok callback whose event carries the error |
| `tests/executor.rs` (14) | EX-1..EX-8 (+SH-3, SH-6, SH-7 end to end; RT-RACE, pool side) | executor alone on both paths (stored bytes checked against a Borsh mirror; window tests against the reference model), its wrappers (`on_settled` private), notify's two receipts (`settle` 40 Tgas, then `on_settled` 10 Tgas, `promise_return` of the settle), the window (`WINDOW == MAX_RECORDS`, `MAX_JOBS >= 2*WINDOW` read from the pinned source), a random differential against the model (24 x 400, a third from near the cap), end-to-end flows, and `ex7_d1_*`, upstream's sandbox D-1 test in the mocked runtime (300 failed executes confirmed; EX-7 after every transaction; final first_id 44, then 46) |
| `tests/wedge.rs` (3) | LIVE-2 (F-1), LIVE-1 (control), RT-WEDGE (mocked runtime), +SH-3, SH-5, EX-4, EX-5 | the F-1 hedge refused with Bounds through the shell, the curing hedge admitted and resolvable from every state; control; end to end with the executor, F-1 refused and the F-2 `risk` run whose credit release stays Ok |
| `tests/scenarios.rs` (1) | SCN-1 | the six published `scenarios/<name>/{config,trace}.json` equal a re-derivation from `pool_core` (preview, submit, dispatch, acknowledge, settle at the fixed epoch 1,794,441,600) |
| `tests/lockfile.rs` (1) | precondition of the near-tests IDs the tests check (scope tag) | lockfile equality |

Tags: each test has its own tag line directly above `#[test]`, naming
the catalogue IDs (`prolog/requirements/traceability.pl`, `property/3`)
that test checks; each file header lists the union. `run.sh`,
`run-mutants.sh` and `wasm-surface.sh` tag what they run. Finding IDs are
not tags: each finding sits under the property it breaks (F-1 under
LIVE-2). Tags that name other layers' properties are cross-references:
LIVE-1, LIVE-2, LIVE-ALL, RT-WEDGE and RT-RACE (mocked-runtime analogues)
and ASM-DEPLOY (wasm surface). `tests/lockfile.rs` checks the content of
no property; it carries the near-tests IDs its tests check only as the
scope of its precondition. SH-11 and SH-12 are checked by
`wasm-surface.sh`, which builds from the mirror.

`check-tags.py` (python3 -I, standard library only, under a second) checks:
every tag line is a comment of IDs only, each ID in `property/3`; every
`#[test]` has its own tag line; every property whose layers include
near-tests is tagged on a test that checks it (lockfile.rs does not count)
or on the tag line of a `wasm-surface.sh` check SURF-n; each file header
equals the union of its tests' tags; `run.sh`, `run-mutants.sh` and
`wasm-surface.sh` carry tags, and SURF-1, SURF-2 and SURF-3 each have a tag
line; no SURF-n is used as a tag; the tag set `check-status.sh` extracts
equals the one found. Checked 2026-11-14: 0 problems (catalogue 118 IDs,
22 in layer near-tests; 38 tests tagged; 28 distinct IDs).

## Property results (v1.1.0)

| ID | Result | Evidence |
|---|---|---|
| SH-1 | pass | `sh1_caller_is_the_predecessor_not_the_signer`, `sh1_now_is_the_block_timestamp_in_whole_seconds`, 15% relayed calls in SH-8; mutants s02, s10 |
| SH-2 | pass | `sh2_new_is_private_and_initialises_once`; mutant s09 |
| SH-3 | pass (NT-F4 fixed) | `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh3_non_string_promise_results_become_unknown` (a failed call, no result, two results, more than 64 bytes, or anything but one JSON string is "unknown"), the NotFound event for a missing or pruned id (`sh9_pruned_*`); mutants s01, s03, s04, s08, s14, s23, s24, s25 |
| SH-4 | pass | `sh4_rejected_submit_returns_the_decision_and_writes_nothing`; the crafted allowed-but-failing submit in `sh5_core_errors_are_panics_with_the_error_string`; mutant s13 |
| SH-5 | pass (NT-F1 fixed) | `sh5_core_errors_are_panics_with_the_error_string` on both paths, including a crafted state (`first_id = u32::MAX`) where the preview allows a submit the core refuses with Arithmetic: the shell panics with it and writes nothing; `nt_f1_documented_build_reports_every_panic_message`: the documented build reports the error string (the Plain build, the negative control, still traps bare); SURF-3: build.sh refuses a wasm without `panic_utf8`; mutants s07, s13, s14, s25 |
| SH-6 | pass | `sh6_dispatch_promise_shape` (`promise_create` execute 30 Tgas, `promise_then` on_result 30 Tgas, `promise_return` of the callback); mutants s11, s27, s28 |
| SH-7 | pass | `sh7_reconcile_guard_and_promise_shape`; mutants s05, s15, s28 |
| SH-8 | pass (host, wasm, window, traces) | `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `conformance_traces_through_the_shell` (216 traces, 8,865 calls, host and documented build) |
| SH-9 | pass | `sh9_views_and_paging_by_id_through_the_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id`; mutants s06, s26 |
| SH-10 | pass (NT-F2 fixed) | `sh10_mutating_methods_reject_attached_deposits`, `nt_f2_reconcile_rejects_an_attached_deposit`; mutants s12, s22 |
| SH-11 | pass | `wasm-surface.sh` SURF-1: `pool_contract` exports its 12 methods, `contract_source_metadata` and `memory` (14), `pool_executor` its 8 methods (`on_settled` new) and the same two (10), as in `../wasm-surface.expected`; no upgrade, migrate or reset export |
| SH-12 | pass (NT-F3 fixed) | SURF-2: no forbidden import (deploy, keys, transfer, stake, accounts, global contracts, `promise_batch_*`); SURF-3(a): the imports equal build.sh's `ALLOWED` exactly (contract 18, executor 16) |
| RD-PREVIEW | pass (shell half) | the `preview` view equals `Pool::preview` in SH-8 and at the window (`sh9_views_*`). No mutant targets it |
| SCN-1 | pass | `scn1_published_scenarios_are_the_core` (6 scenarios) |
| EX-1 | pass | `ex1_only_the_lock_executes`, `ex1_executor_wrappers`; mutant s17 |
| EX-2 | pass | `ex2_payload_binding_and_dedupe` (512-job crafted state), the differential; mutants s16, s21 |
| EX-3 | pass | `ex3_only_the_operator_attests`; mutant s18 |
| EX-4 | pass | `ex4_terminal_conflict_rejected`; mutant s19 |
| EX-5 | pass | `ex5_notify_calls_lock_settle`, end-to-end flows; mutants s20, s35 |
| EX-6 | pass | `ex6_window_forgets_and_caps`, `ex6_window_constants_match_the_lock`, `ex6_random_differential_against_the_model`; mutants s31-s34, s36, s37, s00b |
| EX-7 | pass (mocked runtime) | `ex7_d1_confirmed_failed_executes_stay_within_256` (EX-7 checked after every transaction), `ex6_window_constants_match_the_lock`; mutant s34, s00b. The sandbox half is RT-D1 |
| EX-8 | pass | `ex8_on_settled_protocol`, `ex8_callbacks_in_any_order`, `ex1_executor_wrappers` (privacy on wasm); mutants s29, s30, s35, s38, s00b |
| LIVE-2 (F-1) | regression pass | `wedge.rs`: the F-1 hedge is refused with Bounds; so is trace 0 of the v1.1.0 corpus (`conformance_traces_through_the_shell`) |

## Wasm surface (`./run.sh surface`)

`wasm-surface.sh` builds the documented build once and checks, all three
must hold (exit 0; 1 a check fails; 2 a build fails):

- SURF-1 (SH-11, SH-5, ASM-DEPLOY): the surface (exports, imports, class
  `promise` or `-`) equals `../wasm-surface.expected`, regenerated for
  v1.1.0.
- SURF-2 (SH-12, ASM-DEPLOY): no forbidden import. The local list equals
  build.sh's `FORBIDDEN` plus the `promise_batch_` prefix.
- SURF-3 (SH-5, SH-12): (a) upstream `scripts/build.sh` run in the mirror
  (CARGO_TARGET_DIR, RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS and RUSTUP_TOOLCHAIN
  unset) passes; `artifacts/build.json` has `dirty: false`, `source_commit`
  equal to the gitlink pin and `rustflags: "--cfg near"`; its sha256 values
  equal this layer's wasm (contract `e6383aab…e2ea`, 377,358 bytes;
  executor `b92f1469…4e83`, 223,068 bytes; rustc 1.98.1); each wasm's
  imports equal build.sh's `ALLOWED` set exactly (no extra, no unused
  entry: contract 18, executor 16). (b) build.sh's own checker (the
  heredoc, extracted and run with `python3 -I` in scratch dirs) exits 1
  with the expected message for the no-cfg wasm ("does not import
  env.panic_utf8"), a synthetic module importing
  `promise_batch_action_transfer` ("imports host functions for actions
  other than function calls") and one importing `promise_yield_create`
  ("has imports that are not in the reviewed list").

ASM-DEPLOY: the in-contract half is now checked (no deploy, key, transfer,
stake, account or global-contract import, no upgrade export). What remains
assumed is the account holder's own keys and redeploys.

Last run 2026-11-14 (the check; `../wasm-surface.expected` was regenerated
with `--update` on 2026-11-13): exit 0, all of SURF-1, SURF-2, SURF-3(a) and
SURF-3(b) ok; the mirror's git status stays clean (`target/` and
`artifacts/` are gitignored upstream).

## Mutants (`./run.sh mutants`)

41 mutants, each applied with `patch -s -p1` (no fuzz) to a temporary copy
of the source; the unpatched copy runs first as the control. Per mutant,
the documented build (`--cfg near`) of the patched tree is rebuilt before
the suite runs (a build failure is ERROR, never a kill); the no-cfg wasm,
used only by `nt_f1_*`, is built once from the unpatched tree. Sizes: host
store_diff 300 x 40, wasm 40 x 40, everything else at its default.

- s01-s21: v1.0.0's m01-m21, re-based on v1.1.0 (`s` for shell, so the
  names do not clash with the Prolog model mutants). s08 is now "a
  `Failed` promise result reads as active" in the raw-result parse.
- s22-s38 (s32 split into s32a in `execute` and s32b in
  `confirm_not_executed`): reverts of the v1.1.0 fixes (NT-F2, NT-F4, F-3
  in the lock and in the executor, NT-F3's `promise_return`, D-1's
  `on_settled`).
- s00a, s00b: historical mutants, the v1.0.0 production code of the
  contract and of the executor (`6f61616`) over the v1.1.0 core. They
  build against v1.1.0 (the contract uses only `records`, and `record()`
  is additive). The harness needs two cfg shims for them, set by
  `run-mutants.sh` from the patch header: `nt_v100_contract` passes the
  `#[callback_result]` argument of v1.0.0's `on_result` on the host path;
  `nt_v100_executor` reads v1.0.0's executor state (no `newest`) and has
  no host `on_settled`. Ordinary builds set neither.

s13 is equivalent in every reachable state of v1.1.0: `preview` is the
candidate of `submit`, and the only check `submit` adds beyond it,
`first_id + len` overflowing, needs more ids than the version cap allows.
The first full run had s13 survive; the crafted state at the end of
`sh5_core_errors_are_panics_with_the_error_string` (`first_id = u32::MAX`)
now kills it.

The default run includes `tests/traces.rs` (the committed v1.1.0 corpus);
`NT_SKIP_TRACES=1` leaves it out for a quicker run.

Last recorded run 2026-11-14 at the integration commit a7781c7 (`./run.sh
all`, `tests/traces.rs` included; 38 of 38 tests and the surface checks
pass, then the mutants, about 21 min of the 22-minute run): the control
survives, 41 of 41 mutants killed, exit 0. Each verdict below lists every
failing test. `conformance_traces_through_the_shell` fails on 8 of them
(s00a, s04, s07, s08, s10, s11, s25, s28), each also killed by other
tests; the run before the corpus was committed (`NT_SKIP_TRACES=1
./run-mutants.sh`, 18 min) gave the same verdicts and the same failing
tests apart from that one.

The historical mutants also fail the surface checks, outside
`run-mutants.sh`: the documented build of s00a + s00b imports 19
`promise_batch_*` functions per wasm (contract 35 imports, executor 32;
SURF-2 fails), and build.sh's own checker exits 1 on them ("imports host
functions for actions other than function calls") without writing
`artifacts/build.json`.

| Mutant | Crate | Breaks | Change | Verdict and failing tests |
|---|---|---|---|---|
| s00a_v100_contract | contract | SH-3/SH-10/SH-12 | the v1.0.0 contract (NT-F2, NT-F3, NT-F4 unfixed; Promise type) | killed (20): `conformance_traces_through_the_shell`, `ex5_settle_before_callback_is_absorbed`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `f1_control_without_the_disturbing_observe`, `f1_hedge_is_refused_through_the_shell`, `nt_f1_documented_build_reports_every_panic_message`, `nt_f2_reconcile_rejects_an_attached_deposit`, `sh10_mutating_methods_reject_attached_deposits`, `sh1_caller_is_the_predecessor_not_the_signer`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh5_core_errors_are_panics_with_the_error_string`, `sh6_dispatch_promise_shape`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id`, `sh9_views_and_paging_by_id_through_the_window` |
| s00b_v100_executor | executor | EX-6/EX-7/EX-8 | the v1.0.0 executor (no window, 256-job cap, no on_settled; Promise type) | killed (11): `ex1_executor_wrappers`, `ex4_terminal_conflict_rejected`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_notify_calls_lock_settle`, `ex6_random_differential_against_the_model`, `ex6_window_constants_match_the_lock`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol`, `f1_no_wedge_end_to_end_with_the_executor` |
| s01_on_result_not_private | contract | SH-3 | on_result accepts any predecessor | killed (2): `nt_f1_documented_build_reports_every_panic_message`, `sh3_on_result_is_private` |
| s02_caller_is_signer | contract | SH-1 | caller() is the signer instead of the predecessor | killed (4): `sh1_caller_is_the_predecessor_not_the_signer`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s03_swap_completed_cancelled | contract | SH-3 | on_result maps completed and cancelled to the opposite settlement | killed (9): `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_settle_before_callback_is_absorbed`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `f1_hedge_is_refused_through_the_shell`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_maps_the_status_string`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s04_unknown_settles_cancelled | contract | SH-3 | an "unknown" status is treated as definitive non-execution | killed (13): `conformance_traces_through_the_shell`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_settle_before_callback_is_absorbed`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `f1_hedge_is_refused_through_the_shell`, `f1_no_wedge_end_to_end_with_the_executor`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_maps_the_status_string`, `sh5_core_errors_are_panics_with_the_error_string`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s05_reconcile_without_status_guard | contract | SH-7 | reconcile no longer refuses records that are not in flight | killed (5): `nt_f1_documented_build_reports_every_panic_message`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s06_get_commitments_limit_100 | contract | SH-9 | get_commitments pages up to 100 records | killed (3): `sh8_store_diff_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id`, `sh9_views_and_paging_by_id_through_the_window` |
| s07_settle_emits_no_event | contract | SH-5/SH-8 | settle forgets its "settled" event | killed (11): `conformance_traces_through_the_shell`, `ex5_settle_before_callback_is_absorbed`, `f1_control_without_the_disturbing_observe`, `f1_hedge_is_refused_through_the_shell`, `sh1_caller_is_the_predecessor_not_the_signer`, `sh5_core_errors_are_panics_with_the_error_string`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_views_and_paging_by_id_through_the_window` |
| s08_on_result_promise_error_is_active | contract | SH-3 | a failed execute (PromiseError::Failed) acknowledges the record as Active | killed (12): `conformance_traces_through_the_shell`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_settle_before_callback_is_absorbed`, `f1_hedge_is_refused_through_the_shell`, `f1_no_wedge_end_to_end_with_the_executor`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_maps_the_status_string`, `sh5_core_errors_are_panics_with_the_error_string`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s09_new_not_private | contract | SH-2 | anyone may initialise the pool | killed (1): `sh2_new_is_private_and_initialises_once` |
| s10_now_rounds_up | contract | SH-1 | now() rounds the block timestamp up to the next second | killed (24): `conformance_traces_through_the_shell`, `ex5_settle_before_callback_is_absorbed`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `f1_control_without_the_disturbing_observe`, `f1_hedge_is_refused_through_the_shell`, `f1_no_wedge_end_to_end_with_the_executor`, `nt_f1_documented_build_reports_every_panic_message`, `nt_f2_reconcile_rejects_an_attached_deposit`, `sh10_mutating_methods_reject_attached_deposits`, `sh1_caller_is_the_predecessor_not_the_signer`, `sh1_now_is_the_block_timestamp_in_whole_seconds`, `sh2_new_is_private_and_initialises_once`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh4_rejected_submit_returns_the_decision_and_writes_nothing`, `sh5_core_errors_are_panics_with_the_error_string`, `sh6_dispatch_promise_shape`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id`, `sh9_views_and_paging_by_id_through_the_window` |
| s11_execute_sent_to_actor | contract | SH-6 | dispatch sends execute to the recipe's actor instead of its executor | killed (21): `conformance_traces_through_the_shell`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_settle_before_callback_is_absorbed`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `f1_control_without_the_disturbing_observe`, `f1_hedge_is_refused_through_the_shell`, `f1_no_wedge_end_to_end_with_the_executor`, `nt_f1_documented_build_reports_every_panic_message`, `nt_f2_reconcile_rejects_an_attached_deposit`, `sh10_mutating_methods_reject_attached_deposits`, `sh1_caller_is_the_predecessor_not_the_signer`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh5_core_errors_are_panics_with_the_error_string`, `sh6_dispatch_promise_shape`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_views_and_paging_by_id_through_the_window` |
| s12_submit_payable | contract | SH-10 | submit accepts an attached deposit | killed (1): `sh10_mutating_methods_reject_attached_deposits` |
| s13_submit_ignores_preview | contract | SH-4/SH-5 | submit tries the core directly and keeps only an Ok id (a core error after an allowed preview no longer panics) | killed (1): `sh5_core_errors_are_panics_with_the_error_string` |
| s14_on_result_panics_on_core_error | contract | SH-3/SH-5 | on_result panics on a core error instead of reporting it in the event | killed (3): `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s15_reconcile_status_gas_30 | contract | SH-7 | reconcile attaches 30 Tgas to status instead of 10 | killed (5): `nt_f2_reconcile_rejects_an_attached_deposit`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s16_executor_dedupe_dropped | executor | EX-2 | execute no longer deduplicates a replayed id (a second job is pushed) | killed (6): `ex2_payload_binding_and_dedupe`, `ex4_terminal_conflict_rejected`, `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_on_settled_protocol` |
| s17_executor_lock_is_signer | executor | EX-1 | execute checks the signer, not the predecessor | killed (1): `ex1_only_the_lock_executes` |
| s18_executor_set_mode_unguarded | executor | EX-3 | anyone may set the executor's mode | killed (1): `ex3_only_the_operator_attests` |
| s19_executor_terminal_conflict_dropped | executor | EX-4 | finish may replace a terminal outcome with the other one | killed (3): `ex4_terminal_conflict_rejected`, `ex6_random_differential_against_the_model`, `f1_no_wedge_end_to_end_with_the_executor` |
| s20_executor_notify_inverted | executor | EX-5 | notify reports the opposite outcome to the lock | killed (9): `ex4_terminal_conflict_rejected`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_notify_calls_lock_settle`, `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol`, `f1_no_wedge_end_to_end_with_the_executor` |
| s21_executor_payload_unbound | executor | EX-2 | a replayed id with another payload returns the recorded status | killed (2): `ex2_payload_binding_and_dedupe`, `ex6_random_differential_against_the_model` |
| s22_reconcile_takes_a_deposit | contract | SH-10 | reconcile's deposit check is removed (NT-F2 reverted) | killed (6): `nt_f2_reconcile_rejects_an_attached_deposit`, `sh10_mutating_methods_reject_attached_deposits`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s23_on_result_reads_65_bytes | contract | SH-3 | on_result accepts a 65-byte promise result | killed (4): `sh3_non_string_promise_results_become_unknown`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s24_on_result_any_result_count | contract | SH-3 | on_result reads result 0 of a callback with two or more promise results | killed (4): `sh3_non_string_promise_results_become_unknown`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window` |
| s25_on_result_not_found_panics | contract | SH-3/SH-5 | on_result of a missing record panics NotFound again (NT-F4 reverted) | killed (6): `conformance_traces_through_the_shell`, `sh3_on_result_maps_the_status_string`, `sh8_store_diff_host`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id` |
| s26_get_commitments_skip_from | contract | SH-9 | get_commitments skips `from` records, not `from - first_id` (pages by index, not id) | killed (3): `sh8_store_diff_window`, `sh9_pruned_ids_are_not_found_and_pages_are_by_id`, `sh9_views_and_paging_by_id_through_the_window` |
| s27_dispatch_indexes_records | contract | SH-6 | dispatch reads records[id], the index, not the record with that id | killed (3): `ex7_d1_confirmed_failed_executes_stay_within_256`, `sh8_store_diff_window`, `sh9_views_and_paging_by_id_through_the_window` |
| s28_promise_return_is_the_call | contract | SH-6/SH-7 | dispatch and reconcile return the executor call, not the on_result callback | killed (18): `conformance_traces_through_the_shell`, `ex5_end_to_end_normal_reject_unknown_panic`, `ex5_settle_before_callback_is_absorbed`, `f1_control_without_the_disturbing_observe`, `f1_hedge_is_refused_through_the_shell`, `nt_f1_documented_build_reports_every_panic_message`, `nt_f2_reconcile_rejects_an_attached_deposit`, `sh10_mutating_methods_reject_attached_deposits`, `sh1_caller_is_the_predecessor_not_the_signer`, `sh3_non_string_promise_results_become_unknown`, `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh5_core_errors_are_panics_with_the_error_string`, `sh6_dispatch_promise_shape`, `sh7_reconcile_guard_and_promise_shape`, `sh8_store_diff_wasm`, `sh8_store_diff_window`, `sh9_views_and_paging_by_id_through_the_window` |
| s29_on_settled_advances_on_failed | executor | EX-8 | on_settled advances even when the lock's settle failed | killed (4): `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_on_settled_protocol` |
| s30_on_settled_not_private | executor | EX-8 | on_settled accepts any predecessor | killed (3): `ex6_random_differential_against_the_model`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_on_settled_protocol` |
| s31_retain_keeps_the_boundary | executor | EX-6 | advance keeps the job exactly WINDOW below newest (`>=`) | killed (5): `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol` |
| s32a_execute_stale_off_by_one | executor | EX-6 | execute starts an id exactly WINDOW below newest (`<`, not `<=`) | killed (3): `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex8_on_settled_protocol` |
| s32b_confirm_stale_off_by_one | executor | EX-6 | confirm_not_executed accepts an id exactly WINDOW below newest (`>=`, not `>`) | killed (3): `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256` |
| s33_confirm_without_stale_check | executor | EX-6 | confirm_not_executed accepts a forgotten id | killed (3): `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256` |
| s34_max_jobs_256 | executor | EX-6/EX-7 | the executor holds at most 256 jobs (the v1.0 cap) | killed (6): `ex6_random_differential_against_the_model`, `ex6_window_constants_match_the_lock`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol` |
| s35_notify_returns_on_settled | executor | EX-5/EX-8 | notify returns on_settled's result instead of the lock's settle | killed (6): `ex5_notify_calls_lock_settle`, `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol` |
| s36_advance_after_the_cap | executor | EX-6 | execute checks the cap before advance frees jobs | killed (2): `ex6_window_forgets_and_caps`, `ex8_on_settled_protocol` |
| s37_execute_advances_on_a_stored_job | executor | EX-6 | an execute of an id that has a job raises newest (D-9) | killed (5): `ex2_payload_binding_and_dedupe`, `ex6_random_differential_against_the_model`, `ex6_window_forgets_and_caps`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_on_settled_protocol` |
| s38_on_settled_never_advances | executor | EX-8 | on_settled never raises newest | killed (4): `ex6_random_differential_against_the_model`, `ex7_d1_confirmed_failed_executes_stay_within_256`, `ex8_callbacks_in_any_order`, `ex8_on_settled_protocol` |

## Gaps

- The mocked runtime is not the real NEAR runtime: gas, storage staking,
  receipt timing across blocks and the real promise-result plumbing are the
  sandbox layer's (RT-*). The router runs receipts FIFO in one block;
  `ex8_callbacks_in_any_order` permutes the `on_settled` callbacks
  explicitly.
- The wasm hashes are local reproducibility evidence (rustc 1.98.1 on this
  machine); they become a CI gate only once CI produces the same hashes.
- SH-8 random configurations are small (1-3 metrics, 1-3 recipes, values
  near +-10); Full and Arithmetic are covered by crafted states.
