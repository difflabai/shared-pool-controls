#!/usr/bin/env bash
# Kani layer runner for shared-pool (pool-core at the pinned source-mirror).
#
# usage: run.sh plain               plain mode: every harness body with the
#                                   random source (cargo test --release).
#                                   A test, not a proof.
#        run.sh kani [h...]         Kani, one harness at a time (all by
#                                   default): every harness verifies with
#                                   unwinding assertions on, h_canary FAILS on
#                                   its own assertion only, and every cover is
#                                   SATISFIED in some harness of its family.
#        run.sh mutants [--kani] [glob]
#                                   applies each mutants/*.patch to a temporary
#                                   copy of crates/core and requires plain mode
#                                   to fail (and with --kani, its "killed-by"
#                                   harnesses to FAIL; CBMC stops at the
#                                   first failed check). A "# must-survive"
#                                   patch is an equivalent mutant: plain mode
#                                   (and with --kani its "checked-by"
#                                   harnesses) must PASS on it.
#        run.sh ref                 Kani, the h_ref_* harnesses (REF-1..3,
#                                   tla.rs) one at a time; their covers (the
#                                   "ref ..." family) all SATISFIED.
#        run.sh all                 plain, kani, mutants.
#        run.sh list                the harnesses `kani` runs by default, one
#                                   per line (CI checks its groups cover them).
#
# A mutant patch applies to crates/core (paths a/crates/core/...) unless it
# has "# applies-to: kani", then to this crate (paths a/src/..., e.g. a
# mutant of the TLA+ transcription src/tla.rs). The mutant tree also has
# conformance/traces/core.txt (plain mode replays it, conform.rs).
#
# env: KANI (default: cargo kani), KANI_TIMEOUT seconds per harness (default
#      1800; the h_step_*_inv_3 parts 7200 unless it is set), KANI_MAX_GB
#      resident GB of CBMC before a harness is stopped
#      (default 40; reported as OUT-OF-MEMORY), KANI_LOG_DIR (default:
#      target/kani-logs), KEEP_LOGS=dir. KANI_DEFER_COVERS=1: `kani` writes
#      KANI_LOG_DIR/covers.tsv (family, cover, status) but leaves the family
#      check to tools/scripts/check-covers.sh over every group's file (CI
#      runs a family's harnesses in separate jobs).
# Exit 0 iff every check passes.
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

here=$(cd "$(dirname "$0")" && pwd)
proj=$(cd "$here/.." && pwd)
kani=${KANI:-cargo kani}
# The core is #![forbid(unsafe_code)]; the memory safety of std internals is
# trusted (ASM-TOOLS). Panic, overflow, bounds and unwinding checks stay on.
# Reachability checks of the assertions are off (they cost one SAT round per
# assertion); vacuity is covered by the covers instead (RESULTS.md).
# -Z stubbing: the fixed-capacity Vec::with_capacity_in and Vec::reserve
# models (src/model.rs).
flags=${KANI_FLAGS:---no-memory-safety-checks --no-assertion-reach-checks -Z stubbing}
timeout_s=${KANI_TIMEOUT:-1800}
# harness_timeout <h>: the per-harness limit. The INV parts from 3 records
# are the largest SAT calls (h_step_settle_inv_3: one call of 1712 s).
harness_timeout() {
  case "$1" in
    # h_full_256_prune: at the _inv_3 level until measured (W11).
    h_step_*_inv_3 | h_full_256_prune) echo "${KANI_TIMEOUT:-7200}" ;;
    *) echo "$timeout_s" ;;
  esac
}
max_gb=${KANI_MAX_GB:-40}
mod=${KANI_MOD:-proofs}

# kill_tree <pid>: the process and all its descendants (children first are
# collected before anything is killed, so none is reparented and missed).
kill_tree() {
  local c
  for c in $(pgrep -P "$1" 2>/dev/null); do kill_tree "$c"; done
  kill -KILL "$1" 2>/dev/null
}

# tree_rss_kb <pid>: resident KB of the process and its descendants.
tree_rss_kb() {
  local c s
  s=$(ps -o rss= -p "$1" 2>/dev/null | tr -d ' ')
  s=${s:-0}
  for c in $(pgrep -P "$1" 2>/dev/null); do s=$(( s + $(tree_rss_kb "$c") )); done
  echo "$s"
}

# run_timeout <seconds> cmd...: exit status of cmd, 124 on timeout, 125 when
# the tree's memory passes KANI_MAX_GB. Not timeout(1): cargo-kani starts
# CBMC in its own process group, so a timeout that signals only the child
# (or its group) leaves CBMC running, orphaned.
run_timeout() {
  local t=$1; shift
  "$@" &
  local pid=$! start
  start=$(date +%s)
  while kill -0 "$pid" 2>/dev/null; do
    if [ $(( $(date +%s) - start )) -ge "$t" ]; then
      kill_tree "$pid"; wait "$pid" 2>/dev/null; return 124
    fi
    if [ "$(tree_rss_kb "$pid")" -ge $(( max_gb * 1048576 )) ]; then
      kill_tree "$pid"; wait "$pid" 2>/dev/null; return 125
    fi
    sleep 2
  done
  wait "$pid"
}

# Per-loop unwinding (RESULTS.md, cost ladder). The core's fallible
# `collect`s merge their error exit into the result's length, so CBMC cannot
# fold the trip count of a loop over values, deltas or terms and unrolls it
# to the harness's bound (5-7, set by the record loops). Those loops have at
# most 2 elements here, so they get bound 3; unwinding assertions stay on, so
# a bound that is too small fails the harness, it never hides a path. The
# loop ids come from the harness's goto binary (cbmc --show-loops).
cbmc_bin=${KANI_CBMC:-$(ls -d "$HOME"/.kani/kani-*/bin/cbmc 2>/dev/null | tail -1)}
short_loops='extend_desugared|Iter<.*, (i64|u64|pool_core::Term|[(]u32, u32[)])>|Zip<|^pool_core::Pool::envelope::\{closure|^pool_core::Pool::check_bounds$|drop_glue::<[[]pool_core::(Recipe|Metric)[]]>'

# short_bound <harness>: 3; 4 for the raw configurations (vectors of up to
# 3 values); nothing for h_energy_max (64 terms). The record loops of
# h_full_* (256 records) are not short loops. Loops over requirement pairs
# `(u32, u32)` (at most 2 everywhere) get at most 3.
short_bound() {
  case $1 in
    h_energy_max) ;;
    h_init | h_validate) echo 4 ;;
    *) echo 3 ;;
  esac
}
pair_loops='Iter<.*, [(]u32, u32[)]>'
# The byte loops of `spec::Text::<24>::of` and `spec::parse_dec` (preview's
# numbers) and `spec::Text::<16>` (preview's reason) run 24, 20 and 16
# times and need bound 25 (enough for all three numeral loops) and 17,
# whatever the harness's bound.
long_loops='^spec::(Text::<24>::of|parse_dec)$'
text_loops='^spec::Text::<16>::(of|eq)$'
# `spec::eq_i64s` compares M + 2 = 4 elements (raw vectors in plain mode
# have up to 3), a concrete loop of 4 iterations: bound 5, above the unwind
# of h_init and h_validate (4).
vec_loops='^spec::eq_i64s$'
# The loop over an observed vector's values in `observe` (`any(!bounded)`)
# runs after the length check, so on vectors of length n <= 2; mutant 20
# (the length check dropped, an equivalent mutant) runs it on the observe
# domain's vectors of up to 3 values: bound 4, so that the must-survive
# check is a proof and not an unwinding failure.
obs_loops='^<std::slice::Iter<.*i64> as std::iter::Iterator>::any::<[{]closure@pool_core::Pool::observe::'
# `best_quantity`'s two loops over `1..=units` (`h_best`: units <= 7 under
# Kani): bound 8.
range_loops='^<std::ops::RangeInclusive<u32> as '
# In h_energy_max (64 terms, no short bound): the loop
# under each `next()` of `energy_terms`' fallible collect exits in its first
# iteration (the shunt breaks on every item), but CBMC does not fold it, and
# at the harness bound it is unrolled 66 times inside the 66 of the collect:
# bound 2.
shunt_loops='^<std::slice::Iter<.*pool_core::Term> as std::iter::Iterator>::try_fold::<[(][)], [{]closure@std::iter::adapters::map::map_try_fold'

# `gen::copy_vec`'s loop over values, deltas, requirements, metrics, terms
# or completed units, the slice equality over requirements and terms (`==`
# in `spec::same_config`) and the spec's loops over a recipe's requirements
# (`spec::prereqs_met`, `spec::prereqs_met_scan`) and over completed units
# (`spec::cu_at`; at most 3 elements under Kani), in the harnesses with a
# large unwind (h_full_*: 258, for the records; h_preview_post: 24): bound
# 4. CBMC does not fold those vectors' lengths: at 258 each copy was
# unrolled about 253 times, and in v1.0 `prereqs_met` ran its scan of the
# 255 records once per unrolled requirement.
copy_loops='^gen::copy_vec::<(i64|u32|[(]u32, u32[)]|pool_core::Metric|pool_core::Term)>$|^<([(]u32, u32[)]|pool_core::Term) as core::slice::cmp::SlicePartialEq<|^spec::(prereqs_met|prereqs_met_scan|cu_at)$'
copy_bound() {
  case $1 in
    h_full_* | h_preview_post) echo 4 ;;
  esac
}

# extra_cbmc <harness>: further CBMC options. h_full_255 reads its 255
# records back from the heap (the spec's and the core's scans): the Vec's
# buffer is one byte array of 257 * 24 bytes, beyond CBMC's field
# sensitivity limit (64), so no read folded to the record's constant
# fields, every record took the `open` branch of both envelopes (a
# multiplication each), and symbolic execution was still in the core's
# envelope after 13 minutes. With the limit above the buffer's size the
# reads are constants (cost ladder). h_full_256 needs it only on mutant 21
# (`Full` one record later: the mutant goes on to the scans), but gets it
# too, so that its result and the mutant's run use one encoding.
extra_cbmc() {
  case $1 in
    h_full_*) echo "--max-field-sensitivity-array-size 8192" ;;
  esac
}

# unwindset <target-dir> <harness>: "--cbmc-args --unwindset id:3,..." or "".
unwindset() {
  local b f ids cb
  b=$(short_bound "$2")
  cb=$(copy_bound "$2")
  [ -x "$cbmc_bin" ] || return 0
  f=$(ls -t "$1"/kani/*/debug/build/shared-pool-kani/*/out/*${#mod}${mod}${#2}"$2".out 2>/dev/null | head -1)
  [ -n "$f" ] || return 0
  ids=$("$cbmc_bin" --show-loops "$f" 2>/dev/null | awk -v ore="$obs_loops" -v re="$short_loops" -v pre="$pair_loops" -v lre="$long_loops" -v rre="$range_loops" -v sre="$shunt_loops" -v tre="$text_loops" -v vre="$vec_loops" -v cre="$copy_loops" -v cb="$cb" -v b="$b" '
    /^Loop / { id = $2; sub(/:$/, "", id); next }
    id != "" { fn = $0; sub(/.* function /, "", fn); k = ""
               if (b == "") { if (fn ~ sre) k = 2 }
               else if (fn ~ ore) k = (b > 4 ? b : 4)
               else if (fn ~ re) k = (fn ~ pre && b > 3 ? 3 : b); else if (fn ~ lre) k = 25; else if (fn ~ tre) k = 17; else if (fn ~ vre) k = 5; else if (fn ~ rre) k = 8; else if (cb != "" && fn ~ cre) k = cb
               if (k != "") printf "%s%s:%s", (n++ ? "," : ""), id, k
               id = "" }')
  echo "-Z unstable-options --cbmc-args --unwindset ${ids:+$ids,}$memcmp_loop $(extra_cbmc "$2")"
}
# The loop of CBMC's own `memcmp` (<builtin-library-memcmp>), which the
# bytewise slice equalities call: `caller != r.actor` (account names of at
# most 2 bytes) and, from v1.1.0, observe's `values != self.values`, 8 bytes
# per value. CBMC links the library after codegen, so the loop is not in the
# goto binary's --show-loops and gets the harness's bound (6), which
# observe's comparison of 2 values (16 bytes) exceeds: an unwinding failure
# (probe P5, v1.1). Bound 8 * 3 + 1, for vectors of up to 3 values. CBMC
# only warns about the id where memcmp is not linked.
memcmp_loop='memcmp.0:25'

# codegen <dir>: compile every harness (the goto binaries unwindset reads).
codegen() {
  (cd "$1" && $kani $flags --only-codegen) >/dev/null 2>&1
}

harnesses() {
  # Every #[kani::proof] fn h_* in proofs.rs, plus the harnesses the macros
  # generate (probe_* harnesses are excluded).
  { grep -A8 '#\[kani::proof\]' "$here/src/proofs.rs" | sed -n 's/^fn \(h_[a-z0-9_]*\)().*/\1/p'
    grep -oE '\bh_(version_cap|version_max|wf|bmc_from_init)_[a-z_]+ = ' "$here/src/proofs.rs" | cut -d' ' -f1
    grep -oE '\bh_step_[a-z]+_(tx|post|inv|cov)\b' "$here/src/proofs.rs"
    grep -oE '\bh_step_[a-z]+_inv_[0-9]+ = ' "$here/src/proofs.rs" | cut -d' ' -f1
    grep -oE '\bh_ref_[a-z]+ = ' "$here/src/proofs.rs" | cut -d' ' -f1; } | awk '!seen[$0]++'
}

family() {
  local f=$1
  f=${f%_tx}; f=${f%_post}; f=${f%_inv}; f=${f%_cov}; f=${f%_exact}
  # One family per body: the step bodies share one cover list (each step's
  # covers are reachable in its own `_cov` harness only).
  case $f in
    h_step_*) f=h_step ;; h_version_cap_*) f=h_version_cap ;; h_version_max_*) f=h_version_max ;;
    h_wf_*) f=h_wf ;; h_full_*) f=h_full ;;
    h_bmc_from_init_*) f=h_bmc_from_init ;; h_slot_cycle_*) f=h_slot_cycle ;;
    h_ref_*) f=h_ref ;;
  esac
  echo "$f"
}

# Plain-mode test filter (plain and the mutants' plain runs): none.
# h_ref_traces (conform.rs) replays conformance/traces/core.txt, the v1.1.0
# corpus in s/7 form, against the core and tla.rs on every call.
plain_skip=''

do_plain() {
  (cd "$here" && cargo test --release -q -- $plain_skip 2>&1) | tee "${PLAIN_LOG:-/dev/null}" | grep -E '^test |test result|panicked|unsatisfied' | grep -vE '\.\.\. ok$'
  return "${PIPESTATUS[0]}"
}

do_kani() {
  local logs=${KANI_LOG_DIR:-$here/target/kani-logs}
  mkdir -p "$logs"
  [ $# -gt 0 ] || set -- $(harnesses)
  local fail=0 covers_all="$logs/covers.tsv" h log start rc secs covers result failed
  : >"$covers_all"
  codegen "$here"
  printf '%-26s %-13s %8s  %s\n' harness result seconds covers
  for h in "$@"; do
    log="$logs/$h.log"
    start=$(date +%s)
    (cd "$here" && run_timeout "$(harness_timeout "$h")" $kani $flags --harness "$mod::$h" --exact $(unwindset "$here/target" "$h")) >"$log" 2>&1
    rc=$?
    secs=$(( $(date +%s) - start ))
    covers=$(grep -oE '[0-9]+ of [0-9]+ cover properties satisfied' "$log" | tail -1)
    # The "ref ..." covers (checks::ref_covers) are the h_ref family's and
    # only its: the step bodies that h_step_* and h_ref_* share reach both
    # cover lists, each family's part flags only its own.
    awk -v fam="$(family "$h")" '/^Check [0-9]+: .*\.cover\./ {c=1; next}
         c && /Status:/ {st=$NF; next}
         c && /Description:/ {sub(/.*Description: "?/, ""); sub(/"$/, "");
                              if ((fam == "h_ref") == ($0 ~ /^ref /)) print fam "\t" $0 "\t" st; c=0}' \
      "$log" >>"$covers_all"
    if [ "$h" = h_canary ]; then
      # It must fail on its own assertion and nothing else.
      failed=$(grep '^Failed Checks:' "$log" | tr -d '"' | sort -u)
      if grep -q 'VERIFICATION:- FAILED' "$log" &&
         [ "$failed" = 'Failed Checks: canary: no submit is ever accepted' ]; then
        result=ok-FAILED
      elif grep -q 'VERIFICATION:- SUCCESSFUL' "$log"; then
        result=CANARY-PASSED; fail=1
      else
        result=CANARY-OTHER; fail=1
      fi
    elif [ $rc -eq 124 ] || [ $rc -eq 142 ]; then
      result=TIMEOUT; fail=1
    elif [ $rc -eq 125 ] || grep -q 'run out of memory' "$log"; then
      result=OUT-OF-MEMORY; fail=1
    elif grep -q 'VERIFICATION:- SUCCESSFUL' "$log"; then
      result=ok
    else
      result=FAILED; fail=1
    fi
    printf '%-26s %-13s %8s  %s\n' "$h" "$result" "$secs" "${covers:--}"
    if [ "$result" = FAILED ] || [ "$result" = CANARY-OTHER ]; then
      grep '^Failed Checks:' "$log" | sort -u | head -5 | sed 's/^/    /'
    fi
  done
  # Covers, per family: each must be SATISFIED in some harness of its family.
  if [ "${KANI_DEFER_COVERS:-0}" = 1 ]; then
    echo "covers: $(cut -f1,2 "$covers_all" | sort -u | wc -l | tr -d ' ') recorded in $covers_all, family check deferred"
    return $fail
  fi
  local missing
  missing=$(awk -F'\t' '{k=$1 "\t" $2; if ($3=="SATISFIED") s[k]=1; else if (!(k in s)) s[k]=0}
                         END {for (k in s) if (!s[k]) print k}' "$covers_all")
  if [ -n "$missing" ]; then
    echo "unsatisfied covers:"; echo "$missing" | sed 's/^/    /'; fail=1
  else
    echo "covers: $(cut -f1,2 "$covers_all" | sort -u | wc -l | tr -d ' ') checked, all satisfied"
  fi
  return $fail
}

# plain_verdict <rc> <log>: survived | killed | ERROR
plain_verdict() {
  if [ "$1" -eq 0 ] && grep -q '^test result: ok' "$2" && ! grep -q '^test result: FAILED' "$2"; then
    echo survived
  elif [ "$1" -ne 0 ] && grep -q '^test result: FAILED' "$2"; then
    echo killed
  else
    echo ERROR
  fi
}

# kani_verdict <rc> <log>: survived | killed | ERROR. An unwinding failure or
# an unsupported construct means part of the search was skipped: ERROR.
kani_verdict() {
  if [ "$1" -eq 124 ] || [ "$1" -eq 125 ] || [ "$1" -eq 142 ] || [ "$1" -eq 137 ] || grep -q 'run out of memory' "$2"; then
    echo ERROR
  elif grep -q '^Failed Checks: .*unwinding assertion' "$2" || grep -q '^Failed Checks: .*not currently supported by Kani' "$2"; then
    echo ERROR
  elif [ "$1" -eq 0 ] && grep -q 'VERIFICATION:- SUCCESSFUL' "$2"; then
    echo survived
  elif [ "$1" -ne 0 ] && grep -q 'VERIFICATION:- FAILED' "$2" && grep -q '^Failed Checks:' "$2"; then
    echo killed
  else
    echo ERROR
  fi
}

# mutant_verdict <rc> <log>: killed / survived / ERROR from a mutant's Kani
# run. With --stop-on-fail Kani 0.68.0's parser of CBMC's JSON panics
# ("data did not match any variant of untagged enum ParserItem"), so the
# mutant runs use `--output-format old` (CBMC's own text): killed iff CBMC
# reports a violated property that is not an unwinding assertion, an
# unsupported construct or a Kani-internal check (there are no reachability
# checks or covers in a mutant's run).
mutant_verdict() {
  local vp
  if [ "$1" -eq 124 ] || [ "$1" -eq 125 ] || [ "$1" -eq 142 ] || [ "$1" -eq 137 ] || grep -q 'run out of memory' "$2"; then
    echo ERROR
  elif [ "$1" -eq 0 ] && grep -q '^VERIFICATION SUCCESSFUL' "$2"; then
    echo survived
  else
    vp=$(grep -A3 '^Violated property:' "$2")
    if [ "$1" -ne 0 ] && grep -q '^VERIFICATION FAILED' "$2" && [ -n "$vp" ] &&
      ! printf '%s\n' "$vp" | grep -qE 'unwinding assertion|not currently supported by Kani|KANI_CHECK_ID'; then
      echo killed
    else
      echo ERROR
    fi
  fi
}

do_mutants() {
  local use_kani=0
  if [ "${1:-}" = --kani ]; then use_kani=1; shift; fi
  local glob=${1:-*}
  local work
  work=$(mktemp -d "${TMPDIR:-/tmp}/sp-mutants.XXXXXX") || return 2
  trap 'rm -rf "$work"' RETURN
  export CARGO_TARGET_DIR="$work/target"
  local fail=0 p name expect harnesses rc plain kres v h ok us
  printf '%-42s %-8s %-8s %s\n' mutant expect plain kani
  for p in "$here"/mutants/$glob.patch; do
    name=$(basename "$p" .patch)
    rm -rf "$work/tree"
    mkdir -p "$work/tree/source-mirror/crates" "$work/tree/kani"
    # The core inherits version/edition from the mirror's workspace manifest.
    cp "$proj/source-mirror/Cargo.toml" "$work/tree/source-mirror/"
    cp -r "$proj/source-mirror/crates/core" "$work/tree/source-mirror/crates/"
    cp -r "$here/Cargo.toml" "$here/src" "$work/tree/kani/"
    [ -f "$here/Cargo.lock" ] && cp "$here/Cargo.lock" "$work/tree/kani/"
    mkdir -p "$work/tree/conformance/traces"
    cp "$proj/conformance/traces/core.txt" "$work/tree/conformance/traces/"
    pdir="$work/tree/source-mirror"
    grep -q '^# applies-to: kani' "$p" && pdir="$work/tree/kani"
    if ! (cd "$pdir" && patch -s -p1 <"$p"); then
      printf '%-42s %s\n' "$name" PATCH-FAILED; fail=1; continue
    fi
    # The vacuity guards are the main run's: in the mutant's copy the Kani
    # covers are compiled out, so that CBMC can stop at the first failed
    # check (--stop-on-fail; a SATISFIED cover would count as one).
    sed -i.orig 's/^        kani::cover!(c, \$msg);$/        let _ = c;/' "$work/tree/kani/src/src.rs"
    if grep -q 'kani::cover!' "$work/tree/kani/src/src.rs"; then
      printf '%-42s %s\n' "$name" COVER-STRIP-FAILED; fail=1; continue
    fi
    if grep -q '^# must-survive' "$p"; then expect=survive; else expect=killed; fi
    harnesses=$(sed -n 's/^# killed-by: //p; s/^# checked-by: //p' "$p")
    (cd "$work/tree/kani" && cargo test --release -q -- $plain_skip) >"$work/plain.log" 2>&1
    rc=$?
    [ -n "${KEEP_LOGS:-}" ] && cp "$work/plain.log" "$KEEP_LOGS/$name.plain.log"
    plain=$(plain_verdict $rc "$work/plain.log")
    kres=-
    if [ $use_kani -eq 1 ]; then
      kres=ERROR
      [ -n "$harnesses" ] && kres=survived
      [ -n "$harnesses" ] && codegen "$work/tree/kani"
      for h in $harnesses; do
        # One failed check kills a mutant: CBMC stops at the first.
        us=$(unwindset "$CARGO_TARGET_DIR" "$h")
        us=${us:-"-Z unstable-options --cbmc-args"}" --stop-on-fail"
        (cd "$work/tree/kani" && run_timeout "$(harness_timeout "$h")" $kani $flags --output-format old --harness "proofs::$h" --exact $us) >"$work/kani-$h.log" 2>&1
        rc=$?
        [ -n "${KEEP_LOGS:-}" ] && cp "$work/kani-$h.log" "$KEEP_LOGS/$name.$h.log"
        v=$(mutant_verdict $rc "$work/kani-$h.log")
        if [ "$v" != survived ]; then kres=$v; break; fi
      done
    fi
    ok=1
    if [ "$expect" = killed ]; then
      [ "$plain" = killed ] || ok=0
      [ $use_kani -eq 0 ] || [ "$kres" = killed ] || ok=0
    else
      [ "$plain" = survived ] || ok=0
      [ $use_kani -eq 0 ] || [ "$kres" = survived ] || ok=0
    fi
    [ $ok -eq 1 ] || fail=1
    printf '%-42s %-8s %-8s %s%s\n' "$name" "$expect" "$plain" "$kres" "$([ $ok -eq 1 ] || echo '  <-- UNEXPECTED')"
  done
  return $fail
}

cmd=${1:-all}
[ $# -gt 0 ] && shift
case "$cmd" in
  plain) do_plain ;;
  kani) do_kani "$@" ;;
  mutants) do_mutants "$@" ;;
  ref) do_kani $(harnesses | grep '^h_ref_') ;;
  list) harnesses ;;
  all)
    rc=0
    do_plain || rc=1
    do_kani || rc=1
    do_mutants || rc=1
    exit $rc ;;
  *) echo "usage: $0 plain | kani [h...] | ref | mutants [--kani] [glob] | all | list" >&2; exit 2 ;;
esac
