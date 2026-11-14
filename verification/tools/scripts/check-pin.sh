#!/usr/bin/env bash
# Checks that a project's source mirror is the commit status.toml pins and
# that every file listed in [source_hashes] still has its recorded sha256.
#
# If [project] has an `identity` list, also checks each entry
# { label, commit, paths }: every path (relative to upstream_path) must be
# byte-identical at the pin and at that commit, the same git tree or blob
# entry (mode, type and object id) at both. An entry whose commit is the pin
# passes without a fetch or a comparison. Any other commit is fetched by SHA
# from the mirror's origin if the clone lacks it (a CI submodule checkout has
# depth 1); a commit that cannot be fetched fails. The older form
# (verified_release_commit with identical_paths) is refused, so a status.toml
# that still uses it cannot skip the check.
#
# usage: tools/scripts/check-pin.sh <project>
# Needs the submodule checked out and python3 >= 3.11 (tomllib).
set -euo pipefail

project=${1:?usage: check-pin.sh <project>}
root=$(cd "$(dirname "$0")/../.." && pwd)
dir="$root/projects/$project"

python3 -I - "$root" "$dir" <<'PY'
import hashlib, posixpath, re, subprocess, sys, tomllib, pathlib
root, d = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
st = tomllib.loads((d / "status.toml").read_text())
pin = st["project"]["pinned_commit"]
base = d / "source-mirror" / st["project"].get("upstream_path", "")
fail = 0

rel = (d / "source-mirror").relative_to(root)
gitlink = subprocess.run(["git", "-C", str(root), "ls-tree", "HEAD", str(rel)],
                         capture_output=True, text=True, check=True).stdout.split()
gitlink = gitlink[2] if len(gitlink) > 2 else "<none>"
if gitlink != pin:
    print(f"gitlink {gitlink} != pinned_commit {pin}"); fail = 1
head = subprocess.run(["git", "-C", str(d / "source-mirror"), "rev-parse", "HEAD"],
                      capture_output=True, text=True).stdout.strip()
if head != pin:
    print(f"checked-out mirror {head or '<not checked out>'} != pinned_commit {pin}"); fail = 1

for name, want in st.get("source_hashes", {}).items():
    p = base / name
    got = hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else "<missing>"
    if got != want:
        print(f"hash drift: {name}: {got} != {want}"); fail = 1

# ASM-COMPILE: the deployed build must keep overflow checks in release.
if st["project"].get("release_overflow_checks"):
    cargo = base / "Cargo.toml"
    prof = tomllib.loads(cargo.read_text()).get("profile", {}).get("release", {}) if cargo.is_file() else {}
    if prof.get("overflow-checks") is not True:
        print(f"ASM-COMPILE: {cargo.relative_to(root)} [profile.release] does not set overflow-checks = true"); fail = 1

# Identity: each entry's commit holds the pin's code at its paths, byte for
# byte (strategy.md section 12).
proj = st["project"]
for key in ("verified_release_commit", "identical_paths"):
    if key in proj:
        print(f"[project] {key} is no longer read: list the commit under identity = [{{ label, commit, paths }}]"); fail = 1
ident = proj.get("identity", [])
mirror = d / "source-mirror"
def git(*args):
    return subprocess.run(["git", "-C", str(mirror), *args], capture_output=True, text=True)
def entry(commit, path):
    # "<mode> <type> <object>" of the path in the commit, or None.
    out = git("ls-tree", commit, "--", path).stdout.splitlines()
    return out[0].split("\t")[0] if len(out) == 1 and out[0].endswith("\t" + path) else None
if not isinstance(ident, list):
    print("[project] identity is not a list of { label, commit, paths } tables"); fail = 1
    ident = []
for n, it in enumerate(ident, 1):
    if not isinstance(it, dict):
        print(f"identity entry {n} is not a table"); fail = 1
        continue
    commit = it.get("commit", "")
    label = it.get("label") or f"identity entry {n}"
    paths = it.get("paths", [])
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        print(f"{label}: commit {commit!r} is not a 40-character sha"); fail = 1
        continue
    if not isinstance(paths, list) or not paths or not all(isinstance(p, str) and p.strip("/") for p in paths):
        print(f"{label}: paths must be a non-empty list of paths"); fail = 1
        continue
    up = proj.get("upstream_path", "")
    gps = [posixpath.join(up, p.strip("/")) if up else p.strip("/") for p in paths]
    if commit == pin:
        gone = [gp for gp in gps if entry(pin, gp) is None]
        for gp in gone:
            print(f"{label}: {gp} is missing at the pin"); fail = 1
        if not gone:
            print(f"{label} ({commit[:7]}) is the pin")
        continue
    have = git("cat-file", "-e", commit + "^{commit}").returncode == 0
    if not have:
        f = git("fetch", "--quiet", "--no-tags", "--depth", "1", "origin", commit)
        have = f.returncode == 0 and git("cat-file", "-e", commit + "^{commit}").returncode == 0
        if not have:
            print(f"{label}: cannot fetch {commit}: {f.stderr.strip()}"); fail = 1
            continue
    bad = []
    for gp in gps:
        a, b = entry(pin, gp), entry(commit, gp)
        if a is None or b is None:
            where = " and ".join(w for w, e in (("the pin", a), (label, b)) if e is None)
            print(f"{label}: {gp} is missing at {where}"); bad.append(gp)
        elif a != b:
            print(f"{label}: {gp} differs between the pin and {commit[:7]} ({a} != {b})"); bad.append(gp)
    if bad:
        stat = git("diff", "--stat", pin, commit, "--", *bad).stdout.rstrip()
        if stat:
            print(stat)
        fail = 1
    else:
        print(f"{label} ({commit[:7]}): {', '.join(paths)} byte-identical to the pin")

print("pin ok" if not fail else "pin check FAILED")
sys.exit(fail)
PY
