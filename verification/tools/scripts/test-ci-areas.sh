#!/usr/bin/env bash
# Self-test for ci-areas.sh: which jobs each kind of change runs.
#
# usage: tools/scripts/test-ci-areas.sh
set -uo pipefail
areas="$(cd "$(dirname "$0")" && pwd)/ci-areas.sh"
fail=0
p=projects/near-approval

# expect "<jobs that run>" <changed path>...
expect() {
  local want=$1 got
  shift
  got=$(printf '%s\n' "$@" | "$areas" | sed -n 's/=true$//p' | tr '\n' ' ')
  got=${got% }
  if [ "$got" != "$want" ]; then
    echo "FAIL: $* -> '$got', want '$want'"
    fail=1
  fi
}

all="plain loops kani tla lean prolog near_unit sandbox verus"
expect "" $p/strategy.md $p/kani/RESULTS.md README.md
expect "verus" $p/status.toml
expect "" tools/scripts/check-pin.sh tools/scripts/check-status.sh tools/scripts/test-check-status.sh
expect "" tools/scripts/check-kani-record.py tools/scripts/test-check-kani-record.sh
expect "" .github/approval-seen tools/scripts/record-approval-check.sh tools/scripts/test-record-approval-check.sh
expect "" tools/scripts/approval-changed.sh tools/scripts/test-approval-changed.sh .github/workflows/near-approval-watch.yml
expect "" docs/architecture.svg projects/near-approval/lean/docs/diagram.png .github/CODEOWNERS
expect "" projects/ironclaw/verus/src/lib.rs
expect "prolog" projects/near-approval/prolog/requirements/r.pl README.md .gitignore
expect "" tools/upstream-watch.toml tools/scripts/upstream-watch.sh
expect "verus" $p/verus/src/lib.rs $p/verus/mutants.toml tools/scripts/run-verus.sh tools/scripts/get-verus.sh tools/scripts/erasure-check.py
expect "" # no changes
expect "near_unit" $p/near-tests/tests/shell.rs $p/wasm-surface.expected tools/scripts/wasm-surface.sh
expect "" $p/near-tests/mutants/m01.patch $p/near-tests/run-mutants.sh
expect "prolog near_unit" $p/prolog/extracted/api_facts.pl
expect "sandbox" $p/sandbox/tests/runtime.rs $p/conformance/traces/prolog.txt $p/deploy-checks/check_deployment.py
expect "prolog sandbox" $p/prolog/verification/verifier_rules.pl
expect "near_unit sandbox" tools/scripts/get-cargo-near.sh
expect "$all" $p/source-mirror
expect "$all" $p/kani/src/lib.rs
expect "$all" $p/kani/Cargo.lock
expect "$all" tools/versions.env
expect "$all" .github/workflows/near-approval.yml
expect "$all" tools/scripts/ci-areas.sh
expect "$all" $p/some-new-dir/file
expect "plain" $p/kani/mutants/01-guard.patch
expect "loops" $p/kani/unwind-rules.tsv
expect "kani" tools/scripts/run-kani.sh
expect "tla lean prolog" $p/tla/ApprovalCore.tla
expect "tla" tools/scripts/run-tla.sh
expect "lean" $p/lean/ApprovalLean/Step.lean $p/lean/lean-toolchain
expect "prolog" $p/prolog/requirements/r.pl
expect "plain lean prolog" $p/prolog/extracted/code_facts.pl
expect "tla lean prolog" $p/lean/Main.lean $p/tla/MC_Safety.cfg $p/strategy.md
expect "$all" $p/lean/Main.lean $p/kani/src/trace.rs

out=$("$areas" --all </dev/null | tr '\n' ' ')
[ "$out" = "plain=true loops=true kani=true tla=true lean=true prolog=true near_unit=true sandbox=true verus=true " ] ||
  { echo "FAIL: --all -> $out"; fail=1; }

[ "$fail" = 0 ] && echo "ci-areas: all cases pass"
exit "$fail"
