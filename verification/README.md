# Formal verification (copied from formalver)

This directory is a verbatim copy of the shared-pool-controls verification from
[difflabai/formalver](https://github.com/difflabai/formalver), taken at commit `da343e2`
(formalver PR #32, which is stacked on PR #31 at `651852b`). It verifies contract **v1.1.0**, this
repository's commit `57b3f02`. No file under `crates/` has changed between `57b3f02` and `7c11f88`,
the `main` this branch starts from, so the verified code is the code on `main`.

The formalver CI run on `da343e2` is green:
[run 43642798201](https://github.com/difflabai/formalver/actions/runs/43642798201) (18 jobs passed;
the 4 dispatch-only jobs were skipped).

## What is here

| Path | Contents |
|---|---|
| `tla/` | TLA+ models (with the executor model) and TLC configurations |
| `lean/` | Lean proofs |
| `prolog/` | Prolog model and checks |
| `kani/` | Kani harnesses; `kani/local/` is the fingerprinted local record of the 48 large harnesses, and `kani/local/not-run.tsv` lists the 27 large mutants with no Kani result at v1.1.0 |
| `conformance/` | 216 conformance traces and their replay |
| `near-tests/`, `sandbox/` | NEAR unit tests and sandbox tests |
| `status.toml`, `strategy.md` | Claims, findings and the verification strategy |
| `tools/` | The formalver scripts these layers and the CI workflow call |
| `ci/shared-pool.yml` | formalver's CI workflow, kept here **inactive** (it is not under `.github/workflows/`) |

## Not wired up yet

The files are unchanged from formalver, so their paths still assume formalver's layout:

| formalver path | Path in this repository |
|---|---|
| `projects/shared-pool/source-mirror/` (a submodule pinned at `57b3f02`) | the repository root |
| `projects/shared-pool/` | `verification/` |
| `tools/` | `verification/tools/` |
| `.github/workflows/shared-pool.yml` | `verification/ci/shared-pool.yml` |

Nothing here runs in this repository's CI yet, and the root Cargo workspace does not include these
crates. Follow-ups to make it run here:

1. Rewrite the paths above (90 of the copied files refer to `source-mirror`).
2. Replace the submodule pin (`tools/scripts/check-pin.sh`) with a check against this repository's
   own commit, and decide how the Kani record's fingerprint should treat contract changes: inside this
   repository, any change to the contract makes the record stale.
3. Check the verification crates' toolchains against the root `rust-toolchain.toml`. The crates
   (`kani`, `conformance`, `near-tests`, `sandbox` with `sandbox/relay`) already declare their own
   `[workspace]`, so the root workspace leaves them alone.
4. Move `ci/shared-pool.yml` into `.github/workflows/`, limited by `paths:` filters (a full run takes
   about 48 minutes), and set `vars.CI_RUNNER` / `vars.KANI_RUNNER` if self-hosted runners are used.

## History

The root `README.md` and `docs/verification-handoff.md` point here as well as to formalver.
The commit history, review threads and CI runs stay in formalver: PR #31 (v1.0.0 verification) and
PR #32 (v1.1.0 re-verification). The v1.0.0 findings, their fixes in this repository's PR #4, and the
regression evidence (fails on v1.0.0, passes on v1.1.0) are recorded in `status.toml` and
`strategy.md`.
