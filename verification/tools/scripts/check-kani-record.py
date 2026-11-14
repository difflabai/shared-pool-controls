#!/usr/bin/env python3
"""shared-pool's local Kani record (projects/shared-pool/kani/local): CI's check.

The Kani harnesses and mutants whose measured peak is above what a hosted
runner holds in memory (the kani-large and kani-mutants-large jobs of
.github/workflows/shared-pool.yml) run on a local machine
(kani/local/run-local.py), and kani/local/record.py records the run. CI
runs only the rest (the kani and kani-mutants jobs) and checks the record
instead of re-running the large part:

  check-kani-record.py [--stale-ok-harnesses] [--stale-ok-mutants] [--stale-ok]
      fails unless
      - the record's fingerprint of the Kani inputs (fingerprint below) is
        this commit's ("re-run the local Kani record: ..."). A stale record
        is only a warning when this run verifies everything it records:
        --stale-ok-harnesses when the kani-large jobs run (on KANI_RUNNER),
        --stale-ok-mutants when the kani-mutants-large jobs run, --stale-ok
        both;
      - the fingerprint is the hash of the record's input lines and, when
        the record's commit is in this clone, that commit's (a shallow
        clone without it skips this);
      - every recorded harness is ok and every recorded mutant as its patch
        expects (killed/killed/killed; survive/survived/survived for a
        `# must-survive` patch), its results, covers and not-run.tsv
        unedited (digest);
      - the recorded harnesses plus the kani job's groups are `run.sh list`,
        each exactly once, and the kani-large groups are the recorded
        harnesses; the same for mutants/*.patch with the recorded mutants,
        kani-mutants and kani-mutants-large, except that a
        kani-mutants-large mutant may instead be declared not run in the
        record's not-run.tsv (mutant<TAB>reason; record.py refuses one
        that a run finished): each such one is a warning, never hidden;
      - no kani-large / kani-mutants-large entry's timeout is below the
        limit_s its recorded run used (a higher one is fine; a warning
        under the matching --stale-ok flag, as then that job runs);
      - every kani and kani-mutants entry peaked at IN_MEMORY_GB or less;
      - RESULTS.md has the local-record block, and it is `record.py table`.
  check-kani-record.py fingerprint [REV]   the inputs and fingerprint at REV
                                           (default HEAD), as record.py
                                           writes them
  check-kani-record.py pathspecs           the inputs as git pathspecs (for
                                           git diff)
  check-kani-record.py cargo-configs [REV] the repository's .cargo/config[.toml]
                                           files above kani/ at REV (default
                                           HEAD; an input, for run-local.py's
                                           lanes)
  check-kani-record.py matrix JOB          JOB's matrix entries in the
                                           workflow (name, gb, timeout)
  check-kani-record.py harnesses           the recorded harnesses
  check-kani-record.py covers              the recorded covers, in run.sh's
                                           covers.tsv format (for
                                           check-covers.sh)

The fingerprint reads git objects only, so it works in a shallow checkout
without the source-mirror submodule (the gitlink is the core's commit).
Lists (run.sh list, mutants/*.patch, the workflow) are read from the work
tree; in CI that is HEAD. The record is attested by whoever ran and
committed it: the check shows that it is consistent and belongs to these
inputs, not that the run took place as recorded.

env: KANI_RECORD_DIR (default projects/shared-pool/kani/local),
     KANI_RECORD_WORKFLOW (default .github/workflows/shared-pool.yml),
     KANI_RESULTS_MD (default projects/shared-pool/kani/RESULTS.md),
     KANI_RECORD_ROOT (the repository; default this script's),
     KANI_RECORD_REV (the commit the check compares with; default HEAD).
Exit 0 iff every check passes; 2 on a usage error.
"""
import hashlib
import os
import re
import subprocess
import sys

ROOT = os.environ.get("KANI_RECORD_ROOT") or os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
SP = "projects/shared-pool"
RECORD = os.environ.get("KANI_RECORD_DIR") or os.path.join(ROOT, SP, "kani/local")
WORKFLOW = os.environ.get("KANI_RECORD_WORKFLOW") or os.path.join(ROOT, ".github/workflows/shared-pool.yml")
RESULTS_MD = os.environ.get("KANI_RESULTS_MD") or os.path.join(ROOT, SP, "kani/RESULTS.md")
RECORD_PY = os.path.join(ROOT, SP, "kani/local/record.py")

# What a Kani verdict depends on. First the Kani crate, by default-include:
# every file under kani/ but the record itself (kani/local/) and the
# Markdown, hashed as one line (KANI_ALL), so that a new file there
# (.cargo/config.toml, rust-toolchain.toml, build.rs) changes the
# fingerprint too. The named paths of the crate are in that line and also
# have their own, which say what changed. Then the pinned core (a gitlink:
# its commit id), the conformance traces plain mode replays (a mutant's
# plain verdict), the repository's cargo configuration above kani/
# (CARGO_ABOVE), and the tool versions: of versions.env only KANI and CBMC
# (the rest pins other layers' tools; RUST_TOOLCHAIN, plain mode's, is not
# in it: CI's mutants job reruns every mutant's plain mode on every run).
KANI_DIR = SP + "/kani"
KANI_ALL = KANI_DIR + "/(all but local, *.md)"
INPUT_PATHS = [
    SP + "/kani/src",
    SP + "/kani/Cargo.toml",
    SP + "/kani/Cargo.lock",
    SP + "/kani/mutants",
    SP + "/kani/run.sh",
    SP + "/kani/.gitignore",
    SP + "/source-mirror",
    SP + "/conformance/traces/core.txt",
]
INPUT_VERSIONS = ["KANI", "CBMC"]  # tools/versions.env
# Cargo also reads .cargo/config[.toml] in every directory above the crate,
# so such a file in the repository above kani/ can set rustflags or a
# profile for the groups' builds. Those the commit has are one line, "none"
# when it has none (the record's commit has none); run-local.py copies them
# into its lanes. Cargo configuration outside the repository (CARGO_HOME's,
# or above the checkout) is the host's, not in the record: run-local.py
# refuses one that changes a build.
CARGO_CONFIGS_ABOVE = [d + ".cargo/" + n for d in ("", "projects/", SP + "/") for n in ("config", "config.toml")]
CARGO_ABOVE = ".cargo/config[.toml] above kani/ (the root, projects, projects/shared-pool)"
# The kani and kani-mutants jobs run on ubuntu-latest (16 GB, 15.6 usable):
# their entries must have peaked at this or less (kani/RESULTS.md).
IN_MEMORY_GB = 13
JOBS_COLUMNS = ["kind", "name", "result", "seconds", "job_seconds", "cbmc_peak_gb",
                "tree_peak_gb", "limit_s", "covers", "kani_harnesses", "started_utc"]
COVERS_COLUMNS = ["family", "cover", "status", "harness"]
# kani-mutants-large mutants that no local run finished, each with its
# reason (record.py checks that none is in a run); in the digest when present.
NOT_RUN = "not-run.tsv"
NOT_RUN_COLUMNS = ["mutant", "reason"]
BEGIN = "<!-- kani-local-record: begin"
END = "<!-- kani-local-record: end -->"
RERUN = ("re-run the local Kani record: projects/shared-pool/kani/local/run-local.py --record "
         "(many hours on a 64 GB machine; projects/shared-pool/kani/local/README.md), and commit kani/local")


REV = os.environ.get("KANI_RECORD_REV") or "HEAD"


class Fail(Exception):
    pass


def run(args, cwd=None):
    r = subprocess.run(args, capture_output=True, text=True, cwd=cwd)
    if r.returncode != 0:
        raise Fail("%s: exit %d: %s" % (" ".join(args), r.returncode, (r.stderr or r.stdout).strip()))
    return r.stdout


def kani_all(rev):
    """sha256 of rev's kani/ tree, file by file (mode, type, object id, path), less local/ and *.md."""
    r = subprocess.run(["git", "-C", ROOT, "ls-tree", "-r", "-z", "%s:%s" % (rev, KANI_DIR)], capture_output=True)
    if r.returncode != 0:
        raise Fail("%s:%s is not in the repository" % (rev, KANI_DIR))
    h = hashlib.sha256()
    for entry in r.stdout.split(b"\0"):
        if not entry:
            continue
        meta, path = entry.split(b"\t", 1)
        if path.startswith(b"local/") or path.endswith(b".md"):
            continue
        h.update(meta + b"\t" + path + b"\n")
    return h.hexdigest()


def cargo_above_entries(rev):
    """rev's .cargo/config[.toml] files above kani/: [b"mode type object-id\tpath"]."""
    r = subprocess.run(["git", "-C", ROOT, "ls-tree", "--full-tree", "-r", "-z", rev, "--"] + CARGO_CONFIGS_ABOVE,
                       capture_output=True)
    if r.returncode != 0:
        raise Fail("%s is not a commit of the repository" % rev)
    return [e for e in r.stdout.split(b"\0") if e]


def cargo_above(rev):
    """sha256 of rev's .cargo/config[.toml] files above kani/ (mode, type, object id, path), or none."""
    entries = cargo_above_entries(rev)
    if not entries:
        return "none"
    h = hashlib.sha256()
    for entry in entries:
        h.update(entry + b"\n")
    return h.hexdigest()


def fingerprint(rev="HEAD"):
    """[(name, value)], sha256 hex of their lines."""
    lines = []
    for p in INPUT_PATHS:
        r = subprocess.run(["git", "-C", ROOT, "rev-parse", "--verify", "-q", "%s:%s" % (rev, p)],
                           capture_output=True, text=True)
        if r.returncode != 0:
            raise Fail("%s:%s is not in the repository" % (rev, p))
        lines.append((p, r.stdout.strip()))
    lines.append((KANI_ALL, kani_all(rev)))
    lines.append((CARGO_ABOVE, cargo_above(rev)))
    env = run(["git", "-C", ROOT, "show", "%s:tools/versions.env" % rev])
    for k in INPUT_VERSIONS:
        m = re.search(r"^%s=(\S+)\s*$" % k, env, re.M)
        if not m:
            raise Fail("%s:tools/versions.env has no %s" % (rev, k))
        lines.append(("tools/versions.env:" + k, m.group(1)))
    return lines, digest_lines(lines)


def pathspecs():
    """The inputs as git pathspecs: the files fingerprint reads, and versions.env whole."""
    return ([KANI_DIR, ":(exclude)%s/local" % KANI_DIR, ":(exclude)%s/*.md" % KANI_DIR]
            + [p for p in INPUT_PATHS if not p.startswith(KANI_DIR + "/")] + ["tools/versions.env"]
            + CARGO_CONFIGS_ABOVE)


def is_commit(c):
    return bool(re.fullmatch(r"[0-9a-f]{40}", c)) and subprocess.run(
        ["git", "-C", ROOT, "cat-file", "-e", c + "^{commit}"], capture_output=True).returncode == 0


def digest_lines(lines):
    return hashlib.sha256("".join("%s\t%s\n" % nv for nv in lines).encode()).hexdigest()


def digest_files(*paths):
    h = hashlib.sha256()
    for p in paths:
        with open(p, "rb") as f:
            h.update(f.read())
    return h.hexdigest()


ENTRY = re.compile(r'^\s*- \{ (group|mutant): "([^"]+)", gb: (\d+), timeout: (\d+) \}\s*$')


def matrices(path=None):
    """{job: [{kind, name, gb, timeout}]} from the workflow's matrix entries."""
    path = path or WORKFLOW
    out, in_jobs, job = {}, False, None
    with open(path) as f:
        for i, line in enumerate(f, 1):
            if line.rstrip() == "jobs:":
                in_jobs = True
                continue
            if not in_jobs:
                continue
            m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
            if m:
                job = m.group(1)
                continue
            if job and re.match(r"^\s*- \{ *(group|mutant):", line):
                e = ENTRY.match(line)
                if not e:
                    raise Fail('%s:%d: a matrix entry must read - { group|mutant: "...", gb: N, timeout: N }'
                               % (path, i))
                out.setdefault(job, []).append(dict(kind=e.group(1), name=e.group(2),
                                                    gb=int(e.group(3)), timeout=int(e.group(4))))
    return out


def read_tsv(path, columns):
    with open(path) as f:
        rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    if not rows or rows[0] != columns:
        raise Fail("%s: the header must be %s" % (path, "\\t".join(columns)))
    out = []
    for n, r in enumerate(rows[1:], 2):
        if len(r) != len(columns):
            raise Fail("%s:%d: %d fields, want %d" % (path, n, len(r), len(columns)))
        out.append(dict(zip(columns, r)))
    return out


def read_record(d=None):
    """provenance {key: value}, inputs [(name, value)], jobs, covers, not run [{mutant, reason}]."""
    d = d or RECORD
    prov_path = os.path.join(d, "provenance.txt")
    if not os.path.exists(prov_path):
        raise Fail("no local Kani record in %s (provenance.txt): %s" % (os.path.relpath(d, ROOT), RERUN))
    prov, inputs = {}, []
    with open(prov_path) as f:
        for n, line in enumerate(f, 1):
            line = line.rstrip("\n")
            if not line.strip() or line.startswith("#"):
                continue
            parts = line.split("\t")
            if parts[0] == "input" and len(parts) == 3:
                inputs.append((parts[1], parts[2]))
            elif len(parts) == 2:
                prov[parts[0]] = parts[1]
            else:
                raise Fail("%s:%d: want key<TAB>value or input<TAB>name<TAB>value" % (prov_path, n))
    jobs = read_tsv(os.path.join(d, "jobs.tsv"), JOBS_COLUMNS)
    covers = read_tsv(os.path.join(d, "covers.tsv"), COVERS_COLUMNS)
    not_run = read_tsv(os.path.join(d, NOT_RUN), NOT_RUN_COLUMNS) if os.path.exists(os.path.join(d, NOT_RUN)) else []
    return prov, inputs, jobs, covers, not_run


def record_digest(d=None):
    """sha256 of jobs.tsv, covers.tsv and, when there is one, not-run.tsv (record.py's digest)."""
    d = d or RECORD
    return digest_files(*[os.path.join(d, f) for f in ("jobs.tsv", "covers.tsv", NOT_RUN)
                          if f != NOT_RUN or os.path.exists(os.path.join(d, f))])


def harness_list():
    out = run(["bash", os.path.join(ROOT, SP, "kani/run.sh"), "list"])
    return [h for h in out.split() if h]


def mutant_patches():
    d = os.path.join(ROOT, SP, "kani/mutants")
    return sorted(f[:-len(".patch")] for f in os.listdir(d) if f.endswith(".patch"))


def mutant_expected(name):
    """(expect, plain, kani) that run.sh prints for a mutant as its patch requires."""
    with open(os.path.join(ROOT, SP, "kani/mutants", name + ".patch")) as f:
        survive = any(l.rstrip("\n") == "# must-survive" for l in f)
    return ("survive", "survived", "survived") if survive else ("killed", "killed", "killed")


def dups(xs):
    seen, out = set(), []
    for x in xs:
        if x in seen and x not in out:
            out.append(x)
        seen.add(x)
    return out


def words(xs):
    return " ".join(sorted(xs))


def check(stale_ok_harnesses, stale_ok_mutants):
    errs, warns = [], []
    prov, inputs, jobs, covers, not_run = read_record()
    rec_h = [j["name"] for j in jobs if j["kind"] == "harness"]
    rec_m = [j["name"] for j in jobs if j["kind"] == "mutant"]
    decl_m = [n["mutant"] for n in not_run]

    # The record is what record.py wrote: its inputs hash to its
    # fingerprint, its results and covers to its digest.
    if prov.get("fingerprint") != digest_lines(inputs):
        errs.append("provenance.txt: the fingerprint is not the hash of its input lines (edited by hand?): "
                    "write the record with kani/local/record.py")
    if prov.get("digest") != record_digest():
        errs.append("jobs.tsv, covers.tsv or not-run.tsv is not what record.py wrote (digest in provenance.txt): "
                    "write the record with kani/local/record.py")
    # ... and its fingerprint is its commit's, where this clone has that
    # commit (a shallow CI checkout may not: skipped).
    commit = prov.get("commit", "")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        errs.append("provenance.txt: commit %r is not a full commit id: write the record with kani/local/record.py"
                    % commit)
    elif is_commit(commit):
        try:
            commit_fp = fingerprint(commit)[1]
        except Fail:
            commit_fp = None
        if commit_fp != prov.get("fingerprint"):
            errs.append("provenance.txt: the fingerprint is not commit %s's (recomputed here), so the record is not "
                        "that commit's run: write the record with kani/local/record.py" % commit[:12])
    else:
        commit = None

    # Fingerprint: the record's inputs are this commit's. A stale record is
    # a warning only if this run verifies all it records (kani-large its
    # harnesses, kani-mutants-large its mutants).
    head, head_fp = fingerprint(REV)
    if prov.get("fingerprint") != head_fp:
        have = dict(inputs)
        changed = [n for n, v in head if have.get(n) != v] or ["the input list"]
        files = []
        if commit:
            files = run(["git", "-C", ROOT, "diff", "--no-renames", "--name-only", commit, REV, "--"]
                        + pathspecs()).split()
        msg = ("the local Kani record (commit %s) is for other Kani inputs than this commit's: %s changed%s; "
               % (prov.get("commit", "?")[:12], ", ".join(changed),
                  " (%s%s)" % (", ".join(files[:8]), ", ..." if len(files) > 8 else "") if files else ""))
        unverified = []
        if rec_h and not stale_ok_harnesses:
            unverified.append("its %d harnesses (no kani-large jobs)" % len(rec_h))
        if rec_m and not stale_ok_mutants:
            unverified.append("its %d mutants (no kani-mutants-large jobs)" % len(rec_m))
        if unverified:
            # kani-mutants-large runs only on a manual run with `mutants`, so
            # without a new record a pull request or push fails here even
            # on KANI_RUNNER.
            errs.append(msg + "nothing in this run verifies " + " or ".join(unverified) + "; " + RERUN
                        + (" (on KANI_RUNNER, a manual run with `kani` and `mutants` verifies them instead, for "
                           "that run only)" if stale_ok_harnesses else ""))
        else:
            warns.append(msg + "this run's kani-large and kani-mutants-large jobs verify what it records; " + RERUN)
    if REV == "HEAD":
        dirty = subprocess.run(["git", "-C", ROOT, "diff", "--quiet", "HEAD", "--"] + pathspecs(), capture_output=True)
        if dirty.returncode == 1:
            warns.append("uncommitted changes to the Kani inputs: the fingerprint is HEAD's and does not see them")

    # Results: every harness ok, every mutant as its patch expects.
    patches = mutant_patches()
    for j in jobs:
        if j["kind"] == "harness":
            want = "ok-FAILED" if j["name"] == "h_canary" else "ok"
            if j["result"] != want:
                errs.append("recorded harness %s: %s, not %s" % (j["name"], j["result"], want))
        elif j["kind"] == "mutant":
            if j["name"] not in patches:
                errs.append("recorded mutant %s: no mutants/%s.patch" % (j["name"], j["name"]))
                continue
            want = "/".join(mutant_expected(j["name"]))
            if j["result"] != want:
                errs.append("recorded mutant %s: expect/plain/kani %s, not %s" % (j["name"], j["result"], want))
        else:
            errs.append("jobs.tsv: kind %r of %s is neither harness nor mutant" % (j["kind"], j["name"]))
    for c in covers:
        if c["harness"] not in rec_h:
            errs.append("covers.tsv: a cover of %s, which is not a recorded harness" % c["harness"])
    for n in not_run:
        if n["mutant"] not in patches:
            errs.append("%s: %s is not a mutants/*.patch (only a kani-mutants-large mutant can be declared not run)"
                        % (NOT_RUN, n["mutant"]))
        if not n["reason"].strip():
            errs.append("%s: %s has no reason" % (NOT_RUN, n["mutant"]))

    # Harnesses: the kani groups and the record split run.sh list.
    wf = matrices()
    for job in ("kani", "kani-large", "kani-mutants", "kani-mutants-large"):
        if job not in wf:
            errs.append("%s: no matrix entries for the %s job" % (os.path.relpath(WORKFLOW, ROOT), job))
    want = harness_list()
    ci_h = [h for e in wf.get("kani", []) for h in e["name"].split()]
    large_h = [h for e in wf.get("kani-large", []) for h in e["name"].split()]
    for what, xs in (("in two kani groups", ci_h), ("recorded twice", rec_h), ("in two kani-large groups", large_h)):
        if dups(xs):
            errs.append("harnesses %s: %s" % (what, words(dups(xs))))
    both = set(ci_h) & set(rec_h)
    missing = set(want) - set(ci_h) - set(rec_h)
    extra = (set(ci_h) | set(rec_h)) - set(want)
    if both:
        errs.append("harnesses in a kani group and in the local record: %s" % words(both))
    if missing:
        errs.append("harnesses of run.sh list in no kani group and not in the local record: %s" % words(missing))
    if extra:
        errs.append("not harnesses of run.sh list: %s" % words(extra))
    if set(large_h) != set(rec_h):
        errs.append("the kani-large groups must be the recorded harnesses: only in kani-large: %s; only recorded: %s"
                    % (words(set(large_h) - set(rec_h)) or "-", words(set(rec_h) - set(large_h)) or "-"))

    # Mutants: kani-mutants and the record split mutants/*.patch.
    def resolve(job):
        """[(entry, the mutants/*.patch names it matches)] of job's matrix."""
        out = []
        for e in wf.get(job, []):
            ms = [p for p in patches if p.startswith(e["name"] + "-")]
            if len(ms) != 1:
                errs.append("%s entry %s: matches %d mutants/*.patch, want 1" % (job, e["name"], len(ms)))
            out.append((e, ms))
        return out
    ci_e, large_e = resolve("kani-mutants"), resolve("kani-mutants-large")
    ci_m, large_m = [p for _, ms in ci_e for p in ms], [p for _, ms in large_e for p in ms]
    for what, xs in (("in kani-mutants twice", ci_m), ("recorded twice", rec_m), ("in kani-mutants-large twice", large_m),
                     ("declared not run twice", decl_m)):
        if dups(xs):
            errs.append("mutants %s: %s" % (what, words(dups(xs))))
    both = set(ci_m) & set(rec_m)
    missing = set(patches) - set(ci_m) - set(rec_m) - set(decl_m)
    if both:
        errs.append("mutants in kani-mutants and in the local record: %s" % words(both))
    if set(decl_m) & set(rec_m):
        errs.append("mutants declared not run (%s) and in the local record: %s" % (NOT_RUN, words(set(decl_m) & set(rec_m))))
    if set(decl_m) & set(ci_m):
        errs.append("mutants declared not run (%s) and in kani-mutants: %s" % (NOT_RUN, words(set(decl_m) & set(ci_m))))
    if missing:
        errs.append("mutants/*.patch in neither kani-mutants nor the local record: %s" % words(missing))
    if set(large_m) != set(rec_m) | set(decl_m):
        errs.append("kani-mutants-large must be the recorded mutants and those declared not run (%s): only in "
                    "kani-mutants-large: %s; only recorded or declared: %s"
                    % (NOT_RUN, words(set(large_m) - set(rec_m) - set(decl_m)) or "-",
                       words((set(rec_m) | set(decl_m)) - set(large_m)) or "-"))
    # Declared not run: loud, every time.
    for n in not_run:
        warns.append("kani-mutants-large mutant %s was NOT RUN locally (declared in kani/local/%s: %s): the local "
                     "record does not show its Kani leg" % (n["mutant"], NOT_RUN, n["reason"]))

    # Limits: the record shows that a large job passes within the limit_s
    # it ran under (its matrix timeout then), so within any timeout at
    # least that, but not within a lower one. A stale_ok flag means this
    # run's large jobs verify them under the current timeouts: a warning.
    rec_limit = {}
    for j in jobs:
        if j["kind"] not in ("harness", "mutant"):
            continue
        if re.fullmatch(r"[0-9]+", j["limit_s"]):
            rec_limit[(j["kind"], j["name"])] = int(j["limit_s"])
        else:
            errs.append("recorded %s %s: limit_s %r is not a whole number of seconds: write the record with "
                        "kani/local/record.py" % (j["kind"], j["name"], j["limit_s"]))
    for job, kind, stale_ok, entries in (
            ("kani-large", "harness", stale_ok_harnesses, [(e, e["name"].split()) for e in wf.get("kani-large", [])]),
            ("kani-mutants-large", "mutant", stale_ok_mutants, large_e)):
        for e, names in entries:
            # A name the record lacks: the membership checks above report it.
            over = [n for n in names if rec_limit.get((kind, n), 0) > e["timeout"]]
            if not over:
                continue
            ran = max(rec_limit[(kind, n)] for n in over)
            msg = ("%s entry %r: timeout %d is below the %d s the local record ran %s under; "
                   % (job, e["name"], e["timeout"], ran, ", ".join(over)))
            fix = "raise it back to %d or more, or %s" % (ran, RERUN)
            if stale_ok:
                warns.append(msg + "this run's %s job verifies %s in %d s, but the record does not show it: %s"
                             % (job, "them" if len(over) > 1 else "it", e["timeout"], fix))
            else:
                errs.append(msg + "the record does not show that %s in %d s: %s"
                            % ("they pass" if len(over) > 1 else "it passes", e["timeout"], fix))

    # The hosted jobs' entries fit the hosted runner in memory.
    for job in ("kani", "kani-mutants"):
        for e in wf.get(job, []):
            if e["gb"] > IN_MEMORY_GB:
                errs.append("%s entry %r peaked at %d GB, above %d GB: it does not fit ubuntu-latest in memory; "
                            "move it to %s-large and the local record" % (job, e["name"], e["gb"], IN_MEMORY_GB, job))

    # RESULTS.md has the block, and it is the record's table: the
    # human-readable record goes with the data.
    md = os.path.relpath(RESULTS_MD, ROOT)
    block = "kani/local/record.py table, between %s ...--> and %s" % (BEGIN, END)
    if not os.path.exists(RESULTS_MD):
        errs.append("%s does not exist; it must hold the record's table (%s)" % (md, block))
    else:
        with open(RESULTS_MD) as f:
            text = f.read()
        m = re.search(re.escape(BEGIN) + r"[^\n]*\n(.*?)^" + re.escape(END), text, re.S | re.M)
        if BEGIN not in text:
            errs.append("%s has no local-record block: %s" % (md, block))
        elif not m:
            errs.append("%s: a local-record block opens but does not close (%s)" % (md, END))
        else:
            table = run([sys.executable, "-I", RECORD_PY, "table"])
            if m.group(1).strip() != table.strip():
                errs.append("%s: the local-record block is not the record's table: %s" % (md, block))

    gh = bool(os.environ.get("GITHUB_ACTIONS"))
    for w in warns:
        print(("::warning title=local Kani record::" if gh else "warning: ") + w)
    for e in errs:
        print(("::error title=local Kani record::" if gh else "error: ") + e)
    if errs:
        return 1
    print("local Kani record ok: commit %s, %s; %d harnesses ok, %d mutants as expected%s; with the kani groups "
          "%d harnesses of run.sh list once each, with kani-mutants %d mutants once each"
          % (prov.get("commit", "?")[:12], "fingerprint " + head_fp[:12] if prov.get("fingerprint") == head_fp
             else "STALE (warning above)", len(rec_h), len(rec_m),
             ", %d kani-mutants-large mutants NOT RUN (declared in %s; warnings above)" % (len(decl_m), NOT_RUN)
             if decl_m else "", len(want), len(patches)))
    return 0


STALE_OK = ("--stale-ok", "--stale-ok-harnesses", "--stale-ok-mutants")


def main(argv):
    args = argv[1:]
    is_check = args in ([], ["check"]) or (len(set(args)) == len(args) and all(a in STALE_OK for a in args))
    try:
        if is_check:
            return check("--stale-ok" in args or "--stale-ok-harnesses" in args,
                         "--stale-ok" in args or "--stale-ok-mutants" in args)
        if args[0] == "fingerprint" and len(args) <= 2:
            lines, fp = fingerprint(args[1] if len(args) == 2 else "HEAD")
            for n, v in lines:
                print("%s\t%s" % (n, v))
            print("fingerprint\t%s" % fp)
            return 0
        if args == ["pathspecs"]:
            print("\n".join(pathspecs()))
            return 0
        if args[0] == "cargo-configs" and len(args) <= 2:
            for e in cargo_above_entries(args[1] if len(args) == 2 else "HEAD"):
                print(e.split(b"\t", 1)[1].decode())
            return 0
        if args[0] == "matrix" and len(args) == 2:
            wf = matrices()
            if args[1] not in wf:
                raise Fail("%s: no matrix entries for the %s job" % (WORKFLOW, args[1]))
            for e in wf[args[1]]:
                print("%s\t%d\t%d" % (e["name"], e["gb"], e["timeout"]))
            return 0
        if args == ["harnesses"]:
            for j in read_record()[2]:
                if j["kind"] == "harness":
                    print(j["name"])
            return 0
        if args == ["covers"]:
            for c in read_record()[3]:
                print("%s\t%s\t%s" % (c["family"], c["cover"], c["status"]))
            return 0
    except Fail as e:
        # The check's errors go with its report; a subcommand's stdout is data.
        print(("::error title=local Kani record::" if os.environ.get("GITHUB_ACTIONS") else "error: ") + str(e),
              file=sys.stdout if is_check else sys.stderr)
        return 1
    print(__doc__.split("\n\n")[2], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
