# Shared Pool Controls Formal Verification Strategy

## Target

[`difflabai/shared-pool-controls`](https://github.com/difflabai/shared-pool-controls)
v1.1.0 (PR #4, merged) is a NEAR contract that admits agent work against one shared pool of
bounded metrics. The pool holds metric values, an immutable policy, and a
sliding window of the 256 most recent commitment records (ids are never
reused; a pruned id is NotFound). The policy has metrics with hard bounds,
weighted potential terms, and recipes. Each recipe names one actor and one
executor, which must differ, with reservation and completion effects, a
unit cap, and prerequisites on the completed units of earlier recipes.
`pool-core` admits a request only if the reservation does not raise the
potential and, for every metric, the interval that contains every outcome
of the open records and the new one (each stays open, completes, or
releases its reservation) violates the hard bounds by no more than the
metric does now or after the reservation. The actor dispatches, the contract calls the
recipe's fixed executor, and the executor settles the record as completed
or not executed. The owner pauses the pool and replaces the measured values
(`observe`); an observe of the current values with records open refreshes
the snapshot. New-work calls stop at a version cap below `u32::MAX`, so
settlement always has version room.

| | |
|---|---|
| Pinned commit | `57b3f02d18e85149db3c1035431368c4e07a4d8a`, the merge commit of upstream PR #4 on `main` (2026-11-13 UTC, "Merge pull request #4 from difflabai/gabehamilton/v1.1-verification-findings"). Its tree is the tree of the PR head `6fcbd56` (`status.toml` `[project] identity`, checked by `tools/scripts/check-pin.sh`). |
| Version under verification | shared-pool-controls v1.1.0 (root `Cargo.toml` `version = "1.1.0"`), on the branch `gabehamilton/shared-pool-controls-v1.1` (PR #32). |
| Previous verification | v1.0.0: pin `a4bb5c3`, whose contract code is that of upstream's v1.0.0 commit `6f61616` (upstream's findings page calls it `80fb89e`; `git diff 6f61616 80fb89e -- crates scripts Cargo.toml Cargo.lock` is empty). Recorded in PR #31 (open). Its last results commit is `a072a2d` (the local Kani record); the two commits after it, `22df7b6` and `651852b` (#31's head), harden the record check and change no layer's results. The tag `shared-pool-v1.0.0` goes on #31's head once its `kani`, `kani-record` and `covers` jobs pass (the tag does not exist yet; until it does, cite the commits). It found 6 core findings (F-1..F-6, with F-2b) and 4 shell findings (NT-F1..NT-F4); v1.1.0 closes all of them (§4). |
| Crates | `pool-core` (`crates/core/src/lib.rs`, 566 lines): every decision; zero dependencies in the default build; `#![forbid(unsafe_code)]`; serde and borsh optional. `pool-contract` (452 lines): a near-sdk 5.29.1 shell (predecessor → caller, block time in whole seconds, core errors → panics, a low-level promise to the executor, a private callback that reads the raw result, views). `pool-executor` (571 lines): the synthetic executor with a window of 256 job ids and a cap of 512 stored jobs (`execute`, `status`, `finish`, `confirm_not_executed`, the private `on_settled`, fault modes). `crates/sandbox`: upstream's near-workspaces tests, including `res1.rs` (gas). `scripts/build.sh`: the documented build. |
| Toolchain | Upstream `rust-toolchain.toml` 1.98 (rustc 1.98.1 here). Release profile: codegen-units 1, `opt-level = "z"`, LTO, `panic = "abort"`, `overflow-checks = true` (ASM-COMPILE). `scripts/build.sh` builds with `--cfg near` and checks the wasm's imports against an allowlist. |
| TCB via `Cargo.lock` | near-sdk 5.29.1, borsh 1.8.1, serde_json 1.0.154, and their dependencies |
| Mirror | git submodule `projects/shared-pool/source-mirror` (`upstream_path = ""`) |
| Upstream docs | `docs/spec.md`, `docs/api.md`, `docs/verification-handoff.md`, cited as `spec:<line>`, `api:<line>`, `handoff:<line>` at the pin. Text that v1.1.0 deleted is cited at v1.0.0 as `spec@v1.0.0:<line>` (and so on), at the v1.0.0 commits above. |

The split matches near-approval's. Every decision is in `pool-core`, which
has no dependencies, so Kani can target the deployed logic itself. The
shell is thin enough to cover with tests on the host, on the wasm, and on a
local chain.

**Status (2026-11-14, pin `57b3f02`, v1.1.0).** Every layer has
re-checked v1.1.0 at the pin, and each passes: Kani (bounded proofs on the
real core, §6), Lean (the model at every
size, §18), TLA+ (small universes, the version cap and the executor
protocol, §17), Prolog (the docs, seven MC-1 universes, the traces and the
reads, §8), near-tests (the shell and the executor on the documented build,
§9.1) and the sandbox (the real wasm on a local chain, §9.2). The v1.1.0
conformance corpus (216 traces, 8,865 calls) replays with equal results
and post-states through TLA+, Lean, Prolog and the shell (§10). No v1.0.0
finding reproduces: each witness is inverted into the property it guarded,
and each upstream fix hunk has a fix-revert mutant (FR-n) that the closing
layers kill (§4.1, §4.5). The re-verification found no new finding (§4.7);
three behaviours are documented limits, not findings (§4.6). Kani's
large harnesses and mutants are a local record that CI checks (§14).
Nothing is verified: there is no spec reviewer (§12).

---

## 1. What we establish

For `pool-core` at the pin (v1.1.0), and for the shell built as
`scripts/build.sh` builds it (`--cfg near`), under the assumptions of §5.14:

| Claim | Properties |
|---|---|
| **(a) Admission.** An accepted `submit` came from the recipe's actor at the current version, and the actor is not the recipe's executor. Its prerequisites were met by the lifetime completed units of earlier recipes. It did not raise the potential, and in a disturbed state its completion strictly lowers it. Per metric, the interval over every outcome of the open records (open, completed or released) violates the hard bounds by no more than now. From a healthy baseline every combination of outcomes lands in bounds. | AUTH-1, CFG-2, STALE-1, DEP-1, ADM-1, ADM-2, HARD-STEP, INV-RES, INV-HARD, INV-HEALTHY |
| **(b) Atomicity and authority.** A rejected call changes nothing in the core, the shell, or the wasm. Only the executor settles. Only the owner pauses and observes. The actor or owner cancels. The caller is the predecessor. `new`, `on_result` and the executor's `on_settled` are private. The wasm imports only allowlisted host functions. | TX-0, SH-5, RT-REVERT, AUTH-1, SH-1, RT-PRED, SH-2, SH-3, RT-PRIVATE, SH-12 |
| **(c) Lifecycle and accounting.** Ids are issued in order and never reused; the core keeps the 256 most recent records and prunes only a terminal front; records follow the status graph; terminal statuses are absorbing. Values equal the last observed baseline plus the reservations and completions since. An unknown outcome keeps its reservation. The version counts effective transitions and new work stops at `VERSION_CAP`. | LIFE-1, WIN-1, WIN-2, TERM-1, INV-LEDGER, ASYNC-1, INV-VER, INV-VERCAP, VER-CAP |
| **(d) Settlement.** In every reachable state, each open record can be settled or cancelled with `Ok` whatever the others do, with no premise on the baseline or the version: no wedge (F-1, F-2 fixed), no version exhaustion (F-5), no lock-out by a full window (F-3), and an observe of the current values always refreshes (F-6). Under executor fairness every dispatched record ends (LIVE-2). | LIVE-ALL, LEAN-LIVE-ALL, LIVE-1, LIVE-2, WIN-ESC, WIN-LIVE, OBS-2, REFRESH-1 |
| **(e) Capacity and freshness of the executor.** The synthetic executor keeps a window of 256 job ids and at most 512 jobs; it never forgets the job of a record the lock retains; an honest confirmation or settle never fills the cap. Model-checked in TLA+ (TLA-EXEC) and tested on the mocked runtime and the sandbox. | EX-6, EX-7, EX-8, TLA-EXEC, RT-D1 |

**What "establish" means today.** At v1.1.0, (a), (c), (d) and the core
half of (b) are proved at every size for a Lean transcription (LIVE-ALL
with no premise beyond a valid config and INV), model-checked in TLA+ in
small universes (including states at and above a scaled version cap, and
LIVE-2 and WIN-LIVE under named fairness), and checked over 216 real-core
traces by TLA+, Lean, Prolog and the shell. (e) and the shell and runtime
half of (b) are tested on the mocked runtime and on a local chain, and the
executor protocol is model-checked at windows 2 and 3. Kani's bounded
proofs on the real core hold within small shapes (§6). None is verified
(§12).

**Not established:** that a deployment runs this code (ASM-DEPLOY); that
the executor reports at all, or truly (ASM-EXEC), and true observations
(ASM-OWNER); that callers attach enough gas (ASM-GAS; RES-1 measures what
suffices); termination outside the models' universes and fairness
assumptions (LIVE-2 is conditional on authoritative evidence and gas,
handoff:34); that a full window empties before its oldest record's
executor reports (head-of-line `Full`, §4.6); that the synthetic executor
can always send its report once its job cap is full (§4.6); that a policy
models the world (spec:33); receipt order across shards; optimality of
`best_quantity` (spec:29); safety under arbitrary future market moves
(handoff:32).

### Tiers

| Tier | Layers | State | Result |
|---|---|---|---|
| **1. Code-level core** | Kani (§6) | built (v1.1.0) | Bounded proofs on the real `pool-core`: every transition contract, the step properties, INV preservation up to 3 records, the numerics, the reads, and REF-1..3, which tie each TLA+ step to the code. Small shapes and anchored values (§6). |
| **2. Models and operational** | TLA+ (§17), Lean (§18), Prolog (§8), near-tests and sandbox (§9) | built (v1.1.0) | Every-size safety and LIVE-ALL for the model (Lean). Liveness under named fairness, the version cap and the executor protocol over all event sequences in small universes (TLC). An independent reading of the docs (Prolog). The shell on the host and on the documented wasm build. Runtime assumptions and gas on a local chain. TLA+, Lean, Prolog and the shell are tied to the real core by the traces (§10); the sandbox by a host `pool-core` oracle (§9.2). |
| **3. Unbounded on the code** | Verus (§7) | deferred | Removes the transcription gap between the Lean model and the code. |

Tier 1: CI runs the 50 harnesses that fit its
hosted runner and checks the local record of the other
48 (§14). Tier 3: upstream decided F-1 and
F-2 in v1.1.0, so the code a proof would cover is settled; revisit only if
"formally verified" is to be claimed for the code rather than the model
(§7).

### Upstream's obligations (handoff:9-26)

| Obligation | Properties | State (v1.1.0; Kani: §6) |
|---|---|---|
| CFG | CFG-1, CFG-2 | Proved (model): `validate` decides `Valid`, and a valid recipe's actor is not its executor. Trace 210's config is refused by the core and every replayer. |
| NUM | NUM-1, NUM-2, ROB-1 | Kani (§6). Trace 206 tests the ±1,000,000 boundary; an envelope endpoint past it is refused at admission (F-2b: trace 208, MC-1 `f2b`). |
| AUTH | AUTH-1, SH-1, RT-PRED | Proved (model), model-checked, tested. Actor = executor is refused (F-4 fixed); the owner may be an executor (§4.6). |
| ADM | ADM-1, ADM-2, HARD-STEP, INV-HARD, INV-RES | Proved (model), model-checked, tested. |
| ATOM | TX-0, SH-5, RT-REVERT | Proved (model), tested; every failure carries its message on the documented build (NT-F1 fixed). |
| STALE | STALE-1 | Proved (model), model-checked, tested. |
| ASYNC | ASYNC-1, INV-LEDGER | Proved (model), model-checked, tested. RT-CALLBACK. |
| TERM | TERM-1 | Proved (model), model-checked, tested, for settle and cancel: cancelling a Completed record is `InvalidState`. EX-4. |
| DEP | DEP-1, INV-DEP | Proved (model): prerequisites read `completed_units`, which counts pruned completions (`dep1_iff`). Model-checked, tested (trace 207). |
| BIND | LIFE-1, SH-6, EX-2 | Proved (model), tested. |
| STOCK, CASH | C-5 | Hold on the fixtures (bounded check). |
| RISK | C-5 | Holds on the fixtures: the release search finds no blocked release (v1.0.0: F-2). |
| LIVE (handoff:24) | LIVE-ALL, LEAN-LIVE-ALL, INV-RES, INV-VERCAP, VER-CAP, OBS-2, REFRESH-1, LIVE-1, LIVE-2 | Proved (model) at every size with no premise on the baseline or the version; model-checked (LIVE-2 under named fairness, the cap from 4 below it); tested on the traces and on chain (RT-WEDGE). |
| WIN (handoff:25) | WIN-1, WIN-2, WIN-ESC, WIN-LIVE, RT-WINDOW | Proved (model), model-checked (windows of 1 and 2; WIN-LIVE under fairness of the front's resolver), tested (trace 207; 300 records on chain). Head-of-line `Full` is a documented limit (§4.6). |
| EXEC (handoff:26) | EX-6, EX-7, EX-8, TLA-EXEC, RT-D1 | Model-checked (TLA-EXEC, windows 2 and 3), tested (mocked runtime and sandbox). The job cap through refused confirmations is a documented limit (§4.6). |

---

## 2. Scope

| In scope | Detail |
|---|---|
| `pool-core` | `Config::validate`, `Pool::new`, `energy_terms`, `energy`, `submit`, `dispatch`, `acknowledge`, `cancel`, `settle`, `set_paused`, `observe`; the private `add`, `violations`, `envelope`, `check_bounds`, `candidate`, `index`, `bump`, `bump_new`, `release`; the reads `preview`, `best_quantity` and `record`; all 16 `Error` variants; the `first_id` and `completed_units` fields; `MAX_METRICS` 32, `MAX_RECIPES` 32, `MAX_TERMS` 64, `MAX_RECORDS` 256, `MAX_VALUE` 1,000,000, `MAX_WEIGHT` 1,000, `MAX_UNITS` 100, `VERSION_CAP` = `u32::MAX` − 768, and the 86,400 s expiry window |
| `pool-contract` | The 12 `#[near]` methods (`new`, `get_pool`, `get_commitments`, `preview`, `submit`, `dispatch`, `reconcile`, `on_result`, `settle`, `cancel`, `set_paused`, `observe`); events; low-level promise shapes (`dispatch` and `reconcile` return `()` and the promise); Borsh state; deposits; the wasm surface and its import allowlist. Builds: upstream `scripts/build.sh` (`--cfg near` since v1.1.0) and a plain cargo control build without it (NT-F1's negative control, SURF-3). |
| `pool-executor` | The counterparty settlement depends on (EX-*): its 8 methods, its job window (`WINDOW` 256 ids), job cap (`MAX_JOBS` 512), `newest` id and the private `on_settled` callback |
| NEAR runtime | Tested, not proven: predecessor identity, `#[private]`, callbacks and `PromiseError`, per-receipt atomicity, receipt interleaving, gas at the largest state |
| Docs as requirements | `spec.md`, `api.md`, `verification-handoff.md`, transcribed in Prolog (§8) |
| Published scenarios | `scenarios/<name>/{config,trace}.json` for the six fixtures, re-derived from `pool-core` (SCN-1) |

**Out of scope:** `crates/demo`, `ui/` and upstream's HTTP smoke checks;
deployment (none exists; code replacement, spec:60, is ASM-DEPLOY);
storage staking (no layer runs an account at its storage-stake minimum;
§9.2, §9.3); policy design and economics; executor honesty and owner
truth; broker fills, physical delivery and token custody (handoff:47).

---

## 3. Threats and trust assumptions

Transcribed in `prolog/requirements/threat_model.pl`. C-6 checks that
every threat has a mitigation or residual, and every finding a residual.

### 3.1 Adversaries

| ID | Adversary | Capabilities |
|---|---|---|
| ADV-ACTOR | A recipe actor, or anyone holding its key | `submit`, `dispatch` and `cancel` with any arguments, in any order; reads all state |
| ADV-ANY | Any NEAR account | Every public method, including `reconcile` (api:15) and `preview` with any proposed actor |
| ADV-RELAY | A contract between a key holder and the pool | Its calls are attributed to it, not to the signer (spec:13) |
| ADV-EXEC | The recipe's executor | Attests completion or nonexecution. Trusted only for the truth of that (ASM-EXEC). Never the recipe's actor (CFG-2). |
| ADV-OWNER | The owner | Pause, observe, cancel. Trusted for the truth of observations (ASM-OWNER). Not assumed powerless (spec:60). May also be a recipe's executor (`amb-owner-roles`, §4.6). |
| ADV-DEPLOY | A full-access key on the deployment account | Can replace the code (spec:60). Out of scope (ASM-DEPLOY). |

### 3.2 Threats → mitigating properties

| T | Threat | Mitigated by | Residual |
|---|---|---|---|
| T-ATTRIB | A call attributed to an actor that did not make it | AUTH-1, SH-1, RT-PRED, C-1; ASM-PRED | none |
| T-INIT | Foreign or repeated initialization | SH-2, RT-PRIVATE, C-1; ASM-PRIVATE | none |
| T-FORGE | Execution evidence forged through the callback or a non-executor settle | SH-3, RT-PRIVATE, AUTH-1, C-1; ASM-CALLBACK | none |
| T-REPLAY | An accepted request replayed (there is no nonce) | STALE-1, TX-VER, INV-VER | none |
| T-OVERCOMMIT | An admission lets some subset of open outcomes (completions or releases) break a bound, or worsens a disturbed metric | HARD-STEP, INV-HARD, INV-HEALTHY, INV-RES, TX-SUBMIT | none |
| T-ENERGY | An admission raises the potential | ADM-1, ADM-2 | none |
| T-OVERFLOW | Overflow in values, envelopes, energy, version or `first_id`; or version exhaustion blocks the resolution of open records | NUM-1, NUM-2, ROB-1, VER-CAP, INV-VERCAP, TLA-CAP, C-2, C-4; ASM-COMPILE | none (v1.0.0: F-5) |
| T-PARTIAL | A rejected call leaves part of its effect | TX-0, SH-5, RT-REVERT; ASM-ATOMIC | none |
| T-DOUBLE | A record settles twice, or both ways | TERM-1, LIFE-1, EX-4, C-3 | none |
| T-UNKNOWN | An unknown acknowledgment releases a reservation whose job may have run | ASYNC-1, RT-CALLBACK, SH-3, INV-LEDGER | ASM-EXEC |
| T-PREREQ | Dependent work admitted before its prerequisites complete, or pruning erases the completed count | DEP-1, INV-DEP, WIN-1, C-5 | none |
| T-SELF | A worker authorizes its own acceptance | CFG-2, C-5, AUTH-1; ASM-OWNER | The owner may be a recipe's executor (`amb-owner-roles`; §4.6), trusted under ASM-OWNER (v1.0.0: F-4) |
| T-PAUSE | Admission while paused, or pausing blocks settlement | PAUSE-1, VER-CAP, C-2 | none |
| T-OBS | `observe` with open records, changing policy, or deleting history | OBS-1, OBS-2, INV-WF | ASM-OWNER |
| T-BIND | The executor runs another recipe or quantity, or a duplicate runs twice | EX-1, EX-2, SH-6, LIFE-1 | none |
| T-RECONCILE | Anyone churns the version with `reconcile` to make requests `Stale` | SH-7, TX-NOOP, C-3 | none (a repeated acknowledgment changes nothing, spec:41, api:30) |
| T-DEPOSIT | A mutating method keeps an attached deposit | SH-10, C-1 | none (v1.0.0: NT-F2) |
| T-GROWTH | Unbounded storage or gas | INV-WF, WIN-1, RES-1, SH-9, C-4 | none |
| T-SLOTS | An actor fills the record window so nobody can be admitted again | WIN-1, WIN-2, WIN-ESC, WIN-LIVE, LIVE-ALL, RT-WINDOW, INV-WF, TX-SUBMIT, C-4 | Head-of-line: a dispatched oldest record keeps submit at `Full` until its executor reports (api:28; handoff:38; ASM-EXEC; §4.6) (v1.0.0: F-3) |
| T-FRESHNESS | With `max_age > 0`, an open record blocks the only snapshot refresh | OBS-2, REFRESH-1, TX-SUBMIT, TX-DISPATCH, OBS-1 | none (v1.0.0: F-6) |
| T-WEDGE | A record can never become terminal, and the pool can never be observed again | LIVE-ALL, LEAN-LIVE-ALL, INV-RES, INV-VERCAP, LIVE-1, LIVE-2, TLA-LIVE-F1, TLA-LIVE-F2, LEAN-WIT, RT-WEDGE, MC-1, C-5; ASM-EXEC, ASM-GAS | Liveness is conditional on authoritative evidence and gas (handoff:34); there is no forced administrative release (spec:48) (v1.0.0: F-1, F-2) |
| T-PRUNE | An open record is pruned, an id is reused, or a call on a pruned id (a late callback, a repeated executor settle) acts on another record | WIN-1, WIN-2, SH-3, SH-7, C-2 | none |
| T-FORGET | The executor forgets the job of a record the lock still retains, so a later `execute` or `status` reports cancelled although the job ran | EX-6, EX-7, EX-8, RT-PRIVATE, TLA-EXEC | none |
| T-JOBCAP | The executor's 512-job cap fills, so confirmations and new executes fail and submit stays `Full` | EX-6, RES-1, TLA-EXEC, RT-D1; ASM-EXEC | Only refused confirmations (operator error) or confirmations in flight fill it; the exits are a settle signed with the executor account's key or an accepted `finish(id, false)` (api:42; handoff:39; §4.6) |
| T-REDEPLOY | Code replaced through a full-access key | SH-11, SH-12; ASM-DEPLOY | out of scope |

`threat_model.pl` also records an accepted risk for T-REDEPLOY, T-UNKNOWN,
T-OBS, T-WEDGE, T-SLOTS, T-SELF and T-JOBCAP, each with its doc line. For
T-SLOTS, T-WEDGE and T-JOBCAP the listed properties bound the threat to
its documented residual; they do not remove it.

### 3.3 Trust assumptions (TCB)

The 15 assumptions, with their levels and what checks each, are the table
in §5.14 (`threat_model.pl`'s `assumption/3`). They are in `status.toml`
as `[assumptions.<ID>]`, next to the properties (§12). PRED, PRIVATE,
CALLBACK and ATOMIC (high) and INTERLEAVE (med, one shard) are tested on
the real runtime. GAS (med, new in v1.1.0: callers attach the prepaid gas
`api.md` documents) is measured by RES-1. COMPILE is checked by
`check-pin.sh`. TIME (high) and EXEC, OWNER and DEPLOY (policy) are
untested; the in-contract half of DEPLOY (no deploy, key or transfer
import, no upgrade export) is checked by SH-11 and SH-12. TOOLS, MODEL and
LEAN-TLA (med) rest on the layers' own controls. SPEC (low) rests on a
reviewer who has not been named.

---

## 4. Findings

The v1.0.0 verification (pin `a4bb5c3`, PR #31 with its results at
`a072a2d`; #31's head is to be tagged `shared-pool-v1.0.0`) found six
core findings (F-1..F-6, with F-2b) and four shell findings
(NT-F1..NT-F4); their full text is in that record, and upstream records
the fixes in `docs/verification-findings.md`.
v1.1.0 (upstream PR #4) closes each. A finding counts as closed here only
with (t) the v1.0.0 result showing it, and (r) a fix-revert mutant (§4.5)
killed by the closing property or (c) the corpus (§10: `coverage.py` and
the differential replay of the two corpora). Doc cites of text v1.1.0
deleted are written `spec@v1.0.0:<line>`; hunk lines are
`crates/core/src/lib.rs` at the pin. Severity rates the harm to the
property the docs promise. The re-verification found no new finding
(§4.7). Three behaviours are documented limits, not findings (§4.6).

### 4.1 Closed in v1.1.0: core

| ID (sev) | v1.0.0 finding | Upstream fix (commit; hunk at the pin) | Closed by | Regression (v1.0.0 fails; v1.1.0 passes) |
|---|---|---|---|---|
| F-1 (high) | **Disturbed-state hedge wedge.** After an `observe` outside [min, max], a hedge whose reservation cured a metric and whose completion re-violated it was admitted; then both settles failed `Bounds` and `cancel` and `observe` failed `Pending`, forever (trace 0). Contradicted spec@v1.0.0:46 and handoff@v1.0.0:31 (`div-F1-inverse`). | `59e404f`: H5, the release-aware envelope (`:216-228`); H6, `check_bounds` against the violation now and after the reservation (`:258-261`) | LIVE-ALL, INV-RES, LIVE-2, TLA-LIVE-F1, LEAN-WIT, MC-1, RT-WEDGE | v1.0.0: trace 0 `ok(0)`, then both settles `Bounds`; TLC `Witness_Wedge` and `MC_Liveness_F1` violated; Lean `f1_forever`; MC-1 `wedge` 228 dead states; RT-WEDGE reproduced on chain. v1.1.0: trace 0's hedge `Bounds`; the recovery hedge admitted and settled (traces 214, 215; `WitStep_Recovery`; Lean `recovery_admitted`); `liveness-f1` holds; Lean `f1_rejected`; MC-1 `wedge` 0 dead; `rt_wedge_f1_refused_and_cure_settles`. FR-1, FR-2. |
| F-2 (high) | **Healthy-state release wedge in the `risk` fixture.** A hedge was admitted on the strength of an unexecuted position's reservation; the position's honest `settle(false)` then failed `Bounds`, and only a false `settle(true)` ended it (trace 209). Contradicted spec@v1.0.0:46 (`div-F2-fixture-release`). **F-2b:** a release could leave ±1,000,000 and fail `Arithmetic` (trace 208). | `59e404f`: H5 (the release term `x = −reserve·u`, each endpoint within ±10^6), H6 | LIVE-ALL, INV-RES, LIVE-2, TLA-LIVE-F2, C-5, MC-1; NUM-2 (F-2b) | v1.0.0: C-5 20 counterexamples in `risk`; MC-1 `reliance` 150 release-blocked states; `MC_Liveness_F2` violated; trace 208's release `Arithmetic`. v1.1.0: trace 209's hedge refused (`test_model`: 7 units `Bounds`, 6 Ok, then the position's release Ok); C-5 0 blocked in every fixture; MC-1 `reliance` 0 blocked, `f2b` refuses at admission; `liveness-f2` holds; Lean `f2_rejected`; `rt_wedge_f2_honest_release_after_the_hedge`. FR-2 (F-2b: t34, m23), FR-1. |
| F-3 (low) | **Lifetime slot exhaustion.** 256 lifetime records, then `Full` for every actor, forever (trace 207). handoff@v1.0.0:34: "256 lifetime records bound ordinary growth". | `1f01d43`: H2, H4, H7, H8, H10, H11, H14 (`Full` only with an open front, `:287`; prune and `first_id + 1`, `:370`; prerequisites read `completed_units`, `:301-305`, `:519`); the executor window, `4d69f19` | WIN-1, WIN-2, WIN-ESC, WIN-LIVE, DEP-1, INV-DEP, RT-WINDOW; executor: EX-6, EX-7, EX-8, TLA-EXEC, RT-D1 | v1.0.0: trace 207 ends `full` twice; TLC `WinLive` violated on the v1.0.0 model (`a072a2d`); the v1.0.0 executor (s00b) fails "full" at job 257. v1.1.0: trace 207 prunes three records (`ok(256)`, `ok(257)` on a pruned prerequisite, `Full` on an open front, `NotFound` on id 0); `liveness-win` and `liveness-win2` hold; MC-1 `window`, `window1`; Lean `win_esc`, `dep1_iff`; RT-WINDOW (300 records, `first_id` 44); RT-D1. FR-3, FR-4, FR-13, FR-14. |
| F-4 (low) | **A principal could attest its own work.** `validate` accepted a recipe whose actor is its executor (trace 210). Contradicted spec@v1.0.0:64 and spec@v1.0.0:50 (`amb-self-executor`). | `dc774f8`: H3 (`:130`) | CFG-2, C-5, AUTH-1 | v1.0.0: trace 210 accepted; `code_role_checks(validate, [])`. v1.1.0: trace 210 is `init_error(invalid_config)` in the core and every replayer; Lean `self_executor_invalid`; C-5 CASH-SELF-ENFORCED passes. FR-5. Residual: the owner may be an executor (§4.6). |
| F-5 (low) | **Version saturation.** At version u32::MAX every transition failed `Arithmetic`, settling in-flight records included (handoff@v1.0.0:34). | `da43188`: H1 (`VERSION_CAP`), H9 (the cap is `candidate`'s last check), H12 (`bump_new` in dispatch, set_paused and observe; `:401`, `:319-321`, `:526`) | VER-CAP, INV-VERCAP, TLA-CAP, LIVE-ALL, C-4 | v1.0.0: Prolog "F-5 witness"; Lean mutant 04; Kani `h_version_max_*`. v1.1.0: Lean `cap_reach`, `cap_instance`, `cap_drain`; TLC `MC_Cap` (the first TLC run to reach a cap); MC-1 `cap`; `test_model` "F-5 fixed" at 4,294,966,527; RT-REVERT at a patched `VERSION_CAP`; COV-2 at the 12 reads states at or above the cap. FR-6, FR-6b. |
| F-6 (low) | **An open record blocked the only snapshot refresh.** `observe` was `Pending` with any record open, so with `max_age > 0` one long-open record froze admission (trace 211; spec@v1.0.0:19, 37, 54). | `3eaf1cb`: H15 (`:538-540`) | OBS-2, REFRESH-1, OBS-1 | v1.0.0: trace 211 `Pending`; TLC REFRESH-1 violated on `a072a2d`. v1.1.0: trace 211's refresh Ok with record 0 Active, then submit Ok; Lean `refresh_ok`, `refresh_instance`; TLC REFRESH-1 and `WitStep_Refresh`; MC-1 REFRESH-1 and OBS-2 on every state and edge. FR-7. |

### 4.2 Closed in v1.1.0: shell, build and runtime

| ID (sev) | v1.0.0 finding | Upstream fix | Closed by | Regression (v1.0.0 fails; v1.1.0 passes) |
|---|---|---|---|---|
| NT-F1 = SB-1 (medium; diagnostics, not safety) | The documented build omitted `--cfg near`; near-sdk then dropped every panic message (a bare `unreachable` trap), so a caller could not tell `Pending` from `Bounds` (api@v1.0.0:40). | `effd174`: `scripts/build.sh` sets `RUSTFLAGS="--cfg near"` and refuses a wasm that does not import `panic_utf8` | SH-5, SH-11 | v1.0.0: `nt_f1_upstream_build_drops_every_panic_message`; sandbox `sb1_upstream_build_loses_panic_messages`. v1.1.0: `nt_f1_documented_build_reports_every_panic_message`; sandbox `sb1_documented_build_reports_panic_messages`; SURF-3: build.sh's own checker refuses the no-cfg wasm, which is FR-12, the negative control. |
| NT-F2 (low) | `reconcile` (`&self`) accepted and kept an attached deposit (api@v1.0.0:3). | `1d441c5`: an explicit deposit check | SH-10 | v1.0.0: `nt_f2_reconcile_accepts_and_keeps_an_attached_deposit`. v1.1.0: `nt_f2_reconcile_rejects_an_attached_deposit`; on chain a 1-yocto `reconcile` fails "Method reconcile doesn't accept deposit" (RT-REVERT). FR-9. |
| NT-F3 (info) | Both contracts imported 15 host functions that deploy, delete accounts, add keys, transfer, stake, or use gas keys or global contracts (near-sdk's `Promise`), so the import surface could not rule out an escape hatch. | `3598902`: only `promise_create`, `promise_then` and `promise_return`; build.sh's import allowlist | SH-12, SH-11 | v1.0.0: SURF-2 expected-fail with 15 forbidden imports. v1.1.0: SURF-2 passes; each wasm's imports equal build.sh's `ALLOWED` (contract 18, executor 16). FR-11. |
| NT-F4 (low) | A promise result that is not a JSON string made `#[callback_result]` panic; the record stayed where it was instead of becoming Unknown (api@v1.0.0:34). | `1d441c5`: `on_result` reads the raw promise result | SH-3, RT-CALLBACK | v1.0.0: `sh3_non_string_promise_results_fail_the_callback`. v1.1.0: `sh3_non_string_promise_results_become_unknown`; on chain `rt_callback_non_string_executor_results`. FR-10. |

### 4.3 Docs vs code: resolved (`138ef98`)

| ID | v1.0.0 | v1.1.0 |
|---|---|---|
| `div-ack-noop-version` | spec@v1.0.0:39 listed Dispatched/Unknown → Unknown, and api@v1.0.0:26 said every successful transition changes the version; the code returned Ok with no bump. | The docs follow the code: an acknowledgment that would not change the status is an Ok no-op without a version change (spec:41; api:30). C-3 reports no divergence; TLA+ mutant 07 and Prolog m02 pin the behaviour. |
| `div-cancel-terminal-name` | spec@v1.0.0:43, api@v1.0.0:15: cancelling a Completed record was rejected with `Pending`. | The code follows the docs: `InvalidState` (lib.rs:467-469; spec:43; api:17). TERM-1, TX-CANCEL; FR-8. |
| `div-F1-inverse`, `div-F2-fixture-release` | F-1 and F-2 against spec@v1.0.0:46 and handoff@v1.0.0:31. | Closed with F-1 and F-2: admission refuses any commitment whose outcomes could make a metric worse than now, so the checked inverse cannot fail in a reachable state (spec:48; handoff:34). |

Where the docs are silent, the Prolog model follows the code and records
the gap (`ambiguity/4`). Seven remain: the violation amount of an interval
(spec:23); the freshness boundary (spec:21, 39); the recovery error's name
(spec:25); acknowledgment from Reserved or Active (spec:40-41, 46; api:30);
whether `reconcile` mutates (api:15; api:5); the observe vector length
(spec:58); and whether the owner may be an executor (spec:11; api:65). Two
of v1.0.0's are resolved by v1.1.0's docs: what a state transition is
(api:30) and actor = executor (spec:11; api:65).

### 4.4 Observations, closed or unchanged

- **Preview at u32::MAX** (v1.0.0: `preview` said allowed while `submit`
  failed `Arithmetic`). Closed: the cap is `candidate`'s last check (H9),
  so `preview.allowed ⇔ submit Ok` with no version premise (RD-PREVIEW;
  COV-2 at the reads states at or above the cap; FR-6b).
- **`value_return("null")` before `promise_return`** in `dispatch` and
  `reconcile`. Closed: both return `()` (`3598902`); SH-6 and SH-7 check
  the promise shape.
- **Prepaid gas** (v1.0.0 OQ-9: `api.md` gave none). Closed: api:44-59
  gives a table per call, and RES-1 reproduces it exactly: `dispatch` 70,
  `reconcile` 46, `confirm_not_executed` and `finish` 55 (56 at 512 jobs),
  largest receipt 14.81 Tgas (§9.2). v1.0.0's 43 Tgas for `finish` is 55
  at v1.1.0, which attaches 40 Tgas to the lock's `settle` and 10 to
  `on_settled`.
- **`reconcile` churn.** Unchanged: anyone's `reconcile` bumps a record's
  version at most twice (T-RECONCILE).
- **A rejected `submit` rewrites `STATE` with the same bytes** (gas only):
  a v1.0.0 observation, not re-examined at v1.1.0.
- **Test harness, not the contract:** near-workspaces 0.23's
  `TransactionStatus` await can return before detached callbacks (the
  executor's `on_settled`) run, so a sandbox test that checks the receipts
  of an asynchronous `finish` or `confirm_not_executed` must wait for them
  (`sandbox/RESULTS.md`).

### 4.5 Fix-revert mutants (FR-n): each v1.0.0 finding shown fixed

Each FR mutant reverts one upstream fix hunk on top of v1.1.0. The FR ids
are shared; each layer keeps its own patch numbers and records the FR id
in the mutant header and in its `RESULTS.md`. Mutant names: `tNN` TLA+,
`lNN` Lean, `mNN` Prolog model, `cNN` Prolog code (through `extract.py`),
`NN` Kani (the plan's N1-N14 are Kani patches 32-45), and `sNN`
near-tests (`s01`-`s21` the v1.0.0 near-tests mutants re-based, `s22`-`s38`
new, `s32` split into `s32a` in `execute` and `s32b` in
`confirm_not_executed`; `s00a` is the v1.0.0 contract and `s00b` the
v1.0.0 executor over the v1.1.0 core). Hunks `H1`-`H15` are the core fix
hunks; lines are `crates/core/src/lib.rs` at the pin. Every TLA+, Lean,
Prolog and near-tests mutant below is killed at v1.1.0 (§17, §18, §8,
§9.1). The Kani column gives each patch's v1.1.0 Kani verdict
(`kani/RESULTS.md`, Mutants); plain mode kills every one (CI's `mutants`
job).

| FR | Reverts | Finding | Must be killed by | TLA+ | Lean | Prolog | Kani | near-tests, sandbox, other | v1.0.0 evidence (`a072a2d`) |
|---|---|---|---|---|---|---|---|---|---|
| FR-1 | H6 (`59e404f`, `:258-261`): `check_bounds` compares with the violation now only (`v > old[i]`) | F-1 | LIVE-ALL, INV-RES | t33 | l18 | m22 | N2 `33-check-bounds-old-only` (Kani: not run at v1.1.0, `not-run.tsv`) | C-5; TLC `liveness-f1` | `Witness_Wedge` reachable, `f1_forever`, MC-1 228 dead, `h_wedge` cover |
| FR-2 | H5 (`59e404f`, `:216-228`): the envelope drops the release term `x` (F-2b: `t34`, `m23` drop only its ±10^6 bound) | F-1, F-2, F-2b | LIVE-ALL, INV-RES | t32; t34 | l17 | m21; m23 | N1 `32-envelope-no-release-term` (Kani: not run at v1.1.0, `not-run.tsv`) | C-5 (0 counterexamples at v1.1.0); traces 208, 209; Kani `h_wedge_healthy` (the position releases) | C-5 20 counterexamples, MC-1 150 blocked, trace 208 |
| FR-3 | H7 (`1f01d43`, `:287`): `Full` whenever 256 records are retained; prune without `first_id + 1` (`:370`) | F-3 | WIN-1, WIN-2, WIN-ESC, WIN-LIVE | t35, t36, t38 | l19, l20 | m24, m25, c11 | N5 `36-full-ignores-front` (Kani: not run at v1.1.0, `not-run.tsv`), N11 `42-prune-keeps-first-id` (Kani: not run at v1.1.0, `not-run.tsv`), N12 `43-index-no-filter` (Kani: CI's manual `kani-mutants`); re-based 21, 30 | trace 207 replay; s26 (`get_commitments` skips `from`), s27 (dispatch indexes `records[id]`); RT-WINDOW | trace 207 `full`; `WinLive` violated on the v1.0.0 model |
| FR-4 | H8 (`1f01d43`, `:301-305`, `:519`): prerequisites scan the retained records instead of `completed_units` | F-3 | DEP-1, INV-DEP, WIN-1 | t37 | l21 | m26 | N13 `44-prereq-scan-retained` (Kani: not run at v1.1.0, `not-run.tsv`); N9 `40-settle-no-completed-units` (Kani: not run at v1.1.0, `not-run.tsv`), N10 `41-completed-units-wrap` (Kani: not run at v1.1.0, `not-run.tsv`) | trace 207 (`submit:prereq_after_prune`) | none (v1.0.0 never pruned) |
| FR-5 | H3 (`dc774f8`, `:130`): `validate` accepts actor = executor | F-4 | CFG-2 | t39 | l22 | c10 | N6 `37-validate-actor-is-executor` (Kani: not run at v1.1.0, `not-run.tsv`) | trace 210 `init_error` | trace 210 accepted; `code_role_checks(validate, [])` |
| FR-6 | H12 (`da43188`, `:401`, `:319-321`, `:526`): `bump_new` → `bump`, no cap in `candidate` | F-5 | VER-CAP, INV-VERCAP | t40-t44 | l23 (l04′) | m27, m28, c12 | N3 `34-pause-without-cap` (Kani: CI's manual `kani-mutants`), N4 `35-candidate-without-cap` (Kani: not run at v1.1.0, `not-run.tsv`); P2 `dispatch-plain-bump`, `observe-plain-bump` | TLC `MC_Cap` (TLA-CAP) | Prolog "F-5 witness"; Lean mutant 04; Kani `h_version_max_*` |
| FR-6b | H9 (`da43188`): the cap check moved from `candidate` into `submit` | preview at u32::MAX | RD-PREVIEW | none | none | none | N14 `45-cap-in-submit-only` (Kani: not run at v1.1.0, `not-run.tsv`) | COV-2 reads at the cap; Kani `h_preview` | reads generator disagrees at u32::MAX |
| FR-7 | H15 (`3eaf1cb`, `:538-540`): observe `Pending` whenever a record is open | F-6 | OBS-2, REFRESH-1 | t45, t46, t47 | l24, l26 (l01′) | m29, m30, m31 | N7 `38-observe-no-refresh` (Kani: CI's manual `kani-mutants`) | trace 211 | trace 211 `Pending`; TLC REFRESH-1 violated |
| FR-8 | H13 (`138ef98`, `:467-469`): cancel of Completed gives `Pending` | `div-cancel-terminal-name` | TERM-1, TX-CANCEL | t48 | l25 | m32 | N8 `39-cancel-completed-pending` (Kani: CI's manual `kani-mutants`) | the 18 traces with cancel(Completed) | 18 traces `Pending` |
| FR-9 | contract (`1d441c5`): `reconcile` without its deposit check | NT-F2 | SH-10 | none | none | none | none | s22, s00a | `nt_f2_*` keeps 1 yocto |
| FR-10 | contract (`1d441c5`): back to `#[callback_result]` | NT-F4 | SH-3, RT-CALLBACK | none | none | none | none | s23, s24, s25, s00a | callback panics on a non-string result |
| FR-11 | contract and executor (`3598902`): near-sdk's `Promise` API | NT-F3 | SH-12 | none | none | none | none | s28, s35, s00a, s00b (their build imports 19 `promise_batch_*` functions per wasm; SURF-2 fails) | 15 forbidden imports |
| FR-12 | build (`effd174`): no `--cfg near` | NT-F1 | SH-5 | none | none | none | none | the no-cfg control build, refused by SURF-3(b) | bare `unreachable` trap |
| FR-13 | executor (`4d69f19`): `on_settled` does not advance `newest` | F-3 (executor), D-1 | EX-6, EX-7, EX-8, TLA-EXEC, RT-D1 | t53 (t50: `on_settled` advances whatever settle returned) | none | none | none | s38 (never advances), s29 (advances on `Failed`), s30 (not private), s34, s36, s37, s00b; RT-D1 | v1.0.0 executor `full` at job 257 |
| FR-14 | executor (`4d69f19`): no `"cancelled"` early return for a forgotten id | F-3 (executor) | EX-6 | t51 (t49, t52: the window's other guards) | none | none | none | s31, s32a, s32b, s33, s00b | none (v1.0.0 kept every job) |

Not counted as FR mutants but run with them: TLA+ t54 (the lock answers Ok
to a settle of a pruned id), the Lean probes (`mutants/probes/`, one per
mutant 17-26, each showing the named property fails on the mutant), and
the differential replay of the two corpora through the v1.1.0 Prolog
model (§10.1: 44 traces differ, each at the call a fix changes).

### 4.6 Documented limits (not findings)

Upstream documents each; the layers reach each and record it.

| Limit | What happens | Docs | Evidence | Assumption |
|---|---|---|---|---|
| Head-of-line `Full` | With 256 records retained and the oldest open, `submit` fails `Full` even if the other 255 are terminal. A Reserved front can be cancelled by its actor or the owner; a dispatched one closes only when its executor reports. | api:28; handoff:38 | WIN-ESC (one call by the front's resolver frees the window); WIN-LIVE (under weak fairness of that resolver); TLC `Witness_FullOpenFront`; MC-1 `window` (`Full` on an open front: 3,344 edges); trace 207; RES-1 (submit #257 prunes id 0; `Full` only with the front open, submit #288) | ASM-EXEC |
| Executor job cap through refused confirmations (upstream D-1) | The synthetic executor holds at most 512 jobs. Only confirmations the lock refused (operator error: an unassigned or pruned id, a record still Reserved, the other outcome recorded, another executor's recipe) or whose `on_settled` has not run can fill it. At the cap `confirm_not_executed` fails "full", and so does an `execute` that frees no job, whose record then stays Unknown. The exits are a `settle` signed with the executor account's own key, or an accepted `finish(id, false)` of a refused job whose record the lock holds cancelled or open. LIVE still holds at the lock. | api:42; handoff:39 | TLA-EXEC: `Witness_JobCapExec`, `Witness_JobCapConfirm`, `WitStep_ExecFull`; `WitStep_CapExit` shows an exit is reachable, not that one always exists (safety only); `InvCapShape`: the cap needs at least WINDOW refused or in-flight confirmations. RT-D1: 300 confirmed failed executes keep at most 256 jobs, and a refused confirmation keeps its job and moves nothing. RES-1: `finish` at the 512-job cap | ASM-EXEC; upstream follow-up OQ-12 |
| The owner as executor | `validate` refuses actor = executor only. The owner may be a recipe's executor, and then settles work it can also pause and observe around. | spec:11; api:65 | CFG-2; C-5 CASH-SELF-ENFORCED; trace 213 (the owner as executor settles); Lean `owner_executor_valid`; `amb-owner-roles` | ASM-OWNER |

### 4.7 Open (v1.1.0)

New findings are numbered from F-7 (core) and NT-F5 (shell, build,
runtime); ids are never reused. None so far: at v1.1.0 no layer that has
run disagrees with the code or the docs beyond the limits of §4.6. Kani's
v1.1.0 run is still to be recorded (§6).

### 4.8 Rejected

| ID | Claim | Why rejected |
|---|---|---|
| L-1b | In a disturbed state, a reservation that cured the violation cannot be cancelled by the actor or the owner, and the executor's `settle(false)` fails `Bounds`. | Rejected at v1.0.0 as documented behaviour (spec@v1.0.0:41-42 made cancel and nonexecution a checked inverse; spec@v1.0.0:46: no forced administrative release). Moot at v1.1.0: a reservation never changes a metric's violation amount (handoff:32), and LIVE-ALL makes the cancel and both settles Ok in every reachable state. |

---

## 5. Property catalogue

The IDs below are exactly the 118 `property/3` IDs of
`prolog/requirements/traceability.pl`: 103 properties and 15 assumptions.
C-6 (`prolog/verification/consistency.pl`) compares `traceability.pl`
with this section, not with the brief: the IDs are the first cell of every
table row between this heading and the next `## ` heading, and each row's
*Layer(s)* must equal `property/3`'s layer list, in order. None may be
missing, extra or repeated on either side. A missing file, a missing
section or an empty ID set fails C-6. The `prolog` CI job runs it.
status-lint (`check-status.sh`, the `status-lint` job) reads the same rows
and compares `status.toml` with them (§12, "Catalogue"): the IDs, the
modules, `layer` and `cross` against *Layer(s)*, and each `status` against
the row's *Status* cell. Every table row in this section starts with a
bare ID, and the section has no other tables.

**v1.1.0.** The IDs and layer lists were frozen at the start of the
v1.1.0 re-verification. The 83 properties and 14 assumptions of v1.0.0
keep their IDs; 20 properties (VER-CAP, OBS-2, WIN-2, INV-RES,
INV-VERCAP, WIN-1, LIVE-ALL, WIN-ESC, REFRESH-1, WIN-LIVE, SH-12, SCN-1,
EX-6, EX-7, EX-8, RT-WINDOW, RT-D1, TLA-CAP, TLA-EXEC, LEAN-LIVE-ALL) and
ASM-GAS are new. A witness that guarded a v1.0.0 finding is inverted in
place and keeps its ID (§12). CFG-2 now includes actor ≠ executor (there
is no CFG-3); `AUTH-SELF` is a theorem name, not an ID. INV-HARD is a
corollary of INV-RES and `PV(values) ≤ B` (Kani's INV-PV is tagged
INV-HARD), and INV-HEALTHY is restated over every combination of
outcomes. Kani's INV-CU is tagged WIN-1. SH-11 is the exports only;
SURF-3 is tagged SH-5 and SH-12. The fix-revert mutants FR-n are in §4.5.
Every *Evidence* and *Status* cell is the v1.1.0 result at the pin,
Kani's included (§6: CI's `kani` groups and the local record at
`2d33d33`).

**Notation (v1.1.0; every model and `kani/src/spec.rs` uses it).**
- **Point violation:** `PV_i(v) = max(0, min_i − v_i) + max(0, v_i − max_i)`.
- **Open and in flight:** a record is *open* unless Completed or
  Cancelled. It is *in flight* when Dispatched, Active or Unknown.
- **Ids and the window:** `next = first_id + len(records)`; record `k`
  has id `first_id + k`; an id is *retained* iff `first_id ≤ id < next`.
  A submit issues id `next`. At 256 retained records it prunes
  `records[0]` (which must be terminal) and increments `first_id`;
  `Full` iff 256 are retained and `records[0]` is open. A pruned or
  unissued id is `NotFound`.
- **Envelope (release-aware):** over the open records (optionally
  skipping one and adding one), with `d = complete·u` and
  `x = −reserve·u`: `low_i = v_i + Σ min(0, d_i, x_i)` and
  `high_i = v_i + Σ max(0, d_i, x_i)`. It is computable iff every
  partial sum stays within ±1,000,000. Its violation is
  `EV_i = max(0, min_i − low_i) + max(0, high_i − max_i)`. Admission
  requires `EV_i ≤ min(PV_i(values), PV_i(candidate))`.
- **Completed units:** `CU_r = completed_units[r] = min(u32::MAX,
  lifetime Completed units of recipe r)`, pruned records included
  (`saturating_add` in `settle(true)`). Prerequisites read `CU`.
- **Version cap:** `VERSION_CAP = u32::MAX − 3·MAX_RECORDS` (= 4,294,966,527).
  `bump_new` (fails `Arithmetic` at or above the cap) is used by
  `dispatch`, `set_paused` and `observe`, and `submit`'s `candidate`
  checks the cap last; `acknowledge`, `settle` and `cancel` use the
  plain `bump`. `rem(status)`: Reserved 3, Dispatched 3, Unknown 2,
  Active 1, terminal 0.
- **Observe:** `Pending` only if the new values differ from the current
  ones and some record is open. With records open and equal values it is
  a *refresh*: only `observed_at` and the version change.
- **Ghosts:** `base` and `k0` (`k0` is an id). `new` sets `base = values`,
  `k0 = 0`. On observe Ok: keep them iff the new values equal the current
  values and some record is open (the refresh); otherwise re-base:
  `base = values`, `k0 = next`, pruned sum 0. TLA+ and Prolog fold a
  pruned record's contribution into `base`; Lean keeps the full history;
  Kani keeps a summary (pruned, done). `B_i = PV_i(base)`, and *healthy*
  means B = 0.

**Columns.** *Layer(s)* is `property/3`'s layer list. *Evidence* names
the artifact. Its §12 kind follows from the layer: Lean is model-proof, TLC
and MC-1 model-check, replays and tests test, Prolog C-* and the wasm
surface (SH-11, SH-12) check, and Kani bounded-proof. *Status* values:
- `proved (model)`: a Lean theorem about the transcription, at every size.
- `model-checked`: TLC or MC-1, within bounds. `tested`: on the real code
  (the traces, the shell, the chain).
- `checked`: a Prolog consistency check, or the comparison of the built
  wasm with `wasm-surface.expected` and build.sh's allowlist (SH-11,
  SH-12).
- `bounded proof (Kani)`: a Kani proof on the real core for every input
  within the harnesses' bounds (§6), and nothing outside them.
- `(v1.0.0: ...)`: what changed from v1.0.0, where it did (a finding fixed,
  a witness inverted).

No row is verified (§12).

### 5.1 Configuration and numerics

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| CFG-1 | `Pool::new` is Ok iff `validate` is Ok, and the initial state (`first_id = 0`, `completed_units = 0`) satisfies INV | kani, lean | Kani `h_init`, `h_validate`, `h_init_inv`, `h_init_inv_2` (a composition, §6); Lean `inv_init` (`Valid cfg ⇒ Inv (init cfg now)`; strings not modelled); TLC TraceCheck on the configs the core accepted, and trace 210's refused config fails `ValidCfg` | proved (model) without the string checks; tested (216 traces); bounded proof (Kani, a composition) |
| CFG-2 | `validate` ⇒ references in range, vectors of length n, prerequisites strictly earlier and distinct, values bounded, and every recipe's actor differs from its executor (v1.1.0, F-4; the owner may be an executor) | kani, lean | Kani `h_validate` (raw configurations, §6); Lean `valid_iff` (`validate` decides `Valid`), `valid_actor_ne_executor`, `self_executor_invalid`, `owner_executor_valid`; trace 210 (`init_error(invalid_config)`) refused by TLC `ValidCfg`, Lean, Prolog `valid_config/1` and the shell; C-5 CASH-SELF-ENFORCED; FR-5 (t39, l22, c10) | proved (model); tested (trace 210); bounded proof (Kani) |
| NUM-1 | Valid config and bounded values ⇒ `energy_terms` and `energy` are Ok and exact | kani, lean | Kani `h_energy` (any value in ±1,000,000), `h_energy_exact`, `h_energy_max` (64 terms, 2.56·10^17 exact); Lean computes exactly in `Int` and does not model overflow; TLC cannot represent large energies (§17.5) | bounded proof (Kani) |
| NUM-2 | `add` fails closed exactly when the exact result leaves ±1,000,000; so does each envelope endpoint (completion and release terms); from an INV state no release or completion `add` fails (INV-RES) | kani, lean | Kani `h_step_submit_tx`, `h_step_cancel_tx`, `h_step_settle_tx` (every `Arithmetic` exit of `add`, values at both extremes); trace 206: `arithmetic` from `add` at 999,990 + 15 and from the envelope at 1,000,005; F-2b's endpoint refused at admission (trace 208, MC-1 `f2b`, TLC `Witness_F2bRefused`); TLA+ mutants 23 and 24 | boundary tested (216 traces); bounded proof (Kani) |
| ROB-1 | No core panic from any INV state, for any arguments | kani | Kani: every harness that calls the core (panic, overflow, bounds and unwinding checks); Lean shows every index the guards read is in range (INV-WF), but does not model panics | bounded proof (Kani) |

### 5.2 Transition contracts

Each contract gives the exact result (the first failing guard, in the
code's order), the exact post-state, and the frame.

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| TX-0 | Every `Err` leaves the Pool unchanged | kani, tla, lean, prolog | Kani `h_step_<op>_tx`, `h_wf_<op>`, `h_version_cap_<op>`, `h_version_max_<op>`, `h_full_*`; Lean `stepProps` (every call, every state); TLC TraceCheck; MC-2 | proved (model); tested on 216 traces; bounded proof (Kani) |
| TX-SUBMIT | `submit`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_submit_tx`, `_post`, `h_wf_submit`, `h_full_255`, `h_full_256`, `h_full_256_prune`; REF-5, REF-5L, MC-2 on every replayed submit (trace 207: the prune, `Full` on an open front); TLC `StepProperty` (time, capacity and unit guards, effect); C-2 guard order | tested on 216 traces; model-checked in part; bounded proof (Kani) |
| TX-DISPATCH | `dispatch`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_dispatch_tx`, `_post`, `h_wf_dispatch`; REF-5, REF-5L, MC-2; TLC `StepProperty`; C-2 | tested on 216 traces; model-checked in part; bounded proof (Kani) |
| TX-ACK | `acknowledge`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_ack_tx`, `_post`, `h_wf_ack`; REF-5, REF-5L, MC-2; C-2, C-3 | tested on 216 traces; bounded proof (Kani) |
| TX-CANCEL | `cancel`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_cancel_tx`, `_post`, `h_wf_cancel`; REF-5, REF-5L, MC-2 (cancel of a Completed record is `InvalidState` in 18 traces); C-2; FR-8 (t48, l25, m32) | tested on 216 traces; bounded proof (Kani) |
| TX-SETTLE | `settle`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_settle_tx`, `_post`, `h_wf_settle`; REF-5, REF-5L, MC-2; C-2 | tested on 216 traces; bounded proof (Kani) |
| TX-PAUSE | `set_paused`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_pause_tx`, `_post`; REF-5, REF-5L, MC-2; C-2 | tested on 216 traces; bounded proof (Kani) |
| TX-OBSERVE | `observe`: exact result in guard order, exact post-state, frame | kani, tla, lean, prolog | Kani `h_step_observe_tx`, `_post`; REF-5, REF-5L, MC-2; C-2 | tested on 216 traces; bounded proof (Kani) |
| TX-NOOP | Idempotent Ok paths change nothing, version included | kani, tla, lean, prolog | Kani `h_step_<op>_tx`, `h_wf_<op>`, `h_version_max_<op>` (ack, cancel, settle at u32::MAX); Lean `stepProps`; TLC TraceCheck; MC-2 | proved (model); tested; bounded proof (Kani) |
| TX-VER | Every other Ok increments the version by exactly 1 | kani, tla, lean, prolog | Kani `h_step_<op>_tx`, `h_version_cap_<op>`, `h_version_max_<op>` (ack, cancel, settle at u32::MAX); Lean `stepProps`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| VER-CAP | version ≥ `VERSION_CAP` (u32::MAX − 768) ⇒ `submit` (the last check of `candidate`), `dispatch`, `set_paused` and `observe` fail `Arithmetic` at their guard positions and change nothing (`bump_new`); `acknowledge`, `settle` and `cancel` use the plain increment (`bump`) and stay available | kani, tla, lean, prolog | Lean `cap_stops`, `cap_reach`, `cap_instance`, `cap_drain`; TLC `StepProperty` in `MC_Cap` (TLA-CAP), `Witness_AtCap`, `WitStep_ResolveAtCap`; Prolog `test_model` "F-5 fixed" at the real cap, MC-1 `cap` (submit, dispatch, set_paused and observe stop, ack, cancel and settle stay Ok); COV-2 at the 12 reads states at or above the cap; RT-REVERT at a patched `VERSION_CAP`; FR-6 (t40-t44, l23, m27, m28, c12), FR-6b; Kani `h_version_cap_<op>` (7, INV states at or above the cap), `h_version_max_<op>` (ack, cancel, settle at u32::MAX) | proved (model); model-checked (scaled cap); tested; bounded proof (Kani) |

### 5.3 Step properties

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| AUTH-1 | Effective submit and dispatch ⇒ caller = actor; cancel ⇒ actor or owner; settle ⇒ executor; pause and observe ⇒ owner | kani, tla, lean, prolog | Kani `h_step_<op>_tx`; Lean `exec_auth` (every Ok, no-ops included); TLC `StepProperty`; MC-2; actor = executor refused by `validate` (CFG-2) | proved (model); model-checked; tested; bounded proof (Kani) (v1.0.0: F-4, actor = executor accepted) |
| STALE-1 | Submit Ok ⇒ request version = version; versions never repeat, so an accepted request cannot replay | kani, tla, lean, prolog | Kani `h_step_submit_tx`, `h_bmc_from_init_s_submit` (no replay along 2-operation traces); Lean `stale1_no_replay`, `version_mono_run`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| ADM-1 | Submit Ok ⇒ energy(post) ≤ energy(pre) | kani, tla, lean, prolog | Kani `h_step_submit_tx`; Lean `stepProps`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| ADM-2 | Submit Ok in a violating state ⇒ energy(post + completion) < energy(pre) | kani, tla, lean, prolog | Kani `h_step_submit_tx`; Lean `stepProps`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| HARD-STEP | Every Ok except observe: PV_i(post) ≤ PV_i(pre) for every metric | kani, tla, lean, prolog | Kani `h_step_<op>_post`; Lean `stepProps`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| DEP-1 | Submit Ok ⇒ prerequisites met by `completed_units` (lifetime Completed units, pruned records included) | kani, tla, lean, prolog | Kani `h_step_submit_tx`, `h_wf_submit`, `h_full_256_prune` (a pruned record's completions count); Lean `dep1`, `dep1_iff` (pruned completions count); TLC `StepProperty`, `WitStep_PrereqAfterPrune`; MC-1 `window` (a prerequisite admitted after its record was pruned); MC-2; trace 207; FR-4 (t37, l21, m26) | proved (model); model-checked; tested; bounded proof (Kani) |
| LIFE-1 | Status graph Reserved → Dispatched or Cancelled, Dispatched → Active or Unknown, Unknown → Active, in flight → Completed or Cancelled; terminals absorbing; a record is appended at id `next`; only the oldest record is pruned, and only when terminal; id, recipe, units and expiry immutable while retained | kani, tla, lean, prolog | Kani `h_step_<op>_post`, `h_full_256_prune`; Lean `stepProps`; TLC `StepProperty`; C-3; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| ASYNC-1 | Acknowledge never changes values; Active and Unknown keep the reservation | kani, tla, lean, prolog | Kani `h_step_<op>_post`; Lean `stepProps`; TLC `StepProperty`; MC-2; RT-CALLBACK | proved (model); model-checked; tested; bounded proof (Kani) |
| TERM-1 | The same terminal repeated is an Ok no-op; the opposite terminal is `InvalidState`, for settle and for cancel of a Completed record (v1.0.0 gave `Pending`, §4.3) | kani, tla, lean, prolog | Kani `h_step_<op>_post`; Lean `term1`, `term1_cancel`, `cancel_completed_instance`; TLC invariant (error view), `Witness_CancelCompletedInvalid`; MC-2; FR-8 | proved (model); model-checked; tested; bounded proof (Kani) |
| OBS-1 | Observe Ok ⇒ no open record, or the values are unchanged (the refresh, OBS-2); config unchanged | kani, tla, lean, prolog | Kani `h_step_observe_tx`; Lean `obs1`; TLC `StepProperty`; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| PAUSE-1 | Paused ⇒ submit and dispatch have no effect; settle, ack, cancel and observe are unaffected | kani, tla, lean, prolog | Kani `h_step_<op>_tx`, `_post`; Lean `pause1_blocks`, `pause1_transparent`; TLC `StepProperty` and the `WitStep_Paused*` witnesses; MC-2 | proved (model); model-checked; tested; bounded proof (Kani) |
| OBS-2 | Observe Ok with some record open ⇒ the new values equal the current values, and only `observed_at` and the version change; the ghosts `base` and `k0` are kept (the refresh, F-6) | kani, tla, lean, prolog | Lean `obs2`, `refresh_instance`; TLC `StepProperty` (OBS-2), `WitStep_Refresh`; Prolog OBS-2 on every MC-1 edge and MC-2 call; trace 211 and the other refresh traces; FR-7 (t45-t47, l24, l26, m29-m31); Kani `h_step_observe_tx`, `_post` | proved (model); model-checked; tested; bounded proof (Kani) |
| WIN-2 | A pruned record was terminal when it was pruned, and its id is `NotFound` for `dispatch`, `acknowledge`, `cancel`, `settle` and `record`, which change nothing; ids are never reused | kani, tla, lean, prolog | Lean `pruned_terminal`, `pruned_notFound`; TLC WIN-2 in `InvMC`, `Witness_Prune`; MC-1 `window`, `window1` (`NotFound` on the pruned id: 20,090 and 8,450 edges); MC-2; trace 207 (`NotFound` on pruned id 0); near-tests `sh9_pruned_ids_are_not_found_and_pages_are_by_id`; RT-WINDOW; FR-3 (t35, t36, t38, l19, l20, m24, m25, c11); Kani `h_full_256_prune` (the prune of a terminal front at 256 records), `h_step_<op>_post` (no prune below 256) | proved (model); model-checked; tested; bounded proof (Kani) |

### 5.4 Invariants

These hold in every reachable state. Lean proves their conjunction `Inv`
with `inv_init`, `inv_step` and `reach_inv`.

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| INV-WF | Values have length n and are bounded; `records[k].id = first_id + k` (WIN-1); recipe in range; 1 ≤ units ≤ max_units; at most 256 records; one `completed_units` entry per recipe; config never changes | tla, lean, kani, prolog | Lean `reach_inv`; TLC MC_Safety and every trace state; MC-1, MC-2; Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-LEDGER | values = base + Σ reservations of records with id ≥ k0 not Cancelled + Σ completions of Completed records with id ≥ k0, over every issued record (pruned ones through the ghost summary) | tla, lean, kani, prolog | Lean `reach_inv`; TLC; MC-1, MC-2; Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-HARD | The release-aware envelope over the open records is computable and EV_i ≤ B_i (a corollary of INV-RES and PV_i(values) ≤ B_i) | tla, lean, kani, prolog | Lean `reach_inv`; TLC; MC-1, MC-2; Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-HEALTHY | B = 0 ⇒ every combination of outcomes of the open records (stay open, complete, release) lands in bounds | tla, lean, kani, prolog | Lean `healthy`; TLC; MC-1, MC-2; Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-DEP | Every retained record's prerequisites are met by `completed_units` (monotone) | tla, lean, kani, prolog | Lean `reach_inv`; TLC; MC-1, MC-2; Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-VER | The version equals the number of effective transitions | tla, lean, kani, prolog | Lean `reach_inv`; TLC; MC-2 per edge; Kani `h_bmc_from_init_*` (2-operation traces from `Pool::new`) | proved (model); model-checked; tested; bounded proof (Kani, 2-operation traces) |
| INV-RES | The release-aware envelope over the open records (each may stay open, complete, or release its reservation) is computable, every endpoint within ±1,000,000, and `EV_i = PV_i(values)` for every metric (upstream `findings.rs` INV-RES) | tla, lean, kani, prolog | Lean `Inv` (`reach_inv`), `res_needed`, canary `LiveAllNoRes`; TLC `InvRes` in `InvMC` (MC_Safety, MC_Cap, MC_Recovery, MC_F2b); Prolog on every MC-1 state and every trace state (MC-2); FR-1 (t33, l18, m22), FR-2 (t32, t34, l17, m21, m23); Kani induction: `h_init_inv*`, `h_step_<op>_inv`, `_inv_0`, `_inv_1`, `_inv_3` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |
| INV-VERCAP | `version + Σ_open rem(status) ≤ u32::MAX`, with rem Reserved 3, Dispatched 3, Unknown 2, Active 1, terminal 0; so the plain increment of ack, settle and cancel never overflows | tla, lean, kani, prolog | Lean `Inv` (`reach_inv`), `remSum_le`, `bump_ok`, `cap_headroom`; TLC `InvAbs` in `MC_Cap` (TLA-CAP) and the liveness configs; MC-1 `cap`; Prolog on every trace state (MC-2); FR-6 (t40-t44, l23, m27, m28, c12); Kani induction (as INV-RES) and `h_version_cap_<op>`; plain mode `cap_walk` | proved (model); model-checked (scaled cap); tested; bounded proof (Kani, at most 3 records) |
| WIN-1 | Window structure: `records[k].id = first_id + k`, at most 256 records, `first_id + len ≤ version`, and `completed_units[r] = min(u32::MAX, lifetime Completed units of r)` (Kani's INV-CU is tagged WIN-1) | tla, lean, kani, prolog | Lean `win1`, `submit_prunes`; TLC `InvMC`, `InvAbs` (`firstId + NR <= version`) in `MC_Cap`, every TraceCheck state; MC-1 `window`, `window1`; MC-2; trace 207 (three prunes); FR-3, FR-4; Kani induction (as INV-RES; `win`, INV-CU), `h_full_255`, `h_full_256`, `h_full_256_prune` | proved (model); model-checked; tested; bounded proof (Kani, at most 3 records) |

### 5.5 Reads

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| RD-PREVIEW | `preview.allowed` ⇔ `candidate` Ok ⇔ submit Ok (the version cap is `candidate`'s last check, so no version premise); no mutation, no panic | kani, near-tests, prolog | Kani `h_preview`, `h_preview_post`; Prolog `test_reads` (COV-2): the real core's `preview` (allowed, reason, candidate, energies) on 39,627 requests at 1,300 trace states (12 at or above `VERSION_CAP`; `reads.txt`) equals the model's; the generator checks that `submit` agrees with each of those previews on the real core; Prolog `preview_allowed` over a request grid (model); near-tests: the shell's `preview` equals the core's after every SH-8 host call and at the window (`sh9_views_*`), the shell half; FR-6b | tested (shell half; core half by differential against the model); bounded proof (Kani, core half) |
| RD-BEST | `best_quantity` is sound and minimal as documented | kani, prolog | Kani `h_best` with `h_preview`, `h_step_submit_*` and `h_energy` (a composition, §6); Prolog `test_reads` (COV-2): the real core's `best_quantity` on the same 39,627 requests equals the model's `best_quantity/4`; Prolog `best_quantity` against brute force on small cases (model) | tested by differential against the model; bounded proof (Kani, a composition) |

### 5.6 Liveness

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| LIVE-1 | INV ⇒ `settle(executor, id, true)` is Ok for every Dispatched, Active or Unknown record (a corollary of LIVE-ALL; v1.0.0 needed B = 0 and version < u32::MAX) | lean, tla, prolog | Lean `live1`, `live1_exec`, `live1_reach` at every size, with no B or version premise; `regression/Live1NoPremise.lean` (v1.0.0's canary) now compiles; TLC invariant in `InvMC`; Prolog on every MC-1 state and trace state; Kani `h_live_all` (3 records; LIVE-1 is its `settle(true)` half) | proved (model); model-checked; tested; bounded proof (Kani, 3 records) |
| LIVE-2 | Every dispatched record eventually becomes terminal under executor fairness to either outcome, and in the F-2 universe under fairness of the honest `settle(false)` and of `cancel` | tla, prolog | TLA-LIVE-1, TLA-LIVE-F1 and TLA-LIVE-F2 hold (`liveness-unfair` violated, as required); MC-1: no dead or release-blocked record in any of the seven universes; traces 0 (F-1) and 209 (F-2) refuse the hedges, 214 and 215 settle the recovery hedge each way; near-tests `wedge.rs`; RT-WEDGE | model-checked (v1.0.0: fails: F-1, F-2) |
| LIVE-ALL | INV ⇒ for every open record: if Reserved, `cancel` by its actor and by the owner are Ok; if Dispatched, Unknown or Active, `settle(executor, id, true)` and `settle(executor, id, false)` are Ok; none raises a metric's violation. No premise on B or the version (upstream `findings.rs` `assert_live`) | lean, tla, kani, prolog | Lean `live_all`, `live_all_reach`, `no_wedge` (LEAN-LIVE-ALL); TLC `LiveAll` in `InvMC` (MC_Safety, MC_Cap, MC_Recovery, MC_F2b); Prolog on every MC-1 state and every trace state; traces 0, 208, 209, 214, 215; near-tests `wedge.rs` and RT-WEDGE at the shell; violated on the v1.0.0 model (`a072a2d`, 3 states); FR-1, FR-2; Kani `h_live_all` (3 records), `h_wedge` exit assertions, `h_wedge_healthy`, `h_version_cap_<op>` | proved (model); model-checked; tested; bounded proof (Kani) |
| WIN-ESC | `Full` ⇔ 256 records retained and `records[0]` open. From every INV state with 256 retained and an open front, one call by the front's resolver (LIVE-ALL) makes it terminal, after which a valid submit does not fail `Full` | lean, tla, kani, prolog | Lean `full_iff_front_open`, `submit_not_full_of_front_terminal`, `win_esc`; TLC WIN-ESC in `InvMC`, `Witness_FullOpenFront`; Prolog probe on MC-1 `window` and `window1` (`Full` on an open front: 3,344 and 2,120 edges) and MC-2; trace 207; FR-3; Kani `h_full_255`, `h_full_256`, `h_full_256_prune` | proved (model); model-checked; tested; bounded proof (Kani) |
| REFRESH-1 | Version below `VERSION_CAP` ⇒ the owner's `observe` of the current values is Ok in every reachable state, open records included (F-6 fixed) | lean, tla, kani, prolog | Lean `refresh_ok`, `refresh_instance`; TLC REFRESH-1 in `InvMC` (violated on the v1.0.0 model, 2 states); Prolog `test_model` "F-6 fixed" and a probe on every MC-1 state and trace state; trace 211; FR-7; Kani `h_step_observe_tx`, `h_wedge` | proved (model); model-checked; tested; bounded proof (Kani) |
| WIN-LIVE | `[](Full ⇒ <>¬Full)` under weak fairness of the front record's resolver (its cancel and settle) | tla | TLC `WinLive`: `liveness-win` (MaxRecords 1, 1,868 states) and `liveness-win2` (MaxRecords 2, 8,228 states) hold; violated on the v1.0.0 model (`a072a2d`, F-3); FR-3 | model-checked |

### 5.7 Contract shell

Mocked runtime (`near-tests/`), on the host and on the documented wasm
build (`scripts/build.sh`, `--cfg near`); the build without the cfg is
only NT-F1's negative control. SH-11 and SH-12 are the surface checks
(`near-tests/wasm-surface.sh`).

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| SH-1 | caller = predecessor; now = block time in whole seconds | near-tests | `sh1_caller_is_the_predecessor_not_the_signer`, `sh1_now_is_the_block_timestamp_in_whole_seconds`; 15% relayed calls in SH-8; mutants s02, s10 | tested |
| SH-2 | `new` is private and initializes once | near-tests | `sh2_new_is_private_and_initialises_once`; mutant s09 | tested |
| SH-3 | `on_result` is private; one JSON string result of at most 64 bytes is mapped as documented, and anything else (failed, missing, two results, not a string) is Unknown; an unknown or pruned id is logged with `NotFound` and changes nothing | near-tests | `sh3_on_result_is_private`, `sh3_on_result_maps_the_status_string`, `sh3_non_string_promise_results_become_unknown` (a failed call, no result, two results, more than 64 bytes, or anything but one JSON string), the `NotFound` event for a missing or pruned id (`sh9_pruned_*`); mutants s01, s03, s04, s08, s14, s23, s24, s25; FR-10 | tested (v1.0.0: NT-F4) |
| SH-4 | `submit` = core `preview` then `submit`; a rejected submit writes nothing | near-tests | `sh4_rejected_submit_returns_the_decision_and_writes_nothing`; the crafted allowed-but-failing submit in `sh5_core_errors_are_panics_with_the_error_string`; mutant s13 | tested |
| SH-5 | A core `Err` panics with exactly the error string and changes nothing, on `scripts/build.sh`'s output (`--cfg near`); the no-cfg control build must fail it (SURF-3) | near-tests | `sh5_core_errors_are_panics_with_the_error_string` on both paths (with a crafted state, `first_id = u32::MAX`, where the preview allows a submit the core refuses); `nt_f1_documented_build_reports_every_panic_message` (the no-cfg build, the negative control, still traps bare); SURF-3: build.sh refuses a wasm without `panic_utf8`; SH-8; mutants s07, s13, s14, s25; FR-12 | tested (v1.0.0: fails on the documented build: NT-F1) |
| SH-6 | `dispatch` promise shape: `promise_create` of `execute{id,recipe,units}` to the executor, 30 Tgas, then `promise_then(on_result, 30 Tgas)` and `promise_return`, no deposit | near-tests | `sh6_dispatch_promise_shape` (`promise_create` execute 30 Tgas, `promise_then` on_result 30 Tgas, `promise_return` of the callback); mutants s11, s27, s28 | tested |
| SH-7 | `reconcile` guard and promise shape: rejects a deposit, `NotFound` for an unknown or pruned id; `status{id}` 10 Tgas, then `on_result`, by anyone, no state write | near-tests | `sh7_reconcile_guard_and_promise_shape`; mutants s05, s15, s28 | tested |
| SH-8 | Borsh state round trip (with `first_id` and `completed_units`, windows past 256 records): the shell equals the core, with a reload before every call | near-tests | `sh8_store_diff_host` (2,000 × 40 calls), `sh8_store_diff_wasm` (300 × 40), `sh8_store_diff_window` (4 host and 1 wasm sequences of 1,500 submit-heavy calls past 256 records), `conformance_traces_through_the_shell` (216 traces, 8,865 calls, host and documented build) | tested |
| SH-9 | Views: `get_pool`; `get_commitments(from, limit)` with `from` an id (below `first_id` it starts at the oldest retained record), pages of at most 50 | near-tests | `sh9_views_and_paging_by_id_through_the_window` (300 submits, `first_id` 44), `sh9_pruned_ids_are_not_found_and_pages_are_by_id`; mutants s06, s26 | tested |
| SH-10 | Every mutating method, `reconcile` included, rejects deposits (NT-F2 fixed) | near-tests | `sh10_mutating_methods_reject_attached_deposits`, `nt_f2_reconcile_rejects_an_attached_deposit`; mutants s12, s22; FR-9 | tested (v1.0.0: NT-F2, `reconcile` kept deposits) |
| SH-11 | Export surface of `scripts/build.sh`'s wasm: `pool_contract` exports exactly the 12 documented methods plus `contract_source_metadata` and `memory`, and `pool_executor` its 8 methods (with `on_settled`) plus the same two; no upgrade, migrate or reset export | near-tests | `wasm-surface.sh` SURF-1 against `wasm-surface.expected`, regenerated for v1.1.0 (`run.sh surface`): contract 14 exports, executor 10 | checked |
| SH-12 | Import surface: both wasm files, as `scripts/build.sh` builds them, import exactly its allowlist; no deploy, key, transfer, stake, account, global-contract or `promise_batch_*` import (NT-F3 fixed) | near-tests | `wasm-surface.sh` SURF-2 (no forbidden import; passes, v1.0.0 expected-fail) and SURF-3(a) (build.sh run in the mirror; each wasm's imports equal its `ALLOWED`: contract 18, executor 16); s00a and s00b fail it; FR-11 | checked (v1.0.0: NT-F3) |
| SCN-1 | The published scenarios equal the core: for each of the six fixtures, re-deriving the scenario from `pool_core` (preview, submit, and when allowed dispatch, acknowledge and settle) reproduces `scenarios/<name>/trace.json` | near-tests | `scn1_published_scenarios_are_the_core` (`tests/scenarios.rs`; six scenarios, fixed epoch 1,794,441,600) | tested |

### 5.8 Executor

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| EX-1 | Only the lock executes | near-tests | `ex1_only_the_lock_executes`, `ex1_executor_wrappers`; mutant s17 | tested |
| EX-2 | Payload binding and dedupe: a replay returns the recorded status; another payload fails | near-tests | `ex2_payload_binding_and_dedupe` (a 512-job crafted state), the random differential; mutants s16, s21 | tested |
| EX-3 | Only the operator finishes, confirms or sets the mode | near-tests | `ex3_only_the_operator_attests`; mutant s18 | tested |
| EX-4 | A conflicting terminal outcome is rejected | near-tests | `ex4_terminal_conflict_rejected`; mutant s19 | tested |
| EX-5 | Notify calls `lock.settle{id,completed}` at 40 Tgas, then `on_settled{id}` at 10 Tgas | near-tests | `ex5_notify_calls_lock_settle`, end-to-end flows; mutants s20, s35 | tested |
| EX-6 | Executor window: `newest` is the largest id for which `execute` created a job or `on_settled` saw a successful settle; a job is kept iff `id + 256 > newest`; at most 512 jobs; `execute` of a forgotten id returns `"cancelled"` and stores nothing; `confirm_not_executed` of one fails `"stale id"`; `execute` of a stored id returns its status and moves nothing | near-tests | `ex6_window_forgets_and_caps`, `ex6_window_constants_match_the_lock` (`WINDOW == MAX_RECORDS`, `MAX_JOBS >= 2*WINDOW`), `ex6_random_differential_against_the_model` (24 × 400 calls); TLA-EXEC (`InvJobWindow`); mutants s31-s34, s36, s37, s00b; t49, t51, t52; FR-13, FR-14 | tested |
| EX-7 | Cross-contract (upstream EXEC): the executor never forgets the job of a record the lock retains | near-tests, sandbox | near-tests `ex7_d1_confirmed_failed_executes_stay_within_256` (EX-7 after every transaction), `ex6_window_constants_match_the_lock`; sandbox `rt_window_300_records_through_the_executor`, `rt_d1_confirmed_failed_executes_keep_256_jobs` (EX-7 after every transaction); TLA-EXEC (`InvRetainedJob`, the model-level check); mutants s34, s00b; FR-13 | tested (mocked runtime and sandbox) |
| EX-8 | `on_settled` is private, needs exactly one promise result, and advances `newest` only when the lock's `settle` succeeded | near-tests, sandbox | `ex8_on_settled_protocol`, `ex8_callbacks_in_any_order`, `ex1_executor_wrappers` (privacy on wasm); sandbox `rt_private_new_and_on_result` (`on_settled` private; on its own key "not a settle callback"), `rt_d1_confirmed_failed_executes_keep_256_jobs` (a refused settle moves nothing); TLA-EXEC (`NewestOnVouch`, `AdvanceOnVouch`); mutants s29, s30, s35, s38, s00b; t50, t53; FR-13 | tested (mocked runtime and sandbox) |

### 5.9 Runtime (sandbox, real wasm)

One build: upstream `scripts/build.sh` (§9.2).

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| RT-PRED | Calls through a relay are attributed to the relay | sandbox | `rt_pred_relay_is_the_caller` | tested |
| RT-PRIVATE | `new`, `on_result` and the executor's `on_settled` reject foreign callers | sandbox | `rt_private_new_and_on_result`: `new`, `on_result` and `on_settled` fail for foreign callers and the relay; on the pool's own key `on_result` is Ok with outcome unknown and changes nothing, and `on_settled` on the executor's own key fails "not a settle callback" | tested |
| RT-CALLBACK | `execute` panic → Unknown; "cancelled" → Cancelled and released; "active" → Active; a result that is not a string → Unknown | sandbox | `rt_callback_status_mapping`, `rt_callback_non_string_executor_results` (the number 5, a 70-byte string: Unknown; NT-F4 fixed); FR-10 | tested |
| RT-REVERT | A core error leaves storage byte-identical and emits nothing; with the version patched to `VERSION_CAP`, submit and set_paused fail and write nothing, and settle is Ok | sandbox | `rt_revert_core_errors_write_nothing`: core errors leave storage byte-identical and emit nothing; a 1-yocto `reconcile` fails (NT-F2 fixed); at a patched `VERSION_CAP`, `set_paused`, `observe` and `dispatch` fail `Arithmetic`, `preview` and `submit` reject, and `cancel` and `finish` still resolve the open records (F-5 fixed) | tested |
| RT-RACE | Settle before the execute callback; the late ack is ignored | sandbox | `rt_race_settle_before_callback`, `rt_race_executor_key_settles_while_callback_in_flight`, `rt_race_operator_finish_vs_callback`; an accepted `finish` has three receipts (finish, settle, on_settled) | tested on one shard; the operator's `finish` cannot overtake the callback there |
| RT-WEDGE | F-1 end to end, inverted: the F-1 configuration's hedge is refused on chain; a recovery by completion settles end to end, and the lock and the executor agree | sandbox | `rt_wedge_f1_refused_and_cure_settles` (F-1's hedge at [12, 5] refused `Bounds`; the cure-by-completion hedge settles through `finish(true)`, then `observe` is Ok), `rt_wedge_f2_honest_release_after_the_hedge` (8 and 7 units refused, 6 admitted, the position's honest release Ok) | tested (v1.0.0: F-1 reproduced) |
| RES-1 | Gas at the maximum-size state stays under 300 Tgas, and `api.md`'s documented prepaid gas suffices (executor at 256 and 512 jobs) | sandbox | `res_1_gas_at_max_state`, `res_1_min_prepaid_gas_and_executor_cap`: at the maximum-size state (32 metrics, 64 terms, 32 recipes, 256 open records) every pool receipt is under 30 Tgas (worst 14.81, a submit with 255 open records); least whole-Tgas prepaid gas `dispatch` 70, `reconcile` 46, `confirm_not_executed` 55 (254 jobs), `finish` 55 (256 jobs) and 56 (512 jobs), each at least 10 below api.md's attach; every smaller amount from 40 Tgas fails out of gas and changes neither contract; `on_settled` at most 3.68 of 10 Tgas | tested: equal to api:44-59 (v1.0.0: worst receipt 12.56 Tgas; `finish` 43) |
| RT-WINDOW | F-3 end to end on the real wasm: 300 records through the executor leave `first_id` 44, 256 records and at most 256 jobs; pruned ids are `NotFound` for dispatch, reconcile and settle; `get_commitments` pages across `first_id` | sandbox | `rt_window_300_records_through_the_executor` (port of upstream `integration.rs`, host oracle after every step): `first_id` 44, jobs 44..=299; `reconcile`, `cancel` and `settle` of pruned id 0 `NotFound`; the executor's `finish(0)` "unknown job", `confirm(0)` "stale id" | tested |
| RT-D1 | The D-1 chain (confirmed failed executes) keeps the executor at 256 jobs or fewer, and never above 512, at upstream's ten checkpoints | sandbox | `rt_d1_confirmed_failed_executes_keep_256_jobs` (port of upstream `integration.rs`, separate operator): at most 256 jobs at the ten checkpoints; a refused confirmation keeps its job and moves nothing; a Reserved record confirmed early is cancelled at dispatch and `finish(id, false)` forgets the oldest job | tested |

### 5.10 Requirement checks and Prolog model checks

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| C-1 | Authority matrix: docs = shell attributes and caller sources = core caller checks | prolog | 12 methods, 12 shell methods, 6 core caller checks, 17 shell → core calls (7 reads) | checked |
| C-2 | Guard order: docs = code = TLA+ `GuardOrder` | prolog | 8 op kinds, 57 guards (49 doc, 8 doc_condition; submit 15, dispatch 7, ack 4, cancel 9, settle 8 + 8, pause 2, observe 4); 10 unreachable-site rules; the documented `Full` and version-cap conditions compared with the code's (5) | checked |
| C-3 | Lifecycle: docs = code = TLA+ `Edges` = the traces (v1.1.0: no divergence) | prolog | 36 rows, 11 edges, 3 code-only rows; all 36 rows observed in the traces; v1.0.0's two divergences closed | checked (v1.0.0: one divergence, `div-ack-noop-version`) |
| C-4 | Constants: docs = code | prolog | 9 compared (`VERSION_CAP` = 4,294,966,527), 5 derived | checked |
| C-5 | Fixture conservation (STOCK, CASH, RISK, self-authorization): every claim holds (v1.1.0: no known finding) | prolog | 23 claims on trace configs 1-5; a two-record release search per fixture (1,308 / 1,308 / 36 / 28 / 81 admitted pairs; 0 blocked releases) and on the trace states (0 blocked); RISK-HEADROOM 46 states | checked (v1.0.0: F-2 and F-4 reproduced) |
| C-6 | Traceability: `traceability.pl` = this section (IDs and layer lists); requirements, threats and findings traced | prolog | `verification/consistency.pl` reads the first cell and the *Layer(s)* cell of every table row of §5 from this file: 118 IDs, 118 rows, 118 equal layer lists; 79 requirements, 25 threats, findings open 0 / fixed 6, divergences open 0 / closed 4 | checked |
| MC-1 | Small model check of `step/4` (v1.1.0: no dead and no release-blocked state) | prolog | seven universes, each complete: `wedge` 4,672 states, `healthy` 5,032, `reliance` 454, `window` 4,754, `window1` 2,186, `cap` 1,834, `f2b` 1,776 | model-checked (v1.0.0: 228 dead states in `wedge`, 150 release-blocked in `reliance`) |
| MC-2 | Replay of every trace through `step/4` | prolog | 216 traces, 8,865 calls (2,565 Ok), 0 divergences | tested |

### 5.11 TLA+

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| TLA-SAFE | `InvMC` and `StepProperty` in every reachable state and step of MC_Safety | tla | `MC_Safety` (MaxRecords 2, VIEW): 435,342 distinct views, depth 20; `MC_Recovery` 92,444 states; `MC_F2b` 30,904 (v1.0.0: 2,306,738 states at MaxRecords 3) | model-checked |
| TLA-IND | `Inv` is inductive (Apalache) | tla | not done: Apalache 0.47.2 rejects the model's recursive sums | open |
| TLA-LIVE-1 | LIVE-2 on the completion path: observes in bounds, executor weakly fair to completing | tla | `liveness` (7,856 states), `liveness-f2-done` (5,202); `liveness-unfair` must fail and does | model-checked |
| TLA-LIVE-F1 | LIVE-2 in F-1's setting (disturbing observes, the executor weakly fair to settling either way): holds at v1.1.0 | tla | `liveness-f1` holds (7,918 states) | model-checked (v1.0.0: violated, F-1) |
| TLA-LIVE-F2 | LIVE-2 in F-2's configuration (healthy observations, the executor weakly fair to the honest `settle(false)` only, callers to cancelling): holds at v1.1.0 | tla | `liveness-f2` holds (5,202 states) | model-checked (v1.0.0: violated, F-2) |
| TLA-WIT | Completed, prerequisite, disturbed, recovery, prune and refresh states and steps are reachable; the wedge is not (`NoWedge`) | tla | 20 witnesses in MC_Safety, MC_Recovery, MC_F2b and MC_Cap (`tla/RESULTS.md`), each violated, as required; `NoWedge` is an invariant | model-checked: each violated, as required |
| REF-5 | The TLA+ model accepts every real-core trace | tla | TraceCheck, 9,081 of 9,081 states (216 initial + 8,865 steps) | tested |
| TLA-CAP | The near-cap harness `MC_Cap`: from states a few steps below `VERSION_CAP`, `InvMC` (with InvVerCap and LiveAll) and `StepProperty` hold up to u32::MAX | tla | `MC_Cap` (MaxRecords 2, no VIEW, from `VersionCap - 4`): 64,644 states, depth 9; `Witness_AtCap`, `WitStep_ResolveAtCap`; mutants t40-t44 | model-checked |
| TLA-EXEC | Executor protocol model `Executor.tla`: the executor's job window against the lock's record window, every receipt in any order (WINDOW 2 and 3, MAX_JOBS = 2·WINDOW): the executor never forgets the job of a record the lock retains (EX-7; checked as `InvRetainedJob`: a retained record whose `execute` made a job still has it), and only refused or in-flight confirmations can fill the job cap (`InvCapShape`) | tla | `run.sh executor`: WINDOW 2 3,880,089 distinct views (depth 33), WINDOW 3 7,323,777 (depth 30); 12 witnesses reachable (the job-cap refusal and its exit among them, §4.6); mutants t49-t54; quotient cross-check `executor-w2-fullview` (13,996,226 states, opt-in) | model-checked |

### 5.12 Lean

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| LEAN-IND | `Inv` holds initially and after every call | lean | `inv_init`, `inv_step`, `reach_inv`, `hard_of_inv`, `bump_ok`, `remSum_le` | proved (model) |
| LEAN-STEP | Every call from every state satisfies the step properties | lean | `stepProps`, `stepProps_step`, `exec_auth`, `term1`, `term1_cancel`, `stale1_no_replay`, `version_mono_run` | proved (model) |
| LEAN-LIVE1 | LIVE-1 at every size | lean | `live1`, `live1_exec`, `live1_reach` (no B or version premise) | proved (model) |
| LEAN-WIT | Inverted: the F-1 and F-2 hedges are refused (`f1_rejected`, `f2_rejected`), with healthy-side and recovery non-vacuity | lean | `Witness.lean`: `f1_rejected`, `f1_envelope`, `recovery_admitted`, `res_needed`, `live_all_needs_res`, `f2_rejected`, `self_executor_invalid`, `owner_executor_valid`, `cap_reach`, `cap_instance`, `cap_drain`, `refresh_instance` and the healthy-side instances | proved (model) (v1.0.0: `f1_forever`, the wedge permanent) |
| REF-5L | The Lean machine agrees with the real core on every trace | lean | `pool-lean replay`: 216 traces (1 refused at init), 8,865 calls (2,565 Ok), every result and post-state equal | tested |
| LEAN-GUARD | Each call's guard list is the code's order | lean | `Guard.guards_order`, `Guard.outcome_mem`; `pool-lean-guards` prints `guardOrder`; equal to Prolog `guard/5` once the sites that cannot fire in an `Inv` state are dropped (a script, not in `run.sh`) | proved (model) |
| LEAN-LIVE-ALL | LIVE-ALL at every size | lean | `res_sub`, `close_ok`, `live_all`, `live_all_reach`, `no_wedge`; canary `LiveAllNoRes` fails to compile; `regression/Live1NoPremise.lean` compiles | proved (model) |

### 5.13 Kani refinement links

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| REF-1 | Guard exactness: the core's error is the TLA+ error operator's, so Ok exactly when the action is enabled | kani | Kani `h_ref_<op>` (7) against `tla.rs`, a hand transcription of `SharedPool.tla`; 43 `ref` covers; plain mode on the 216 traces (8,865 calls) and with prunes in `window_walk`; mutant 31 (§17.2) | bounded proof (Kani, step bounds) |
| REF-2 | Effect exactness: on Ok, the abstraction of the post-state is the TLA+ action's post-state | kani | Kani `h_ref_<op>` (7), as REF-1 | bounded proof (Kani, step bounds) |
| REF-3 | Rejected and idempotent calls stutter in the model | kani | Kani `h_ref_<op>` (7), as REF-1 | bounded proof (Kani, step bounds) |

### 5.14 Assumptions

The levels are those of `threat_model.pl`: high, med, low, or policy.

| ID | Statement | Layer(s) | Evidence | Status |
|---|---|---|---|---|
| ASM-PRED | The predecessor cannot be forged | assumption | RT-PRED (relay attribution on near-sandbox), SH-1 | trusted (high); tested |
| ASM-PRIVATE | `#[private]` ⇔ predecessor = current account | assumption | RT-PRIVATE (`new`, `on_result`, the executor's `on_settled`), SH-2, SH-3, EX-8 | trusted (high); tested |
| ASM-CALLBACK | The callback sees the executor's return or `PromiseError` | assumption | RT-CALLBACK (non-string results included), SH-3 | trusted (high); tested |
| ASM-ATOMIC | A panicking receipt writes nothing | assumption | RT-REVERT (storage byte-identical, no event); RES-1 (84 out-of-gas attempts leave both contracts' storage byte-identical) | trusted (high); tested |
| ASM-INTERLEAVE | Receipts can run between `dispatch` and `on_result`; ack and settle are independent model steps | assumption | RT-RACE (one shard) | trusted (med); tested in part |
| ASM-TIME | The block timestamp is monotone | assumption | none; TLA+ and Lean allow any time per call (a superset); MC-1 assumes it | trusted (high); untested |
| ASM-EXEC | The executor is honest: `settle(false)` only on definitive nonexecution | assumption | none. Since F-2 is fixed no property needs a false attestation; the residuals of T-UNKNOWN, T-SLOTS (head-of-line) and T-JOBCAP rest on it (§4.6) | policy |
| ASM-OWNER | Observations are true | assumption | none; the residual of T-SELF (the owner may be an executor, §4.6) | policy |
| ASM-DEPLOY | No code replacement; the deployment lock is out of scope | assumption | none for the account's keys; the in-contract half (no deploy, key, transfer, stake, account or global-contract import, no upgrade export) is SH-11 and SH-12 (NT-F3 fixed) | policy |
| ASM-TOOLS | Scryer, TLC, Lean, Kani and the extractors are sound for what is used | assumption | canaries, witnesses and mutants in every runner; the Lean axiom audit (65 of 65 headline theorems); Kani's canary, covers and mutants (`h_canary` fails as required, the covers (CI's `covers` job, pending), the mutants as §6 says; two Kani 0.68.0 hazards worked around, §6) | trusted (med) |
| ASM-SPEC | The specs, and the Prolog transcription of the docs, mean what we intend | assumption | `[review] spec_reviewer` (empty); the Prolog author had read the code (§8) | open: no reviewer (low) |
| ASM-MODEL | The small scopes are representative: Kani's bounds, TLC's universes and 32-bit integers, MC-1's universes | assumption | Lean at every size (model); traces at 256 records with prunes and ±1,000,000; reads at and above `VERSION_CAP`; TLC's scaled window and cap (§17.5); Kani's bounds, stated per harness (§6) | trusted (med); tested on 216 traces |
| ASM-LEAN-TLA | The Lean and TLA+ models transcribe the code | assumption | REF-5, REF-5L, MC-2 (216 traces), C-2; REF-1..3 (TLA+, Kani, within the step bounds) | trusted (med); tested on 216 traces; bounded proof per step for TLA+ (Kani, REF-1..3) |
| ASM-COMPILE | `overflow-checks = true` in the release profile | assumption | `tools/scripts/check-pin.sh` (`release_overflow_checks = true`) | trusted (high); checked |
| ASM-GAS | Callers attach at least the prepaid gas `api.md` documents; a receipt that runs out of gas changes nothing (ASM-ATOMIC) and can be retried. Liveness is conditional on authoritative evidence and gas (handoff:34) | assumption | RES-1 at 57b3f02: every pool receipt under 30 Tgas (largest of the calls without promises 14.81); least prepaid `dispatch` 70, `reconcile` 46, `confirm_not_executed` 55, `finish` 55 (56 at 512 jobs), each at least 10 Tgas below api.md's attach; smaller amounts fail out of gas and change neither contract (ASM-ATOMIC); `on_settled` at most 3.68 of 10 Tgas | trusted (med); tested: equal to api:44-59 |

---

## 6. Kani on the real core

Results, bounds, deviations and the measurement: `kani/RESULTS.md`. Kani
0.68.0 with CBMC 6.11.0, on the pin `57b3f02` (v1.1.0's code). The
harnesses and mutants were measured with `kani/local/run-local.py --all`
at `2d33d33` (Kani inputs equal to the record commit's: the same
fingerprint), up to three jobs at a time on a 10-core, 64 GB Apple M1 Max, on
2026-11-14 (runs 1, 3 and 4; W7's probes and `run.sh ref` before them, on
the same harness code). The 48 harnesses and
1 mutant that do not fit CI's hosted runner are
recorded from those runs in `kani/local` at `2d33d33`, and CI
checks the record (§14; `kani/RESULTS.md`, Local record).

**Design.** `projects/shared-pool/kani/` is a standalone crate whose only
dependency is `pool-core` from the mirror, with `default-features = false`.
Kani checks the code that is deployed, not a transcription. `src/spec.rs`
is an executable specification written from the brief and the v1.1.0
docs, not from the code: each call's exact result in guard order, its
exact post-state and frame, the release-aware `envelope`, the window
(prune at 256 records, `Full` only with an open front), the version cap
(`bump_new`), `completed_units`, `energy`, `validate`, and the invariants
with their ghost (`base`, `k0`, a summary of pruned records). Harness
bodies are dual-mode, as in near-approval §6.1: symbolic under Kani, which
is the proof, and random in plain `cargo test --release`, which is a test
that takes seconds. Each step harness is four CBMC runs, so that each fits
in memory: `_tx` (exact result, TX-0, TX-NOOP, TX-VER), `_post` (exact
post-state and the step properties), `_inv` (INV preservation, INV-RES,
INV-VERCAP and WIN-1 included) and `_cov` (the covers). `src/tla.rs`
transcribes the v1.1.0 `tla/SharedPool.tla` for REF-1..3 (§17.2).

**Results.** 98 harnesses (`run.sh list`; v1.0.0: 89). Every one ran as
required: `ok`, and `h_canary` FAILED on its own assertion only.
78 of them were verified on the Mac (the
measurement, the probes and `run.sh ref`), the
48 of the local record among them; CI's `kani`
job runs the 50 that fit its hosted runner on every
change that reaches the Kani layer (its run with the v1.1.0 record is pending on PR #32). CI's `covers`
job checks that every cover is SATISFIED in some harness of its family (pending;
v1.0.0: 164 covers); the covers include every
exit of every step and the v1.1.0 witnesses of the fixed findings. Plain
mode: 33 of 33 tests, `h_ref_traces` on all 216 traces (8,865 calls).
Mutants: 45, 44 that must be killed (43 of the core, mutant 31 of
`tla.rs`) and mutant 20, equivalent, which must survive; 32 to 45 are the
fix-revert mutants N1 to N14 (§4.5). In plain mode every one is as its
header expects (`run.sh mutants`, and CI's `mutants` job on every
change). Under Kani: the recorded large mutant 20 as expected (surviving, the equivalent mutant), mutant 31 killed by `h_ref_cancel` (W10, at `6b120fd`), the other 16 hosted ones left to CI's manual `kani-mutants` job, and 27 large mutants declared not run at v1.1.0 (`kani/local/not-run.tsv`; plain mode kills them). No property
failed: within the bounds the code agrees with the specification on every
result, post-state and read. Kani found no new defect. The harnesses
measured in the runs took 34,619 s in all (9.6 h,
two or three at a time).

| Group | Harnesses | Shows |
|---|---|---|
| Configuration | `h_validate`, `h_init`, `h_init_inv`, `h_init_inv_2` | CFG-2: `validate` equals the specification on raw configurations, and refuses actor = executor (F-4 fixed). CFG-1, as a composition: `new` is Ok iff `validate` is, and its pool (`first_id = 0`, `completed_units = 0`) satisfies INV. |
| Numerics | `h_energy`, `h_energy_exact`, `h_energy_max` | NUM-1: `energy_terms` and `energy` are Ok on bounded values and exact (64 terms: 2.56·10^17) |
| Steps | `h_step_<op>_{tx,post,inv,cov}` (28) | TX-*, NUM-2, ROB-1 and the §5.3 step properties from 2-record INV states; INV (with INV-RES, INV-VERCAP, WIN-1) preserved |
| Induction | `h_step_<op>_inv_{0,1,3}` (21) | INV preserved from 0, 1 and 3 records as well |
| WF states | `h_wf_<op>` (5: submit, dispatch, ack, cancel, settle) | TX and the post-state from well-formed states with `first_id`, `completed_units`, the values and the version free (mutants 07, 27, 41, 44) |
| Traces | `h_bmc_from_init_po`, `h_bmc_from_init_s_<op>` (8) | Exact post-states along 2-operation traces from `Pool::new`; INV-VER; STALE-1's no-replay |
| Version cap | `h_version_cap_<op>` (7), `h_version_max_<op>` (3: ack, cancel, settle) | VER-CAP, INV-VERCAP and LIVE-ALL at or above `VERSION_CAP` from INV states; TX-VER at `u32::MAX` on WF states, nothing wraps (F-5 fixed) |
| Window | `h_full_255`, `h_full_256`, `h_full_256_prune`, `h_slot_cycle_0`, `h_slot_cycle_2` | WIN-1, WIN-ESC: `Full` only with 256 records and an open front; WIN-2: with a terminal front the submit prunes it and `first_id` advances, and `completed_units` keeps the pruned record's completions (F-3 fixed); the submit, cancel, submit cycle |
| Reads | `h_preview`, `h_preview_post`, `h_best` | RD-PREVIEW (the cap is `candidate`'s last check); RD-BEST as a composition with `h_preview`, `h_step_submit_*` and `h_energy` |
| Liveness, findings | `h_live_all`, `h_wedge`, `h_wedge_healthy` | LIVE-ALL (and LIVE-1) with up to two other open records, no premise on the base or the version; F-1 fixed from `Pool::new` (each exit of the dispatched hedge); F-2 fixed from INV states (the honest release is Ok) |
| Refinement | `h_ref_<op>` (7) | REF-1..3 against the v1.1.0 transcription (§17.2) |
| Canary | `h_canary` | Asserts that no submit is ever accepted; must FAIL |

**Bounds.** Every result holds for every input within these bounds, and
says nothing outside them.
- **Steps, REF-1..3, `h_preview`:** 1 metric, 1 term, 2 recipes (recipe 1
  may require recipe 0), and an INV pre-state with exactly 2 records in
  any statuses and `first_id = 0`. Values, bounds and targets anchored at
  −1,000,000, 0 and 1,000,000, each ±4; deltas −3..3; `max_units` 1..3;
  weights 1, 2 or 1,000. Accounts `o_`, `a1`, `x1` and `z_` in every role
  and as every caller. Times, versions, expiries and the paused flag free.
  Arguments the code only compares (recipe, units, ids) narrow.
- **INV:** true initially and preserved from exactly 0, 1, 2 and 3
  records with `first_id = 0`, so INV-WF to INV-DEP, INV-RES, INV-VERCAP
  and WIN-1 hold in every reachable state with at most 3 records and no
  prune. A prune under INV, and REF across a prune and its ghost fold,
  only in plain mode (`window_walk`) and on the traces. INV-VER only
  along 2-operation traces. `h_live_all`: 3 records.
- **WF states (`h_wf_<op>`, `h_version_max_<op>`):** up to 2 records with
  `first_id`, `completed_units`, the values and the version free;
  `h_version_max_<op>` at `u32::MAX`. **`h_version_cap_<op>`:** INV
  states at `version ≥ VERSION_CAP`.
- **`h_energy`:** 2 metrics, 2 terms, any value in ±1,000,000, any weight.
  `h_energy_max`: 64 terms, concrete.
- **`h_validate`, `h_init`:** raw configurations of up to 2 metrics, 2
  terms, 3 recipes and 2 requirements per recipe, strings of 0 or 2 bytes,
  each field in range, at an edge or out of range.
- **`h_full_*`:** 255 and 256 records as one constant array;
  `h_full_256_prune` with a terminal front and `first_id = 1`.
- **Runner:** flags `--no-memory-safety-checks --no-assertion-reach-checks
  -Z stubbing`, unwinding assertions on, and `memcmp.0:25` in every
  unwindset (CBMC's library `memcmp`, which `[i64]` equality calls; without
  it `h_step_observe_tx` fails its unwinding assertion); 1,800 s per
  harness by default (7,200 s for `h_step_*_inv_3` and
  `h_full_256_prune`), raised for the measurement where an estimate
  needed it (`kani/local/README.md`); memory cap 40 GB. Peaks reached
  39.6 GB (`h_bmc_from_init_s_submit`), and
  38 harnesses peaked above 16 GB. A peak is resident
  memory (`ps`'s RSS of CBMC and of the job's process tree, sampled every
  3 s), and so is what `run.sh`'s memory cap compares. Under memory
  pressure macOS compresses pages, and resident memory then understates
  the footprint. A side sampler of `top`'s MEM column during run 3 (not
  part of the record) saw `h_bmc_from_init_po`'s CBMC at a physical
  footprint of 63 GB while run 3 recorded 37.3 GB resident; `h_full_256`'s
  at 72 GB (recorded: 39.2 GB); and `h_step_observe_inv_3`'s at 15 GB
  (recorded: 10.3 GB) while `h_full_256` ran beside it. These peaks (for
  a harness the runs did not measure, W7's, else v1.0.0's times 1.25)
  split the harnesses for CI (§14): CI's `kani` job runs the
  50 whose peak, rounded up, is 13 GB or less, in
  7 groups, on the hosted runner, and that job, not
  the measurement, shows that each of them fits it. The other
  48, in 17 groups (with
  `h_wedge`, which the record's self-test samples, and `h_full_256`,
  which filled the hosted runner's memory and swap at v1.1.0), are in the
  local record, which CI checks; they run in CI only on a `KANI_RUNNER` of
  at least 50 GB.

**Deviations.** `RESULTS.md` lists the v1.0.0 deviations from the plan,
which still apply, and v1.1.0's. All but two of v1.0.0's came from CBMC
running out of memory or time. None weakens an assertion: each narrows the
inputs (stated in the bounds), replaces a library function by a model that
can only make a harness fail, rewrites a specification function into an
equal one, or splits a property into harnesses whose conclusions combine.
The main ones are a fixed-capacity model of `Vec::with_capacity_in` and
`Vec::reserve`; fixed shapes and record counts; PAUSE-1 for the
transparent operations at the specification level; fixed roles for F-1,
and INV states for F-2; traces rebuilt with constant shapes after each
operation; `h_full_*`'s records as one constant array; and a free
`validate` in `h_init` and free `candidate` and `energy` in `h_best`, each
pinned down by other harnesses. v1.1.0 adds seven (`RESULTS.md`,
Deviations, v1.1.0), among them: `spec::M` and `spec::GR` depend on
`cfg!(kani)` (2 and 3 under Kani, 8 in plain mode, so the
fixture tests run through the same specification); the INV domain has
`first_id = 0` and the WF domain frees it; 256 concrete records only in
`h_full_*`; `h_wedge` takes one nondeterministic exit call; `memcmp.0:25`
in every unwindset; and five `ref` covers of v1.0.0 are gone, because
v1.1.0 makes them unreachable on INV states (ack, cancel and settle
`Arithmetic`; cancel and settle `Bounds`), while REF-1 still checks on
every call that the core and the transcription agree they do not occur.

**Findings on the real core.** Each v1.0.0 finding is shown fixed on the
v1.1.0 core, and its fix-revert mutant puts the v1.0.0 behaviour back for
Kani to kill (§4.5). F-1: `h_wedge` asserts each exit of the dispatched
hedge in a disturbed state from `Pool::new`, and covers the hedge refused
with `Bounds` after a disturbing observe. F-2: `h_wedge_healthy` (the
position releases) and `h_live_all` (the F-2 shape), from INV states.
F-2b: INV-RES in every `_inv` harness. F-3: `h_full_256` (`Full` only with
an open front) and `h_full_256_prune`; the 256-cycle scenario is plain
mode's (`window_walk`, `f3_slot_exhaustion`). F-4: `h_validate` refuses a
configuration with actor = executor. F-5: `h_version_cap_*`,
`h_version_max_*` and INV-VERCAP. F-6: the observe refresh with an open
record (`h_step_observe_tx`, REFRESH-1). The states of F-2, F-2b and F-5
are constructed; Kani does not show that they are reachable from
`Pool::new` (TLA+, Lean and the traces do, for the model).

**Tool issues** (Kani 0.68.0). Two soundness hazards were found at v1.0.0
and stay worked around. A static that a harness writes can share storage
with a constant of equal bytes: `h_best`'s stub changed `RawVec`'s zero
capacity, and the harness failed. The models' statics carry a distinct
tag. A reference to a field of a local array of structs, at an index CBMC
does not fold, read unconstrained values in the function it was passed
to, and three `h_ref_*` harnesses failed. `tla.rs`'s lookups return
copies. A third issue is harmless: with `--stop-on-fail`, Kani's JSON
parser panics, so the mutant runs read CBMC's text output. v1.1.0 adds
the `memcmp` unwinding bound above: CBMC's `--show-loops` does not list
that library loop, so it would otherwise get the harness's bound.

**What is not shown.** Steps over more metrics, terms or recipes, or from
4 or more records (other than a submit at 255 or 256 records); a prune
under INV, other than `h_full_256_prune`'s; values strictly between the
anchors and the extremes, outside energy; the reachability of F-2, F-2b
and F-5's states from `Pool::new`; RD-BEST and CFG-1 other than as
compositions; LIVE-2 (TLA+'s); REF-1..3 outside the step bounds, and the
agreement of `tla.rs` with the `.tla` other than on the traces; memory
safety of std (the core forbids `unsafe`); the shell, the executor and
NEAR (§9). Plain mode, the traces and Lean at every size cover parts of
this, as tests or for the model. Under Kani, the 27 large mutants of `kani/local/not-run.tsv` (01, 03, 05, 06, 10, 11, 12, 15, 16, 18, 19, 21, 23, 25, 28, 29, 30, 32, 33, 35, 36, 37, 40, 41, 42, 44, 45) were not run at v1.1.0: the Kani half of their verdict is a gap, and plain mode kills each of them on every CI run.

---

## 7. Verus (deferred)

**Decision: not started.** The Lean layer proves the model at every size
(§18). Verus would close only the gap between that model and the code
(ASM-LEAN-TLA), which REF-5L, MC-2 and Kani narrow. v1.1.0 settled F-1
and F-2 (§4.1), so a proof would now target fixed admission and release
rules.

**Fit,** compared with near-approval's `approval-core`, where Verus path A
worked:
- **Vec-heavy state.** `Pool` holds `Vec<i64>` values,
  `Vec<Commitment>` records and, since v1.1.0, `first_id` and a
  `Vec<u32>` of `completed_units`. `Config` holds `Vec`s of metrics, terms
  and recipes; each recipe has two `Vec<i64>` effects, a `Vec<(u32, u32)>`
  of prerequisites and three `String`s. Every spec needs a `Seq` view of
  each, with INV-WF's and WIN-1's length and bound facts carried through
  every function.
- **Energy arithmetic.** Values are `i64`, units `u32`, and energy a
  `u64` built with `checked_mul` and `checked_add`. One term reaches
  4·10^15, and 64 terms 2.56·10^17 (spec:15). Each checked operation needs
  a nonlinear bound, which is where Z3 is weakest.
- **Loops over metrics and records.** Most of the code is iterator chains
  (`zip`, `map`, `collect`, `try_fold`, `filter`, `sum`, `any`,
  `min_by_key`) and a mutating closure in `envelope`. vstd's iterator specs
  are thin. In near-approval, one `filter_map` chain had to become a loop
  upstream before Verus could take the code (trust-framework PR #20).
- **Strings.** Callers are compared as `String` against `&str`.

**Cost of a twin.** `crates/core/src/lib.rs` is 566 lines at the pin
(492 at v1.0.0). The iterator chains become loops with invariants,
upstream or in `REWRITES.toml` with a Kani equivalence harness each.
Lemmas for INV-RES and ADM-2 go over `Seq<int>`. That is at least
near-approval's path A (8-13 days), likely more for the arithmetic.

**Revisit** only if "formally verified" will be claimed for the code
rather than the model.

---

## 8. Prolog

Results: `prolog/RESULTS.md`. Scryer Prolog 0.10.0. The last full run of
`run.sh` (2026-11-14, at `a7781c7`, the v1.1.0 corpus and its trace reader)
exits 0 in 1,777 s: suites 182 s, mutants 1,554 s summed, most of it the
MC-1 mutants.

| File | Contents |
|---|---|
| `requirements/pool_req.pl` | 79 `requirement/3` facts, one per doc statement used; the api.md method table; `guard/5` (57 rows in the docs' order, each `doc` or `doc_condition`; `guard_condition/4` for the `Full` and version-cap conditions); the lifecycle table; constants (`version_cap` = u32::MAX − 768); five fixtures with 23 claims; `finding/4` (F-1..F-6, each `fixed('v1.1.0', Props)`); 4 `closed_divergence/4` and 7 `ambiguity/4` facts |
| `requirements/threat_model.pl` | 6 adversaries, 25 threats, 15 assumptions (§3) |
| `requirements/traceability.pl` | `property/3`, which is §5 (118 IDs); `prolog_evidence/2` for the 48 IDs this layer carries; `trace_row/2`, 79 requirement → property rows |
| `model/pool_machine.pl` | `step/4` over `pool(Config, Values, Version, ObservedAt, Paused, Records, FirstId, Done)`, written from the docs. It walks the guard table in order, so the model's behaviour is the requirement table. The window (256) and the cap can be scaled down only inside `with_window_size/2` (MC-1). 13 places are tagged `CODE-ONLY`. Reads: `admissible/3`, `preview_allowed/3`, `best_quantity/4`. |
| `extract.py` → `extracted/` | Facts from the core and contract `lib.rs`, `fixtures.rs`, `SharedPool.tla` and `Props.tla` (constants, error sites with their window and version conditions, caller checks, status guards, role comparisons in `Config::validate`). A narrow parser that fails loudly on unknown constructs. `run.sh` fails on a regeneration diff. |
| `verification/`, `tests/`, `mutants/` | C-1..C-6, MC-1, MC-2, the COV-2 reads differential and the property checkers (`props.pl`: invariants, step properties, and the LIVE-1, LIVE-ALL, REFRESH-1 and WIN-ESC probes); five suites; 32 model (m21-m32 each restore one v1.0.0 behaviour), 2 requirements (r01, r02), 11 code (c10-c12 the F-4, F-3 and F-5 reverts) and 2 reads mutants (d01, d02) |

**Results.**
- **C-1..C-6 pass** (§5.10). C-5 reproduces none of the six findings
  (`findings([])`); its release search finds 0 blocked releases (1,308 /
  1,308 / 36 / 28 / 81 admitted pairs; 10,464 / 10,464 / 288 / 224 / 648
  releases tried). C-6 compares `traceability.pl` with §5: requirements
  mutant r01 drops the TLA-IND row, r02 adds `tla` to NUM-2's layers, and
  only that comparison kills them. The requirements suite (19 tests) takes
  13.9 s.
- **MC-1** explores seven universes, each complete, with no violation and
  no dead or release-blocked record: `wedge` 4,672 states (149,584 edges),
  `healthy` 5,032 (134,752), `reliance` 454 (10,810), `window` 4,754
  (160,976), `window1` 2,186 (65,070), `cap` 1,834 (52,770), `f2b` 1,776
  (43,240); the suite takes 111 s. The F-1 hedge in [12, 5] and the F-2
  hedge are rejected `Bounds`; in `window` and `window1` prunes, `Full` on
  an open front, prerequisites admitted after a prune and `NotFound` on the
  pruned id all occur; in `cap` submit, dispatch, set_paused and observe
  stop with `Arithmetic` while ack, cancel and settle stay Ok (v1.0.0: 228
  dead states in `wedge`, F-1; 150 release-blocked in `reliance`, F-2).
- **MC-2** replays 216 traces (8,865 calls, 2,565 Ok) from the code's
  pre-states with 0 divergences; trace 210's refused config is refused by
  `valid_config/1`. Every op kind returns Ok, and all 15 error kinds the
  code can return from an INV state occur. The invariants, step properties
  and probes hold on every core state. The suite (23 tests) takes 14.0 s.
- **Unit tests:** 34 `step/4` tests pass (1.4 s), including the "F-1
  fixed" .. "F-6 fixed" tests, each keeping its v1.0.0 outcome in a
  comment.
- **Reads differential (COV-2).** `test_reads` rebuilds each of the 1,300
  states of `conformance/traces/reads.txt` (12 at `VERSION_CAP` or
  u32::MAX) and compares the real core's `preview` and `best_quantity` on
  39,627 requests with the model: 0 divergences (3 tests, 41.7 s). 1,785
  requests are allowed; 6,233 have a best quantity. Mutants m19, m20 and
  m28 are killed by this suite; reads mutant d01 gives 1,645 divergences,
  and d02 stops the generator (4,919 preview/submit disagreements). This is
  testing, not proof.
- **Differential replay.** The v1.0.0 corpus through this model diverges
  on exactly 44 traces, each at the call a fix changes, and the v1.1.0
  corpus through the v1.0.0 model on the same 44 (`prolog/RESULTS.md`).
- **Canaries and mutants.** Each of three suites exits 1 with only its
  canary failing. 47 of 47 mutants are killed (1,554 s summed).

**What it does not show.**
- **Independence (ASM-SPEC).** Unlike near-approval's M4, this layer's
  author had read the code. Doc-backed facts cite their line, and
  code-only facts are labelled, but no independent reviewer has checked
  either.
- **Bounds.** MC-1's universes have at most 3 records or a window of 1-2,
  1-3 times and fixed observation vectors; `cap` covers the last 4
  versions below a scaled cap. The real window and cap appear only in the
  unit tests and the traces. C-5's release search covers two records from
  each fixture's init state and the trace states.
- **MC-2 coverage.** The traces reach `full` once (trace 207) and
  `arithmetic` 3 times; no trace comes near `VERSION_CAP` (only the reads
  have states there).
- **Reads.** RD-PREVIEW's core half and RD-BEST are tested on the recorded
  requests only; other states and requests, the per-term energies and the
  strings are not compared.
- **C-2 unreachable sites.** The alignment skips ten kinds of code error
  site that cannot fire from an INV state; each rule names what discharges
  it.
- **TLA leg.** C-2 and C-3 compare `GuardOrder` and `Edges` as written;
  that the TLA+ operators follow them is REF-5.
- **Extraction (ASM-TOOLS).** A construct inside `extract.py`'s grammar
  could be misread. Code mutants c01-c12 show it sees their changes.
- **Shell and strings** belong to near-tests and the sandbox.
- **MC-1 canary.** `test_mc1` has no canary run; its counts, witnesses and
  the mutants it kills show it is not vacuous.

---

## 9. Shell, runtime and deployment

### 9.1 `near-tests/` (mocked runtime)

Results: `near-tests/RESULTS.md` (2026-11-14). Toolchain 1.98.1 (the
mirror's `rust-toolchain.toml`), near-sdk 5.29.1 with `unit-testing`; the
mocked runtime is near-vm-runner 0.37.4 `VMLogic`, the wasm interpreter
wasmi 2.0.0.
- **Host path.** Calls the `#[near]` methods as Rust functions. State is
  read first and written only on success; a panic discards every write.
- **Wasm path.** Runs the real wasm of the documented build, upstream
  `scripts/build.sh` (`RUSTFLAGS="--cfg near"`), with every `env` import
  forwarded to the same `VMLogic`. This covers what near-sdk generates only
  for wasm32: init-once, `#[private]`, deposit checks, JSON and
  `#[callback_result]`. The build without the cfg is only NT-F1's negative
  control. Forbidden host calls trap. A receipt router runs the pool and
  executor wasm side by side.
- **Oracle.** `pool_core::Pool`, a reference model of the executor written
  from api.md's executor protocol, and `docs/api.md`; never the contract.

| File | Tests | Covers |
|---|---|---|
| `tests/shell.rs` | 15 | SH-1..SH-7, SH-9, SH-10, RD-PREVIEW (shell half); the inverted NT-F1, NT-F2 and NT-F4 witnesses; F-3 through the shell |
| `tests/store_diff.rs` | 3 | SH-8: 2,000 × 40 host calls, 300 × 40 on the documented build; `sh8_store_diff_window`, 4 host and 1 wasm sequences of 1,500 submit-heavy calls past 256 records |
| `tests/traces.rs` | 1 | all 216 traces (8,865 calls) on the host and the documented build; trace 210's `new` fails and writes nothing |
| `tests/executor.rs` | 14 | EX-1..EX-8: the window, the job cap, `on_settled`, a random differential against the model (24 × 400), the D-1 chain in the mocked runtime |
| `tests/wedge.rs` | 3 | F-1's hedge refused `Bounds` through the shell and the curing hedge resolvable; a control; end to end with the executor, and F-2's release Ok |
| `tests/scenarios.rs` | 1 | SCN-1: the six published scenarios re-derived from `pool_core` |
| `tests/lockfile.rs` | 1 | the packages under test resolve exactly as in the mirror |

All 38 tests pass. `run.sh test` and `run.sh all` first run
`check-tags.py` (stdlib Python, under a second), which fails the run
unless every test has its own PROP tag line, every tag is a `property/3`
ID, and each near-tests property is tagged on a test that checks it, or
on a `wasm-surface.sh` check (SURF-n). It reports 0 problems: catalogue
118 IDs, 22 in layer near-tests, 38 tests tagged, 28 distinct IDs. Each of
the 41 mutants (s01-s38 with s32 split, and the historical s00a and s00b:
v1.0.0's contract and executor over the v1.1.0 core) is applied to a copy
of the source and the documented build is rebuilt; the unpatched control
survives and all 41 are killed. `run.sh all` at `a7781c7` took about 22
minutes, 21 of them the mutants.

**Wasm surface.** `near-tests/wasm-surface.sh` builds the documented build
once. SURF-1 (exports, and each import classed as promise or other)
equals `wasm-surface.expected`, regenerated for v1.1.0: `pool_contract`
exports its 12 methods, `contract_source_metadata` and `memory` (14),
`pool_executor` its 8 methods and the same two (10); it is SH-11's check.
SURF-2 (no forbidden import) passes (NT-F3 fixed; v1.0.0: expected to
fail). SURF-3 runs build.sh in the mirror, requires its `build.json`
(`dirty: false`, the pinned commit, `--cfg near`) to hash to this layer's
wasm and each wasm's imports to equal build.sh's `ALLOWED` (contract 18,
executor 16), and runs build.sh's own checker on the no-cfg wasm (refused:
no `panic_utf8`) and on two synthetic modules. The historical mutants
import 19 `promise_batch_*` functions per wasm and fail SURF-2.

### 9.2 Sandbox (real wasm, local chain)

Results: `sandbox/RESULTS.md`. near-sandbox 2.13.4 through
near-workspaces 0.23.0, the versions upstream `crates/sandbox` pins. Every
test keeps a host `pool_core::Pool` in step with the chain and compares the
whole `get_pool` state after each step. 342 shared registry packages match
the mirror. One build: `scripts/build-wasm.sh` runs upstream
`scripts/build.sh` (rustc 1.98.1, `--cfg near`) and requires its
`build.json` to be clean, at the pin, and to hash to the copied wasm.
v1.0.0's second variant, `cfg-near`, is what build.sh now does.

| Wasm | sha256 | Bytes | Imports |
|---|---|---|---|
| pool_contract | `e6383aabed62dcf694ac2f97da4520a019b18c9db637b1ee2afc7180c697e2ea` | 377358 | 18 |
| pool_executor | `b92f146919be69f36f517edc10f1e04e6ee96430ba0d745e4422132615634e83` | 223068 | 16 |

These equal the hashes near-tests' SURF-3 records and those upstream's
RES-1 figures (api:44-59) were measured on.

- **Build.** Every expected failure reads `Smart contract panicked: <msg>`;
  a bare trap fails the test (SB-1 = NT-F1 fixed;
  `sb1_documented_build_reports_panic_messages`).
- **RT-RACE.** On one shard, a `settle` from inside `execute` always lands
  before the callback, and the late ack is a no-op (version +2, not +3).
  For every accepted operator `finish` there are three receipts (finish,
  settle, `on_settled`), and `on_settled` raises the executor's `newest`.
  The operator's `finish` cannot overtake the callback on one shard: a
  gap.
- **RT-WINDOW, RT-D1.** 300 records through the executor leave `first_id`
  44 and jobs 44..=299; pruned id 0 is `NotFound`. Upstream's D-1 chain
  keeps the executor at 256 jobs or fewer at its ten checkpoints. EX-7 is
  checked after every transaction.
- **RES-1.** The largest state, built with real transactions: 32 metrics,
  64 terms, 32 recipes with full prerequisites, 256 open records. `STATE`
  is 45,749 bytes of Borsh, `storage_usage` 423,334 bytes, and `get_pool`
  53,401 bytes of JSON. The worst pool receipt is a submit with 255 open
  records at 14.81 Tgas, 15.19 under the documented 30 Tgas attach.
- **RES-1, prepaid gas.** At that state, the least whole-Tgas prepaid gas
  that succeeds is 70 Tgas for `dispatch` (attach 100), 46 for `reconcile`
  (60), 55 for `confirm_not_executed` and for `finish` (70), and 56 for
  `finish` at the executor's 512-job cap. Each of the 84 smaller attempts
  from 40 Tgas fails with "Exceeded the prepaid gas." and changes nothing:
  both contracts' raw storage byte-identical, no event, the record's
  status kept. `on_settled` uses at most 3.68 of its 10 Tgas. Every figure
  equals api:44-59 (v1.0.0: worst receipt 12.56 Tgas; `finish` 43).
- `run.sh` took 1,041 s with warm caches: `tests/runtime.rs` (13 tests)
  510 s, `tests/resources.rs` (RES-1, 2 tests) 523 s.
- **Not shown:** multiple shards and congestion; other protocol versions'
  gas; that RES-1's state is the worst case, for gas or for the least
  prepaid gas (the calls without promises ran at 300 Tgas only); storage
  staking; any live deployment.

### 9.3 Deployment

Out of scope: no deployment of this pin exists. If one is made, follow
near-approval §5.10 (code hash, lock at deploy, owner as a multisig), and:
- deploy to fresh accounts: v1.1.0 stores more state than v1.0.0 in both
  contracts, so it cannot replace v1.0.0 code in place, and no migration
  is provided (api:66);
- build with `scripts/build.sh`, which sets `--cfg near` and checks the
  imports (NT-F1 and NT-F3 closed); the hashes are §9.2's;
- fund the pool and executor accounts above their storage stake at their
  largest states (§9.2; the executor's `STATE` is 12,145 bytes at 512
  jobs); attach at least api.md's prepaid gas (ASM-GAS).

---

## 10. Correspondence

### 10.1 The conformance traces

`conformance/` is a standalone crate over the pinned `pool-core`.
`cargo run --release -- --out traces/core.txt` writes the traces, and
`--check traces/core.txt` regenerates them and fails on any difference.
`--reads-out` and `--reads-check` do the same for `traces/reads.txt`.
The format is Prolog terms, documented at the top of `src/main.rs`: states
are `s/7` (with `FirstId` and the per-recipe completed units), and a
refused config is `init_error(invalid_config)`. Accounts are `o`, `a1`,
`a2`, `x1`, `x2` and `z`.

`traces/core.txt` (sha256 `dd24852e…de4268`) holds 216 traces of 8,865
calls, 2,565 of them Ok (no-ops included). The highest state version is
518 (trace 207's last state).

| Traces | Content |
|---|---|
| 0 | `wedge`: F-1's hedge, now refused at submit |
| 1-5 | the five upstream fixtures by name (`gpu`, `friday`, `inventory`, `hiring`, `risk`): each scenario, then 40 random calls |
| 6-205 | random configs (1-3 metrics, 1-3 recipes), 40 calls each |
| 206 | `arith`: `Arithmetic` at the 10^6 bound; the expiry horizon edges |
| 207 | `window` (F-3): 256 records, three prunes, a prerequisite admitted across a prune, `Full` with an open front, `NotFound` for pruned id 0 from dispatch, ack, cancel and settle, a refresh (523 calls) |
| 208 | `release-arith` (F-2b): refused `Arithmetic` at submit |
| 209 | `f2-risk` (F-2): the hedge refused `Bounds` at submit in the supplied `risk` fixture |
| 210 | `f4-self` (F-4): actor = executor; `init_error(invalid_config)` |
| 211 | `f6-stale` (F-6): the refresh with records open |
| 212 | `station`: the sixth fixture (its actors collapsed to `a1`, `a2`, `a2`), then 40 random calls |
| 213 | `owner_exec`: the owner as a recipe's executor settles (`amb-owner-roles`) |
| 214, 215 | `f1_recovery`: the recovery hedge admitted in [12, 5], settled `true` and `false` |

The traces come from the real core, so a disagreement is either a model
bug or a finding in the code. Every replay compares each result and the
whole post-state. Against v1.0.0's corpus (212 traces), traces 0-211 keep
their numbers: 168 are identical and 44 differ, each at the call a fix
changes (§8, differential replay); 212-215 are new.

`traces/reads.txt` (COV-2, sha256 `9733bc8f…700103`, 3.2 MB) holds the
real core's two reads at 1,300 states of those traces: every pre-state of
the wedge, fixture and finding traces (but `window`), every 10th
pre-state of the others, every pre-state with 256 records retained, and
each final state; a state equal to the one read just before it, at the
same time, is skipped. For traces 1-5 and 207 it also reads the final
state with the version set to `VERSION_CAP − 1`, `VERSION_CAP` and
u32::MAX (18 states; no trace reaches them). 39,627 requests in all: every
recipe's actor with units 0, 1, 2, max_units, max_units + 1, 100, 101 and
10^6; a recipe index past the end; for one recipe, every account as
caller, version ± 1, and the expiry and freshness edges. The generator
fails if one of 29 read classes is never reached, or if `submit` on a copy
of the pool disagrees with a `preview` (at u32::MAX v1.0.0 disagreed 11
times; FR-6b). Prolog `test_reads` compares every line with the model
(§8).

`conformance/coverage.py` recomputes the coverage classes from the terms:
47 `REQUIRED` classes and the `FORBIDDEN` `cancel|settle` × `bounds|arithmetic`
(LIVE-ALL). It exits 1 on the v1.0.0 corpus (10 required classes missing,
4 forbidden present) and 0 on v1.1.0's. The `conformance` CI job runs it.

| Replayer | ID | Result |
|---|---|---|
| TLA+ `TraceCheck` (`trace2tla.py`) | REF-5 | 9,081 of 9,081 states (216 initial + 8,865 steps); `Inv` and `StepProperty` on every core state and step; trace 210's config fails `ValidCfg` |
| Lean `pool-lean replay` | REF-5L | 216 traces (1 refused at init), 8,865 calls equal |
| Prolog `step/4` | MC-2 | 0 divergences; trace 210 refused by `valid_config/1` |
| Shell: host and the documented build | SH-8 (`traces.rs`) | every result and post-state equal; trace 210's `new` fails and writes nothing |

**Coverage.** The `op:result` counts over all 8,865 calls (`noop` means Ok
with the state unchanged):

| Op | Results |
|---|---|
| submit | bounds 854, ok 585, paused 378, unauthorized 217, prerequisite 149, stale 143, expired 118, energy 98, invalid_recipe 98, invalid_units 90, snapshot_expired 60, arithmetic 3, full 1 |
| dispatch | not_found 915, invalid_state 228, ok 149, paused 70, unauthorized 63, expired 40, snapshot_expired 5 |
| ack | not_found 587, noop 334, ok 70 |
| cancel | not_found 355, ok 338, noop 51, pending 46, unauthorized 42, invalid_state 34 |
| settle | not_found 1012, invalid_state 366, ok 112, noop 110, unauthorized 74 |
| pause | ok 405, unauthorized 51 |
| observe | ok 411 (11 of them refreshes), pending 127, unauthorized 51, bounds 25 |

### 10.2 What the traces do not establish

- **Equivalence.** 8,865 equal calls are evidence that the models
  transcribe the code, not a proof (ASM-LEAN-TLA). REF-1..3 (Kani, §17.2)
  add a bounded proof per step for TLA+, within the step bounds; Lean and
  Prolog have only the traces.
- **The version cap.** No `core.txt` state comes near `VERSION_CAP` (the
  highest version is 518). Submit's and preview's cap exit is reached only
  in `reads.txt` (`arithmetic:version` 30 times); dispatch, set_paused and
  observe at the cap are shown by MC-1 `cap`, TLC `MC_Cap`, Lean
  `cap_stops` and RT-REVERT at a patched cap; Kani's part is §6.
- **Exits that must not occur.** A `Bounds` or `Arithmetic` from `cancel`
  or `settle` is `FORBIDDEN` (LIVE-ALL; v1.0.0's corpus had 9 such calls),
  so the traces show only that no trace reaches one.
- **Not told apart:** exits of one kind (submit's two `Energy` exits).
- **Small random configs:** bounds within [−5, 10] and effects within ±3;
  only trace 207 retains 256 records.

### 10.3 The chain and staleness

| Link | Checked by |
|---|---|
| docs ↔ code facts | C-1..C-4 (`extract.py`) |
| docs ↔ TLA+ guard order and edges | C-2, C-3 |
| brief ↔ real core | Kani: `spec.rs`, written from the brief and the v1.1.0 docs, equals the core within the bounds (§6) |
| Prolog `step/4`, TLA+, Lean ↔ real core | MC-2, REF-5, REF-5L (traces); REF-1..3 for TLA+ (Kani, bounded proof within the step bounds, §17.2) |
| Prolog reads ↔ real core `preview`, `best_quantity` | COV-2 (`reads.txt`, `test_reads`) |
| Lean guard lists ↔ Prolog `guard/5` | LEAN-GUARD: `pool-lean-guards` prints `guardOrder`, compared by a script (not in `run.sh`) |
| real core ↔ shell (host, documented build) | SH-8 |
| wasm ↔ NEAR runtime | sandbox RT-*, with the host core as oracle |
| executor job window ↔ lock record window | TLA-EXEC (model), EX-6..EX-8 (mocked runtime), RT-WINDOW, RT-D1 (sandbox) |
| deployed code ↔ pin | none (out of scope) |

**Staleness.** `check-pin.sh shared-pool` checks the gitlink, recomputes
the 27 `[source_hashes]` (`scripts/build.sh` among them), and requires
`overflow-checks = true`. The `conformance` job runs `--check` and
`coverage.py`; `extract.py` and both lockfile checks fail on drift. On a
pin bump, a changed `crates/core/src/lib.rs` invalidates every core
property, the traces and all three models. A changed
`crates/contract/src/lib.rs`, `crates/executor/src/lib.rs`,
`scripts/build.sh` or `Cargo.lock` invalidates SH, EX, RT and RES. Changed
docs invalidate the Prolog requirements.

---

## 11. Layout

```text
formalver/
├── Cargo.toml          # exclude += shared-pool/{source-mirror,kani,conformance,near-tests,sandbox}
├── .gitmodules         # + projects/shared-pool/source-mirror
├── .github/workflows/shared-pool.yml     # §14
├── tools/scripts/      # check-pin.sh (+ test-check-pin.sh), check-status.sh (shared with near-approval); check-kani-record.py (§14)
└── projects/shared-pool/
    ├── strategy.md  status.toml  wasm-surface.expected
    ├── source-mirror/                   # submodule → shared-pool-controls @ pinned_commit (57b3f02)
    ├── conformance/{src,traces}/        # generator; traces/core.txt and reads.txt checked in; coverage.py (§10.1)
    ├── kani/{src,mutants}/              # §6; run.sh, RESULTS.md
    ├── kani/local/                      # §14; the local record of the 48 large harnesses and 1 mutant (not-run.tsv: 27), run-local.py, record.py
    ├── tla/                             # §17; SharedPool.tla, Props.tla, MC*.tla/cfg, Executor.tla + MC_Executor_W{2,3}.cfg, TraceCheck, mutants/ (52), run.sh, trace2tla.py
    ├── lean/{PoolLean,canary,regression,mutants}/  # §18; Main.lean (pool-lean), Guards.lean (pool-lean-guards), Audit.lean; mutants/probes/; run.sh
    ├── prolog/{requirements,model,verification,extracted,tests,mutants}/  # §8; extract.py, run.sh
    ├── near-tests/{src,tests,mutants}/  # §9.1; run.sh, run-mutants.sh, wasm-surface.sh, check-tags.py
    └── sandbox/{src,tests,scripts,relay}/  # §9.2; run.sh; scripts/build-wasm.sh runs upstream build.sh
```

Each layer has a `RESULTS.md` and a `run.sh`. `run.sh` exits 0 only when
every check passes, vacuity guards included.

---

## 12. `status.toml` conventions

near-approval §12 applies unchanged (status values, keys, evidence per
layer, ceilings, tables), enforced by `check-status.sh shared-pool`.
Specific to this project:

**Re-verification (v1.1.0).** When the pin moves to a new release, every
property and assumption becomes `stale` (rank 0, like `unverified`), and
keeps its other keys as the record of the previous verification (here
v1.0.0 at `a4bb5c3`, PR #31, results at `a072a2d`) until its
layer re-checks it at the new pin and rewrites them from that layer's
`RESULTS.md`. New rows start `unverified` with the planned evidence in
`harness`. Kani re-checked its rows at v1.1.0 (§6), so no row is
`stale`. Nothing passes `in-progress`
while `[review] spec_reviewer` is empty.

| Topic | Convention |
|---|---|
| `release_overflow_checks = true` | In `[project]`. `check-pin.sh` fails unless upstream keeps `overflow-checks = true` (ASM-COMPILE). |
| Pin and version | `pinned_commit` is `57b3f02`, the merge commit of upstream PR #4 on `main` (v1.1.0); `verified_release = "v1.1.0"` names the release under verification. `[project] identity = [{ label, commit, paths }]` lists the commits that must hold the pin's code: "v1.1.0 merge commit" (`57b3f02`, the pin itself: its paths must exist) and "PR #4 head" (`6fcbd56`), each with `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates`, `scripts`, `scenarios` and `docs`. `check-pin.sh` fetches each commit the mirror lacks by SHA and fails unless every path is the same git tree or blob, mode included, at that commit and at the pin; it refuses the v1.0.0 form (`verified_release_commit`, `identical_paths`). `tools/scripts/test-check-pin.sh` tests the check on scratch repositories (§14). The v1.0.0 pin (`a4bb5c3`, identical in contract code to `6f61616`) is `previous_pinned_commit`. |
| Properties | The 103 non-ASM rows of §5, under `[modules.<m>.properties]`, one module per §5 subsection: `config_numerics` (§5.1), `transitions` (§5.2), `step_properties` (§5.3), `invariants` (§5.4), `reads` (§5.5), `liveness` (§5.6), `contract_shell` (§5.7), `executor` (§5.8), `runtime` (§5.9), `requirements` (§5.10), `tla_model` (§5.11), `lean_model` (§5.12) and `kani_refinement` (§5.13). IDs equal `traceability.pl`'s. `layer` is the first layer of the row's *Layer(s)* cell. `cross` holds the row's other layers, plus any other layer whose files carry the ID's `PROP:` tag. |
| Kani rows | Every property with `layer = "kani"` carries the Kani result (§6): `in-progress` (no spec reviewer, and Kani's ceiling is `verified-with-assumptions`), `bounded-proof` evidence, the harnesses first in `harness`, the bounds in `kani_bound`, and in `kani_run` where each of its Kani harnesses is checked now: "CI kani job" (the 50 that fit the hosted runner, rerun whenever a change reaches the Kani layer) or "local record" (the 48 that peaked above 13 GB, and `h_wedge`, run on the 64 GB Mac and recorded in `kani/local` at `2d33d33`; CI checks the record, §14). The other layers' results stay in `cross` and `harness`. What Kani does not show is not claimed for it: LIVE-2, and the reachability of F-2, F-2b, F-3 and F-5 from `Pool::new`. The §5.4 invariants list `tla` first, so TLC is their primary layer; their Kani induction is in `harness`, `kani_bound` and `kani_run`, as is LIVE-1's `h_live_all`. |
| Layer names | `near-tests/` carries `unit` (`shell.rs`, `executor.rs`, `wedge.rs`) and `difftest` (`store_diff.rs`, `traces.rs`, `scenarios.rs`). The surface check is `wasm-check`. The conformance generator is not a layer: each replay is recorded under the layer that runs it (REF-5 `tla`, REF-5L `lean`, MC-2 `prolog`, SH-8 `difftest`). REF-5 is `model-check`, a TLC run. REF-5L is a replay, test-class evidence (§5.12 says tested), recorded as `model-check` because `lean` allows no `test`; its `harness` says so. The sandbox has one build (`scripts/build.sh`), so RT-* rows name no variant. |
| False properties | None at v1.1.0. LIVE-2 holds (model-checked; v1.0.0: false, F-1 and F-2), and SH-5 holds on `scripts/build.sh`'s output (v1.0.0: false there, NT-F1). `finding` stays only on a property that an open finding makes false or caveats; there is none at v1.1.0. |
| Closed findings | TLA-LIVE-F1, TLA-LIVE-F2, LEAN-WIT, RT-WEDGE and C-5 passed at v1.0.0 while their finding reproduced. At v1.1.0 each witness is inverted in place and keeps its ID: it passes because the finding is fixed (§5). On a property, `closes = ["F-1", …]` replaces `finding`. One `[findings.<ID>]` table per v1.0.0 finding (severity, `found_in`, `status` open, closed, documented or rejected, `closed_in`, the upstream `fix` commits and `hunks`, `closed_by` rows, `regression`) records the fix. status-lint checks the tables against the rows: each `closes` entry is a finding whose `closed_by` lists the row; every `closed_by` entry is a property or assumption, and a property there has the finding in its `closes` (an assumption needs none); a closed finding has `closed_in` and a `closed_by` row past `unverified`/`stale`; a `finding` names an open one. Each fix hunk has a fix-revert mutant (FR-n, §4.5) that the closing property's layer must kill. |
| Catalogue | `[project] catalogue = "strategy.md"` makes status-lint compare `status.toml` with §5's rows, read as C-6 reads them: the properties are exactly the rows whose *Layer(s)* is not `assumption` and the `[assumptions.<ID>]` tables exactly the others, with no ID twice; `layer` is the first entry of *Layer(s)* (near-tests: `unit`, `difftest` or `wasm-check`) and every other entry is in `cross`; a §5 subsection's rows share one module, and no two subsections share one; each `status` is what the *Status* cell records for the row's primary layer (§5, *Columns*). "stale; v1.0.0: …", or `Kani: pending (v1.0.0: …)` on a Kani row or an assumption, is `stale`; a bare `Kani: pending` there is `unverified` (a new row) or `stale`; otherwise a result outside the "(v1.0.0: …)" brackets (proved, model-checked, tested, checked; a Kani row needs `bounded proof`) is `in-progress`, or a `verified*` status the cell names, and no result (open, untested, policy) is `unverified`. The cell is read without its markup (`*`, backticks, `_` outside a word), and a result right after "not", "no" or "never" ("not tested") is no result. An assumption's `level` is its cell's "(high)", "(med)", "(low)" or "policy". So a row's `status` and its §5 cell change in the same commit. `tools/scripts/test-check-status.sh` has a failing case per rule, and fails if `status.toml` loses the `catalogue` key, which would turn the check off. |
| `[assumptions.<ID>]` | The 15 rows of §5.14 (ASM-GAS new in v1.1.0). `statement` is §5.14's; `level` is `threat_model.pl`'s, the same as §5.14's. `checked_by` lists properties only: those of `assumption_checked_by/2` and of §5.14's *Evidence* cell that have a result. A check that is not a property (`check-pin.sh`, the wasm surface) is named in `observed`. `observed_on` is rewritten only from the v1.1.0 near-tests and sandbox runs, never by hand. `source` cites the NEAR pages near-approval read, or the upstream docs as `spec:<line>`. |
| Property `assumptions` | The ASMs the row's claim rests on: its tools and models (TOOLS, SPEC, MODEL, LEAN-TLA), the runtime behaviour it relies on, and the environment it assumes (TIME for MC-1 and LIVE-2, which cites MC-1; an honest executor, EXEC, for LIVE-2 and MC-1; enough prepaid gas, GAS, for LIVE-2 and RES-1). Where a §3.2 threat's residual is an ASM, every property that mitigates the threat lists it: EXEC on ASYNC-1, RT-CALLBACK, SH-3 and INV-LEDGER (T-UNKNOWN), OWNER on OBS-1 and INV-WF (T-OBS). Since F-2 is fixed, TLA-LIVE-F2 no longer rests on EXEC as "the honest report fails"; its fairness on the honest `settle(false)` is a premise of the claim. The in-bounds observations of LIVE-1 and TLA-LIVE-1 are premises of those claims, not OWNER. No property lists DEPLOY: T-REDEPLOY's only mitigation is the assumption itself, and no §5 row checks a deployment (§9.3). |

---

## 13. Milestones

| # | Deliverable | v1.0.0 (pin `a4bb5c3`, PR #31) | v1.1.0 (pin `57b3f02`, PR #32) |
|---|---|---|---|
| M0 | Submodule, `status.toml` hashes and pin check, conformance generator and traces | Done: 16 `[source_hashes]`, 212 traces | Done: 27 `[source_hashes]` (`scripts/build.sh` included), the identity check against `6fcbd56`, 216 traces, `coverage.py` |
| M1 | Kani on the real core (§6) | Done: 89 harnesses, 164 covers, 31 mutants, REF-1..3; CI runs 53 harnesses, the local record (`89f99d6`) holds the other 36 and 16 mutants | Done but for CI (`kani`, `kani-record` and `covers` pending on PR #32): 98 harnesses, 45 mutants (N1-N14 the fix-revert ones), REF-1..3; CI runs 50 harnesses, the local record (`2d33d33`) holds the other 48 harnesses and 1 mutant (27 large mutants declared not run) |
| M1.5 | TLA+ (§17) | Done, except TLA-IND (open) | Done, except TLA-IND (open); TLA-CAP and TLA-EXEC new; REF-1..3 (Kani) |
| M1.6 | Lean (§18) | Done | Done (LEAN-LIVE-ALL new) |
| M3 | Shell: `near-tests/`, wasm surface | Done (NT-F1..NT-F4 found) | Done: one build, SURF-2 passes, SCN-1 and EX-6..EX-8 new |
| M4 | Prolog (§8) | Done | Done: seven MC-1 universes, 47 mutants |
| M5 | Verus | Deferred (§7) | Deferred (§7) |
| M6 | Sandbox | Done, on one shard, two builds | Done, on one shard, one build; RT-WINDOW, RT-D1 new; RES-1 equal to api:44-59 |
| M7 | Deployment | Out of scope | Out of scope (§9.3) |
| CI | `.github/workflows/shared-pool.yml` (§14) | Green on PR #31 for every layer but Kani (run 43194245726); the Kani split had not run in CI | Every layer's v1.1.0 job (`tla` adds `run.sh executor`, `conformance` adds `coverage.py`). On PR #32 at `5cf571e` (run 43600054037, hosted runners) `pin`, `status-lint`, `conformance`, `tla`, `lean`, `prolog` and `near-unit` pass; `kani-record` and `covers` failed there until the v1.1.0 Kani record (§14), and two hosted `kani` groups failed on v1.0.0's split (`h_full_256` exhausted the runner's memory; `h_version_max_pause`, `_dispatch` and `_observe` no longer exist at v1.1.0); `kani`, `kani-record` and `covers` with the v1.1.0 record are pending; `sandbox` is manual |

**Pin decision (v1.1.0).** The pin is `57b3f02`, the merge commit of
upstream PR #4 on `main`, whose tree equals the PR head `6fcbd56` where
the fixes were reviewed; `check-pin.sh` checks that identity (§12, §14).
v1.0.0 was pinned at `a4bb5c3`, byte-identical in contract code to
upstream's v1.0.0 commit `6f61616`; that argument does not carry over,
because v1.1.0 changes the core, the contract, the executor and the docs.
The verified release is v1.1.0; v1.0.0's results stay on PR #31's branch
(`a072a2d`), #31's head to be tagged `shared-pool-v1.0.0`. No
`status.toml` row is `verified` until a spec reviewer signs (§12).

**Next, in order.**
1. `kani`, `kani-record` and `covers` must pass on PR #32 with the v1.1.0
   record (§14).
2. Merge PR #31 and create the tag `shared-pool-v1.0.0`; then PR #32.
3. Upstream follow-ups: release tags for v1.0.0 and v1.1.0; the
   executor's job cap (a rollback callback for refused confirmations,
   OQ-12).
4. A named reviewer who wrote neither `pool-core` nor the models signs off
   the specs (`[review] spec_reviewer`).

Closed from v1.0.0's list: the report of F-1..F-6, NT-F1..NT-F4 and the
two doc divergences upstream (fixed in v1.1.0, §4); the re-pin to v1.1.0
with the traces regenerated, the hashes recomputed, every layer re-run,
and the finding witnesses inverted (§12); Kani at v1.1.0 (the measurement,
the CI split, the local record, §6 and §14).

---

## 14. CI (`.github/workflows/shared-pool.yml`)

The workflow is authoritative. Jobs run on pull requests and pushes to
`main` that touch the project. Changes only to documentation (`*.md`,
`docs/`) do not trigger them, except `strategy.md`: C-6 reads its §5, so
the path filters include it again. `sandbox` runs only on
`workflow_dispatch` with `sandbox: true`, and `kani-mutants` and
`kani-mutants-large` only with `mutants: true`. `kani` and `kani-large`
take hours, so they and `covers` run only when the change reaches the
Kani layer: `kani/` (not its Markdown, nor `kani/local/*.py`, the local
runner and recorder; a change to the record's data counts), the gitlink or
`.gitmodules`, `conformance/traces/core.txt`, `tools/versions.env`, a
`.cargo/config[.toml]` at the root, in `projects/` or in
`projects/shared-pool/` (cargo reads those above the crate too), the
scripts they call, or the workflow (the `pin` job's `kani` output; a
manual run sets it with the `kani` input, default on). Every input of the
local record's fingerprint is among these paths. `plain`, `mutants` and
`kani-record` take minutes and run on every change.

Kani's runners. The repository is public, so GitHub's hosted
`ubuntu-latest` has 4 CPUs and 16 GB (15.6 GB usable). The workflow
splits the Kani work by the peaks that `kani/local/run-local.py --all`
measured on the 64 GB Mac (runs 1, 3 and 4, up to three jobs at a time; a
harness they did not measure is sized by W7's measurement, else by
v1.0.0's peak times 1.25; `kani/RESULTS.md`). A peak is resident memory,
which can understate a job's footprint when macOS compresses pages (§6),
and
`tools/scripts/kani-matrices.py` writes the four matrices from those
runs:

| Where | Harnesses | Mutants under Kani |
|---|---|---|
| Hosted runner, in memory | `kani`: the 50 harnesses whose peak, rounded up, is 13 GB or less, in 7 groups | `kani-mutants` (manual): the 17 whose Kani leg (the first `killed-by` harness; every `checked-by` of a must-survive patch) peaked at 13 GB or less |
| Local record, checked by `kani-record` (or `KANI_RUNNER`) | `kani-large`: the other 48 (38 above 16 GB, `h_bmc_from_init_s_submit` at 39.6 GB), in 17 groups, with `h_wedge` (the record's self-test samples it) and `h_full_256` (it filled the hosted runner's memory and swap at v1.1.0) | `kani-mutants-large`: the other 28 (mutant 21 among them): 1 recorded, and 27 declared not run at v1.1.0 in `kani/local/not-run.tsv` (CI's `mutants` job checks them in plain mode) |

On the hosted runner the large groups ran on swap, slowly, and some
never finished (below). So they and the large mutants run on a local
machine. `kani/local/run-local.py` runs
`kani-large`'s groups and `kani-mutants-large`'s mutants from the
workflow's lists, with `run.sh` unchanged and each job's matrix timeout,
or, with `--all`, every harness and mutant as a measurement.
`kani/local/record.py` records the large matrices' jobs in `kani/local`
(`provenance.txt`, `jobs.tsv`, `covers.tsv`, and `not-run.tsv` when a
large mutant is declared not run; `kani/local/README.md`) from one or
more runs at one commit and fingerprint, `--record` runs or `--all`
measurements, complete or stopped early: each job from the latest run
that finished and passed it, so a later failure does not replace a pass
(`kani/local/results-tables.py` takes the same rule for
`kani/RESULTS.md`'s tables). A large harness cannot be declared not run.
The record holds the runs' commit, a fingerprint of the Kani inputs, each job's result, time, peak
and limit (the `KANI_TIMEOUT` it ran under), and which run it came from. The inputs are every file under
`kani/` except `kani/local` and the Markdown (so a new file there, such as
`kani/.cargo/config.toml`, counts), with the git object ids of
`kani/src`, `Cargo.toml`, `Cargo.lock`, `mutants`, `run.sh` and
`.gitignore` on lines of their own; the core's gitlink and
`conformance/traces/core.txt`; the repository's cargo configuration
above `kani/` (none at the record's commit; `run-local.py` copies it into
its lanes); and `tools/versions.env`'s `KANI` and `CBMC`. `run-local.py`
refuses to run when the environment or a cargo configuration file
outside its lanes (parsed as TOML, so any spelling of a key counts) sets
something that changes a verdict but not the fingerprint (`KANI_FLAGS`,
`RUSTFLAGS`, a profile, and the like).

The v1.1.0 record had no `--record` run of its own: `record.py`
assembled it from three `run-local.py --all` measurements at
`2d33d33` (the record's commit, with HEAD's Kani inputs) on
a 10-core, 64 GB Apple M1 Max, runs 1, 3 and 4 of `kani/RESULTS.md` (`run1`
to `run3` in `provenance.txt`, which numbers the runs it was given).
Run 1 (every job, harnesses first) was stopped before it had finished the
harnesses. Run 3 (`--only`, `--estimates-from` run 1's results plus an
estimate for `h_full_256`) measured the harnesses run 1 had not finished
that decide the split, then the large mutants it finished before the
v1.1.0 deadline. Run 4 ran `h_energy_max` alone: it needs the machine to itself, and run 3's scheduler kept starting mutants that fit beside each other. The other 27 large mutants are
declared not run at v1.1.0 in `kani/local/not-run.tsv`: the record has no
Kani result for them, `kani-record` warns for each on every run, and CI's
`mutants` job still checks them in plain mode. A measurement's limit (its
`KANI_TIMEOUT`) is 3 times the job's estimate, at least 1,800 s (7,200 s
for `run.sh`'s long tier); `--estimates-from` raised a harness's limit to
3 times its time there if higher (4,500 s for `h_full_256`), and a
mutant's to at least its harnesses'. So a recorded limit can be above the
timeout a matrix would otherwise get, and `tools/scripts/kani-matrices.py`
writes each `kani-large` and `kani-mutants-large` timeout at no less than
the `limit_s` its job was recorded under, which `kani-record` checks
(below). `kani/RESULTS.md`, Local record, renders the record.

`kani-record` runs on every change, in about a minute and without the
submodule: the self-test `tools/scripts/test-check-kani-record.sh`, then
`tools/scripts/check-kani-record.py`. It fails when:
- the record's fingerprint is not the commit's. The message names the
  changed inputs and says "re-run the local Kani record";
- the data do not match their digest, or the fingerprint is not the hash
  of the record's input lines or (where the clone has it) of the
  record's commit;
- a recorded harness is not `ok`, or a recorded mutant is not as its
  patch expects;
- the `kani` groups and the record do not cover `run.sh list` exactly
  once, or `kani-mutants`, the record and `not-run.tsv` `mutants/*.patch`
  (each declared mutant is also a warning, "NOT RUN locally", on every
  run);
- a `kani-large` or `kani-mutants-large` timeout is below the limit the
  record ran that job under (the workflow is not in the fingerprint; a
  warning when this run's large jobs verify it);
- a hosted entry peaked above 13 GB;
- `kani/RESULTS.md` has no block, or its block is not the record's table.

`covers` adds the record's harnesses and covers to the groups'. CI checks
that the record is consistent and belongs to the commit's inputs, not
that the run took place as recorded (§15).

`kani-large` and `kani-mutants-large` run only when the `KANI_RUNNER`
repository variable names a runner other than `ubuntu-latest` and the run
is not routed to `ubuntu-latest`: the `pin` job's `large` output. That
runner needs at least 50 GB, 1.25 times the largest
`gb` of the `kani-large` and `kani-mutants-large` entries (a peak rounded
up; a mutant's own peak can be above its harnesses'). Those are resident
peaks (§6), so the first `kani-large` run on such a runner is what shows
that it is large enough. They have no
swap step. Their runner-size step fails a group whose peak is above the
runner's memory plus swap, or, on a hosted runner, above its memory. A
stale record is then only a warning if the run verifies everything it
records: `kani-large` its harnesses and `kani-mutants-large` (manual,
`mutants: true`) its mutants. Otherwise it fails as before. So a change
to the Kani inputs needs a new record whatever `KANI_RUNNER` is: until it
is re-made, `kani-record` fails on every pull request and push, and only
a manual run with `kani` and `mutants` on `KANI_RUNNER` passes without
it. When `kani-large` runs, `covers` uses the groups' logs
alone. The Kani jobs take `KANI_RUNNER`, then
`CI_RUNNER`, then `ubuntu-latest`, as near-approval's do. Routed runs
(Dependabot, automated agents) always get `ubuntu-latest`, and so the
record. On hosted Linux, `kani` and `kani-mutants` add a swap file of
`KANI_SWAP_GB` (default 16, bounded by free disk). It is a margin for a
Linux peak above the macOS one: macOS resident memory leaves out
compressed pages (§6). A group that runs out of time fails, it is not
skipped. Per group, `KANI_TIMEOUT` is three times its slowest harness
(at least 1,800 s): in `kani` the hosted time (a CI run's, else 2.5
times the local one), in `kani-large` the local time and at least the
limit the record ran each harness under. `KANI_MAX_GB` is 1.5 times the
group's peak. On a
runner with less memory than the group's peak, each harness may instead
use an equal share of 330 of the job's 360 minutes. No loop-bounds job:
near-approval's U-1 serves its cutoff, and this project claims none;
`run.sh` keeps unwinding assertions on.

v1.0.0 history: before the split, runs 43469880167 and 43488691444 ran
all 29 Kani groups on the hosted runner; 4 and then 2 groups did not
finish on swap, which led to the local record (this section at
`a072a2d`, PR #31's branch, has the details).

| Job | Runs | Fails on |
|---|---|---|
| `pin` | `tools/scripts/test-check-pin.sh` (the identity check on scratch repositories), then `tools/scripts/check-pin.sh shared-pool`, which also fetches each `[project] identity` commit the mirror lacks (PR #4's head `6fcbd56`) by SHA and compares its paths with the pin (§12); the changed paths, for the `kani` output | the self-test fails; gitlink ≠ `pinned_commit`; `[source_hashes]` drift; no `overflow-checks = true`; an identity path not the same tree or blob at its commit and at the pin, or that commit not fetchable; the old `verified_release_commit` / `identical_paths` keys present. Every job but `status-lint` needs it. |
| `status-lint` | `tools/scripts/test-check-status.sh` (each failing case must fail for its own reason), then `tools/scripts/check-status.sh shared-pool` | the self-test fails; evidence not allowed for its layer; a status without its artifact; a status above its ceiling; `closes` and `[findings]` disagreeing (§12, "Closed findings"); `status.toml` and §5 disagreeing on an ID, a module, `layer`, `cross`, a `status` or an assumption's `level` (§12, "Catalogue") |
| `conformance` | `cargo run --release --locked -- --check traces/core.txt`, then `-- --reads-check traces/reads.txt`, then `python3 coverage.py traces/core.txt` | the checked-in traces or reads differ from the generator's output; a read class not reached; `preview` and `submit` disagree; a `REQUIRED` coverage class missing or a `FORBIDDEN` one present (§10.1) |
| `tla` | `tla/run.sh safety liveness trace` (5 min 8 s locally), then `run.sh executor` (TLA-EXEC, about 5-6 min), then `run.sh mutants` (52 mutants, 6 min 57 s); 120 min limit | a check failing; a witness or required violation passing; a mutant surviving |
| `lean` | `lean/run.sh` (about 185 s locally, 178 s of it the 26 mutants); 60 min limit | a `sorry` or forbidden construct; an extra axiom; a canary compiling; the regression file not compiling; a REF-5L difference; a mutant surviving |
| `prolog` | `prolog/run.sh` (1,777 s locally, about 30 min; 47 mutants); 120 min limit | a suite failing; a loader warning; extraction drift; a canary passing; a mutant surviving |
| `near-unit` | `near-tests/run.sh all`: the PROP-tag check, 38 tests on the host and the documented build, the surface (SURF-1, SURF-2, SURF-3), and the 41 mutants (about 22 min locally); 120 min limit | a tag problem (`check-tags.py`); a test failing; lockfile drift; a surface check failing (SURF-2 must now pass); a mutant surviving |
| `sandbox` | `sandbox/run.sh` (manual, `sandbox: true`): one build through upstream `scripts/build.sh`, 13 runtime tests and RES-1 (1,041 s warm locally with 3 test threads; a hosted runner runs the runtime tests one at a time, an estimated 17 min, then RES-1); 90 min limit | any failure; lockfile drift; `build.json` not clean, not at the pin, or not the wasm's hashes |
| `plain` | `kani/run.sh plain`: the harness bodies with a random source, 33 tests, and REF-1..3 on every call of the 216 traces (`h_ref_traces`) and with prunes in `window_walk` (11 s of tests; not proof); 15 min limit | a test failing; a cover that a body evaluates never firing |
| `mutants` | `kani/run.sh mutants`: the 45 mutants in plain mode (394 s locally; about 13 min on the hosted runner); 30 min limit | a must-kill mutant passing plain mode; mutant 20 (`must-survive`) failing it |
| `kani` | `kani/run.sh kani <group>` with `KANI_DEFER_COVERS=1`, a matrix of the 7 groups (50 harnesses) that peaked at 13 GB or less, on the hosted runner unless `KANI_RUNNER` or `CI_RUNNER` is set (see above); per group `KANI_TIMEOUT` and `KANI_MAX_GB`; logs uploaded | a harness failing, out of time or out of memory; `h_canary` passing, or failing on anything but its own assertion; a runner whose memory plus swap is below the group's peak; a runner that shuts down |
| `kani-large` | as `kani`, for the 17 groups (48 harnesses) that peaked above 13 GB, with `h_wedge`; only when `KANI_RUNNER` is set, is not `ubuntu-latest`, and the run is not routed (the `pin` job's `large`); no swap step | as `kani`; a hosted runner with less memory than the group's peak |
| `kani-record` | every change: `tools/scripts/test-check-kani-record.sh`, then `tools/scripts/check-kani-record.py` on the local record in `kani/local` (see above; `--stale-ok-harnesses` when `kani-large` runs, `--stale-ok-mutants` when `kani-mutants-large` runs) | the record's fingerprint not the commit's ("re-run the local Kani record"), unless this run's large jobs verify all it records; data not matching the digest, or a fingerprint not its input lines' or its commit's; a recorded harness not `ok` or a recorded mutant not as its patch expects; `kani` plus the record not `run.sh list` once each, or `kani-mutants` plus the record and `not-run.tsv` not `mutants/*.patch` once each; a large entry's timeout below the record's limit for it; a hosted entry above 13 GB; a missing or stale `kani/RESULTS.md` block |
| `covers` | after `kani`, `kani-large` and `kani-record`: every harness of `run.sh list` in exactly one group's logs or (without `KANI_RUNNER`) the local record, then `tools/scripts/check-covers.sh` over every group's `covers.tsv` and the record's covers (98 harnesses and their covers) | a harness in no group nor the record, or in two; a family's cover satisfied in none of its harnesses |
| `kani-mutants` | `kani/run.sh mutants --kani <NN>-*`, one job per mutant for the 17 whose Kani leg peaked at 13 GB or less, with the leg's peak and its harnesses' `KANI_TIMEOUT`, at least 1,800 s (manual, `mutants: true`) | a mutant whose plain or Kani result differs from its header (`killed-by`, `must-survive`) |
| `kani-mutants-large` | as `kani-mutants`, for the other 28, only when `KANI_RUNNER` is set, is not `ubuntu-latest`, and the run is not routed (manual, `mutants: true`); otherwise they are the local record's, or declared not run (`not-run.tsv`) | as `kani-mutants`; a hosted runner with less memory than the mutant's peak |

---

## 15. Risks

| Risk | Mitigation |
|---|---|
| The pin is not the commit the fixes were reviewed at. | The pin is the release: `57b3f02`, the merge of PR #4 on upstream `main`. Its tree equals the PR head `6fcbd56`, and the `pin` job fails if that stops being true or either commit cannot be fetched (`check-pin.sh`, §12, §14). A squash merge or a moved head would fail it loudly. |
| Three hand-written models share a transcription error. | All replay the same real-core traces; C-2 compares guard orders; every layer has mutants; REF-1..3 (Kani) tie TLA+ to the code per step, within the step bounds (§17.2). |
| The ghost semantics differ between models (the pruned prefix of the ledger, the refresh and `base`/`k0`, the saturation of `completed_units`). | One definition in the §5 notation, frozen before any layer started. The traces include refreshes with records open (207, 211) and a re-base (213), so every replayer must agree on them. |
| "Fails on v1.0.0" asserted but not shown. | Each closed finding has v1.0.0's result (PR #31's branch, `a072a2d`) and a fix-revert mutant that its closing property's layer kills at v1.1.0 (§4.5). |
| Trace renumbering breaks every "trace N" cross-reference. | Fixtures chosen by name; traces 0-211 keep their numbers and the new ones are appended (§10.1). |
| Doc citations point at deleted v1.0.0 text. | `spec@v1.0.0:<line>` for text v1.1.0 deleted; every other cite re-anchored to the pin with a line map and reviewed by hand. |
| `status.toml` and §5 drift apart. | C-6 checks §5 against `traceability.pl`, and status-lint checks `status.toml` against §5: IDs, modules, layers, statuses against the *Status* cells, and the assumptions' levels (§12, "Catalogue"). It cannot check that a cell's words are true, only that the two files say the same; `check-status.sh` also checks each row's evidence, artifact and ceiling. |
| The Prolog author read the code (ASM-SPEC). | Doc citations on every fact; a named independent reviewer. |
| Kani is slow on `Vec`-heavy state, and the window adds 256-record vectors. | Fixed small shapes, step parts, per-loop unwinding; pruning only in the concrete `h_full_*` harnesses (§6). The measurement sized every job; the harnesses above 13 GB, and their mutants, are a local record (§6, §14). |
| The Kani split's peaks are resident memory, which understates a job's footprint when macOS compresses pages (§6). | A hosted harness's peak, rounded up, is 13 GB or less, which leaves 2.6 GB of the runner's 15.6 GB, and the `kani` job adds a swap file; it runs every hosted harness whenever a change reaches the Kani layer, and one that does not fit fails, it is not skipped. A `KANI_RUNNER` needs 1.25 times the largest large peak; its first `kani-large` run shows whether that is enough (§14). |
| CI does not run the large Kani harnesses and mutants; it checks their local record (§14). | The record names the commit, the tool versions and the machine, and fingerprints the Kani inputs: any change to them fails `kani-record` until the run is repeated. It cannot show that the run took place as recorded. Large mutants declared in `kani/local/not-run.tsv` have no Kani verdict at v1.1.0, only plain mode's; the check warns on each. |
| Kani's bounds are small: 1 metric, 1 term, 2 recipes and 2 records per step. | Plain mode at larger shapes, Lean at every size, the traces at 256 records and ±1,000,000 (ASM-MODEL). |
| Kani or CBMC is unsound for a construct the harnesses use; two hazards were found in Kani 0.68.0, one about references at a symbolic offset, which v1.1.0's `record(id)` returns. | Canary, covers and mutants; both hazards worked around and documented (`kani/RESULTS.md`, Tool issues), with a scan of every goto program for the first (ASM-TOOLS). The hazard concerns local arrays in harness code, not the core's heap vectors; `spec.rs` and `tla.rs` must keep returning copies. |
| TLC's 32-bit integers and small universes. | Overflow aborts a run, so no run passes silently. The cap is scaled (`VersionCap` = 2^31 − 7 in `MC_Cap`), the window is 1 or 2 records; Lean covers every size and large values, and the traces and reads the real cap and window (§17.5). |
| TLC memory on a GitHub-hosted runner. | Closed for v1.1.0: the `tla` job, `run.sh executor` included (`MC_Executor_W3`, 7,323,777 distinct views), passed on a hosted runner at `TLA_HEAP=5g` on PR #32 (run 43600054037, 22 min). |
| One-shard sandbox | RT-RACE records what one shard cannot show. The models treat ack and settle as independent steps, and TLA-EXEC every receipt order. |
| Machine contention: the Kani measurement holds a 64 GB machine for hours. | One heavy job at a time on the shared machine; the layers' run times in their `RESULTS.md` were measured on a machine shared with other jobs. |

---

## 16. Open questions

| # | Question |
|---|---|
| OQ-1 | Closed (v1.1.0): re-pinned to `57b3f02`, PR #4's merge, with every layer re-run. v1.0.0's results (pin `a4bb5c3`) stay recorded on PR #31's branch (`a072a2d`). |
| OQ-2 | Closed (v1.1.0): upstream chose the resolution envelope. A reservation never changes a metric's violation amount (handoff:32), and the envelope over every open record's completion or release must not exceed the current violation (§4.1, F-1, F-2). |
| OQ-3 | Closed (v1.1.0): `scripts/build.sh` sets `--cfg near` and checks that the wasm imports `panic_utf8` (NT-F1; SH-5, SURF-3). |
| OQ-4 | Closed (v1.1.0): build.sh checks both wasm files' imports against a reviewed list, which excludes deploy, key, transfer, stake, account and global-contract actions (NT-F3; SH-12). |
| OQ-5 | Closed (v1.1.0): `validate` refuses actor = executor (F-4; CFG-2). The docs forbid only actor = executor (spec:11, api:65), so the owner may be a recipe's executor: a documented limit (§4.6). |
| OQ-6 | Closed (v1.1.0): F-3 by the 256-record window with pruning of terminal records (`Full` only while the oldest is open; WIN-1, WIN-ESC, WIN-LIVE); F-6 by the refresh, an `observe` of the current values with records open (REFRESH-1, OBS-2). |
| OQ-7 | TLA-IND: rewrite the model for Apalache, or accept LEAN-IND as the every-size safety result? |
| OQ-8 | Closed: moot. A reservation never changes a violation amount, and LIVE-ALL makes cancel and both settles Ok in every reachable state (L-1b, §4.8). |
| OQ-9 | Closed (v1.1.0): spec:41 and api:30 describe the no-op acknowledgment, api:40 the non-string results (NT-F4), and api:44-59 give the prepaid gas per method. RES-1 reproduces every figure (§9.2); the executor's `finish` needs 55 Tgas at v1.1.0 (v1.0.0: 43). |
| OQ-10 | Who reviews the specs? |
| OQ-11 | Closed (2026-11-13): Kani in CI, swap or a large runner? Neither, by default: the harnesses that fit the 16 GB hosted runner run there, the others and their mutants are run locally and recorded, and CI checks the record (§14). `KANI_RUNNER` still runs them in CI when it is set. The v1.1.0 split comes from the measurement of every harness and mutant (§6, §14). |
| OQ-12 | The executor's job cap (upstream D-1): only refused or in-flight confirmations can fill it (TLA-EXEC `InvCapShape`), and its exits are documented (api:42, handoff:39; §4.6). Should upstream add a callback that rolls back a refused confirmation's job? An upstream follow-up. |
| OQ-13 | Closed by TLA-EXEC: should the lock's and the executor's windows be model-checked together for EX-7, or stay tests? Model-checked at WINDOW 2 and 3, every receipt in any order; the tests keep the concrete 256 and 512. The checked invariant deviates from the plan's wording ("a retained in-flight record's job is present"), which is false in reachable states where an in-flight record's `execute` panicked, failed "full" or has not arrived. `InvRetainedJob` states it as: the executor never forgets the job of a record the lock retains (EX-7, T-FORGET); a record whose execute never made a job has none to keep. |

---

## 17. TLA+ over every event sequence

Kani proves one step of the real core within bounds (§6). That says nothing about liveness
or safety at depth. The TLA+ model is checked over all event sequences in
small universes, and tied to the real core by the traces (REF-5) and,
within the step bounds, step by step (REF-1..3, §17.2; Kani). Results: `tla/RESULTS.md`. TLC 2.19 (tla2tools 1.7.4), OpenJDK 23,
4 workers, on a 10-core Mac shared with other jobs; runs on 2026-11-13
and 2026-11-14, the trace check and mutants again at `a7781c7`.

### 17.1 The model (`tla/`)

| File | Contents |
|---|---|
| `SharedPool.tla` | The pool as variables (`cfg`, `values`, `version`, `observedAt`, `paused`, `records`, `firstId`, `doneU`). No action writes the config, so one run replays traces of many configs. Record `k` has id `firstId + k − 1`. Ghosts: `base`, `k0` (a pruned counted record is folded into `base`), `steps`, `last`. One error operator per entry point gives the first failing guard in code order, `"Noop"` or `"Ok"`; an action is enabled exactly when it gives `"Ok"`, so rejected and idempotent calls stutter. `GuardOrder` lists each entry point's exits. `ValidCfg` transcribes `validate`. `InitAt(c, t0, v0)` is `Pool::new` at version `v0`. |
| `Props.tla` | `InvRel` (view-safe: INV-WF, INV-LEDGER, INV-RES, INV-HARD, INV-DEP, INV-VER, TERM-1, LIVE-ALL, LIVE-1, `NoWedge`, WIN-ESC, WIN-2, REFRESH-1); `InvAbs` (INV-VERCAP, WIN-1's `firstId + NR <= version`); `InvOutcomes` (INV-HEALTHY over every outcome combination). `StepProperty`: config fixed, TX-VER, VER-CAP, LIFE-1 with the prune, STALE-1, AUTH-1, ADM-1, ADM-2, HARD-STEP, DEP-1, `completed_units`, ASYNC-1, OBS-1, OBS-2, TX-SUBMIT, TX-DISPATCH, PAUSE-1. The witnesses. Properties restate the conditions they rely on rather than reuse the model's operators, so a mutant of an operator is checked against the statement. |
| `MCCommon.tla` | Four configurations: `MCConfig` (the F-1 wedge scaled down), `CreditHedgeConfig` (F-2, the `risk` fixture's credit and hedge), `RecoveryConfig` (the admitted recovery hedge of traces 214 and 215) and `F2bConfig` (F-2b, values at ±10^6). Accounts o, a1, x1, z. |
| `MC_Safety.{tla,cfg}` | TLA-SAFE and TLA-WIT on `MCConfig`, MaxRecords 2, times {0,1,2} per call, expiry offsets {0,1,2,86400,86401}, 14 observation vectors. A `VIEW` drops the unbounded `version`, `steps` and `firstId` (ids relative to `firstId`). `MC_Safety_1.cfg` (MaxRecords 1), `MC_Recovery.cfg` and `MC_F2b.cfg` reuse the module. |
| `MC_Cap.{tla,cfg}` | TLA-CAP: `Pool::new` at `VersionCap - 4`, no VIEW, every invariant (the absolute ones included) and `StepProperty`. Every behaviour reaches the cap. |
| `MC_Liveness.tla` + 7 cfgs | LIVE-2 and WIN-LIVE on `MC_Cap`'s behaviours, under named fairness assumptions. |
| `TraceCheck.{tla,cfg}`, `trace2tla.py` | REF-5: all 216 traces in one run; `Inv` and `StepProperty` on every core state and step; trace 210's config must fail `ValidCfg`. |
| `Executor.tla`, `MC_Executor_W{2,3}.cfg` | TLA-EXEC: the executor's job window against the lock's record window, every receipt in any order. |
| `mutants/*.patch` | 52: 46 core-model mutants (32-48 the FR mutants t32-t48, §4.5) and 6 executor mutants (49-54) |

**Scaled constants.** `MaxRecords` (256): 2 in `MC_Safety`, `MC_Cap`,
`MC_Recovery`, `MC_F2b` and the liveness configs, 1 in `MC_Safety_1` and
`MC_Liveness_Win`, 256 in TraceCheck. `MaxVersion` (u32::MAX) is
2^31 − 1, so `VersionCap` = 2^31 − 7 at MaxRecords 2 and 2^31 − 769 in
TraceCheck. `DoneCap` (u32::MAX) is 1 in the small universes and 2^31 − 1
in TraceCheck. `MAX_VALUE`, `MAX_UNITS`, `MAX_WEIGHT` and the 86,400 s
window keep their real values.

### 17.2 The refinement link

REF-1..3 are bounded proofs on the real core (`kani/RESULTS.md`, REF-1..3,
v1.0.0 deviation 26 and v1.1.0 deviation 7). `kani/src/tla.rs` transcribes `SharedPool.tla`
operator by operator from the `.tla` text, not from `spec.rs` or the
code: the state image, the abstraction α (a `Pool` and its ghost `base`
and `k0`, mapped to the TLA+ variables), the error operators `SubmitErr`
to `ObserveErr` in `GuardOrder`'s order, and the actions. The
`h_ref_<op>` harnesses use the step harnesses' generators, so they share
their bounds: every INV pre-state with 1 metric, 1 term, 2 recipes and
exactly 2 records, and every operation of the step domains (§6). For each
call they assert:

- **REF-1 (guard exactness):** the core's error is the error operator's
  value on α(pre), and the core is Ok exactly when the operator gives
  "Ok" (the action is enabled) or "Noop" (an idempotent call).
- **REF-2 (effect exactness):** when the operator gives "Ok", α(post) is
  the action's post-state, configuration and ghost included; submit
  returns the new record's index.
- **REF-3 (stutter):** every other call leaves α unchanged.

`kani/run.sh ref` (exit 0, before the measurement) and the measurement
runs: 7 of 7 `ok`, 57 to 509 s each, at most 15.1 GB. The 43 `ref` covers, every operator
value the step domain reaches, are SATISFIED (v1.0.0 had 46: v1.1.0 makes
ack, cancel and settle `Arithmetic` and cancel and settle `Bounds`
unreachable on INV states, and adds cancel `InvalidState` of a Completed
record and the observe refresh); `Full` needs 256 records and is reached
only on the traces. Plain mode runs the same check on every sample of the
step tests, with prunes and the ghost fold in `checked_step`
(`window_walk`, `cap_walk`, the f-tests), and on every call of the 216
traces (`h_ref_traces`, 8,865 calls: "Ok" 2,070 times, "Noop" 495, and
each of the 15 errors the traces contain; 3 prunes folded into the
ghost). Mutant 31, an error in `tla.rs`'s `ReleaseErr`, is killed by
`h_ref_cancel`.

**Limits.** The proofs hold within the step bounds only; other shapes and
record counts are tested, in plain mode and on the traces. `tla.rs` is a
hand transcription of the `.tla`. It meets TLC only on the traces, where
both agree with the core (REF-5), and so with each other. The history
variables `steps` and `last` are not mapped, and `Init` is linked by
CFG-1 and the traces, not by a REF harness.

### 17.3 What is checked

At v1.0.0, LIVE-2 depended on what the executor was assumed to do: it held
on the completion path, and failed with a disturbing observe (F-1) and
with an executor fair only to the honest `settle(false)` (F-2). At v1.1.0
LIVE-ALL is an invariant of every reachable state of every universe
(`MC_Cap` at and above the cap included), so every fairness assumption
that makes some resolving call happen gives LIVE-2:

| Check | Assumption | Result |
|---|---|---|
| TLA-LIVE-1 (`liveness`, `liveness-f2-done`) | Observes in bounds; the executor weakly fair to completing each record. | Holds in `MCConfig` and `CreditHedgeConfig`. Without fairness (`liveness-unfair`) it must fail, and it does. |
| TLA-LIVE-F1 (`liveness-f1`) | Disturbing observes allowed (F-1's setting); the executor weakly fair to settling either way. | Holds (v1.0.0: violated by the F-1 wedge). |
| TLA-LIVE-F2 (`liveness-f2`) | `CreditHedgeConfig`; the executor weakly fair to `settle(false)` only, callers to cancelling. | Holds (v1.0.0: violated by the F-2 wedge). The hedge that relied on the credit's reservation is refused at submit. |
| WIN-LIVE (`liveness-win`, `liveness-win2`) | MaxRecords 1 and 2; weak fairness of resolving the oldest record (cancel, or settle either way). | A `Full` window is eventually not `Full`. |

v1.0.0's `MC_Liveness_F1Shape` and `MC_Liveness_F2Shape` (every
counterexample is the wedge) are removed: there is no counterexample to
classify.

### 17.4 Results

- **TLA-SAFE:** `MC_Safety` 435,342 distinct views (12,091,340
  generated), depth 20, 186 s; `MC_Recovery` 92,444 states; `MC_F2b`
  30,904; `MC_Safety_1` 11,222 (v1.0.0: 2,306,738 states at MaxRecords 3).
- **TLA-CAP:** `MC_Cap` 64,644 states, depth 9, 28 s.
- **TLA-WIT:** 20 witnesses, each violated as required (traces of 2-9
  states): `Witness_Completed`, `_Prerequisite`, `_Disturbed`, `_Prune`,
  `_FullOpenFront`, `_CancelCompletedInvalid`, `_SubmitRejectedHull`,
  `_EdgeValue`, `_F2bRefused`, `_AtCap` and the step witnesses
  `WitStep_CancelledAfterDispatch`, `_UnknownToActive`, `_Refresh`,
  `_PrereqAfterPrune`, `_PausedSettle/Ack/Cancel/Observe`, `_Recovery`,
  `_ResolveAtCap`. v1.0.0's `Witness_Wedge` is now `NoWedge`, an
  invariant.
- **Liveness:** as in §5.11: `liveness` 7,856 states, `liveness-f2-done`
  5,202, `liveness-f1` 7,918, `liveness-f2` 5,202, `liveness-win` 1,868,
  `liveness-win2` 8,228.
- **REF-5:** 9,081 of 9,081 states (216 initial + 8,865 steps), in 9 s.
- **TLA-EXEC:** WINDOW 2 3,880,089 distinct views (depth 33, 86 s), WINDOW
  3 7,323,777 (depth 30, 184 s), 12 witnesses reachable (the job-cap
  refusal among them); the quotient cross-check `executor-w2-fullview`
  13,996,226 states, no error.
- **REF-1..3 (Kani):** 7 of 7 `h_ref_<op>` `ok`, 43 of 43 `ref` covers
  (§17.2).
- **Mutants:** 52 of 52 killed (46 core, 6 executor), 6 min 57 s at
  `a7781c7`. v1.0.0's 29 and 31 are retired: they required F-1's and F-2's
  violations. Only the trace check kills 01, 06, 14, 26, 28 and 39;
  23, 24, 34 and 37 are killed by the witness whose behaviour they remove
  (24, 34 and 37 also by a trace).
- **v1.0.0 evidence.** Stated in v1.0.0's vocabulary and checked once on
  the v1.0.0 model (`a072a2d`), each new property fails there, as
  required: LIVE-ALL (3 states), `NoWedge` (4), REFRESH-1 (2), WIN-LIVE
  (temporal). The same model's `MC_Liveness_F1` and `MC_Liveness_F2`, now
  must-hold, are violated (F-1, F-2).
- **Run times:** `run.sh` (default: safety, liveness, trace) 5 min 8 s
  locally (`MC_Safety` 181 s); `run.sh executor` about 5-6 min; the
  mutants 6 min 57 s. The `tla` job on a hosted runner (`TLA_HEAP=5g`)
  took 22 min on PR #32 (run 43600054037).

### 17.5 Limits

- **TLA-IND is open.** Apalache 0.47.2 rejects the recursive sums and the
  symbolic ranges; `run.sh apalache` prints `skip`. Every-size safety
  comes from Lean.
- **Small universes.** `MC_Safety`: 2 metrics, 1 term, 2 recipes, at most
  2 records (v1.0.0 explored 3), 4 accounts, 3 block times, 14 observation
  vectors. `MC_Cap` and liveness: at most 2 records (WIN-LIVE at 1 and 2).
  Larger configs and the real window are covered only by the traces.
- **The prune at scale** meets the real code only in trace 207
  (TraceCheck at 256 records).
- **TLC integers are 32-bit.** TLC stops with "Overflow" rather than
  wrapping. No run reaches `MaxVersion` (INV-VERCAP keeps the plain bump
  below it, checked in `MC_Cap` from 4 below the cap). The 7 request
  versions of u32::MAX in the traces are mapped to −1 and still give
  `Stale` (every state version is ≤ 518). Energy at large residuals
  (NUM-1) is Kani's (§6).
- **Not modelled:** `preview` and `best_quantity`, the NEAR shell, and
  promise interleavings beyond independent ack and settle steps
  (ASM-INTERLEAVE); the executor only in `Executor.tla`.
- **Executor model (TLA-EXEC):** ids 0..6; Dispatched, Active and Unknown
  collapsed into one in-flight status; one executor; a lock that never
  refuses an in-flight record's settle (it relies on LIVE-ALL and
  INV-VERCAP); no foreign callers; safety only, so no liveness of the
  job-cap exit (§4.6); `InvConfSettles` at WINDOW 2 only; the VIEW is a
  hand-argued quotient, sound if `InvJobWindow` holds, which the same run
  checks.
- **Refinement to the code** is REF-5 (traces) and Kani's REF-1..3, which
  link the step at `first_id = 0` (§17.2).

---

## 18. Lean: the core's model at every size

Results: `lean/RESULTS.md`. Lean 4.34.1, core library only (no Mathlib).

Every other layer is bounded somewhere. The Lean layer proves the
transcribed core's safety and LIVE-ALL for every number of metrics,
recipes, terms, records and accounts, any config `validate` accepts, and
any times and arguments.

### 18.1 The model (`lean/`)

`PoolLean/Machine.lean` transcribes the v1.1.0 `lib.rs`. Each call is its
guard list in code order, then `effect`. The first guard that fires is the
outcome; errors and no-ops stutter. Values are `Int`, counters `Nat`, and
accounts a type parameter with decidable equality.

**History encoding.** `records` is the whole history (record `j` has id
`j`), and `firstId` is the code's `first_id`; the code's window is
`State.win = records.drop firstId`. The lookups read only window
quantities, and the envelope and observe's "some record is open" equal
their window values because a pruned record is terminal (`envS_win`,
`anyOpen_win`). `completed_units[r]` is `doneU s r`, the history's
Completed units of `r` saturated at u32::MAX. `bump_new` (the cap guard,
then the plain bump) gates submit, dispatch, set_paused and observe; ack,
cancel and settle use the plain `checked_add`. Ghosts: `base` and `k0`
(values and next id at the last re-basing `observe` or `new`; a refresh
keeps them) and `eff` (effective transitions).

`Inv` (`Props.lean`) is INV-WF (with WIN-1 and WIN-2) ∧ INV-LEDGER ∧
INV-RES ∧ PV_i(values) ≤ B_i ∧ INV-DEP ∧ INV-VER ∧ INV-VERCAP; INV-HARD is
the corollary `hard_of_inv`. The modules are `Machine`, `Props`, `Lemmas`,
`Facts`, `Inv`, `StepProps`, `Live`, `Calls`, `Window`, `Guard`, `Witness`
and `Text`. Two executables, split at `6b120fd`: `Main.lean` is
`pool-lean` (Machine and Text only; `replay`), and `Guards.lean` is
`pool-lean-guards`, which prints `guardOrder` for LEAN-GUARD.

### 18.2 What is proved

| ID | Theorems | Statement |
|---|---|---|
| LEAN-IND | `inv_init`, `inv_step`, `reach_inv`, `hard_of_inv`, `bump_ok`, `remSum_le` | `Valid cfg ⇒ Inv (init cfg now)`; `Inv s ⇒ Inv (step s op)` for every op, with no validity premise on the step |
| LEAN-LIVE-ALL | `res_sub`, `close_ok`, `live_all`, `live_all_reach`, `no_wedge` | LIVE-ALL with no premise beyond `Valid` and `Inv`: a Reserved record's cancel by its actor and by the owner, and an in-flight record's settle either way, pass every guard; every open record of every reachable state is made terminal by one Ok call |
| LEAN-LIVE1 | `live1`, `live1_exec`, `live1_reach` | LIVE-1 as a corollary of LIVE-ALL; v1.0.0's premises (B = 0, version < u32::MAX) are gone |
| LEAN-STEP | `stepProps`, `stepProps_step`, `exec_auth`, `term1`, `term1_cancel`, `stale1_no_replay`, `version_mono_run` | From every state, with no `Inv` premise: TX-0, TX-NOOP, TX-VER, LIFE-1 with the prune, AUTH-1, STALE-1, ADM-1, ADM-2, HARD-STEP, ASYNC-1; TERM-1 for settle and cancel |
| (also) | `dep1`, `dep1_iff`, `obs1`, `obs2`, `refresh_ok`, `cap_stops`, `pause1_blocks`, `pause1_transparent`, `healthy` | DEP-1 and its converse (pruned completions count); OBS-1, OBS-2; REFRESH-1; VER-CAP; PAUSE-1; INV-HEALTHY over every outcome combination |
| WIN-1, WIN-2, WIN-ESC | `win1`, `pruned_terminal`, `pruned_notFound`, `full_iff_front_open`, `submit_not_full_of_front_terminal`, `win_esc`, `submit_prunes` | At most 256 retained, ids `firstId + k`, `firstId + len ≤ version`; a pruned record is terminal and every call naming it is `NotFound`; `Full` iff 256 retained and the front open, and one Ok call resolves the front |
| CFG-2 | `valid_iff`, `valid_actor_ne_executor` | `validate` decides `Valid`; a valid recipe's actor is not its executor |
| LEAN-GUARD | `Guard.guards_order`, `Guard.outcome_mem` | Every call's outcome codes are exactly `guardOrder`, written from `lib.rs` at the pin with line numbers. With the sites that cannot fire in an `Inv` state dropped, it equals Prolog's `guard/5` for all eight call kinds (a script over `pool-lean-guards`, not in `run.sh`) |
| LEAN-WIT | `Witness.lean` | Inverted, by `decide` and `rfl` in the kernel (below) |
| REF-5L | `pool-lean replay` | 216 traces (1 refused at init), 8,865 calls (2,565 Ok); every result and post-state equal |

**LEAN-WIT.** On the corpus's configs: `f1_rejected` (trace 0: after
`observe [12,5]` the F-1 hedge is rejected `Bounds`; v1.0.0 admitted it
and wedged) and `f1_envelope` (the hull that rejects it);
`recovery_admitted` (trace 214); `res_needed` (the v1.0.0 wedge state
satisfies every conjunct of `Inv` but INV-RES, and both its settles fail
`Bounds`) and `live_all_needs_res`; `f2_rejected` (trace 209);
`self_executor_invalid` and `owner_executor_valid` (traces 210, 213);
`cap_reach`, `cap_instance` and `cap_drain` (a reachable state at
`VERSION_CAP` = 4,294,966,527 where submit, dispatch, set_paused and
observe fail `Arithmetic`, and both records are resolved past the cap);
`refresh_instance` (trace 211). The healthy side is not vacuous:
`live1_instance`, `healthy_settle`, `unknown_settle`, `cancelled_reach`,
`stale_instance`, `cancel_completed_instance`. v1.0.0's `f1_forever` (the
wedge permanent) is gone.

### 18.3 Hygiene gate and mutants

- **Axiom audit** (`Audit.lean`): 65 of 65 headline theorems use only
  `propext`, `Classical.choice` and `Quot.sound`; 189 theorems in all.
- **Canaries** must fail to compile, and do: `canary/False.lean`,
  `canary/NotInv.lean` (every state satisfies `Inv`) and
  `canary/LiveAllNoRes.lean` (LIVE-ALL with INV-RES dropped from `Inv`).
- **Regression** must compile, and does: `regression/Live1NoPremise.lean`,
  v1.0.0's canary with its statement unchanged (LIVE-1 without B = 0). It
  failed to compile on v1.0.0 (F-1, F-5) and is `live1` at v1.1.0.
- **Scan.** With comments stripped, there is no `sorry`, `admit`, `axiom`,
  `native_decide`, `implemented_by`, `extern`, `unsafe` or `opaque`.
- **Mutants.** 26 mutants of `Machine.lean`: v1.0.0's 16, re-anchored,
  and 17-26, each restoring one v1.0.0 behaviour (FR-1..FR-8, §4.5), with
  a property probe (`mutants/probes/`, 10 probes) that must fail to
  compile unmutated and compile on the mutant. All 26 are killed by the
  build.
- **Run times** (at `6b120fd`): build 10 s, audit 1 s, canary 1 s,
  regression under 1 s, replay 1 s, mutants 178 s; the whole `run.sh`
  185 s.

### 18.4 Gaps

- **Transcription.** `Machine.lean` is transcribed by hand. REF-5L found
  no disagreement on 216 traces. That is evidence, not proof. The history
  encoding is a representation choice; `envS_win`, `anyOpen_win` and
  `win1` show the guards' quantities equal the window's.
- **Numbers.** The model computes exactly in `Int` and `Nat`; fixed-width
  overflow in the code is NUM-1 and NUM-2 (Kani). The code's `energy`
  `Arithmetic` error is not modelled. The envelope's fail-on-first partial
  sums are modelled by the final sums: argued, not proved. The u32
  version (both bumps) and the saturation of `completed_units` are exact.
- **Types.** Times, units and versions are unbounded `Nat`. `max_age`'s
  u32 type and all strings are not modelled.
- **Panics (ROB-1).** Not modelled. `Inv` shows every index the guards
  read is in range, but not that the Rust never panics; Kani checks that
  on the real core within its bounds (ROB-1).
- **Not stated in Lean:** RD-PREVIEW, RD-BEST and the shell; LIVE-2 and
  WIN-LIVE, which are temporal (LIVE-ALL and WIN-ESC are their per-state
  halves); TX-SUBMIT..TX-OBSERVE as separate theorems (the model's
  definitions plus REF-5L).
