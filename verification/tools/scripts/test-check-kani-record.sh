#!/usr/bin/env bash
# Self-test for check-kani-record.py, kani/local/record.py and
# run-local.py's environment and cargo configuration checks: a synthetic
# finished local run of the kani-large and kani-mutants-large jobs (at HEAD)
# records and passes the check, and so do the same jobs split over two
# runs (measurements stopped early; the latest passed result winning, a
# later failure not replacing a pass), a job without a start line (its
# limit jobs.json's timeout), and runs
# without the large mutants that not-run.tsv declares; a run with a large
# job failed or missing, a foreign run, runs at different commits or
# fingerprints, a declared mutant that a run finished, a declared harness,
# and a limit above the matrix timeout are refused and leave the record
# alone; and each kind of stale, wrong or inconsistent record, each
# workflow matrix that does not split the harnesses and mutants with the
# record, and each large timeout below the limit the record ran it under,
# fails the check. The cases take their matrix entries from the workflow
# (any split of v1.1's harnesses and mutants), not from fixed names. Commits
# with other Kani inputs are made in a scratch object directory over the
# repository's (no object, ref or index of the repository is written).
# Also kani/local/run-local.py's job lists (the record's, and --all's
# measurement of every harness and mutant), --all's refusals and its
# heavy-job lock (a scratch one), without running a job.
#
# usage: tools/scripts/test-check-kani-record.sh
# env: KANI_RECORD_WORKFLOW, the workflow whose matrices the cases use
#      (default .github/workflows/shared-pool.yml)
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
check="$here/check-kani-record.py"
record="$root/projects/shared-pool/kani/local/record.py"
wf=${KANI_RECORD_WORKFLOW:-$root/.github/workflows/shared-pool.yml}
# record.py, run-local.py and the check's subcommands read the same matrices.
export KANI_RECORD_WORKFLOW="$wf"
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
fail=0
unset GITHUB_ACTIONS  # plain error:/warning: lines, not workflow commands
py() { python3 -I "$@"; }

# fixture <run dir>: a finished run of every kani-large harness (ok, one
# SATISFIED cover each) and kani-mutants-large mutant (as its patch
# expects), with HEAD's provenance.
fixture() {
  py - "$1" "$check" "$root" <<'PY'
import json, os, re, subprocess, sys
d, check, root = sys.argv[1:4]
def out(*a):
    return subprocess.run(list(a), capture_output=True, text=True, check=True).stdout
def git(*a):
    return out("git", "-C", root, *a).strip()
ver = git("show", "HEAD:tools/versions.env")
K = re.search(r"^KANI=(\S+)", ver, re.M).group(1)
C = re.search(r"^CBMC=(\S+)", ver, re.M).group(1)
mdir = os.path.join(root, "projects/shared-pool/kani/mutants")
jobs = []
for line in out(sys.executable, "-I", check, "matrix", "kani-large").splitlines():
    g, gb, to = line.split("\t")
    for h in g.split():
        jobs.append(dict(name=h, kind="harness", peak=float(gb), est=100, timeout=int(to), args=["kani", h]))
for line in out(sys.executable, "-I", check, "matrix", "kani-mutants-large").splitlines():
    nn, gb, to = line.split("\t")
    jobs.append(dict(name="mutant-" + nn, kind="mutant", peak=float(gb), est=100, timeout=int(to),
                     args=["mutants", "--kani", nn + "-*"]))
os.makedirs(os.path.join(d, "logs"))
rows = []
for j in jobs:
    ld = os.path.join(d, "logs", j["name"])
    os.makedirs(ld)
    if j["kind"] == "harness":
        h = j["name"]
        with open(ld + "/stdout.txt", "w") as f:
            f.write("%-26s %-13s %8s  %s\n" % ("harness", "result", "seconds", "covers"))
            f.write("%-26s %-13s %8s  %s\n" % (h, "ok", 42, "1 of 1 cover properties satisfied"))
            f.write("covers: 1 recorded in %s/covers.tsv, family check deferred\n" % ld)
        with open(ld + "/covers.tsv", "w") as f:
            f.write("fam_%s\tcover of %s\tSATISFIED\n" % (h, h))
        open(ld + "/%s.log" % h, "w").write("VERIFICATION:- SUCCESSFUL\n")
        v = "ok"
    else:
        nn = j["name"].split("-", 1)[1]
        p = [f[:-6] for f in sorted(os.listdir(mdir)) if f.startswith(nn + "-") and f.endswith(".patch")][0]
        text = open(os.path.join(mdir, p + ".patch")).read()
        e, k = ("survive", "survived") if "# must-survive\n" in text else ("killed", "killed")
        h0 = re.search(r"^# (?:killed-by|checked-by): (\S+)", text, re.M).group(1)
        with open(ld + "/stdout.txt", "w") as f:
            f.write("%-42s %-8s %-8s %s\n" % ("mutant", "expect", "plain", "kani"))
            f.write("%-42s %-8s %-8s %s\n" % (p, e, k, k))
        open(ld + "/%s.plain.log" % p, "w").write("")
        open(ld + "/%s.%s.log" % (p, h0), "w").write("")
        v = "%s/%s/%s" % (e, k, k)
    rows.append([j["name"], j["kind"], "lane1", "2026-01-01T00:00:00Z", "100", "0", v, "5.0", "4.0", str(j["peak"]), "100"])
json.dump(jobs, open(d + "/jobs.json", "w"))
open(d + "/results.tsv", "w").write("".join("\t".join(r) + "\n" for r in rows))
open(d + "/progress.log", "w").write("00:00:00Z %d jobs; budget 50.0 GB\n" % len(jobs) + "".join(
    "00:00:01Z start %s on lane1 (peak %s GB, est 100 s, limit %d s)\n" % (j["name"], j["peak"], j["timeout"])
    for j in jobs) + "00:59:00Z ALL DONE\n")
open(d + "/env.txt", "w").write(
    "commit %s\nkani-tree %s\ncore-pin %s\nkani Kani Rust Verifier %s (cargo plugin)\nCBMC %s\ncbmc %s (cbmc-%s)\n"
    "host test host\nstarted 2026-01-01T00:00:00Z\n"
    % (git("rev-parse", "HEAD"), git("rev-parse", "HEAD:projects/shared-pool/kani"),
       git("rev-parse", "HEAD:projects/shared-pool/source-mirror"), K, C, C, C))
PY
}

# edit <file> <python statement on s, the file's text>
edit() {
  py -c "import re, sys; p = sys.argv[1]; s = open(p).read(); $2; open(p, 'w').write(s)" "$1"
}

# redigest <record dir>: provenance's digest of an edited jobs.tsv/covers.tsv/not-run.tsv.
redigest() {
  py -c "
import hashlib, os, re, sys
d = sys.argv[1]; h = hashlib.sha256()
for f in ('jobs.tsv', 'covers.tsv', 'not-run.tsv'):
    if f != 'not-run.tsv' or os.path.exists(d + '/' + f): h.update(open(d + '/' + f, 'rb').read())
p = d + '/provenance.txt'; s = open(p).read()
open(p, 'w').write(re.sub(r'(?m)^digest\t.*$', 'digest\t' + h.hexdigest(), s))" "$1"
}

# expect <exit> <name> <output pattern or ""> <command...>
expect() {
  local want=$1 name=$2 pat=$3 got
  shift 3
  "$@" >"$t/out" 2>&1
  got=$?
  [ "$got" -gt 1 ] && got=2
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name: exit $got, want $want"; sed 's/^/  /' "$t/out"; fail=1
  elif [ -n "$pat" ] && ! grep -qE -- "$pat" "$t/out"; then
    echo "FAIL: $name: output lacks /$pat/"; sed 's/^/  /' "$t/out"; fail=1
  fi
}

begin='<!-- kani-local-record: begin (kani/local/record.py writes this block; do not edit) -->'
end='<!-- kani-local-record: end -->'
# run_record <run dir> <record dir> [RESULTS.md, default: one with an old block]
run_record() {
  local r=${3:-$t/rr.md}
  [ -n "${3:-}" ] || printf 'before\n%s\nold table\n%s\nafter\n' "$begin" "$end" >"$r"
  KANI_RECORD_DIR="$2" KANI_RESULTS_MD="$r" "$record" "$1"
}
# run_check <record dir> [workflow] [RESULTS.md, default: the record's own
# table in a block] [args...]
run_check() {
  local d=$1 w=${2:-$wf} r=${3:-}
  shift 3 2>/dev/null || shift $#
  if [ -z "$r" ]; then
    r="$t/auto.md"
    { echo "$begin"; KANI_RECORD_DIR="$d" "$record" table 2>/dev/null; echo "$end"; } >"$r"
  fi
  KANI_RECORD_DIR="$d" KANI_RECORD_WORKFLOW="$w" KANI_RESULTS_MD="$r" "$check" "$@"
}

# The matrix entries the cases edit, from the workflow: KG a kani group of
# two or more harnesses (KH its first), WG the kani-large group with
# h_wedge, TWO a kani-large group of two or more harnesses (if any), MC a
# kani-mutants entry, ML a kani-mutants-large mutant that its patch expects
# killed (MLP its patch); each with its gb (..GB) and timeout (..TO).
picks=$(py - "$check" "$root" <<'PY'
import os, shlex, subprocess, sys
check, root = sys.argv[1:3]
def matrix(job):
    out = subprocess.run([sys.executable, "-I", check, "matrix", job], capture_output=True, text=True).stdout
    return [l.split("\t") for l in out.splitlines() if l]
mdir = os.path.join(root, "projects/shared-pool/kani/mutants")
def patch(nn):
    ps = [f[:-6] for f in sorted(os.listdir(mdir)) if f.startswith(nn + "-") and f.endswith(".patch")]
    return ps[0] if len(ps) == 1 else None
def killed(nn):
    p = patch(nn)
    return p and "# must-survive\n" not in open(os.path.join(mdir, p + ".patch")).read()
pick = {}
for pre, job, ok in (("KG", "kani", lambda e: len(e[0].split()) > 1),
                     ("WG", "kani-large", lambda e: "h_wedge" in e[0].split()),
                     ("TWO", "kani-large", lambda e: len(e[0].split()) > 1),
                     ("MC", "kani-mutants", lambda e: patch(e[0])),
                     ("ML", "kani-mutants-large", lambda e: killed(e[0]) and int(e[2]) > 600)):
    e = next((e for e in matrix(job) if ok(e)), ["", "", ""])
    pick.update({pre: e[0], pre + "GB": e[1], pre + "TO": e[2]})
pick["KH"] = pick["KG"].split()[0] if pick["KG"] else ""
pick["MLP"] = patch(pick["ML"]) or "" if pick["ML"] else ""
pick["MCP"] = patch(pick["MC"]) or "" if pick["MC"] else ""
for k, v in sorted(pick.items()):
    print("%s=%s" % (k, shlex.quote(v)))
PY
)
eval "$picks"
for v in KG WG MC ML MLP; do
  [ -n "${!v}" ] || { echo "FAIL: no $v in $wf's matrices (see the comment above picks)"; echo "$picks" | sed 's/^/  /'; fail=1; }
done
# ent <group|mutant> <name> <gb> <timeout>: a matrix entry as the workflow writes it.
ent() { printf '{ %s: "%s", gb: %s, timeout: %s }' "$@"; }

# --- record.py ---------------------------------------------------------------
fixture "$t/run"
mkdir -p "$t/rec"
printf 'before\n<!-- kani-local-record: begin (stale) -->\nold table\n<!-- kani-local-record: end -->\nafter\n' >"$t/results.md"
expect 0 "a complete run records" "recorded [0-9]+ harnesses and [0-9]+ mutants" run_record "$t/run" "$t/rec" "$t/results.md"
for f in provenance.txt jobs.tsv covers.tsv; do
  [ -s "$t/rec/$f" ] || { echo "FAIL: record.py wrote no $f"; fail=1; }
done
grep -q '^| `h_wedge` | ok |' "$t/results.md" && grep -q '^after$' "$t/results.md" && ! grep -q 'old table' "$t/results.md" ||
  { echo "FAIL: record.py did not rewrite the RESULTS.md block"; sed 's/^/  /' "$t/results.md"; fail=1; }
expect 0 "the new record passes" "local Kani record ok" run_check "$t/rec" "$wf" "$t/results.md"
cp -R "$t/rec" "$t/good"

refuse() { # refuse <name> <pattern> <python edit of the run> [file in the run]
  rm -rf "$t/bad" "$t/rec2"; cp -R "$t/run" "$t/bad"; cp -R "$t/good" "$t/rec2"
  edit "$t/bad/${4:-results.tsv}" "$3"
  expect 1 "$1" "$2" run_record "$t/bad" "$t/rec2"
  cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" || { echo "FAIL: $1: the record changed"; fail=1; }
}
refuse "a large job not finished (still running)" "^  [^ ]+: in no run" "s = s.split('\n', 1)[1]"
refuse "a limit not the one it ran under" "h_wedge: progress.log's limit 60 s is not jobs.json's timeout [0-9]+ \(in run1\)" \
  "s = re.sub(r'(?m)^(.* start h_wedge on .*limit )[0-9]+ s\)$', r'\g<1>60 s)', s)" progress.log
refuse "a harness out of time" "h_wedge: TIMEOUT, not ok" "s = re.sub(r'(?m)^(h_wedge\t.*\t0\t)ok\t', r'\1TIMEOUT\t', s)"
refuse "a harness's stdout disagrees" "h_wedge: FAILED in stdout.txt, not ok" "s = s.replace(' ok ', ' FAILED ')" logs/h_wedge/stdout.txt
refuse "run.sh exit 1" "h_wedge: run.sh exit 1" "s = re.sub(r'(?m)^(h_wedge\t(?:[^\t]*\t){4})0\t', r'\g<1>1\t', s)"
refuse "a mutant unexpected" "want killed/killed/killed" \
  "s = re.sub(r'(?m)^($ML-\S+\s+killed\s+killed\s+)killed\s*$', r'\1survived  <-- UNEXPECTED', s)" "logs/mutant-$ML/stdout.txt"
refuse "foreign kani tree" "kani-tree" "s = re.sub(r'(?m)^kani-tree .*$', 'kani-tree ' + '0' * 40, s)" env.txt
refuse "other Kani version" "is not Kani" "s = re.sub(r'(?m)^kani .*$', 'kani Kani Rust Verifier 0.0.1 (cargo plugin)', s)" env.txt
refuse "other CBMC version" "is not CBMC" "s = re.sub(r'(?m)^cbmc .*$', 'cbmc 1.0.0 (cbmc-1.0.0)', s)" env.txt
refuse "unknown commit" "not in this repository" "s = re.sub(r'(?m)^commit .*$', 'commit ' + 'f' * 40, s)" env.txt
# A mutant whose Kani leg left no log is not recorded from its header.
rm -rf "$t/bad" "$t/rec2"; cp -R "$t/run" "$t/bad"; cp -R "$t/good" "$t/rec2"
find "$t/bad/logs/mutant-25" -name '25-*.h_*.log' -exec rm {} +
expect 1 "a mutant without its Kani log" "mutant-25 .*no log of its Kani leg" run_record "$t/bad" "$t/rec2"
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" || { echo "FAIL: a mutant without its Kani log: the record changed"; fail=1; }
# RESULTS.md without the block: refused, nothing written.
printf 'no block here\n' >"$t/noblock.md"
expect 1 "RESULTS.md without a block" "has no local-record block" run_record "$t/run" "$t/rec2" "$t/noblock.md"
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" && [ "$(cat "$t/noblock.md")" = "no block here" ] ||
  { echo "FAIL: RESULTS.md without a block: the record or RESULTS.md changed"; fail=1; }
# Complete and ok, but not the workflow's large jobs: the check refuses it.
rm -rf "$t/bad" "$t/rec2"; cp -R "$t/run" "$t/bad"; cp -R "$t/good" "$t/rec2"
edit "$t/bad/results.tsv" "s = re.sub(r'(?m)^h_wedge\t.*\n', '', s)"
edit "$t/bad/jobs.json" "import json; s = json.dumps([j for j in json.loads(s) if j['name'] != 'h_wedge'])"
expect 1 "a run without one large harness" "h_wedge" run_record "$t/bad" "$t/rec2"
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" || { echo "FAIL: a run without h_wedge: the record changed"; fail=1; }
expect 1 "a missing run directory" "no env.txt" run_record "$t/nowhere" "$t/rec2"

# --- record.py: measurements, several runs, not-run.tsv ----------------------
# The record from measurements (run-local.py --all), stopped early or not:
# the large jobs of the matrices, each from the run that finished it, the
# latest when several did; a large mutant no run finished only if
# not-run.tsv declares it. run_records <record dir> <run dir>...
run_records() {
  local d=$1; shift
  printf 'before\n%s\nold table\n%s\nafter\n' "$begin" "$end" >"$t/rr.md"
  KANI_RECORD_DIR="$d" KANI_RESULTS_MD="$t/rr.md" "$record" "$@"
}
measure='mode measurement (run-local.py --all: every harness of run.sh list and every mutant; not a record)'
rm -rf "$t/m"; cp -R "$t/run" "$t/m"
printf '%s\nfingerprint %s\n' "$measure" "$("$check" fingerprint HEAD | tail -1 | cut -f2)" >>"$t/m/env.txt"
edit "$t/m/progress.log" "s = s.replace('ALL DONE', 'stopped by signal 15; not complete')"
rm -rf "$t/rec2"; mkdir "$t/rec2"
expect 0 "a measurement stopped early records" "recorded [0-9]+ harnesses and [0-9]+ mutants from 1 run" run_records "$t/rec2" "$t/m"
grep -q '^assembled	from 1 measurement run (run-local.py --all), not a separate --record run' "$t/rec2/provenance.txt" &&
  grep -q '^run1\.status	not complete (no ALL DONE)' "$t/rec2/provenance.txt" && cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" ||
  { echo "FAIL: a measurement: not the run's record, or provenance without its runs"; sed 's/^/  /' "$t/rec2/provenance.txt"; fail=1; }
# split <a> <b> <python expression on the job name n>: two copies of $t/m, a
# with the finished jobs for which it holds, b (started a day later) with the rest.
split() {
  rm -rf "$1" "$2"; cp -R "$t/m" "$1"; cp -R "$t/m" "$2"
  edit "$1/results.tsv" "s = ''.join(l for l in s.splitlines(True) if (lambda n: $3)(l.split('\t')[0]))"
  edit "$2/results.tsv" "s = ''.join(l for l in s.splitlines(True) if not (lambda n: $3)(l.split('\t')[0]))"
  edit "$2/env.txt" "s = re.sub(r'(?m)^started .*$', 'started 2026-01-02T00:00:00Z', s)"
}
m1=$(sed -n 's/^\(mutant-[^	]*\)	.*/\1/p' "$t/m/results.tsv" | head -1)
m1p=$(sed -n "s/^\($(echo "${m1#mutant-}")-[^ ]*\) .*/\1/p" "$t/m/logs/$m1/stdout.txt")
split "$t/A" "$t/B" "not n.startswith('mutant-')"
rm -rf "$t/rec2"; mkdir "$t/rec2"
expect 0 "two runs merged (harnesses, then mutants)" "recorded [0-9]+ harnesses and [0-9]+ mutants from 2 runs" \
  run_records "$t/rec2" "$t/B" "$t/A"
grep -q '^assembled	from 2 measurement runs' "$t/rec2/provenance.txt" && grep -q "^run1	$t/A$" "$t/rec2/provenance.txt" &&
  grep -q "^run2\.recorded	[0-9]* jobs: .*$m1p" "$t/rec2/provenance.txt" && grep -q '^run2\.started	2026-01-02' "$t/rec2/provenance.txt" &&
  cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" && cmp -s "$t/good/covers.tsv" "$t/rec2/covers.tsv" ||
  { echo "FAIL: two runs merged: not the single run's jobs, or provenance without both runs"; sed 's/^/  /' "$t/rec2/provenance.txt"; fail=1; }
expect 0 "... and the merged record passes" "local Kani record ok" run_check "$t/rec2"
# The later run wins (provenance says so) when it passed; a failed result
# never replaces a passed one (taken from the earlier run, provenance says
# so); a job no run passed is refused, each run's result listed.
split "$t/A" "$t/B" "True"
edit "$t/B/results.tsv" "s = '\n'.join(l.replace('2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z') for l in open('$t/m/results.tsv').read().splitlines() if l.startswith('h_wedge\t')) + '\n'"
rm -rf "$t/rec2"; mkdir "$t/rec2"
expect 0 "a job in two runs: the later run's" "superseded: h_wedge from run2 \(ok\), not run1 \(ok\)" run_records "$t/rec2" "$t/A" "$t/B"
grep -q '^harness	h_wedge	.*	2026-01-02T00:00:00Z$' "$t/rec2/jobs.tsv" &&
  grep -q "^superseded\.h_wedge	run1's result (ok, rc 0, started 2026-01-01T00:00:00Z) replaced by run2's" "$t/rec2/provenance.txt" ||
  { echo "FAIL: a job in two runs: not the later run's, or not in provenance"; fail=1; }
edit "$t/B/results.tsv" "s = re.sub(r'(?m)^(h_wedge\t.*\t)0\tok\t', r'\g<1>1\tTIMEOUT\t', s)"
rm -rf "$t/rec2"; mkdir "$t/rec2"
expect 0 "a job in two runs, the later one failed: the earlier pass" "superseded: h_wedge from run1 \(ok\), not run2 \(TIMEOUT\)" \
  run_records "$t/rec2" "$t/A" "$t/B"
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" &&
  grep -q "^superseded\.h_wedge	run2's result (TIMEOUT, rc 1, started 2026-01-02T00:00:00Z) not taken: it did not pass, and a failed result never replaces run1's passed one$" "$t/rec2/provenance.txt" &&
  grep -q "^run1\.recorded	.* h_wedge" "$t/rec2/provenance.txt" && grep -q "^run2\.recorded	0 jobs$" "$t/rec2/provenance.txt" ||
  { echo "FAIL: the later run failed: not run1's h_wedge, or provenance does not say so"; sed 's/^/  /' "$t/rec2/provenance.txt"; fail=1; }
expect 0 "... and that record passes" "local Kani record ok" run_check "$t/rec2"
edit "$t/A/results.tsv" "s = re.sub(r'(?m)^(h_wedge\t.*\t)0\tok\t', r'\g<1>1\tTIMEOUT\t', s)"
rm -rf "$t/rec2"; cp -R "$t/good" "$t/rec2"
expect 1 "a job in two runs, both failed" "h_wedge: run\.sh exit 1 \(TIMEOUT\) \(in run1\)" run_records "$t/rec2" "$t/A" "$t/B"
grep -q "h_wedge: run\.sh exit 1 (TIMEOUT) (in run2)" "$t/out" ||
  { echo "FAIL: both runs failed: run2's result not listed"; sed 's/^/  /' "$t/out"; fail=1; }
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" || { echo "FAIL: both runs failed: the record changed"; fail=1; }
# A job without a start line in progress.log: its limit is jobs.json's
# timeout (the KANI_TIMEOUT run-local.py ran it under).
rm -rf "$t/A"; cp -R "$t/m" "$t/A"
edit "$t/A/progress.log" "s = re.sub(r'(?m)^.* start h_wedge on .*\n', '', s)"
rm -rf "$t/rec2"; mkdir "$t/rec2"
expect 0 "a job without a start line: jobs.json's timeout" "recorded [0-9]+ harnesses" run_records "$t/rec2" "$t/A"
cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" ||
  { echo "FAIL: no start line: not the record with jobs.json's timeout as limit_s"; grep h_wedge "$t/rec2/jobs.tsv" | sed 's/^/  /'; fail=1; }
# Refused, the record untouched: a large mutant in no run and not declared;
# runs at different fingerprints or commits; a run given twice.
nogo() { # nogo <name> <pattern> <run dir>...
  local name=$1 pat=$2; shift 2
  rm -rf "$t/rec2"; cp -R "$t/good" "$t/rec2"; [ -z "${NOT_RUN:-}" ] || printf "$NOT_RUN" >"$t/rec2/not-run.tsv"
  expect 1 "$name" "$pat" run_records "$t/rec2" "$@"
  cmp -s "$t/good/jobs.tsv" "$t/rec2/jobs.tsv" || { echo "FAIL: $name: the record changed"; fail=1; }
}
split "$t/A" "$t/B" "not n.startswith('mutant-')"
nogo "a large mutant in no run, not declared" "$m1: in no run and not declared in not-run\.tsv" "$t/A"
edit "$t/B/env.txt" "s = re.sub(r'(?m)^fingerprint .*$', 'fingerprint ' + '0' * 64, s)"
nogo "runs at different fingerprints" "the runs have different fingerprints" "$t/A" "$t/B"
rm -rf "$t/A"; cp -R "$t/m" "$t/A"; edit "$t/A/env.txt" "s = re.sub(r'(?m)^fingerprint .*$', 'fingerprint ' + '0' * 64, s)"
nogo "a run whose fingerprint is not its commit's" "run1 \(.*\): env\.txt fingerprint 0{64} is not commit [0-9a-f]{12}'s" "$t/A"
split "$t/A" "$t/B" "not n.startswith('mutant-')"
edit "$t/B/env.txt" "s = re.sub(r'(?m)^commit .*$', 'commit ' + 'f' * 40, s)"
nogo "runs at different commits" "the runs are at different commits" "$t/A" "$t/B"
nogo "a run given twice" "given twice" "$t/m" "$t/m/"
# not-run.tsv: a large mutant that no run finished, with its reason: recorded
# without it, said by record.py and the check (a warning), and in the table.
split "$t/A" "$t/B" "not n.startswith('mutant-')"
rm -rf "$t/rec2"; mkdir "$t/rec2"
{ printf 'mutant\treason\n'; for p in "$root"/projects/shared-pool/kani/mutants/*.patch; do
    p=${p##*/}; p=${p%.patch}; grep -q "^mutant	$p	" "$t/good/jobs.tsv" && printf '%s\tnot run by the deadline\n' "$p"
  done; } >"$t/rec2/not-run.tsv"
expect 0 "large mutants declared not run" "NOT RUN \(declared in not-run\.tsv\): $m1p: not run by the deadline" run_records "$t/rec2" "$t/A"
grep -q '^mutant	' "$t/rec2/jobs.tsv" && { echo "FAIL: declared not run: a mutant recorded"; fail=1; }
grep -q "^not_run\.$m1p	not run by the deadline$" "$t/rec2/provenance.txt" && grep -q '^- `'"$m1p"'`: not run by the deadline$' "$t/rr.md" ||
  { echo "FAIL: declared not run: not in provenance.txt or the table"; fail=1; }
expect 0 "... the check passes, with a warning per mutant" "warning: kani-mutants-large mutant $m1p was NOT RUN locally \(declared in kani/local/not-run\.tsv: not run by the deadline\)" \
  run_check "$t/rec2"
grep -qE "local Kani record ok: .*, [0-9]+ kani-mutants-large mutants NOT RUN" "$t/out" ||
  { echo "FAIL: declared not run: the check's summary does not say so"; sed 's/^/  /' "$t/out"; fail=1; }
cp -R "$t/rec2" "$t/declared"
edit "$t/rec2/not-run.tsv" "s = s.replace('not run by the deadline', 'later', 1)"
expect 1 "not-run.tsv edited by hand" "not-run\.tsv is not what record\.py wrote" run_check "$t/rec2"
rm -rf "$t/rec2"; cp -R "$t/declared" "$t/rec2"; edit "$t/rec2/not-run.tsv" "s = '\n'.join(s.splitlines()[:-1]) + '\n'"; redigest "$t/rec2"
expect 1 "a large mutant neither recorded nor declared" "kani-mutants-large must be the recorded mutants and those declared not run" run_check "$t/rec2"
rm -rf "$t/rec2"; cp -R "$t/good" "$t/rec2"; printf 'mutant\treason\n%s\tx\n' "$m1p" >"$t/rec2/not-run.tsv"; redigest "$t/rec2"
expect 1 "a recorded mutant also declared not run" "declared not run \(not-run\.tsv\) and in the local record: $m1p" run_check "$t/rec2"
rm -rf "$t/rec2"; cp -R "$t/declared" "$t/rec2"; printf '%s\tx\n' "$MCP" >>"$t/rec2/not-run.tsv"; redigest "$t/rec2"
expect 1 "a kani-mutants mutant declared not run" "declared not run \(not-run\.tsv\) and in kani-mutants: $MCP" run_check "$t/rec2"
rm -rf "$t/rec2"; cp -R "$t/declared" "$t/rec2"; printf 'h_wedge\tx\n' >>"$t/rec2/not-run.tsv"; redigest "$t/rec2"
expect 1 "a harness declared not run (the check)" "not-run\.tsv: h_wedge is not a mutants/\*\.patch" run_check "$t/rec2"
NOT_RUN=$(printf 'mutant\\treason\\n%s\\tnot run by the deadline\\n' "$m1p")
nogo "a declared mutant that a run finished" "$m1 \($m1p\) is declared not run in not-run\.tsv, but run1 finished it" "$t/m"
NOT_RUN='mutant\treason\nh_wedge\tno time\n'
nogo "a harness declared not run" "declares h_wedge not run, but it is not a kani-mutants-large mutant" "$t/m"
NOT_RUN='job\treason\n'
nogo "not-run.tsv with another header" "not-run\.tsv: the header must be mutant<TAB>reason" "$t/m"
unset NOT_RUN
# A job's limit raised above its matrix timeout (run-local.py
# --estimates-from): the record would fail the check, so record.py refuses
# it; with the workflow's timeout raised to that limit, it records it.
hi=$((WGTO + 600))
rm -rf "$t/A"; cp -R "$t/m" "$t/A"
edit "$t/A/progress.log" "s = re.sub(r'(?m)^(.* start h_wedge on .*limit )[0-9]+ s\)$', r'\g<1>$hi s)', s)"
edit "$t/A/jobs.json" "import json; j = json.loads(s); [x.update(timeout=$hi) for x in j if x['name'] == 'h_wedge']; s = json.dumps(j)"
nogo "a limit above the matrix timeout" "kani-large entry '$WG': timeout $WGTO is below the $hi s the local record ran h_wedge under" "$t/A"
cp "$wf" "$t/wf-hi.yml"
edit "$t/wf-hi.yml" "s = s.replace('$(ent group "$WG" "$WGGB" "$WGTO")', '$(ent group "$WG" "$WGGB" "$hi")', 1)"
rm -rf "$t/rec2"; mkdir "$t/rec2"
KANI_RECORD_WORKFLOW="$t/wf-hi.yml" expect 0 "... recorded once the matrix timeout is raised to it" "recorded [0-9]+ harnesses" \
  run_records "$t/rec2" "$t/A"
grep -q "^harness	h_wedge	\([^	]*	\)\{5\}$hi	" "$t/rec2/jobs.tsv" ||
  { echo "FAIL: a raised limit: not h_wedge's limit_s in jobs.tsv"; grep h_wedge "$t/rec2/jobs.tsv" | sed 's/^/  /'; fail=1; }

# --- check-kani-record.py ----------------------------------------------------
bad() { rm -rf "$t/r"; cp -R "$t/good" "$t/r"; }
expect 1 "no record" "no local Kani record" run_check "$t/empty"

# Commits with other Kani inputs, in the scratch object directory: HEAD plus
# kani/.cargo/config.toml (a new file, which can set rustflags), HEAD plus a
# .cargo/config.toml at the repository's root (above kani/, which cargo also
# reads), HEAD with kani/run.sh changed, and HEAD with only the Markdown and
# kani/local changed. at <commit> <command...>: the check compares with that
# commit.
objs="$t/objects"; mkdir -p "$objs"
alt="$(cd "$root" && cd "$(git rev-parse --git-common-dir)" && pwd)/objects"
sgit() {
  GIT_OBJECT_DIRECTORY="$objs" GIT_ALTERNATE_OBJECT_DIRECTORIES="$alt" GIT_INDEX_FILE="$t/index" \
    GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.invalid GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.invalid \
    git -C "$root" "$@"
}
commit_with() { # commit_with <path> <content> [<path> <content>...]: its id
  local b
  sgit read-tree HEAD || return 1
  while [ $# -ge 2 ]; do
    b=$(printf '%s\n' "$2" | sgit hash-object -w --stdin) && sgit update-index --add --cacheinfo "100644,$b,$1" || return 1
    shift 2
  done
  sgit commit-tree "$(sgit write-tree)" -p HEAD -m test
}
at() { local rev=$1; shift; KANI_RECORD_REV=$rev GIT_OBJECT_DIRECTORY=$objs GIT_ALTERNATE_OBJECT_DIRECTORIES=$alt "$@"; }
k=projects/shared-pool/kani
c_cfg=$(commit_with "$k/.cargo/config.toml" '[build]
rustflags = ["-C", "overflow-checks=off"]')
c_root=$(commit_with ".cargo/config.toml" '[profile.dev]
overflow-checks = false')
c_run=$(commit_with "$k/run.sh" "$(git -C "$root" show "HEAD:$k/run.sh")
# changed")
c_doc=$(commit_with "$k/RESULTS.md" "$(git -C "$root" show "HEAD:$k/RESULTS.md")
more prose" "$k/local/provenance.txt" "another record")
if [ -z "$c_cfg" ] || [ -z "$c_root" ] || [ -z "$c_run" ] || [ -z "$c_doc" ] ||
   [ -n "$(git -C "$root" cat-file -t "$c_cfg" 2>/dev/null)" ]; then
  echo "FAIL: could not make the scratch commits (or they reached the repository's objects)"; fail=1
fi
cfg_changed='kani/\(all but local, \*\.md\) changed \(projects/shared-pool/kani/\.cargo/config\.toml\)'
expect 1 "a new file under kani/ (.cargo/config.toml): stale" "$cfg_changed; nothing in this run verifies.*re-run the local Kani record" \
  at "$c_cfg" run_check "$t/good"
expect 0 "stale with --stale-ok (kani-large and kani-mutants-large run)" "warning: .*$cfg_changed.*re-run the local Kani record" \
  at "$c_cfg" run_check "$t/good" "" "" --stale-ok
expect 0 "stale with --stale-ok-harnesses --stale-ok-mutants" "warning: .*$cfg_changed" \
  at "$c_cfg" run_check "$t/good" "" "" --stale-ok-harnesses --stale-ok-mutants
expect 1 "stale with --stale-ok-harnesses only (no kani-mutants-large)" "verifies its [0-9]+ mutants \(no kani-mutants-large jobs\);.*a manual run with .kani. and .mutants. verifies them" \
  at "$c_cfg" run_check "$t/good" "" "" --stale-ok-harnesses
expect 1 "stale with --stale-ok-mutants only (no kani-large)" "verifies its [0-9]+ harnesses \(no kani-large jobs\);" \
  at "$c_cfg" run_check "$t/good" "" "" --stale-ok-mutants
expect 1 "a .cargo/config.toml at the root (above kani/): stale" '\.cargo/config\[\.toml\] above kani/ \(the root, projects, projects/shared-pool\) changed \(\.cargo/config\.toml\)' \
  at "$c_root" run_check "$t/good"
[ "$(at "$c_root" "$check" cargo-configs "$c_root")" = .cargo/config.toml ] && [ -z "$("$check" cargo-configs HEAD)" ] ||
  { echo "FAIL: cargo-configs: not the commit's .cargo/config[.toml] above kani/"; fail=1; }
expect 1 "kani/run.sh changed: stale" "kani/run.sh, projects/shared-pool/kani/\(all but local, \*\.md\) changed \(projects/shared-pool/kani/run\.sh\)" \
  at "$c_run" run_check "$t/good"
expect 0 "only the Markdown and kani/local changed: current" "local Kani record ok: .*fingerprint [0-9a-f]{12};" \
  at "$c_doc" run_check "$t/good"
expect 2 "an unknown flag" "" run_check "$t/good" "" "" --stale-ok-everything

# Provenance that is not its commit's: HEAD's input lines and fingerprint
# under another commit, and input lines changed with a recomputed
# fingerprint. A commit this clone lacks (a shallow checkout) is skipped.
bad; edit "$t/r/provenance.txt" "s = re.sub(r'(?m)^commit\t.*$', 'commit\t$c_cfg', s)"
expect 1 "another commit's provenance" "the fingerprint is not commit [0-9a-f]{12}'s" at HEAD run_check "$t/r"
bad; edit "$t/r/provenance.txt" "s = re.sub(r'(?m)^(input\tprojects/shared-pool/kani/run.sh\t).*$', r'\g<1>' + '1' * 40, s)"
py -c "
import hashlib, re, sys
p = sys.argv[1]; s = open(p).read()
lines = re.findall(r'(?m)^input\t(.*)$', s)
fp = hashlib.sha256(''.join(l + '\n' for l in lines).encode()).hexdigest()
open(p, 'w').write(re.sub(r'(?m)^fingerprint\t.*$', 'fingerprint\t' + fp, s))" "$t/r/provenance.txt"
expect 1 "input lines changed and re-hashed by hand" "the fingerprint is not commit [0-9a-f]{12}'s" run_check "$t/r"
bad; edit "$t/r/provenance.txt" "s = re.sub(r'(?m)^commit\t.*$', 'commit\t' + 'e' * 40, s)"
expect 0 "a commit this clone lacks (shallow checkout)" "local Kani record ok: commit eeeeeeeeeeee" run_check "$t/r"
bad; edit "$t/r/provenance.txt" "s = re.sub(r'(?m)^commit\t.*$', 'commit\tHEAD', s)"
expect 1 "a commit that is not a full id" "commit 'HEAD' is not a full commit id" run_check "$t/r"
bad; edit "$t/r/provenance.txt" "s = re.sub(r'(?m)^(input\ttools/versions.env:CBMC\t).*$', r'\g<1>9.9.9', s)"
expect 1 "fingerprint edited by hand" "not the hash of its input lines" run_check "$t/r"
bad; edit "$t/r/jobs.tsv" "s = s.replace('\th_wedge\tok\t', '\th_wedge\tFAILED\t')"
expect 1 "results edited by hand" "not what record.py wrote" run_check "$t/r"
redigest "$t/r"
expect 1 "a recorded harness not ok" "recorded harness h_wedge: FAILED, not ok" run_check "$t/r"
bad; edit "$t/r/jobs.tsv" "s = re.sub(r'(?m)^(mutant\t$ML-\S+\t)killed/killed/killed', r'\1killed/killed/survived', s)"
redigest "$t/r"
expect 1 "a recorded mutant not as expected" "recorded mutant $ML-.*killed/killed/survived, not killed/killed/killed" run_check "$t/r"
bad; edit "$t/r/jobs.tsv" "s = re.sub(r'(?m)^harness\th_wedge\t.*\n', '', s)"; redigest "$t/r"
expect 1 "a large harness missing from the record" "only in kani-large: h_wedge" run_check "$t/r"
bad; edit "$t/r/covers.tsv" "s = s + 'fam\tcover\tSATISFIED\th_nobody\n'"; redigest "$t/r"
expect 1 "a cover of no recorded harness" "h_nobody, which is not a recorded harness" run_check "$t/r"

w() { cp "$wf" "$t/wf.yml"; edit "$t/wf.yml" "$1"; }
kg=$(ent group "$KG" "$KGGB" "$KGTO") rest=${KG#"$KH" }
w "s = s.replace('$kg', '$(ent group "$KH h_wedge $rest" "$KGGB" "$KGTO")', 1)"
expect 1 "a recorded harness also in a kani group" "in a kani group and in the local record: h_wedge" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$kg', '$(ent group "$rest" "$KGGB" "$KGTO")', 1)"
expect 1 "a harness in no group nor the record" "in no kani group and not in the local record: $KH\$" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$kg', '$(ent group "$KH $KG" "$KGGB" "$KGTO")', 1)"
expect 1 "a harness in two kani groups" "in two kani groups: $KH\$" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$kg', '$(ent group "$KH h_nonesuch $rest" "$KGGB" "$KGTO")', 1)"
expect 1 "not a harness of run.sh" "not harnesses of run.sh list: h_nonesuch" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$kg', '$(ent group "$KG" 14 "$KGTO")', 1)"
expect 1 "a kani group above 13 GB" "peaked at 14 GB, above 13 GB" run_check "$t/good" "$t/wf.yml"
wg=$(ent group "$WG" "$WGGB" "$WGTO")
w "s = re.sub(r'(?m)^ *- ' + re.escape('$wg') + r'\n', '', s)"
expect 1 "kani-large without a recorded group" "only recorded: ([a-z0-9_]+ )*h_wedge( |\$)" run_check "$t/good" "$t/wf.yml"
mc=$(ent mutant "$MC" "$MCGB" "$MCTO") ml=$(ent mutant "$ML" "$MLGB" "$MLTO")
w "s = s.replace('$mc', '$mc\n          - $ml', 1)"
expect 1 "a recorded mutant also in kani-mutants" "mutants in kani-mutants and in the local record: $MLP\$" run_check "$t/good" "$t/wf.yml"
w "s = re.sub(r'(?m)^ *- ' + re.escape('$mc') + r'\n', '', s, count=1)"
expect 1 "a mutant in neither" "in neither kani-mutants nor the local record: $MC-" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$mc', '$(ent mutant 99 "$MCGB" "$MCTO")', 1)"
expect 1 "a mutant entry with no patch" "entry 99: matches 0 mutants" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('$mc', '{ mutant: \"$MC\", gb: $MCGB }', 1)"
expect 1 "a matrix entry in another form" "a matrix entry must read" run_check "$t/good" "$t/wf.yml"
w "s = s.replace('  kani-large:', '  kani-huge:', 1)"
expect 1 "no kani-large job" "no matrix entries for the kani-large job" run_check "$t/good" "$t/wf.yml"
# Timeouts: a large entry's may not drop below the limit_s its recorded
# run used (the record shows a pass within that, not within less); a
# higher one is fine; under the job's --stale-ok flag (the job runs) a
# lower one is a warning. The fixture ran each job under its entry's
# timeout.
wnames=$(echo "$WG" | sed 's/ /, /g')
w "s = s.replace('$wg', '$(ent group "$WG" "$WGGB" 600)', 1)"
expect 1 "a kani-large timeout below the recorded limit" \
  "error: kani-large entry '$WG': timeout 600 is below the $WGTO s the local record ran $wnames under; the record does not show that (it passes|they pass) in 600 s: raise it back to $WGTO or more, or re-run the local Kani record" \
  run_check "$t/good" "$t/wf.yml"
expect 0 "... with --stale-ok-harnesses: a warning" \
  "warning: kani-large entry '$WG': timeout 600 is below the $WGTO s .*this run's kani-large job verifies (it|them) in 600 s" \
  run_check "$t/good" "$t/wf.yml" "" --stale-ok-harnesses
if [ -n "$TWO" ]; then
  w "s = s.replace('$(ent group "$TWO" "$TWOGB" "$TWOTO")', '$(ent group "$TWO" "$TWOGB" $((TWOTO - 1)))', 1)"
  expect 1 "a kani-large group of two or more harnesses 1 s below the recorded limit" \
    "kani-large entry '$TWO': timeout $((TWOTO - 1)) is below the $TWOTO s the local record ran $(echo "$TWO" | sed 's/ /, /g') under; the record does not show that they pass" \
    run_check "$t/good" "$t/wf.yml"
else
  echo "skip: a kani-large group of two or more harnesses below its limit (every kani-large group in $wf has one harness)"
fi
w "s = s.replace('$ml', '$(ent mutant "$ML" "$MLGB" 600)', 1)"
expect 1 "a kani-mutants-large timeout below the recorded limit" \
  "error: kani-mutants-large entry '$ML': timeout 600 is below the $MLTO s the local record ran $MLP under" \
  run_check "$t/good" "$t/wf.yml"
expect 1 "... with --stale-ok-harnesses only (no kani-mutants-large)" "error: kani-mutants-large entry '$ML': timeout 600" \
  run_check "$t/good" "$t/wf.yml" "" --stale-ok-harnesses
expect 0 "... with --stale-ok-mutants: a warning" "warning: kani-mutants-large entry '$ML': timeout 600 .*verifies it in 600 s" \
  run_check "$t/good" "$t/wf.yml" "" --stale-ok-mutants
w "s = s.replace('$wg', '$(ent group "$WG" "$WGGB" $((WGTO + 1800)))', 1).replace('$ml', '$(ent mutant "$ML" "$MLGB" $((MLTO + 1800)))', 1)"
expect 0 "large timeouts raised above the recorded limits" "local Kani record ok" run_check "$t/good" "$t/wf.yml"
grep -q '^warning: kani-' "$t/out" && { echo "FAIL: large timeouts raised: a timeout warning"; sed 's/^/  /' "$t/out"; fail=1; }
bad; edit "$t/r/jobs.tsv" "s = re.sub(r'(?m)^(harness\th_wedge\t(?:[^\t]*\t){5})$WGTO\t', r'\g<1>30m\t', s)"; redigest "$t/r"
expect 1 "a recorded limit_s that is not whole seconds" "recorded harness h_wedge: limit_s '30m' is not a whole number of seconds" \
  run_check "$t/r"

expect 0 "RESULTS.md block current" "" run_check "$t/good" "$wf" "$t/results.md"
cp "$t/results.md" "$t/stale.md"; edit "$t/stale.md" "s = s.replace('| \`h_wedge\` | ok |', '| \`h_wedge\` | ok?? |')"
expect 1 "RESULTS.md block stale" "not the record's table" run_check "$t/good" "$wf" "$t/stale.md"
expect 1 "RESULTS.md without the block" "has no local-record block" run_check "$t/good" "$wf" "$t/noblock.md"
expect 1 "no RESULTS.md" "none.md does not exist" run_check "$t/good" "$wf" "$t/none.md"

# Subcommands the covers job uses.
want=$("$check" matrix kani-large | cut -f1 | tr ' ' '\n' | sort)
got=$(KANI_RECORD_DIR="$t/good" "$check" harnesses | sort)
[ "$got" = "$want" ] || { echo "FAIL: harnesses: not kani-large's harnesses"; fail=1; }
KANI_RECORD_DIR="$t/good" "$check" covers >"$t/covers.tsv"
awk -F'\t' 'NF != 3 {bad=1} END {exit bad}' "$t/covers.tsv" && [ "$(wc -l <"$t/covers.tsv")" -eq "$(echo "$want" | wc -l)" ] &&
  "$here/check-covers.sh" "$t/covers.tsv" >/dev/null ||
  { echo "FAIL: covers: not run.sh's covers.tsv format, or check-covers.sh rejects it"; fail=1; }

# The fingerprint: each named input's git object id, the kani/ tree less
# local/ and the Markdown, the .cargo/config[.toml] files above kani/ (none,
# then one), and versions.env's KANI and CBMC, in a repository with only
# those paths. One input changed changes its line(s) and the fingerprint
# only; a new file under kani/ changes the kani/ line; the Markdown and
# kani/local change nothing.
g="$t/repo"
mkdir -p "$g/projects/shared-pool/kani/src" "$g/projects/shared-pool/kani/mutants" "$g/projects/shared-pool/conformance/traces" "$g/tools"
( cd "$g" && git init -q && git config user.email t@example.invalid && git config user.name t &&
  echo a >projects/shared-pool/kani/src/lib.rs && echo m >projects/shared-pool/kani/mutants/01-x.patch &&
  echo r >projects/shared-pool/kani/run.sh && echo c >projects/shared-pool/kani/Cargo.toml &&
  echo l >projects/shared-pool/kani/Cargo.lock && echo /target >projects/shared-pool/kani/.gitignore && echo t >projects/shared-pool/conformance/traces/core.txt &&
  echo results >projects/shared-pool/kani/RESULTS.md &&
  printf 'RUST_TOOLCHAIN=1\nKANI=0.68.0\nCBMC=6.11.0\n' >tools/versions.env &&
  git add -A && git update-index --add --cacheinfo "160000,$(printf '%040d' 1),projects/shared-pool/source-mirror" &&
  git commit -qm one && echo r2 >projects/shared-pool/kani/run.sh && git commit -qm two projects/shared-pool/kani/run.sh &&
  sed -i.bak 's/^KANI=.*/KANI=0.69.0/' tools/versions.env && rm tools/versions.env.bak && git commit -qm three tools/versions.env &&
  mkdir projects/shared-pool/kani/.cargo && echo '[build]' >projects/shared-pool/kani/.cargo/config.toml &&
  git add projects/shared-pool/kani/.cargo && git commit -qm four &&
  echo more >>projects/shared-pool/kani/RESULTS.md && mkdir projects/shared-pool/kani/local &&
  echo record >projects/shared-pool/kani/local/provenance.txt && git add projects/shared-pool/kani && git commit -qm five &&
  mkdir projects/shared-pool/.cargo && echo '[build]' >projects/shared-pool/.cargo/config &&
  git add projects/shared-pool/.cargo && git commit -qm six ) ||
  { echo "FAIL: could not build the fingerprint repository"; fail=1; }
fp() { KANI_RECORD_ROOT="$g" "$check" fingerprint "$1"; }
changed() { diff <(fp "$1") <(fp "$2") | grep '^>' | cut -f1 | tr '\n' ' '; }
all='projects/shared-pool/kani/(all but local, *.md)'
cargo='.cargo/config[.toml] above kani/ (the root, projects, projects/shared-pool)'
tab=$(printf '\t')
if [ "$(changed HEAD~5 HEAD~4)" != "> projects/shared-pool/kani/run.sh > $all > fingerprint " ] ||
   [ "$(changed HEAD~4 HEAD~3)" != "> tools/versions.env:KANI > fingerprint " ] ||
   [ "$(changed HEAD~3 HEAD~2)" != "> $all > fingerprint " ] || [ -n "$(changed HEAD~2 HEAD~1)" ] ||
   [ "$(changed HEAD~1 HEAD)" != "> $cargo > fingerprint " ] ||
   [ "$(fp HEAD~5 | grep -c .)" != 13 ] || ! fp HEAD~1 | grep -qxF "$cargo${tab}none" ||
   ! fp HEAD | grep -qx "projects/shared-pool/source-mirror$tab$(printf '%040d' 1)"; then
  echo "FAIL: fingerprint: one changed input must change its line(s) and the fingerprint only"
  for n in 5 4 3 2 1; do echo "  HEAD~$n to HEAD~$((n - 1)): $(changed "HEAD~$n" "HEAD~$((n - 1))")"; done
  fp HEAD~5 | sed 's/^/  /'; fail=1
fi
expect 1 "fingerprint of a missing input" "is not in the repository" env KANI_RECORD_ROOT="$t" "$check" fingerprint HEAD

# --- run-local.py's refusals (--dry-run checks as a run does) ---------------
# The environment, and the cargo configuration outside the lanes (CARGO_HOME
# here, and a .cargo/ above the run directory), parsed as TOML: every
# spelling of rustflags, rustc or a wrapper, a profile, or an include is
# refused, and so is a file that does not parse; other settings are not.
runlocal="$root/projects/shared-pool/kani/local/run-local.py"
mkdir -p "$t/ch" "$t/tmpdir" "$t/above/.cargo"
dry() { # dry <run dir> [VAR=value...]: a dry run in a clean environment
  local out=$1; shift
  env -i PATH="$PATH" HOME="$HOME" CARGO_HOME="$t/ch" TMPDIR="$t/tmpdir" "$@" python3 -I "$runlocal" --dry-run --out "$out"
}
ok_dry='^commit [0-9a-f]{12}; [0-9]+ jobs'
cfg() { printf '%s\n' "$2" >"$t/ch/$1"; }
expect 0 "run-local.py: no cargo configuration" "$ok_dry" dry "$t/lr"
while IFS='|' read -r name keys text; do
  cfg config.toml "$(printf '%b' "$text")"
  expect 1 "run-local.py: $name" "ch/config\.toml \($keys\) set rustflags, rustc or a wrapper, a profile or an include" \
    dry "$t/lr"
done <<'CASES'
a quoted key in [build]|build\.rustflags|[build]\n"rustflags" = ["-C", "overflow-checks=off"]
a dotted key|build\.rustflags|build.rustflags = ["-C", "overflow-checks=off"]
an inline table|build\.rustflags|build = { rustflags = ["-C", "overflow-checks=off"] }
a target's rustflags|target\."cfg\(unix\)"\.rustflags|[target.'cfg(unix)']\nrustflags = ["-C", "overflow-checks=off"]
a quoted rustc-wrapper|build\.rustc-wrapper|[build]\n'rustc-wrapper' = "sccache"
build.rustc|build\.rustc|[build]\nrustc = "/somewhere/rustc"
a dotted profile key|profile|profile.dev.opt-level = 0
a profile table|profile|[profile.release]\noverflow-checks = false
an include|include|include = "other.toml"
CASES
cfg config.toml '[build'
expect 1 "run-local.py: a cargo configuration that does not parse" "unparsable cargo configuration .*ch/config\.toml" dry "$t/lr"
cfg config.toml '[net]
offline = true
[build]
jobs = 2
[env]
RUST_LOG = "rustflags"'
expect 0 "run-local.py: other cargo settings" "$ok_dry" dry "$t/lr"
rm "$t/ch/config.toml"; cfg config '[build]
"rustflags" = ["-C", "overflow-checks=off"]'
expect 1 "run-local.py: CARGO_HOME/config (no .toml)" "ch/config \(build\.rustflags\) set" dry "$t/lr"
rm "$t/ch/config"; printf '[profile.dev]\noverflow-checks = false\n' >"$t/above/.cargo/config.toml"
expect 1 "run-local.py: a .cargo/config.toml above the run directory" "above/\.cargo/config\.toml \(profile\) set" \
  dry "$t/above/lr"
expect 0 "run-local.py: ... not above another run directory" "$ok_dry" dry "$t/lr"
expect 1 "run-local.py: RUSTFLAGS in the environment" "RUSTFLAGS set in the environment" dry "$t/lr" RUSTFLAGS=-Coverflow-checks=off

# --- run-local.py's jobs and --all -------------------------------------------
# The record's jobs are the large matrix entries. --all, a measurement,
# runs every harness of run.sh list and every mutant, each once, with an
# estimate and its source; it is not a record, writes outside the
# repository, and holds the machine's heavy-job lock while it runs (here a
# lock in $t: KANI_CBMC=/usr/bin/false fails the tool check after it is
# taken, so no job starts). A run in the background is a plain command, so
# that $! is run-local.py's pid (a shell function would run in a subshell,
# which the test's signal would stop instead).
kd="$root/projects/shared-pool/kani"
names() { awk '/^  [^ ]/ {print $1}' "$t/out" | sort; }
clean_env=(env -i PATH="$PATH" HOME="$HOME" TMPDIR="$t" SP_HEAVY_LOCK="$t/lock" KANI_CBMC=/usr/bin/false)
clean() { "${clean_env[@]}" "$@"; }
expect 0 "run-local.py --dry-run" "jobs \(KANI_TIMEOUT" py "$runlocal" --dry-run
want=$( { "$check" matrix kani-large | cut -f1 | tr ' ' '\n'; "$check" matrix kani-mutants-large | cut -f1 | sed 's/^/mutant-/'; } | sort)
[ "$(names)" = "$want" ] || { echo "FAIL: run-local.py --dry-run: not the kani-large harnesses and kani-mutants-large mutants"; fail=1; }
expect 0 "run-local.py --all --dry-run" "measurement \(--all, not a record\)" py "$runlocal" --all --dry-run
want=$( { bash "$kd/run.sh" list; for p in "$kd"/mutants/*.patch; do p=${p##*/}; echo "mutant-${p%%-*}"; done; } | sort)
if [ "$(names)" != "$want" ] || [ -n "$(names | uniq -d)" ]; then
  echo "FAIL: run-local.py --all --dry-run: not each harness of run.sh list and each mutant exactly once"
  diff <(names) <(echo "$want") | sed 's/^/  /'; fail=1
fi
grep -E '^  [^ ]' "$t/out" | grep -vE ' s  (W7|v1\.0\.0 RESULTS\.md|RESULTS\.md at|killed-by|checked-by|unknown)' | grep -q . &&
  { echo "FAIL: run-local.py --all --dry-run: an estimate without its source"; fail=1; }
expect 2 "run-local.py --all --record" "a measurement, not a record: --record is refused" py "$runlocal" --all --record
expect 2 "run-local.py --only without --all" "--only needs --all" py "$runlocal" --only h_wedge --dry-run
expect 2 "run-local.py --all --only, not a job" "not a job of --all: h_nonesuch" py "$runlocal" --all --only h_wedge,h_nonesuch --dry-run
expect 0 "run-local.py --all --only" "2 jobs, 1 harnesses of run.sh list and 1 mutants" py "$runlocal" --all --only h_wedge,01 --dry-run
[ "$(names | tr '\n' ' ')" = "h_wedge mutant-01 " ] || { echo "FAIL: run-local.py --all --only h_wedge,01: $(names | tr '\n' ' ')"; fail=1; }
expect 1 "run-local.py --all, output in the repository" "is in the repository; a measurement is not a record" \
  clean python3 -I "$runlocal" --all --only h_full_256 --out "$kd/target/measure"
expect 1 "run-local.py --all, a failure with the lock held" "holding $t/lock" \
  clean python3 -I "$runlocal" --all --only h_full_256 --out "$t/measure"
grep -q "released $t/lock" "$t/out" && [ ! -e "$t/lock" ] && [ ! -e "$t/measure" ] ||
  { echo "FAIL: run-local.py --all: the lock not released on failure (or the run started)"; sed 's/^/  /' "$t/out"; fail=1; }
# Another process's lock: --all waits for it, and leaves it when stopped.
mkdir "$t/lock"
"${clean_env[@]}" python3 -I "$runlocal" --all --only h_full_256 --out "$t/measure" >"$t/wait.out" 2>&1 &
pid=$!
for _ in $(seq 40); do grep -q "waiting for $t/lock" "$t/wait.out" && break; sleep 0.25; done
kill -TERM "$pid" 2>/dev/null; wait "$pid"; rc=$?
[ "$rc" = 143 ] && grep -q "waiting for $t/lock" "$t/wait.out" && grep -q "stopped by signal 15" "$t/wait.out" &&
  [ -d "$t/lock" ] && ! grep -q "holding" "$t/wait.out" ||
  { echo "FAIL: run-local.py --all: did not wait for another's lock, or removed it (exit $rc)"; sed 's/^/  /' "$t/wait.out"; fail=1; }
rmdir "$t/lock"

# --all's scheduling: a job estimated at ALONE_GB or more starts only when
# no job runs, and none starts beside it, nor beside a running job whose
# sampled tree reached ALONE_GB; other jobs share the budget. The record
# (alone None) keeps the budget rule alone. The dry run says so.
expect 0 "run-local.py --all: a job of ALONE_GB or more runs alone" "^ok$" py - "$runlocal" <<'PY'
import importlib.util, sys
sys.dont_write_bytecode = True  # no __pycache__ in kani/local
spec = importlib.util.spec_from_file_location("run_local", sys.argv[1])
rl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rl)
A = rl.ALONE_GB
def job(gb, secs=0):
    return {"peak": gb, "est": secs}
bad = [c for c, want in [
    ((job(40), [], 50, A), True),            # alone, it fits
    ((job(40), [3], 50, A), False),          # not beside a running job
    ((job(A), [3], 50, A), False),
    ((job(3), [40], 50, A), False),          # nothing beside it
    ((job(3), [A], 50, A), False),           # nor beside a tree that reached A
    ((job(A - 0.1), [3], 50, A), True),      # below A: the budget
    ((job(10), [39.9 - 10], 50, A), True),
    ((job(10), [30, 10.1], 50, A), False),
    ((job(10), [40], 50, None), True),       # the record: the budget only
    ((job(10.1), [40], 50, None), False),
] if rl.fits(*c) != want]
# 40 GB for 100 s, then the two small jobs together: 110 s (100 s beside it).
jobs = [job(40, 100), job(3, 10), job(3, 10)]
sim = (rl.simulate(jobs, 2, 50, A), rl.simulate(jobs, 2, 50, None))
print("ok" if not bad and sim == (110, 100) and A == 35 else "wrong: fits %s, simulate %s, ALONE_GB %s" % (bad, sim, A))
PY
expect 0 "run-local.py --all --dry-run: the alone line" "^alone \(estimated at 35 GB or more; no job beside them\): [0-9]+ jobs" \
  py "$runlocal" --all --dry-run

# A run whose jobs run, without Kani: stand-ins for cargo (cargo kani),
# rustc and CBMC, first on PATH and in run.sh's $HOME/.cargo/bin. Each job
# leaves a process in its group and one in a session of its own (run.sh
# says cargo-kani once started CBMC so; the group kill misses it). Stopped
# by SIGTERM, or failing on an exception (here progress.log turned into a
# directory), while a job runs, the run kills both and then releases the
# lock. Needs the source-mirror at the core pin for the lanes: CI's
# checkout has no submodule, so this is skipped there.
core=$(git -C "$root" rev-parse HEAD:projects/shared-pool/source-mirror)
if git -C "$root/projects/shared-pool/source-mirror" cat-file -e "$core^{commit}" 2>/dev/null; then
  fh="$t/fakehome"; fb="$t/fakehome/.cargo/bin"; pids="$t/pids"
  mkdir -p "$fb" "$pids"
  ver() { git -C "$root" show HEAD:tools/versions.env | sed -n "s/^$1=//p"; }
  printf '#!/bin/sh\necho "%s (stand-in)"\n' "$(ver CBMC)" >"$fb/cbmc"
  printf '#!/bin/sh\necho "rustc %s (stand-in)"\n' "$(ver RUST_TOOLCHAIN)" >"$fb/rustc"
  { echo '#!/usr/bin/env bash'
    echo "kv='$(ver KANI)' pids='$pids'"
    cat <<'SH'
case " $* " in
  *" --version "*) echo "cargo-kani $kv"; exit 0 ;;
  *" --only-codegen "*) exit 0 ;;
esac
prev=
for x in "$@"; do [ "$prev" = --harness ] && h=${x#*::}; prev=$x; done
if [ "$h" = h_step_ack_tx ] && [ -e "$pids/crash" ]; then
  # Once the other job runs: the scheduler's next log line raises.
  until [ -s "$pids/h_step_pause_tx.own" ]; do sleep 0.1; done
  out=$(dirname "$(dirname "$KANI_LOG_DIR")")
  rm "$out/progress.log"; mkdir "$out/progress.log"
  exit 1
fi
sleep 120 &
echo $! >"$pids/$h.group"
# Its pid file is written once it is in a session of its own.
python3 -c 'import os, sys, time; os.setsid(); open(sys.argv[1], "w").write(str(os.getpid())); time.sleep(120)' \
  "$pids/$h.own" &
wait
SH
  } >"$fb/cargo"
  chmod +x "$fb/cargo" "$fb/rustc" "$fb/cbmc"
  fake_env=(env -i PATH="$fb:$PATH" HOME="$fh" TMPDIR="$t" SP_HEAVY_LOCK="$t/lock" KANI_CBMC="$fb/cbmc")
  alive() { local s; s=$(ps -o stat= -p "$1" 2>/dev/null | tr -d ' '); [ -n "$s" ] && [ "${s#Z}" = "$s" ]; }
  # killed <harness>: its two processes are gone (within 5 s: reaped).
  killed() {
    local f p
    for f in group own; do
      [ -s "$pids/$1.$f" ] || return 1
      p=$(cat "$pids/$1.$f")
      for _ in $(seq 20); do alive "$p" || continue 2; sleep 0.25; done
      return 1
    done
  }
  # sweep: whatever a failed case left of the stand-ins' sleepers.
  sweep() {
    local f p
    for f in "$pids"/*.group "$pids"/*.own; do
      [ -s "$f" ] || continue
      p=$(cat "$f")
      ps -o command= -p "$p" 2>/dev/null | grep -q sleep && kill -KILL "$p" 2>/dev/null
    done
    rm -f "$pids"/*
  }
  "${fake_env[@]}" python3 -I "$runlocal" --all --only h_step_ack_tx --lanes 1 --out "$t/m1" >"$t/m1.out" 2>&1 &
  pid=$!
  for _ in $(seq 240); do [ -s "$pids/h_step_ack_tx.own" ] && break; kill -0 "$pid" 2>/dev/null || break; sleep 0.25; done
  kill -TERM "$pid" 2>/dev/null; wait "$pid"; rc=$?
  if [ "$rc" != 1 ] || ! grep -q "stopped by signal 15; not complete" "$t/m1.out" ||
     ! grep -q "released $t/lock" "$t/m1.out" || [ -e "$t/lock" ] || ! killed h_step_ack_tx; then
    echo "FAIL: run-local.py --all: SIGTERM with a job running: exit $rc, its processes or the lock left"
    sed 's/^/  /' "$t/m1.out"; ls "$pids" | sed 's/^/  pids: /'; fail=1
  fi
  sweep; rmdir "$t/lock" 2>/dev/null
  touch "$pids/crash"
  "${fake_env[@]}" python3 -I "$runlocal" --all --only h_step_ack_tx,h_step_pause_tx --lanes 2 --out "$t/m2" \
    >"$t/m2.out" 2>&1 &
  pid=$!
  for _ in $(seq 240); do kill -0 "$pid" 2>/dev/null || break; sleep 0.25; done
  kill -0 "$pid" 2>/dev/null && kill -TERM "$pid"
  wait "$pid"; rc=$?
  if [ "$rc" != 1 ] || ! grep -q "^Traceback" "$t/m2.out" || ! grep -q "released $t/lock" "$t/m2.out" ||
     [ -e "$t/lock" ] || ! killed h_step_pause_tx; then
    echo "FAIL: run-local.py --all: an exception with a job running: exit $rc, its processes or the lock left"
    sed 's/^/  /' "$t/m2.out"; ls "$pids" | sed 's/^/  pids: /'; fail=1
  fi
  sweep; rmdir "$t/lock" 2>/dev/null
else
  echo "skip: run-local.py --all with running jobs (no source-mirror at $core, as in CI's checkout)"
fi

[ "$fail" = 0 ] && echo "check-kani-record: all cases pass"
exit "$fail"
