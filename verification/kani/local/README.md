# The local Kani record

At v1.0.0, 36 of the 89 Kani harnesses and 16 of the 31 mutants peaked
above 13 GB when measured (`../RESULTS.md`; `h_wedge` reached 39.0 GB).
GitHub's hosted runner for a public repository has 16 GB, and on swap
those harnesses failed or crawled (CI runs 43469880167 and 43488691444;
strategy.md section 14). So they run on a local machine with enough
memory, and this directory records the run. Without the `KANI_RUNNER`
repository variable, CI checks the record instead of running them again.
v1.1.0's split comes from `run-local.py --all`'s measurement of every
harness and mutant (below).

The split is in `.github/workflows/shared-pool.yml`:

| Hosted runner | Local record (or `KANI_RUNNER`) |
|---|---|
| `kani` (when a change reaches the Kani layer): the harnesses that peaked at 13 GB or less, in groups | `kani-large`: the rest, always with `h_wedge` |
| `kani-mutants` (manual runs): the mutants that peaked at 13 GB or less | `kani-mutants-large`: the rest |

## Files

| File | Written by | Contents |
|---|---|---|
| `provenance.txt` | `record.py` | The runs' provenance as `key<TAB>value` lines: `commit` (the commit run), `kani_tree` (its `projects/shared-pool/kani` tree), one `input<TAB>name<TAB>value` line per Kani input, `fingerprint`, `digest` (sha256 of `jobs.tsv`, `covers.tsv` and `not-run.tsv` if there is one), `kani`, `cbmc`, `rustc`, `host`, `started`, `finished`, `parallel` (lanes and memory budget), `kani_max_gb`, `kani_env` (the run's other `KANI_*` variables), `cargo_config` (the cargo configuration files outside the lanes that `run-local.py` found), `runner`, `jobs` (counts), `assembled` (from which runs, and that each job's result is the latest that passed); then per run `runN` (its directory), `runN.status` (complete, or stopped with how many jobs finished), `runN.finished`, every `env.txt` fact as `runN.<key>` (lanes, budget, mode, fingerprint, ...) and `runN.recorded` (the jobs taken from it); `superseded.<job>` for a job that another run also finished (an earlier result the later run's replaced, or a later one that did not pass), `not_run.<mutant>` for a declared one. |
| `jobs.tsv` | `record.py` | One row per job: `kind` (harness or mutant), `name` (a mutant's full patch name), `result` (`ok`, `ok-FAILED` for `h_canary`; a mutant's expect/plain/kani, such as `killed/killed/killed`), `seconds` (`run.sh`'s time for a harness; the whole job for a mutant), `job_seconds`, `cbmc_peak_gb` (CBMC's largest resident memory, sampled every 3 s), `tree_peak_gb` (the whole `run.sh` process tree), `limit_s` (the `KANI_TIMEOUT` the job ran under: its matrix timeout in a `--record` run, `run-local.py`'s limit in a measurement; the matrix timeout may not be lower), `covers` (a harness's "N of M"), `kani_harnesses` (the harnesses a mutant ran), `started_utc`. Harnesses come first, in `run.sh list` order, then mutants by name. |
| `covers.tsv` | `record.py` | The recorded harnesses' cover results: `family`, `cover`, `status` (`run.sh`'s `covers.tsv`, run with `KANI_DEFER_COVERS=1`) plus the `harness`. The `covers` job checks them with the hosted groups' covers. |
| `not-run.tsv` | by hand, before `record.py` | Optional: `kani-mutants-large` mutants that no run finished, one `mutant<TAB>reason` line each under that header (the mutant's full patch name; a harness cannot be declared). They are recorded as not run: the record has no Kani result for them, and CI's `mutants` job still checks their plain mode. `record.py` refuses one that a run finished, and an undeclared large mutant that no run finished; the digest covers the file; the check prints a warning for each, every time, and the RESULTS.md table lists them. |
| `run-local.py` | (source) | Runs the jobs on this machine and, with `--record`, records them; with `--all`, measures every harness and mutant. |
| `record.py` | (source) | Assembles the record of the large jobs from one or more runs (measurements or a record's run) and writes the three files above, or prints the record's Markdown (`table`). |
| `results-tables.py` | (source) | Writes `../RESULTS.md`'s measured tables (the summary, "Kani results", "Mutants" and "Covers" blocks) from run directories: every harness and mutant, each job from the latest run that passed it, else W7's numbers, else the hosted CI job's or not run, with an estimate. It does not touch the local-record block. |

`kani/RESULTS.md` holds the record's tables between
`<!-- kani-local-record: begin ... -->` and `<!-- kani-local-record: end -->`.
`record.py` rewrites that block. The CI check fails if the block is
missing or is not `record.py table`. The measured tables of every job
are between `<!-- kani-results-<name>: begin ... -->` and
`<!-- kani-results-<name>: end -->` (`summary`, `harnesses`, `mutants`,
`covers`), and `results-tables.py` rewrites those; CI does not check
them.

## The fingerprint

A Kani verdict depends on these inputs, and the record is valid only while
they are unchanged:

- every file under `projects/shared-pool/kani/` except this directory
  (`kani/local/`) and the Markdown (`*.md`, such as `RESULTS.md`). This
  is a default-include: a file added there, such as
  `kani/.cargo/config.toml`, `kani/rust-toolchain.toml` or
  `kani/build.rs`, changes the fingerprint. `kani/src`,
  `kani/Cargo.toml`, `kani/Cargo.lock`, `kani/mutants`, `kani/run.sh` and
  `kani/.gitignore` also have lines of their own, which name what
  changed.
- `projects/shared-pool/source-mirror`: the gitlink, which is the pinned
  core's commit
- `projects/shared-pool/conformance/traces/core.txt`: the traces a
  mutant's plain mode replays (`src/conform.rs`)
- the repository's cargo configuration above `kani/`:
  `.cargo/config` and `.cargo/config.toml` at the root, in `projects/`
  and in `projects/shared-pool/`. `kani/` is a standalone workspace, but
  cargo reads such a file in every directory above the crate, so one
  that sets `rustflags` or a profile changes the groups' builds. The
  record's commit has none (the line is `none`). `run-local.py` copies
  those the commit has into each lane, so a run sees them as CI does.
- `tools/versions.env`: `KANI` and `CBMC`. The file's other pins are for
  other layers' tools, and `RUST_TOOLCHAIN`, the toolchain of a mutant's
  plain mode, is left out: the record does not cover the plain half of a
  mutant's verdict, which CI's `mutants` job recomputes on every run.

A named path's value is its git object id at the commit (`git rev-parse
REV:path`): a tree id for a directory, a blob id for a file, a commit id
for the gitlink. The line `projects/shared-pool/kani/(all but local, *.md)`
is the sha256 of `git ls-tree -r REV:projects/shared-pool/kani` (each
file's mode, type, blob id and path), less `local/` and `*.md`. The cargo
configuration's line is `none`, or the sha256 of those files' `git
ls-tree` entries when the commit has any. The two versions are their
values. The fingerprint is the sha256 of the
`name<TAB>value\n` lines in that order. Git objects are all it reads, so
CI computes it in a shallow checkout without the submodule:

    tools/scripts/check-kani-record.py fingerprint [REV]
    tools/scripts/check-kani-record.py pathspecs    # the same inputs, for git diff

These do not change the fingerprint: the Markdown, this directory, the
workflow, and the rest of the repository. The check compares the
workflow's matrices with the record instead (see CI below): the large
jobs must be the recorded ones, and their timeouts may not drop below
the `limit_s` the record ran them under. Neither do the run's environment and
the host's cargo configuration outside the repository (`CARGO_HOME`'s,
or a `.cargo/` above the checkout), so `run-local.py` refuses those that
can change a verdict (below).

## Commands

    # Run the kani-large harnesses and kani-mutants-large mutants at HEAD,
    # then record them (v1.0.0's 36 and 16: about 4 hours on a 10-core,
    # 64 GB Mac; two jobs at a time, their measured peaks summing to at
    # most 50 GB):
    projects/shared-pool/kani/local/run-local.py --record
    git add projects/shared-pool/kani/local projects/shared-pool/kani/RESULTS.md

    # The jobs, without running them (after the environment and cargo
    # configuration checks below, as a run makes them):
    projects/shared-pool/kani/local/run-local.py --dry-run

    # Record a run directory that has already finished:
    projects/shared-pool/kani/local/record.py projects/shared-pool/kani/target/local-run/run-<time>

    # Or assemble the record from measurements (run-local.py --all, also
    # stopped early) at one commit and fingerprint: the jobs of the large
    # matrices, each from the latest run that finished and passed it (a
    # later failure does not replace a pass), large mutants that none
    # finished declared in not-run.tsv;
    # --dry-run checks without writing, and lists what is missing:
    projects/shared-pool/kani/local/record.py --dry-run ~/shared-pool-kani-measure/<run1> ~/shared-pool-kani-measure/<run2>
    projects/shared-pool/kani/local/record.py ~/shared-pool-kani-measure/<run1> ~/shared-pool-kani-measure/<run2>

    # The record's Markdown (RESULTS.md's block):
    projects/shared-pool/kani/local/record.py table

    # RESULTS.md's measured tables from the runs, each named as in the
    # tables' Run column (later runs win for a job two passed; a failed
    # result never replaces a pass). Without --write it prints them;
    # --final once the runs are over (a job no run measured and no hosted
    # matrix holds is "not run", not "pending", and the Verdict line is
    # final); --covers LABEL=FILE adds a run.sh covers.tsv to the cover
    # table (v1.1.0's add W7's four: "W7 ref", "W7 P3", "W7 P4" and
    # "W7 Xval"); it reads git, run.sh list and the matrices only, in
    # about 2 s:
    projects/shared-pool/kani/local/results-tables.py --write [--final] [--covers LABEL=FILE ...] "run 1=DIR" "run 3=DIR"

    # What CI runs:
    tools/scripts/check-kani-record.py
    tools/scripts/test-check-kani-record.sh

`run-local.py` needs Python 3.11 or later (`tomllib`, for the cargo
configuration check), Kani and CBMC at `tools/versions.env`'s versions
(`cargo install --locked kani-verifier --version <KANI>; cargo kani
setup`), the Rust toolchain `RUST_TOOLCHAIN` (`rustup toolchain install
<RUST_TOOLCHAIN> --profile minimal`) and the submodule (`git submodule
update --init projects/shared-pool/source-mirror`). It runs `kani/run.sh`
unchanged in a `git archive` copy of the commit's inputs (with the
repository's cargo configuration above `kani/`, if it has any), one copy
per lane, with each job's matrix timeout as `KANI_TIMEOUT`,
`KANI_DEFER_COVERS=1`, and `KEEP_LOGS` set to the job's log directory. The
mutant jobs also get `RUSTUP_TOOLCHAIN=<RUST_TOOLCHAIN>` for their plain
mode, as CI's `kani-mutants` jobs do. It writes to `kani/target/local-run/`
(git-ignored). If it is stopped, its jobs are stopped too, and nothing is
recorded.

It refuses to start when the environment sets a variable that changes what
`run.sh` runs or how cargo builds: `KANI`, `KANI_FLAGS`, `KANI_MOD`,
`RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTUP_TOOLCHAIN`, `RUSTC`,
`RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_BUILD_*`,
`CARGO_PROFILE_*` or `CARGO_TARGET_*_RUSTFLAGS`. It also refuses when a
cargo configuration file outside the lanes (`CARGO_HOME`'s, or a
`.cargo/config[.toml]` above the run directory or `TMPDIR`) sets
`rustflags`, `rustc` or a wrapper (at any depth, such as
`target.<triple>.rustflags`), a profile, or an `include`. It parses each
file as TOML, as cargo does, so every spelling of a key counts (`[build]`
then `"rustflags" = ...`, `build.rustflags = ...`, `build = { rustflags =
... }`), and the error names the keys it found; a file that does not
parse is refused too. With the default run
directory, inside the repository, that includes the work tree's own,
such as `kani/.cargo/config.toml`: cargo would read it on top of the
lane's copy of the commit's, so a `rustflags` there would apply twice.
For such a file, pass `--out` outside the repository. `env.txt` records
the other `KANI_*` variables (`kani-env`) and those files
(`cargo-config`).

`record.py RUN_DIR...` records the jobs of the workflow's `kani-large`
and `kani-mutants-large` matrices from one or more runs of `run-local.py`
at one commit: `--record` runs, or measurements (`--all`), complete or
stopped early. Each job comes from the latest run (by `env.txt`'s
`started`) that finished it (a row in `results.tsv`; a job a stopped run
was still running has none) and passed it (exit 0, and `ok` or as its
patch expects): a failed result never replaces a passed one, as in
`results-tables.py`'s tables. `provenance.txt` names every run (`runN`,
numbered by start time among the runs given, so v1.1.0's runs 1, 3 and 4
of `../RESULTS.md` are its `run1`, `run2` and `run3`),
which jobs each gave (`runN.recorded`) and every result not taken
(`superseded.<job>`: an earlier result a later run's replaced, or a later
one that did not pass). A job's `limit_s` is the `KANI_TIMEOUT` it ran
under in that run: the start line in `progress.log`, which must agree
with `jobs.json` and, for a measurement, `estimates.tsv`; without a start
line, `jobs.json`'s timeout, which `run-local.py` runs it under. It can be
above its matrix timeout after `--estimates-from` raised it; the matrix
timeout must then be raised to it. Other jobs of a measurement are not
recorded (CI's hosted jobs run them). A large mutant that no run
finished can be declared in `not-run.tsv`; a large harness cannot. It
refuses, leaving the record and RESULTS.md untouched, when any of these
holds:

- a large job is in no run (a harness), or in no run and not declared in `not-run.tsv` (a mutant)
- a mutant declared in `not-run.tsv` is in a run, or is not a `kani-mutants-large` mutant (a harness, say)
- no run that finished a large job passed it: exit 0, and a harness `ok` (`h_canary`: `ok-FAILED`), a mutant as its patch expects (`killed/killed/killed`, or `survive/survived/survived` for `# must-survive`); each run's result is listed
- the result taken disagrees with its logs: a harness's result line in `stdout.txt`, or no `covers.tsv` (`KANI_DEFER_COVERS=1`); a mutant's line in `stdout.txt`, or no log of its Kani leg (`<mutant>.<harness>.log`) in its log directory
- a recorded job's limit in `progress.log` is not its `jobs.json` timeout (or a measurement's `estimates.tsv` limit)
- a run directory is given twice, the runs are at different commits or fingerprints, or a run's commit, fingerprint, Kani tree, core pin or Kani/CBMC versions do not match
- the runs' commit has other Kani inputs than HEAD
- `kani/RESULTS.md` has no local-record block
- the new record would fail `check-kani-record.py` (so a large matrix timeout must be at least the limit its job ran under)

To re-record after any change to an input, run the first command again
at the new commit (or assemble the record from measurements at it) and
commit the result.

## Measuring every harness and mutant (`--all`)

The matrices' 13 GB split and timeouts come from a measurement of every
harness and mutant, made before a record when the Kani layer changes
(as for v1.1.0). `run-local.py --all` makes it with the record's lanes,
`git archive` copies, environment and cargo checks, sampling and
`results.tsv` format, but:

- Its jobs are every harness of the commit's `run.sh list` and every
  `mutants/*.patch` (`mutants --kani 'NN-*'`), not the matrices'.
  Harnesses run first, so that a mutant's estimate can come from its
  harness measured in the same run.
- Each job's peak and time are estimated from the best source, named in
  `jobs.json` and `estimates.tsv` (and by `--dry-run`): W7's
  measurements of the v1.1.0 layer (`V11_MEASURED` in `run-local.py`);
  then v1.0.0's `kani/RESULTS.md` (as of `a072a2d`) for a harness whose
  name is unchanged, its peak times 1.25; a mutant from the harnesses
  its Kani leg is expected to run, plus 240 s of plain mode and build;
  otherwise unknown, 40 GB (`KANI_MAX_GB`, the most `run.sh` lets a job
  use) and 1800 s.
- A job estimated at 35 GB or more runs alone (the unknowns, and
  v1.0.0 numbers scaled near 40 GB): it starts only when no job runs,
  and no job starts beside it, nor beside a running job whose sampled
  process tree has reached 35 GB. Other jobs share the budget
  (`--budget-gb`, 50), a running one counted at its estimate or its
  sampled tree peak, whichever is more. (By the budget alone, a 40 GB
  job could run beside one estimated at 10 GB, and v1.1.0's small step
  harnesses peaked at up to 1.8 times their v1.0.0 numbers.)
- The limit (`KANI_TIMEOUT`) is 3 times the estimate, at least 1800 s,
  and at least 7200 s for `run.sh`'s 7200 s tier and for an unknown.
  `KANI_MAX_GB` stays 40.
- It holds `/tmp/sp-heavy.lock` (the machine's lock for heavy jobs:
  `mkdir` takes it, `rmdir` releases it) from its start to its exit,
  also on a failure or a signal. While another process holds the lock,
  it waits, checking every 30 s. It removes only a lock it took. Before
  it releases the lock (SIGINT, SIGTERM, an error), it kills its running
  jobs: each one's process group and every process in its tree, listed
  first, so that a process in a group of its own is killed too. SIGKILL
  of `run-local.py` leaves the jobs running and the lock held.
- Every peak is resident memory, `ps`'s RSS (below: it is not the
  footprint).
- It is not a record by itself. `--record` is refused, the output is
  outside the repository (default
  `~/shared-pool-kani-measure/all-<commit>-<time>`), and `env.txt` says
  `mode measurement`. `record.py` takes the large matrices' jobs from one
  or more measurements (above), and `provenance.txt` says the record was
  assembled from them.

### Resident memory is not the footprint

Every peak in the record, in `../RESULTS.md` and in the matrices is
resident memory: `run-local.py` samples `ps`'s RSS of each job's CBMC and
of its whole process tree every 3 s (`cbmc_peak_gb`, `tree_peak_gb`), and
`run.sh`'s `KANI_MAX_GB` cap compares the tree's RSS too. Under memory
pressure macOS compresses pages, and RSS leaves the compressed ones out,
so it can understate a job's footprint by a lot. During run 3 at v1.1.0, a
side sampler of `top`'s MEM column (the physical footprint; not part of
the record or of the repository; it started at 18:28Z and missed about
the first 16 minutes of `h_bmc_from_init_po`) saw:

| Job | Run 3 recorded (RSS) | Footprint (`top`) |
|---|---|---|
| `h_bmc_from_init_po` | 37.3 GB | 63 GB |
| `h_full_256` | 39.2 GB | 72 GB, on the 64 GB machine |
| `h_step_observe_inv_3` | 10.3 GB | 15 GB, while `h_full_256` ran beside it |

So:

- The 13 GB split and each entry's `gb` are RSS peaks. The hosted
  runner's 2.6 GB to spare and its swap file are the margin for the
  difference, and CI's `kani` job, which runs every hosted harness when a
  change reaches the Kani layer, is the check that a hosted harness fits:
  one that does not fit fails there, it is not skipped. Of the hosted
  harnesses the side sampler saw, `h_step_observe_inv_3` (15 GB, above)
  came nearest to the runner's memory; it is in a `kani` group at v1.1.0.
  If a hosted harness fails there for memory, add it to `CI_LARGE` in
  `tools/scripts/kani-matrices.py`, as `h_full_256` is: that keeps it in
  `kani-large` (and a mutant whose Kani leg runs it in
  `kani-mutants-large`) whatever its local peak. Then rerun
  `kani-matrices.py --write`, `record.py` and `results-tables.py --write`
  on the same runs. A run must have passed the harness (run 3 passed
  `h_step_observe_inv_3`); a large mutant that no run finished goes into
  `not-run.tsv`.
- `KANI_MAX_GB` (40) did not stop `h_full_256` at a 72 GB footprint,
  because its RSS stayed at 39.2 GB.
- The memory budget (`--budget-gb`) counts RSS (a job's estimate or its
  sampled tree peak), so jobs that fit the budget together can exceed the
  machine's memory. macOS then compresses and swaps, and the jobs can slow
  down; their times are with a neighbour, as `../RESULTS.md` says.
- Linux does not compress pages by default, so a Linux runner may need
  nearer the footprint than the RSS. A `KANI_RUNNER` sized at 1.25 times
  the largest `gb` (CI, below) is a rule from RSS peaks; its first
  `kani-large` run shows whether it is enough.

Commands:

    projects/shared-pool/kani/local/run-local.py --all --dry-run   # jobs, estimates, hours
    projects/shared-pool/kani/local/run-local.py --all             # about 12-18 h at 5cf571e
    projects/shared-pool/kani/local/run-local.py --all --only h_step_ack_tx,mutant-09

## CI

The `kani-record` job runs on every workflow run. It runs the self-test,
then `check-kani-record.py`, which fails when:

- The record's fingerprint is not HEAD's. The message names the changed
  inputs (and, where the clone has the record's commit, the changed
  files) and says "re-run the local Kani record:" with the command above.
- The fingerprint is not the hash of its input lines, or `jobs.tsv`,
  `covers.tsv` or `not-run.tsv` does not match the digest, as when they
  are edited by hand.
- The fingerprint is not that of the record's `commit`, recomputed. This
  is checked where the clone has that commit; CI's shallow checkout
  usually does not, and skips it.
- A recorded harness is not `ok`, or a recorded mutant is not as its patch
  expects.
- The `kani` groups and the recorded harnesses do not cover `run.sh list`
  exactly once each, or the `kani-large` groups are not the recorded
  harnesses.
- The same holds for `mutants/*.patch`: the `kani-mutants` entries plus the
  recorded mutants must cover them exactly once, and the
  `kani-mutants-large` entries must be the recorded mutants, except those
  declared in `not-run.tsv`. Each declared one is a warning on every run
  ("NOT RUN locally"), and the summary line counts them; one that is also
  recorded or in `kani-mutants`, or `not-run.tsv` edited after
  `record.py` (the digest), fails.
- A `kani-large` or `kani-mutants-large` entry's timeout is below the
  `limit_s` its recorded jobs ran under: the record shows a pass within
  that limit, not within less. A higher timeout is fine. When this run's
  `kani-large` (`--stale-ok-harnesses`) or `kani-mutants-large`
  (`--stale-ok-mutants`) jobs run, they verify under the new timeout, and
  it is a warning.
- A `kani` or `kani-mutants` entry peaked above 13 GB.
- RESULTS.md has no block, or its block is not `record.py table`.

The check shows that the record is consistent and belongs to the commit's
inputs. It cannot show that the run took place as recorded: the record is
attested by whoever ran and committed it, as the measurements in
`../RESULTS.md` are.

The `covers` job counts the recorded harnesses with the groups' harnesses
(all of `run.sh list`) and checks the recorded covers with the groups' covers.

When `KANI_RUNNER` names a runner with enough memory (at least 1.25
times the largest `gb` of the `kani-large` and `kani-mutants-large`
entries, each a measured RSS peak rounded up; strategy.md section 14
gives the number; v1.0.0's was 49 GB), other than `ubuntu-latest`, and the run is not routed
to `ubuntu-latest`, the pin job's `large` output is true. Then
`kani-large` runs there when the change reaches the Kani layer, and
`kani-mutants-large` on a manual run with `mutants`. A hosted
`KANI_RUNNER` with less memory than a group's peak fails that group at
once. `kani-record` still checks the record, and a stale record is only a
warning when this run verifies everything it records: `kani-large` its
harnesses (`--stale-ok-harnesses`) and `kani-mutants-large` its mutants
(`--stale-ok-mutants`). `kani-mutants-large` runs only on a manual run
with `mutants`, so a change to the Kani inputs needs a new record
whatever `KANI_RUNNER` is: until it is re-made, `kani-record` fails on
every pull request and push, and passes (with a warning) only on a
manual run with `kani` and `mutants` on `KANI_RUNNER`, for that run
alone. When `kani-large` runs, `covers` uses the groups' covers alone.

## Notes on the recorded runs

- From v1.1.0 the record is assembled from `run-local.py --all`
  measurements of one commit (`2d33d33`, whose fingerprint is v1.1.0's)
  rather than a separate `--record` run: `provenance.txt` says so
  (`assembled`), lists the runs, and gives each job's run. The
  `kani-mutants-large` mutants that no measurement finished before the
  v1.1.0 deadline are in `not-run.tsv`, recorded as not run at v1.1.0;
  CI's `mutants` job still checks their plain mode.
- `KANI_MAX_GB` is `run.sh`'s default, 40 GB, as in the measurements in
  `../RESULTS.md`. `h_wedge` peaked at 39.0 GB at v1.0.0 (32.6 GB in W7's
  v1.1.0 probe) and `h_full_256` at 39.2 GB in run 3, so a run where one
  of them needs 1 GB more resident memory stops it (OUT-OF-MEMORY, not
  ok), and it is not recorded.
- v1.0.0's run at `89f99d6` was made by `sched.py`, the prototype of
  `run-local.py`: the same jobs, lanes and sampling, but without the
  environment checks. Its scheduler's environment had no `KANI*`, `RUST*`
  or `CARGO*` variables, and no cargo configuration file applied outside
  the Kani tree (both checked while it ran). Its mutants' plain mode used
  the host's default Rust toolchain (rustc 1.95.0), not `RUST_TOOLCHAIN`
  (1.97.0); `run-local.py` now gives the mutant jobs
  `RUSTUP_TOOLCHAIN=<RUST_TOOLCHAIN>`, as CI does. Kani pins its own
  toolchain. The record does not cover the plain half of a mutant's
  verdict: CI's `mutants` job re-checks plain mode for every mutant with
  the pinned toolchain on every run.
