#!/usr/bin/env python3
"""Run shared-pool's large Kani harnesses and mutants on this machine (and record them).

The jobs are the kani-large groups' harnesses and the kani-mutants-large
mutants of .github/workflows/shared-pool.yml (measured peaks above 13 GB,
which ubuntu-latest cannot hold in memory), each with its matrix timeout as
KANI_TIMEOUT. Each job is kani/run.sh (`kani <harness>` or
`mutants --kani 'NN-*'`) from an exact copy of the commit's Kani inputs
(git archive of projects/shared-pool/kani, conformance/traces/core.txt and
the repository's .cargo/config[.toml] above kani/ if it has any, and of
the source-mirror submodule at the commit's gitlink), one copy per lane,
with its own KANI_LOG_DIR and KEEP_LOGS and KANI_DEFER_COVERS=1. At
most --lanes jobs run at once, and only while their measured peaks
(kani/RESULTS.md) sum to at most --budget-gb; the longest start first.
Memory is sampled every 3 s: the run.sh process tree and its largest cbmc
process. run.sh is used as it is; KANI_MAX_GB stays its default (40) unless
set in the environment. The mutant jobs run with RUSTUP_TOOLCHAIN set to
tools/versions.env's RUST_TOOLCHAIN (their plain mode), as CI's
kani-mutants jobs do; the harness jobs without it, as CI's kani jobs.

The fingerprint does not see the environment, so the run refuses to start
when a variable that changes what run.sh runs or how cargo builds is set
(KANI, KANI_FLAGS, KANI_MOD, RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS,
RUSTUP_TOOLCHAIN, RUSTC, RUSTC_WRAPPER, RUSTC_WORKSPACE_WRAPPER,
CARGO_BUILD_*, CARGO_PROFILE_*, CARGO_TARGET_*_RUSTFLAGS), or when a cargo
configuration file outside the lanes (CARGO_HOME's, or a .cargo/ above the
run directory or TMPDIR) sets rustflags, rustc or a wrapper (at any depth,
such as target.<triple>.rustflags), a profile, or an include. The files
are parsed as TOML, as cargo reads them, so every spelling of those keys
(quoted, dotted, an inline table) counts, and one that does not parse is
refused too. That includes the work tree's own (such as
kani/.cargo/config.toml) when the run directory is inside the repository,
the default: cargo would read it on top of the lane's copy of the
commit's. Then pass --out outside the repository. env.txt records the
other KANI_* variables and those files.

usage: run-local.py [--out DIR] [--rev REV] [--lanes N] [--budget-gb GB]
                    [--record] [--dry-run]
                    [--all [--only JOB,...] [--estimates-from RESULTS_TSV]]
  --out DIR      output (default kani/target/local-run/run-<UTC time>)
  --rev REV      the commit to run (default HEAD)
  --record       when every job passed, run record.py DIR (writes kani/local;
                 then commit it)
  --dry-run      check the environment and cargo configuration as a run
                 does, print the jobs and exit
  --all          a measurement of every harness and mutant, not a record
                 (below)
  --only JOBS    with --all: only these jobs (comma-separated harness names
                 and mutant-NN)
  --estimates-from RESULTS_TSV
                 with --all: an earlier --all run's results.tsv; each
                 harness it passed is estimated at its measured time and
                 tree peak (and runs first), its limit raised to 3 times
                 that time if higher, and the mutants re-estimated from
                 them (a mutant's limit at least its harnesses')

Output, as record.py reads it: env.txt (commit, kani-tree, core-pin, tool
versions, host, lanes, budget, start and end), jobs.json (each job's
timeout, its KANI_TIMEOUT), progress.log (a start line per job with its
limit; ends with ALL DONE when complete), results.tsv (job kind lane
start_utc secs rc verdict peak_tree_gb peak_cbmc_gb est_peak_gb est_secs;
a harness's verdict is run.sh's result, a mutant's expect/plain/kani with
"!" if run.sh flagged it UNEXPECTED), logs/<job>/ (stdout.txt, covers.tsv,
the Kani and plain logs). About 4 hours for v1.0.0's 36 large harnesses
and 16 large mutants on a 10-core, 64 GB Mac with the defaults. Needs
Python 3.11 or later (tomllib, for the cargo configuration), Kani and
its CBMC at tools/versions.env's versions (cargo install --locked
kani-verifier --version <KANI>; cargo kani setup), the Rust toolchain
RUST_TOOLCHAIN (rustup toolchain install <RUST_TOOLCHAIN> --profile
minimal) and the source-mirror submodule (git submodule update --init
projects/shared-pool/source-mirror).

A measurement (--all) is not a record: it sizes the matrices (their 13 GB
split and timeouts) before a record is made. Its jobs are every harness of
the commit's `run.sh list` and every one of its mutants/*.patch, not the
matrices'. Each job's peak and time are estimated from the best source,
named in jobs.json and estimates.tsv: W7's measurements of the v1.1.0
Kani layer (V11_MEASURED); then, for a harness whose name is unchanged,
v1.0.0's kani/RESULTS.md, its peak times 1.25 (v1.1.0 measured up to 1.53
times v1.0.0's, h_init); a mutant from the harnesses its Kani leg is
expected to run (its first killed-by, or every checked-by of a
`# must-survive` patch) plus 240 s of plain mode and build; otherwise
unknown: 40 GB (KANI_MAX_GB, the most run.sh lets a job use) and 1800 s.
A job estimated at 35 GB or more (the unknowns, and v1.0.0 numbers scaled
near 40 GB) runs alone: it starts only when no job runs, and no job
starts beside it; so does a running job whose sampled process tree has
reached 35 GB. Other jobs share the budget, a running one counted at its
estimate or its sampled tree peak, whichever is more. A job's limit
(KANI_TIMEOUT) is 3 times its estimate, at least 1800 s, and at least
7200 s for run.sh's 7200 s tier and for an unknown; a mutant's is the
largest of its harnesses'. KANI_MAX_GB stays 40 (another value is
refused). The run holds /tmp/sp-heavy.lock (SP_HEAVY_LOCK) from its start
to its exit, failures and signals included, so that no other heavy job
overlaps it: while another process holds it, the run waits (checking
every 30 s), and it removes only a lock it took; before it releases the
lock, it kills every job it started (process group and process tree).
--record is refused; the output is outside the repository (default
~/shared-pool-kani-measure/all-<commit>-<UTC time>) and env.txt says `mode
measurement` and the commit's fingerprint. record.py can assemble the
record from one or more such runs at one commit, finished or stopped
early: the large matrices' jobs, each from the latest run that finished
and passed it, with the limit it ran under (which the matrix timeout may
not be below), and
kani-mutants-large mutants no run finished declared in kani/local/
not-run.tsv. The lanes, the environment and cargo checks, the sampling
and results.tsv are the record's. --dry-run prints the jobs, their
estimates and the estimated hours.
"""
import argparse
import atexit
import datetime
import fnmatch
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

try:
    import tomllib
except ImportError:  # Python < 3.11: build_keys refuses (fails closed)
    tomllib = None

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
SP = "projects/shared-pool"
CHECK = os.path.join(ROOT, "tools/scripts/check-kani-record.py")
RECORD = os.path.join(HERE, "record.py")
# Variables that change what run.sh runs (KANI, KANI_FLAGS, KANI_MOD) or how
# cargo and rustc build (a verdict), which the fingerprint does not see.
REFUSED_ENV = re.compile(r"^(KANI|KANI_FLAGS|KANI_MOD|RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTUP_TOOLCHAIN|RUSTC"
                         r"|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|CARGO_BUILD_\w+|CARGO_PROFILE_\w+"
                         r"|CARGO_TARGET_\w+_RUSTFLAGS)$")
# Set per job by this script.
OWN_ENV = {"KANI_LOG_DIR", "KANI_DEFER_COVERS", "KANI_TIMEOUT", "KANI_CBMC", "KEEP_LOGS"}
# Cargo configuration keys that change how cargo builds: these at any depth
# (build.rustflags, target.<triple>.rustflags, ...), and at the top level
# profile (any profile's settings) and include (another file, which the
# check would not read).
BUILD_KEYS = {"rustflags", "rustc", "rustc-wrapper", "rustc-workspace-wrapper"}
TOP_BUILD_KEYS = {"profile", "include"}

# --all, the measurement. The machine's lock for heavy jobs (Kani, CBMC):
# taken with mkdir, released with rmdir, by whoever took it.
LOCK = os.environ.get("SP_HEAVY_LOCK") or "/tmp/sp-heavy.lock"
# Its estimates, best source first. W7's measurements of the v1.1.0 Kani
# layer (.context/shared-pool/v1.1-verify/w7-notes.md, 2026-11-14): job
# seconds (run.sh's time plus its codegen) and peak GB (the largest 3 s
# sample of the process tree; of cbmc during the harness for run.sh ref).
# The probes ran on 69cca54's Kani tree, run.sh ref and mutant 31 on
# db957d4's (31's time at 6b120fd); the harness code has not changed since.
V11_MEASURED = {
    "h_full_256_prune": (505, 20.5, "W7 probe P1"),
    "h_step_submit_inv": (1111, 29.7, "W7 probe P2"),
    "h_init": (512, 33.0, "W7 probe P3"),
    "h_wedge": (624, 32.6, "W7 probe P4"),
    "h_step_observe_tx": (166, 11.7, "W7 probe P5"),
    "h_validate": (794, 32.9, "W7 probe h_validate"),
    # run.sh ref: run.sh's time plus 50 s, the one codegen of its 1,222 s job.
    "h_ref_submit": (557, 14.7, "W7 run.sh ref"),
    "h_ref_dispatch": (117, 7.5, "W7 run.sh ref"),
    "h_ref_ack": (107, 6.0, "W7 run.sh ref"),
    "h_ref_cancel": (179, 11.1, "W7 run.sh ref"),
    "h_ref_settle": (323, 14.7, "W7 run.sh ref"),
    "h_ref_pause": (109, 6.0, "W7 run.sh ref"),
    "h_ref_observe": (130, 8.6, "W7 run.sh ref"),
    "mutant-31": (161, 9.8, "W7/W10 run.sh mutants --kani '31-*'"),
}
# Then v1.0.0's measurement: kani/RESULTS.md as of the v1.0.0 record
# (unchanged since), its Kani results table (time, peak). v1.1.0 adds the
# release term and completed_units: its measured peaks are 0.84 to 1.53
# times v1.0.0's (h_wedge, h_init), so the peak is scaled.
V10_RESULTS = ("a072a2dec788afcf0741786cdb458c3906e350fa", SP + "/kani/RESULTS.md")
V10_PEAK_SCALE = 1.25
# Otherwise unknown: KANI_MAX_GB, the most run.sh lets a job use, and
# 1800 s, run.sh's default limit.
MAX_GB = 40.0
UNKNOWN_SECS = 1800
# A job counted at this or more runs alone: it starts only when no job
# runs, and no job starts beside it (a running job counts its estimate or
# its sampled tree peak, whichever is more). These are the unknowns and
# v1.0.0 numbers scaled near MAX_GB (h_energy_max, h_bmc_from_init_s_submit);
# v1.1.0's largest measured peak, h_init's 33.0 GB (W7), is below it. By
# the budget alone, a 40 GB job could run beside one estimated at 10 GB,
# and v1.1.0's small step harnesses peaked at 1.3 to 1.8 times their
# v1.0.0 numbers (W11's smoke runs): 40 + 18 GB on a 64 GB machine.
ALONE_GB = 35.0
# A mutant job: plain mode (cargo test --release from scratch) and the
# mutant tree's Kani codegen, before its Kani leg (v1.0.0's local record:
# mutant 21's job 338 s around h_full_256's 46 s).
MUTANT_OVERHEAD = 240
# Limits: 3 times the estimate, at least LIMIT_MIN; at least LIMIT_LONG for
# run.sh's long tier and an unknown.
LIMIT_MIN, LIMIT_LONG = 1800, 7200


def sh(*args, **kw):
    r = subprocess.run(list(args), capture_output=True, text=True, **kw)
    if r.returncode != 0:
        sys.exit("run-local.py: %s: exit %d: %s" % (" ".join(args), r.returncode, (r.stderr or r.stdout).strip()))
    return r.stdout


def build_keys(text):
    """The key paths (such as build.rustflags) in a cargo configuration file's text that change how cargo builds.

    The text is parsed as TOML, as cargo parses it, so a quoted, dotted or
    inline-table spelling of a key is the same path. Raises ValueError
    (tomllib.TOMLDecodeError) when the text is not TOML.
    """
    if tomllib is None:
        sys.exit("run-local.py needs Python 3.11+ (tomllib) to check cargo configuration (%s is %s)"
                 % (sys.executable, platform.python_version()))
    found = []

    def name(path):
        return ".".join(k if re.fullmatch(r"[A-Za-z0-9_-]+", k) else json.dumps(k) for k in path)

    def walk(v, path):
        if isinstance(v, dict):
            for k, x in v.items():
                if k in BUILD_KEYS or (not path and k in TOP_BUILD_KEYS):
                    found.append(name(path + [k]))
                else:
                    walk(x, path + [k])
        elif isinstance(v, list):
            for x in v:
                walk(x, path)
    walk(tomllib.loads(text), [])
    return found


def now():
    return datetime.datetime.now(datetime.timezone.utc)


def iso(t):
    return t.strftime("%Y-%m-%dT%H:%M:%SZ")


def host():
    if sys.platform == "darwin":
        cpu = sh("sysctl", "-n", "machdep.cpu.brand_string").strip()
        n = sh("sysctl", "-n", "hw.ncpu").strip()
        gb = int(sh("sysctl", "-n", "hw.memsize")) // 2 ** 30
        return "%s %s cores %d GB, macOS %s" % (cpu, n, gb, platform.mac_ver()[0])
    cpu = "?"
    try:
        with open("/proc/cpuinfo") as f:
            cpu = next((l.split(":", 1)[1].strip() for l in f if l.startswith("model name")), "?")
        with open("/proc/meminfo") as f:
            gb = int(next(l.split()[1] for l in f if l.startswith("MemTotal:"))) // 2 ** 20
    except OSError:
        gb = 0
    return "%s %d cores %d GB, %s %s" % (cpu, os.cpu_count() or 0, gb, platform.system(), platform.release())


def jobs_from_workflow():
    """kani-large's harnesses and kani-mutants-large's mutants, with measured estimates."""
    meas = {}
    try:
        with open(os.path.join(ROOT, SP, "kani/RESULTS.md")) as f:
            for m in re.finditer(r"^\| `(h_\w+)` \| (\S+) \| (\d+) \| ([\d.]+) \|", f.read(), re.M):
                meas.setdefault(m.group(1), (int(m.group(3)), float(m.group(4))))
    except OSError:
        pass
    jobs = []
    for line in sh(sys.executable, "-I", CHECK, "matrix", "kani-large").splitlines():
        group, gb, to = line.split("\t")
        for h in group.split():
            secs, peak = meas.get(h, (int(to) // 3, float(gb)))
            jobs.append(dict(name=h, kind="harness", peak=peak, est=secs, timeout=int(to), args=["kani", h]))
    for line in sh(sys.executable, "-I", CHECK, "matrix", "kani-mutants-large").splitlines():
        mu, gb, to = line.split("\t")
        jobs.append(dict(name="mutant-" + mu, kind="mutant", peak=float(gb), est=int(to) / 3, timeout=int(to),
                         args=["mutants", "--kani", mu + "-*"]))
    # Longest first; the scheduler takes the first job that fits the budget.
    jobs.sort(key=lambda j: -j["est"])
    return jobs


def limit(secs, at_least):
    """3 times secs, at least at_least, in whole minutes."""
    return max(at_least, -(-int(3 * secs) // 60) * 60)


def jobs_all(commit):
    """Every harness of commit's run.sh list and every mutant of its mutants/, with estimates and limits."""
    kani = SP + "/kani"
    tmp = tempfile.mkdtemp(prefix="run-local-list.")
    try:
        os.makedirs(os.path.join(tmp, "src"))
        for p in ("run.sh", "src/proofs.rs"):
            with open(os.path.join(tmp, p), "w") as f:
                f.write(sh("git", "-C", ROOT, "show", "%s:%s/%s" % (commit, kani, p)))
        harnesses = sh("bash", os.path.join(tmp, "run.sh"), "list").split()
        with open(os.path.join(tmp, "run.sh")) as f:
            runsh = f.read()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    if not harnesses:
        sys.exit("run-local.py: %s's run.sh list is empty" % commit[:12])
    # run.sh's long tier: harness_timeout's `<globs>) echo "${KANI_TIMEOUT:-7200}"`.
    m = re.search(r"^\s*([^\n()]+)\)\s*echo \"\$\{KANI_TIMEOUT:-%d\}\"" % LIMIT_LONG, runsh, re.M)
    if not m:
        sys.exit("run-local.py: no %d s tier in %s's run.sh harness_timeout" % (LIMIT_LONG, commit[:12]))
    long_tier = [g.strip() for g in m.group(1).split("|")]

    rev, path = V10_RESULTS
    if subprocess.run(["git", "-C", ROOT, "cat-file", "-e", rev + "^{commit}"], capture_output=True).returncode:
        rev, v10_name = commit, "RESULTS.md at %s (v1.0.0's %s is not in this clone)" % (commit[:12], rev[:7])
    else:
        v10_name = "v1.0.0 RESULTS.md"
    v10 = {}
    for m in re.finditer(r"^\| `(h_\w+)` \| (\S+) \| (\d+) \| ([\d.]+) \|", sh("git", "-C", ROOT, "show",
                                                                               rev + ":" + path), re.M):
        v10.setdefault(m.group(1), (int(m.group(3)), float(m.group(4))))

    est = {}
    for h in harnesses:
        tier = any(fnmatch.fnmatchcase(h, g) for g in long_tier)
        if h in V11_MEASURED:
            secs, peak, src = V11_MEASURED[h]
            basis, src = "W7", "%s: %d s, %.1f GB" % (src, secs, peak)
        elif h in v10:
            secs, gb = v10[h]
            peak = min(MAX_GB, round(gb * V10_PEAK_SCALE, 1))
            basis, src = "v1.0.0", "%s: %d s, %.1f GB; peak x%g" % (v10_name, secs, gb, V10_PEAK_SCALE)
        else:
            secs, peak, basis, src = UNKNOWN_SECS, MAX_GB, "unknown", "unknown"
        lim = LIMIT_LONG if basis == "unknown" else limit(secs, LIMIT_LONG if tier else LIMIT_MIN)
        est[h] = dict(name=h, kind="harness", peak=peak, est=secs, timeout=lim, args=["kani", h], basis=basis,
                      source=src)
    hest = {h: (j["est"], j["peak"], j["basis"]) for h, j in est.items()}

    names = sorted(l for l in sh("git", "-C", ROOT, "ls-tree", "--name-only", "%s:%s/mutants" % (commit, kani))
                   .splitlines() if l.endswith(".patch"))
    jobs = [est[h] for h in harnesses]
    for p in names:
        nn = p.split("-", 1)[0]
        same = [q for q in names if q.startswith(nn + "-")]
        if len(same) != 1:
            # run.sh mutants --kani 'NN-*' would run them all in one job.
            sys.exit("run-local.py: mutants %s share the prefix %s-" % (" ".join(same), nn))
        text = sh("git", "-C", ROOT, "show", "%s:%s/mutants/%s" % (commit, kani, p))
        survive = re.search(r"^# must-survive$", text, re.M) is not None
        hs = []
        for m in re.finditer(r"^# (?:killed-by|checked-by): (.*)$", text, re.M):
            hs += m.group(1).split()
        name = "mutant-" + nn
        for h in hs:
            if h not in est:
                print("run-local.py: warning: mutants/%s names %s, which is not in run.sh list" % (p, h),
                      file=sys.stderr)
        # The harnesses its Kani leg is expected to run: run.sh stops at
        # the first that kills it; a must-survive patch runs them all.
        leg = hs if survive else hs[:1]
        lim = max([est[h]["timeout"] if h in est else LIMIT_LONG for h in hs] or [LIMIT_LONG])
        if name in V11_MEASURED:
            secs, peak, src = V11_MEASURED[name]
            basis, src = "W7", "%s: %d s, %.1f GB" % (src, secs, peak)
        else:
            secs, peak, basis, src = mutant_estimate(leg, survive, hest)
        jobs.append(dict(name=name, kind="mutant", peak=peak, est=secs, timeout=lim,
                         args=["mutants", "--kani", nn + "-*"], basis=basis, source=src, patch=p[:-len(".patch")],
                         leg=leg, survive=survive))
    jobs.sort(key=all_order)
    return jobs, hest


def all_order(j):
    """--all's order: the harnesses, then the mutants (estimated from them by then), each longest first."""
    return (j["kind"] != "harness", -j["est"])


def mutant_estimate(leg, survive, hest):
    """(secs, peak, basis, source) of a mutant whose Kani leg runs leg; hest: {harness: (secs, peak, basis)}."""
    what = "%s %s" % ("checked-by" if survive else "killed-by", " + ".join(leg) or "none")
    known = [hest[h] for h in leg if h in hest and hest[h][2] != "unknown"]
    if not leg or len(known) != len(leg):
        return (MUTANT_OVERHEAD + sum(hest[h][0] if h in hest else UNKNOWN_SECS for h in leg), MAX_GB, "unknown",
                "unknown (%s)" % what)
    return (MUTANT_OVERHEAD + sum(k[0] for k in known), max(k[1] for k in known), "harnesses",
            "%s [%s] + %d s plain mode and build" % (what, ", ".join(k[2] for k in known), MUTANT_OVERHEAD))


def preload(jobs, hest, path):
    """Take the estimates of the harnesses an earlier --all run passed from its results.tsv (path).

    Its secs and tree peak replace the job's estimate (the limit only rises), and
    the mutants are re-estimated from them, as the main loop does for a
    harness measured in this run. Returns how many harnesses it covered.
    """
    byname, n = {j["name"]: j for j in jobs}, 0
    with open(path) as f:
        for line in f:
            r = line.rstrip("\n").split("\t")
            if len(r) != 11 or r[1] != "harness" or r[0] not in byname or r[5] != "0":
                continue
            j = byname[r[0]]
            if not passed(j, r[6]):
                continue
            secs, peak = int(float(r[4])), float(r[7])
            j["est"], j["peak"], j["basis"] = secs, peak, "measured"
            # A measured time can be well above the estimate the limit came from.
            j["timeout"] = max(j["timeout"], limit(secs, LIMIT_MIN))
            j["source"] = "%s: %d s, %.1f GB" % (os.path.basename(os.path.dirname(os.path.abspath(path))), secs, peak)
            hest[j["name"]] = (secs, peak, "measured")
            n += 1
    for m in jobs:
        if m["kind"] == "mutant" and m["basis"] != "W7":
            m["est"], m["peak"], m["basis"], m["source"] = mutant_estimate(m["leg"], m["survive"], hest)
            m["timeout"] = max([m["timeout"]] + [byname[h]["timeout"] for h in m["leg"] if h in byname])
    jobs.sort(key=all_order)
    return n


def only_jobs(jobs, only):
    """jobs restricted to only's comma-separated names (a harness, mutant-NN, NN or a patch name), or an error."""
    want, bad = set(), []
    for w in [w.strip() for w in only.split(",") if w.strip()]:
        hit = [j["name"] for j in jobs if w in (j["name"], j.get("patch"), j["name"].replace("mutant-", "", 1))]
        if hit:
            want.update(hit)
        else:
            bad.append(w)
    if bad or not want:
        return None, bad
    return [j for j in jobs if j["name"] in want], []


def fits(job, counted, budget, alone):
    """Whether job may start beside the running jobs, counted at counted (GB each), within budget.

    Alone, a job always fits (h_wedge's peak may be most of the budget).
    alone: the size from which a job runs alone, the pending one or a
    running one (--all's ALONE_GB; None for a record, whose peaks are
    measured).
    """
    if not counted:
        return True
    if alone is not None and (job["peak"] >= alone or max(counted) >= alone):
        return False
    return sum(counted) + job["peak"] <= budget


def simulate(jobs, lanes, budget, alone):
    """The schedule's length in seconds with the estimates: the main loop's policy, without the sampling."""
    t, pending, running = 0.0, list(jobs), []
    while pending or running:
        for _ in range(lanes - len(running)):
            pick = next((j for j in pending if fits(j, [p for _, p in running], budget, alone)), None)
            if pick is None:
                break
            pending.remove(pick)
            running.append((t + pick["est"], pick["peak"]))
        t = min(e for e, _ in running)
        running = [(e, p) for e, p in running if e > t]
    return t


def take_lock():
    """Wait for and take LOCK (mkdir); release it at exit (rmdir), and only if taken. Seconds waited."""
    def on_signal(signum, _frame):
        print("run-local.py: stopped by signal %d" % signum, file=sys.stderr, flush=True)
        sys.exit(128 + signum)

    # Until the main loop's own handler: exit, which releases a taken lock.
    signal.signal(signal.SIGINT, on_signal)
    signal.signal(signal.SIGTERM, on_signal)
    t0, said = time.monotonic(), False
    while True:
        try:
            os.mkdir(LOCK)
            break
        except FileExistsError:
            if not said:
                print("run-local.py: waiting for %s (another heavy job holds it; checking every 30 s)" % LOCK,
                      file=sys.stderr, flush=True)
                said = True
            time.sleep(30)

    def release():
        try:
            os.rmdir(LOCK)
            print("run-local.py: released %s" % LOCK, file=sys.stderr, flush=True)
        except OSError as e:
            print("run-local.py: warning: could not release %s: %s" % (LOCK, e), file=sys.stderr, flush=True)

    atexit.register(release)
    waited = time.monotonic() - t0
    print("run-local.py: holding %s (waited %.0f s)" % (LOCK, waited), file=sys.stderr, flush=True)
    return waited


def cargo_configs(*dirs):
    """The cargo configuration files above dirs (their .cargo/config[.toml]) and in CARGO_HOME."""
    out = []
    for d in dirs:
        d = os.path.abspath(d)
        while True:
            out += [os.path.join(d, ".cargo", n) for n in ("config", "config.toml")]
            if os.path.dirname(d) == d:
                break
            d = os.path.dirname(d)
    home = os.environ.get("CARGO_HOME") or os.path.expanduser("~/.cargo")
    out += [os.path.join(home, n) for n in ("config", "config.toml")]
    seen = []
    for p in out:
        if os.path.isfile(p) and p not in seen:
            seen.append(p)
    return seen


def make_lane(lane, commit, core, cargo_above):
    """An exact copy of the commit's Kani inputs at lane/ (cargo_above: its .cargo/config[.toml] above kani/)."""
    os.makedirs(lane)
    a = subprocess.Popen(["git", "-C", ROOT, "archive", "--format=tar", commit, SP + "/kani",
                          SP + "/conformance/traces/core.txt"] + cargo_above, stdout=subprocess.PIPE)
    subprocess.run(["tar", "-x", "-C", lane], stdin=a.stdout, check=True)
    if a.wait() != 0:
        sys.exit("run-local.py: git archive %s failed" % commit)
    sm = os.path.join(lane, SP, "source-mirror")
    os.makedirs(sm)
    a = subprocess.Popen(["git", "-C", os.path.join(ROOT, SP, "source-mirror"), "archive", "--format=tar", core],
                         stdout=subprocess.PIPE)
    subprocess.run(["tar", "-x", "-C", sm], stdin=a.stdout, check=True)
    if a.wait() != 0:
        sys.exit("run-local.py: git archive of the source-mirror at %s failed" % core)


def processes():
    """Every process: ({ppid: [pid]}, {pid: rss KB}, {pid: command}); empty when ps fails."""
    try:
        out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,rss=,comm="], capture_output=True, text=True).stdout
    except OSError:
        return {}, {}, {}
    kids, rss, comm = {}, {}, {}
    for line in out.splitlines():
        p = line.split(None, 3)
        if len(p) < 4:
            continue
        pid, ppid, r = int(p[0]), int(p[1]), int(p[2])
        kids.setdefault(ppid, []).append(pid)
        rss[pid], comm[pid] = r, p[3]
    return kids, rss, comm


def tree(root, kids):
    """root and its descendants (kids: processes()'s)."""
    out, stack = [], [root]
    while stack:
        p = stack.pop()
        out.append(p)
        stack.extend(kids.get(p, []))
    return out


def tree_rss(root):
    """(tree KB, largest cbmc KB) for the process tree rooted at pid root."""
    kids, rss, comm = processes()
    tot, cb = 0, 0
    for p in tree(root, kids):
        tot += rss.get(p, 0)
        if os.path.basename(comm.get(p, "")).startswith("cbmc"):
            cb = max(cb, rss.get(p, 0))
    return tot, cb


def kill_job(proc):
    """SIGKILL a job: its process group (run.sh leads one) and its process tree, listed before anything is killed.

    The tree catches a descendant in a group of its own, which the group
    kill misses and which, once its parent dies, is no longer in the tree
    (run.sh's kill_tree does the same: it says cargo-kani starts CBMC in
    its own group; under Kani 0.68 CBMC shares cargo-kani's).
    """
    pids = tree(proc.pid, processes()[0])
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except OSError:
        pass
    for p in pids:
        try:
            os.kill(p, signal.SIGKILL)
        except OSError:
            pass


def verdict(job, so):
    if job["kind"] == "harness":
        m = re.search(r"^%s\s+(\S+)\s+(\d+)" % re.escape(job["name"]), so, re.M)
        return m.group(1) if m else "NO-RESULT"
    nn = job["name"].split("-", 1)[1]
    m = re.search(r"^(%s-\S*)\s+(\S+)\s+(\S+)\s+(\S+)(.*)$" % re.escape(nn), so, re.M)
    if not m:
        return "NO-RESULT"
    return "%s/%s/%s%s" % (m.group(2), m.group(3), m.group(4), "!" if "UNEXPECTED" in m.group(5) else "")


def passed(job, v):
    if job["kind"] == "harness":
        return v == ("ok-FAILED" if job["name"] == "h_canary" else "ok")
    return v in ("killed/killed/killed", "survive/survived/survived")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out")
    ap.add_argument("--rev", default="HEAD")
    ap.add_argument("--lanes", type=int, default=2)
    ap.add_argument("--budget-gb", type=float, default=50.0)
    ap.add_argument("--record", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--only")
    ap.add_argument("--estimates-from", metavar="RESULTS_TSV")
    a = ap.parse_args()
    if a.all and a.record:
        ap.error("--all is a measurement, not a record: --record is refused (record the large jobs with "
                 "run-local.py --record once the matrices are re-split from the measurement)")
    if a.estimates_from is not None and not a.all:
        ap.error("--estimates-from needs --all")
    if a.only is not None and not a.all:
        ap.error("--only needs --all (a record runs every job of kani-large and kani-mutants-large)")

    commit = sh("git", "-C", ROOT, "rev-parse", "--verify", a.rev + "^{commit}").strip()
    kani_tree = sh("git", "-C", ROOT, "rev-parse", commit + ":" + SP + "/kani").strip()
    core = sh("git", "-C", ROOT, "rev-parse", commit + ":" + SP + "/source-mirror").strip()
    versions = sh("git", "-C", ROOT, "show", commit + ":tools/versions.env")
    kani_v = re.search(r"^KANI=(\S+)", versions, re.M).group(1)
    cbmc_v = re.search(r"^CBMC=(\S+)", versions, re.M).group(1)
    rust_v = re.search(r"^RUST_TOOLCHAIN=(\S+)", versions, re.M).group(1)
    if a.all:
        jobs, hest = jobs_all(commit)
        preloaded = preload(jobs, hest, a.estimates_from) if a.estimates_from else 0
        if a.only is not None:
            jobs, bad = only_jobs(jobs, a.only)
            if jobs is None:
                ap.error("--only %s: %s" % (a.only, "not a job of --all: " + ", ".join(bad) if bad else "no job"))
    else:
        jobs = jobs_from_workflow()
    if not jobs:
        sys.exit("run-local.py: no kani-large or kani-mutants-large entries in the workflow")
    # The environment and cargo configuration: nothing the fingerprint does
    # not see may change a verdict. A dry run checks them too.
    refused = sorted(k for k, v in os.environ.items() if REFUSED_ENV.match(k) and v != "")
    if refused:
        sys.exit("run-local.py: %s set in the environment, which changes what run.sh runs or how cargo builds; "
                 "the record's fingerprint does not see the environment: unset it and start again"
                 % ", ".join(refused))
    if a.all and (os.environ.get("KANI_MAX_GB") or "40") != "40":
        sys.exit("run-local.py: KANI_MAX_GB=%s: a measurement keeps run.sh's default, 40, as CI and the record do: "
                 "unset it and start again" % os.environ["KANI_MAX_GB"])
    started = now()
    if a.all:
        out = os.path.abspath(a.out or os.path.join(os.path.expanduser("~"), "shared-pool-kani-measure", "all-%s-%s"
                                                    % (commit[:12], started.strftime("%Y%m%dT%H%MZ"))))
        real, root = os.path.realpath(out), os.path.realpath(ROOT)
        if real == root or real.startswith(root + os.sep):
            sys.exit("run-local.py: %s is in the repository; a measurement is not a record and writes outside it: "
                     "pass --out outside %s" % (out, ROOT))
    else:
        out = os.path.abspath(a.out or os.path.join(ROOT, SP, "kani/target/local-run",
                                                    "run-" + started.strftime("%Y%m%dT%H%MZ")))
    if os.path.exists(out):
        sys.exit("run-local.py: %s exists" % out)
    configs = cargo_configs(out, os.environ.get("TMPDIR") or "/tmp")
    bad, unparsable = [], []
    for p in configs:
        try:
            with open(p, "rb") as f:
                keys = build_keys(f.read().decode("utf-8"))
        except ValueError as e:  # not TOML (or not UTF-8): cargo would not read it either
            unparsable.append("%s (%s)" % (p, e))
            continue
        if keys:
            bad.append((p, keys))
    if unparsable:
        sys.exit("run-local.py: unparsable cargo configuration %s: cannot tell whether it sets rustflags, rustc or a "
                 "wrapper, a profile or an include: move it aside and start again" % "; ".join(unparsable))

    def named(ps):
        return ", ".join("%s (%s)" % (path, ", ".join(ks)) for path, ks in ps)
    in_repo = [(path, ks) for path, ks in bad if path.startswith(ROOT + os.sep)]
    if in_repo:
        # The lanes have the commit's copies (fingerprinted); cargo would
        # read the work tree's above them as well.
        sys.exit("run-local.py: %s set rustflags, rustc or a wrapper, a profile or an include, and are in the work "
                 "tree above the run directory %s, so cargo would read them in every lane; the lanes have %s's own "
                 "cargo configuration (the fingerprinted one) from git archive: pass --out outside the repository "
                 "and start again" % (named(in_repo), out, commit[:12]))
    if bad:
        sys.exit("run-local.py: %s set rustflags, rustc or a wrapper, a profile or an include, for the run's builds; "
                 "the record's fingerprint does not see them: move them aside and start again" % named(bad))
    if a.dry_run and a.all:
        nh = sum(j["kind"] == "harness" for j in jobs)
        print("commit %s; measurement (--all, not a record): %d jobs, %d harnesses of run.sh list and %d "
              "mutants (KANI_TIMEOUT, estimated peak GB and s, the estimate's source; in run order):"
              % (commit[:12], len(jobs), nh, len(jobs) - nh))
        for j in jobs:
            print("  %-28s %-8s %5d s  %5.1f GB  %5.0f s  %s" % (j["name"], j["kind"], j["timeout"], j["peak"],
                                                               j["est"], j["source"]))
        basis = [j["basis"] for j in jobs]
        total = sum(j["est"] for j in jobs)
        print("sources: %d W7 (v1.1.0 measured), %d v1.0.0 RESULTS.md, %d mutants from their harnesses, "
              "%d unknown (%.1f h of the estimate)"
              % (basis.count("W7"), basis.count("v1.0.0"), basis.count("harnesses"), basis.count("unknown"),
                 sum(j["est"] for j in jobs if j["basis"] == "unknown") / 3600))
        alone = [j for j in jobs if j["peak"] >= ALONE_GB]
        print("alone (estimated at %g GB or more; no job beside them): %d jobs, %.1f h of the estimate"
              % (ALONE_GB, len(alone), sum(j["est"] for j in alone) / 3600))
        print("estimate: %.1f h of jobs; / %d lanes = %.1f h; scheduled (%d lanes, budget %g GB, %g GB or more "
              "alone, in this order, with these estimates): %.1f h"
              % (total / 3600, a.lanes, total / 3600 / a.lanes, a.lanes, a.budget_gb, ALONE_GB,
                 simulate(jobs, a.lanes, a.budget_gb, ALONE_GB) / 3600))
        return 0
    if a.dry_run:
        print("commit %s; %d jobs (KANI_TIMEOUT, measured peak GB, estimated s):" % (commit[:12], len(jobs)))
        for j in jobs:
            print("  %-28s %-8s %5d s  %5.1f GB  %5.0f s  run.sh %s" % (j["name"], j["kind"], j["timeout"], j["peak"],
                                                                     j["est"], " ".join(j["args"])))
        return 0
    if sh("git", "-C", ROOT, "diff", "--name-only", commit, "--",
          *sh(sys.executable, "-I", CHECK, "pathspecs").splitlines()).strip():
        print("run-local.py: note: the work tree's Kani inputs differ from %s; the run uses %s's" % (commit[:12],
              commit[:12]), file=sys.stderr)

    cargo_above = sh(sys.executable, "-I", CHECK, "cargo-configs", commit).splitlines()
    kani_env = " ".join("%s=%s" % (k, os.environ[k]) for k in sorted(os.environ)
                        if k.startswith("KANI_") and k not in OWN_ENV) or "none"
    if a.all:
        # From here to the exit: no other heavy job on the machine.
        waited = take_lock()
        started = now()

    # Tools: the pinned Kani and its own CBMC (run.sh reads loop ids with it).
    kani_line = sh("cargo", "kani", "--version").splitlines()[0].strip()
    if not re.search(r"(^|\s)%s(\s|$)" % re.escape(kani_v), kani_line):
        sys.exit("run-local.py: %r is not Kani %s (tools/versions.env)" % (kani_line, kani_v))
    cbmc = os.environ.get("KANI_CBMC") or os.path.join(os.environ.get("KANI_HOME") or os.path.expanduser("~/.kani"),
                                                       "kani-" + kani_v, "bin", "cbmc")
    cbmc_line = sh(cbmc, "--version").strip()
    if cbmc_line.split()[:1] != [cbmc_v]:
        sys.exit("run-local.py: %s is %r, not CBMC %s" % (cbmc, cbmc_line, cbmc_v))
    # The mutants' plain mode: RUST_TOOLCHAIN, as CI's kani-mutants jobs.
    r = subprocess.run(["rustc", "--version"], capture_output=True, text=True,
                       env=dict(os.environ, RUSTUP_TOOLCHAIN=rust_v))
    rustc = r.stdout.strip()
    if r.returncode != 0 or not rustc.startswith("rustc %s " % rust_v):
        sys.exit("run-local.py: the mutant jobs' plain mode needs Rust %s (tools/versions.env RUST_TOOLCHAIN), as "
                 "in CI: rustup toolchain install %s --profile minimal (rustc: %s)"
                 % (rust_v, rust_v, (rustc or r.stderr.strip())[:200]))
    smr = os.path.join(ROOT, SP, "source-mirror")
    if subprocess.run(["git", "-C", smr, "cat-file", "-e", core + "^{commit}"], capture_output=True).returncode:
        sys.exit("run-local.py: the source-mirror submodule lacks %s: git submodule update --init %s/source-mirror"
                 % (core, SP))

    os.makedirs(os.path.join(out, "logs"))
    lanes = ["lane%d" % i for i in range(1, a.lanes + 1)]
    for lane in lanes:
        make_lane(os.path.join(out, "lanes", lane), commit, core, cargo_above)
    with open(os.path.join(out, "env.txt"), "w") as f:
        f.write("commit %s\nkani-tree %s\ncore-pin %s\nkani %s\nCBMC %s\ncbmc %s\nrustc %s\nhost %s\n"
                "lanes %d\nbudget-gb %g\nkani-max-gb %s\nkani-env %s\ncargo-config %s\n"
                "runner projects/shared-pool/kani/local/run-local.py\nstarted %s\n"
                % (commit, kani_tree, core, kani_line, cbmc_v, cbmc_line,
                   rustc + " (RUSTUP_TOOLCHAIN of the mutant jobs' plain mode, as in CI)", host(), a.lanes,
                   a.budget_gb, os.environ.get("KANI_MAX_GB") or "40 (run.sh's default)", kani_env,
                   " ".join(configs) or "none", iso(started)))
    if a.all:
        fp = sh(sys.executable, "-I", CHECK, "fingerprint", commit).splitlines()[-1].split("\t")[1]
        nh = sum(j["kind"] == "harness" for j in jobs)
        with open(os.path.join(out, "env.txt"), "a") as f:
            f.write("mode measurement (run-local.py --all: %s; not a record)\nfingerprint %s\nlock %s (waited %.0f s)\n"
                    "estimates V11_MEASURED, then %s (peak x%g), mutants from their harnesses + %d s, unknown "
                    "%g GB %d s\n"
                    % ("--only " + a.only + ": %d harnesses, %d mutants" % (nh, len(jobs) - nh)
                       if a.only is not None else "every harness of run.sh list and every mutant",
                       fp, LOCK, waited, V10_RESULTS[0][:12] + ":" + V10_RESULTS[1], V10_PEAK_SCALE,
                       MUTANT_OVERHEAD, MAX_GB, UNKNOWN_SECS))
        if a.estimates_from:
            with open(os.path.join(out, "env.txt"), "a") as f:
                f.write("estimates-from %s (%d harnesses measured there come first)\n"
                        % (os.path.abspath(a.estimates_from), preloaded))
        with open(os.path.join(out, "estimates.tsv"), "w") as f:
            f.write("job\tkind\test_peak_gb\test_secs\tlimit_s\tsource\n")
            for j in jobs:
                f.write("%s\t%s\t%s\t%.0f\t%d\t%s\n" % (j["name"], j["kind"], j["peak"], j["est"], j["timeout"],
                                                       j["source"]))
    with open(os.path.join(out, "jobs.json"), "w") as f:
        json.dump(jobs, f, indent=1)

    def log(msg):
        line = now().strftime("%H:%M:%SZ ") + msg
        with open(os.path.join(out, "progress.log"), "a") as f:
            f.write(line + "\n")
        print(line, flush=True)

    running, pending, ok = {}, list(jobs), True
    # A signal while a job is being started waits until it is in running
    # (so that stop kills it too).
    starting, deferred = [False], []

    def stop(signum, _frame):
        if starting[0]:
            deferred.append(signum)
            return
        for j in running.values():
            kill_job(j["proc"])
        log("stopped by signal %d; not complete" % signum)
        sys.exit(1)

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    if a.all:
        # Any exit (an error too) stops the jobs before the lock is released
        # (atexit runs this first: it was registered last).
        def kill_running():
            for j in running.values():
                kill_job(j["proc"])
        atexit.register(kill_running)
    log(("measurement (run-local.py --all, not a record): " if a.all else "")
        + "%d jobs: %d harnesses, %d mutants; budget %g GB; output %s"
        % (len(jobs), sum(j["kind"] == "harness" for j in jobs), sum(j["kind"] == "mutant" for j in jobs),
           a.budget_gb, out))
    results = open(os.path.join(out, "results.tsv"), "a")
    while pending or running:
        for lane in lanes:
            if lane in running or not pending:
                continue
            # A measurement's estimates are uncertain: a running job counts
            # what it already uses when that is more, and a large one runs
            # alone.
            counted = [max(j["peak"], j["ptree"] / 1048576) if a.all else j["peak"] for j in running.values()]
            pick = next((j for j in pending if fits(j, counted, a.budget_gb, ALONE_GB if a.all else None)), None)
            if pick is None:
                break
            pending.remove(pick)
            d = os.path.join(out, "logs", pick["name"])
            os.makedirs(d, exist_ok=True)
            env = dict(os.environ, KANI_LOG_DIR=d, KANI_DEFER_COVERS="1", KANI_TIMEOUT=str(pick["timeout"]),
                       KEEP_LOGS=d, KANI_CBMC=cbmc)
            if pick["kind"] == "mutant":
                env["RUSTUP_TOOLCHAIN"] = rust_v
            lane_dir = os.path.join(out, "lanes", lane)
            so = open(os.path.join(d, "stdout.txt"), "w")
            starting[0] = True
            try:
                pick["proc"] = subprocess.Popen(["bash", os.path.join(lane_dir, SP, "kani/run.sh")] + pick["args"],
                                                env=env, stdout=so, stderr=subprocess.STDOUT, cwd=lane_dir,
                                                start_new_session=True)
                pick["t0"], pick["start"] = time.monotonic(), iso(now())
                pick["ptree"], pick["pcbmc"] = 0, 0
                running[lane] = pick
            finally:
                starting[0] = False
            if deferred:
                stop(deferred[0], None)
            log("start %s on %s (peak %s GB, est %.0f s, limit %d s)" % (pick["name"], lane, pick["peak"],
                                                                         pick["est"], pick["timeout"]))
        time.sleep(3)
        for lane, j in list(running.items()):
            t, c = tree_rss(j["proc"].pid)
            j["ptree"], j["pcbmc"] = max(j["ptree"], t), max(j["pcbmc"], c)
            rc = j["proc"].poll()
            if rc is None:
                continue
            secs = time.monotonic() - j["t0"]
            with open(os.path.join(out, "logs", j["name"], "stdout.txt")) as f:
                v = verdict(j, f.read())
            ok = ok and rc == 0 and passed(j, v)
            row = [j["name"], j["kind"], lane, j["start"], "%.0f" % secs, str(rc), v, "%.1f" % (j["ptree"] / 1048576),
                   "%.1f" % (j["pcbmc"] / 1048576), "%s" % j["peak"], "%.0f" % j["est"]]
            results.write("\t".join(row) + "\n")
            results.flush()
            log("done  %s rc=%d %s %.0f s peak %.1f GB cbmc / %.1f GB tree"
                % (j["name"], rc, v, secs, j["pcbmc"] / 1048576, j["ptree"] / 1048576))
            del running[lane]
            if a.all and j["kind"] == "harness" and rc == 0 and passed(j, v):
                # A harness measured here is the best source for the mutants
                # whose Kani leg runs it (they run after the harnesses).
                hest[j["name"]] = (round(secs), round(j["ptree"] / 1048576, 1), "this run")
                for m in pending:
                    if m["kind"] == "mutant" and m["basis"] != "W7" and j["name"] in m["leg"]:
                        m["est"], m["peak"], m["basis"], m["source"] = mutant_estimate(m["leg"], m["survive"], hest)
                        log("estimate %s: %.1f GB, %.0f s (%s)" % (m["name"], m["peak"], m["est"], m["source"]))
                pending.sort(key=all_order)
    results.close()
    with open(os.path.join(out, "env.txt"), "a") as f:
        f.write("finished %s\n" % iso(now()))
    log("ALL DONE")
    if a.all:
        print("run-local.py: measurement, not a record: %s; %s" % ("every job passed" if ok else
                                                                  "SOME JOBS FAILED", os.path.join(out, "results.tsv")))
        return 0 if ok else 1
    print("run-local.py: %s; record it with: %s %s" % ("every job passed" if ok else "SOME JOBS FAILED (results.tsv)",
                                                       os.path.relpath(RECORD), out))
    if a.record and ok:
        return subprocess.run([sys.executable, "-I", RECORD, out]).returncode
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
