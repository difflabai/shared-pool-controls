# shared-pool: Kani layer results (bounded proofs on the real `pool-core`)

**Subject: shared-pool-controls v1.1.0.** Whether its Kani layer is
verified is the summary's **Verdict** line below (generated, by the
Status rule). Pin: difflabai/shared-pool-controls
`57b3f02d18e85149db3c1035431368c4e07a4d8a` (the merge commit of upstream
PR #4; its tree is the PR head `6fcbd56`), `crates/core/src/lib.rs`
sha256 `c3bb825b91f7c277dee8651beafd45955f7cf8935c858f928b6a1b62d607ba48`
(the `kani/src/lib.rs` header). The crate depends on the pinned
submodule's `pool-core` with `default-features = false` (zero
dependencies): Kani checks the deployed decision logic itself, not a
transcription. Tools: Kani 0.68.0, CBMC 6.11.0 (CaDiCaL), rustc 1.97.0
for plain mode and the mutants' plain half (`tools/versions.env`;
`overflow-checks = true` in release, as upstream: ASM-COMPILE). Machine:
Apple M1 Max, 10 cores, 64 GB, macOS, shared with other agents (one heavy
job holder at a time, `/tmp/sp-heavy.lock`). Run dates 2026-11-14.
v1.0.0's results (pin `a4bb5c3`, 89 harnesses, 31 mutants, the local
record at `89f99d6`) are this file at `a072a2d`.

Runner: `projects/shared-pool/kani/run.sh plain | kani [h...] | ref |
mutants [--kani] [glob] | all | list` (exit 0 iff every check passes;
`ref` runs the seven `h_ref_*` harnesses). The measurement and the local
record: `kani/local/run-local.py` (`--all` measures every harness and
mutant; `--record` runs the large matrices), `kani/local/record.py`, and
`kani/local/results-tables.py`, which writes the generated blocks of this
file from the run directories (below).

**Status.** The summary below and the harness, mutant and cover tables
("Kani results", "Mutants", "Covers") are `kani/local/results-tables.py`'s
output from the run directories, and the "Local record" block is
`kani/local/record.py`'s; every count, time and peak of the v1.1.0 Kani
runs is in them, and the prose around them gives none of its own. The
Kani layer is verified at v1.1.0 when the summary's Failures line is
"none", no harness is "not run" or "pending", and CI's `kani` and
`kani-record` jobs pass on the commit; a large mutant declared not run is
a gap of the Kani half of its verdict only, and stays listed. The
summary's Verdict line applies this rule to the tables. It cannot see
CI, so it names CI's two jobs as the condition, and it says "open" until
the tables are written for the ended runs (`--final`). The summary:

<!-- kani-results-summary: begin (kani/local/results-tables.py writes this block; do not edit) -->
**Verdict** (the Status rule; the Kani half of a declared mutant's verdict is a gap, not a failure): verified within the bounds, provided CI's `kani` and `kani-record` jobs pass on the commit: no failure, and every harness verified on this machine (74), by W7 on the same Kani harness code (4) or left to CI's hosted `kani` job (20).

**Harnesses** (98, `run.sh list` at `c8dd19f`): 74 verified as required on this machine (run 1: 35, run 3: 38, run 4: 1); 4 only in W7's probes and `run.sh ref` on the same Kani harness code; 20 not measured here, CI's hosted `kani` job runs them on every change.

**Mutants** (45, `mutants/*.patch` at `c8dd19f`): plain mode as expected for all 45 at `6b120fd` (`run.sh mutants`; CI's `mutants` job reruns every one). Under Kani: 1 as expected on this machine (run 3: 1); 1 earlier (W10, mutant 31); 16 not run here, CI's hosted `kani-mutants` job's (manual runs); 27 declared not run at v1.1.0 (`kani/local/not-run.tsv`; plain mode only): 01, 03, 05, 06, 10, 11, 12, 15, 16, 18, 19, 21, 23, 25, 28, 29, 30, 32, 33, 35, 36, 37, 40, 41, 42, 44, 45.

**Split** (the workflow's matrices; hosted runner: `ubuntu-latest`, 16 GB): `kani` 50 harnesses in 7 groups; `kani-large` 48 harnesses in 17 groups; `kani-mutants` 17 mutants; `kani-mutants-large` 28 mutants.

**Split check** (each job in one matrix; a job measured here, peak the larger of CBMC's and the tree's, rounded up, against the 13 GB line): every job of `run.sh list` and `mutants/*.patch` is in exactly one matrix and every matrix name is one of them; the 26 measured here that are in `kani` or `kani-mutants` peaked at 13 GB or less.

**Failures**: none: every harness measured here as required and every mutant run here as expected.
<!-- kani-results-summary: end -->

What the status rests on, besides the Kani runs (these do not change with
the runs):

- **Plain mode** at `6b120fd` (the last change to `kani/src`): `run.sh
  plain`, 33 passed, 0 filtered out, 11 s; `h_ref_traces` replays the 216
  conformance traces (8,865 calls; 1 refused at init) with REF-1..3 on
  every call. Testing, not proof ("Plain mode").
- **Mutants, plain half**: `run.sh mutants` at `6b120fd`: rc 0 in 386 s,
  the 44 must-kill mutants killed and the equivalent mutant 20 surviving
  as its `# must-survive` header requires (394 s at `a7781c7`). CI's
  `mutants` job reruns all 45 in plain mode on every run.
- **REF-1..3** under Kani are the seven `h_ref_*` harnesses (W7's `run.sh
  ref` at `db957d4`: 7 of 7 `ok`, "covers: 43 checked, all satisfied";
  the rows of `h_ref_submit`, `h_ref_cancel` and `h_ref_settle` are
  re-measured in run 1). Mutant 31 (a `tla.rs` transcription error) is
  killed by `h_ref_cancel` under Kani at `6b120fd` ("REF-1 guard
  exactness").

Inputs measured: commit `2d33d33` of this repository (Kani tree
`dd55c46`, core pin `57b3f02`, fingerprint `384dac67...d500f`). Every
later commit up to this file's has the same Kani fingerprint
(`tools/scripts/check-kani-record.py fingerprint`): what changed since
(`kani/local/`, the Markdown, `status.toml`, `tools/`, `.github/`) is not
a Kani input.

## What changed from v1.0.0

Harness code (W7, `6e729ed`, `69cca54`, `db957d4`; W10 `6b120fd`, plain
mode only):

- `spec.rs`: the v1.1.0 constants (`VERSION_CAP = MAX_VERSION -
  3*MAX_RECORDS`), the release-aware envelope, `check_bounds` against
  `min(old, new)`, Full iff 256 records are retained and the front record
  is open, prerequisites from `completed_units`, the cap last in
  `candidate`, ids `first_id + len` (checked), the prune at 256,
  `bump_new` for dispatch, pause and observe, cancel of a Completed
  record `InvalidState`, observe `Pending` iff the values differ and a
  record is open, the ghost re-base rule (plan 2.2). New invariants
  INV-RES, INV-VERCAP, INV-CU (tagged WIN-1), WIN-1, INV-PV.
- `tla.rs` transcribes the v1.1.0 `SharedPool.tla` (`first_id`,
  `DoneUnits`, `CapErr`/`VerErr`, the release-aware envelope, the
  window, prerequisites from `doneU`, the ghost fold of a prune).
  `checks.rs`: `tla_ghost`; `ref_covers` lost five covers that v1.1.0
  makes unreachable on INV states and gained two (43 covers, was 46).
  Plain mode checks REF-1..3 also in `checked_step` (the walks and the
  f-tests, prunes and the ghost fold included) and in `h_wf` (WF states,
  `6b120fd`).
- Harnesses (`run.sh list`, 98, was 89): removed `h_live1` and
  `h_version_max_{submit,dispatch,pause,observe}`; added `h_live_all`
  (LIVE-ALL), `h_full_256_prune` (WIN-1, WIN-2, WIN-ESC with a prune),
  `h_version_cap_{submit,dispatch,ack,cancel,settle,pause,observe}`
  (VER-CAP, INV states at or above the cap) and
  `h_wf_{submit,dispatch,ack,cancel,settle}` (the WF domain).
  `h_version_max_{ack,cancel,settle}` are WF states at `u32::MAX` (TX-VER).
  New cover families: `h_version_cap`, `h_wf`, `h_live_all`; `h_full`
  has three harnesses.
- `run.sh`: `memcmp.0:25` in every unwindset (W7's probe P5: without it
  `h_step_observe_tx` fails on CBMC's library `memcmp` unwinding
  assertion only); `h_full_256_prune` in the 7,200 s tier.
- PROP tags (48): added INV-RES, INV-VERCAP, VER-CAP, LIVE-ALL, OBS-2,
  REFRESH-1, WIN-1, WIN-2, WIN-ESC; removed LIVE-1.
- Mutants (`mutants/*.patch`, 45, was 31): 01-30 re-based on v1.1.0
  (`11-prerequisite-counts-non-completed` became
  `11-release-counts-completed-units`), 31 re-based on the v1.1.0
  `ReleaseErr`, and 32-45 new, each a v1.1.0 fix reverted or a new rule
  broken (the table in "Mutants").
- Deviations: v1.1.0 adds seven ("Deviations from the plan": v1.1.0).

## CI and the local record

The peaks in "Kani results" split the work by what GitHub's hosted
runner (`ubuntu-latest`: 4 CPUs, 16 GB, 15.6 GB usable) holds in memory.
The line is 13 GB (`IN_MEMORY_GB` in `tools/scripts/check-kani-record.py`):

- A harness whose measured peak, rounded up, is 13 GB or less is in a
  group of the `kani` job, which runs on every change that reaches the
  Kani layer. A mutant whose Kani leg's harnesses peak at 13 GB or less
  is in `kani-mutants` (manual runs). The check fails any `kani` or
  `kani-mutants` entry sized above 13 GB.
- Every other harness is in `kani-large` and every other mutant in
  `kani-mutants-large`. They are run on a local machine and recorded
  ("Local record"); CI's `kani-record` job checks that record on every
  run instead of running them, and the `covers` job counts the record's
  covers with the groups'. They run in CI only when the `KANI_RUNNER`
  repository variable names a large runner.
- Fixed placements: `h_wedge` is always in `kani-large` (the record's
  self-test samples it); `h_full_256` is in `kani-large` because it does
  not fit the hosted runner (CI runs 43600054037 and 43618869922: it
  filled the 15 GB of memory and 14-15 GB of swap, and the runner was
  lost after 13-17 minutes; v1.0.0's 46 s and 0.6 GB are not v1.1.0's);
  mutant 21 (killed by `h_full_256`) is in `kani-mutants-large`.
- A `kani-large` / `kani-mutants-large` entry's timeout is at least the
  `KANI_TIMEOUT` its recorded run used (`limit_s`; the check fails a lower
  one). `run-local.py --estimates-from` raised some limits above 3 times
  the estimate, from an earlier run's measured times.
- A `kani-mutants-large` mutant that no local run finished is declared in
  `kani/local/not-run.tsv` (mutant, reason) and shown in the record as not
  run at v1.1.0; the check reports each as a warning. Its plain-mode kill
  still runs in CI (`mutants`). A harness can never be declared not run.

The split's counts are the summary's "Split" line. At `a4e3179` (before
the split was redone) CI failed, as expected, `kani-record` and `covers`
(the record was v1.0.0's) and two Kani groups: the one that still named
v1.0.0's `h_version_max_{submit,dispatch,pause,observe}` (FAILED in 2 s,
no such harness) and the one with `h_full_256` (runner lost, above). The matrices are now generated from v1.1.0's `run.sh
list` and these measurements.

## Layout

| File | Contents |
|---|---|
| `src/spec.rs` | Executable specification written from the brief ("Exact core semantics"), not from the code: `expected(pre, op)` (exact result in guard order, with the reason of each exit, `Why`), `post_matches` (exact post-state and frame), `candidate`, `envelope` (release-aware), `check_bounds`, `energy`, `valid`, the invariants (INV-WF to INV-DEP, INV-RES, INV-VERCAP, INV-CU, WIN-1, INV-PV), the ghost `(base, k0, eff)` and `ghost_step` (plan 2.2). Arithmetic is exact (deviation 2); the code is only ever called (`apply`), never reused. `M`/`GR` depend on `cfg!(kani)` (v1.1.0 deviation 1). |
| `src/src.rs` | Dual-mode value source (near-approval design): `Sym` under `cfg(kani)` (symbolic: the proof), `Rng` (xorshift with hints: a test, not a proof). `assume!`, `cover!`, `Dom`, `Shape`, `Cfg`. |
| `src/gen.rs` | Bounded builders: valid configs, INV states with their ghost (at most 3 records, `first_id = 0`), WF states (2 records, free `first_id` and `completed_units`), raw configs, operations, fixed-capacity copies. |
| `src/checks.rs` | Harness bodies (`PROP:` tags): `check_tx`, `check_post`, `assert_inv`, `covers`, and one body per harness (`version_cap`, `version_max`, `wf_step`, `live_all`, `full`, `wedge`, `wedge_healthy`, `validate`, `init`, `preview`, `best`, ...); `step_ref` (the `h_ref_*` body), `refines`, `tla_ghost` and `ref_covers` (REF-1..3 and their 43 covers). Plain mode only: `window_walk`, `cap_walk`, `checked_step` (REF-1..3 at image `T<8,64,8,8,256>`), and REF in `step`, `version_max`, `bmc_from_init` and `wf_step` (`PART_REF`). |
| `src/proofs.rs` | `#[kani::proof]` wrappers: each fixes a `Cfg` (bounds, value domain, part). |
| `src/tla.rs` | REF-1..3: the v1.1.0 `tla/SharedPool.tla` transcribed, operator by operator, from the `.tla` text: the state image `T` and the abstraction `alpha`, the error operators (guard order of `GuardOrder`), the actions, the ghost (`TGhost`, `ghost_after`), state equality `same`, and `assert_refines`. |
| `src/conform.rs` | Plain mode: `h_ref_traces`, REF-1..3 on every call of the 216 conformance traces (`conformance/traces/core.txt`, 8,865 calls), each replayed on the core against its recorded result and post-state. |
| `src/plain.rs` | Plain mode: every body with `Rng`; every cover a body evaluates must fire. |
| `src/model.rs` | The fixed-capacity `Vec` models, `h_init`'s free `Config::validate`, `h_best`'s free `Pool::candidate` and `Pool::energy`, and the tagged statics they record into ("Tool issues"). |
| `mutants/*.patch` | 45 mutants: 44 to be killed (43 of the core; 31 of `tla.rs`, `# applies-to: kani`) and the equivalent 20 (`# must-survive`); the `# killed-by:` / `# checked-by:` headers name the Kani harnesses. |
| `run.sh` | The runner (per-harness timeout 1800 s, 7200 s for the `h_step_*_inv_3` parts and `h_full_256_prune`; `KANI_TIMEOUT` overrides it; memory cap `KANI_MAX_GB`, per-loop unwinding with `memcmp.0:25`, cover check, mutants). |
| `local/` | The local record ("Local record"): `provenance.txt`, `jobs.tsv`, `covers.tsv` and `not-run.tsv`; `run-local.py` (`--all`: the measurement; `--record`: the large matrices), `record.py` (checks finished runs, writes the record and this file's local-record block), `results-tables.py` (this file's generated blocks); `README.md` (format, fingerprint, commands). |

## Kani results

Each job is `run.sh kani <h>` (or `run.sh mutants --kani 'NN-*'`), run by
`kani/local/run-local.py --all` in a `git archive` copy of the commit's
Kani inputs with the source-mirror at its gitlink: flags
`--no-memory-safety-checks --no-assertion-reach-checks -Z stubbing`,
unwinding assertions on, `KANI_DEFER_COVERS=1` (each harness's covers in
its log directory, the family check below), `KANI_MAX_GB` 40 (run.sh's
default), `KANI_TIMEOUT` the job's limit (3 times its estimate, at least
1,800 s, at least 7,200 s for run.sh's long tier; `--estimates-from`
raises it to 3 times a measured time). Two or three jobs ran at a time
under a memory budget (a job estimated at 35 GB or more alone), so the
times are with a neighbour; the peaks are each job's own (sampled every
3 s).

The runs (the Run column):

- **run 1**: `run-local.py --all` at `2d33d33`
  (`~/shared-pool-kani-measure/all-2d33d33`): every harness and mutant,
  harnesses first, 2 lanes, 50 GB budget; started 2026-11-14 14:30 UTC,
  stopped by signal at 18:10 UTC with the harnesses it had finished.
- **run 3**: `run-local.py --all --only ... --estimates-from` at
  `2d33d33` (`~/shared-pool-kani-measure/run3-2d33d33`): of the harnesses
  run 1 had not finished, the ones that decide the split: every one
  estimated at 10 GB or more (v1.0.0's peak times 1.25, or W7's), that
  is the large ones and the hosted ones near the 13 GB line, except
  `h_version_max_cancel` (11.0 GB); and `h_full_256`, scheduled at 30 GB
  after its CI runs ("CI and the local record"). The ones estimated
  lower are left to CI's hosted `kani` job. Then the large mutants it
  could reach (21, 45, 32, 33, 40, 37, 36, 42, 35, 44, 41, 20, in that
  order); 3 lanes, 48 GB budget; started 18:12 UTC, with run 1's results
  plus an estimate for `h_full_256` (1,500 s, 30 GB) as estimates (env.txt
  `estimates-from`). Its `--only` list is in
  `env.txt`, its schedule `jobs.json`, its end `progress.log` (stopped
  by signal at 21:13 UTC). (Run 2 is not used: run 3 superseded it.)
- **run 4**: `run-local.py --all --only h_energy_max --estimates-from`
  (run 3's file) at `2d33d33` (`~/shared-pool-kani-measure/run4-2d33d33`),
  1 lane, started 21:14 UTC: the one large harness run 3 never started.
  It runs alone (estimated at 40 GB, above run-local.py's 35 GB line),
  and run 3's scheduler kept its lanes busy with mutants that fit beside
  each other, so the machine was never empty for it; run 3 was stopped
  to run it.
- **W7** (earlier, one job at a time): the probes at `69cca54` and `run.sh
  ref` at `db957d4`, on the same Kani harness code (`6b120fd` changed
  plain mode only). A W7 row stands only for a harness no run finished.

Columns: Time is `run.sh`'s per-harness seconds (compilation included);
Peak is CBMC's resident memory, Tree the job's whole process tree (both
sampled every 3 s; resident memory understates a job's footprint when
macOS compresses its pages: kani/local/README.md, "Resident memory is
not the footprint"; W7's probes recorded both, its `run.sh ref` rows
CBMC's only); Clauses
the last CNF CBMC hands to CaDiCaL; Covers the family's covers this
harness satisfied (a family's covers live in its `_cov` part or in the
harness itself); CI job the workflow's matrix that now holds the harness;
v1.0.0 the time and peak of this file at `a072a2d`. A harness no run
measured shows an estimate: `run-local.py`'s (v1.0.0's peak times 1.25,
W7's, or measured), or a run's schedule where that had a better basis
than v1.0.0's (`h_full_256`: run 3's 30 GB); while a run is going, it is
"pending (run N)". `h_canary` must be `ok-FAILED` (FAILED on its own
assertion "canary: no submit is ever accepted" and nothing else).

<!-- kani-results-harnesses: begin (kani/local/results-tables.py writes this block; do not edit) -->
98 harnesses (`run.sh list` at `c8dd19f`): 74 measured in the runs (Run column; all as required), 4 by W7 only, 20 not measured here. Sum of the measured harnesses' times: 34619 s (9.6 h).

| Harness | Result | Time (s) | Peak (GB) | Tree (GB) | Clauses (M) | Covers SATISFIED (this harness) | Run | CI job | v1.0.0 (time, peak) |
|---|---|---:|---:|---:|---:|---|---|---|---|
| `h_init` | ok | 502 | 32.6 | 32.7 | 54.7 | 2 of 2 | run 3 | `kani-large` | 576 s, 21.6 GB |
| `h_init_inv` | CI `kani` (not measured here) | - | - | - | - | - | est. 7.2 GB (v1.0.0 x1.25) | `kani` | 61 s, 5.8 GB |
| `h_init_inv_2` | CI `kani` (not measured here) | - | - | - | - | - | est. 7.9 GB (v1.0.0 x1.25) | `kani` | 63 s, 6.3 GB |
| `h_validate` | ok | 790 | 36.7 | 36.7 | 77.5 | 11 of 11 | run 3 | `kani-large` | 737 s, 26.3 GB |
| `h_energy` | ok | 168 | 17.0 | 17.1 | 32.9 | 4 of 4 | run 3 | `kani-large` | 150 s, 18.8 GB |
| `h_energy_exact` | ok | 1092 | 17.5 | 17.5 | 34.7 | 0 of 4 | run 1 | `kani-large` | 1674 s, 18.9 GB |
| `h_energy_max` | ok | 348 | 32.4 | 32.4 | 226.3 | 1 of 1 | run 4 | `kani-large` | 447 s, 32.8 GB |
| `h_preview` | ok | 1933 | 30.0 | 30.0 | 74.2 | 6 of 6 | run 1 | `kani-large` | 1539 s, 23.5 GB |
| `h_preview_post` | ok | 1131 | 23.7 | 23.8 | 41.5 | 0 of 6 | run 1 | `kani-large` | 1436 s, 22.0 GB |
| `h_best` | CI `kani` (not measured here) | - | - | - | - | - | est. 3.9 GB (v1.0.0 x1.25) | `kani` | 137 s, 3.1 GB |
| `h_live_all` | ok | 1677 | 18.8 | 18.8 | 39.3 | 12 of 12 | run 1 | `kani-large` | new |
| `h_wedge` | ok | 679 | 31.8 | 31.8 | 80.7 | 3 of 3 | run 3 | `kani-large` | 426 s, 39.0 GB |
| `h_wedge_healthy` | ok | 124 | 14.8 | 14.8 | 24.0 | 1 of 1 | run 3 | `kani-large` | 88 s, 9.5 GB |
| `h_bmc_from_init_po` | ok | 1950 | 37.3 | 37.3 | 200.5 | 0 of 2 | run 3 | `kani-large` | 1583 s, 27.0 GB |
| `h_slot_cycle_0` | ok | 586 | 17.7 | 17.7 | 45.3 | 1 of 1 | run 3 | `kani-large` | 467 s, 19.8 GB |
| `h_slot_cycle_2` | ok | 1236 | 30.1 | 30.1 | 65.3 | 1 of 1 | run 3 | `kani-large` | 880 s, 24.3 GB |
| `h_full_255` | ok | 1015 | 27.9 | 27.9 | 149.3 | 2 of 6 | run 1 | `kani-large` | 764 s, 19.9 GB |
| `h_full_256` | ok | 752 | 39.2 | 39.2 | 148.9 | 1 of 6 | run 3 | `kani-large` | 46 s, 0.6 GB |
| `h_full_256_prune` | ok | 467 | 20.5 | 20.5 | 39.4 | 3 of 6 | run 1 | `kani-large` | new |
| `h_canary` | ok-FAILED | 173 | 12.2 | 12.2 | 20.0 | - | run 3 | `kani` | 100 s, 10.3 GB |
| `h_version_cap_submit` | ok | 466 | 18.0 | 18.0 | 32.3 | 1 of 7 | run 1 | `kani-large` | new |
| `h_version_cap_dispatch` | ok | 55 | 5.2 | 5.2 | 9.3 | 1 of 7 | run 1 | `kani` | new |
| `h_version_cap_ack` | ok | 51 | 5.1 | 5.1 | 6.1 | 1 of 7 | run 1 | `kani` | new |
| `h_version_cap_cancel` | ok | 120 | 10.8 | 10.8 | 18.7 | 1 of 7 | run 1 | `kani` | new |
| `h_version_cap_settle` | ok | 217 | 14.2 | 14.3 | 29.0 | 1 of 7 | run 1 | `kani-large` | new |
| `h_version_cap_pause` | ok | 47 | 5.0 | 5.0 | 6.0 | 1 of 7 | run 1 | `kani` | new |
| `h_version_cap_observe` | ok | 59 | 6.0 | 6.0 | 13.1 | 1 of 7 | run 1 | `kani` | new |
| `h_version_max_ack` | CI `kani` (not measured here) | - | - | - | - | - | est. 3.4 GB (v1.0.0 x1.25) | `kani` | 34 s, 2.7 GB |
| `h_version_max_cancel` | CI `kani` (not measured here) | - | - | - | - | - | est. 11.0 GB (v1.0.0 x1.25) | `kani` | 90 s, 8.8 GB |
| `h_version_max_settle` | ok | 300 | 11.6 | 11.6 | 23.9 | 2 of 6 | run 1 | `kani` | 202 s, 11.7 GB |
| `h_wf_submit` | ok | 406 | 20.1 | 20.1 | 39.0 | 1 of 7 | run 1 | `kani-large` | new |
| `h_wf_dispatch` | ok | 58 | 6.7 | 6.9 | 15.1 | 2 of 7 | run 1 | `kani` | new |
| `h_wf_ack` | ok | 53 | 6.1 | 6.1 | 14.3 | 2 of 7 | run 1 | `kani` | new |
| `h_wf_cancel` | ok | 122 | 12.8 | 12.8 | 24.2 | 4 of 7 | run 1 | `kani` | new |
| `h_wf_settle` | ok | 293 | 16.0 | 16.0 | 34.5 | 4 of 7 | run 1 | `kani-large` | new |
| `h_bmc_from_init_s_submit` | ok | 1141 | 39.6 | 39.6 | 121.1 | 2 of 2 | run 3 | `kani-large` | 1121 s, 29.6 GB |
| `h_bmc_from_init_s_dispatch` | ok | 266 | 18.7 | 18.7 | 42.1 | 0 of 2 | run 3 | `kani-large` | 256 s, 17.4 GB |
| `h_bmc_from_init_s_ack` | ok | 253 | 18.6 | 18.7 | 41.6 | 0 of 2 | run 3 | `kani-large` | 270 s, 16.7 GB |
| `h_bmc_from_init_s_cancel` | ok | 351 | 19.6 | 19.6 | 50.5 | 0 of 2 | run 1 | `kani-large` | 377 s, 16.4 GB |
| `h_bmc_from_init_s_settle` | ok | 416 | 20.5 | 20.6 | 59.8 | 0 of 2 | run 3 | `kani-large` | 332 s, 18.0 GB |
| `h_bmc_from_init_s_pause` | ok | 251 | 15.7 | 15.7 | 41.5 | 0 of 2 | run 1 | `kani-large` | 219 s, 16.2 GB |
| `h_bmc_from_init_s_observe` | ok | 300 | 21.5 | 21.5 | 51.9 | 0 of 2 | run 3 | `kani-large` | 265 s, 18.4 GB |
| `h_step_submit_tx` | ok | 705 | 19.2 | 19.2 | 35.4 | 0 of 67 | run 1 | `kani-large` | 553 s, 20.8 GB |
| `h_step_submit_post` | ok | 619 | 19.2 | 19.2 | 40.4 | 0 of 67 | run 1 | `kani-large` | 412 s, 17.8 GB |
| `h_step_submit_inv` | ok | 1039 | 24.9 | 25.0 | 74.8 | 0 of 67 | run 3 | `kani-large` | 877 s, 29.5 GB |
| `h_step_submit_cov` | ok | 308 | 18.2 | 18.3 | 32.2 | 18 of 67 | run 3 | `kani-large` | 229 s, 17.0 GB |
| `h_step_dispatch_tx` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.8 GB (v1.0.0 x1.25) | `kani` | 47 s, 4.6 GB |
| `h_step_dispatch_post` | ok | 91 | 9.3 | 9.4 | 19.2 | 0 of 67 | run 3 | `kani` | 74 s, 9.2 GB |
| `h_step_dispatch_inv` | ok | 140 | 7.6 | 7.6 | 21.0 | 0 of 67 | run 3 | `kani` | 103 s, 9.9 GB |
| `h_step_dispatch_cov` | CI `kani` (not measured here) | - | - | - | - | - | est. 6.2 GB (v1.0.0 x1.25) | `kani` | 55 s, 5.0 GB |
| `h_step_ack_tx` | CI `kani` (not measured here) | - | - | - | - | - | est. 3.1 GB (v1.0.0 x1.25) | `kani` | 29 s, 2.5 GB |
| `h_step_ack_post` | ok | 99 | 9.5 | 9.5 | 20.0 | 0 of 67 | run 3 | `kani` | 79 s, 8.7 GB |
| `h_step_ack_inv` | ok | 122 | 8.9 | 8.9 | 17.8 | 0 of 67 | run 3 | `kani` | 82 s, 9.0 GB |
| `h_step_ack_cov` | CI `kani` (not measured here) | - | - | - | - | - | est. 4.1 GB (v1.0.0 x1.25) | `kani` | 44 s, 3.3 GB |
| `h_step_cancel_tx` | ok | 137 | 7.6 | 7.6 | 19.0 | 0 of 67 | run 3 | `kani` | 101 s, 8.4 GB |
| `h_step_cancel_post` | ok | 233 | 17.0 | 17.0 | 37.6 | 0 of 67 | run 1 | `kani-large` | 197 s, 14.4 GB |
| `h_step_cancel_inv` | ok | 339 | 16.4 | 16.4 | 31.3 | 0 of 67 | run 1 | `kani-large` | 229 s, 13.2 GB |
| `h_step_cancel_cov` | ok | 114 | 13.1 | 13.2 | 18.4 | 8 of 67 | run 3 | `kani-large` | 83 s, 9.4 GB |
| `h_step_settle_tx` | ok | 243 | 15.7 | 15.7 | 29.3 | 0 of 67 | run 1 | `kani-large` | 204 s, 13.7 GB |
| `h_step_settle_post` | ok | 526 | 20.5 | 20.5 | 50.9 | 0 of 67 | run 3 | `kani-large` | 355 s, 18.4 GB |
| `h_step_settle_inv` | ok | 942 | 20.4 | 20.5 | 42.4 | 0 of 67 | run 1 | `kani-large` | 438 s, 18.0 GB |
| `h_step_settle_cov` | ok | 203 | 17.3 | 17.4 | 28.6 | 11 of 67 | run 3 | `kani-large` | 135 s, 12.5 GB |
| `h_step_pause_tx` | CI `kani` (not measured here) | - | - | - | - | - | est. 3.5 GB (v1.0.0 x1.25) | `kani` | 32 s, 2.8 GB |
| `h_step_pause_post` | ok | 89 | 9.5 | 9.5 | 18.6 | 0 of 67 | run 3 | `kani` | 69 s, 8.6 GB |
| `h_step_pause_inv` | ok | 114 | 8.7 | 8.7 | 17.8 | 0 of 67 | run 3 | `kani` | 90 s, 8.7 GB |
| `h_step_pause_cov` | CI `kani` (not measured here) | - | - | - | - | - | est. 3.5 GB (v1.0.0 x1.25) | `kani` | 37 s, 2.8 GB |
| `h_step_observe_tx` | ok | 110 | 12.6 | 12.7 | 25.9 | 0 of 67 | run 3 | `kani` | 97 s, 11.3 GB |
| `h_step_observe_post` | ok | 145 | 13.1 | 13.1 | 31.2 | 0 of 67 | run 3 | `kani-large` | 164 s, 12.0 GB |
| `h_step_observe_inv` | ok | 187 | 9.0 | 9.1 | 25.9 | 0 of 67 | run 3 | `kani` | 124 s, 11.1 GB |
| `h_step_observe_cov` | CI `kani` (not measured here) | - | - | - | - | - | est. 8.6 GB (v1.0.0 x1.25) | `kani` | 70 s, 6.9 GB |
| `h_step_submit_inv_0` | ok | 478 | 22.1 | 22.1 | 63.8 | 0 of 67 | run 3 | `kani-large` | 394 s, 19.4 GB |
| `h_step_submit_inv_1` | ok | 652 | 25.0 | 25.1 | 69.3 | 0 of 67 | run 1 | `kani-large` | 547 s, 21.6 GB |
| `h_step_submit_inv_3` | ok | 1356 | 26.2 | 26.2 | 80.4 | 0 of 67 | run 1 | `kani-large` | 1181 s, 23.6 GB |
| `h_step_dispatch_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.1 GB (v1.0.0 x1.25) | `kani` | 47 s, 4.1 GB |
| `h_step_dispatch_inv_1` | ok | 95 | 9.2 | 9.2 | 16.4 | 0 of 67 | run 3 | `kani` | 74 s, 8.1 GB |
| `h_step_dispatch_inv_3` | ok | 164 | 13.3 | 13.4 | 25.7 | 0 of 67 | run 3 | `kani-large` | 139 s, 12.3 GB |
| `h_step_ack_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.0 GB (v1.0.0 x1.25) | `kani` | 72 s, 4.0 GB |
| `h_step_ack_inv_1` | CI `kani` (not measured here) | - | - | - | - | - | est. 6.9 GB (v1.0.0 x1.25) | `kani` | 63 s, 5.5 GB |
| `h_step_ack_inv_3` | ok | 172 | 13.0 | 13.0 | 22.5 | 0 of 67 | run 3 | `kani` | 124 s, 12.2 GB |
| `h_step_cancel_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.2 GB (v1.0.0 x1.25) | `kani` | 52 s, 4.2 GB |
| `h_step_cancel_inv_1` | ok | 152 | 10.6 | 10.6 | 23.6 | 0 of 67 | run 3 | `kani` | 135 s, 9.6 GB |
| `h_step_cancel_inv_3` | ok | 775 | 20.1 | 20.1 | 39.2 | 0 of 67 | run 1 | `kani-large` | 621 s, 16.4 GB |
| `h_step_settle_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.1 GB (v1.0.0 x1.25) | `kani` | 46 s, 4.1 GB |
| `h_step_settle_inv_1` | ok | 251 | 17.8 | 17.8 | 31.6 | 0 of 67 | run 1 | `kani-large` | 218 s, 13.2 GB |
| `h_step_settle_inv_3` | ok | 2761 | 22.1 | 22.1 | 53.5 | 0 of 67 | run 1 | `kani-large` | 1788 s, 18.3 GB |
| `h_step_pause_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 5.1 GB (v1.0.0 x1.25) | `kani` | 57 s, 4.1 GB |
| `h_step_pause_inv_1` | CI `kani` (not measured here) | - | - | - | - | - | est. 7.1 GB (v1.0.0 x1.25) | `kani` | 62 s, 5.7 GB |
| `h_step_pause_inv_3` | ok | 133 | 12.5 | 12.5 | 22.5 | 0 of 67 | run 3 | `kani` | 124 s, 12.8 GB |
| `h_step_observe_inv_0` | CI `kani` (not measured here) | - | - | - | - | - | est. 9.5 GB (v1.0.0 x1.25) | `kani` | 76 s, 7.6 GB |
| `h_step_observe_inv_1` | ok | 126 | 9.0 | 9.1 | 21.0 | 0 of 67 | run 3 | `kani` | 91 s, 9.6 GB |
| `h_step_observe_inv_3` | ok | 199 | 10.3 | 10.3 | 31.1 | 0 of 67 | run 3 | `kani` | 162 s, 14.1 GB |
| `h_ref_submit` | ok | 509 | 15.1 | 15.1 | 28.3 | 12 of 43 | run 1 | `kani-large` | 351 s, 12.4 GB |
| `h_ref_dispatch` | ok | 67 | 7.5 | - | 12.3 | 8 of 43 | W7 ref run | `kani` | 62 s, 5.7 GB |
| `h_ref_ack` | ok | 57 | 6.0 | - | 12.0 | 3 of 43 | W7 ref run | `kani` | 55 s, 5.2 GB |
| `h_ref_cancel` | ok | 154 | 11.7 | 11.7 | 19.9 | 6 of 43 | run 1 | `kani` | 104 s, 9.6 GB |
| `h_ref_settle` | ok | 289 | 14.4 | 14.4 | 28.6 | 5 of 43 | run 1 | `kani-large` | 221 s, 11.0 GB |
| `h_ref_pause` | ok | 59 | 6.0 | - | 12.0 | 3 of 43 | W7 ref run | `kani` | 55 s, 5.3 GB |
| `h_ref_observe` | ok | 80 | 8.6 | - | 16.8 | 6 of 43 | W7 ref run | `kani` | 69 s, 8.4 GB |
<!-- kani-results-harnesses: end -->

## Local record

The local record is the `kani-large` harnesses and the
`kani-mutants-large` mutants, taken by `kani/local/record.py` from the
runs above (each job from the latest run that finished it; a large
mutant no run finished is declared in `kani/local/not-run.tsv`), at the
commit and fingerprint the block names. `record.py` checks each run (its
commit's Kani inputs equal HEAD's, the core pin, Kani and CBMC versions;
every recorded job exit 0, every harness `ok`, every mutant as its patch
expects, with the log of its Kani leg) and writes `provenance.txt`,
`jobs.tsv` and `covers.tsv` into `kani/local/` and the block below.
Each job's `limit_s` is the `KANI_TIMEOUT` it ran under (its
`progress.log` start line, else `jobs.json`). Format and commands:
`kani/local/README.md`.

On every run, CI's `kani-record` job (`tools/scripts/check-kani-record.py`)
checks that:
- the record's fingerprint of the Kani inputs is the commit's. The
  inputs are every file under `kani/` except `kani/local` and the
  Markdown, with the git object ids of `kani/src`, `Cargo.toml`,
  `Cargo.lock`, `mutants`, `run.sh` and `.gitignore` on lines of their
  own, the core's gitlink and `conformance/traces/core.txt`, the
  repository's `.cargo/config[.toml]` above `kani/` (none here), and
  `tools/versions.env`'s `KANI` and `CBMC`;
- `jobs.tsv`, `covers.tsv` and `not-run.tsv` match the digest in
  `provenance.txt`, and the fingerprint is the hash of the record's input
  lines and, where the clone has the record's commit, that commit's;
- every recorded harness is `ok` and every recorded mutant as its patch
  expects;
- the `kani` groups and the record together are `run.sh list`, each
  harness once; `kani-mutants`, the record and the declared mutants are
  `mutants/*.patch`, each mutant once (each declared one a warning);
- no `kani-large` / `kani-mutants-large` timeout is below the `limit_s`
  its recorded run used, and no `kani` / `kani-mutants` entry is sized
  above 13 GB;
- the block below is there and is `record.py table`.

A change to any Kani input fails that job until the record has been
re-run at the new commit and committed. CI cannot check that the run
took place as recorded: the record, like the tables, rests on the word of
whoever ran it. Anyone with a 64 GB machine can repeat it with
`run-local.py --record` and compare. The record's runs are measurements
(`--all`, several jobs at a time), so its times are with a neighbour. A
mutant's time is its whole job, plain mode included (rustc 1.97.0, as
CI); the record does not cover the plain half of a mutant's verdict, and
`RUST_TOOLCHAIN` is not in the fingerprint: CI's `mutants` job reruns
every mutant's plain mode on every run.

<!-- kani-local-record: begin (kani/local/record.py writes this block; do not edit) -->
Local record (`kani/local/`, 48 harnesses and 1 mutants): commit `2d33d33`, Kani inputs fingerprint `384dac67293e` (CI recomputes it, `tools/scripts/check-kani-record.py`); Kani 0.68.0, CBMC 6.11.0; Apple M1 Max 10 cores 64 GB, macOS 27.2.1; 2026-11-14T14:30:33Z to 2026-11-14T21:21:04Z, 3 runs of run-local.py --all (provenance.txt): 2 jobs at a time within 50 GB; 3 jobs at a time within 48 GB; 1 jobs at a time within 60 GB. Time is wall clock per harness including compilation (a mutant: its whole run, plain mode included); peak is CBMC's resident memory, sampled every 3 s (it can understate the footprint: kani/local/README.md, Resident memory is not the footprint); limit is the job's KANI_TIMEOUT.

| Harness | Result | Time (s) | Peak (GB) | Limit (s) | Covers SATISFIED (this harness) |
|---|---|---:|---:|---:|---|
| `h_init` | ok | 502 | 32.6 | 1800 | 2 of 2 |
| `h_validate` | ok | 790 | 36.7 | 2400 | 11 of 11 |
| `h_energy` | ok | 168 | 17.0 | 1800 | 4 of 4 |
| `h_energy_exact` | ok | 1092 | 17.5 | 5040 | 0 of 4 |
| `h_energy_max` | ok | 348 | 32.4 | 1800 | 1 of 1 |
| `h_preview` | ok | 1933 | 30.0 | 4620 | 6 of 6 |
| `h_preview_post` | ok | 1131 | 23.7 | 4320 | 0 of 6 |
| `h_live_all` | ok | 1677 | 18.8 | 7200 | 12 of 12 |
| `h_wedge` | ok | 679 | 31.8 | 1920 | 3 of 3 |
| `h_wedge_healthy` | ok | 124 | 14.8 | 1800 | 1 of 1 |
| `h_bmc_from_init_po` | ok | 1950 | 37.3 | 4800 | 0 of 2 |
| `h_slot_cycle_0` | ok | 586 | 17.7 | 1800 | 1 of 1 |
| `h_slot_cycle_2` | ok | 1236 | 30.1 | 2640 | 1 of 1 |
| `h_full_255` | ok | 1015 | 27.9 | 2340 | 2 of 6 |
| `h_full_256` | ok | 752 | 39.2 | 4500 | 1 of 6 |
| `h_full_256_prune` | ok | 467 | 20.5 | 7200 | 3 of 6 |
| `h_version_cap_submit` | ok | 466 | 18.0 | 7200 | 1 of 7 |
| `h_version_cap_settle` | ok | 217 | 14.2 | 7200 | 1 of 7 |
| `h_wf_submit` | ok | 406 | 20.1 | 7200 | 1 of 7 |
| `h_wf_settle` | ok | 293 | 16.0 | 7200 | 4 of 7 |
| `h_bmc_from_init_s_submit` | ok | 1141 | 39.6 | 3420 | 2 of 2 |
| `h_bmc_from_init_s_dispatch` | ok | 266 | 18.7 | 1800 | 0 of 2 |
| `h_bmc_from_init_s_ack` | ok | 253 | 18.6 | 1800 | 0 of 2 |
| `h_bmc_from_init_s_cancel` | ok | 351 | 19.6 | 1800 | 0 of 2 |
| `h_bmc_from_init_s_settle` | ok | 416 | 20.5 | 1800 | 0 of 2 |
| `h_bmc_from_init_s_pause` | ok | 251 | 15.7 | 1800 | 0 of 2 |
| `h_bmc_from_init_s_observe` | ok | 300 | 21.5 | 1800 | 0 of 2 |
| `h_step_submit_tx` | ok | 705 | 19.2 | 1800 | 0 of 67 |
| `h_step_submit_post` | ok | 619 | 19.2 | 1800 | 0 of 67 |
| `h_step_submit_inv` | ok | 1039 | 24.9 | 3360 | 0 of 67 |
| `h_step_submit_cov` | ok | 308 | 18.2 | 1800 | 18 of 67 |
| `h_step_cancel_post` | ok | 233 | 17.0 | 1800 | 0 of 67 |
| `h_step_cancel_inv` | ok | 339 | 16.4 | 1800 | 0 of 67 |
| `h_step_cancel_cov` | ok | 114 | 13.1 | 1800 | 8 of 67 |
| `h_step_settle_tx` | ok | 243 | 15.7 | 1800 | 0 of 67 |
| `h_step_settle_post` | ok | 526 | 20.5 | 1800 | 0 of 67 |
| `h_step_settle_inv` | ok | 942 | 20.4 | 1800 | 0 of 67 |
| `h_step_settle_cov` | ok | 203 | 17.3 | 1800 | 11 of 67 |
| `h_step_observe_post` | ok | 145 | 13.1 | 1800 | 0 of 67 |
| `h_step_submit_inv_0` | ok | 478 | 22.1 | 1800 | 0 of 67 |
| `h_step_submit_inv_1` | ok | 652 | 25.0 | 1800 | 0 of 67 |
| `h_step_submit_inv_3` | ok | 1356 | 26.2 | 7200 | 0 of 67 |
| `h_step_dispatch_inv_3` | ok | 164 | 13.3 | 7200 | 0 of 67 |
| `h_step_cancel_inv_3` | ok | 775 | 20.1 | 7200 | 0 of 67 |
| `h_step_settle_inv_1` | ok | 251 | 17.8 | 1800 | 0 of 67 |
| `h_step_settle_inv_3` | ok | 2761 | 22.1 | 7200 | 0 of 67 |
| `h_ref_submit` | ok | 509 | 15.1 | 1800 | 12 of 43 |
| `h_ref_settle` | ok | 289 | 14.4 | 1800 | 5 of 43 |

| Mutant | Expected | Plain | Kani (harness) | Time (s) | Peak (GB) | Limit (s) |
|---|---|---|---|---:|---:|---:|
| `20-observe-length-check-dropped` | survive | survived | survived (`h_step_observe_tx`, `h_step_observe_post`) | 363 | 12.7 | 1800 |

**Not run locally** (27 kani-mutants-large mutants declared in `kani/local/not-run.tsv`; the record does not show their Kani leg):

- `01-guard-swap-paused-stale`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `03-submit-expiry-off-by-one`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `05-check-bounds-without-skip`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `06-envelope-ignores-reserved`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `10-settle-accepts-opposite-terminal`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `11-release-counts-completed-units`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `12-energy-ge`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `15-submit-actor-check-dropped`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `16-settle-executor-check-dropped`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `18-preview-candidate-divergence`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `19-recovery-gt`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `23-envelope-ignores-extra`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `25-energy-excess-sign`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `28-check-bounds-vs-candidate`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `29-observe-keeps-observed-at`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `30-full-rejects-at-255`: not run locally at v1.1.0 before the deadline; plain-mode kill in CI
- `21-full-off-by-one`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `32-envelope-no-release-term`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `33-check-bounds-old-only`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `35-candidate-without-cap`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `36-full-ignores-front`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `37-validate-actor-is-executor`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `40-settle-no-completed-units`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `41-completed-units-wrap`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `42-prune-keeps-first-id`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `44-prereq-scan-retained`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
- `45-cap-in-submit-only`: not run locally at v1.1.0 before the deadline (run 3 stopped first); plain-mode kill in CI
<!-- kani-local-record: end -->

## Bounds

**v1.1.0.** The bounds below are v1.0.0's, with v1.1.0's changes (v1.1.0 deviation 2): INV states have at most 3 records and `first_id = 0`; the `h_wf_*` and `h_version_max_*` harnesses use WF states (2 records, free `first_id` and `completed_units`; `h_version_max_*` at version `u32::MAX`); `h_version_cap_*` use INV states at or above `VERSION_CAP`; only `h_full_255`, `h_full_256` and `h_full_256_prune` have 256 concrete records, and a prune happens under Kani only there. `h_live1`, `h_version_max_{submit,dispatch,pause,observe}` are gone (`h_live_all`, `h_version_cap_*`).

*v1.0.0's text (this file at `a072a2d`), kept: it holds for v1.1.0 except where "What changed from v1.0.0" and the v1.1.0 deviations say otherwise; its run letters, times, peaks and counts are v1.0.0's.*


All harnesses (`proofs.rs`, `gen.rs`):

- **Accounts** `o_`, `a1`, `x1`, `z_` (the brief's `o, a1, x1, z`; all 2
  bytes, deviation 5). Every role (owner, actor, executor) and every caller
  is any of the four, so every equality pattern among the at most three role
  holders a step compares and the caller is reached (including actor =
  executor, F-4).
- **Step harnesses** (`h_step_*`): a valid configuration with **1 metric, 1
  term, 2 recipes** (recipe 1 may require recipe 0, 1 to 100 units) and an
  INV state with **exactly 2 records** (statuses free, the first `k0`
  terminal, `k0` free; values by INV-LEDGER from a free base). Metric bounds,
  base values and targets from `Anchored(4)`: `a + o` with
  `a ∈ {-1e6, 0, 1e6}`, `o ∈ -4..=4`, clipped to ±1e6, so small values and
  both extremes of the value range. Deltas (`reserve`, `complete`) in
  `-3..=3`; `max_units` in `1..=3`; weights in `{1, 2, 1000}`; any term shape;
  `max_age_seconds` free. Version, `observed_at`, `paused`, expiries free
  (`u32`/`u64`/`bool`). Operation arguments: caller any account; request
  recipe in `0..4` (2 and 3 are `InvalidRecipe`), units in `0..8` (0 and
  4..7 are `InvalidUnits`), request version and expiry and `now` free; record
  ids in `0..8` (2..7 are `NotFound`); `accepted`, `completed`, `paused` free;
  observe vectors of length 0..3 with values from the value domain or
  ±1,000,001 or `i64::MIN`/`MAX`. Narrow ranges are used only for arguments
  the code compares before any arithmetic or index (deviation 4).
- **`h_step_<op>_inv_<k>`** (`k` = 0, 1, 3; one harness per kind and
  count, the `h_step` cover family): the `_inv` part of the steps from INV
  states with exactly `k` records instead of 2, everything else as the
  steps. With `_inv` (2 records) every kind runs from every count `0..=3`.
- **`h_ref_<op>`** (REF-1..3, one harness per kind, the `h_ref` cover
  family): the steps' state and operation, from the same generators with
  the same `Cfg` (`STEP`: 1 metric, 1 term, 2 recipes, exactly 2 records,
  the domains above), through `checks::step_ref`. The TLA+ image `TK` has
  room for 2 metrics, 2 terms, 2 recipes, 2 requirements per recipe and 4
  records, above every pre- and post-state of the step shape (a submit ends
  at 3 records); `alpha` asserts that the pool fits. In plain mode the same
  check runs inside every step test (`h_step_*`: up to 2 metrics, 2 terms,
  2 recipes, 3 records), in `h_version_max` and in `h_bmc_from_init`
  (depth 5, image of 8 records), and over every call of the conformance
  traces (`h_ref_traces`, image `TB`: 32 metrics, 64 terms, 32 recipes, 32
  requirements, 256 records; the traces have at most 8 metrics, 5 terms,
  6 recipes, 2 requirements and 256 records).
- **`h_live1`**: as the steps with **3 records** (so two other records can
  be open).
- **`h_version_max_<op>`** (one harness per operation kind, one cover
  family): as the steps with `version = u32::MAX`.
- **`h_preview`, `h_preview_post`**: as the steps.
- **`h_best`**: under Kani the configuration of the steps with no record,
  any submit request of the step domain (units `0..8`), the core's
  `candidate` stubbed by a free result per quantity (`Ok` with any values
  in `[-1e6, 1e6]`, or `Err`) and its `energy` by a free result per call
  (`Ok` with any `u64`, or `Err`; deviation 24). In plain mode as the steps,
  the real `candidate`, request units ≤ 3 (`max_units` ≤ 3, so larger
  requests only add `InvalidUnits` candidates).
- **`h_energy`** (contract): **2 metrics, 2 terms**, values and targets
  anywhere in `[-1e6, 1e6]` (`Full`), any weight in `1..=1000`, vectors of
  length 0..3 with values in range, ±1,000,001 or `i64::MIN`/`MAX`.
  **`h_energy_exact`**: the same shape with `Anchored(4)` values and any
  weight. **`h_energy_max`**: the 64-term worst case, concrete.
- **`h_init`**: a raw configuration (at most 2 metrics, 2 terms, 3 recipes,
  2 requirements per recipe, vector lengths free, every field free among
  in-range, edge and out-of-range values: values and targets
  `Anchored(4)`, deltas `-3..=3`, each also ±1,000,001 or `i64::MIN`/`MAX`;
  term metrics, weights, `max_units`, requirement indices and units any
  `u32`; strings of 0 or 2 bytes); `now` free; `validate` free (deviation
  23). The raw domain contains every valid configuration of that shape and
  value set. Plain mode draws up to 3 metrics, also 128/129-byte strings
  and, half the time, a valid configuration with free initial values.
  **`h_validate`**: the same raw configurations, the real `validate`.
  **`h_init_inv`**, **`h_init_inv_2`**: a valid configuration of the step
  shape (1 metric, 1 term, 2 recipes), resp. 2 metrics, 2 terms and 2
  recipes; values `Anchored(4)`, deltas `-3..=3`, initial values free in
  `[min, max]`; under Kani `new`'s `Ok` value built by `pool_of`
  (deviation 23).
- **`h_wedge`**: from `Pool::new` over a symbolic configuration of the
  step shape (1 metric, 1 term, 2 recipes), bounds, initial values, targets
  and observations in `-4..=4`, deltas in `-3..=3`, any weight in
  `{1, 2, 1000}`, any term shape; under Kani the roles are fixed (owner
  `o_`, actors `a1`, executors `x1`; deviation 19). The script of F-1:
  observe, submit, dispatch, then settle both ways, cancel, observe.
- **`h_wedge_healthy`**: under Kani from every healthy INV state (`B = 0`,
  version < MAX) of the step shape with values in `-4..=4` whose record 0
  is Dispatched and record 1 Completed (deviation 19): settle(cancelled)
  of record 0 by its executor, cancel by the owner, observe. In plain mode
  the script of F-2 from `Pool::new` over random configurations (up to 2
  metrics and 2 terms, 2 recipes): two submits, two dispatches, the hedge
  completes, then settle(cancelled), cancel, observe, and
  settle(completed).
- **`h_bmc_from_init_po`**, **`h_bmc_from_init_s_<op>`** (one cover
  family): from `Pool::new` over a valid configuration of the step shape,
  **2 operations**: a pause or an observe, then any operation
  (`_po`); or a submit, then an operation of kind `<op>` (`_s_<op>`, one
  harness per kind; a second submit is possibly a replay of the first).
  After the first operation the exact post-state is asserted and the
  second starts from that state rebuilt with constant shapes, after a
  submit only from its `Ok` outcome (deviation 20). Values in `-3..=3`,
  deltas in `-2..=2`, `max_units` ≤ 2.
- **`h_full_255`**, **`h_full_256`**: 255 or 256 Cancelled records of
  recipe 0 with ids `0..count` (a concrete count each, one constant array
  moved into the vector, deviation 21), configuration, values, version,
  `observed_at`, `paused` and the submit request as the steps.
- **`h_slot_cycle_0`**, **`h_slot_cycle_2`** (one cover family): a valid
  configuration of the step shape and domain; from `Pool::new` (`_0`,
  `pool_of`, `observed_at` free) or from an INV state of the steps with 2
  records (`_2`), restricted to quiet states (no open record, every metric
  within `[min, max]`) with `version < u32::MAX - 2`; a submit of the
  step domain, then (if it is accepted) a cancel of the new record by the
  same caller and the same request again at the new version, each on the
  previous post-state rebuilt with constant shapes (deviation 25).

## Properties

**v1.1.0.** New PROP ids: INV-RES, INV-VERCAP, VER-CAP, LIVE-ALL, OBS-2, REFRESH-1, WIN-1, WIN-2, WIN-ESC; LIVE-1 is gone ("What changed from v1.0.0"; `grep -rn 'PROP:' kani/src` lists every tag with its check). The table below is v1.0.0's.

*v1.0.0's text (this file at `a072a2d`), kept: it holds for v1.1.0 except where "What changed from v1.0.0" and the v1.1.0 deviations say otherwise; its run letters, times, peaks and counts are v1.0.0's.*


PROP tags are in `checks.rs`; each is asserted inside the bounds above.

| ID | Harnesses | What is asserted |
|---|---|---|
| CFG-1 | `h_init`, `h_init_inv`, `h_init_inv_2`, `h_validate` | `Pool::new` calls `validate` and is Ok iff it is Ok (`h_init`, `validate` free), so iff the spec's `valid` (CFG-2); it fails only `InvalidConfig`; the initial pool is the config, initial values, version 0, `observed_at = now`, unpaused, no records; a valid configuration is accepted and its initial pool satisfies every INV-* with the initial ghost (`h_init_inv`, `h_init_inv_2`, deviation 23). |
| CFG-2 | `h_validate` | `validate` = spec on raw configs; Ok ⇒ nonempty, term metrics in range, weights 1..1000, metric bounds ordered and bounded, vectors of length n, deltas bounded, `max_units` 1..100, requirements strictly earlier, distinct, units 1..100. |
| NUM-1 | `h_energy`, `h_energy_exact`, `h_energy_max` | Bounded values of length n ⇒ `energy_terms` and `energy` Ok with one term per term (any value in ±1e6, any weight); each term exactly `w·r²` and `energy` their exact sum (anchored values, any weight; 64-term worst case 2.56e17 exact); other vectors fail `Bounds`. |
| NUM-2 | `h_step_submit_tx`, `h_step_cancel_tx`, `h_step_settle_tx` | `add` fails closed exactly at ±1e6: the exact result in guard order includes every `Arithmetic` exit (`Why::Add`), with values at both extremes (covers "Arithmetic (reserve)", "(release)"). |
| ROB-1 | every step harness | No panic, overflow or out-of-bounds access in the core from any INV state within the bounds, any argument within the domains (Kani checks every one). |
| TX-SUBMIT … TX-OBSERVE | `h_step_<op>_tx`, `_post` (2 records); for submit also `h_full_255`, `h_full_256` | Exact result in guard order (`expected`), exact post-state and frame (`post_matches`, including the configuration). At the record limit (`h_full_*`): the exact result and post-state of a submit at 255 and at 256 Cancelled records, so `Full` exactly from 256 records on (a submit at 255 is admitted when the other guards pass: cover "full: Ok at 255 records"). |
| TX-0, TX-NOOP, TX-VER | `h_step_*_tx`, `h_version_max_*`, `h_full_*` | Every `Err` leaves the pool unchanged; idempotent `Ok` paths change nothing (version included); every other `Ok` increments the version by exactly 1; at `u32::MAX` every effective transition fails `Arithmetic` and nothing wraps. |
| AUTH-1 | `h_step_*_tx` | A call that changes the state was made by the role of the brief. |
| STALE-1 | `h_step_submit_tx`, `h_bmc_from_init_s_submit` | Submit Ok ⇒ request version = version and the version strictly increases; along every 2-operation trace no accepted request is accepted again. |
| ADM-1, ADM-2, DEP-1 | `h_step_submit_tx` | As in the catalogue, against the spec's exact energy. |
| HARD-STEP, LIFE-1, ASYNC-1, TERM-1 | `h_step_*_post` | As in the catalogue. |
| OBS-1 | `h_step_observe_tx` | Observe Ok ⇒ no open record, config unchanged. |
| PAUSE-1 | `h_step_*_tx` (blocked ops), `h_step_*_post` (transparent ops) | Paused: submit and dispatch change nothing. Settle, acknowledge, cancel and observe: the spec's outcome with `paused` flipped is the same (flag kept flipped); with `_tx`/`_post` (code = spec from every state, `paused` either way) the code's outcome is independent of `paused` (deviation 18). |
| INV-WF, INV-LEDGER, INV-HARD, INV-HEALTHY, INV-DEP, INV-VER | `h_step_*_inv` (2 records), `h_step_*_inv_0`, `_inv_1`, `_inv_3`, `h_init_inv*`, `h_bmc_from_init_*` | INV-WF to INV-DEP hold in the initial state (`h_init_inv*`) and are preserved, with the ghost update, by every step of every kind from every INV state of the step shape and domains with **exactly 0, 1, 2 or 3 records** (one harness per kind and count). By induction, every state with **at most 3 records** that calls within the domains reach from `Pool::new` satisfies them (the steps' configurations fix `initial = min`; only `new` reads `initial`). A submit from 3 records ends at 4 records, never a pre-state here: states with 4 or more records are not covered by Kani. INV-VER (version = the ghost's count of effective steps from `Init`) only along the 2-operation traces of `h_bmc_from_init_*`. |
| RD-PREVIEW | `h_preview`, `h_preview_post` | `allowed` iff the spec's candidate is Ok; `reason` (Debug name), candidate values, `before`, `after` and the term strings as coded; no mutation; allowed and version < MAX ⇒ submit Ok with the previewed candidate. |
| RD-BEST | `h_best` (with `h_preview`, `h_step_submit_tx`, `h_step_submit_post`, `h_energy`) | `best_quantity` evaluates `candidate` exactly once for each quantity in `1..=units` with the request's other fields, `energy` exactly once on each `Ok` candidate's values and on nothing else, and returns the quantity of least energy among those whose energy is `Ok`, ties to the larger, `None` iff none (Kani: any `Ok`/`Err` pattern of `candidate` and of `energy`, any `Ok` values, units ≤ 7; deviation 24); `candidate` is the spec's (`h_preview`: `Ok` iff the spec's, the reason; `h_step_submit_*`: the `Ok` values, below `u32::MAX`) and `energy` is the spec's and `Ok` on them (`h_energy`, NUM-1), so the result is the spec's choice, admissible, and `submit` accepts it below `u32::MAX` (plain mode also calls `submit`). |
| LIVE-1 | `h_live1`, `h_wedge_healthy` (in the F-2 state) | From an INV state with B = 0 and version < MAX, `settle(executor, id, true)` is Ok for every Dispatched, Active or Unknown record (with up to two other open records). |
| F-1, F-2 | `h_wedge`, `h_wedge_healthy` | Covers that must be SATISFIED (F-1 from `Pool::new`, F-2 from INV states; "Findings"), plus TX-0 on the failed calls. |
| REF-1 | `h_ref_submit`, `h_ref_dispatch`, `h_ref_ack`, `h_ref_cancel`, `h_ref_settle`, `h_ref_pause`, `h_ref_observe`; plain `h_step_*`, `h_version_max`, `h_bmc_from_init`, `h_ref_traces` | Guard exactness: for every INV state and operation of the step bounds, the core's `Err` is the TLA+ error operator's value (`SubmitErr`, `DispatchErr`, `AckErr`, `CancelErr`, `SettleErr`, `PauseErr`, `ObserveErr` of `alpha(pre)`; `InvalidConfig` is no operator value), and the core returns `Ok` exactly when the operator gives "Ok" (the action is enabled) or "Noop" (the idempotent paths, a stutter in the model; TraceCheck's `Agrees`). At `version = u32::MAX` no action is enabled (plain `h_version_max`). |
| REF-2 | as REF-1 | Effect exactness: when the operator gives "Ok", `alpha(post)` is the action's post-state (`T::next`: every TLA+ variable but the history variables `steps` and `last`, configuration included), with the implementation's ghost (`base`, `k0` from the core's post-state after an `Ok` observe); submit returns the new record's index (`NR`). |
| REF-3 | as REF-1 | Stutter: every other call (an error, or "Noop") leaves `alpha` unchanged, configuration, values, version, `observedAt`, `paused`, records and ghost. |
| F-3 (the cycle) | `h_slot_cycle_0`, `h_slot_cycle_2` | From `Pool::new` and from quiet INV states with 2 records (version below `u32::MAX - 2`): an accepted submit is the spec's; the same caller's cancel of the new record is then accepted; afterwards the values, `paused`, `observed_at` and the older records are as before, the new record is Cancelled and the version is 2 higher; and the same request at the new version is accepted with the same reserved values and the next id. Cover "slot cycle: ..." SATISFIED. |

## Covers (vacuity)

Every cover is a `kani::cover!` in the `_cov` part of its family (or in
the harness itself). `run.sh kani` collects every cover's status from
every log and requires each to be SATISFIED in some harness of its family
(`run.sh`'s `family()`: one family per body); `h_canary` must FAIL on its
own assertion ("canary: no submit is ever accepted") and on nothing else.
With `KANI_DEFER_COVERS=1` each job writes its harness's `covers.tsv`
instead, and the family check is `tools/scripts/check-covers.sh` over the
logs (CI's `covers` job: the hosted groups' logs with the local record's
covers). The table counts the covers in the logs this file's runs read
(and W7's `covers.tsv` where named); a family whose harnesses are partly
CI's hosted `kani` job's is complete only in CI's `covers` job.

v1.1.0's cover changes: v1.0.0's finding witnesses are gone where v1.1.0
fixed the finding, replaced by covers of the fix ("Findings"); the `h_ref`
family has 43 covers (v1.0.0: 46; v1.1.0 deviation 7); new families
`h_version_cap`, `h_wf`, `h_live_all`.

<!-- kani-results-covers: begin (kani/local/results-tables.py writes this block; do not edit) -->
| Family | Covers (in the logs read) | SATISFIED | Not satisfied | Harnesses read |
|---|---:|---:|---|---|
| `h_init` | 2 | 2 | none | 1 of 2 in the runs; with W7 P3's covers.tsv |
| `h_init_inv_2` | - | - | - | 0 of 1 in the runs |
| `h_validate` | 11 | 11 | none | 1 of 1 in the runs; with W7 Xval's covers.tsv |
| `h_energy` | 4 | 4 | none | 2 of 2 in the runs |
| `h_energy_max` | 1 | 1 | none | 1 of 1 in the runs |
| `h_preview` | 6 | 6 | none | 2 of 2 in the runs |
| `h_best` | - | - | - | 0 of 1 in the runs |
| `h_live_all` | 12 | 12 | none | 1 of 1 in the runs |
| `h_wedge` | 3 | 3 | none | 1 of 1 in the runs; with W7 P4's covers.tsv |
| `h_wedge_healthy` | 1 | 1 | none | 1 of 1 in the runs |
| `h_bmc_from_init` | 2 | 2 | none | 8 of 8 in the runs |
| `h_slot_cycle` | 1 | 1 | none | 2 of 2 in the runs |
| `h_full` | 6 | 6 | none | 3 of 3 in the runs |
| `h_canary` | - | - | - | 1 of 1 in the runs |
| `h_version_cap` | 7 | 7 | none | 7 of 7 in the runs |
| `h_version_max` | 6 | 2 | 4 in these logs (not measured here: `h_version_max_ack`, `h_version_max_cancel`) | 1 of 3 in the runs |
| `h_wf` | 7 | 7 | none | 5 of 5 in the runs |
| `h_step` | 67 | 37 | 30 in these logs (not measured here: 15 harnesses) | 34 of 49 in the runs |
| `h_ref` | 43 | 43 | none | 3 of 7 in the runs, 4 by W7 only; with W7 ref's covers.tsv |
<!-- kani-results-covers: end -->

## Plain mode

`run.sh plain` (`cargo test --release`): every harness body with the
random source, and the plain-only tests: `window_walk` (60 seeds x 3,000
steps through prunes), `cap_walk`, `f1_brief_instance`,
`f2_small_instance`, `f2_risk_fixture`, `f3_slot_exhaustion` (300
submit/cancel cycles: ids 0..299, `first_id` 44, id 43 `NotFound`),
`f5_open_records_resolve_above_the_cap`, `f6_refresh_with_records_open`,
and `h_ref_traces` (REF-1..3 on every call of the 216 conformance traces,
8,865 calls, each replayed on the core against its recorded result and
post-state: 1 refused at init, 11 observe refreshes, 400 re-bases, 3
prunes folded into the ghost). REF-1..3 are also checked on every sample
of the step tests, `h_version_cap`, `h_bmc_from_init`, `h_wf` (WF states,
the two lines no INV state reaches) and every step of `checked_step` (the
walks and f-tests), with `spec::ghost_step`'s ghost equal to the TLA+ one
(plan 2.2). `M`/`GR` are 8 in plain mode, so the fixtures (`risk`, `gpu`)
run through the same spec (v1.1.0 deviation 1). Every cover a body
evaluates must fire at least once.

Result at `6b120fd`: 33 passed, 0 filtered out, 11 s (`h_ref_traces`
0.05 s). **This is testing, not proof.**

## Mutants

`run.sh mutants [--kani]`: 45 mutants (`mutants/*.patch`): 44 of the
core, each applied to a temporary copy of `crates/core`, and one of the
TLA+ transcription (31, `# applies-to: kani`: applied to a copy of this
crate's `src`). A mutant to be killed must make plain mode fail and, with
`--kani`, the harness named first in its `# killed-by:` header FAIL
(`run.sh` stops at the first kill; CBMC stops at the first violated
property, the covers compiled out of the mutant's copy). Mutant 20 is
equivalent (`# must-survive`): plain mode and every `# checked-by:`
harness must pass on it, as proofs. A job's verdict is expect / plain /
Kani (`killed/killed/killed`, or `survive/survived/survived` for 20).

Plain mode, every mutant: `run.sh mutants` at `6b120fd`, rc 0 (386 s):
44 killed, 20 survived. CI's `mutants` job reruns all 45 in plain mode on
every run; `kani-mutants` (manual runs) reruns the hosted ones under
Kani. The Kani column below is this file's runs' (a mutant's leg is the
harness its job runs), W10's mutant-31 run, or the job that holds it.
Time is the whole job (plain mode, the mutant tree's build and the Kani
leg). Mutants 32-45 are new in v1.1.0: each reverts a v1.1.0 fix (the
v1.0.0 rule) or breaks a new rule.

<!-- kani-results-mutants: begin (kani/local/results-tables.py writes this block; do not edit) -->
45 mutants (`mutants/*.patch` at `c8dd19f`): 1 run under Kani in the runs (Run column; all as expected), 1 earlier (W10) only, 43 not run under Kani here. Plain mode: every one as expected at `6b120fd` (`run.sh mutants`), and CI's `mutants` job reruns all of them.

| Mutant | What it changes | Expected | Plain | Kani (harness: violated property) | Time (s) | Peak (GB) | Run | CI job |
|---|---|---|---|---|---:|---:|---|---|
| `01-guard-swap-paused-stale` | candidate: Stale checked before Paused | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `02-guard-swap-dispatch-auth-paused` | dispatch: Paused checked before Unauthorized | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_dispatch_tx` | - | - | est. 5.8 GB (its harnesses) | `kani-mutants` |
| `03-submit-expiry-off-by-one` | candidate: a request expiring now is accepted (<= became <) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `04-dispatch-expiry-off-by-one` | dispatch: dispatch at the expiry instant is accepted (>= became >) | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_dispatch_tx` | - | - | est. 5.8 GB (its harnesses) | `kani-mutants` |
| `05-check-bounds-without-skip` | settle(completed): check_bounds keeps the settled record in the envelope (skip dropped) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_settle_tx` | - | - | est. 15.7 GB (its harnesses) | `kani-mutants-large` |
| `06-envelope-ignores-reserved` | envelope: Reserved records are left out (pending hedges ignored) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `07-release-without-bounds-check` | release: no check_bounds before undoing a reservation. | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_wf_cancel` | - | - | est. 12.8 GB (its harnesses) | `kani-mutants` |
| `08-dispatch-version-not-bumped` | dispatch: version not incremented | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_dispatch_tx` | - | - | est. 5.8 GB (its harnesses) | `kani-mutants` |
| `09-ack-noop-bumps-version` | acknowledge: the reverted reconcile griefing (a repeated ack bumps the version) | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_ack_tx` | - | - | est. 3.1 GB (its harnesses) | `kani-mutants` |
| `10-settle-accepts-opposite-terminal` | settle: the opposite terminal outcome is applied (Completed can be released) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_settle_tx` | - | - | est. 15.7 GB (its harnesses) | `kani-mutants-large` |
| `11-release-counts-completed-units` | release: the released units are also added to completed_units (cancelled work counts for prerequisites). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_cancel_post` | - | - | est. 17.0 GB (its harnesses) | `kani-mutants-large` |
| `12-energy-ge` | candidate: equal energy rejected (> became >=) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `13-cancel-allows-dispatched` | cancel: a Dispatched record can be cancelled (reservation released while in flight) | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_cancel_tx` | - | - | est. 7.6 GB (its harnesses) | `kani-mutants` |
| `14-observe-ignores-open-records` | observe: no Pending check (a new baseline replaces the values under open obligations) | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_observe_tx` | - | - | est. 12.7 GB (its harnesses) | `kani-mutants` |
| `15-submit-actor-check-dropped` | candidate: caller not checked against the recipe actor | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `16-settle-executor-check-dropped` | settle: caller not checked against the executor | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_settle_tx` | - | - | est. 15.7 GB (its harnesses) | `kani-mutants-large` |
| `17-cancel-owner-check-dropped` | cancel: the owner can no longer cancel (actor only) | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_cancel_tx` | - | - | est. 7.6 GB (its harnesses) | `kani-mutants` |
| `18-preview-candidate-divergence` | preview: admission evaluated one second later than submit would (preview/candidate divergence) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_preview` | - | - | est. 30.0 GB (its harnesses) | `kani-mutants-large` |
| `19-recovery-gt` | candidate: recovery accepts a completion that does not decrease energy (>= became >) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `20-observe-length-check-dropped` | observe: vector length not checked. | survive | survived | survived (`h_step_observe_post`, `h_step_observe_tx`) | 363 | 12.7 | run 3 | `kani-mutants-large` |
| `21-full-off-by-one` | candidate: Full only above 256 records (>= became >). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_full_256` | - | - | est. 39.2 GB (its harnesses) | `kani-mutants-large` |
| `22-version-wraps` | bump: version wraps at u32::MAX instead of failing. | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_version_max_ack` | - | - | est. 3.4 GB (its harnesses) | `kani-mutants` |
| `23-envelope-ignores-extra` | envelope: the requested recipe's own completion is not included | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `24-best-ties-to-fewer-units` | best_quantity: ties go to the smaller quantity | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_best` | - | - | est. 3.9 GB (its harnesses) | `kani-mutants` |
| `25-energy-excess-sign` | energy_terms: Excess measured below the target (sign flip) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_energy_exact` | - | - | est. 17.5 GB (its harnesses) | `kani-mutants-large` |
| `26-ack-accepted-inverted` | acknowledge: accepted maps to Unknown and rejected to Active | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_ack_tx` | - | - | est. 3.1 GB (its harnesses) | `kani-mutants` |
| `27-add-bound-check-dropped` | add: result not checked against +-1e6 (only i64 overflow). | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_wf_cancel` | - | - | est. 12.8 GB (its harnesses) | `kani-mutants` |
| `28-check-bounds-vs-candidate` | check_bounds: compares against the violations of the candidate only (old computed on the candidate, the v1.0 mutant's change), not min(current, candidate). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_tx` | - | - | est. 19.2 GB (its harnesses) | `kani-mutants-large` |
| `29-observe-keeps-observed-at` | observe: the new baseline keeps the old observation time (snapshot age never refreshed) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_observe_post` | - | - | est. 13.1 GB (its harnesses) | `kani-mutants-large` |
| `30-full-rejects-at-255` | candidate: the 256th record is refused (Full one record early, with an open front) | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_full_255` | - | - | est. 27.9 GB (its harnesses) | `kani-mutants-large` |
| `31-tla-release-skip-dropped` | transcription: ReleaseErr checks the envelope without skipping the record it releases (CheckBounds(.., -1) for CheckBounds(.., id)), so the TLA+ side counts that record's own release and completion terms as if it stayed open and rejects (Bounds) a cancel the core accepts: REF-1 (the transcription is load-bearing, not only the core). | killed | killed (6b120fd) | killed (`h_ref_cancel`: "REF-1 guard exactness: the core's result is the TLA+ error operator's") | 161 | 9.8 | W10 at 6b120fd | `kani-mutants` |
| `32-envelope-no-release-term` | envelope: the release outcome x of an open record is left out (the v1.0.0 envelope: low += d.min(0), high += d.max(0)). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_inv` | - | - | est. 25.0 GB (its harnesses) | `kani-mutants-large` |
| `33-check-bounds-old-only` | check_bounds: the envelope's violation is compared with the current violation only (the v1.0.0 rule `v > old[i]`), not min(old, new): a submit whose candidate is worse than the current values passes as long as the envelope is no worse than now (INV-RES). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_submit_inv` | - | - | est. 25.0 GB (its harnesses) | `kani-mutants-large` |
| `34-pause-without-cap` | set_paused: the plain increment (bump), not bump_new: pausing at the cap succeeds and eats the reserve of the open records (VER-CAP, INV-VERCAP). | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_version_cap_pause` | - | - | est. 5.0 GB (its harnesses) | `kani-mutants` |
| `35-candidate-without-cap` | candidate: the VERSION_CAP check deleted: a submit at the cap is admitted (VER-CAP, INV-VERCAP). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_version_cap_submit` | - | - | est. 18.0 GB (its harnesses) | `kani-mutants-large` |
| `36-full-ignores-front` | candidate: Full whenever 256 records are retained (the v1.0.0 rule), even with a terminal front that could be pruned (WIN-1, WIN-2). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_full_256_prune` | - | - | est. 20.5 GB (its harnesses) | `kani-mutants-large` |
| `37-validate-actor-is-executor` | validate: a recipe whose actor is its executor is accepted (CFG-2). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_validate` | - | - | est. 36.7 GB (its harnesses) | `kani-mutants-large` |
| `38-observe-no-refresh` | observe: Pending whenever a record is open, even when the values are the current ones (no snapshot refresh with records open; OBS-2). | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_observe_tx` | - | - | est. 12.7 GB (its harnesses) | `kani-mutants` |
| `39-cancel-completed-pending` | cancel: a Completed record is Pending (the v1.0.0 result), not InvalidState (TERM-1). | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_cancel_tx` | - | - | est. 7.6 GB (its harnesses) | `kani-mutants` |
| `40-settle-no-completed-units` | settle(true): completed_units not updated: the lifetime count that prerequisites read misses the completion (INV-CU). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_step_settle_inv` | - | - | est. 20.5 GB (its harnesses) | `kani-mutants-large` |
| `41-completed-units-wrap` | settle(true): completed_units wraps instead of saturating. | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_wf_settle` | - | - | est. 16.0 GB (its harnesses) | `kani-mutants-large` |
| `42-prune-keeps-first-id` | submit: the prune removes the front record but first_id is not advanced: every retained id shifts by one (WIN-2, RD-ID). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_full_256_prune` | - | - | est. 20.5 GB (its harnesses) | `kani-mutants-large` |
| `43-index-no-filter` | index: an id at or past first_id + len is not NotFound: record() indexes past the end of records and panics (ROB-1). | killed | killed (6b120fd) | CI `kani-mutants` (not measured here); leg `h_step_dispatch_tx` | - | - | est. 5.8 GB (its harnesses) | `kani-mutants` |
| `44-prereq-scan-retained` | candidate: prerequisites counted from the retained Completed records (the v1.0.0 scan), not completed_units: once the completions are pruned a prerequisite that was met is refused. | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_wf_submit` | - | - | est. 20.1 GB (its harnesses) | `kani-mutants-large` |
| `45-cap-in-submit-only` | The VERSION_CAP check is in submit, not candidate: preview (and best_quantity) at the cap allow a request that submit refuses with Arithmetic (RD-PREVIEW). | killed | killed (6b120fd) | not run at v1.1.0 (declared in `not-run.tsv`); leg `h_preview` | - | - | est. 30.0 GB (its harnesses) | `kani-mutants-large` |
<!-- kani-results-mutants: end -->

## Findings

Whether a property failed in a v1.1.0 Kani job this file shows is the
summary's **Failures** line, generated from the runs; this section adds
no result of its own. Where that line is "none", within the bounds every
asserted property of the harnesses verified in the tables holds on the
pinned core, the executable specification agrees with the code on every
result, post-state and read, and every call refines the v1.1.0 TLA+
model (REF-1..3). A failed property there would be a new defect or a
spec-versus-code divergence at v1.1.0, and the line names its job.

v1.0.0's findings F-1 to F-6 are fixed or bounded in v1.1.0 (upstream PR
#4). Where v1.0.0 had a witness that the finding happens, v1.1.0 has a
cover or an assertion of the fix; v1.0.0's witnesses (this file at
`a072a2d`, Findings, and its `kani/local/covers.tsv`) are what fails on
v1.0.0. Mutants 32-45 each put a v1.0.0 rule back or break a new rule;
they are killed in plain mode (`run.sh mutants` at `6b120fd`), and under
Kani as the Mutants table shows (run here, CI's `kani-mutants`, or
declared not run):

| Finding | v1.1.0 Kani evidence | Plain mode |
|---|---|---|
| F-1, wedge after a disturbing observe (fixed) | `h_wedge` asserts each exit of the dispatched hedge in a disturbed state and covers "wedge: submit refused with Bounds after a disturbing observe" | `f1_brief_instance` |
| F-2, healthy-state release wedge (fixed) | `h_wedge_healthy`: "the position releases (F-2 fixed)"; `h_live_all`: the F-2 shape | `f2_small_instance` (`[Ok(Some(0)), Err(Bounds)]`), `f2_risk_fixture` |
| F-2b, a release fails (fixed) | `h_step_*_inv`: INV-RES (a resolution's release never breaks the envelope); LIVE-ALL | the step tests |
| F-3, slot exhaustion (fixed: prune) | `h_full_256` (Full only with an open front), `h_full_256_prune` (WIN-1, WIN-2, WIN-ESC with a prune), `h_full_255` | `window_walk`, `f3_slot_exhaustion` |
| F-4, actor = executor accepted (fixed) | `h_validate`: "InvalidConfig with actor = executor (F-4 fixed)" | `h_validate` |
| F-5, frozen at `u32::MAX` (fixed: VERSION_CAP) | `h_version_cap_*` (VER-CAP), `h_version_max_*` (TX-VER at `u32::MAX`), INV-VERCAP | `cap_walk`, `f5_open_records_resolve_above_the_cap` |
| F-6, stale snapshot with an open record (fixed: refresh) | "observe: Ok refresh with an open record", REFRESH-1 | `f6_refresh_with_records_open` |

Each Kani entry holds only within the bounds ("Bounds", "What is not
shown"), and only once its harness is verified in the tables above (or
in CI's hosted `kani` job, for the hosted ones).

## Deviations from the plan, and why

### v1.1.0

v1.1.0 adds these seven to v1.0.0's 26 (below, numbered as in v1.0.0;
they stand for v1.1.0 where the harness still exists):

1. `spec::M` and `spec::GR` depend on `cfg!(kani)`: 2 and 3 under Kani
   (the harness bounds), 8 in plain mode, so the fixture tests (`risk`, 4
   metrics; `gpu`, 5 metrics and 6 recipes) run through the same spec.
   For the same reason plain `checked_step` checks REF at image
   `T<8,64,8,8,256>`.
2. INV domain: at most 3 records with `first_id = 0`. WF domain: 2
   records, free `first_id` and `completed_units`. 256 concrete records
   only in `h_full_*`. A prune under INV, and the REF check of a prune and
   its ghost fold, only in plain mode (`window_walk`, `h_ref_traces`: 3
   folds in the corpus).
3. `h_full_256_prune` builds its records from the constant array of plan
   section 4 (as v1.0.0's deviation 21 for `h_full_255`).
4. `h_wedge` takes one nondeterministic exit call (plan R3), which keeps
   its peak under v1.0.0's.
5. `memcmp.0:25` in every harness's unwindset (W7's probe P5): CBMC's
   library `memcmp`, which `[i64]` equality calls, would otherwise get the
   harness bound; the run without it fails on that unwinding assertion
   only.
6. `h_ref_traces` was filtered from plain mode until the v1.1.0
   conformance corpus and `conform.rs` landed (W10, `6b120fd`); it runs
   since.
7. Five `ref ...` covers of v1.0.0 are gone, because v1.1.0 makes them
   unreachable on INV states: ack/cancel/settle `Arithmetic` (INV-VERCAP:
   the cap error is submit-side, `VerErr` only at `u32::MAX`) and
   cancel/settle `Bounds` (INV-RES: a resolution's release never breaks
   the envelope). REF-1 still checks those error values on every call:
   the core and the transcription must agree that they do not occur.

### v1.0.0

All but two came from CBMC running out of memory or time on the design as
planned (cost ladder below); 25 applies deviation 20's design to a harness
added later, without measuring the direct design first, and 26 records
choices for REF-1..3 that the plan left open. None weakens an
assertion: each either narrows the
inputs (stated in Bounds), replaces a library function by a model that can
only make a harness fail, rewrites a spec function into an equal one that
CBMC handles better, or splits a property into harnesses whose conclusions
combine (deviations 18 to 25; in 23 and 24 a core function is replaced
by a free result in one harness and pinned down by others).

1. **Fixed-capacity `Vec::with_capacity_in` model** (`model.rs`, every
   harness except `h_validate`, `h_energy_max`, `h_full_*`). The core's
   fallible `collect` in `add` merges its error exit into the result, so the
   length of `next` is symbolic, and `values.to_vec()` then allocates a
   symbolic size, which CBMC encodes with the array theory (a single submit
   with 1 metric, 1 term, 1 recipe and no record ran out of 20 GB). The model
   returns an empty vector with a constant capacity (24 bytes, 3 large
   elements, 4 otherwise) and asserts that the request fits, so a larger
   request fails the harness; std guarantees only "at least `capacity`", and
   the core never reads a capacity. `Vec::reserve` (called by `extend` and
   `collect` with a symbolic size hint: `release`'s `undo`, `violations`) is
   modelled the same way: it gives an empty vector the constant capacity and
   asserts that `additional` fits in the spare capacity. std reallocates only
   when `capacity - len < additional`, so the model never removes a path the
   code takes within the constant capacity; the std growth path it replaces
   was 41M of the 45M clauses of a cancel step (cost ladder).
2. **Spec integers are `i64` with overflow checks**, not `i128`, and
   `d·u` is a 32-bit product (`mul_du`, its preconditions asserted): every
   spec quantity fits by a wide margin (energy ≤ 2.56e17), and an overflow
   would be a failed check, never a silent wrap. `i128` doubled every
   multiplier.
3. **Concrete-slot reads and writes.** Every lookup by a symbolic index in
   the spec goes through a loop over concrete slots (`rcp`, `rec`), and the
   generated records and the vectors of the raw configurations are written
   at concrete offsets and then truncated: an index into a heap buffer is a
   symbolic byte offset, a mux over every byte of the buffer. (With
   conditional pushes in the raw configurations, `h_init` and `h_validate`
   ran out of 40 GB in the propositional encoding after 0.56M and 0.97M
   symbolic-execution steps.)
4. **Narrow operation arguments under Kani** (`Src::wide` is false): request
   recipes in `0..4`, units in `0..8`, record ids in `0..8`. Each is only
   compared (`recipe >= #recipes`, `units == 0 || units > max_units`,
   `id < #records`) before it reaches arithmetic or an index, so the
   ranges reach every guard outcome; plain mode also draws them from all of
   `u32`.
5. **Equal-length accounts and bounded string equality** (`eq_str`,
   `eq_text`, `eq_i64s`, `same_config`, `same_state`). `==` on `str` is a
   `memcmp` whose length CBMC cannot fold, each byte read a case split over
   every string object in the state (12.5M of 27M clauses in a submit). The
   lengths are compared first and then at most `K` bytes (2 for accounts);
   a longer string fails the harness on an assertion, it is never compared
   partially. `preview`'s strings are read once into a local array
   (`Text<K>`, again asserting the length): the `reason` (16 bytes; the
   longest reason, `SnapshotExpired`, has 15) is compared there with the
   spec's, and each number (24 bytes) is parsed there and compared with the
   spec's number (`is_dec`: the canonical decimal numeral, 1 to 20 digits
   and no leading zero unless it is `0`, is unique, so this is
   `s == x.to_string()`; plain mode checks the parser against `to_string`
   on every call). With the reason compared three times on the heap
   `h_preview` was 62.2M clauses; with the spec formatting its numbers by
   `to_string` (a 64-bit division per digit pair) `h_preview_post` had no
   CNF after 20 minutes. The same holds for value vectors: `y.values == x.values`
   (PAUSE-1) is a `memcmp` over `[i64; 2]` that CBMC did not unroll within
   the bound (`h_step_cancel_post` failed its unwinding assertion), so it is
   `eq_vals`, a loop over the two elements.
6. **Configuration equality in the step harnesses only in `_post`.** Under
   Kani, `same_state` (TX-0, TX-NOOP, AUTH-1's "changed", PAUSE-1) and the
   post-state checks of `h_full_*` and `h_bmc_from_init_*` compare values,
   version, `observed_at`, `paused` and records, not the configuration;
   `_post` proves that no step changes the configuration (frame of
   `post_matches`), so the two together give the full equality. In
   `h_full_*` the configuration is not copied or compared under Kani at all
   (at its unwind of 258 the copy's loops and then the slice comparisons
   of `same_config` were unrolled about 257 times each; cost ladder 28c,
   28d); there the frame is `h_step_submit_post`'s over 2 records. Plain
   mode compares everything everywhere, `h_full` included.
7. **Fixed shapes and record counts** (deviation from the brief's ≤ 2
   metrics, ≤ 2 terms, ≤ 3 records in the steps): 1 metric, 1 term, 2
   recipes, exactly 2 records (3 in `h_live1`; 0, 1 and 3 in
   `h_step_<op>_inv_<k>`, one harness per count, so that the invariants'
   induction covers every count up to 3). A symbolic record count
   doubled the formula and each further metric, term or recipe multiplies it
   (cost ladder); with 2 metrics, 2 terms and 3 records the submit step did
   not fit in 26 GB. 2 metrics and 2 terms are covered by `h_energy*`; all
   shapes up to 2/2/2/3 by plain mode and the other layers.
8. **`pool_of` instead of `Pool::new`** in `h_energy*`, `h_energy_max`,
   `h_wedge*` and `h_bmc_from_init` (under Kani; plain mode calls
   `Pool::new`). Validating a config that is valid by construction cost
   more than the energy (13.8M of 52.5M clauses); in `h_energy_max`, without
   the capacity model, `validate` unrolled each requirement loop 66 times
   inside a recipe loop unrolled 66 times. `pool_of` builds `Pool::new`'s
   Ok value directly (CFG-1, shown by `h_init`, says it is the same; its
   INV too).
9. **Energy exactness by a same-circuit product.** Under Kani each term is
   compared with `w·r²` computed by a wrapping `u64` product after
   asserting `r ≤ 2e6` and `w ≤ 1000` linearly, so equality modulo 2^64 is
   equality; an `i128` product against the code's `u64` product (a miter of
   two different multipliers) did not finish. Exactness runs on anchored
   values (`h_energy_exact`), the Ok/`Bounds` contract over the full range
   (`h_energy`).
10. **Parts.** Each step harness is four CBMC runs: `_tx` (exact result and
    the result-level properties), `_post` (exact post-state, frame, LIFE-1,
    ASYNC-1, TERM-1, HARD-STEP, PAUSE-1), `_inv` (INV-* after the step),
    `_cov` (covers). Each SATISFIED cover is one more SAT call, and with the
    assertions in the same run every call carries the whole formula.
11. **Per-loop unwinding** (`run.sh`, `short_loops`). The core's iterator
    loops over values, deltas, terms and requirement pairs (at most 2
    elements in the steps; 3 recipes and 2 pairs in `h_init` and
    `h_validate`, bound 4) get bound 3, the record and string loops the
    harness unwind (4 to 7; 24 in `h_preview_post`, 258 in `h_full_*`).
    `h_energy_max` (64 terms) has no short loops. Some loops get their own
    bound: the 24- and 20-byte loops of `spec::Text::<24>::of` and
    `spec::parse_dec` 25, the 16-byte loops of `spec::Text::<16>` 17,
    `observe`'s loop over the observed values (`any(!bounded)`) 4 (it runs
    after the length check, so on n ≤ 2 values; mutant 20, which drops that
    check, runs it on the observe domain's vectors of up to 3 values),
    `spec::eq_i64s`'s loop over `M + 2 = 4` elements 5 (above the unwind of
    `h_init` and `h_validate`, 4, where it failed its unwinding assertion),
    `best_quantity`'s two loops over `1..=units` 8 (`h_best`: units ≤ 7
    under Kani), `gen::copy_vec`'s loops over values, deltas,
    requirements, metrics and terms, the slice comparisons over
    requirements and terms and `spec::prereqs_met`'s loop over a recipe's
    requirements 4 in `h_full_*` and `h_preview_post` (CBMC does not fold
    those lengths: at `h_full`'s unwind of 258 each copy was unrolled about
    253 times, and `prereqs_met` repeated its scan of the 255 records once
    per unrolled requirement), and in `h_energy_max` the loop inside each
    `next()` of `energy_terms`' fallible `collect` 2 (it exits in its first
    iteration, but CBMC does not fold it, and at 66 it was unrolled 66 times
    inside the 66 of the `collect`). Unwinding assertions stay on, so a
    bound that is too small fails the harness; it never hides a path.
12. **`assume` returns a constant `true` under Kani** (`src.rs`), so an
    early return after an assumption is dead code: a live early return
    merged every field of the returned state with an unconstrained value.
13. **Reachability checks of assertions off** (`--no-assertion-reach-checks`;
    one SAT round per assertion). Vacuity is covered by the covers.
14. **`h_bmc_from_init` at depth 2** (plain mode: depth 5). Every level adds
    every operation's formula again. The covers "a record completes" and
    "observe after a completion" need depth 3 and 4, so they are plain
    mode's (`h_bmc_from_init`, whose body is plain-only; the Kani
    harnesses run `bmc_two`).
15. **`h_init` and `h_validate` under Kani: at most 2 metrics, strings of
    0 or 2 bytes, the capacity model.** `h_init` draws raw configurations
    only (they contain every valid configuration of the domain; a second
    builder doubled the heap objects every pointer may refer to). Both draw
    at most 2 metrics and vectors of at most 2 deltas (`RAW_M`; 3 recipes,
    2 terms and 2 requirements per recipe as planned) and strings of 0 or 2
    bytes (`INIT_LONG`). `validate` reads only the length of a string
    (`short`: 1 to 128 bytes), so the 128/129-byte boundary of `short` is
    plain mode's, as are 3-metric raw configurations. The runs before: with
    129-byte strings `h_init` did not finish symbolic execution in 30 min
    (the requirement-pair loops unrolled 6 to 8 times); `h_validate` with
    3 metrics, long strings and no capacity model (1.09M symbolic-execution
    steps) and `h_init` with 3 metrics, short strings and the capacity model
    (1.66M steps) ran out of 40 GB.
16. **`--no-memory-safety-checks`.** The core is `#![forbid(unsafe_code)]`;
    pointer checks would only re-verify std (ASM-TOOLS). Panic, overflow,
    bounds and unwinding checks stay on.
17. **Spec rewrites that CBMC handles better** (equal functions):
    `inv_healthy` computes each slot's liveness and completion deltas once
    and sums them over the 16 subsets (it recomputed them per subset), and
    `valid`'s delta-chain loop is one loop per vector instead of a chained
    iterator.
18. **PAUSE-1 for the transparent operations at the spec level under
    Kani.** The plan called the code a second time on the pre-state with
    `paused` flipped; that doubled the formula and the cancel and settle
    `_post` runs went out of 32 GB. The harness asserts that the spec's
    outcome does not depend on `paused`; `_tx` and `_post` assert that the
    code's outcome is the spec's from every state of the domain (`paused`
    is free), so the code's outcome does not depend on it either. Plain
    mode also calls the code again.
19. **The wedge harnesses under Kani: fixed roles in `h_wedge`, a healthy
    INV state in `h_wedge_healthy`.** The plan's `h_wedge` (symbolic roles,
    `Pool::new`) ran out of 40 GB after 26 min. Under Kani its owner is
    `o_`, every actor `a1`, every executor `x1` (with symbolic roles every
    `==` on an account is a case split over the four accounts); the
    configuration stays symbolic, so its TX-0 assertions on the wedged
    state hold for every instance of the domain. `h_wedge_healthy`'s script
    makes nine calls on the core over two records, and every outcome is
    symbolic, so the length of the record vector is too from the first
    submit on: with the spec's steps and the invariants it was stopped in
    symbolic execution; with the script on the core alone it ran out of 40
    GB (3.7M steps); and on one concrete configuration with the spec's
    checks it still ran out of 40 GB (5.2M steps, and 20,347 verification
    conditions left after simplification: CBMC did not reduce the concrete
    run to constants; we did not find out which values stayed symbolic).
    Under Kani it starts from the state the script reaches, any
    healthy INV state whose record 0 is Dispatched and record 1 Completed,
    and covers the wedge there; TX-0 for the three failed calls and LIVE-1
    for record 0 are `h_step_{settle,cancel,observe}_tx`'s and `h_live1`'s
    over these states. Reachability from `Pool::new` is plain mode's
    (`h_wedge_healthy`, `f2_small_instance`, `f2_risk_fixture`), and the
    TLA+ and sandbox layers'.
20. **`h_bmc_from_init` under Kani: the first operation a submit, pause or
    observe, the state after it rebuilt, one harness per second kind after
    a submit.** From `Pool::new` there is no record, so every other kind
    fails (`NotFound` or earlier) and changes nothing. The plan's version
    (`Pool::new`, INV of the initial state, any kind first) was still in
    symbolic execution after 16 minutes; with the first kind restricted and
    any kind second it ran out of 40 GB (6.3M symbolic-execution steps);
    with one harness per second kind it timed out at 30 minutes (2.8M
    steps, 37 GB; 2.4M without the copy of the pre-state). The cause is the
    first operation's symbolic outcome: the record count and the value
    vector after it depend on it, so every later read is a case split and a
    second submit pushes at a symbolic offset. So `bmc_two` asserts the
    exact result and the exact post-state of the first operation (the
    spec's: values, version, `observed_at`, `paused`, the pushed record),
    then rebuilds that state with constant shapes (the configuration moved,
    not copied) and runs the second operation from it, with INV after both.
    After a submit it continues from the `Ok` outcome only: an `Err` leaves
    the initial pool (asserted above), and the pause/observe harness
    continues from it too (a pause by a non-owner fails). `_po` takes any
    second kind (from a state without records every kind but submit, pause
    and observe fails `NotFound`); `_s_<op>` fixes it. Together they cover
    every 2-operation trace from `Pool::new` within the bounds.
21. **`h_full` split into `h_full_255` and `h_full_256`**, each with a
    concrete record count, **the records one constant array**, and CBMC's
    field sensitivity raised. With a symbolic count (255 or 256) every push
    was guarded, the vector length symbolic from the first record on, and
    63 of the 256 pushes took 9 minutes of symbolic execution (cost ladder
    24). `h_full_*` run CBMC with `--max-field-sensitivity-array-size 8192`
    (`run.sh`, `extra_cbmc`): the records' buffer (257 × 24 bytes) is one
    byte array, above CBMC's default limit of 64 elements, so no read of a
    record folded to its constant fields, both envelopes took the `open`
    branch (a multiplication) for each of the 255 records, and symbolic
    execution was still in the core's envelope after 13 minutes; with the
    limit above the buffer's size each byte is its own symbol and the reads
    are constants (cost ladder 28e to 28g). But then every write into the
    buffer rewrote all of its bytes: built by 255 pushes, the records cost
    16 to 17 minutes of symbolic execution (8.05M steps) and the first SAT
    call did not finish in the rest of the 30 minutes (28h, 28i: the
    harness timed out, with the steps' values and with values 0). The
    records are now `OLD_RECORDS` (`checks.rs`), 257 Cancelled records of
    recipe 0 with ids `0..257` computed by the compiler (a `const fn`),
    moved into the vector by `Vec::from` (one write of the whole buffer)
    and truncated to 255 or 256, so the buffer has room for the core's push
    and never reallocates (mutant 21 pushes at 256). The post-state check
    compares the old records over `0..count`, a concrete trip count (it
    was the post-state's length, 255 or 256). The pool is the same as
    before: the same 255 or 256 records, the same configuration, values and
    request domains, the same assertions. `h_full_255` then took 2.42M
    steps (493 s of symbolic execution) and two SAT calls of 147 s and
    93 s (37.9M clauses), and passes in 764 s (28j); `h_full_256` 46 s (it
    took 376 s). The option and the
    construction change the encoding and the capacity of the vector, not
    the program under test or the properties. Under Kani `h_full_*` do not
    evaluate INV-DEP (true by construction: every record is of recipe 0,
    which has no prerequisite) or INV-WF of the whole post-state (the old
    records are asserted unchanged and are well formed by construction,
    the new one is asserted to be the spec's, and the count is asserted to
    be 256); plain mode evaluates both. The configuration is not compared
    under Kani (deviation 6).
22. **`h_version_max` split by operation kind** (`h_version_max_<op>`,
    one cover family). All seven kinds in one formula (73.1M clauses, 35 GB)
    timed out at 30 minutes.
23. **`h_init` with a free `validate`, and `h_init_inv`.** `h_init` stubs
    `Config::validate` by `model::validate`, whose result is free and
    recorded, and asserts that `Pool::new` calls it, is `Ok` exactly when
    it is, fails only with `InvalidConfig`, and builds the specified
    initial pool. `h_validate` shows that the real `validate` is the spec's
    validity over the same raw configurations, so the two give CFG-1's
    "`new` Ok iff valid". The INV of the initial pool is `h_init_inv`'s
    and `h_init_inv_2`'s, over valid configurations of fixed shapes (1
    metric, 1 term, 2 recipes; 2, 2, 2): each configuration is `valid`
    (asserted), so `new` accepts it (`h_init`, `h_validate`) and returns
    the pool `h_init` describes, which these harnesses build directly
    (`pool_of`) and check against INV with the initial ghost. The plan's
    single harness (raw configuration, real `validate`, the spec's `valid`
    and the invariants in one formula) ran out of 40 GB (deviation 15), and
    `h_init_inv` with the real `validate` and symbolic shapes had no CNF
    after 20 minutes. Plain mode checks everything in one body, with the
    real `validate`.
24. **`h_best`: the search alone, with `candidate` and `energy` stubbed.**
    The search runs one admission of the core per quantity, and the
    harness one of the spec per quantity. With up to 3 units and the plan's
    second call to `submit` on the result, the formula did not reach the
    solver in 20 minutes (9.3M symbolic-execution steps); with up to 2
    units and no second call it had 161.9M clauses, its first SAT call took
    1230 s and it timed out. Under Kani the core's private
    `Pool::candidate` is stubbed (`model::candidate`): each call returns a
    free result, `Ok` with any values of length n in `[-1e6, 1e6]` (the
    real `candidate`'s `Ok` is `add`'s result, such a vector) or `Err`, and
    records it with the call's arguments. Ranking the candidates by the
    spec's energy, or by the core's `energy` again in the harness, left a
    miter of multiplier circuits per quantity: the last SAT call did not
    finish (12 minutes with full-range values; 30 minutes, a timeout, with
    anchored values; 7 minutes with the core's energy on both sides). So
    `Pool::energy` is stubbed too (`model::energy`): a free result per
    call, `Ok` with any `u64` or `Err`, recorded under the quantity of the
    preceding `candidate` call with whether its argument is exactly that
    candidate's `Ok` values. `h_best` shows, for requests of up to 7 units,
    that `best_quantity` calls `candidate` exactly once for each quantity
    in `1..=units` with the request's other fields, the caller and `now`,
    calls `energy` exactly once on each `Ok` candidate's values and never
    otherwise, and returns the quantity whose energy is least among those
    with an `Ok` energy, ties to the larger, or `None`. That the real
    `candidate` is the spec's is `h_preview`'s (`allowed` = `candidate` is
    `Ok`, iff the spec's; the reason) and `h_step_submit_tx`'s and
    `_post`'s (`submit` is `candidate` followed by the version bump, so its
    `Ok` post-values below `u32::MAX` are `candidate`'s `Ok` values); that
    the real `energy` is `Ok` and the spec's on every vector of length n in
    `[-1e6, 1e6]` is `h_energy`'s (NUM-1). At `version = u32::MAX` the `Ok`
    values of `candidate` are not observed by any harness; `candidate`
    reads the version only in the `Stale` guard. The cap of 100 units
    (`min(units, 100)`) is plain mode's (wide units), as are 3 units with
    the real `candidate` and `energy` and the call to `submit` on the
    result.

25. **`h_slot_cycle_*`: each call of the cycle on a rebuilt state** (added
    after the first review, F-3; deviation 20's design, used from the start
    and not measured against the direct script). The submit's outcome is
    symbolic, so after it the record count and the value vector would be
    too, and every later read a case split. `slot_cycle` asserts that the submit's result
    and post-state are the spec's (values, version, the pushed record),
    continues from the accepted outcome only, and runs the cancel on that
    post-state rebuilt with constant shapes (the configuration moved); the
    cancel's post-state is asserted (values and older records as before,
    the new record Cancelled, the version + 2, `paused` and `observed_at`
    unchanged) and rebuilt the same way for the second submit. Plain mode
    runs the same body. That the cycle can be repeated until `Full` is an
    induction this layer shows only in its base and step at 0 and 2 earlier
    records and at the limit (255, 256 records); the 256-cycle trace is
    plain mode's (`f3_slot_exhaustion`).
26. **REF-1..3: what `alpha` maps, and the ghost** (added with REF-1..3;
    choices the plan left open, not cost). `alpha` maps the `Pool` field
    by field to `cfg, values, version, observedAt, paused, records` (names
    and units, which the core only length-checks, are not modelled) and
    leaves out `steps` and `last`: history variables that only `Step`
    writes and no operator or action reads. The model's ghost `base, k0`
    has no `Pool` field; it is the implementation's ghost, the values and
    record count after `new` or the last `Ok` observe, read off the core's
    post-state (under Kani the pre-state's ghost is `gen::inv_state`'s).
    "Noop" (an idempotent `Ok(())`) agrees with the core's `Ok` and must
    stutter, as TraceCheck's `Agrees`. `alpha` is partial: it asserts that
    the pool fits the image, that `records[j].id = j` and that every recipe
    vector spans the metrics (the `.tla`'s "by construction" and
    `ValidCfg`), so a post-state outside its domain fails the harness. Two
    products are written as equal functions that CBMC handles better
    (deviations 9 and 17): `Rcp(r).complete[m] * units` as a 32-bit product
    after asserting `|d| <= 1e6` and `units <= 100` (exact: at most 1e8),
    and each energy term as `(r * r) * w` in `u64` after asserting
    `r <= 2e6` and `w <= 1000` (at most 4e15). Accounts are compared with
    `eq_str` (deviation 5). The configuration is compared in full under
    Kani (deviation 6's restriction was not needed: see the `h_ref` rows).

## Tool issues: a static sharing storage with a constant, references into local arrays (Kani 0.68.0)

*v1.0.0's text (this file at `a072a2d`), kept: it holds for v1.1.0 except where "What changed from v1.0.0" and the v1.1.0 deviations say otherwise; its run letters, times, peaks and counts are v1.0.0's.*


The first complete run of `h_best` with both stubs FAILED: "RD-BEST:
energy of the candidate's values" and memory checks in `Vec::<i64>`
(`push_mut`: pointer invalid; `__rust_dealloc`/`__rust_realloc`: invalid
or double free). The CBMC trace (`cbmc --trace` on the harness's goto
binary, property `Vec::<i64>::push_mut.safety_check.2`) showed a vector
created by the `candidate` stub with capacity 4 and the dangling pointer
`NULL + 8`: `RawVecInner::new_in` had read `RawVec`'s constant `ZERO_CAP`
as 4. In the goto program, `new_in` reads that constant from
`model::CAND_LAST`, the stub's `AtomicUsize` static initialized to 0:
Kani keeps one global per allocation content, and the static and the
8-byte zero constant had the same bytes. The stub's store of the
quantity (4) to `CAND_LAST` therefore changed the constant, and every later
`Vec::new_in` got capacity 4 without a buffer. Not a defect of the core;
a soundness hazard of the tool whenever a harness writes a static whose
initial bytes equal a constant's (a write could equally hide a bug).

The fix: every static the models write is `model::Tagged`, a distinct
64-bit tag in front of the value, so its bytes match no constant; in the
rebuilt goto program `new_in` reads an anonymous constant global. A scan
of every harness's goto program for named statics read outside their own
functions found no other case (only std's read-only tables and the
models' own readers): `VALIDATED` (`h_init`) was read only by `validate`'s
stub and `checks::init`. `h_best` and `h_init` are the only harnesses
that reach these statics, and their results in the table are from the
tagged version (the same scan of the tagged build: every model static is read only by its stub and its harness body).

A second hazard, found with REF-1..3: in a function, a reference to a
field of an element of a local array of structs at an index CBMC does
not fold (`&t.recipes[r].reserve` in `tla.rs`, with `r` read from
`t.records`), passed to another function, read unconstrained values
there. The first `h_ref_*` run (on the sources before the fix) FAILED in
`h_ref_submit`, `h_ref_cancel` and `h_ref_settle` (REF-1, REF-2, `mul`'s
precondition, "attempt to negate with overflow" in `neg`); `h_ref_dispatch`,
`_ack`, `_pause` and `_observe`, which pass no such reference, were `ok`.
The CBMC trace of `h_ref_cancel` (`cbmc --trace` on its goto binary,
property `tla::mul.assertion.2`) showed `alpha` writing
`recipes[1].reserve[0] = 0`, then `release_err`'s `neg(&rec_rcp(i).reserve)`
reading 0 and `next`'s identical call reading -4611686018427387905. A
concrete probe harness (1 metric, 2 recipes, a Reserved record of recipe
1; plain mode passes it) failed the same way under Kani in 26 s; in it,
`neg(&t.rcp(1).reserve)` (a constant index), a read in place
(`t.rec_rcp(0).reserve[0]`) and `neg` of a local copy passed, while
`neg(&t.rec_rcp(0).reserve)`, `neg(&x.reserve)` with `x = t.rec_rcp(0)`,
and `neg(&t.rcp(r).reserve)` with `r = t.rec(0).recipe` failed. The values
were unconstrained (the solver chose a different one at each read), so in
`tla.rs`, where no assumption follows such a read, the hazard could only
make a harness fail. The fix: `rcp`, `rec` and `rec_rcp` return copies
(the image's records are `Copy`), so every reference passed on is to a
local; the probe then passed (9 s) and so did every `h_ref_*` harness
(table). `spec.rs` and the harness bodies do not pass such references
(their lookups are concrete-slot reads, deviation 3), and the core's
vectors are heap objects, not local arrays of structs. The probe was not
kept (`proofs.rs` has no `probe_*` harness).

A third, harmless tool limitation: with CBMC's `--stop-on-fail` (the
mutant runs) Kani 0.68.0's driver panics while parsing CBMC's JSON
("data did not match any variant of untagged enum ParserItem"), after
CBMC has found the violation. `run.sh mutants --kani` therefore runs the
mutants with `--output-format old` (CBMC's own text) and reads the
violated property from it (`mutant_verdict`: a violated property that is
not an unwinding assertion, an unsupported construct or a Kani-internal
check kills the mutant).

## Cost ladder

*v1.0.0's text (this file at `a072a2d`), kept: it holds for v1.1.0 except where "What changed from v1.0.0" and the v1.1.0 deviations say otherwise; its run letters, times, peaks and counts are v1.0.0's.*


Measured on the submit step unless noted (CNF sizes from `--dimacs`,
times wall clock). "m/t/r/c" = metrics/terms/recipes/records.

| Step | Instance | Measurement | Consequence |
|---|---|---|---|
| 1 | `h_energy`, 2 metrics, 2 terms, `Full` values, via `Pool::new` | 52.5M clauses, out of 24 GB after 73 s; `Pool::new`'s validation 13.8M; one 64-bit multiplier 104k | `pool_of` (deviation 8) |
| 2 | Energy exactness against an `i128` product (miter), or an uninterpreted multiplier | did not finish; UF 101.9M vs 47.3M clauses | same-circuit product (deviation 9), anchored values for exactness |
| 3 | submit step, symbolic record count ≤ 3 | out of 26 GB | fixed record counts (deviation 7) |
| 4 | submit step 1/1/1/0 | 28.3M clauses, of which `memcmp` on `str` 12.5M; 290 s | bounded string equality (deviation 5) |
| 5 | submit step, fixed records 0/1/2/3 (1/1/1/c) | 19.4M / 22.3M / 24.7M / 27.1M; symbolic ≤ 3: 56.6M | |
| 6 | submit step 1/1/2/1, 2/2/2/0 | 49.1M, 53.7M | |
| 7 | submit core alone vs spec alone (1/1/1/1, 1/1/2/1) | core 10.1M / 12.4M, spec 1.9M / 4.4M | the cost is the core's heap, not the spec |
| 8 | after `same_state` without the configuration (deviation 6) | 1/1/1/1 15.6M; 1/1/2/1 25.7M (solved in 417 s); 2/2/2/0 29.1M; 1/1/2/2 29.8M; 2/2/2/2 40.7M | step shape 1/1/2/2 |
| 9 | hot spots in 1/1/2/1 (`--write-solver-stats-to`) | `prereqs_met` 3.4M, `add`'s closure 2.7M, `i64` clone 1.9M, `(u32, u32)` clone 1.0M | per-loop unwinding (deviation 11) |
| 10 | first full step run (32 GB cap) | submit tx/post/inv/cov 520/436/991/224 s (inv 72.7M clauses, 33.4 GB); cancel_tx 51.1M, 272 s; settle_tx 57.9M, 483 s; cancel post/inv and settle post/inv/cov out of 32 GB | probes below |
| 11 | cancel_tx with `--max-field-sensitivity-array-size 1024` | 47.2M vs 51.1M | not adopted |
| 12 | probes 1/1/2/2: INV state alone; core cancel; spec cancel; core settle(completed); core submit | 3.7M; 45.4M; 4.8M; 9.8M; 15.6M | the core's cancel is the outlier |
| 13 | core cancel with a fixed record id; then with the `Vec::reserve` model | 45.3M → 8.4M (std's growth path in `release`'s `undo` collect) | `Vec::reserve` model (deviation 1); cancel_tx 51.1M → 14.1M |
| 14 | PAUSE-1 by calling the core a second time | doubled `_post` (cancel, settle out of 32 GB) | spec-level PAUSE-1 (deviation 18) |
| 15 | `h_energy_max` (64 terms, `pool_of`, no capacity model) at unwind 66 | killed after 744 s and 526 s in symbolic execution (21.6 GB): the loop inside each `next()` of `energy_terms`' fallible `collect` unrolled 66 times inside the 66 of the `collect` | that loop bound 2 (deviation 11) |
| 16 | `h_version_max`, all seven kinds in one formula | 73.1M clauses, 34.7 GB, timed out at 1802 s | one harness per kind: 26 to 357 s (deviation 22) |
| 17 | `h_preview` with `eq_text::<24>` at the harness bound 6 | unwinding assertion failed (28.2M clauses, 567 s) | bound 25 for that loop |
| 18 | `h_preview` with the bound 25, `reason` compared three times on the heap | 62.2M clauses, 32.9 GB, solved in 859 s (1634 s in all) | `reason` copied once into `Text<16>` (deviation 5) |
| 19 | `h_preview_post`: the spec formats its numbers with `to_string` (a 64-bit division per digit pair) and compares 24 bytes | no CNF after 20 min of propositional encoding (22 GB); stopped at 1193 s | `is_dec`: parse the core's numeral (multiplications by 10) |
| 20 | `h_best` at the harness bound 6 | stopped after 808 s, 23 GB still in symbolic execution: `best_quantity`'s two loops over `1..=units` unrolled 5 times each | those loops bound 4 (units ≤ 3) |
| 20b | `h_best`, units ≤ 3 (loops at 4), with the second call to `submit` | 9.3M symbolic-execution steps (468 s), no CNF after 20 min; stopped at 1188 s | units ≤ 2 (loops at 3), soundness by decomposition (deviation 24) |
| 20c | `h_best`, units ≤ 2 (loops at 3), no second call to `submit` | 6.43M steps, 161.9M clauses, 36 GB; the first SAT call took 1230 s; timed out at 1801 s | `candidate` stubbed by a free result: the search alone, up to 7 units (deviation 24) |
| 20d | `h_best` with `candidate` stubbed, `Ok` values anywhere in `[-1e6, 1e6]`, units ≤ 7 | 1.09M steps, 12.5M clauses; four SAT calls in 45 s, the last (the core's energies against the spec's, a miter of two multipliers per quantity) not done after 12 min; stopped at 749 s | the stub's values from the step domain `Anchored(4)` |
| 20e | `h_best`, `candidate` stubbed with values from `Anchored(4)`, ranked by the spec's energy | 1.10M steps, 12.5M clauses; three SAT calls in 71 s, the fourth (the spec's energies against the core's) not done; timed out at 1800 s | the harness ranks by the core's `energy` |
| 20f | `h_best`, ranked by the core's `energy` (two copies of the core's circuit per quantity) | 1.30M steps, 15.2M clauses; three SAT calls in 91 s, the fourth not done after 7 min; stopped | `energy` stubbed too: a free result per call, `h_energy` pins it down (deviation 24) |
| 20g | `h_best` with `candidate` and `energy` stubbed | 0.70M steps, 7.7M clauses, 272 s: FAILED on memory checks in `Vec::<i64>` and "RD-BEST: energy of the candidate's values"; the trace shows the tool issue (a static sharing storage with `RawVec`'s `ZERO_CAP`) | tagged statics ("Tool issues"); then 0.52M steps, 4.3M clauses, ok in 137 s |
| 21 | `h_wedge`, symbolic roles, `Pool::new` | 3.66M symbolic-execution steps, out of 40 GB at 1558 s | fixed roles, `pool_of` (deviation 19) |
| 22 | `h_wedge_healthy` with the spec's step and the invariants along the script; then the script on the core alone; then one concrete configuration with the spec's checks | stopped at 196 s in symbolic execution; 3.73M steps, out of 40 GB at 216 s; 5.19M steps, 20,347 VCCs left, out of 40 GB at 1465 s | from a healthy INV state (deviation 19) |
| 23 | `h_bmc_from_init` from `Pool::new`, INV of the initial state, any first kind | still in symbolic execution after 16 min (stopped at 988 s) | `pool_of`, first operation submit, pause or observe (deviation 20) |
| 23b | `h_bmc_from_init`, first kind restricted, second any kind | 6.34M steps (641 s of symbolic execution), out of 40 GB at 685 s | one harness per second kind (deviation 20) |
| 23c | `h_bmc_from_init_pause` (one harness per second kind), with a copy of the pre-state at each step; then without the copy | 2.79M steps, 37 GB, timed out at 1803 s with no verdict; 2.41M steps (stopped there) | the first operation's post-state asserted equal to the spec's and rebuilt with constant shapes, a submit continued from `Ok` only (deviation 20) |
| 24 | `h_full`, symbolic count 255 or 256 | 63 of 256 pushes in 9 min of symbolic execution (stopped at 586 s) | concrete counts (deviation 21) |
| 25 | `h_init` with 129-byte strings; then short strings, 3 metrics, real `validate` and INV in one formula | not through symbolic execution in 30 min; then 1.66M steps, out of 40 GB at 190 s (39.1 GB) | free `validate` and `h_init_inv` (deviation 23), 2 metrics (deviation 15) |
| 25b | `h_init_inv`: symbolic shapes (up to 2/2/2), the real `validate` via `Pool::new`, INV | 2.12M steps; no CNF after 20 min of propositional reduction (stopped at 1450 s) | fixed shapes (`h_init_inv`, `h_init_inv_2`), `pool_of` and the spec's `valid` (deviation 23) |
| 25c | `h_init` (free `validate`) at its unwind 4 | `spec::eq_i64s`'s loop over 4 elements failed its unwinding assertion (33.9M clauses, 258 s) | that loop bound 5 (deviation 11); then 54.2M clauses, ok in 576 s |
| 26 | `h_validate` with 3 metrics, 128/129-byte strings, no capacity model | 1.09M steps, out of 40 GB at 817 s (37.5 GB) | 2 metrics, short strings, capacity model (deviation 15) |
| 27 | `h_step_cancel_post`: PAUSE-1 compares `[i64; 2]` with `==` | `memcmp` loop of 16 bytes, unwinding assertion failed at bound 6 | `eq_vals` (deviation 5) |
| 28 | `h_full_256` with `assume(inv_dep)` over the 256 records | still in symbolic execution after 9.5 min (767 `completed_units` loops over up to 256 records); stopped at 582 s | INV-DEP by construction, not evaluated under Kani |
| 28b | `h_full_255` with a copy of the pool for the spec and TX-0 | the copy of 255 records took most of 13 min of symbolic execution; stopped at 816 s | the spec's outcome computed before the call, the post-state compared with it and with the known records (no copy) |
| 28c | `h_full_256` with the configuration copied for the exact post-state, harness unwind 258 | `gen::copy_vec`'s loops over the recipes' deltas unrolled about 253 times each (1,532 unwindings in 8 min of symbolic execution); stopped | those loops bound 4 in `h_full_*` and `h_preview_post` (deviation 11) |
| 28d | `h_full_256` with the copy's loops at 4 | the slice comparisons of `same_config` unrolled 257 times each; 5.98M steps (381 s), timed out at 1801 s in the first SAT call | the configuration not compared under Kani (deviation 6) |
| 28e | `h_full_255` at the harness unwind 258 | `spec::prereqs_met`'s loop over a recipe's requirements unrolled toward 258, each iteration a scan of the 255 records (about 4,000 loop iterations in 16 min of symbolic execution); stopped at 991 s | that loop bound 4 (deviation 11) |
| 28f | `h_full_255` with that loop at 4 (CBMC by hand on the goto binary) | no read of a record folded to its constant fields (the buffer, 257 × 24 bytes, is above CBMC's field-sensitivity limit of 64 elements): both envelopes took the `open` branch, with a multiplication, for every record (`spec::rcp` reached 514 times); still in the core's envelope after 13 min; stopped | `--max-field-sensitivity-array-size 8192` for `h_full_255` (deviation 21) |
| 28g | `h_full_255` with the field-sensitivity limit 8192 (by hand) | the 257 pushes took 8 min (field-sensitive writes, the machine shared with a CI virtual machine using 5 cores), then every scan folded (`spec::rcp` reached 4 times) until INV-WF's scan of the post-state, after the conditional push, ran at about 4 records a minute; stopped | INV-WF of the post-state not evaluated under Kani (deviation 21) |
| 28h | `h_full_255` (`run.sh`) with the loop at 4, the field-sensitivity limit and no INV-WF scan, step domain | 8.05M steps, 1001 s of symbolic execution, 113 s converting SSA, the first SAT call (the admission arithmetic of the step harnesses' submit, result and post-state in one formula) not done; timed out at 1801 s (28.8 GB) | values, bounds and targets 0, deltas in `-1..=1` |
| 28i | `h_full_255` as 28h with values, bounds and targets 0 and deltas in `-1..=1` | 8.05M steps, 962 s of symbolic execution, 93 s converting SSA, the first SAT call again not done; timed out at 1802 s (31.1 GB) | not the arithmetic: the harness back on the steps' domain; the records built without pushes (28j) |
| 28j | `h_full_255` with the records one constant array (`OLD_RECORDS`, `Vec::from`, then `truncate`), the field-sensitivity limit, INV-WF of the post-state not scanned, the steps' domain (as 28h) | 2.42M steps (8.05M in 28h), 493 s of symbolic execution, 14 s converting SSA, 37.9M clauses, two SAT calls of 147 s and 93 s; ok in 764 s (19.9 GB); `h_full_256` ok in 46 s | adopted (deviation 21); mutant 30 (`Full` at 255) added for the boundary it now shows |
| 29 | `h_init`, `h_validate`: raw configurations built by conditional pushes | 0.56M and 0.97M steps, out of 40 GB in the propositional encoding after 86 s and 145 s | concrete-offset writes, then truncate (deviation 3) |
| 30 | `h_energy_max` (64 terms, concrete) | 226.3M clauses, 27.2 GB, 447 s | |
| 31 | the `_inv` parts from 0, 1, 2 and 3 records (submit; settle) | submit 53.4M / 57.7M / 62.3M / 65.6M clauses, one SAT call of 297 / 438 / 772 / 1072 s; settle 8.5M / 25.8M / 33.1M / 40.8M, 17 / 165 / 378 / 1712 s (`h_step_settle_inv_3` ok in 1788 s, 12 s inside the limit of 1800 s) | `h_step_*_inv_3` get a 7200 s limit in `run.sh` |
| 32 | `h_ref_*` (REF-1..3) as first designed: the step state, the image `TK` with the configuration compared in full, `mul` in 32 bits and energy as one `(r * r) * w` product (deviation 26) | 0.34M to 1.00M steps, 10.7M (pause) to 23.0M (submit) clauses, largest SAT call 189 s (submit), 5.2 to 12.4 GB, 55 to 351 s per harness | kept; no reduction needed (deviation 6's restriction was not) |

## What is not shown

**v1.1.0.** As below, with v1.1.0's bounds ("Bounds"); in addition: a prune under Kani only in `h_full_256_prune` (from 256 constant records), the ghost fold of a prune and REF with prunes only in plain mode; the large mutants declared in `kani/local/not-run.tsv` have no Kani leg at v1.1.0 (their plain-mode kill runs in CI), and a hosted-only harness's numbers are CI's, not this file's.

*v1.0.0's text (this file at `a072a2d`), kept: it holds for v1.1.0 except where "What changed from v1.0.0" and the v1.1.0 deviations say otherwise; its run letters, times, peaks and counts are v1.0.0's.*


Every Kani result is a bounded proof: it holds for every input inside the
bounds above and says nothing outside them. In particular:

- **Shapes.** The step, liveness, preview and version harnesses use 1
  metric, 1 term, 2 recipes and exactly 2 records (3 in `h_live1`; 0, 1
  and 3 in the `h_step_<op>_inv_<k>` parts, which check INV preservation
  only); 2 metrics and 2 terms appear only in `h_energy*`, `h_init*`,
  `h_validate` and `h_init_inv_2`, and the 64-term worst case only in
  `h_energy_max`. The exact result and post-state (`_tx`, `_post`) from
  0, 1 or 3 records (other than the states `h_bmc_from_init_*` and
  `h_slot_cycle_0` reach), any step from 4 or more records (other than a
  submit at 255 or 256 Cancelled records), steps over 2 metrics or 2
  terms, and anything larger than the brief's small scope are plain
  mode's (random, not proof) and the other layers' (TLA+, Lean at every
  size for the properties it states).
- **Values.** The steps use anchored values (`-1e6`, `0`, `1e6` plus or
  minus 4) and deltas in `-3..=3`; only `h_energy` covers the full value
  range. A bug that needs a value strictly between the anchors and the
  extremes (other than through energy) is not excluded.
- **Depth.** `h_bmc_from_init_*` runs 2 operations from `Pool::new`
  and `h_slot_cycle_0` 3 (submit, cancel, submit); longer traces rest on
  the induction (`h_init_inv*` + `h_step_*_inv` + `h_step_*_inv_<k>`),
  which shows INV-WF to INV-DEP in every reachable state with at most 3
  records (step shape and domains) and nothing about states with 4 or
  more. The induction is over INV only: the per-step results (`_tx`,
  `_post`) are shown from 2-record INV states, and INV-VER only along
  the 2-operation traces. The depth-3 and depth-4 covers ("a record
  completes", "observe after a completion") are plain mode's.
- **The record limit with other records.** `h_full_255` and
  `h_full_256` hold 255 or 256 Cancelled records of recipe 0 (deviation
  21): the submit there is exactly the spec's, so the limit is exactly
  256 (mutants 21 and 30 move it by one either way). A limit state with
  open or Completed records (prerequisites met by old records, an
  envelope over 255 reservations) is plain mode's (`h_full` draws only
  Cancelled records too) and the other layers'.
- **Reachability of F-2, F-2b, F-3 and F-5 from `Pool::new`.**
  `h_wedge_healthy` covers the wedge from every healthy INV state of its
  shape whose records are a Dispatched position and a Completed hedge;
  that such a state is reachable from `Pool::new` is shown by plain mode
  (`f2_small_instance`, `f2_risk_fixture`) and by the TLA+ and sandbox
  layers, not by Kani. F-2b's covers are from 2-record INV states. F-3:
  Kani shows the cycle from `Pool::new` and from quiet 2-record states,
  and the limit at 255 and 256 Cancelled records; the 256 cycles as one
  trace (the cycle from quiet states with 3 to 255 records) are plain
  mode's (`f3_slot_exhaustion`). F-5: Kani shows every effective kind
  frozen from states at `u32::MAX`; reaching `u32::MAX` takes 2^32 - 1
  effective transitions (INV-VER), not run by Kani.
- **RD-BEST** is a composition (deviation 24): the search with a free
  `candidate` and a free `energy` (units ≤ 7), plus `candidate` = spec
  from `h_preview` and `h_step_submit_*` and `energy` = spec from
  `h_energy`. The `min(units, 100)` cap, the real `candidate` and `energy`
  with 3 units and the call to `submit` on the result are plain mode's; at
  `version = u32::MAX` the equality of `candidate`'s `Ok` values with the
  spec's rests on reading the code.
- **CFG-1** is a composition (deviation 23): `h_init` (free `validate`),
  `h_validate` (2 metrics, strings of 0 or 2 bytes) and `h_init_inv*` (two
  fixed shapes). 3-metric raw configurations and the 128/129-byte boundary
  of `short` are plain mode's.
- **LIVE-2** (eventual termination under executor fairness) is not a Kani
  property; it is the TLA+ layer's. LIVE-1 is shown for 3 records with
  B = 0; Lean shows it at every size.
- **REF-1..3** (the real core refines the TLA+ model per step) are
  bounded proofs within the step bounds only: pre-states of 1 metric, 1
  term, 2 recipes and exactly 2 records, the step domains. Other record
  counts and shapes are plain mode's (the step tests up to 2 metrics, 2
  terms, 2 recipes and 3 records, `h_bmc_from_init` to depth 5) and the
  212 conformance traces' (up to 8 metrics and 256 records): tests, not
  proofs. The link is to `tla.rs`, a hand transcription of
  `SharedPool.tla`, trusted to be the `.tla` (part of ASM-LEAN-TLA). It is
  checked against the TLA+ model only indirectly: on the 8,804 trace
  calls `tla.rs` (`h_ref_traces`) and TLC's TraceCheck (REF-5) both agree
  with the core's recorded results and states, so with each other there.
  Mutant 31 (a transcription error in `ReleaseErr`) shows the check
  depends on the transcription. `steps` and `last` are not mapped
  (deviation 26); the TLA+ initial state (`Init`) is linked by CFG-1 and
  the traces, not by a REF harness.
- **Memory safety of std** is not checked (`--no-memory-safety-checks`;
  the core forbids `unsafe`). Panics, overflow, bounds and unwinding
  assertions are checked.
- **The shell, the executor and NEAR** (callbacks, gas, promises,
  predecessor) are out of scope here: near-tests and sandbox.
- **The models** (`model.rs`) are trusted to be what deviation 1, 23 and
  24 say: the capacity models can only make a harness fail; the free
  `validate`, `candidate` and `energy` are pinned down by other harnesses.
- **The tool.** Kani 0.68.0 and CBMC 6.11.0 are trusted (ASM-TOOLS), with
  the two hazards found and worked around ("Tool issues"): the scan for
  statics sharing storage with constants covers the named statics of
  every harness's goto program, not every allocation Kani could merge.
- **The specification** (`spec.rs`) is trusted to mean the brief
  (ASM-SPEC); it was written from the brief, not from the code, and the
  plain-mode mutants check that it distinguishes the code from 29 changed
  versions of it.
