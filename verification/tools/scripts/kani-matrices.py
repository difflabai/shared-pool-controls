#!/usr/bin/env python3
"""Write shared-pool's four Kani matrices from the local measurement.

usage: tools/scripts/kani-matrices.py [--write] [--rev REV] RUN_DIR...

The kani, kani-large, kani-mutants and kani-mutants-large matrices of
.github/workflows/shared-pool.yml (KANI_RECORD_WORKFLOW), regenerated from
REV's `kani/run.sh list` and kani/mutants/*.patch (default HEAD), and from
the results.tsv of each RUN_DIR, a run of kani/local/run-local.py --all
(a measurement; stopped early, or still running, is fine: only its
finished rows count) at REV's Kani fingerprint. A run directory with
another fingerprint is refused: its numbers describe other Kani inputs.
Only results.tsv is read as results (never estimates.tsv, which is
scheduling input).

Each harness's peak and local time, best source first: the latest run
(env.txt started) that passed it (peak: the larger of its process tree's
and its cbmc's sampled peaks; time: the job's seconds); CI_LARGE below (a
harness that a hosted runner could not hold: unknown, 40 GB and 1800 s);
W7's v1.1.0 measurements; v1.0.0's kani/RESULTS.md with the peak times
1.25 (the last two as run-local.py --all estimates them, so it shares its
V11_MEASURED table and rules). Its time on a hosted runner: CI run
CI_RUN's (CI_SECS), else HOSTED_X times the local time. A job's limit is
the KANI_TIMEOUT the run that passed it ran it under (progress.log's start
line, else jobs.json: the record's limit_s); not passed yet, the largest
of run-local.py --all's limit for it and the limits the given runs planned
for it (jobs.json, start lines: --estimates-from raises some), so that a
run still going needs no new timeout when it passes the job.

- kani: the harnesses whose peak, rounded up, is 13 GB or less
  (check-kani-record.py's IN_MEMORY_GB), in run.sh list order, packed into
  groups. A harness's timeout (KANI_TIMEOUT) is 3 times its hosted time,
  rounded up to the minute, at least 1800 s; a group's is its largest, its
  gb its largest peak rounded up. A group takes the next harness while it
  has at most MAX_GROUP harnesses, their count times the group's timeout
  fits the job's timeout-minutes less SETUP_MIN (so every harness may use
  all of its KANI_TIMEOUT), and their hosted times sum to at most
  HOSTED_GROUP_SECS (a short wall clock for CI).
- kani-large: the others; the CI_LARGE harnesses whatever their local
  peak (h_full_256), at gb CI_LARGE_GB or more; and h_wedge always (the
  record's self-test samples it), in a group of its own. A harness's timeout is the largest of
  1800 s, 3 times its local time (to the minute) and its limit (above):
  the record's limit_s, which no kani-large timeout may undercut
  (check-kani-record.py). Packed as kani's, with LARGE_GROUP_SECS of local
  time.
- kani-mutants, kani-mutants-large: every mutants/*.patch, sized by the
  harnesses of its Kani leg (its first killed-by; every checked-by of a
  `# must-survive` patch): gb their largest gb (or the mutant's own
  measured peak, if larger, rounded up). kani-mutants takes those at 13 GB
  or less whose leg runs no CI_LARGE harness (so mutant 21, killed by
  h_full_256, is always large); timeout the largest of 1800 s and the leg
  harnesses' hosted timeouts. kani-mutants-large takes the others;
  timeout the largest of 1800 s, the leg harnesses' kani-large timeouts
  and the mutant's limit (above).

Without --write it prints the matrices and the per-job sources; with
--write it also replaces the four matrices (and the comment lines between
each `matrix:` and its `include:`) in the workflow. The output depends only
on REV and the run directories' files (re-running on unchanged inputs
changes nothing). Either way it checks the result: every harness of
run.sh list once across kani and kani-large, every mutant once across the
mutant matrices; kani and kani-mutants at 13 GB or less; h_wedge alone in
kani-large; the CI_LARGE harnesses in kani-large and their mutants in
kani-mutants-large; every large timeout at least its job's limit; each
entry's harnesses times its timeout within its job's timeout-minutes less
SETUP_MIN. It lists the kani-large harnesses and kani-mutants-large
mutants that no run passed (the record needs a run of each harness; a
mutant may instead be declared in kani/local/not-run.tsv), and checks
not-run.tsv against the matrices (warnings: the record owner's file).

After a new measurement run, one command:
  tools/scripts/kani-matrices.py --write RUN_DIR1 RUN_DIR2 ...
Exit 0 on success, 1 on a refused run directory or a failed check, 2 on a
usage error.
"""
import importlib.util
import json
import math
import os
import re
import shutil
import sys
import tempfile

sys.dont_write_bytecode = True  # no __pycache__ next to the imported scripts

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
SP = "projects/shared-pool"
WORKFLOW = os.environ.get("KANI_RECORD_WORKFLOW") or os.path.join(ROOT, ".github/workflows/shared-pool.yml")
NOT_RUN_TSV = os.path.join(ROOT, SP, "kani/local/not-run.tsv")
JOBS = ("kani", "kani-large", "kani-mutants", "kani-mutants-large")

# Every harness of a group may use all of its KANI_TIMEOUT within the
# job's timeout-minutes less these minutes (setup, codegen, the upload).
SETUP_MIN = 30
MAX_GROUP = 8
# Hosted seconds per kani group (CI's wall clock: about 25 minutes of
# harnesses plus about 5 of setup), local seconds per kani-large group.
HOSTED_GROUP_SECS = 1500
LARGE_GROUP_SECS = 3600
LIMIT_MIN = 1800
# Kept in kani-large, alone in its group: test-check-kani-record.sh samples it.
ALWAYS_LARGE = "h_wedge"
# A harness's time on ubuntu-latest (4 CPUs) without a CI measurement: the
# hosted kani job took 1.0 to 1.3 times the v1.1.0 local job seconds and
# up to 2.3 times v1.0.0's RESULTS.md seconds (CI_RUN below).
HOSTED_X = 2.5
# CI run 43618869922 (PR #32, head 2d33d33's Kani inputs, ubuntu-latest:
# 4 CPUs, 15 GB, 15 GB swap): run.sh's seconds of each harness that passed
# in the kani job (v1.0.0's groups).
CI_RUN = "43618869922"
CI_SECS = {
    "h_canary": 167,
    "h_init_inv": 71, "h_init_inv_2": 80,
    "h_version_max_ack": 23, "h_version_max_cancel": 124, "h_version_max_settle": 375,
    "h_step_ack_tx": 84, "h_step_ack_post": 148, "h_step_ack_inv": 176, "h_step_ack_inv_0": 73,
    "h_step_ack_inv_1": 138, "h_step_ack_inv_3": 209, "h_step_ack_cov": 90,
    "h_step_dispatch_tx": 96, "h_step_dispatch_post": 148, "h_step_dispatch_inv": 197,
    "h_step_dispatch_inv_0": 72, "h_step_dispatch_inv_1": 168, "h_step_dispatch_inv_3": 244,
    "h_step_dispatch_cov": 116,
    "h_step_pause_tx": 73, "h_step_pause_post": 148, "h_step_pause_inv": 191, "h_step_pause_inv_0": 73,
    "h_step_pause_inv_1": 123, "h_step_pause_inv_3": 218, "h_step_pause_cov": 85,
    "h_step_observe_tx": 147, "h_step_observe_post": 173, "h_step_observe_inv": 187,
    "h_step_observe_inv_0": 90, "h_step_observe_inv_1": 141, "h_step_observe_cov": 116,
    "h_step_cancel_tx": 196, "h_step_cancel_inv_0": 61, "h_step_cancel_inv_1": 236, "h_step_cancel_cov": 161,
    "h_step_settle_inv_0": 61, "h_step_settle_cov": 300,
    "h_ref_submit": 663, "h_ref_dispatch": 97, "h_ref_ack": 83, "h_ref_cancel": 196, "h_ref_settle": 458,
    "h_ref_pause": 110, "h_ref_observe": 121,
}
# Harnesses a hosted runner could not hold: always kani-large (and a mutant
# whose leg runs one, kani-mutants-large), whatever the local peak; until
# a local run passes one, sized as unknown (run-local.py's 40 GB, 1800 s,
# limit 7200 s), as its v1.0.0 numbers are wrong for v1.1.0.
CI_LARGE = {
    "h_full_256": "CI runs 43600054037 and 43618869922: filled ubuntu-latest's 15 GB and 14-15 GB of swap, "
                  "runner lost after 13-17 minutes (v1.0.0: 46 s, 0.6 GB)",
}
# Their gb (and their mutants') is at least this: on Linux they used 15 GB
# of memory and about 15 GB of swap, whatever a local (macOS) run samples,
# so the runner-size step wants a KANI_RUNNER that holds that much.
CI_LARGE_GB = 30


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


rl = load(os.path.join(ROOT, SP, "kani/local/run-local.py"), "run_local")
ck = load(os.path.join(HERE, "check-kani-record.py"), "check_kani_record")
IN_MEMORY_GB = ck.IN_MEMORY_GB


class Refused(Exception):
    pass


def minutes(secs):
    """secs rounded up to a whole minute."""
    return -(-int(math.ceil(secs)) // 60) * 60


def gb_of(peak):
    return max(1, int(math.ceil(peak - 1e-9)))


def list_order(commit):
    """commit's `run.sh list`, in its order (its run.sh and src/proofs.rs in a temp dir, as run-local.py runs it)."""
    tmp = tempfile.mkdtemp(prefix="kani-matrices-list.")
    try:
        os.makedirs(os.path.join(tmp, "src"))
        for p in ("run.sh", "src/proofs.rs"):
            with open(os.path.join(tmp, p), "w") as f:
                f.write(rl.sh("git", "-C", ROOT, "show", "%s:%s/kani/%s" % (commit, SP, p)))
        return rl.sh("bash", os.path.join(tmp, "run.sh"), "list").split()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def read_run(d, fp):
    """A run-local.py --all directory at fingerprint fp: dict(dir, started, done, rows, passed {job: measurement},
    failed [text], planned {job: limit})."""
    env = {}
    try:
        with open(os.path.join(d, "env.txt")) as f:
            for line in f:
                k, _, v = line.strip().partition(" ")
                if k:
                    env[k] = v.strip()
    except OSError as e:
        raise Refused("%s: no env.txt (not a run-local.py --all directory): %s" % (d, e))
    if env.get("fingerprint") != fp:
        raise Refused("%s: fingerprint %s, not the Kani inputs' %s: a measurement of other Kani inputs"
                      % (d, (env.get("fingerprint") or "none")[:12], fp[:12]))
    if not env.get("mode", "").startswith("measurement"):
        raise Refused("%s: not a run-local.py --all measurement (env.txt has no `mode measurement`)" % d)
    if not env.get("started"):
        raise Refused("%s: env.txt has no `started`" % d)
    # The limit each job ran (or will run) under: its start line, else jobs.json.
    starts, done = {}, False
    progress = os.path.join(d, "progress.log")
    if os.path.exists(progress):
        with open(progress) as f:
            for line in f:
                m = re.match(r"^\S+ start (\S+) on \S+ \(.*\blimit (\d+) s\)$", line.strip())
                if m:
                    starts[m.group(1)] = int(m.group(2))
                done = done or "ALL DONE" in line
    try:
        with open(os.path.join(d, "jobs.json")) as f:
            planned = {j["name"]: int(j["timeout"]) for j in json.load(f)}
    except (OSError, ValueError, KeyError, TypeError) as e:
        raise Refused("%s: no readable jobs.json: %s" % (d, e))
    planned.update(starts)
    passed, failed, seen = {}, [], set()
    path = os.path.join(d, "results.tsv")
    rows = []
    if os.path.exists(path):
        with open(path) as f:
            rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    for r in rows:
        if len(r) != 11:
            raise Refused("%s: a row of %d fields, want 11" % (path, len(r)))
        name, kind, _lane, _start, secs, rc, verdict, tree, cbmc = r[:9]
        if name in seen:
            raise Refused("%s: %s finished twice" % (path, name))
        seen.add(name)
        if rc != "0" or not rl.passed({"name": name, "kind": kind}, verdict):
            failed.append("%s (%s, rc %s)" % (name, verdict, rc))
            continue
        passed[name] = dict(kind=kind, secs=int(float(secs)), peak=max(float(tree), float(cbmc)),
                            limit=planned.get(name), run=os.path.basename(os.path.normpath(d)))
    return dict(dir=d, started=env["started"], done=done, rows=len(rows), passed=passed, failed=failed,
                planned=planned)


def pack(names, timeout, secs, cap, budget):
    """names in order, next fit: a group takes the next while it fits MAX_GROUP, budget and cap seconds."""
    groups = []
    for h in names:
        g = groups[-1] if groups else None
        if g and len(g) < MAX_GROUP and (len(g) + 1) * max(timeout(x) for x in g + [h]) <= budget \
                and sum(secs(x) for x in g + [h]) <= cap:
            g.append(h)
        else:
            groups.append([h])
    return groups


def job_budgets(text):
    """{job: seconds}: each Kani job's timeout-minutes less SETUP_MIN, from the workflow's text."""
    lines = text.split("\n")
    out = {}
    for job in JOBS:
        start, end = job_span(lines, job)
        tm = [int(m.group(1)) for l in lines[start:end] for m in [re.match(r"^    timeout-minutes:\s*(\d+)\s*$", l)]
              if m]
        if len(tm) != 1:
            raise Refused("%s: the %s job has %d timeout-minutes lines, want 1" % (WORKFLOW, job, len(tm)))
        out[job] = (tm[0] - SETUP_MIN) * 60
    return out


def job_span(lines, job):
    """(first, end) line indexes of job's block under `jobs:`."""
    jobs = next((i for i, l in enumerate(lines) if l.rstrip() == "jobs:"), None)
    if jobs is None:
        raise Refused("%s: no `jobs:`" % WORKFLOW)
    start = next((i for i in range(jobs + 1, len(lines)) if lines[i].rstrip() == "  %s:" % job), None)
    if start is None:
        raise Refused("%s: no %s job" % (WORKFLOW, job))
    end = next((i for i in range(start + 1, len(lines)) if re.match(r"^  \S", lines[i])), len(lines))
    return start, end


def plan(rev, run_dirs, text):
    commit = rl.sh("git", "-C", ROOT, "rev-parse", "--verify", rev + "^{commit}").strip()
    fp = ck.fingerprint(commit)[1]
    jobs, _ = rl.jobs_all(commit)
    by = {j["name"]: j for j in jobs}
    harnesses = [j["name"] for j in jobs if j["kind"] == "harness"]
    order = list_order(commit)
    if sorted(order) != sorted(harnesses):
        raise Refused("%s's run.sh list is not run-local.py's harness list" % commit[:12])
    budget = job_budgets(text)

    runs = sorted((read_run(d, fp) for d in run_dirs), key=lambda r: (r["started"], r["dir"]))
    measured, planned = {}, {}
    for r in runs:
        measured.update(r["passed"])  # the latest run that passed a job wins (record.py's rule)
        for n, lim in r["planned"].items():
            planned[n] = max(planned.get(n, 0), lim)

    def unmeasured_limit(name, at_least):
        return max(at_least, planned.get(name, 0))

    H = {}
    for h in harnesses:
        j = by[h]
        e = dict(peak=j["peak"], secs=j["est"], limit=unmeasured_limit(h, j["timeout"]), source=j["source"],
                 measured=False)
        if h in measured:
            m = measured[h]
            e.update(peak=m["peak"], secs=m["secs"], limit=m["limit"] or j["timeout"], measured=True,
                     source="%s: %d s, %.1f GB, limit %d s" % (m["run"], m["secs"], m["peak"],
                                                                 m["limit"] or j["timeout"]))
        elif h in CI_LARGE:
            e.update(peak=rl.MAX_GB, secs=rl.UNKNOWN_SECS, limit=unmeasured_limit(h, rl.LIMIT_LONG),
                     source="unknown: " + CI_LARGE[h])
        e["gb"] = max(gb_of(e["peak"]), CI_LARGE_GB if h in CI_LARGE else 0)
        e["large"] = e["gb"] > IN_MEMORY_GB or h == ALWAYS_LARGE or h in CI_LARGE
        e["hosted_secs"] = CI_SECS[h] if h in CI_SECS else int(math.ceil(HOSTED_X * e["secs"]))
        e["t_hosted"] = max(LIMIT_MIN, minutes(3 * e["hosted_secs"]))
        e["t_large"] = max(LIMIT_MIN, minutes(3 * e["secs"]), e["limit"])
        H[h] = e

    small = [h for h in order if not H[h]["large"]]
    large = [h for h in order if H[h]["large"] and h != ALWAYS_LARGE]
    kani = pack(small, lambda h: H[h]["t_hosted"], lambda h: H[h]["hosted_secs"], HOSTED_GROUP_SECS,
                budget["kani"])
    kani_large = pack(large, lambda h: H[h]["t_large"], lambda h: H[h]["secs"], LARGE_GROUP_SECS,
                      budget["kani-large"])
    if ALWAYS_LARGE in H:
        kani_large.append([ALWAYS_LARGE])

    M = {}
    for j in jobs:
        if j["kind"] != "mutant":
            continue
        nn = j["name"].split("-", 1)[1]
        leg = j["leg"]
        unknown = [h for h in leg if h not in H]
        peak = max([H[h]["peak"] for h in leg if h in H] + ([rl.MAX_GB] if unknown or not leg else []))
        e = dict(patch=j["patch"], leg=leg, survive=j["survive"], measured=False,
                 limit=unmeasured_limit(j["name"], j["timeout"]))
        src = "%s %s" % ("checked-by" if j["survive"] else "killed-by", " + ".join(leg) or "none")
        if j["name"] in measured:
            m = measured[j["name"]]
            peak = max(peak, m["peak"])
            e.update(measured=True, limit=m["limit"] or j["timeout"])
            src += "; %s: %d s, %.1f GB, limit %d s" % (m["run"], m["secs"], m["peak"], e["limit"])
        e.update(peak=peak, gb=max([gb_of(peak)] + [H[h]["gb"] for h in leg if h in H]), source=src)
        e["ci_large"] = [h for h in leg if h in CI_LARGE]
        e["large"] = e["gb"] > IN_MEMORY_GB or bool(e["ci_large"])
        legs = [H[h] for h in leg if h in H]
        if e["large"]:
            e["timeout"] = max([LIMIT_MIN, e["limit"]] + [x["t_large"] for x in legs]
                               + ([rl.LIMIT_LONG] if unknown else []))
        else:
            e["timeout"] = max([LIMIT_MIN] + [x["t_hosted"] for x in legs])
        M[nn] = e

    def gentries(groups, key):
        return [dict(kind="group", name=" ".join(g), gb=max(H[h]["gb"] for h in g),
                     timeout=max(H[h][key] for h in g)) for g in groups]

    mats = {
        "kani": gentries(kani, "t_hosted"),
        "kani-large": gentries(kani_large, "t_large"),
        "kani-mutants": [dict(kind="mutant", name=nn, gb=e["gb"], timeout=e["timeout"])
                         for nn, e in sorted(M.items()) if not e["large"]],
        "kani-mutants-large": [dict(kind="mutant", name=nn, gb=e["gb"], timeout=e["timeout"])
                               for nn, e in sorted(M.items()) if e["large"]],
    }
    return dict(commit=commit, fp=fp, H=H, M=M, mats=mats, runs=runs, order=order, budget=budget)


def comments(p):
    """The generated comment paragraphs of each matrix (between `matrix:` and `include:`)."""
    H, M, mats = p["H"], p["M"], p["mats"]
    small = [h for h in p["order"] if not H[h]["large"]]
    large = [h for h in p["order"] if H[h]["large"]]
    unmeasured_h = [h for h in large if not H[h]["measured"]]
    unmeasured_m = [nn for nn, e in sorted(M.items()) if e["large"] and not e["measured"]]
    forced = [nn for nn, e in sorted(M.items()) if e["ci_large"]]
    head = ("Written by tools/scripts/kani-matrices.py from the run-local.py --all measurement (Kani "
            "fingerprint %s...; runs started %s): do not edit by hand, re-run it."
            % (p["fp"][:8], ", ".join(r["started"] for r in p["runs"])))
    return {
        "kani": [
            head,
            "The %d harnesses whose peak, rounded up, is 13 GB or less (in memory on ubuntu-latest), in %d "
            "groups, run one at a time. gb: the group's largest peak, rounded up. timeout: KANI_TIMEOUT per "
            "harness, 3 times the group's slowest hosted time (CI run %s's, else %g times the local time), at "
            "least 1800 s; the group's harnesses times it fit %d of the job's %d minutes. Peaks and local "
            "times: the local measurement (%d of them), else W7's, else v1.0.0's kani/RESULTS.md (peak times "
            "1.25). These groups and kani-large's, or the local record's harnesses, are every harness of "
            "kani/run.sh once (tools/scripts/check-kani-record.py, the covers job)."
            % (len(small), len(mats["kani"]), CI_RUN, HOSTED_X, p["budget"]["kani"] // 60,
               p["budget"]["kani"] // 60 + SETUP_MIN, sum(H[h]["measured"] for h in small)),
        ],
        "kani-large": [
            head,
            "%d harnesses in %d groups: those whose peak, rounded up, is above 13 GB; %s, whatever its local "
            "peak, at gb %d or more (in CI it filled ubuntu-latest's 15 GB of memory and about 15 GB of "
            "swap); and %s, in a group of its own (the record's self-test samples it). gb: the group's largest peak, rounded up. timeout: the largest of 1800 s, "
            "3 times the slowest local time and the limit the measurement ran each under (the record's "
            "limit_s: a timeout below it fails kani-record; raising one is fine). These are the local "
            "record's harnesses (check-kani-record.py), and kani/local/run-local.py --record runs them, with "
            "these timeouts, from this list. Measured locally: %d of the %d; not yet (sized from estimates; "
            "the record needs a run of each): %s."
            % (len(large), len(mats["kani-large"]), " and ".join(sorted(CI_LARGE)), CI_LARGE_GB, ALWAYS_LARGE,
               len(large) - len(unmeasured_h), len(large), " ".join(unmeasured_h) or "none"),
        ],
        "kani-mutants": [
            head,
            "The %d mutants whose Kani leg (the first killed-by; every checked-by of a must-survive patch) "
            "peaked at 13 GB or less, rounded up, and runs no harness kept in kani-large for ubuntu-latest's "
            "memory (%s). gb: the leg's largest peak (or the mutant's own, if larger), rounded up; timeout: "
            "the leg harnesses' KANI_TIMEOUT as in the kani matrix, at least 1800 s. These, and "
            "kani-mutants-large's, are every mutants/*.patch once (check-kani-record.py)."
            % (len(mats["kani-mutants"]), " ".join(sorted(CI_LARGE))),
        ],
        "kani-mutants-large": [
            head,
            "The %d mutants whose Kani leg peaked above 13 GB, or runs %s%s. gb: the leg harnesses' largest gb "
            "as in kani-large (or the mutant's own peak, if larger, rounded up); timeout: the leg harnesses' "
            "as in kani-large, and at least the limit the mutant's local run used (a "
            "timeout below the record's limit_s fails kani-record). These are the local record's mutants "
            "and those it declares not run in kani/local/not-run.tsv, for each of which kani-record warns "
            "and the mutants job still checks the plain-mode kill (check-kani-record.py); "
            "kani/local/run-local.py --record runs this list. Measured locally: %d of the %d; not: %s."
            % (len(mats["kani-mutants-large"]), " or ".join(sorted(CI_LARGE)),
               " (%s)" % " ".join(forced) if forced else "",
               len(mats["kani-mutants-large"]) - len(unmeasured_m), len(mats["kani-mutants-large"]),
               " ".join(unmeasured_m) or "none"),
        ],
    }


def wrap(text, indent, width=78):
    out, line = [], indent + "#"
    for w in text.split():
        if len(line) + 1 + len(w) > width and line != indent + "#":
            out.append(line)
            line = indent + "#"
        line += " " + w
    out.append(line)
    return out


def entry_line(e, indent):
    return '%s- { %s: "%s", gb: %d, timeout: %d }' % (indent, e["kind"], e["name"], e["gb"], e["timeout"])


def rewrite(text, p):
    lines = text.split("\n")
    com = comments(p)
    for job in JOBS:
        start, end = job_span(lines, job)
        mi = next((i for i in range(start, end) if re.match(r"^\s+matrix:\s*$", lines[i])), None)
        ii = next((i for i in range(mi or start, end) if re.match(r"^\s+include:\s*$", lines[i])), None)
        if mi is None or ii is None:
            raise Refused("%s: the %s job has no matrix: ... include:" % (WORKFLOW, job))
        between = lines[mi + 1:ii]
        if any(l.strip() and not l.strip().startswith("#") for l in between):
            raise Refused("%s: the %s job's matrix has keys other than include: (between matrix: and include:)"
                          % (WORKFLOW, job))
        k = ii + 1
        while k < end and re.match(r"^\s+- \{ *(group|mutant):", lines[k]):
            k += 1
        if k < end and re.match(r"^\s+- ", lines[k]):
            raise Refused("%s: the %s job's include: has an entry kani-matrices.py does not write: %r"
                          % (WORKFLOW, job, lines[k].strip()))
        indent = re.match(r"^(\s*)", lines[ii]).group(1)
        new = []
        for para in com[job]:
            if new:
                new.append(indent + "#")
            new += wrap(para, indent)
        new.append(lines[ii])
        new += [entry_line(e, indent + "  ") for e in p["mats"][job]]
        lines[mi + 1:k] = new
    return "\n".join(lines)


def verify(path, p):
    """The written matrices against run.sh list, mutants/*.patch, the limits and the job budgets: [errors]."""
    wf = ck.matrices(path)
    with open(path) as f:
        budget = job_budgets(f.read())
    H, M = p["H"], p["M"]
    errs = []
    for job in JOBS:
        if not wf.get(job):
            errs.append("%s: no matrix entries" % job)
    want = sorted(H)
    got = [h for job in ("kani", "kani-large") for e in wf.get(job, []) for h in e["name"].split()]
    if sorted(got) != want:
        errs.append("harnesses: not every harness of run.sh list once (missing %s; extra or twice %s)"
                    % (" ".join(sorted(set(want) - set(got))) or "-",
                       " ".join(sorted(h for h in set(got) if got.count(h) > 1 or h not in want)) or "-"))
    mu = [e["name"] for job in ("kani-mutants", "kani-mutants-large") for e in wf.get(job, [])]
    if sorted(mu) != sorted(M):
        errs.append("mutants: not every mutants/*.patch once (missing %s; extra or twice %s)"
                    % (" ".join(sorted(set(M) - set(mu))) or "-",
                       " ".join(sorted(n for n in set(mu) if mu.count(n) > 1 or n not in M)) or "-"))
    for job in ("kani", "kani-mutants"):
        errs += ["%s entry %s: %d GB, above %d" % (job, e["name"], e["gb"], IN_MEMORY_GB)
                 for e in wf.get(job, []) if e["gb"] > IN_MEMORY_GB]
    large_h = {h for e in wf.get("kani-large", []) for h in e["name"].split()}
    large_m = {e["name"] for e in wf.get("kani-mutants-large", [])}
    for h in sorted(set(CI_LARGE) | {ALWAYS_LARGE}):
        if h in H and h not in large_h:
            errs.append("%s is not in kani-large" % h)
    for nn, e in sorted(M.items()):
        if e["ci_large"] and nn not in large_m:
            errs.append("mutant %s (its leg runs %s) is not in kani-mutants-large" % (nn, " ".join(e["ci_large"])))
    if [e["name"] for e in wf.get("kani-large", []) if ALWAYS_LARGE in e["name"].split()] != [ALWAYS_LARGE]:
        errs.append("%s is not alone in a kani-large group" % ALWAYS_LARGE)
    # A large timeout below the limit its job ran (or will run) under fails kani-record.
    for e in wf.get("kani-large", []):
        for h in e["name"].split():
            if h in H and e["timeout"] < H[h]["limit"]:
                errs.append("kani-large %s: timeout %d below %s's limit %d s (%s)"
                            % (e["name"], e["timeout"], h, H[h]["limit"],
                               "the run that passed it" if H[h]["measured"] else "planned"))
    for e in wf.get("kani-mutants-large", []):
        if e["name"] in M and e["timeout"] < M[e["name"]]["limit"]:
            m = M[e["name"]]
            errs.append("kani-mutants-large %s: timeout %d below its limit %d s (%s)"
                        % (e["name"], e["timeout"], m["limit"],
                           "the run that passed it" if m["measured"] else "planned"))
    # Every harness an entry runs may use all of its timeout within the job's.
    for job in JOBS:
        for e in wf.get(job, []):
            n = len(e["name"].split()) if e["kind"] == "group" else max(1, len(M.get(e["name"], {}).get("leg", [])))
            if n * e["timeout"] > budget[job]:
                errs.append("%s entry %s: %d harnesses x %d s is over the job's %d s (timeout-minutes less %d)"
                            % (job, e["name"], n, e["timeout"], budget[job], SETUP_MIN))
    return errs


def not_run_check(p):
    """(warnings) of kani/local/not-run.tsv (the record owner's) against these matrices and runs."""
    if not os.path.exists(NOT_RUN_TSV):
        return [], []
    with open(NOT_RUN_TSV) as f:
        rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    decl = [r[0] for r in rows[1:] if r]
    by_patch = {e["patch"]: nn for nn, e in p["M"].items()}
    warns = []
    for d in decl:
        nn = by_patch.get(d)
        if nn is None:
            warns.append("not-run.tsv declares %s, which is not a mutants/*.patch" % d)
        elif not p["M"][nn]["large"]:
            warns.append("not-run.tsv declares %s, which is in kani-mutants, not kani-mutants-large: "
                         "check-kani-record.py fails (take it out of not-run.tsv)" % d)
        elif p["M"][nn]["measured"]:
            warns.append("not-run.tsv declares %s, which a run passed: record.py refuses (take it out)" % d)
    missing = [e["patch"] for nn, e in sorted(p["M"].items()) if e["large"] and not e["measured"]
               and e["patch"] not in decl]
    return decl, warns + (["kani-mutants-large mutants in no run and not declared in not-run.tsv (the record "
                           "needs one or the other): %s" % " ".join(missing)] if missing else [])


def report(p):
    H, M = p["H"], p["M"]
    print("commit %s, Kani fingerprint %s" % (p["commit"][:12], p["fp"][:12]))
    for r in p["runs"]:
        print("run %s (started %s): %d rows, %d passed%s%s"
              % (r["dir"], r["started"], r["rows"], len(r["passed"]),
                 "" if r["done"] else ", not finished (no ALL DONE)",
                 "; not used (failed): " + ", ".join(r["failed"]) if r["failed"] else ""))
    print("\nharnesses (matrix, gb, timeout; peak GB, local s, hosted s, limit s; source):")
    for h in p["order"]:
        e = H[h]
        print("  %-28s %-10s %3d %5d  %5.1f %5d %5d %5d  %s"
              % (h, "kani-large" if e["large"] else "kani", e["gb"], e["t_large"] if e["large"] else e["t_hosted"],
                 e["peak"], e["secs"], e["hosted_secs"], e["limit"], e["source"]))
    print("\nmutants (matrix, gb, timeout, limit s; source):")
    for nn, e in sorted(M.items()):
        print("  %-38s %-18s %3d %5d %5d  %s" % (e["patch"], "kani-mutants-large" if e["large"] else "kani-mutants",
                                                 e["gb"], e["timeout"], e["limit"], e["source"]))
    for job, es in p["mats"].items():
        print("\n%s (%d entries, %d jobs):" % (job, len(es), sum(len(e["name"].split()) for e in es)))
        for e in es:
            print(entry_line(e, "  "))
    lh = [h for h in p["order"] if H[h]["large"] and not H[h]["measured"]]
    lm = [nn for nn, e in sorted(M.items()) if e["large"] and not e["measured"]]
    print("\nnot measured locally, needed by the record: %d kani-large harnesses: %s" % (len(lh), " ".join(lh) or "-"))
    print("not measured locally: %d kani-mutants-large mutants (run them, or declare them in kani/local/not-run.tsv):"
          " %s" % (len(lm), " ".join("mutant-" + nn for nn in lm) or "-"))
    decl, warns = not_run_check(p)
    if decl:
        print("kani/local/not-run.tsv declares %d: %s" % (len(decl), " ".join(d.split("-", 1)[0] for d in decl)))
    for w in warns:
        print("warning: " + w)


def main(argv):
    args, write, rev = [], False, "HEAD"
    it = iter(argv[1:])
    for a in it:
        if a == "--write":
            write = True
        elif a == "--rev":
            rev = next(it, None)
            if rev is None:
                break
        elif a.startswith("-"):
            rev = None
            break
        else:
            args.append(a)
    if rev is None or not args:
        print(__doc__.split("\n\n")[0] + "\n\n" + __doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    try:
        with open(WORKFLOW) as f:
            text = f.read()
        p = plan(rev, args, text)
        report(p)
        new = rewrite(text, p)
        tmp = WORKFLOW + ".kani-matrices.tmp"
        with open(tmp, "w") as f:
            f.write(new)
        try:
            errs = verify(tmp, p)
            if errs:
                for e in errs:
                    print("error: " + e, file=sys.stderr)
                return 1
            if write:
                if new != text:
                    os.replace(tmp, WORKFLOW)
                print("\n%s: %s" % (os.path.relpath(WORKFLOW, ROOT), "written" if new != text else "unchanged"))
            else:
                print("\n%s: %s (without --write: not written)" % (os.path.relpath(WORKFLOW, ROOT),
                                                                   "would change" if new != text else "unchanged"))
        finally:
            if os.path.exists(tmp):
                os.remove(tmp)
    except (Refused, ck.Fail, OSError) as e:
        print("kani-matrices.py: %s" % e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
