#!/usr/bin/env python3
"""PROP-tag traceability check of the shared-pool near-tests layer.

usage: python3 -I check-tags.py      (no arguments; run.sh runs it first in
                                      `test` and `all`)

Paths are relative to this file: the layer is this directory, the project
is its parent (`projects/shared-pool`). The catalogue is the `property/3`
facts of ../prolog/requirements/traceability.pl. Standard library only.

Exits 1, listing every problem, unless:
  1. every line with `PROP:` in near-tests/ (and ../wasm-surface.expected) is
     a comment tag (`// PROP:` in .rs, `# PROP:` in .sh) listing only IDs,
     and every ID is in the catalogue; a `PROP:` anywhere else (.md,
     .patch, .py, ...) must yield no ID, because check-status.sh would
     count it;
  2. every `#[test]` in tests/*.rs has its own tag line directly above it;
  3. every catalogue property whose layers include near-tests has at least
     one per-test tag on a test that checks it (lockfile.rs's scope tag does
     not count), or a tag line in the block of a wasm-surface.sh check
     SURF-n that checks it (SH-11 is checked only there, SH-12 by SURF-2
     and SURF-3);
  4. each test file's header tag line equals the union of its tests' tags;
  5. run.sh, run-mutants.sh and wasm-surface.sh carry tags, and each local
     check SURF-n in wasm-surface.sh (SURF-1, SURF-2, SURF-3) has a tag
     line in its block; no tag names a SURF-n;
  6. the tag set check-status.sh extracts for the near-tests corpus (its own
     regexes and file selection) equals the set found here.
"""
import os
import pathlib
import re
import sys

nt = pathlib.Path(__file__).resolve().parent
proj = nt.parent
ID = r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+"  # check-status.sh's tag token
TEXT = {".rs", ".tla", ".cfg", ".lean", ".pl", ".py", ".sh", ".toml", ".md", ".expected", ".txt", ".patch"}
SKIP = {"target", ".lake", "build", "node_modules", ".git"}  # check-status.sh's SKIP_DIRS
errors = []


def text_files(root):
    """check-status.sh's selection: text suffixes, no file under a SKIP dir.
    Pruned walk, so the cargo target directory is never listed."""
    out = []
    for d, dirs, fs in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x not in SKIP)
        for f in sorted(fs):
            p = pathlib.Path(d) / f
            if p.suffix in TEXT or p.name.endswith(".expected"):
                out.append(p)
    return out


# --- catalogue -------------------------------------------------------------
cat = {}
trace_pl = proj / "prolog" / "requirements" / "traceability.pl"
for m in re.finditer(r"^property\('([^']+)',\s*\[([^\]]*)\]", trace_pl.read_text(), re.M):
    cat[m.group(1)] = [x.strip().strip("'") for x in m.group(2).split(",")]
if not cat:
    print(f"check-tags: no property/3 facts in {trace_pl}")
    sys.exit(1)
layer_ids = sorted(i for i, ls in cat.items() if "near-tests" in ls)
print(f"catalogue: {len(cat)} IDs; layer near-tests: {' '.join(layer_ids)}")

# --- every PROP occurrence ---------------------------------------------------
files = text_files(nt) + [proj / "wasm-surface.expected"]
found = set()
tags_by_file = {}
for f in files:
    rel = f.relative_to(proj)
    for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
        if "PROP:" not in line:
            continue
        ids = []
        for rest in re.findall(r"PROP:([^\n]*)", line):
            ids += re.findall(ID, rest)
        comment = {".rs": "//", ".sh": "#"}.get(f.suffix)
        m = re.match(r"^\s*(//|#)\s*PROP:\s*(.*)$", line) if comment else None
        if not comment or not m or m.group(1) != comment:
            if ids:
                errors.append(f"{rel}:{n}: `PROP:` outside a comment tag yields IDs {ids} (check-status counts them)")
            continue
        toks = m.group(2).split()
        if not toks:
            errors.append(f"{rel}:{n}: empty tag line")
        for t in toks:
            if not re.fullmatch(ID, t):
                errors.append(f"{rel}:{n}: `{t}` is not an ID (tag lines hold IDs only)")
            elif t.startswith("SURF-"):
                errors.append(f"{rel}:{n}: {t} is a local name, not a catalogue ID")
            elif t not in cat:
                errors.append(f"{rel}:{n}: {t} is not in the catalogue")
        found.update(ids)
        tags_by_file.setdefault(rel, []).append((n, toks))

# --- per-test tags -----------------------------------------------------------
per_test = {}  # test -> tags
covered = {}   # ID -> tests (scope-only tags excluded)
# lockfile.rs tags the whole layer as the scope of its precondition; it
# checks none of them, so it never counts toward rule 3.
SCOPE_ONLY = {"lockfile.rs"}
for f in sorted((nt / "tests").glob("*.rs")):
    lines = f.read_text().splitlines()
    rel = f.relative_to(proj)
    union = set()
    for i, line in enumerate(lines):
        if line.strip() != "#[test]":
            continue
        fn = re.match(r"\s*fn ([a-z0-9_]+)\(", lines[i + 1]) if i + 1 < len(lines) else None
        if not fn:
            errors.append(f"{rel}:{i + 2}: no `fn` directly below #[test]")
            continue
        name = fn.group(1)
        m = re.match(r"^// PROP: (.*)$", lines[i - 1]) if i > 0 else None
        if not m:
            errors.append(f"{rel}: test {name} has no tag line directly above #[test]")
            continue
        tags = m.group(1).split()
        per_test[f"{f.name}::{name}"] = tags
        union.update(tags)
        if f.name in SCOPE_ONLY:
            continue
        for t in tags:
            covered.setdefault(t, []).append(f"{f.stem}::{name}")
    if union:
        header = [toks for (n, toks) in tags_by_file.get(rel, []) if n < 40]
        if not header:
            errors.append(f"{rel}: no header tag line")
        elif set(header[0]) != union:
            errors.append(f"{rel}: header tags {sorted(header[0])} != union of per-test tags {sorted(union)}")

# --- scripts -----------------------------------------------------------------
for s in ["run.sh", "run-mutants.sh", "wasm-surface.sh"]:
    if not tags_by_file.get(pathlib.Path("near-tests") / s):
        errors.append(f"near-tests/{s}: no tag line")
ws = (nt / "wasm-surface.sh").read_text().splitlines()
starts = [i for i, l in enumerate(ws) if re.match(r"^#\s+SURF-\d\s", l)]
end = next((i for i, l in enumerate(ws) if l.startswith("# Exit:")), None)
if end is None:
    errors.append("near-tests/wasm-surface.sh: no `# Exit:` line ending the SURF-n blocks")
    end = len(ws)
for a, b in zip(starts, starts[1:] + [end]):
    surf = ws[a].split()[1]
    block_tags = [re.match(r"^\s*#\s*PROP:(.*)$", l) for l in ws[a:b]]
    block_tags = [m.group(1).split() for m in block_tags if m]
    if not block_tags:
        errors.append(f"near-tests/wasm-surface.sh:{a + 1}: {surf} has no tag line in its block")
    # A SURF-n check checks the IDs on its own tag line (rule 3).
    for toks in block_tags:
        for t in toks:
            covered.setdefault(t, []).append(f"wasm-surface.sh {surf}")
if len(starts) != 3:
    errors.append(f"wasm-surface.sh: expected the SURF-1, SURF-2 and SURF-3 definitions, found {len(starts)}")

for pid in layer_ids:
    if pid not in covered:
        errors.append(f"{pid}: catalogue layer near-tests, but no test or SURF-n check carries a tag for it")

# --- same view as check-status.sh (layers unit and difftest: dir near-tests) --
lint = set()
for line in re.findall(r"PROP:([^\n]*)", "\n".join(f.read_text(errors="replace") for f in text_files(nt))):
    lint.update(re.findall(ID, line))
if lint != found:
    errors.append(f"check-status view differs from this check: {sorted(lint ^ found)}")
missing = [p for p in layer_ids if p not in lint]
if missing:
    errors.append(f"check-status would not find tags for {missing}")

# --- report ------------------------------------------------------------------
print(f"tag lines: {sum(len(v) for v in tags_by_file.values())} in {len(tags_by_file)} files; "
      f"{len(per_test)} tests tagged; distinct IDs: {len(found)}")
print("IDs outside layer near-tests (cross-layer references): "
      + " ".join(sorted(t for t in found if t in cat and "near-tests" not in cat[t])))
for pid in layer_ids:
    print(f"  {pid:<11} {len(covered.get(pid, []))} checks: {', '.join(covered.get(pid, []))}")
for e in errors:
    print(f"check-tags: {e}")
print(f"check-tags: {len(errors)} problem(s)")
sys.exit(1 if errors else 0)
