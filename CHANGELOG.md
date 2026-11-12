# Changelog

All notable changes to this project are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-11-12

This release fixes the findings of an outside verification of v1.0.0. They are
described in [docs/verification-findings.md](docs/verification-findings.md).
The formal results there (TLA+, Lean, Prolog, conformance traces) apply to
v1.0.0. v1.1.0 has not been formally re-verified. Its fixes are covered by
unit tests, seeded random tests and the sandbox suite.

### Fixed

- F-1 (high): a hedge admitted after an `observe` broke a bound could never
  settle. Admission now counts every outcome of every open record, including
  its release. Every open record can now be resolved: a Reserved one by
  cancel, a dispatched one by its executor's report.
  [F-1](docs/verification-findings.md#f-1-high-disturbed-state-hedge-wedge)
- F-2 (high): in the `risk` fixture, a position that a completed hedge relied
  on could not be released. A hedge must now stay in bounds if the position is
  later reported unexecuted.
  [F-2](docs/verification-findings.md#f-2-high-healthy-state-release-wedge-in-the-risk-fixture)
- F-2b: a release could leave the value range and fail `Arithmetic`. The
  release is now inside the admission interval.
  [F-2b](docs/verification-findings.md#f-2b-a-release-can-leave-the-value-range)
- F-3 (low): one actor could use up all 256 lifetime records with free
  submit-and-cancel cycles. The pool now keeps a window of the 256 most recent
  records. [F-3](docs/verification-findings.md#f-3-low-lifetime-record-exhaustion)
- F-4 (low): a recipe's actor could be its own executor. `new` now rejects
  such a config (`InvalidConfig`).
  [F-4](docs/verification-findings.md#f-4-low-a-principal-can-attest-its-own-work)
- F-5 (low): at version u32::MAX every transition failed, including
  settlement. Submit, dispatch, pause and observe now stop 768 versions
  earlier, so acknowledgment, settlement and cancellation always remain
  available.
  [F-5](docs/verification-findings.md#f-5-low-version-saturation-freezes-every-transition)
- F-6 (low): one long-open record blocked the snapshot refresh. The owner can
  now restate the current values at any time.
  [F-6](docs/verification-findings.md#f-6-low-an-open-record-blocks-the-only-snapshot-refresh)
- NT-F1 (medium, diagnostics): the documented build dropped every panic
  message. `scripts/build.sh` now sets `--cfg near` and fails if a Wasm does
  not import `env.panic_utf8`.
  [NT-F1](docs/verification-findings.md#nt-f1-medium-diagnostics-not-safety-the-documented-build-drops-every-panic-message)
- NT-F2 (low): `reconcile` kept an attached deposit. It now rejects any
  deposit. [NT-F2](docs/verification-findings.md#nt-f2-low-reconcile-keeps-an-attached-deposit)
- NT-F4 (low): a promise result that was not a JSON string made the callback
  panic. `on_result` now reads the raw result; anything but one JSON string of
  at most 64 bytes becomes Unknown.
  [NT-F4](docs/verification-findings.md#nt-f4-low-a-non-string-promise-result-does-not-become-unknown)

### Changed

- Stricter admission. Two kinds of request that v1.0.0 admitted are refused:
  a reservation that by itself reduces a violated metric (recovery must come
  from completion), and a request that is in bounds only while another open
  record's reservation stays in place. In the `risk` fixture, with one credit
  position open, the largest admitted selloff hedge is 6 units, not 8.
- Record window. The pool keeps the 256 most recent commitments. A submit
  prunes the oldest one when it is completed or cancelled, and fails `Full`
  while it is open. A dispatched oldest record closes only when its executor
  reports, so a silent executor can hold submit at `Full`. Ids keep counting, and a pruned id is `NotFound`
  (including a repeated executor `settle`, which v1.0.0 accepted). `get_pool`
  gains `first_id` and `completed_units`; prerequisites count completed units
  over the pool's life. `get_commitments` takes `from` as an id.
- `cancel` of a Completed record fails `InvalidState` (was `Pending`).
  Dispatched, Active and Unknown records still fail `Pending`.
- Version reserve: submit, dispatch, `set_paused` and `observe` fail
  `Arithmetic` from `VERSION_CAP` (u32::MAX - 768). `preview` reports it too.
- Executor retention. The synthetic executor forgets a job once its id is 256
  or more behind the newest id it has executed. `execute` of a forgotten id
  returns `"cancelled"`, `confirm_not_executed` of one fails `stale id`, and
  the job cap is 512.
- Wasm import surface (NT-F3). Both contracts create promises with
  `promise_create`, `promise_then` and `promise_return` instead of near-sdk's
  `Promise`. The 15 host imports for accounts, deploys, keys, transfers and
  staking are gone, and `scripts/build.sh` fails if one comes back. It also
  fails on any import that is not on its list of the current 18 contract and
  13 executor imports.
  `dispatch`, `reconcile`, `finish` and `confirm_not_executed` return `()` in
  Rust; on chain the result is still the callback's.
- Build: `scripts/build.sh` sets `RUSTFLAGS="--cfg near"`, and
  `artifacts/build.json` records the flags and each Wasm's imports.
- `on_result` logs a result for an id with no record (`error: "NotFound"`)
  instead of panicking.

### Docs

- An acknowledgment that does not change the status is `Ok` without a
  version change. The docs now say so (the code already did).
- [docs/api.md](docs/api.md) describes the events. Their format version
  stays `1.0.0`: no event shape changed. It also says that the read methods
  keep a deposit sent with a transaction.
- [docs/api.md](docs/api.md) has a gas table, measured on the v1.1.0 Wasm at
  a maximum-size state by `crates/sandbox/tests/res1.rs` (ignored by
  default; run it with `--ignored`).
- [docs/verification-findings.md](docs/verification-findings.md) gives each
  finding's status.

## [1.0.0] - 2026-11-12

First release (commit `80fb89e`, not tagged): the pool core, the NEAR
contract, the synthetic executor, six scenarios, the local demo and static
UIs, and the sandbox integration tests. It is the baseline the outside
verification checked: its core, contract and executor are byte-identical to
commit `a4bb5c3`.
