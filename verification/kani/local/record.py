#!/usr/bin/env python3
"""shared-pool's local Kani record: assemble it from local runs, or print its tables.

usage: record.py [--dry-run] RUN_DIR [RUN_DIR...]
                           record the jobs of the workflow's kani-large and
                           kani-mutants-large matrices from runs of
                           run-local.py at one commit and fingerprint:
                           measurements (--all; stopped early is fine) or a
                           record's run. Each job comes from the latest run
                           (env.txt started) that finished and passed it
                           (results.tsv: exit 0, a harness ok, a mutant as
                           its patch expects): a failed result never
                           replaces a passed one, and provenance.txt names
                           every result not taken. A job that runs finished
                           but none passed is refused. The taken result
                           must agree with its logs (stdout.txt, covers.tsv,
                           the log of a mutant's Kani leg); its limit_s is
                           the KANI_TIMEOUT it ran under (progress.log's
                           start line, which must be jobs.json's timeout;
                           without one, jobs.json's timeout). A
                           kani-mutants-large mutant that no run
                           finished must be declared in not-run.tsv in this
                           directory (mutant<TAB>reason; a harness cannot
                           be); a declared one that a run finished is
                           refused. The commit must have HEAD's Kani inputs,
                           each run its Kani tree, core pin and Kani/CBMC
                           versions. Writes provenance.txt (every run's
                           env.txt), jobs.tsv and covers.tsv into this
                           directory and the new table into kani/RESULTS.md's
                           local-record block. Refuses (exit 1, the record
                           and RESULTS.md untouched) otherwise, if RESULTS.md
                           has no such block, or if
                           tools/scripts/check-kani-record.py fails on the
                           new record (as when a large matrix timeout is
                           below the limit_s its job ran under). --dry-run:
                           every check, nothing written.
       record.py table     print the record's Markdown: provenance, the
                           harness table and the mutant table (RESULTS.md's
                           local-record block)

A run directory is what run-local.py writes (env.txt, jobs.json,
progress.log, results.tsv, logs/<job>/: stdout.txt, covers.tsv, the Kani
logs). env: KANI_RECORD_DIR (default: this directory), KANI_RESULTS_MD
(default: kani/RESULTS.md), KANI_RECORD_WORKFLOW (the workflow whose
matrices name the jobs; check-kani-record.py's default).
"""
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
KANI = os.path.join(ROOT, "projects/shared-pool/kani")
CHECK = os.path.join(ROOT, "tools/scripts/check-kani-record.py")
RECORD = os.environ.get("KANI_RECORD_DIR") or HERE
RESULTS_MD = os.environ.get("KANI_RESULTS_MD") or os.path.join(KANI, "RESULTS.md")
JOBS_COLUMNS = ["kind", "name", "result", "seconds", "job_seconds", "cbmc_peak_gb",
                "tree_peak_gb", "limit_s", "covers", "kani_harnesses", "started_utc"]
COVERS_COLUMNS = ["family", "cover", "status", "harness"]
# Large mutants no run finished, each with its reason (this directory; a
# harness cannot be declared). Part of the digest when present.
NOT_RUN = "not-run.tsv"
NOT_RUN_COLUMNS = ["mutant", "reason"]
BEGIN = "<!-- kani-local-record: begin (kani/local/record.py writes this block; do not edit) -->"
END = "<!-- kani-local-record: end -->"
RESULTS_TSV = ["job", "kind", "lane", "start_utc", "secs", "rc", "verdict", "peak_tree_gb",
               "peak_cbmc_gb", "est_peak_gb", "est_secs"]


class Refuse(Exception):
    pass


def out(*args, **kw):
    r = subprocess.run(list(args), capture_output=True, text=True, **kw)
    if r.returncode != 0:
        raise Refuse("%s: exit %d: %s" % (" ".join(args), r.returncode, (r.stderr or r.stdout).strip()))
    return r.stdout


def git(*args):
    return out("git", "-C", ROOT, *args).strip()


def fingerprint(rev):
    """[(name, value)], fingerprint: check-kani-record.py's, the one source."""
    lines = out(sys.executable, "-I", CHECK, "fingerprint", rev).splitlines()
    pairs = [tuple(l.split("\t", 1)) for l in lines]
    return pairs[:-1], pairs[-1][1]


def read_env(path):
    env = {}
    with open(path) as f:
        for line in f:
            k, _, v = line.strip().partition(" ")
            if k:
                env[k] = v.strip()
    return env


def parse_iso(s):
    return datetime.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ")


def mutant_header(name):
    """(expected (expect, plain, kani), [Kani harnesses in header order])."""
    with open(os.path.join(KANI, "mutants", name + ".patch")) as f:
        text = f.read()
    survive = re.search(r"^# must-survive$", text, re.M) is not None
    hs = []
    for m in re.finditer(r"^# (?:killed-by|checked-by): (.*)$", text, re.M):
        hs += m.group(1).split()
    exp = ("survive", "survived", "survived") if survive else ("killed", "killed", "killed")
    return exp, hs


# progress.log's line for a job's start: "<time> start <job> on <lane> (peak .., est .., limit N s)".
START = re.compile(r"^\S+ start (\S+) on (\S+) \(.*\blimit (\d+) s\)$")


def read_run(run):
    """A run directory of run-local.py, complete or not: dict(dir, env, jobs {name: job}, results {name: row},
    limits {name: KANI_TIMEOUT from its start line}, status, finished, finished_note)."""
    for f in ("env.txt", "jobs.json", "results.tsv", "progress.log"):
        if not os.path.exists(os.path.join(run, f)):
            raise Refuse("%s: no %s (not a run-local.py output directory)" % (run, f))
    env = read_env(os.path.join(run, "env.txt"))
    with open(os.path.join(run, "jobs.json")) as f:
        jobs = {}
        for j in json.load(f):
            if j["name"] in jobs:
                raise Refuse("%s/jobs.json: %s twice" % (run, j["name"]))
            jobs[j["name"]] = j
    results = {}
    with open(os.path.join(run, "results.tsv")) as f:
        for n, line in enumerate(f, 1):
            if not line.strip():
                continue
            r = line.rstrip("\n").split("\t")
            if len(r) != len(RESULTS_TSV):
                raise Refuse("%s/results.tsv:%d: %d fields, want %d" % (run, n, len(r), len(RESULTS_TSV)))
            r = dict(zip(RESULTS_TSV, r))
            if r["job"] in results:
                raise Refuse("%s/results.tsv: %s finished twice" % (run, r["job"]))
            results[r["job"]] = r
    with open(os.path.join(run, "progress.log")) as f:
        log = [l.strip() for l in f if l.strip()]
    unknown = sorted(set(results) - set(jobs))
    if unknown:
        raise Refuse("%s/results.tsv: jobs not in jobs.json: %s" % (run, " ".join(unknown)))
    limits = {}
    for l in log:
        m = START.match(l)
        if m:
            if m.group(1) in limits:
                raise Refuse("%s/progress.log: %s started twice" % (run, m.group(1)))
            limits[m.group(1)] = int(m.group(3))
    # An --all run's estimates.tsv has each job's limit too (after --estimates-from raised it).
    est_limits = {}
    if os.path.exists(os.path.join(run, "estimates.tsv")):
        with open(os.path.join(run, "estimates.tsv")) as f:
            rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
        if not rows or rows[0][:5] != ["job", "kind", "est_peak_gb", "est_secs", "limit_s"]:
            raise Refuse("%s/estimates.tsv: not run-local.py's (job kind est_peak_gb est_secs limit_s ...)" % run)
        est_limits = {r[0]: r[4] for r in rows[1:] if len(r) >= 5}
    if log and log[-1].endswith("ALL DONE"):
        status = "complete (ALL DONE): %d of %d jobs finished" % (len(results), len(jobs))
    else:
        status = ("not complete (no ALL DONE): %d of %d jobs finished; progress.log ends %r"
                  % (len(results), len(jobs), log[-1] if log else ""))
    ends = [parse_iso(r["start_utc"]) + datetime.timedelta(seconds=int(r["secs"])) for r in results.values()]
    if env.get("finished"):
        finished, note = env["finished"], "env.txt"
    elif ends:
        finished, note = max(ends).strftime("%Y-%m-%dT%H:%M:%SZ"), "the last finished job's end; env.txt has none"
    else:
        finished, note = env.get("started", ""), "no job finished"
    m = re.search(r"budget ([\d.]+) GB", log[0]) if log else None
    env.setdefault("budget-gb", "%g" % float(m.group(1)) if m else "?")
    env.setdefault("lanes", str(len({r["lane"] for r in results.values()})))
    return dict(dir=run, env=env, jobs=jobs, results=results, limits=limits, est_limits=est_limits, status=status,
                finished=finished, finished_note=note)


def job_row(run, name, kind, patch, problems):
    """(jobs.tsv row, covers rows) of the job name that run finished, or None with problems appended."""
    probs = []
    got = job_checked(run, name, kind, patch, probs)
    problems += ["%s (in %s)" % (p, run["label"]) for p in probs]
    return None if probs else got


def job_checked(run, name, kind, patch, problems):
    j, r = run["jobs"][name], run["results"][name]
    where = name
    d = os.path.join(run["dir"], "logs", name)
    try:
        with open(os.path.join(d, "stdout.txt")) as f:
            so = f.read()
    except OSError:
        so = ""
    if j["kind"] != kind:
        problems.append("%s: kind %s in jobs.json, not %s" % (where, j["kind"], kind))
        return None
    if r["rc"] != "0":
        problems.append("%s: run.sh exit %s (%s)" % (where, r["rc"], r["verdict"]))
    # Its limit: the KANI_TIMEOUT it ran under, as progress.log's start line,
    # jobs.json and (an --all run) estimates.tsv all say. run-local.py runs
    # a job with jobs.json's timeout as KANI_TIMEOUT, so without a start line
    # (a progress.log that has none with the limit) that is its limit.
    limit, src = run["limits"].get(name), "progress.log's"
    if limit is None:
        if not re.fullmatch(r"[0-9]+", str(j.get("timeout", ""))):
            problems.append("%s: no start line with its limit in progress.log, and jobs.json's timeout %r is not "
                            "whole seconds" % (where, j.get("timeout")))
            return None
        limit, src = int(j["timeout"]), "jobs.json's"
    if str(limit) != str(j["timeout"]):
        problems.append("%s: progress.log's limit %d s is not jobs.json's timeout %s" % (where, limit, j["timeout"]))
    elif run["est_limits"] and run["est_limits"].get(name) != str(limit):
        problems.append("%s: estimates.tsv's limit %s is not %s %d s"
                        % (where, run["est_limits"].get(name, "(none)"), src, limit))
    if kind == "harness":
        want = "ok-FAILED" if name == "h_canary" else "ok"
        m = re.search(r"^%s\s+(\S+)\s+(\d+)\s+(.*)$" % re.escape(name), so, re.M)
        if r["verdict"] != want or not m or m.group(1) != want:
            got = (r["verdict"] if r["verdict"] != want else
                   "%s in stdout.txt" % m.group(1) if m else "no result line in stdout.txt")
            problems.append("%s: %s, not %s" % (where, got, want))
            return None
        if "family check deferred" not in so:
            problems.append("%s: not run with KANI_DEFER_COVERS=1 (no covers.tsv)" % where)
            return None
        cov = re.sub(r" cover properties satisfied$", "", m.group(3).strip())
        covers = []
        try:
            with open(os.path.join(d, "covers.tsv")) as f:
                for line in f:
                    if line.strip():
                        c = line.rstrip("\n").split("\t")
                        if len(c) != 3:
                            raise Refuse("%s/covers.tsv: %r is not family<TAB>cover<TAB>status" % (d, line))
                        covers.append(c + [name])
        except OSError:
            problems.append("%s: no covers.tsv" % where)
            return None
        return dict(kind="harness", name=name, result=want, seconds=m.group(2), job_seconds=r["secs"],
                    cbmc_peak_gb=r["peak_cbmc_gb"], tree_peak_gb=r["peak_tree_gb"], limit_s=str(limit),
                    covers=cov, kani_harnesses="-", started_utc=r["start_utc"]), covers
    nn = name.split("-", 1)[1]
    lines = re.findall(r"^(%s-\S+)\s+(\S+)\s+(\S+)\s+(\S+)(.*)$" % re.escape(nn), so, re.M)
    if len(lines) != 1:
        problems.append("%s: %d result lines for mutants/%s-*.patch in stdout.txt, want 1" % (where, len(lines), nn))
        return None
    full, e, p, k, rest = lines[0]
    if full != patch:
        problems.append("%s: ran mutants/%s.patch, not the workflow's %s" % (where, full, patch))
        return None
    exp, header = mutant_header(full)
    got = "%s/%s/%s" % (e, p, k)
    if (e, p, k) != exp or "UNEXPECTED" in rest or r["verdict"] != got:
        problems.append("%s (%s): expect/plain/kani %s%s, want %s"
                        % (where, full, got, " UNEXPECTED" if "UNEXPECTED" in rest else "", "/".join(exp)))
        return None
    # Its Kani leg ran: run.sh keeps a log per harness (KEEP_LOGS).
    ran = [h for h in header if os.path.exists(os.path.join(d, "%s.%s.log" % (full, h)))]
    if not ran:
        problems.append("%s (%s): no log of its Kani leg in logs/%s (%s)"
                        % (where, full, name, ", ".join("%s.%s.log" % (full, h) for h in header)
                           or "its patch names no killed-by or checked-by harness"))
        return None
    return dict(kind="mutant", name=full, result=got, seconds=r["secs"], job_seconds=r["secs"],
                cbmc_peak_gb=r["peak_cbmc_gb"], tree_peak_gb=r["peak_tree_gb"], limit_s=str(limit), covers="-",
                kani_harnesses=",".join(ran), started_utc=r["start_utc"]), []


def matrix(job):
    """[(name, gb, timeout)] of the workflow's job matrix (check-kani-record.py's reading)."""
    return [tuple(l.split("\t")) for l in out(sys.executable, "-I", CHECK, "matrix", job).splitlines() if l]


def large_jobs():
    """[(run-local.py job name, kind, recorded name)]: kani-large's harnesses, kani-mutants-large's mutants."""
    patches = sorted(f[:-len(".patch")] for f in os.listdir(os.path.join(KANI, "mutants")) if f.endswith(".patch"))
    jobs = [(h, "harness", h) for g, _, _ in matrix("kani-large") for h in g.split()]
    for nn, _, _ in matrix("kani-mutants-large"):
        ms = [p for p in patches if p.startswith(nn + "-")]
        if len(ms) != 1:
            raise Refuse("kani-mutants-large entry %s matches %d mutants/*.patch, want 1" % (nn, len(ms)))
        jobs.append(("mutant-" + nn, "mutant", ms[0]))
    names = [j[0] for j in jobs]
    twice = sorted({x for x in names if names.count(x) > 1})
    if twice:
        raise Refuse("in the large matrices twice: %s" % " ".join(twice))
    return jobs


def read_not_run(d):
    """[(mutant, reason)] that d/not-run.tsv declares not run ([] without the file)."""
    p = os.path.join(d, NOT_RUN)
    if not os.path.exists(p):
        return []
    with open(p) as f:
        rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    if not rows or rows[0] != NOT_RUN_COLUMNS:
        raise Refuse("%s: the header must be %s" % (p, "<TAB>".join(NOT_RUN_COLUMNS)))
    decl = []
    for n, r in enumerate(rows[1:], 2):
        if len(r) != 2 or not r[0].strip() or not r[1].strip():
            raise Refuse("%s:%d: want mutant<TAB>reason (the full patch name; a reason)" % (p, n))
        if r[0] in [m for m, _ in decl]:
            raise Refuse("%s:%d: %s declared twice" % (p, n, r[0]))
        decl.append((r[0], r[1].strip()))
    return decl


def passed(run, name, kind, rec):
    """Whether run's results.tsv row of the job passes: exit 0, a harness ok (h_canary ok-FAILED), a mutant as
    its patch expects. Which run a job is taken from; job_checked then checks that result against its logs."""
    r = run["results"][name]
    if kind == "harness":
        want = "ok-FAILED" if name == "h_canary" else "ok"
    else:
        want = "/".join(mutant_header(rec)[0])
    return r["rc"] == "0" and r["verdict"] == want


def assemble(runs):
    """The large jobs from the runs: rows, covers, superseded [(job, run not taken, run taken)], declared,
    notes."""
    large = large_jobs()
    declared = read_not_run(RECORD)
    problems, rows, covers, superseded = [], [], [], []
    large_m = {p for _, k, p in large if k == "mutant"}
    for m, _ in declared:
        if m not in large_m:
            problems.append("%s declares %s not run, but it is not a kani-mutants-large mutant (only those can be "
                            "declared)" % (NOT_RUN, m))
    dset = dict(declared)
    for name, kind, rec in large:
        have = [r for r in runs if name in r["results"]]
        if kind == "mutant" and rec in dset:
            if have:
                problems.append("%s (%s) is declared not run in %s, but %s: record it (take it out of %s)"
                                % (name, rec, NOT_RUN, "; ".join("%s finished it (%s, rc %s)" % (
                                    r["label"], r["results"][name]["verdict"], r["results"][name]["rc"])
                                    for r in have), NOT_RUN))
            continue
        if not have:
            problems.append("%s: in no run %s" % (name, "and not declared in " + NOT_RUN if kind == "mutant" else
                                                  "(a kani-large harness: only a mutant can be declared not run)"))
            continue
        # The latest run (have is in env.txt started order) whose result
        # passed: a failed result never replaces a passed one. None passed:
        # every run's result is a problem.
        ok = [r for r in have if passed(r, name, kind, rec)]
        if not ok:
            for r in have:
                job_row(r, name, kind, rec, problems)
            continue
        run = ok[-1]
        superseded += [(name, e, run) for e in have if e is not run]
        got = job_row(run, name, kind, rec, problems)
        if got:
            got[0]["run"] = run["label"]  # not a jobs.tsv column: provenance's runN.recorded
            rows.append(got[0])
            covers += got[1]
    if problems:
        raise Refuse("the large jobs are not all run and passed, not recorded:\n  " + "\n  ".join(problems))
    # Failed jobs of the runs that are not large jobs (CI's hosted jobs run them): said, not recorded.
    names = {n for n, _, _ in large}
    notes = ["%s %s: rc %s, %s (not a large job: not recorded; CI's hosted jobs run it)"
             % (r["label"], n, x["rc"], x["verdict"]) for r in runs for n, x in sorted(r["results"].items())
             if n not in names and x["rc"] != "0"]
    # Order: harnesses as run.sh lists them, then mutants by name.
    order = {h: i for i, h in enumerate(out("bash", os.path.join(KANI, "run.sh"), "list").split())}
    rows.sort(key=lambda x: (x["kind"] != "harness", order.get(x["name"], len(order)), x["name"]))
    pos = {x["name"]: i for i, x in enumerate(rows)}
    covers.sort(key=lambda c: pos[c[3]])  # stable: each harness's covers in its log's order
    return rows, covers, superseded, declared, notes


def check_runs(runs):
    """Check the runs' commit, fingerprint and tools against HEAD; return (commit, kani tree, inputs, fp)."""
    commits = {r["env"].get("commit", "") for r in runs}
    if len(commits) != 1:
        raise Refuse("the runs are at different commits (%s): record runs of one commit"
                     % ", ".join("%s %s" % (r["label"], r["env"].get("commit", "?")) for r in runs))
    fps = {r["env"]["fingerprint"] for r in runs if r["env"].get("fingerprint")}
    if len(fps) > 1:
        raise Refuse("the runs have different fingerprints (%s): record runs of one commit and fingerprint"
                     % ", ".join("%s %s" % (r["label"], r["env"].get("fingerprint", "-")) for r in runs))
    commit = commits.pop()
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise Refuse("env.txt: commit %r is not a full commit id" % commit)
    if subprocess.run(["git", "-C", ROOT, "cat-file", "-e", commit + "^{commit}"], capture_output=True).returncode:
        raise Refuse("env.txt: commit %s is not in this repository" % commit)
    problems = []
    kt = git("rev-parse", commit + ":projects/shared-pool/kani")
    cp = git("rev-parse", commit + ":projects/shared-pool/source-mirror")
    inputs, fp = fingerprint(commit)
    head, head_fp = fingerprint("HEAD")
    v = dict(inputs)
    kani_v, cbmc_v = v["tools/versions.env:KANI"], v["tools/versions.env:CBMC"]
    for r in runs:
        env, lab = r["env"], r["label"] + " (" + r["dir"] + ")"
        if env.get("kani-tree") != kt:
            problems.append("%s: env.txt kani-tree %s is not %s:projects/shared-pool/kani (%s)"
                            % (lab, env.get("kani-tree"), commit[:12], kt))
        if env.get("core-pin") != cp:
            problems.append("%s: env.txt core-pin %s is not %s's source-mirror gitlink (%s)"
                            % (lab, env.get("core-pin"), commit[:12], cp))
        if env.get("fingerprint") and env["fingerprint"] != fp:
            problems.append("%s: env.txt fingerprint %s is not commit %s's (%s)"
                            % (lab, env["fingerprint"], commit[:12], fp))
        if not re.search(r"(^|\s)%s(\s|$)" % re.escape(kani_v), env.get("kani", "")):
            problems.append("%s: env.txt kani %r is not Kani %s (versions.env)" % (lab, env.get("kani"), kani_v))
        cbmc = env.get("cbmc") or env.get("CBMC", "")
        if cbmc.split()[:1] != [cbmc_v]:
            problems.append("%s: env.txt cbmc %r is not CBMC %s (versions.env)" % (lab, cbmc, cbmc_v))
        for k in ("host", "started"):
            if not env.get(k):
                problems.append("%s: env.txt has no %s" % (lab, k))
    if fp != head_fp:
        hv = dict(head)
        problems.append("the runs' commit %s has other Kani inputs than HEAD (%s): run run-local.py at HEAD"
                        % (commit[:12], ", ".join(n for n, x in inputs if hv.get(n) != x)))
    if problems:
        raise Refuse("the runs' provenance does not hold:\n  " + "\n  ".join(problems))
    return commit, kt, inputs, fp


def provenance(runs, rows, superseded, declared):
    """The provenance key/values after the inputs: tools, times, every run's env.txt, how it was assembled."""
    def flat(x):
        return re.sub(r"\s+", " ", str(x)).strip()

    def each(key, default):
        return "; ".join(dict.fromkeys(flat(r["env"].get(key) or default) for r in runs))
    measure = all(r["env"].get("mode", "").startswith("measurement") for r in runs)
    nh = sum(1 for r in rows if r["kind"] == "harness")
    if len(runs) == 1 and not measure:
        parallel = ("%s jobs at a time, their measured peaks summing to at most %s GB"
                    % (runs[0]["env"].get("lanes"), runs[0]["env"].get("budget-gb")))
    else:
        parallel = ("%s %s of run-local.py%s (provenance.txt): %s"
                    % (len(runs), "run" if len(runs) == 1 else "runs", " --all" if measure else "",
                       "; ".join("%s jobs at a time within %s GB" % (r["env"].get("lanes"), r["env"].get("budget-gb"))
                                 for r in runs)))
    tail = [
        ("kani", each("kani", "")),
        ("cbmc", "; ".join(dict.fromkeys(flat(r["env"].get("cbmc") or r["env"].get("CBMC", "")) for r in runs))),
        ("rustc", each("rustc", "not recorded (the host's default toolchain, for the mutants' plain mode)")),
        ("host", each("host", "")),
        ("started", min(r["env"]["started"] for r in runs)),
        ("finished", max(r["finished"] for r in runs)),
        ("parallel", parallel),
        ("kani_max_gb", each("kani-max-gb", "40 (run.sh's default)")),
        ("kani_env", each("kani-env", "not recorded (the run's other KANI_* variables)")),
        ("cargo_config", each("cargo-config", "not recorded (cargo configuration outside the lanes)")),
        ("runner", each("runner", "sched.py, the prototype of kani/local/run-local.py (same jobs, lanes and "
                                  "sampling)")),
        ("jobs", "%d harnesses, %d mutants%s" % (nh, len(rows) - nh, ", %d kani-mutants-large mutants declared not "
                                                 "run (%s)" % (len(declared), NOT_RUN) if declared else "")),
        ("assembled", "%s: the jobs of the workflow's kani-large and kani-mutants-large matrices, each from the "
                      "latest run that finished and passed it (a failed result never replaces a passed one; the "
                      "results not taken: superseded.* lines)%s"
         % ("from %d measurement %s (run-local.py --all), not a separate --record run"
            % (len(runs), "run" if len(runs) == 1 else "runs") if measure else
            "from %d %s of run-local.py" % (len(runs), "run" if len(runs) == 1 else "runs"),
            "; kani-mutants-large mutants no run finished are declared in %s (not_run.* lines)" % NOT_RUN
            if declared else "")),
    ]
    for r in runs:
        lab = r["label"]
        took = [x["name"] for x in rows if x["run"] == lab]
        tail += [(lab, r["dir"]), (lab + ".status", r["status"]),
                 (lab + ".finished", "%s (%s)" % (r["finished"], r["finished_note"]))]
        tail += [(lab + "." + k, flat(v)) for k, v in r["env"].items() if k != "finished"]
        tail.append((lab + ".recorded", "%d jobs%s" % (len(took), ": " + " ".join(took) if took else "")))
    # One line per job: each result not taken, in run order (an earlier one
    # replaced by the later run's; a later one that did not pass).
    pos, nottaken = {id(r): i for i, r in enumerate(runs)}, {}
    for name, e, r in superseded:
        x = e["results"][name]
        if pos[id(e)] > pos[id(r)]:
            how = "not taken: it did not pass, and a failed result never replaces %s's passed one" % r["label"]
        else:
            how = "replaced by %s's (the later run)" % r["label"]
        nottaken.setdefault(name, []).append("%s's result (%s, rc %s, started %s) %s"
                                             % (e["label"], x["verdict"], x["rc"], x["start_utc"], how))
    for name, xs in nottaken.items():
        tail.append(("superseded." + name, "; ".join(xs)))
    for m, reason in declared:
        tail.append(("not_run." + m, flat(reason)))
    return [(k, flat(v)) for k, v in tail]


def tsv(columns, rows):
    return "\t".join(columns) + "\n" + "".join("\t".join(r) + "\n" for r in rows)


def digest(d):
    """sha256 of jobs.tsv, covers.tsv and, when there is one, not-run.tsv."""
    h = hashlib.sha256()
    for name in ("jobs.tsv", "covers.tsv", NOT_RUN):
        p = os.path.join(d, name)
        if name != NOT_RUN or os.path.exists(p):
            with open(p, "rb") as f:
                h.update(f.read())
    return h.hexdigest()


def write_record(d, meta, inputs, fp, tail, rows, covers):
    with open(os.path.join(d, "jobs.tsv"), "w") as f:
        f.write(tsv(JOBS_COLUMNS, [[r[c] for c in JOBS_COLUMNS] for r in rows]))
    with open(os.path.join(d, "covers.tsv"), "w") as f:
        f.write(tsv(COVERS_COLUMNS, covers))
    with open(os.path.join(d, "provenance.txt"), "w") as f:
        f.write("# shared-pool's local Kani record: the runs' provenance. Written by\n"
                "# projects/shared-pool/kani/local/record.py from runs of run-local.py; do\n"
                "# not edit. tools/scripts/check-kani-record.py checks it in CI: the input\n"
                "# lines (git object ids, tool versions) hash to fingerprint, which must be\n"
                "# HEAD's; digest is the sha256 of jobs.tsv, covers.tsv and not-run.tsv (if any).\n")
        for k, v in meta:
            f.write("%s\t%s\n" % (k, v))
        for n, v in inputs:
            f.write("input\t%s\t%s\n" % (n, v))
        f.write("fingerprint\t%s\ndigest\t%s\n" % (fp, digest(d)))
        for k, v in tail:
            f.write("%s\t%s\n" % (k, v))


def read_record(d):
    prov, inputs = {}, []
    with open(os.path.join(d, "provenance.txt")) as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            p = line.rstrip("\n").split("\t")
            if p[0] == "input":
                inputs.append((p[1], p[2]))
            else:
                prov[p[0]] = p[1]
    with open(os.path.join(d, "jobs.tsv")) as f:
        lines = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    return prov, dict(inputs), [dict(zip(lines[0], l)) for l in lines[1:]]


def table(d):
    prov, inputs, jobs = read_record(d)
    declared = read_not_run(d)
    hs = [j for j in jobs if j["kind"] == "harness"]
    ms = [j for j in jobs if j["kind"] == "mutant"]
    s = []
    s.append("Local record (`kani/local/`, %d harnesses and %d mutants): commit `%s`, Kani inputs fingerprint "
             "`%s` (CI recomputes it, `tools/scripts/check-kani-record.py`); Kani %s, CBMC %s; %s; %s to %s, %s. "
             "Time is wall clock per harness including compilation (a mutant: its whole run, plain mode "
             "included); peak is CBMC's resident memory, sampled every 3 s (it can understate the footprint: "
             "kani/local/README.md, Resident memory is not the footprint); limit is the job's KANI_TIMEOUT."
             % (len(hs), len(ms), prov["commit"][:7], prov["fingerprint"][:12],
                inputs.get("tools/versions.env:KANI"), inputs.get("tools/versions.env:CBMC"), prov["host"],
                prov["started"], prov["finished"], prov["parallel"]))
    s.append("")
    s.append("| Harness | Result | Time (s) | Peak (GB) | Limit (s) | Covers SATISFIED (this harness) |")
    s.append("|---|---|---:|---:|---:|---|")
    for j in hs:
        s.append("| `%s` | %s | %s | %s | %s | %s |" % (j["name"], j["result"], j["seconds"], j["cbmc_peak_gb"],
                                                      j["limit_s"], j["covers"]))
    if ms:
        s.append("")
        s.append("| Mutant | Expected | Plain | Kani (harness) | Time (s) | Peak (GB) | Limit (s) |")
        s.append("|---|---|---|---|---:|---:|---:|")
        for j in ms:
            e, p, k = j["result"].split("/")
            hn = ", ".join("`%s`" % h for h in j["kani_harnesses"].split(",") if h)
            s.append("| `%s` | %s | %s | %s (%s) | %s | %s | %s |" % (j["name"], e, p, k, hn, j["seconds"],
                                                                     j["cbmc_peak_gb"], j["limit_s"]))
    if declared:
        s.append("")
        s.append("**Not run locally** (%d kani-mutants-large %s declared in `kani/local/%s`; the record does not "
                 "show their Kani leg):" % (len(declared), "mutant" if len(declared) == 1 else "mutants", NOT_RUN))
        s.append("")
        for m, reason in declared:
            s.append("- `%s`: %s" % (m, reason))
    return "\n".join(s) + "\n"


def results_with_block(tbl):
    """RESULTS.md's text with the block rewritten, or None if it has no block."""
    if not os.path.exists(RESULTS_MD):
        return None
    with open(RESULTS_MD) as f:
        text = f.read()
    m = re.search(r"^<!-- kani-local-record: begin[^\n]*\n.*?^" + re.escape(END) + r"$", text, re.S | re.M)
    if not m:
        return None
    return text[:m.start()] + BEGIN + "\n" + tbl + END + text[m.end():]


def write(dirs, dry):
    dirs = [os.path.abspath(d) for d in dirs]
    if len({os.path.realpath(d) for d in dirs}) != len(dirs):
        raise Refuse("a run directory is given twice")
    runs = [read_run(d) for d in dirs]
    runs.sort(key=lambda r: (r["env"].get("started", ""), r["dir"]))
    for i, r in enumerate(runs, 1):
        r["label"] = "run%d" % i
    commit, kt, inputs, fp = check_runs(runs)
    rows, covers, superseded, declared, notes = assemble(runs)
    meta = [("commit", commit), ("kani_tree", kt)]
    tail = provenance(runs, rows, superseded, declared)
    tmp = tempfile.mkdtemp(prefix="kani-record.")
    try:
        if os.path.exists(os.path.join(RECORD, NOT_RUN)):
            shutil.copyfile(os.path.join(RECORD, NOT_RUN), os.path.join(tmp, NOT_RUN))
        write_record(tmp, meta, inputs, fp, tail, rows, covers)
        tbl = table(tmp)
        new_results = results_with_block(tbl)
        if new_results is None:
            raise Refuse("%s has no local-record block (the check requires one): put\n  %s\n  %s\n"
                         "into it, and record.py fills it" % (os.path.relpath(RESULTS_MD), BEGIN, END))
        with open(os.path.join(tmp, "RESULTS.md"), "w") as f:
            f.write(new_results)
        check_env = dict(os.environ, KANI_RECORD_DIR=tmp, KANI_RESULTS_MD=os.path.join(tmp, "RESULTS.md"))
        # The check's RESULTS.md block compares with `record.py table` of
        # KANI_RECORD_DIR, which this process's children inherit.
        r = subprocess.run([sys.executable, "-I", CHECK], env=check_env, capture_output=True, text=True)
        if r.returncode != 0:
            raise Refuse("the new record fails tools/scripts/check-kani-record.py:\n" + r.stdout + r.stderr)
        if not dry:
            os.makedirs(RECORD, exist_ok=True)
            for f in ("jobs.tsv", "covers.tsv", "provenance.txt"):
                shutil.copyfile(os.path.join(tmp, f), os.path.join(RECORD, f))
            with open(RESULTS_MD, "w") as f:
                f.write(new_results)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    nh = sum(1 for x in rows if x["kind"] == "harness")
    print("%s %d harnesses and %d mutants from %d %s (commit %s, fingerprint %s)%s"
          % ("dry run, nothing written: would record" if dry else "recorded", nh, len(rows) - nh, len(runs),
             "run" if len(runs) == 1 else "runs", commit[:12], fp[:12], "" if dry else " in " + os.path.relpath(RECORD)))
    for x in tail:
        if re.fullmatch(r"run\d+(\.status|\.recorded)?", x[0]):
            print("  %s: %s" % x)
    for name, e, later in superseded:
        print("  superseded: %s from %s (%s), not %s (%s)" % (name, later["label"], later["results"][name]["verdict"],
                                                             e["label"], e["results"][name]["verdict"]))
    for m, reason in declared:
        print("  NOT RUN (declared in %s): %s: %s" % (NOT_RUN, m, reason))
    for n in notes:
        print("  note: %s" % n)
    print(r.stdout.strip())
    if not dry:
        print("rewrote the local-record block of %s" % os.path.relpath(RESULTS_MD))
    return 0


def main(argv):
    args = argv[1:]
    try:
        if args == ["table"]:
            sys.stdout.write(table(RECORD))
            return 0
        dry = args[:1] == ["--dry-run"]
        dirs = args[1:] if dry else args
        if dirs and not any(d.startswith("-") for d in dirs):
            return write(dirs, dry)
    except Refuse as e:
        print("record.py: refused: %s" % e, file=sys.stderr)
        return 1
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
