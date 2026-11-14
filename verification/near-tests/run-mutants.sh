#!/usr/bin/env bash
# Applies each shell mutant in projects/shared-pool/near-tests/mutants/ to a
# temporary copy of the pinned source (crates/contract or crates/executor)
# and checks that the near-tests suite fails on it (strategy.md section 9,
# M3; conventions of projects/near-approval/near-tests/run-mutants.sh): a
# verdict needs evidence ("killed" only when a test actually failed); a
# patch or build failure is ERROR. The unpatched copy runs first as the
# control.
#
# The documented wasm build (scripts/build.sh: RUSTFLAGS="--cfg near") of
# the patched tree is built here, before the tests, so a build failure is
# ERROR and never counts as a kill. The no-cfg (Plain) build is the NT-F1
# negative control only (nt_f1_* in tests/shell.rs): it is built once, from
# the unpatched tree, and shared by every mutant.
#
# Each patch names the property it breaks on its first line: s01-s15 and
# s22-s28 (crates/contract) SH-1..SH-10, s16-s21 and s29-s38 (crates/executor)
# EX-1..EX-8. s00a and s00b are historical mutants: the v1.0.0 production
# code of the contract and of the executor, the evidence that the tests of
# the v1.1.0 fixes fail on v1.0.0. Their header names a harness cfg
# (`--cfg nt_v100_contract` / `nt_v100_executor`: the v1.0.0 on_result
# signature and executor state), which their test build gets through
# RUSTFLAGS, in a target dir of its own. No mutant targets RD-PREVIEW
# (tests/shell.rs and store_diff check it against the core) or SCN-1
# (tests/scenarios.rs reads crates/core only).
# PROP: SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-8 SH-9 SH-10 EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8
#
# usage: projects/shared-pool/near-tests/run-mutants.sh [patch-glob]
#   MUTANT_STORE_DIFF_SEQS  host store_diff sequences per mutant (default 300 x 40)
#   MUTANT_WASM_DIFF_SEQS   wasm store_diff sequences per build (default 40 x 40)
#   NT_SKIP_TRACES=1        leave tests/traces.rs (the 216 conformance traces
#                           through the shell) out of the suite, for a quicker
#                           run; the default includes it
#   KEEP_LOGS=<dir>         keep each mutant's test log
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

glob=${1:-s*}
here=$(cd "$(dirname "$0")" && pwd)
proj=$(cd "$here/.." && pwd)
work=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/shared-pool-mutants.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT
# One target dir for every mutant: only the patched crate and the tests rebuild.
export CARGO_TARGET_DIR="$work/target"
export STORE_DIFF_SEQS=${MUTANT_STORE_DIFF_SEQS:-300}
export WASM_DIFF_SEQS=${MUTANT_WASM_DIFF_SEQS:-40}
export POOL_WASM_DIR_PLAIN="$work/wasm/plain/wasm32-unknown-unknown/release"
export POOL_WASM_DIR_NEAR="$work/wasm/near/wasm32-unknown-unknown/release"

verdict() { # <rc> <log>: survived | killed | ERROR
  if [ "$1" -eq 0 ] && grep -q '^test result: ok' "$2" && ! grep -q '^test result: FAILED' "$2"; then
    echo survived
  elif [ "$1" -ne 0 ] && grep -q '^test result: FAILED' "$2"; then
    echo killed
  else
    echo ERROR
  fi
}

# tree <patch|->: a fresh copy of the source (patched) and the suite.
tree() {
  rm -rf "$work/tree"
  mkdir -p "$work/tree/source-mirror" "$work/tree/near-tests"
  # The whole workspace (its members must resolve); Cargo.lock: tests/lockfile.rs
  # compares the suite's lockfile with the mirror's.
  cp -r "$proj/source-mirror/Cargo.toml" "$proj/source-mirror/Cargo.lock" \
    "$proj/source-mirror/rust-toolchain.toml" "$proj/source-mirror/crates" "$work/tree/source-mirror/"
  # tests/scenarios.rs reads the published scenarios.
  cp -r "$proj/source-mirror/scenarios" "$work/tree/source-mirror/"
  cp -r "$here/Cargo.toml" "$here/Cargo.lock" "$here/rust-toolchain.toml" "$here/src" "$here/tests" \
    "$work/tree/near-tests/"
  [ "${NT_SKIP_TRACES:-}" = 1 ] && rm "$work/tree/near-tests/tests/traces.rs"
  # tests/traces.rs replays the conformance traces.
  mkdir -p "$work/tree/conformance"
  cp -r "$proj/conformance/traces" "$work/tree/conformance/"
  [ "$1" = - ] || (cd "$work/tree/source-mirror" && patch -s -p1 < "$1")
}

# wasm <build>: one build of the current tree into the shared wasm target
# dir: near = scripts/build.sh's flags, plain = no cfg.
wasm() {
  local flags=""
  [ "$1" = near ] && flags="--cfg near"
  (cd "$work/tree/source-mirror" && env -u CARGO_TARGET_DIR -u CARGO_ENCODED_RUSTFLAGS RUSTFLAGS="$flags" \
    CARGO_TARGET_DIR="$work/wasm/$1" cargo build --target wasm32-unknown-unknown --release --locked \
    -p pool-contract -p pool-executor) >>"$work/log" 2>&1
}

run() { # [harness-cfg] -> rc of the suite; log in $work/log
  : >"$work/log"
  wasm near || return 99
  if [ -n "${1:-}" ]; then
    # A historical mutant: the harness needs its cfg, in a target dir of its
    # own (RUSTFLAGS rebuilds every dependency).
    (cd "$work/tree/near-tests" && env -u CARGO_ENCODED_RUSTFLAGS RUSTFLAGS="--cfg $1" \
      CARGO_TARGET_DIR="$work/target-$1" cargo test --locked --no-fail-fast) >>"$work/log" 2>&1
  else
    (cd "$work/tree/near-tests" && cargo test --locked --no-fail-fast) >>"$work/log" 2>&1
  fi
}

# Control: the unpatched copy must pass, or every "killed" below is noise.
tree - || exit 2
: >"$work/log"
wasm plain || { tail -n 40 "$work/log"; echo "the Plain wasm does not build" >&2; exit 2; }
run
rc=$?
v=$(verdict $rc "$work/log")
printf '%-44s %-8s\n' "(unpatched control)" "$v"
if [ "$v" != survived ]; then
  tail -n 40 "$work/log"; echo "the unpatched suite does not pass in the mutant tree" >&2; exit 2
fi

fail=0
n=0
printf '%-44s %-8s %s\n' mutant verdict failing-tests
for p in "$here"/mutants/$glob.patch; do
  [ -e "$p" ] || continue
  name=$(basename "$p" .patch)
  n=$((n + 1))
  if ! tree "$p"; then
    printf '%-44s %s\n' "$name" "PATCH-FAILED"; fail=1; continue
  fi
  cfg=$(sed -n '1,6s/.*harness builds with --cfg \([a-z0-9_]*\).*/\1/p' "$p")
  run "$cfg"
  rc=$?
  [ -n "${KEEP_LOGS:-}" ] && cp "$work/log" "$KEEP_LOGS/$name.log"
  if [ $rc -eq 99 ]; then
    v=ERROR
  else
    v=$(verdict $rc "$work/log")
  fi
  failing=$(grep -a -E '^    [a-z0-9_]+$' "$work/log" | sed 's/^ *//' | sort -u | tr '\n' ' ')
  [ "$v" = killed ] || fail=1
  printf '%-44s %-8s %s%s\n' "$name" "$v" "$failing" "$([ "$v" = killed ] || echo '  <-- UNEXPECTED')"
done
[ $n -gt 0 ] || { echo "no mutants matched $glob" >&2; exit 2; }
exit $fail
