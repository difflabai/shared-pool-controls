#!/usr/bin/env bash
# TLA+ layer of shared-pool (pattern: near-approval strategy.md section 17,
# tools/scripts/run-tla.sh). Exit 0 iff every check has its required outcome.
#
# PROP: TLA-SAFE TLA-WIT TLA-CAP TLA-LIVE-1 TLA-LIVE-F1 TLA-LIVE-F2 WIN-LIVE REF-5 TLA-EXEC
#
#   safety     TLC: InvMC and StepProperty (TLA-SAFE) on
#                safety      MC_Safety.cfg (MCConfig, MaxRecords 2, VIEW)
#                cap         MC_Cap.cfg (Pool::new at VersionCap - 4, no VIEW;
#                            InvCapMC adds INV-VERCAP and WIN-1) (TLA-CAP)
#                recovery    MC_Recovery.cfg (RecoveryConfig)
#                f2b         MC_F2b.cfg (F2bConfig, values at +-1e6; F-2b)
#              then every witness must be VIOLATED (TLA-WIT, non-vacuity):
#              the Witness_* invariants and WitStep_* step properties on
#              MC_Safety.cfg, WitStep_Recovery on MC_Recovery.cfg,
#              Witness_EdgeValue and Witness_F2bRefused on MC_F2b.cfg,
#              Witness_AtCap and WitStep_ResolveAtCap on MC_Cap.cfg
#   safety-1   TLC, MC_Safety_1.cfg (MaxRecords 1): the smoke run (opt-in)
#   liveness   TLC, MC_Liveness (LIVE-2; the MC_Cap behaviours):
#                liveness           completion path, MCConfig (TLA-LIVE-1)
#                liveness-f2-done   completion path, CreditHedgeConfig (TLA-LIVE-1)
#                liveness-unfair    without fairness LIVE-2 must FAIL
#                liveness-f1        disturbing observes, executor fair to both
#                                   settles: holds (TLA-LIVE-F1; v1.0.0: F-1)
#                liveness-f2        healthy, executor fair to settle(false):
#                                   holds (TLA-LIVE-F2; v1.0.0: F-2)
#                liveness-win       WinLive, MaxRecords 1 (WIN-LIVE)
#                liveness-win2      WinLive, MaxRecords 2 (WIN-LIVE)
#   executor   TLA-EXEC (EX-6, EX-7, EX-8): TLC, Executor.tla, the executor's
#              job window against the lock's record window with receipts in
#              any order, at WINDOW 2 (MC_Executor_W2, as executor-w2) and 3
#              (MC_Executor_W3, as executor-w3); then every Witness_* and
#              WitStep_* of Executor.tla must be VIOLATED on MC_Executor_W2
#              without its VIEW (non-vacuity, and T-JOBCAP: the job cap
#              refuses a retained record's execute and confirmation)
#   executor-w2-fullview
#              opt-in, not part of executor (about 4 min): the cross-check
#              of executor-w2's dead-receipt quotient. Every invariant and
#              StepExec at WINDOW 2 with ids 0 .. 4, fingerprinted by
#              ViewFull (every variable but the ghost ev) instead of
#              ViewRefused
#   apalache   TLA-IND (Apalache): not implemented, prints "skip" (a gap)
#   trace      REF-5: trace2tla.py converts conformance/traces/core.txt and
#              TLC replays every trace through TraceCheck (TInv: Inv, or for
#              an init_error trace the model's refusal of the config, and
#              StepProperty on every core state too); the state count must
#              equal traces + calls
#   mutants    every mutants/*.patch must make one of its checks fail. A
#              patch may start with `# checks: <check>...` (names as in the
#              safety and liveness lists, or trace / safety-1 / executor-w2 /
#              executor-w3 / executor-w2-noview, the WINDOW 2 universe
#              without the VIEW, which the executor patches use); otherwise
#              the checks are trace, safety-1, cap, liveness-f1, liveness-f2,
#              tried in order until one fails. Each check a mutant names is
#              first run once on the unmutated model, and a check that fails
#              there kills nothing (reported as not killed); the executor
#              checks are exempt (no VIEW: the unmutated run does not
#              finish), `run.sh executor` is their baseline. The report
#              names the killing check and why (invariant, temporal,
#              rejected trace call, or a required violation that
#              disappeared).
#
# usage: projects/shared-pool/tla/run.sh [check...]  (default: safety liveness trace)
# env:   TLA_WORKERS (default 4, keep <= 4), TLA_HEAP (default 6g), TLA_LOG_DIR,
#        TLA_CACHE (as tools/scripts/get-tla-tools.sh), TLA_TRACES (default
#        conformance/traces/core.txt)
set -uo pipefail

dir=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$dir/../../.." && pwd)
. "$root/tools/versions.env"
# Reuse the machine-wide tool cache when it holds the pinned jar.
if [ -z "${TLA_CACHE:-}" ] && [ -f "$HOME/.cache/formalver-tools/tla2tools-$TLA2TOOLS.jar" ]; then
  export TLA_CACHE="$HOME/.cache/formalver-tools"
fi
cache=$("$root/tools/scripts/get-tla-tools.sh") || exit 2
jar="$cache/tla2tools-$TLA2TOOLS.jar"
traces=${TLA_TRACES:-$root/projects/shared-pool/conformance/traces/core.txt}
logs=${TLA_LOG_DIR:-$dir/build/logs}
mkdir -p "$logs" "$dir/build/cfg"
[ $# -gt 0 ] || set -- safety liveness trace

# tlc <spec dir> <module> <cfg> <log> [java -D options...]
tlc() {
  local d=$1 m=$2 c=$3 log=$4 tmp
  shift 4
  tmp=$(mktemp -d)
  (cd "$d" && java -XX:+UseParallelGC -Xmx"${TLA_HEAP:-6g}" -Djava.io.tmpdir="$tmp" "$@" \
    -cp "$jar" tlc2.TLC -workers "${TLA_WORKERS:-4}" -cleanup -metadir "$tmp/states" \
    -config "$c" "$m") >"$log" 2>&1
  rm -rf "$tmp"
  rm -f "$d"/*_TTrace_*
}
tlc_ok() { grep -q 'Model checking completed. No error has been found.' "$1"; }
states() { grep -oE '[0-9,]+ distinct states found' "$1" | tail -1; }
depth() { grep -oE 'depth of the complete state graph search is [0-9]+' "$1" | grep -oE '[0-9]+$'; }
live_violated() { grep -q 'Temporal properties were violated' "$1"; }
# Why a TLC run did not pass: the first violation it reports.
why() {
  local w t
  w=$(grep -oE 'Invariant [A-Za-z0-9_]+ is violated|Action property [A-Za-z0-9_]+ is violated|Temporal properties were violated|Deadlock reached|Parsing or semantic analysis failed|Assumption .* is false' "$1" | head -1)
  # TraceCheck: a deadlock is a rejected call. Name it: trace(N) of core.txt
  # (tr is 1-based) and the call's 1-based position (pos calls replayed).
  if [ "$w" = "Deadlock reached" ]; then
    t=$(awk '/^\/\\ tr = /{tr=$4} /^\/\\ pos = /{pos=$4} END{if (tr != "") printf "trace(%d) call %d", tr-1, pos+1}' "$1")
    w="rejected ${t:-a trace call}"
  fi
  echo "$w"
}

# Each run_<check> <spec dir> <log> returns 0 iff the check has its
# required outcome (shared by the checks and the mutant runner).
# The state count must equal traces + calls: every trace's initial state
# and one state per replayed call.
trace_states() { grep -oE '^[0-9]+ states generated, [0-9]+ distinct states found, 0 states left' "$1" |
  sed -E 's/.* ([0-9]+) distinct.*/\1/'; }
trace_want() { echo $(($(grep -c '^trace(' "$traces") + $(grep -c '^call(' "$traces"))); }
run_trace() {
  tlc "$1" TraceCheck.tla TraceCheck.cfg "$2" -DTLA-Library="$1/build"
  tlc_ok "$2" && [ "$(trace_states "$2")" = "$(trace_want)" ]
}
run_safety() {
  tlc "$1" MC_Safety.tla MC_Safety.cfg "$2"
  tlc_ok "$2"
}
run_safety-1() {
  tlc "$1" MC_Safety.tla MC_Safety_1.cfg "$2"
  tlc_ok "$2"
}
run_cap() {
  tlc "$1" MC_Cap.tla MC_Cap.cfg "$2"
  tlc_ok "$2"
}
run_recovery() {
  tlc "$1" MC_Safety.tla MC_Recovery.cfg "$2"
  tlc_ok "$2"
}
run_f2b() {
  tlc "$1" MC_Safety.tla MC_F2b.cfg "$2"
  tlc_ok "$2"
}
run_liveness() {
  tlc "$1" MC_Liveness.tla MC_Liveness.cfg "$2"
  tlc_ok "$2"
}
run_liveness-f2-done() {
  tlc "$1" MC_Liveness.tla MC_Liveness_F2Done.cfg "$2"
  tlc_ok "$2"
}
run_liveness-unfair() {
  tlc "$1" MC_Liveness.tla MC_Liveness_Unfair.cfg "$2"
  live_violated "$2"
}
# v1.0.0: violated by F-1 / F-2. v1.1.0: must hold (LIVE-ALL).
run_liveness-f1() {
  tlc "$1" MC_Liveness.tla MC_Liveness_F1.cfg "$2"
  tlc_ok "$2"
}
run_liveness-f2() {
  tlc "$1" MC_Liveness.tla MC_Liveness_F2.cfg "$2"
  tlc_ok "$2"
}
run_liveness-win() {
  tlc "$1" MC_Liveness.tla MC_Liveness_Win.cfg "$2"
  tlc_ok "$2"
}
run_liveness-win2() {
  tlc "$1" MC_Liveness.tla MC_Liveness_Win2.cfg "$2"
  tlc_ok "$2"
}
# TLA-EXEC: every invariant and StepExec of Executor.tla.
run_executor-w2() {
  tlc "$1" Executor.tla MC_Executor_W2.cfg "$2"
  tlc_ok "$2"
}
run_executor-w3() {
  tlc "$1" Executor.tla MC_Executor_W3.cfg "$2"
  tlc_ok "$2"
}
# The WINDOW 2 universe without the VIEW. The VIEW is a quotient of the
# unmutated model only (a mutant may change what a dead receipt does), so
# the executor mutants run here.
run_executor-w2-noview() {
  mkdir -p "$1/build/cfg"
  sed '/^VIEW /d' "$1/MC_Executor_W2.cfg" >"$1/build/cfg/MC_Executor_W2_noview.cfg"
  tlc "$1" Executor.tla build/cfg/MC_Executor_W2_noview.cfg "$2"
  tlc_ok "$2"
}
# MC_Executor_W2 with ids 0 .. 4 and VIEW ViewFull. ViewFull drops only ev,
# which no action and no state invariant reads, so this run needs no
# quotient argument; it must agree with executor-w2's verdict.
run_executor-w2-fullview() {
  local c=build/cfg/MC_Executor_W2_fullview.cfg
  mkdir -p "$1/build/cfg"
  {
    echo '\* Generated by run.sh (executor-w2-fullview) from MC_Executor_W2.cfg.'
    sed -e '/^\\\*/d' -e 's/^VIEW .*/VIEW ViewFull/' -e 's/^\( *NIds *=\).*/\1 4/' \
      -e 's/^\( *MaxId *=\).*/\1 4/' "$1/MC_Executor_W2.cfg"
  } >"$1/$c"
  # Fail if the edit did not take (a reformatted MC_Executor_W2.cfg).
  [ "$(grep -cE '^(VIEW ViewFull| *NIds *= 4| *MaxId *= 4)$' "$1/$c")" = 3 ] || {
    echo "MC_Executor_W2.cfg: could not set VIEW ViewFull, NIds 4, MaxId 4" >"$2"
    return 1
  }
  tlc "$1" Executor.tla "$c" "$2"
  tlc_ok "$2"
}

fail=0
report() { # name ok(0/1) detail log
  if [ "$2" = 0 ]; then
    printf '%-36s ok    %s\n' "$1" "$3"
  else
    printf '%-36s FAIL  %s (log %s)\n' "$1" "$3" "$4"
    fail=1
  fi
}

# Witnesses as name:module:cfg (TLA-WIT).
WITNESSES="Witness_Completed:MC_Safety:MC_Safety Witness_Prerequisite:MC_Safety:MC_Safety
  Witness_Disturbed:MC_Safety:MC_Safety Witness_Prune:MC_Safety:MC_Safety
  Witness_FullOpenFront:MC_Safety:MC_Safety Witness_CancelCompletedInvalid:MC_Safety:MC_Safety
  Witness_SubmitRejectedHull:MC_Safety:MC_Safety
  WitStep_CancelledAfterDispatch:MC_Safety:MC_Safety WitStep_UnknownToActive:MC_Safety:MC_Safety
  WitStep_Refresh:MC_Safety:MC_Safety WitStep_PrereqAfterPrune:MC_Safety:MC_Safety
  WitStep_PausedSettle:MC_Safety:MC_Safety WitStep_PausedAck:MC_Safety:MC_Safety
  WitStep_PausedCancel:MC_Safety:MC_Safety WitStep_PausedObserve:MC_Safety:MC_Safety
  WitStep_Recovery:MC_Safety:MC_Recovery
  Witness_EdgeValue:MC_Safety:MC_F2b Witness_F2bRefused:MC_Safety:MC_F2b
  Witness_AtCap:MC_Cap:MC_Cap WitStep_ResolveAtCap:MC_Cap:MC_Cap"

# run_witness <spec dir> <log> <witness>: 0 iff the witness is violated
# (reachable) in its universe (WITNESSES).
run_witness() {
  local d=$1 log=$2 w m c spec
  spec=$(echo $WITNESSES | tr ' ' '\n' | grep "^$3:")
  IFS=: read -r w m c <<<"$spec"
  [ -n "$c" ] || { echo "unknown witness $3" >"$log"; return 1; }
  mkdir -p "$d/build/cfg"
  {
    sed -e '/^INVARIANT /d' -e '/^PROPERTY /d' "$d/$c.cfg"
    case $w in
      Witness_*) echo "INVARIANT $w" ;;
      *) echo "PROPERTY $w" ;;
    esac
  } >"$d/build/cfg/$w.cfg"
  tlc "$d" "$m.tla" "build/cfg/$w.cfg" "$log"
  grep -qE "(Invariant|Action property) $w is violated" "$log"
}
# A mutant or baseline check: a run_<check> function, or a witness name
# (the mutant kills it if the witness becomes unreachable).
run_check() { # dir log check
  case $3 in
    Witness_* | WitStep_*) run_witness "$1" "$2" "$3" ;;
    *) "run_$3" "$1" "$2" ;;
  esac
}

check_witness() { # spec dir, name:module:cfg
  local w=${2%%:*} c=${2##*:}
  run_witness "$1" "$logs/$w.log" "$w"
  report "$w" $? "must be reachable ($c)" "$logs/$w.log"
}

check_safety() {
  local d=$1 c log start w
  for c in safety cap recovery f2b; do
    log="$logs/$c.log"
    start=$(date +%s)
    "run_$c" "$d" "$log"
    report "$c" $? "$(desc "$c"), $(states "$log"), depth $(depth "$log"), $(($(date +%s) - start))s" "$log"
  done
  for w in $WITNESSES; do
    check_witness "$d" "$w"
  done
}

check_safety-1() {
  local d=$1 log="$logs/safety-1.log" start
  start=$(date +%s)
  run_safety-1 "$d" "$log"
  report safety-1 $? "$(desc safety-1), $(states "$log"), depth $(depth "$log"), $(($(date +%s) - start))s" "$log"
}

check_liveness() {
  local d=$1 c log start
  for c in liveness liveness-f2-done liveness-unfair liveness-f1 liveness-f2 liveness-win \
    liveness-win2; do
    log="$logs/$c.log"
    start=$(date +%s)
    "run_$c" "$d" "$log"
    report "$c" $? "$(desc "$c"), $(states "$log"), $(($(date +%s) - start))s" "$log"
  done
}
desc() {
  case $1 in
    safety) echo "InvMC + StepProperty, MCConfig, MaxRecords 2 (TLA-SAFE)" ;;
    safety-1) echo "InvMC + StepProperty, MCConfig, MaxRecords 1 (smoke)" ;;
    cap) echo "InvCapMC + StepProperty from VersionCap - 4 (TLA-CAP)" ;;
    recovery) echo "InvMC + StepProperty, RecoveryConfig" ;;
    f2b) echo "InvMC + StepProperty, F2bConfig (F-2b)" ;;
    liveness) echo "LIVE-2, completion path, MCConfig (TLA-LIVE-1)" ;;
    liveness-f2-done) echo "LIVE-2, completion path, CreditHedgeConfig (TLA-LIVE-1)" ;;
    liveness-unfair) echo "LIVE-2 must fail without fairness" ;;
    liveness-f1) echo "LIVE-2 holds with disturbing observes, fair settles (TLA-LIVE-F1)" ;;
    liveness-f2) echo "LIVE-2 holds, healthy, fair settle(false)/cancel (TLA-LIVE-F2)" ;;
    liveness-win) echo "WinLive, MaxRecords 1, fair resolution of the front (WIN-LIVE)" ;;
    liveness-win2) echo "WinLive, MaxRecords 2, fair resolution of the front (WIN-LIVE)" ;;
    executor-w2) echo "TLA-EXEC, WINDOW 2, MAX_JOBS 4" ;;
    executor-w3) echo "TLA-EXEC, WINDOW 3, MAX_JOBS 6" ;;
    executor-w2-fullview) echo "TLA-EXEC quotient cross-check, WINDOW 2, ids 0 .. 4, ViewFull" ;;
  esac
}

EXEC_STATE_WITNESSES="Witness_Pruned Witness_Cap Witness_JobCapExec Witness_JobCapConfirm
  Witness_ConfirmStale"
EXEC_STEP_WITNESSES="WitStep_ExecStale WitStep_ExecStored WitStep_ExecFull WitStep_ExecForgets
  WitStep_SettleForgets WitStep_SettleRefused WitStep_CapExit"

# TLA-EXEC at WINDOW 2 and 3, then its witnesses (non-vacuity, including
# the documented job-cap refusals T-JOBCAP) on the WINDOW 2 universe,
# without the VIEW: it may expand only one state of a view, and the one it
# keeps need not be a witness's source (a stale execute's state shares its
# view with one where the record was cancelled before dispatch).
check_executor() {
  local d=$1 c w log start
  for c in executor-w2 executor-w3; do
    log="$logs/$c.log"
    start=$(date +%s)
    "run_$c" "$d" "$log"
    report "$c" $? "$(desc "$c"), $(states "$log"), depth $(depth "$log"), $(($(date +%s) - start))s" "$log"
  done
  for w in $EXEC_STATE_WITNESSES $EXEC_STEP_WITNESSES; do
    log="$logs/$w.log"
    {
      sed -e '/^INVARIANT /d' -e '/^PROPERTY /d' -e '/^VIEW /d' "$d/MC_Executor_W2.cfg"
      case $w in
        Witness_*) echo "INVARIANT $w" ;;
        *) echo "PROPERTY $w" ;;
      esac
    } >"$d/build/cfg/$w.cfg"
    tlc "$d" Executor.tla "build/cfg/$w.cfg" "$log"
    grep -qE "(Invariant|Action property) $w is violated" "$log"
    report "$w" $? "must be reachable" "$log"
  done
}

# Opt-in: executor-w2's quotient against ViewFull (run_executor-w2-fullview).
check_executor-w2-fullview() {
  local d=$1 c=executor-w2-fullview log start
  log="$logs/$c.log"
  start=$(date +%s)
  "run_$c" "$d" "$log"
  report "$c" $? "$(desc "$c"), $(states "$log"), depth $(depth "$log"), $(($(date +%s) - start))s" "$log"
}

gen_traces() { # out dir
  python3 -I "$dir/trace2tla.py" "$traces" "$1/TraceData.tla" 2>"$logs/trace2tla.log"
}

check_trace() {
  local d=$1 log="$logs/trace-tlc.log" ntr ncalls want got start r
  gen_traces "$d/build"
  report trace2tla $? "$(cat "$logs/trace2tla.log")" "$logs/trace2tla.log"
  ntr=$(grep -c '^trace(' "$traces")
  ncalls=$(grep -c '^call(' "$traces")
  want=$(trace_want)
  start=$(date +%s)
  run_trace "$d" "$log"
  r=$?
  got=$(trace_states "$log")
  report trace-tlc $r "$ntr traces, $ncalls calls: $got of $want states, $(($(date +%s) - start))s" "$log"
}

# TLA-IND is optional and not implemented: Apalache 0.47.2 rejects the
# model's RECURSIVE sums and symbolic ranges (RESULTS.md, "Not covered").
# Reported as skipped; it does not change the exit code.
check_apalache() {
  printf '%-36s skip  %s\n' apalache "TLA-IND not implemented (gap, see RESULTS.md)"
}

# The unmutated model's verdict for each check the mutants name: a check
# that fails here (e.g. trace before the corpus matches the model) kills
# nothing.
DEFAULT_MUTANT_CHECKS="trace safety-1 cap liveness-f1 liveness-f2"
base_bad=" "
mutant_baseline() {
  local p c all="" tmp
  for p in "$dir"/mutants/*.patch; do
    c=$(sed -n 's/^# checks: //p' "$p" | head -1)
    all="$all ${c:-$DEFAULT_MUTANT_CHECKS}"
  done
  tmp=$(mktemp -d)
  cp "$dir"/*.tla "$dir"/*.cfg "$dir"/trace2tla.py "$tmp/"
  mkdir -p "$tmp/build"
  cp "$dir/build/TraceData.tla" "$tmp/build/" 2>/dev/null
  for c in $(echo "$all" | tr ' ' '\n' | sort -u); do
    # The executor mutants' universe has no VIEW and does not finish on the
    # unmutated model; its baseline is `run.sh executor` (run it first).
    case $c in executor-*) continue ;; esac
    if ! run_check "$tmp" "$logs/baseline-$c.log" "$c"; then
      base_bad="$base_bad$c "
      report "baseline $c" 1 "fails on the unmutated model: kills nothing" "$logs/baseline-$c.log"
    fi
  done
  rm -rf "$tmp"
}

check_mutants() {
  local p tmp killed checks c start r
  mutant_baseline
  for p in "$dir"/mutants/*.patch; do
    start=$(date +%s)
    tmp=$(mktemp -d)
    cp "$dir"/*.tla "$dir"/*.cfg "$dir"/trace2tla.py "$tmp/"
    mkdir -p "$tmp/build"
    if ! patch -s -p1 -d "$tmp" <"$p"; then
      report "mutant $(basename "$p" .patch)" 1 "patch does not apply" "$p"
      rm -rf "$tmp"
      continue
    fi
    cp "$dir/build/TraceData.tla" "$tmp/build/" 2>/dev/null
    checks=$(sed -n 's/^# checks: //p' "$p" | head -1)
    [ -n "$checks" ] || checks=$DEFAULT_MUTANT_CHECKS
    killed=""
    for c in $checks; do
      case $base_bad in *" $c "*) continue ;; esac
      if ! run_check "$tmp" "$tmp/$c.log" "$c"; then
        case $c in
          liveness-unfair)
            r=$(why "$tmp/$c.log")
            killed="$c (required LIVE-2 violation gone: ${r:-no error})"
            ;;
          Witness_* | WitStep_*)
            r=$(why "$tmp/$c.log")
            killed="$c (no longer reachable${r:+: $r})"
            ;;
          trace)
            r=$(why "$tmp/$c.log")
            killed="$c (${r:-$(trace_states "$tmp/$c.log") of $(trace_want) states})"
            ;;
          *) killed="$c ($(why "$tmp/$c.log"))" ;;
        esac
        break
      fi
    done
    report "mutant $(basename "$p" .patch)" "$([ -n "$killed" ] && echo 0 || echo 1)" \
      "killed by ${killed:-nothing (tried: $checks)}, $(($(date +%s) - start))s" "$p"
    rm -rf "$tmp"
  done
}

for c in "$@"; do
  case $c in
    safety | safety-1 | liveness | apalache | trace | executor | executor-w2-fullview) "check_$c" "$dir" ;;
    mutants)
      # A failed conversion leaves no TraceData.tla: the trace baseline
      # then fails and trace kills nothing.
      rm -f "$dir/build/TraceData.tla"
      gen_traces "$dir/build" || report trace2tla 1 "conversion failed" "$logs/trace2tla.log"
      check_mutants
      ;;
    *) echo "unknown check $c" >&2; exit 2 ;;
  esac
done
exit $fail
