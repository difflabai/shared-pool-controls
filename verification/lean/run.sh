#!/usr/bin/env bash
# Lean layer for shared-pool (strategy.md section 18): the v1.1.0 core
# transcribed to Lean 4 (PoolLean/Machine.lean), with Inv (INV-RES and
# INV-VERCAP included), StepProps, LIVE-ALL and LIVE-1 proved for states of
# every size, the bounded window (WIN-*) and the F-1..F-6 witnesses.
#
#   build    lake build; no `sorry` warning, and the hygiene scan finds no
#            sorry, admit, axiom, native_decide, implemented_by, extern,
#            unsafe or opaque in the sources (comments stripped)
#   audit    LEAN-*: the headline theorems (Audit.lean) depend only on
#            propext, Classical.choice and Quot.sound
#   canary   canary/*.lean must each FAIL to compile
#   regression
#            regression/*.lean must each COMPILE: statements that were
#            canaries on v1.0.0 (false there) and are theorems on v1.1.0
#   replay   REF-5L: every trace of conformance/traces/core.txt (generated
#            from the real core) replayed on the Lean machine, every result
#            and post-state equal
#   mutants  every mutants/*.patch must be killed: by replay (the machine
#            no longer matches the code) and/or by build (a proof breaks);
#            both are tried and reported. Replay kills count only when the
#            unmutated machine replays the corpus (checked first); otherwise
#            they are reported as not meaningful and build alone decides.
#            The replay executable `pool-lean` (Main.lean) imports only
#            Machine and Text, so a mutant that breaks a proof module still
#            replays; if `pool-lean` itself does not build, the mutant is
#            reported "no replay (pool-lean does not build)", never as a
#            replay kill.
#            A fix-revert mutant with a property probe
#            (mutants/probes/<name>.lean) is also killed by "property" when
#            the probe compiles on the mutant; every probe must fail to
#            compile on the unmutated machine
#
# usage: projects/shared-pool/lean/run.sh [check...]   (default: all)
# env:   LEAN_LOG_DIR (default: a temp dir); LEAN_CACHE (where
#        tools/scripts/get-lean.sh puts the toolchain when lake is not on
#        PATH; default $HOME/.cache/formalver-tools)
set -uo pipefail

lean=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$lean/../../.." && pwd)
traces="$lean/../conformance/traces/core.txt"
logs=${LEAN_LOG_DIR:-$(mktemp -d)}
mkdir -p "$logs"
[ $# -gt 0 ] || set -- build audit canary regression replay mutants
fail=0

if ! command -v lake >/dev/null; then
  bin=$(LEAN_CACHE=${LEAN_CACHE:-$HOME/.cache/formalver-tools} "$root/tools/scripts/get-lean.sh") ||
    { echo "FAIL setup: no Lean toolchain"; exit 1; }
  export PATH="$bin:$PATH"
fi

report() { # name status message log
  if [ "$2" -eq 0 ]; then
    echo "PASS $1: $3"
  else
    echo "FAIL $1: $3 (log $4)"
    tail -30 "$4" 2>/dev/null | sed 's/^/  /'
    fail=1
  fi
}

# Forbidden in the sources. Comments (`--` lines and nested `/- -/`
# blocks, docstrings included) are stripped before matching.
scan() { # dir-or-file...
  python3 - "$@" <<'PY'
import pathlib, re, sys
bad = re.compile(r"\b(sorry|admit|axiom|native_decide|implemented_by|extern|unsafe|opaque)\b")
files = []
for a in sys.argv[1:]:
    p = pathlib.Path(a)
    files += sorted(p.rglob("*.lean")) if p.is_dir() else [p]
for f in files:
    src, out, depth, i = f.read_text(), [], 0, 0
    while i < len(src):
        if src.startswith("/-", i):
            depth += 1; i += 2
        elif depth and src.startswith("-/", i):
            depth -= 1; i += 2
        elif not depth and src.startswith("--", i):
            j = src.find("\n", i)
            i = len(src) if j < 0 else j
        else:
            out.append(src[i] if not depth or src[i] == "\n" else " "); i += 1
    for n, line in enumerate("".join(out).split("\n"), 1):
        if bad.search(line):
            print(f"{f}:{n}: {line.strip()}")
PY
}

check_build() {
  local log="$logs/build.log"
  (cd "$lean" && lake build) >"$log" 2>&1
  local rc=$?
  if [ $rc -eq 0 ] && grep -q "declaration uses 'sorry'" "$log"; then rc=1; fi
  report build $rc "every theorem proved, no sorry" "$log"
  local hits="$logs/hygiene.log"
  scan "$lean/PoolLean" "$lean/PoolLean.lean" "$lean/Main.lean" "$lean/Guards.lean" "$lean/Audit.lean" "$lean/canary" "$lean/regression" >"$hits"
  [ ! -s "$hits" ]; report hygiene $? "no forbidden constructs in the sources" "$hits"
}

check_audit() {
  local log="$logs/audit.log"
  (cd "$lean" && lake env lean Audit.lean) >"$log" 2>&1
  local rc=$? n want
  want=$(grep -c '^#print axioms' "$lean/Audit.lean")
  n=$(grep -cE "depends on axioms: \[(propext|Classical\.choice|Quot\.sound)(, (propext|Classical\.choice|Quot\.sound))*\]|does not depend on any axioms" "$log")
  [ $rc -eq 0 ] && [ "$n" -eq "$want" ]
  report audit $? "$n/$want theorems use only propext, Classical.choice, Quot.sound" "$log"
}

check_canary() {
  local f log
  for f in "$lean"/canary/*.lean; do
    log="$logs/canary-$(basename "$f" .lean).log"
    (cd "$lean" && lake env lean "$f") >"$log" 2>&1
    [ $? -ne 0 ] && grep -q 'error' "$log"
    report "canary $(basename "$f")" $? "fails to compile, as it must" "$log"
  done
}

check_regression() {
  local f log
  for f in "$lean"/regression/*.lean; do
    log="$logs/regression-$(basename "$f" .lean).log"
    (cd "$lean" && lake env lean "$f") >"$log" 2>&1
    [ $? -eq 0 ] && ! grep -qE "error|declaration uses 'sorry'" "$log"
    report "regression $(basename "$f")" $? "compiles, as it must" "$log"
  done
}

check_replay() {
  local log="$logs/replay.log"
  { (cd "$lean" && lake build pool-lean) && (cd "$lean" && lake exe pool-lean replay "$traces"); } >"$log" 2>&1
  report replay $? "$(tail -1 "$log")" "$log"
}

check_mutants() {
  local m name tmp log how note survived=0 base=1
  if ! { (cd "$lean" && lake build pool-lean) && (cd "$lean" && lake exe pool-lean replay "$traces"); } >"$logs/mutant-baseline.log" 2>&1; then
    base=0
    echo "NOTE mutants: the unmutated machine does not replay $traces; replay kills are not meaningful, build decides (log $logs/mutant-baseline.log)"
  fi
  local p
  for p in "$lean"/mutants/probes/*.lean; do
    [ -e "$p" ] || continue
    log="$logs/probe-$(basename "$p" .lean).log"
    ! (cd "$lean" && lake env lean "$p") >"$log" 2>&1
    report "probe $(basename "$p" .lean)" $? "fails on the unmutated machine, as it must" "$log"
  done
  for m in "$lean"/mutants/*.patch; do
    name=$(basename "$m" .patch)
    log="$logs/mutant-$name.log"
    tmp=$(mktemp -d)
    (cd "$lean" && tar cf - --exclude=mutants .) | (cd "$tmp" && tar xf -)
    if ! (cd "$tmp" && patch -s -p1 <"$m") >"$log" 2>&1; then
      report "mutant $name" 1 "patch does not apply" "$log"
      rm -rf "$tmp"
      continue
    fi
    how="" note=""
    if [ $base -eq 1 ]; then
      if ! (cd "$tmp" && lake build pool-lean) >>"$log" 2>&1; then
        note="; no replay (pool-lean does not build)"
      elif ! (cd "$tmp" && lake exe pool-lean replay "$traces") >>"$log" 2>&1; then
        how="replay"
      fi
    fi
    if ! (cd "$tmp" && lake build) >"$log.build" 2>&1 || grep -q "declaration uses 'sorry'" "$log.build"; then
      how="${how:+$how, }build ($(grep -oE '^error: [A-Za-z/]+\.lean' "$log.build" | sed 's/^error: //' | sort -u | tr '\n' ' ' | sed 's/ $//'))"
    fi
    cat "$log.build" >>"$log"; rm -f "$log.build"
    p="$lean/mutants/probes/$name.lean"
    if [ -e "$p" ] && (cd "$tmp" && lake env lean "$p") >>"$log" 2>&1; then
      how="${how:+$how, }property ($(sed -n 's/^-- \([A-Z][A-Z0-9-]*\).*/\1/p' "$p" | head -1) probe)"
    fi
    if [ -z "$how" ]; then
      survived=1
      report "mutant $name" 1 "survived$note" "$log"
    else
      echo "PASS mutant $name: killed by $how$note"
    fi
    rm -rf "$tmp"
  done
  [ $survived -eq 0 ] || fail=1
}

for c in "$@"; do
  case $c in
    build | audit | canary | regression | replay | mutants)
      t0=$(date +%s)
      "check_$c"
      echo "  ($c: $(($(date +%s) - t0)) s)"
      ;;
    *) echo "unknown check $c" >&2; exit 2 ;;
  esac
done
exit $fail
