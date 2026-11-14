#!/usr/bin/env bash
# Runner of the shared-pool shell layer (near-tests).
#
# usage: projects/shared-pool/near-tests/run.sh [test|mutants|surface|all] [args]
#   test     check-tags.py first (PROP-tag traceability, stdlib python3 -I,
#            under a second; any problem fails the run), then
#            cargo test --locked --no-fail-fast (lockfile, shell, executor,
#            wedge, scenarios, traces, store_diff at the default 2,000 x 40
#            host and 300 x 40 wasm sequences; STORE_DIFF_SEQS /
#            STORE_DIFF_STEPS / STORE_DIFF_SEED / WASM_DIFF_SEQS override).
#            The wasm path runs the documented build (scripts/build.sh's
#            RUSTFLAGS="--cfg near"), built on first use into
#            target/wasm/near; the no-cfg build (target/wasm/plain) is only
#            the NT-F1 negative control. Each test names its properties on
#            its own tag line.
#            PROP: RD-PREVIEW SCN-1 SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-8 SH-9 SH-10 EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8 LIVE-ALL
#   mutants  run-mutants.sh [patch-glob]: every mutants/*.patch must be killed
#            (the unpatched control must pass first), the historical
#            mutants s00a/s00b (the v1.0.0 contract and executor) included.
#            PROP: SH-1 SH-2 SH-3 SH-4 SH-5 SH-6 SH-7 SH-8 SH-9 SH-10 EX-1 EX-2 EX-3 EX-4 EX-5 EX-6 EX-7 EX-8
#   surface  wasm-surface.sh, all three must hold: SURF-1 (the surface ==
#            ../wasm-surface.expected; SH-11), SURF-2 (no import of a
#            forbidden host function: deploy, keys, transfer, stake,
#            accounts, global contracts, promise_batch_*; SH-12), SURF-3
#            ((a) upstream scripts/build.sh passes on the pin, its
#            artifacts/build.json hashes are this layer's wasm and the
#            imports equal its ALLOWED lists; (b) its checker refuses the
#            no-cfg wasm and two synthetic modules; SH-5 SH-12).
#            PROP: SH-5 SH-11 SH-12 ASM-DEPLOY
#   all      test (tag check included), surface, mutants (default).
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
here=$(cd "$(dirname "$0")" && pwd)
mode=${1:-all}
[ $# -gt 0 ] && shift

run_tags() {
  python3 -I "$here/check-tags.py"
}

run_test() {
  local s=0
  run_tags || s=1
  (cd "$here" && cargo test --locked --no-fail-fast "$@") || s=1
  return $s
}

run_surface() {
  "$here/wasm-surface.sh"
}

run_mutants() {
  "$here/run-mutants.sh" "$@"
}

case "$mode" in
  test) run_test "$@" ;;
  surface) run_surface ;;
  mutants) run_mutants "$@" ;;
  all)
    s=0
    run_test || s=1
    run_surface || s=1
    run_mutants || s=1
    exit $s
    ;;
  *) echo "usage: $0 [test|mutants|surface|all]" >&2; exit 2 ;;
esac
