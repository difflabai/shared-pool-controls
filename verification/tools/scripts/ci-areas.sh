#!/usr/bin/env bash
# Which near-approval CI jobs a change needs. Reads changed paths (one per
# line, repo-relative, as `git diff --name-only` prints them) on stdin and
# prints `<job>=true|false` for each gated job, for $GITHUB_OUTPUT.
#
# The rules list what each path can affect. Unknown verification paths run
# every job; unrelated repository paths and documentation run none.
# The pin job is not gated: it always runs (status.toml, the gitlink).
#
# usage: git diff --name-only BASE HEAD | tools/scripts/ci-areas.sh
#        tools/scripts/ci-areas.sh --all     (every job; stdin unread)
#        CI_AREAS_EXPLAIN=1 ...              (also each path's jobs on stderr)
set -euo pipefail

jobs="plain loops kani tla lean prolog near_unit sandbox verus"
on=" "
p=projects/near-approval
s=tools/scripts

if [ "${1:-}" = --all ]; then
  on=" $jobs "
else
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in
      # Docs: no script, test or proof reads a Markdown file.
      *.md | *.mdx | *.rst | *.adoc | docs/* | Plans/* | */docs/*) add="" ;;
      # status.toml also holds run-verus.sh's floor (verus_min_verified), so
      # an edit to it re-runs the verus gate; pin and status-lint check the rest.
      "$p"/status.toml) add="verus" ;;
      # Checked by the pin job alone (pin and status-lint).
      "$s"/check-pin.sh | "$s"/check-status.sh | "$s"/test-check-status.sh) add="" ;;
      # shared-pool's local Kani record check (shared-pool.yml kani-record).
      "$s"/check-kani-record.py | "$s"/test-check-kani-record.sh) add="" ;;
      # A recording PR only advances the already-successful schedule baseline.
      .github/approval-seen | "$s"/record-approval-check.sh | "$s"/test-record-approval-check.sh | "$s"/approval-changed.sh | "$s"/test-approval-changed.sh | .github/workflows/near-approval-watch.yml) add="" ;;
      # The weekly upstream watch.
      "$s"/upstream-watch.sh | tools/upstream-watch.toml) add="" ;;
      # M5: the verified twin, its canary, mutants and erasure check.
      "$p"/verus/* | "$s"/run-verus.sh | "$s"/get-verus.sh | "$s"/erasure-check.py) add="verus" ;;
      # Mutant definitions: the weekly mutants job, and plain's self-test.
      "$p"/kani/mutants/* | "$s"/run-mutants.sh | "$s"/test-run-mutants.sh) add="plain" ;;
      # U-1 loop bounds.
      "$p"/kani/unwind-rules.tsv | "$s"/kani-loops.py) add="loops" ;;
      "$s"/run-kani.sh | "$s"/check-covers.sh) add="kani" ;;
      # ApprovalCore.tla feeds tla_facts.pl (prolog), and Lean's MC-4
      # compares against the TLC graph.
      "$p"/tla/*) add="tla lean prolog" ;;
      "$s"/run-tla.sh) add="tla" ;;
      "$s"/get-tla-tools.sh) add="tla lean" ;;
      "$p"/lean/* | "$s"/run-lean.sh | "$s"/get-lean.sh) add="lean" ;;
      # plain's `facts` test reads code_facts.pl; run-lean.sh reads
      # tla_facts.pl.
      "$p"/prolog/verification/verifier_rules.pl | "$p"/prolog/tests/test_verifier_rules.pl) add="prolog sandbox" ;;
      # near-unit regenerates api_facts.pl from the wasm and diffs it.
      "$p"/prolog/extracted/api_facts.pl) add="prolog near_unit" ;;
      "$p"/prolog/extracted/*) add="prolog plain lean" ;;
      "$p"/prolog/* | "$s"/run-prolog.sh | "$s"/run-mc3.sh | "$s"/tla-facts.py) add="prolog" ;;
      "$s"/mc3.py) add="prolog lean" ;;
      # M3: the shell tests and the wasm surface.
      "$p"/near-tests/mutants/* | "$p"/near-tests/run-mutants.sh) add="" ;;
      "$p"/near-tests/* | "$p"/wasm-surface.expected | "$s"/wasm-surface.sh) add="near_unit" ;;
      "$s"/get-cargo-near.sh) add="near_unit sandbox" ;;
      # M6/M7: the sandbox suite, its traces and the deploy checks it
      # demonstrates; the verifier rules also run in prolog.
      "$p"/sandbox/* | "$p"/conformance/* | "$p"/deploy-checks/*) add="sandbox" ;;
      # Everything else, including the kani crate (every job builds or runs
      # it), the trust-framework gitlink, versions.env, ci-submodule.sh,
      # this script and the workflow itself.
      "$p"/* | tools/* | .gitmodules | Cargo.toml | Cargo.lock | .github/workflows/near-approval.yml) add="$jobs" ;;
      *) add="" ;;
    esac
    if [ -n "${CI_AREAS_EXPLAIN:-}" ]; then
      echo "$f: ${add:-(none)}" >&2
    fi
    for j in $add; do
      case "$on" in *" $j "*) ;; *) on="$on$j " ;; esac
    done
  done
fi

for j in $jobs; do
  case "$on" in *" $j "*) echo "$j=true" ;; *) echo "$j=false" ;; esac
done
