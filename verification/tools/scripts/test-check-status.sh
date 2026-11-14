#!/usr/bin/env bash
# Self-test for check-status.sh: the real status files pass, and each kind of
# overclaim or disagreement fails for exactly its own reason (one problem
# line, matching the case's pattern).
#   - expect: the overclaim and findings cases edit a scratch copy of
#     near-approval's status.toml (read only). near-approval has no
#     [findings], so each findings case adds a [findings.F-T] table and
#     points rows at it. It has no [project] catalogue either.
#   - expect_sp: the catalogue cases (plan-meta section 4.5) edit scratch
#     copies of shared-pool's status.toml and strategy.md, side by side, so
#     its `catalogue = "strategy.md"` names the edited copy. Each case sets
#     the cells and statuses it relies on, or uses a row that is not Kani's,
#     so the v1.1.0 Kani record (which rewrites the Kani rows) leaves the
#     cases valid. They need shared-pool's files to pass the lint as they
#     are, with the catalogue check on (the first expect_sp case and its
#     summary line: without `catalogue =` the check is silently off).
# With an argument, only that project's cases run: near-approval.yml runs
# `near-approval`, so a shared-pool edit cannot fail near-approval's CI;
# shared-pool.yml runs every case (the generic rules are tested on
# near-approval's file).
#
# usage: tools/scripts/test-check-status.sh [near-approval|shared-pool]
set -uo pipefail
only=${1:-}
case "$only" in
  "" | near-approval | shared-pool) ;;
  *) echo "usage: test-check-status.sh [near-approval|shared-pool]" >&2; exit 2 ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
toml="$here/../../projects/near-approval/status.toml"
sp="$here/../../projects/shared-pool"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0

# run <want> <name> <project> <status file> [<pattern>]: check-status.sh's
# exit must be <want>; with a pattern, its output must hold exactly one
# problem line, and that line (without "status-lint: ") must match it (ERE).
run() {
  local want=$1 name=$2 project=$3 file=$4 pat=${5:-}
  STATUS_TOML="$file" "$here/check-status.sh" "$project" >"$tmp/out" 2>&1
  local got=$?
  [ "$got" -gt 1 ] && got=2
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name: exit $got, want $want"; sed 's/^/  /' "$tmp/out"; fail=1; return
  fi
  [ -n "$pat" ] || return 0
  sed -n 's/^\(::error::\)\{0,1\}status-lint: //p' "$tmp/out" | grep -Ev '^[0-9]+ properties, ' >"$tmp/problems"
  if [ "$(grep -c '' "$tmp/problems")" != 1 ] || ! grep -Eq -- "$pat" "$tmp/problems"; then
    echo "FAIL: $name: want exactly one problem matching /$pat/"; sed 's/^/  /' "$tmp/out"; fail=1
  fi
}

# expect <0|1> <name> <python edit of s, near-approval's status.toml> [<pattern>]
expect() {
  local want=$1 name=$2 edit=$3 pat=${4:-}
  [ "$only" != shared-pool ] || return 0
  if ! python3 -I -c "import sys
s=open(sys.argv[1]).read()
$edit
open(sys.argv[2],'w').write(s)" "$toml" "$tmp/s.toml" >"$tmp/out" 2>&1; then
    echo "FAIL: $name: the edit failed"; sed 's/^/  /' "$tmp/out"; fail=1; return
  fi
  run "$want" "$name" near-approval "$tmp/s.toml" "$pat"
}

expect 0 "current status.toml" "pass"
# A Kani property at verified-with-assumptions with no spec reviewer.
expect 1 "K property past in-progress without sign-off" \
  "import re; s=re.sub(r'(ROB-1 = \{ status = )\"[a-z-]+\"', r'\1\"verified-with-assumptions\"', s, count=1)" \
  '^ROB-1: verified-with-assumptions needs \[review\] spec_reviewer'
# ...and above the Kani ceiling even with a reviewer.
expect 1 "Kani at verified" \
  "import re; s=re.sub(r'spec_reviewer = \"\"', 'spec_reviewer = \"someone\"', s, count=1); s=re.sub(r'(ROB-1 = \{ status = )\"[a-z-]+\"', r'\1\"verified\"', s, count=1)" \
  '^ROB-1: Kani results are at most verified-with-assumptions$'
# Sign-off present: verified-with-assumptions is fine.
expect 0 "K property with sign-off" \
  "import re; s=re.sub(r'spec_reviewer = \"\"', 'spec_reviewer = \"someone\"', s, count=1); s=re.sub(r'(ROB-1 = \{ status = )\"[a-z-]+\"', r'\1\"verified-with-assumptions\"', s, count=1)"
# Evidence a layer can't give.
expect 1 "evidence not allowed for layer" \
  "import re; s=re.sub(r'(ROB-1 = \{[^}]*evidence = )\"bounded-proof\"', r'\1\"proof\"', s, count=1)" \
  "^ROB-1: evidence 'proof' not allowed for layer 'kani'"
# Verus path A: `verified` only with no assumptions left.
expect 1 "Verus verified with assumptions" \
  "import re; s=re.sub(r'spec_reviewer = \"\"', 'spec_reviewer = \"someone\"', s, count=1); s=re.sub(r'(TX-1 = \{ status = )\"[a-z-]+\"', r'\1\"verified\"', s, count=1)" \
  '^TX-1: verified with assumptions listed'
expect 0 "Verus verified-with-assumptions with sign-off" \
  "import re; s=re.sub(r'spec_reviewer = \"\"', 'spec_reviewer = \"someone\"', s, count=1); s=re.sub(r'(TX-1 = \{ status = )\"[a-z-]+\"', r'\1\"verified-with-assumptions\"', s, count=1)"
# A property claimed with no artifact behind it.
expect 1 "claimed without PROP tag" \
  "import re; s=s.replace('[modules.requirements.properties]', '[modules.requirements.properties]\n  ZZ-1 = { status = \"in-progress\", layer = \"kani\", evidence = \"bounded-proof\", description = \"x\" }', 1)" \
  '^ZZ-1: in-progress but no `// PROP: ZZ-1` tag'

# Findings (plan-meta section 4). fnd adds [findings.<fid>]; key adds a key
# to a property's inline table. ROB-1 and TX-1 are in-progress rows;
# ASM-SIGNER is an assumption.
H='import json, re
def fnd(status, by, closed_in="v1.1.0", fid="F-T"):
    global s
    body = "status = %s\nclosed_by = %s\n" % (json.dumps(status), json.dumps(by))
    if closed_in:
        body += "closed_in = %s\n" % json.dumps(closed_in)
    s = s.rstrip("\n") + "\n\n[findings.%s]\n" % fid + body
def key(pid, k, v):
    global s
    a = "  " + pid + " = { "
    assert s.count(a) == 1, pid
    s = s.replace(a, a + k + " = " + json.dumps(v) + ", ")
'
expect 0 "finding closed by an in-progress row" \
  "$H"'key("ROB-1", "closes", ["F-T"]); fnd("closed", ["ROB-1"])'
expect 0 "closed_by may also name an assumption" \
  "$H"'key("ROB-1", "closes", ["F-T"]); fnd("closed", ["ROB-1", "ASM-SIGNER"])'
expect 1 "closes names no [findings] table" \
  "$H"'key("ROB-1", "closes", ["F-T", "F-X"]); fnd("closed", ["ROB-1"])' \
  "^ROB-1: closes 'F-X', which is not a \[findings\] table$"
expect 1 "closes without any [findings]" \
  "$H"'key("ROB-1", "closes", ["F-T"])' \
  "^ROB-1: closes 'F-T', which is not a \[findings\] table$"
expect 1 "closed_by names an unknown ID" \
  "$H"'key("ROB-1", "closes", ["F-T"]); fnd("closed", ["ROB-1", "ZZ-9"])' \
  "^findings\.F-T: closed_by 'ZZ-9' is not a property or assumption$"
expect 1 "closed_by lists a row whose closes omits the finding" \
  "$H"'fnd("closed", ["ROB-1"])' \
  '^findings\.F-T: closed_by lists ROB-1, whose closes does not name F-T$'
expect 1 "closes not echoed in closed_by" \
  "$H"'key("ROB-1", "closes", ["F-T"]); key("TX-1", "closes", ["F-T"]); fnd("closed", ["TX-1"])' \
  '^ROB-1: closes F-T but \[findings\.F-T\] closed_by does not list ROB-1$'
expect 1 "closed with every closer at rank 0" \
  "$H"'s = s.replace("  ROB-1 = { status = \"in-progress\"", "  ROB-1 = { status = \"unverified\"", 1); key("ROB-1", "closes", ["F-T"]); fnd("closed", ["ROB-1"])' \
  '^findings\.F-T: closed, but every closed_by row is unverified or stale$'
expect 1 "closed with an empty closed_by" \
  "$H"'fnd("closed", [])' \
  '^findings\.F-T: closed with an empty closed_by$'
expect 1 "closed without closed_in" \
  "$H"'key("ROB-1", "closes", ["F-T"]); fnd("closed", ["ROB-1"], closed_in="")' \
  '^findings\.F-T: closed without closed_in$'
expect 0 "rejected with an empty closed_by" \
  "$H"'fnd("rejected", [], closed_in="")'
expect 1 "unknown finding status" \
  "$H"'fnd("fixed", [], closed_in="")' \
  "^findings\.F-T: unknown status 'fixed'"
expect 0 "finding names an open finding" \
  "$H"'key("TX-1", "finding", "F-T"); fnd("open", [], closed_in="")'
expect 1 "finding names a closed finding" \
  "$H"'key("ROB-1", "closes", ["F-T"]); key("TX-1", "finding", "F-T"); fnd("closed", ["ROB-1"])' \
  "^TX-1: finding F-T is 'closed', not open"
expect 1 "finding names no [findings] table" \
  "$H"'key("TX-1", "finding", "F-T, F-X"); fnd("open", [], closed_in="")' \
  "^TX-1: finding 'F-X' is not a \[findings\] table$"

# Catalogue (plan-meta section 4.5): shared-pool's status.toml against its
# strategy.md section 5. In the edit, s is status.toml's text and L is
# strategy.md's lines. Helpers: cell(id, col, text) sets a section 5 row's
# cell (2 is Layer(s), -1 Status); drop(id) deletes the row; after(id, line)
# adds a line after it; prop(id, key, value) sets a property's key; asm(id,
# key, value) an assumption's; move(id, module) moves a property's line.
expect_sp() {
  local want=$1 name=$2 edit=$3 pat=${4:-}
  [ "$only" != near-approval ] || return 0
  mkdir -p "$tmp/sp"
  if ! python3 -I -c 'import json, re, sys
s = open(sys.argv[1]).read()
L = open(sys.argv[2]).read().split("\n")
def row(rid):
    a = next(i for i, l in enumerate(L) if l.startswith("## 5."))
    b = next(i for i in range(a + 1, len(L)) if L[i].startswith("## "))
    k = [i for i in range(a, b) if L[i].startswith("| " + rid + " |")]
    assert len(k) == 1, rid
    return k[0]
def cell(rid, col, text):
    i = row(rid)
    c = [x.strip() for x in re.split(r"(?<!\\)\|", L[i].strip()[1:-1])]
    c[col] = text
    L[i] = "| " + " | ".join(c) + " |"
def drop(rid):
    del L[row(rid)]
def after(rid, line):
    L.insert(row(rid) + 1, line)
def span(rid):
    a = "\n  " + rid + " = { "
    assert s.count(a) == 1, rid
    i = s.index(a) + 1
    return i, s.index("\n", i) + 1
def prop(rid, k, v):
    global s
    i, j = span(rid)
    line, n = re.subn(r"\b" + k + r" = (\"[^\"]*\"|\[[^\]]*\])", k + " = " + json.dumps(v).replace("\\", "\\\\"), s[i:j], count=1)
    assert n == 1, (rid, k)
    s = s[:i] + line + s[j:]
def move(rid, mod):
    global s
    i, j = span(rid)
    line, s = s[i:j], s[:i] + s[j:]
    h = "[modules." + mod + ".properties]\n"
    assert s.count(h) == 1, mod
    k = s.index(h) + len(h)
    s = s[:k] + line + s[k:]
def asm(rid, k, v):
    global s
    h = "\n[assumptions." + rid + "]\n"
    assert s.count(h) == 1, rid
    i = s.index(h) + 1
    j = s.find("\n[", i)
    j = len(s) if j < 0 else j
    blk, n = re.subn(r"(?m)^" + k + r" = \"[^\"]*\"", k + " = " + json.dumps(v), s[i:j], count=1)
    assert n == 1, (rid, k)
    s = s[:i] + blk + s[j:]
'"$edit"'
open(sys.argv[3], "w").write(s)
open(sys.argv[4], "w").write("\n".join(L))' \
      "$sp/status.toml" "$sp/strategy.md" "$tmp/sp/status.toml" "$tmp/sp/strategy.md" >"$tmp/out" 2>&1; then
    echo "FAIL: $name: the edit failed"; sed 's/^/  /' "$tmp/out"; fail=1; return
  fi
  run "$want" "$name" shared-pool "$tmp/sp/status.toml" "$pat"
}

# sp_summary <name> <ERE>: the summary line (the last line) of the expect_sp
# case just run must match.
sp_summary() {
  [ "$only" != near-approval ] || return 0
  if ! tail -n 1 "$tmp/out" | grep -Eq -- "$2"; then
    echo "FAIL: $1: want a summary line matching /$2/"; sed 's/^/  /' "$tmp/out"; fail=1
  fi
}

expect_sp 0 "shared-pool status.toml agrees with strategy.md section 5" "pass"
# ...with the catalogue check on: deleting `catalogue =` from shared-pool's
# status.toml would turn it off and still pass.
sp_summary "shared-pool status.toml keeps [project] catalogue" \
  '^status-lint: [0-9]+ properties, [0-9]+ assumptions, [0-9]+ findings, [1-9][0-9]* strategy\.md section 5 rows, 0 problem\(s\)$'
# Without [project] catalogue nothing is compared (near-approval has none).
expect_sp 0 "no catalogue key, no catalogue check" \
  's = s.replace("\ncatalogue = \"strategy.md\"", "\n#", 1); drop("C-4")'
sp_summary "no catalogue key, no catalogue summary" \
  '^status-lint: [0-9]+ properties, [0-9]+ findings, 0 problem\(s\)$'
expect_sp 1 "catalogue file missing" \
  'assert "catalogue = \"strategy.md\"" in s; s = s.replace("catalogue = \"strategy.md\"", "catalogue = \"nope.md\"", 1)' \
  '^catalogue: nope\.md not found'
expect_sp 1 "no section 5" \
  'L[:] = [l.replace("## 5.", "## 5x.", 1) if l.startswith("## 5.") else l for l in L]' \
  '^catalogue: strategy\.md has no "## 5\." section$'
# Rows: missing, extra, repeated, not an ID; the same for assumptions.
expect_sp 1 "property row missing from section 5" \
  'drop("C-4")' \
  '^C-4: a status\.toml property with no strategy\.md section 5 property row$'
expect_sp 1 "extra section 5 property row" \
  'after("C-4", "| C-9 | x | prolog | x | checked |")' \
  '^C-9: strategy\.md:[0-9]+ is a section 5 property row with no status\.toml property$'
expect_sp 1 "section 5 row repeated" \
  'after("C-4", L[row("C-4")])' \
  '^C-4: 2 rows in strategy\.md section 5 \(lines [0-9]+, [0-9]+\)$'
expect_sp 1 "section 5 row without a bare ID" \
  'after("C-4", "| c-9 | x | prolog | x | checked |")' \
  "^catalogue: strategy\.md:[0-9]+: a section 5 row whose first cell 'c-9' is not a bare ID$"
expect_sp 1 "assumption row missing from section 5" \
  'drop("ASM-EXEC")' \
  '^ASM-EXEC: \[assumptions\.ASM-EXEC\] has no strategy\.md section 5 row with Layer\(s\) assumption$'
expect_sp 1 "extra section 5 assumption row" \
  'after("ASM-EXEC", "| ASM-ZZ | x | assumption | none | policy |")' \
  '^ASM-ZZ: strategy\.md:[0-9]+ is a section 5 assumption row with no \[assumptions\.ASM-ZZ\]$'
# Layer(s), cross and modules.
expect_sp 1 "layer is not the first entry of Layer(s)" \
  'cell("SH-2", 2, "sandbox")' \
  "^SH-2: layer 'unit', but strategy\.md:[0-9]+ Layer\(s\) 'sandbox' starts with 'sandbox'$"
# layer "review" has no artifact to check, so only the catalogue objects.
expect_sp 1 "near-tests is not unit, difftest or wasm-check" \
  'prop("SH-2", "layer", "review"); prop("SH-2", "evidence", "review")' \
  "^SH-2: layer 'review', but strategy\\.md:[0-9]+ Layer\\(s\\) 'near-tests' starts with 'near-tests' \\(unit, difftest or wasm-check\\)$"
expect_sp 1 "a Layer(s) entry missing from cross" \
  'cell("SH-2", 2, "near-tests, sandbox")' \
  "^SH-2: cross \[\] lacks 'sandbox' of strategy\.md:[0-9]+ Layer\(s\) 'near-tests, sandbox'$"
expect_sp 1 "property in another subsection's module" \
  'move("C-4", "tla_model")' \
  "^C-4: in \[modules\.tla_model\], but section 5\.10's rows are in \[modules\.requirements\] \(C-1\)$"
expect_sp 1 "property in two modules" \
  'i, j = span("C-4"); line = s[i:j]; h = "[modules.tla_model.properties]\n"; k = s.index(h) + len(h); s = s[:k] + line + s[k:]' \
  '^C-4: in \[modules\.requirements\] and \[modules\.tla_model\]$'
expect_sp 1 "two subsections in one module" \
  'move("REF-1", "lean_model"); move("REF-2", "lean_model"); move("REF-3", "lean_model")' \
  '^catalogue: \[modules\.lean_model\] holds sections 5\.12, 5\.13; one module per section 5 subsection$'
# Status against the Status cell (the mismatched cell).
expect_sp 1 "in-progress, Status cell stale" \
  'cell("WIN-LIVE", -1, "stale; v1.0.0: model-checked")' \
  "^WIN-LIVE: status 'in-progress', but strategy\.md:[0-9]+ Status 'stale; v1\.0\.0: model-checked' records a previous release's result, not re-checked at the pin: stale$"
expect_sp 1 "stale, Status cell a result at the pin" \
  'prop("SH-2", "status", "stale")' \
  "^SH-2: status 'stale', but strategy\.md:[0-9]+ Status 'tested' records a result at the pin: in-progress$"
expect_sp 1 "unverified, Status cell a previous result" \
  'cell("TLA-IND", -1, "stale; v1.0.0: model-checked")' \
  "^TLA-IND: status 'unverified', but .* records a previous release's result, not re-checked at the pin: stale$"
expect_sp 1 "verified-with-assumptions the Status cell does not name" \
  's = s.replace("spec_reviewer = \"\"", "spec_reviewer = \"someone\"", 1); prop("SH-2", "status", "verified-with-assumptions")' \
  "^SH-2: status 'verified-with-assumptions', but .* records a result at the pin: in-progress$"
expect_sp 0 "verified-with-assumptions the Status cell names" \
  's = s.replace("spec_reviewer = \"\"", "spec_reviewer = \"someone\"", 1); prop("SH-2", "status", "verified-with-assumptions"); cell("SH-2", -1, "tested; verified-with-assumptions")'
# The cell's markup and negations.
expect_sp 1 "in-progress, Status cell \"not tested\"" \
  'cell("SH-2", -1, "not tested")' \
  "^SH-2: status 'in-progress', but strategy\.md:[0-9]+ Status 'not tested' records no result: unverified$"
expect_sp 1 "in-progress, Status cell stale in markup" \
  'cell("WIN-LIVE", -1, "**stale**; v1.0.0: tested")' \
  "^WIN-LIVE: status 'in-progress', but .* records a previous release's result, not re-checked at the pin: stale$"
expect_sp 0 "markup around a result, a level and a word with _ in it" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "`bounded proof` (_Kani_, h_rob)"); cell("SH-2", -1, "*tested*"); cell("ASM-EXEC", -1, "**policy**")'
# Kani rows (ROB-1, Layer(s) kani only): what W13b's record must write.
expect_sp 1 "Kani row in-progress, Status cell Kani: pending (v1.0.0: ...)" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "Kani: pending (v1.0.0: bounded proof)")' \
  "^ROB-1: status 'in-progress', but .* records a previous release's result, not re-checked at the pin: stale$"
expect_sp 1 "Kani row in-progress, Status cell without bounded proof" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "tested (216 traces)")' \
  "^ROB-1: status 'in-progress', but .* records no Kani result \(\`bounded proof\`\): unverified$"
expect_sp 1 "a result inside a (v1.0.0: ...) bracket is not one" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "tested (v1.0.0: bounded proof (Kani))")' \
  "^ROB-1: status 'in-progress', but .* records no Kani result"
expect_sp 0 "Kani row in-progress, Status cell bounded proof" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "bounded proof (Kani)")'
expect_sp 0 "Kani: pending with no previous result allows unverified" \
  'prop("ROB-1", "status", "unverified"); cell("ROB-1", -1, "Kani: pending")'
expect_sp 1 "Kani: pending with a previous result needs stale" \
  'prop("ROB-1", "status", "unverified"); cell("ROB-1", -1, "Kani: pending (v1.0.0: bounded proof)")' \
  "^ROB-1: status 'unverified', but .* records a previous release's result, not re-checked at the pin: stale$"
expect_sp 1 "Kani row in-progress, Status cell no bounded proof" \
  'prop("ROB-1", "status", "in-progress"); cell("ROB-1", -1, "tested; no bounded proof yet")' \
  "^ROB-1: status 'in-progress', but .* records no Kani result \(\`bounded proof\`\): unverified$"
# Assumptions: status and level.
expect_sp 1 "assumption in-progress, Status cell policy" \
  'asm("ASM-EXEC", "status", "in-progress")' \
  "^ASM-EXEC: status 'in-progress', but strategy\.md:[0-9]+ Status 'policy' records no result: unverified$"
expect_sp 1 "assumption level not the Status cell's" \
  'asm("ASM-EXEC", "level", "high")' \
  "^ASM-EXEC: level 'high', but strategy\.md:[0-9]+ Status 'policy' says 'policy'$"

if [ "$fail" = 0 ]; then echo "check-status self-test ok${only:+ ($only cases only)}"; fi
exit "$fail"
