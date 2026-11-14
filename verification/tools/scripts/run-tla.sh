#!/usr/bin/env bash
# Runs the TLA+ checks of a project (strategy.md section 17.4) and checks the
# outcome each one requires. Emits ::error:: / ::notice:: annotations, which
# a cloud session can read from the check run when Actions logs are blocked.
#
#   safety     TLC, MC_Safety: Inv, StepProperty (TLA-SAFE); then each
#              Witness_* invariant must be VIOLATED (non-vacuity, TR-5)
#   liveness   TLC, MC_Liveness: Live1 and Inv under fairness (TLA-LIVE-1/2);
#              MC_Liveness_Unfair: Live1 must be VIOLATED without fairness;
#              MC_Liveness_SF: Live1Inf under strong fairness (TLA-LIVE-1s)
#   apalache   Apalache, MC_Apalache: Init => Inv, Inv /\ Next => Inv' and
#              Inv /\ Next => StepProps (TLA-IND); IndWitness must be VIOLATED
#   trace      REF-5: kani/src/trace.rs runs the real core and checks it
#              against tla.rs, then TLC replays its traces through TraceCheck
#   mutants    every tla/mutants/*.patch must make trace, apalache or
#              liveness fail
#
# usage: tools/scripts/run-tla.sh <project> [check...]   (default: all but mutants)
# env:   TLA_WORKERS (default auto), TLA_HEAP (default 6g), TLA_LOG_DIR
set -uo pipefail

project=${1:?usage: run-tla.sh <project> [check...]}
shift
root=$(cd "$(dirname "$0")/../.." && pwd)
dir="$root/projects/$project/tla"
[ -d "$dir" ] || { echo "no TLA+ directory at $dir" >&2; exit 2; }
. "$root/tools/versions.env"
cache=$("$root/tools/scripts/get-tla-tools.sh") || exit 2
jar="$cache/tla2tools-$TLA2TOOLS.jar"
apalache="$cache/apalache-$APALACHE/bin/apalache-mc"
logs=${TLA_LOG_DIR:-$dir/build/logs}
mkdir -p "$logs"
[ $# -gt 0 ] || set -- safety liveness apalache trace

# tlc <spec dir> <module> <cfg> <log> [java -D options...]
# Each run gets its own java.io.tmpdir: TLC 1.7.4 unpacks its standard
# modules (Naturals.tla, ...) under fixed names there, so runs sharing one
# (CI jobs on one self-hosted host) would overwrite each other's files.
tlc() {
  local d=$1 m=$2 c=$3 log=$4 tmp
  shift 4
  tmp=$(mktemp -d)
  (cd "$d" && java -XX:+UseParallelGC -Xmx"${TLA_HEAP:-6g}" -Djava.io.tmpdir="$tmp" "$@" \
    -cp "$jar" tlc2.TLC -workers "${TLA_WORKERS:-auto}" -cleanup -metadir "$tmp/states" \
    -config "$c" "$m") >"$log" 2>&1
  rm -rf "$tmp"
  rm -f "$d"/*_TTrace_*
}
tlc_ok() { grep -q 'Model checking completed. No error has been found.' "$1"; }
states() { grep -oE '[0-9,]+ distinct states found' "$1" | tail -1; }

# apa <spec dir> <log> <apalache args...>
apa() {
  local d=$1 log=$2
  shift 2
  (cd "$d" && "$apalache" check --out-dir="$(mktemp -d)" "$@" MC_Apalache.tla) >"$log" 2>&1
}
apa_ok() { grep -q 'EXITCODE: OK' "$1"; }

fail=0
report() { # name ok(0/1) detail
  if [ "$2" = 0 ]; then
    printf '%-26s ok    %s\n' "$1" "$3"
    echo "::notice::tla $1: ok ($3)"
  else
    printf '%-26s FAIL  %s\n' "$1" "$3"
    echo "::error::tla $1: FAILED ($3), log $4"
    fail=1
  fi
}

check_safety() { # spec dir
  local d=$1 log="$logs/safety.log"
  start=$(date +%s)
  tlc "$d" MC_Safety.tla MC_Safety.cfg "$log"
  tlc_ok "$log"; report safety $? "$(states "$log"), $(( $(date +%s) - start ))s" "$log"
  for w in Witness_Approved Witness_Rejected Witness_Void Witness_Revival Witness_Cap; do
    log="$logs/$w.log"
    sed -e "s/^INVARIANT Inv$/INVARIANT $w/" -e '/^PROPERTY /d' "$d/MC_Safety.cfg" >"$d/build/$w.cfg"
    tlc "$d" MC_Safety.tla "build/$w.cfg" "$log"
    grep -q "Invariant $w is violated" "$log"; report "$w" $? "must be reachable" "$log"
  done
}

check_liveness() {
  local d=$1 log="$logs/liveness.log"
  start=$(date +%s)
  tlc "$d" MC_Liveness.tla MC_Liveness.cfg "$log"
  tlc_ok "$log"; report liveness $? "$(states "$log"), $(( $(date +%s) - start ))s" "$log"
  log="$logs/liveness_unfair.log"
  tlc "$d" MC_Liveness.tla MC_Liveness_Unfair.cfg "$log"
  grep -qE 'Temporal propert(y|ies) .* violated' "$log"
  report liveness-unfair $? "Live1 must fail without fairness" "$log"
  log="$logs/liveness_sf.log"
  start=$(date +%s)
  tlc "$d" MC_Liveness.tla MC_Liveness_SF.cfg "$log"
  tlc_ok "$log"; report liveness-sf $? "$(states "$log"), $(( $(date +%s) - start ))s" "$log"
}

check_apalache() {
  local d=$1 log="$logs/apalache-init.log"
  apa "$d" "$log" --init=Init --inv=Inv --length=0
  apa_ok "$log"; report apalache-init $? "Init => Inv" "$log"
  log="$logs/apalache-step.log"
  start=$(date +%s)
  apa "$d" "$log" --init=IndInit --inv=Inv --length=1
  apa_ok "$log"; report apalache-step $? "Inv /\\ Next => Inv', $(( $(date +%s) - start ))s" "$log"
  log="$logs/apalache-step-props.log"
  apa "$d" "$log" --init=IndInit --inv=StepProps --length=1
  apa_ok "$log"; report apalache-step-props $? "Inv /\\ Next => StepProps" "$log"
  log="$logs/apalache-witness.log"
  apa "$d" "$log" --init=IndInit --inv=IndWitness --length=0
  grep -q 'state invariant 0 violated' "$log"; report apalache-witness $? "IndInit is not vacuous" "$log"
}

check_trace() {
  local d=$1 log="$logs/trace-gen.log"
  mkdir -p "$d/build"
  (cd "$root/projects/$project/kani" && REF5_OUT="$d/build" cargo test --release --locked --lib trace::) >"$log" 2>&1
  grep -q 'test result: ok. 1 passed' "$log"; report trace-core $? "core = tla.rs on every step" "$log"
  log="$logs/trace-tlc.log"
  tlc "$d" TraceCheck.tla TraceCheck.cfg "$log" -DTLA-Library="$d/build"
  tlc_ok "$log"; report trace-tlc $? "TLC accepts every core trace" "$log"
}

check_mutants() {
  local tmp
  for p in "$dir"/mutants/*.patch; do
    tmp=$(mktemp -d)
    cp -r "$dir"/. "$tmp/"
    rm -rf "$tmp/mutants"
    if ! patch -s -p1 -d "$tmp" <"$p"; then
      report "mutant $p" 1 "patch does not apply" "$p"
      continue
    fi
    local killed=""
    mkdir -p "$tmp/build"
    cp "$dir/build/TraceData.tla" "$tmp/build/" 2>/dev/null
    tlc "$tmp" TraceCheck.tla TraceCheck.cfg "$tmp/trace.log" -DTLA-Library="$tmp/build"
    tlc_ok "$tmp/trace.log" || killed="trace"
    if [ -z "$killed" ]; then
      apa "$tmp" "$tmp/apa.log" --init=IndInit --inv=Inv --length=1
      apa_ok "$tmp/apa.log" || killed="apalache"
    fi
    if [ -z "$killed" ]; then
      tlc "$tmp" MC_Liveness.tla MC_Liveness.cfg "$tmp/live.log"
      tlc_ok "$tmp/live.log" || killed="liveness"
    fi
    local rc=1 name
    [ -n "$killed" ] && rc=0
    name="mutant $(basename "$p" .patch)"
    report "$name" $rc "killed by ${killed:-nothing}" "$p"
    rm -rf "$tmp"
  done
}

mkdir -p "$dir/build"
for c in "$@"; do
  case $c in
    safety | liveness | apalache | trace) "check_$c" "$dir" ;;
    mutants)
      [ -f "$dir/build/TraceData.tla" ] || check_trace "$dir"
      check_mutants
      ;;
    *) echo "unknown check $c" >&2; exit 2 ;;
  esac
done
exit $fail
