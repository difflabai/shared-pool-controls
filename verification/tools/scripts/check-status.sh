#!/usr/bin/env bash
# status-lint (strategy.md sections 12 and 14). Fails when status.toml claims
# more than the repository backs:
#   - `evidence` not allowed for the property's `layer`;
#   - a property past `unverified` whose artifact doesn't mention it: Rust
#     layers need a `// PROP: <ID>` tag in the layer's crate, other layers the
#     ID somewhere in the layer's directory;
#   - a status above the layer's ceiling (Kani: verified-with-assumptions;
#     only Verus path A may claim `verified` with evidence `proof`);
#   - a K or V property at verified* without `[review] spec_reviewer`.
# When the file has `[findings.<ID>]` tables (shared-pool, plan-meta section
# 4), it also checks them against the rows:
#   - a finding's `status` is open, closed, documented or rejected;
#   - every entry of a property's `closes` is a finding whose `closed_by`
#     lists the property, and every `closed_by` entry is a property or
#     assumption ID; a property in `closed_by` must list the finding in
#     `closes`;
#   - a closed finding has `closed_in` and a non-empty `closed_by` with at
#     least one row past `unverified`/`stale` (a rejected one may have none);
#   - a property's `finding` (comma-separated IDs) names open findings only.
# Without [findings], `closes` is an error and `finding` is free text.
# When `[project] catalogue = "<file>"` names the property catalogue
# (shared-pool: strategy.md, plan-meta section 4.5; the path is relative to
# status.toml), status.toml must match its section 5, whose tables C-6 reads
# too (prolog/verification/consistency.pl): rows are the markdown table rows
# from "## 5." to the next "## ", each table headed ID ... Layer(s) ... Status.
#   - the properties are exactly the rows whose Layer(s) is not `assumption`,
#     the [assumptions.<ID>] tables exactly the others, with no ID twice;
#   - `layer` is the first entry of Layer(s) (near-tests: unit, difftest or
#     wasm-check), and each other entry is in `cross`;
#   - one module per "### 5.N" subsection: a subsection's rows share one
#     [modules.<m>], and no two subsections share one;
#   - `status` is what the Status cell records for the row's primary layer
#     (an assumption's: any layer):
#       "stale; ..." first, or `Kani: pending (vX.Y.Z: ...)` on a Kani row,
#         is a previous release's result not re-checked: stale;
#       `Kani: pending` alone on a Kani row: unverified (new) or stale;
#       otherwise a result word outside "(vX.Y.Z: ...)" brackets (proved,
#         model-checked, tested, checked, bounded proof; a Kani row needs
#         `bounded proof`): in-progress, or a verified* status the cell
#         names; no result word (open, untested, policy): unverified;
#       the cell is read without markup (*, ` and _ outside a word), and a
#         result word right after "not", "no" or "never" ("not tested",
#         "no bounded proof") is no result;
#   - an assumption's `level` is the cell's "(high)", "(med)", "(low)" or
#     leading "policy".
# In-progress K/V properties are allowed without sign-off: they record that
# the proofs pass in CI while the human spec review is still pending.
#
# Whether a verified* property's harness passed "in this run" is the job of
# the jobs that run it (kani, covers, prolog, ...): this script only checks
# that the claim has an artifact behind it.
#
# usage: tools/scripts/check-status.sh <project>
#        STATUS_TOML=<file> ...   (lint another status file against the project's
#                                  artifacts; its catalogue is read next to it;
#                                  test-check-status.sh uses it)
set -euo pipefail

project=${1:?usage: check-status.sh <project>}
root=$(cd "$(dirname "$0")/../.." && pwd)

python3 -I - "$root" "$root/projects/$project" <<'PY'
import os, pathlib, re, sys, tomllib

root, d = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
status_path = pathlib.Path(os.environ.get("STATUS_TOML") or d / "status.toml")
st = tomllib.loads(status_path.read_text())

ALLOWED = {
    "kani": {"bounded-proof"},
    "plain": {"test"},
    "verus": {"proof", "model-proof"},
    "lean": {"model-proof", "model-check"},
    "prolog": {"check", "model-check", "test"},
    "tla": {"model-check", "bounded-proof"},
    "unit": {"test"},
    "difftest": {"test"},
    "sandbox": {"test", "check"},
    "wasm-check": {"check"},
    "deploy-check": {"check"},
    "review": {"review"},
}
# Where each layer's artifacts live (relative to the project), and whether the
# layer tags its properties with `// PROP:`.
DIRS = {
    "kani": (["kani/src"], True),
    "plain": (["kani/src"], True),
    "verus": (["verus"], True),
    "lean": (["lean"], False),
    "prolog": (["prolog"], False),
    "tla": (["tla", "kani/src"], False),
    "unit": (["near-tests"], True),
    "difftest": (["near-tests"], True),
    "sandbox": (["sandbox"], True),
    "wasm-check": (["near-tests", "wasm-surface.expected", "../../tools/scripts/wasm-surface.sh"], False),
    "deploy-check": (["deploy-checks"], False),
    "review": ([], False),
}
SKIP_DIRS = {"target", ".lake", "build", "node_modules", ".git"}
TEXT = {".rs", ".tla", ".cfg", ".lean", ".pl", ".py", ".sh", ".toml", ".md", ".expected", ".txt", ".patch"}
RANK = {"unverified": 0, "stale": 0, "in-progress": 1, "verified-with-assumptions": 2, "verified": 3}

cache = {}
def corpus(layer):
    if layer not in cache:
        texts = []
        for rel in DIRS.get(layer, ([], False))[0]:
            p = (d / rel).resolve()
            files = [p] if p.is_file() else (
                f for f in p.rglob("*") if f.is_file() and not (SKIP_DIRS & set(f.relative_to(p).parts))
            ) if p.is_dir() else []
            for f in files:
                if f.suffix in TEXT or f.name.endswith(".expected"):
                    try:
                        texts.append(f.read_text(errors="replace"))
                    except OSError:
                        pass
        text = "\n".join(texts)
        tags = set()
        for line in re.findall(r"PROP:([^\n]*)", text):
            tags.update(re.findall(r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+", line))
        cache[layer] = (text, tags)
    return cache[layer]

reviewer = st.get("review", {}).get("spec_reviewer", "").strip()
errors, seen = [], 0

def check(pid, p):
    global seen
    seen += 1
    layer, ev, status = p.get("layer"), p.get("evidence"), p["status"]
    if status not in RANK:
        errors.append(f"{pid}: unknown status {status!r}")
        return
    if layer not in ALLOWED:
        errors.append(f"{pid}: unknown layer {layer!r}")
        return
    if ev is not None and ev not in ALLOWED[layer]:
        errors.append(f"{pid}: evidence {ev!r} not allowed for layer {layer!r} (strategy.md section 12)")
    if RANK[status] == 0:
        return
    dirs, tagged = DIRS[layer]
    if dirs:
        text, tags = corpus(layer)
        if tagged and pid not in tags:
            errors.append(f"{pid}: {status} but no `// PROP: {pid}` tag in {', '.join(dirs)}")
        elif not tagged and not re.search(r"(?<![A-Za-z0-9-])" + re.escape(pid) + r"(?![A-Za-z0-9])", text):
            errors.append(f"{pid}: {status} but {pid} appears nowhere in {', '.join(dirs)}")
    if layer == "kani" and RANK[status] > RANK["verified-with-assumptions"]:
        errors.append(f"{pid}: Kani results are at most verified-with-assumptions")
    if status == "verified" and ev == "proof" and (p.get("assumptions") or []):
        errors.append(f"{pid}: verified with assumptions listed; use verified-with-assumptions")
    if layer in ("kani", "verus") and RANK[status] >= RANK["verified-with-assumptions"] and not reviewer:
        errors.append(f"{pid}: {status} needs [review] spec_reviewer (strategy.md section 12)")

props = {}
for mod in st.get("modules", {}).values():
    for pid, p in mod.get("properties", {}).items():
        if isinstance(p, dict) and "status" in p:
            props[pid] = p
            check(pid, p)

# Findings (plan-meta section 4): closes <-> closed_by, and what a status needs.
FSTATUS = {"open", "closed", "documented", "rejected"}
findings = st.get("findings", {})
assumptions = st.get("assumptions", {})
rank_of = {pid: RANK.get(p["status"], 0) for pid, p in props.items()}
rank_of.update({aid: RANK.get(a.get("status"), 0) for aid, a in assumptions.items() if isinstance(a, dict)})

def ids(v):
    if isinstance(v, str):
        return [x.strip() for x in v.split(",") if x.strip()]
    return list(v) if isinstance(v, list) else [v]

for pid, p in props.items():
    for fid in ids(p.get("closes", [])):
        if fid not in findings:
            errors.append(f"{pid}: closes {fid!r}, which is not a [findings] table")
        elif pid not in ids(findings[fid].get("closed_by", [])):
            errors.append(f"{pid}: closes {fid} but [findings.{fid}] closed_by does not list {pid}")
    if findings:
        for fid in ids(p.get("finding", [])):
            if fid not in findings:
                errors.append(f"{pid}: finding {fid!r} is not a [findings] table")
            elif findings[fid].get("status") != "open":
                errors.append(f"{pid}: finding {fid} is {findings[fid].get('status')!r}, not open (use closes)")

for fid, f in findings.items():
    if not isinstance(f, dict):
        errors.append(f"findings.{fid}: not a table")
        continue
    fs, by = f.get("status"), ids(f.get("closed_by", []))
    if fs not in FSTATUS:
        errors.append(f"findings.{fid}: unknown status {fs!r} (one of {', '.join(sorted(FSTATUS))})")
    for x in by:
        if x not in rank_of:
            errors.append(f"findings.{fid}: closed_by {x!r} is not a property or assumption")
        elif x in props and fid not in ids(props[x].get("closes", [])):
            errors.append(f"findings.{fid}: closed_by lists {x}, whose closes does not name {fid}")
    if fs == "closed":
        if not f.get("closed_in"):
            errors.append(f"findings.{fid}: closed without closed_in")
        if not by:
            errors.append(f"findings.{fid}: closed with an empty closed_by")
        elif not any(rank_of.get(x, 0) >= RANK["in-progress"] for x in by):
            errors.append(f"findings.{fid}: closed, but every closed_by row is unverified or stale")

# Catalogue (plan-meta section 4.5): status.toml against the catalogue's
# section 5 (header comment). Only with [project] catalogue.
NEAR = {"near-tests": {"unit", "difftest", "wasm-check"}}
PREV = re.compile(r"\(v\d+(?:\.\d+)*:")
RESULT = re.compile(r"(?<![\w-])(?:proved|model-checked|tested|checked|bounded proof)(?![\w-])")
BOUNDED = re.compile(r"(?<![\w-])bounded proof(?![\w-])")
# Markdown emphasis and code marks; "_" only outside a word (f1_forever keeps it).
MARKUP = re.compile(r"[*`]|(?<![A-Za-z0-9])_+|_+(?![A-Za-z0-9])")
# A negated result or verified* word: removed before the result is looked for.
NEGATED = re.compile(r"(?<![\w-])(?:not|no|never)\s+(?:yet\s+)?"
                     r"(?:proved|model-checked|tested|checked|bounded proof|verified(?:-with-assumptions)?)(?![\w-])")

def md_cells(line):
    # The trimmed cells of "| a | b |"; "\|" inside a cell is not a separator.
    s = line.strip()
    s = s[1:] if s.startswith("|") else s
    s = s[:-1] if s.endswith("|") and not s.endswith("\\|") else s
    return [c.strip() for c in re.split(r"(?<!\\)\|", s)]

def bare_id(s):
    return bool(re.fullmatch(r"[A-Z][A-Z0-9-]*[A-Z0-9]", s)) and "-" in s[1:]

def section5(text, name):
    # [(line, subsection, id, layers, status cell)] in file order, and problems.
    lines, rows, probs = text.split("\n"), [], []
    start = next((i for i, l in enumerate(lines) if l.startswith("## 5.")), None)
    if start is None:
        return rows, [f"catalogue: {name} has no \"## 5.\" section"]
    i, sub = start + 1, None
    while i < len(lines) and not lines[i].startswith("## "):
        m = re.match(r"### (5\.\d+)(?!\d)", lines[i])
        if m:
            sub = m.group(1)
        if not lines[i].startswith("|"):
            i += 1
            continue
        j = i
        while j < len(lines) and lines[j].startswith("|"):
            j += 1
        head = md_cells(lines[i])
        sep = md_cells(lines[i + 1]) if i + 1 < j else []
        if not sep or not all(c and set(c) <= set("-:") and "-" in c for c in sep):
            probs.append(f"catalogue: {name}:{i + 1}: a section 5 table without a separator row")
        elif head[0] != "ID" or "Layer(s)" not in head or "Status" not in head:
            probs.append(f"catalogue: {name}:{i + 1}: a section 5 table not headed ID ... Layer(s) ... Status")
        else:
            li, si = head.index("Layer(s)"), head.index("Status")
            for k in range(i + 2, j):
                c = md_cells(lines[k])
                if not bare_id(c[0]):
                    probs.append(f"catalogue: {name}:{k + 1}: a section 5 row whose first cell {c[0]!r} is not a bare ID")
                elif len(c) <= max(li, si):
                    probs.append(f"{c[0]}: {name}:{k + 1}: {len(c)} cells, no Layer(s) or Status cell")
                else:
                    lay = [x.strip() for x in c[li].split(",") if x.strip()]
                    rows.append((k + 1, sub, c[0], lay, c[si]))
        i = j
    if not rows and not probs:
        probs.append(f"catalogue: {name} section 5 has no rows")
    return rows, probs

def drop_prev(cell):
    # The cell without its "(vX.Y.Z: ...)" brackets (balanced parentheses).
    out, i = [], 0
    while (m := PREV.search(cell, i)):
        out.append(cell[i:m.start()])
        depth, j = 0, m.start()
        while j < len(cell):
            depth += {"(": 1, ")": -1}.get(cell[j], 0)
            j += 1
            if depth == 0:
                break
        i = j
    return "".join(out) + cell[i:]

def implied(cell, kani):
    # (statuses the Status cell allows, what it records). kani: the row's
    # primary layer is Kani, or it is an assumption (any `Kani: pending`).
    cell = MARKUP.sub("", cell)
    pending = kani and "Kani: pending" in cell
    if re.match(r"stale\b", cell) or (pending and re.search(r"Kani: pending \(v\d+(?:\.\d+)*:", cell)):
        return {"stale"}, "a previous release's result, not re-checked at the pin"
    if pending:
        return {"stale", "unverified"}, "`Kani: pending` with no previous result"
    base = NEGATED.sub("", drop_prev(cell))
    if not (BOUNDED if kani == "kani" else RESULT).search(base):
        return {"unverified"}, "no Kani result (`bounded proof`)" if kani == "kani" else "no result"
    named = {v for v in ("verified", "verified-with-assumptions")
             if re.search(r"(?<![\w-])" + v + r"(?![\w-])", base)}
    return {"in-progress"} | named, "a result at the pin"

cat_rows = None
cat_name = st.get("project", {}).get("catalogue")
if cat_name:
    cat_path = status_path.parent / cat_name
    if not cat_path.is_file():
        errors.append(f"catalogue: {cat_name} not found next to {status_path.name}")
    else:
        cat_rows, probs = section5(cat_path.read_text(), cat_name)
        errors.extend(probs)
    if cat_rows:   # with no rows, the problem above is the only one reported
        where = {}
        for r in cat_rows:
            where.setdefault(r[2], []).append(r)
        for rid, rs in where.items():
            if len(rs) > 1:
                errors.append(f"{rid}: {len(rs)} rows in {cat_name} section 5 (lines {', '.join(str(r[0]) for r in rs)})")
        asm_rows = {rid: rs[0] for rid, rs in where.items() if rs[0][3] == ["assumption"]}
        prop_rows = {rid: rs[0] for rid, rs in where.items() if rs[0][3] != ["assumption"]}
        mod_of, twice = {}, set()
        for mn, mod in st.get("modules", {}).items():
            for pid, p in mod.get("properties", {}).items():
                if isinstance(p, dict) and "status" in p:
                    if pid in mod_of:   # props keeps the last; so does mod_of
                        errors.append(f"{pid}: in [modules.{mod_of[pid]}] and [modules.{mn}]")
                        twice.add(pid)
                    mod_of[pid] = mn
        for pid in props:
            if pid not in prop_rows:
                errors.append(f"{pid}: a status.toml property with no {cat_name} section 5 property row")
        for rid, r in prop_rows.items():
            if rid not in props:
                errors.append(f"{rid}: {cat_name}:{r[0]} is a section 5 property row with no status.toml property")
        for aid in assumptions:
            if aid not in asm_rows:
                errors.append(f"{aid}: [assumptions.{aid}] has no {cat_name} section 5 row with Layer(s) assumption")
        for rid, r in asm_rows.items():
            if rid not in assumptions:
                errors.append(f"{rid}: {cat_name}:{r[0]} is a section 5 assumption row with no [assumptions.{rid}]")
        sub_mod = {}
        for n, sub, rid, lay, cell in cat_rows:
            if rid not in props or prop_rows.get(rid, (None,))[0] != n:
                continue
            p, at = props[rid], f"{cat_name}:{n}"
            first = lay[0] if lay else None
            if p.get("layer") not in NEAR.get(first, {first}):
                errors.append(f"{rid}: layer {p.get('layer')!r}, but {at} Layer(s) {', '.join(lay)!r} starts with {first!r}"
                              + (" (unit, difftest or wasm-check)" if first in NEAR else ""))
            for other in lay[1:]:
                if not NEAR.get(other, {other}) & set(p.get("cross", [])):
                    errors.append(f"{rid}: cross {p.get('cross', [])!r} lacks {other!r} of {at} Layer(s) {', '.join(lay)!r}")
            if sub and rid not in twice:
                m0, r0 = sub_mod.setdefault(sub, (mod_of[rid], rid))
                if mod_of[rid] != m0:
                    errors.append(f"{rid}: in [modules.{mod_of[rid]}], but section {sub}'s rows are in [modules.{m0}] ({r0})")
            if p["status"] in RANK:
                ok, says = implied(cell, "kani" if p.get("layer") == "kani" else None)
                if p["status"] not in ok:
                    errors.append(f"{rid}: status {p['status']!r}, but {at} Status {cell!r} records {says}: {' or '.join(sorted(ok))}")
        subs_of = {}
        for sub, (m0, _) in sub_mod.items():
            subs_of.setdefault(m0, []).append(sub)
        for m0, subs in subs_of.items():
            if len(subs) > 1:
                errors.append(f"catalogue: [modules.{m0}] holds sections {', '.join(subs)}; one module per section 5 subsection")
        for rid, (n, sub, _, lay, cell) in asm_rows.items():
            a = assumptions.get(rid)
            if not isinstance(a, dict):
                continue
            at, bare = f"{cat_name}:{n}", MARKUP.sub("", cell)
            lv = re.search(r"\((high|med|low)\)", bare)
            lv = lv.group(1) if lv else "policy" if re.match(r"policy\b", bare) else None
            if a.get("level") != lv:
                errors.append(f"{rid}: level {a.get('level')!r}, but {at} Status {cell!r} says {lv!r}")
            ok, says = implied(cell, "assumption")
            if a.get("status") not in ok:
                errors.append(f"{rid}: status {a.get('status')!r}, but {at} Status {cell!r} records {says}: {' or '.join(sorted(ok))}")

for e in errors:
    print(f"::error::status-lint: {e}" if os.environ.get("GITHUB_ACTIONS") else f"status-lint: {e}")
if cat_rows is not None:
    print(f"status-lint: {seen} properties, {len(assumptions)} assumptions, {len(findings)} findings, "
          f"{len(cat_rows)} {cat_name} section 5 rows, {len(errors)} problem(s)")
elif findings:
    print(f"status-lint: {seen} properties, {len(findings)} findings, {len(errors)} problem(s)")
else:
    print(f"status-lint: {seen} properties, {len(errors)} problem(s)")
sys.exit(1 if errors else 0)
PY
