#!/usr/bin/env bash
# run.sh -- the shared-pool Prolog layer: requirement checks C-1..C-6, the
# reference model, MC-1 (bounded exploration), MC-2 (conformance replay) and
# the COV-2 reads differential (tests/test_reads.pl: the real core's preview
# and best_quantity in conformance/traces/reads.txt against the model).
#
# PROP: C-1 C-2 C-3 C-4 C-5 C-6 MC-1 MC-2
#
# Exit 0 only if every step passes, including the vacuity guards:
#   1. extract.py --check: extracted/*.pl regenerate from the pinned sources
#      with no diff.
#   2. every tests/test_*.pl under scryer-prolog with
#        -g "(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))" </dev/null
#      and no suite may print a loader "Warning:" (e.g. discontiguous test/2
#      clauses overwritten, which would silently drop tests)
#   3. canary: with SHARED_POOL_PROLOG_CANARY=1 each fast suite must exit 1
#      (a failing test makes the file fail). test_mc1 and test_reads have no
#      canary run; mutants m12, m15, m21, m22, m24-m32 (test_mc1) and m18-m20,
#      m28 (test_reads) show that each exits non-zero on a failing test.
#   4. PROP tags: every catalogue property that lists the prolog layer is
#      tagged `PROP: <ID>` in a file of this directory, and every tagged ID
#      is in the catalogue.
#   5. mutants: each mutants/{model,requirements,code,reads}/*.patch is
#      applied to a scratch copy (code and reads mutants: extracted facts are
#      regenerated from the mutated source), and every
#      `# expect: <suite> <test-name prefix>` line must hold: the suite exits
#      non-zero AND prints `FAIL <prefix>`. Reads mutants (the code side of
#      COV-2) patch the core, rebuild the conformance generator from it and
#      regenerate core.txt and reads.txt in the copy (needs cargo). Their
#      `# expect: generator <message>` means the generator itself exits
#      non-zero and prints <message>.
#
# C-6 reads the property catalogue from ../strategy.md section 5, so that
# file must be present (a missing one fails C-6).
#
# usage: projects/shared-pool/prolog/run.sh        (about 30 minutes locally)
#        PROLOG_LOGS=dir run.sh                   (keep the logs in dir)
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

HERE=$(cd "$(dirname "$0")" && pwd)
SP=$(dirname "$HERE")
GOAL='(catch(run_tests,E,(write(E),nl,halt(2))) -> halt(0) ; halt(1))'
LOGS=${PROLOG_LOGS:-$(mktemp -d "${TMPDIR:-/tmp}/sp-prolog-logs.XXXXXX")}
mkdir -p "$LOGS"
SCRATCH=$(mktemp -d "${TMPDIR:-/tmp}/sp-prolog-mut.XXXXXX")
trap 'rm -rf "$SCRATCH"' EXIT

status=0
failed=()
fail() { echo "FAIL: $*"; failed+=("$*"); status=1; }
now() { python3 -I -c 'import time; print(f"{time.time():.1f}")'; }
secs() { python3 -I -c "print(f'{$2 - $1:.1f}')"; }

command -v scryer-prolog >/dev/null || { echo "scryer-prolog not on PATH"; exit 2; }
command -v python3 >/dev/null || { echo "python3 not on PATH"; exit 2; }
echo "=== shared-pool Prolog layer ($(scryer-prolog --version 2>/dev/null | head -1)) logs: $LOGS ==="
t_all=$(now)

# --- 1. extracted facts are current -----------------------------------------
t0=$(now)
if python3 -I "$HERE/extract.py" --check >"$LOGS/extract.log" 2>&1; then
  echo "ok   extract.py --check ($(secs "$t0" "$(now)") s)"
else
  cat "$LOGS/extract.log"; fail "extract.py --check: extracted facts are stale"
fi

# --- 2. the suites ---------------------------------------------------------
run_suite() { # <dir> <test file> <log>
  (cd "$1" && scryer-prolog "$2" -g "$GOAL" </dev/null) >"$3" 2>&1
}
for f in "$HERE"/tests/test_*.pl; do
  n=$(basename "$f" .pl)
  t0=$(now)
  run_suite "$HERE/tests" "$n.pl" "$LOGS/$n.log"; rc=$?
  dt=$(secs "$t0" "$(now)")
  if [ "$rc" -eq 0 ] && grep -q 'Warning:' "$LOGS/$n.log"; then
    grep 'Warning:' "$LOGS/$n.log"; fail "$n: loader warning (clauses dropped or overwritten?)"
  elif [ "$rc" -eq 0 ]; then
    echo "ok   $n: $(tail -1 "$LOGS/$n.log") ($dt s)"
  else
    cat "$LOGS/$n.log"; fail "$n exited $rc ($dt s)"
  fi
done

# --- 3. canary --------------------------------------------------------------
for n in test_requirements test_model test_mc2; do
  SHARED_POOL_PROLOG_CANARY=1 run_suite "$HERE/tests" "$n.pl" "$LOGS/canary-$n.log"; rc=$?
  if [ "$rc" -eq 1 ] && grep -q '^FAIL canary_must_fail' "$LOGS/canary-$n.log" \
     && grep -q '^=== 1 TEST(S) FAILED' "$LOGS/canary-$n.log"; then
    echo "ok   canary: $n exits 1 with the canary test failing"
  else
    fail "canary: $n exited $rc with SHARED_POOL_PROLOG_CANARY=1 (want 1, only the canary failing)"
  fi
done

# --- 4. PROP tags -----------------------------------------------------------
if python3 -I - "$HERE" >"$LOGS/prop-tags.log" 2>&1 <<'EOF'
import os, re, sys
here = sys.argv[1]
cat = {}
for m in re.finditer(r"^property\('([^']+)',\s*\[([^\]]*)\]", open(os.path.join(here, "requirements", "traceability.pl")).read(), re.M):
    cat[m.group(1)] = [x.strip().strip("'") for x in m.group(2).split(",")]
tags = {}
for d, _, fs in os.walk(here):
    if os.sep + "mutants" in d:
        continue
    for f in fs:
        if f.endswith((".pl", ".py", ".sh")):
            p = os.path.join(d, f)
            for line in open(p, encoding="utf-8"):
                m = re.search(r"PROP:\s*(.*)", line)
                if m:
                    for t in re.findall(r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+", m.group(1)):
                        tags.setdefault(t, set()).add(os.path.relpath(p, here))
want = sorted(k for k, ls in cat.items() if "prolog" in ls)
missing = [k for k in want if k not in tags]
unknown = sorted(t for t in tags if t not in cat)
print(f"catalogue {len(cat)}, prolog layer {len(want)}, tagged {len(tags)}")
for k in missing: print("untagged:", k)
for t in unknown: print("not in catalogue:", t, sorted(tags[t]))
sys.exit(1 if missing or unknown or not want else 0)
EOF
then
  echo "ok   PROP tags: $(head -1 "$LOGS/prop-tags.log")"
else
  cat "$LOGS/prop-tags.log"; fail "PROP tags"
fi

# --- 5. mutants -------------------------------------------------------------
# A scratch copy laid out like the repository: prolog/, strategy.md (C-6's
# catalogue) and the sources it extracts from are copied; the traces and TLA+
# are linked read-only. For a reads mutant the conformance crate and the
# core's manifests are copied instead, and the traces are regenerated.
mirror() { # <dir> [reads]
  local t=$1
  mkdir -p "$t/projects/shared-pool/source-mirror/crates/core" "$t/projects/shared-pool/source-mirror/crates/contract"
  cp -R "$HERE" "$t/projects/shared-pool/prolog"
  cp "$SP/strategy.md" "$t/projects/shared-pool/strategy.md"
  cp -R "$SP/source-mirror/crates/core/src" "$t/projects/shared-pool/source-mirror/crates/core/src"
  cp -R "$SP/source-mirror/crates/contract/src" "$t/projects/shared-pool/source-mirror/crates/contract/src"
  if [ "${2:-}" = reads ]; then
    cp "$SP/source-mirror/Cargo.toml" "$t/projects/shared-pool/source-mirror/Cargo.toml"
    cp "$SP/source-mirror/crates/core/Cargo.toml" "$t/projects/shared-pool/source-mirror/crates/core/Cargo.toml"
    mkdir -p "$t/projects/shared-pool/conformance/traces"
    cp -R "$SP/conformance/src" "$SP/conformance/Cargo.toml" "$SP/conformance/Cargo.lock" "$t/projects/shared-pool/conformance/"
  else
    ln -s "$SP/conformance" "$t/projects/shared-pool/conformance"
  fi
  [ -e "$SP/tla" ] && ln -s "$SP/tla" "$t/projects/shared-pool/tla"
  return 0
}
killed=0; survived=0
for p in "$HERE"/mutants/model/*.patch "$HERE"/mutants/requirements/*.patch "$HERE"/mutants/code/*.patch "$HERE"/mutants/reads/*.patch; do
  [ -f "$p" ] || continue
  id=$(basename "$p" .patch)
  kind=$(basename "$(dirname "$p")")
  t="$SCRATCH/$id"; mkdir -p "$t"; mirror "$t" "$kind"
  t0=$(now)
  if ! patch -s -p1 -d "$t/projects/shared-pool" <"$p" >"$LOGS/mut-$id-apply.log" 2>&1; then
    cat "$LOGS/mut-$id-apply.log"; fail "mutant $id does not apply"; rm -rf "$t"; continue
  fi
  case "$kind" in
    code|reads)
      if ! python3 -I "$t/projects/shared-pool/prolog/extract.py" >"$LOGS/mut-$id-extract.log" 2>&1; then
        cat "$LOGS/mut-$id-extract.log"; fail "mutant $id: extract.py failed on the mutated source"; rm -rf "$t"; continue
      fi
      if [ "$kind" = code ] && cmp -s "$HERE/extracted/code_facts.pl" "$t/projects/shared-pool/prolog/extracted/code_facts.pl"; then
        fail "mutant $id: extracted facts did not change"
      fi ;;
  esac
  if [ "$kind" = reads ]; then
    # One cargo target directory for all reads mutants; pool-core has no
    # dependencies, so each rebuild takes seconds.
    glog="$LOGS/mut-$id-generator.log"
    (cd "$t/projects/shared-pool/conformance" \
      && CARGO_TARGET_DIR="$SCRATCH/.cargo-target" cargo run --release --locked -- \
           --out traces/core.txt --reads-out traces/reads.txt) >"$glog" 2>&1
    echo $? >"$glog.rc"
    if [ "$(cat "$glog.rc")" -ne 0 ] && ! grep -q '^# expect: generator ' "$p"; then
      tail -20 "$glog"; fail "mutant $id: the generator failed on the mutated core"; rm -rf "$t"; continue
    fi
    if [ "$(cat "$glog.rc")" -eq 0 ] && cmp -s "$SP/conformance/traces/reads.txt" "$t/projects/shared-pool/conformance/traces/reads.txt"; then
      fail "mutant $id: reads.txt did not change"
    fi
  fi
  ok=1; n_exp=0; why=""
  while IFS= read -r line; do
    set -- $line; suite=$3; shift 3; prefix="$*"
    n_exp=$((n_exp + 1))
    log="$LOGS/mut-$id-$suite.log"
    if [ "$suite" = generator ]; then
      want="$prefix"
    else
      want="FAIL $prefix"
      if [ ! -f "$log.rc" ]; then
        run_suite "$t/projects/shared-pool/prolog/tests" "$suite.pl" "$log"; echo $? >"$log.rc"
      fi
    fi
    rc=$(cat "$log.rc" 2>/dev/null || echo none)
    if [ "$rc" != 0 ] && [ "$rc" != none ] && grep -qF "$want" "$log"; then
      why="$why $suite"
    else
      ok=0; why="$why $suite(exit $rc; want $want)"
    fi
  done < <(grep '^# expect: ' "$p")
  dt=$(secs "$t0" "$(now)")
  if [ "$n_exp" -eq 0 ]; then
    fail "mutant $id has no '# expect:' line"
  elif [ "$ok" -eq 1 ]; then
    killed=$((killed + 1)); echo "ok   mutant $id killed by$why ($dt s)"
  else
    survived=$((survived + 1)); fail "mutant $id survived:$why"
  fi
  rm -rf "$t"
done
echo "     mutants: $killed killed, $survived survived"

echo "=== total $(secs "$t_all" "$(now)") s ==="
if [ "$status" -ne 0 ]; then
  echo "=== FAILED: ${#failed[@]} step(s) ==="; printf '  - %s\n' "${failed[@]}"
else
  echo "=== ALL PROLOG CHECKS PASSED ==="
fi
exit "$status"
