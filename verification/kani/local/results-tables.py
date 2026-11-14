#!/usr/bin/env python3
"""kani/RESULTS.md's measured tables, generated from run-local.py output directories.

usage: results-tables.py [--write] [--final] [--rev REV] [--covers LABEL=FILE] [RUN ...]
  RUN       LABEL=DIR: a run-local.py output directory (--all or --record;
            results.tsv, jobs.json, progress.log, logs/<job>/), named LABEL
            in the tables' Run column ("run 1=~/shared-pool-kani-measure/
            all-2d33d33"). Later RUNs win for a job that passed in two; a
            failed job never replaces a passed one.
  --write   replace RESULTS.md's generated blocks (<!-- kani-results-<name>:
            begin ... --> to <!-- kani-results-<name>: end -->, for the names
            summary, harnesses, mutants and covers) instead of printing them.
            The local-record block (record.py's) is not touched.
  --final   the runs are over: label a job that no RUN measured and that no
            hosted CI matrix (kani, kani-mutants) holds "not run" instead of
            "pending", and no job "pending (LABEL)" (a mutant declared in
            kani/local/not-run.tsv is "not run at v1.1.0" either way).
  --rev     the commit whose run.sh list and mutants/ are the job list
            (default HEAD; a RUN at another commit with the same Kani
            fingerprint describes the same inputs).
  --covers  LABEL=FILE (repeatable): more cover statuses for the cover-family
            table, a covers.tsv of run.sh (family, cover, status), e.g. W7's
            run.sh ref.

Rows: every harness of run.sh list and every mutants/*.patch. A job a RUN
finished has its numbers (run.sh's seconds, CBMC's and the process tree's
peak, the CNF size, the covers; a mutant's verdicts, job seconds, the Kani
harness of its leg and the violated property). A job no RUN finished but
that W7 measured on the v1.1.0 Kani harness code (the probes at 69cca54 and
run.sh ref at db957d4; the Kani-mode code of those harnesses is unchanged
since, 6b120fd changed plain mode only) has W7's numbers. A job of a RUN
still going (its progress.log ends in neither ALL DONE nor a stop) is
"pending (LABEL)". Any other job is the hosted CI job's whose matrix holds
it (the kani and kani-mutants jobs run them on every change), declared not
run (not-run.tsv), or pending / not run (--final), with an estimate:
run-local.py's from the RUNs' results, or, for a harness still at v1.0.0's
peak x1.25, the latest RUN's schedule (jobs.json) where it had a better
basis (run 3's holds h_full_256 at 30 GB); mutants from their harnesses.
The CI job column is the workflow's matrices as they are now
(check-kani-record.py matrix; a matrix it cannot read is an error).

The summary: a Verdict line (the Status rule of RESULTS.md: no failure and,
with --final, no harness not run; "open" without --final), the harnesses,
the mutants, the Split (the four matrices' sizes), the Split check (every
job in exactly one matrix, no matrix name that is not a job, no job
measured here in kani / kani-mutants above 13 GB) and the Failures.
"""
import argparse
import importlib.util
import json
import math
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
KANI = os.path.join(ROOT, "projects/shared-pool/kani")
CHECK = os.path.join(ROOT, "tools/scripts/check-kani-record.py")
RESULTS_MD = os.environ.get("KANI_RESULTS_MD") or os.path.join(KANI, "RESULTS.md")
IN_MEMORY_GB = 13  # check-kani-record.py's: the hosted matrices' entries peak at this or less
COLS = ["job", "kind", "lane", "start_utc", "secs", "rc", "verdict", "peak_tree_gb", "peak_cbmc_gb",
        "est_peak_gb", "est_secs"]
# Large mutants declared not run (record.py's and check-kani-record.py's file).
NOT_RUN = os.path.join(HERE, "not-run.tsv")

# W7's v1.1.0 measurements (w7-notes.md "Probes" and "Kani: h_ref"): run.sh's
# seconds, CBMC's peak GB, the process tree's peak GB (None: not sampled),
# clauses (M, the logs' last CNF), covers, label. A probe has both peaks
# (w7-probes/probes.tsv cbmc_peak_gb, tree_peak_gb); a ref run has cbmc's
# while that harness ran only. Kani harness code of 69cca54 (probes) and
# db957d4 (run.sh ref), one job at a time.
W7 = {
    "h_full_256_prune": (451, 20.5, 20.5, 39.4, "3 of 6", "probe P1"),
    "h_step_submit_inv": (1061, 29.7, 29.7, 74.8, "0 of 67", "probe P2"),
    "h_init": (452, 32.9, 33.0, 54.7, "2 of 2", "probe P3"),
    "h_wedge": (573, 32.6, 32.6, 80.7, "3 of 3", "probe P4"),
    "h_step_observe_tx": (113, 11.7, 11.7, 25.9, "0 of 67", "probe P5"),
    "h_validate": (746, 32.9, 32.9, 77.5, "11 of 11", "probe"),
    "h_ref_submit": (507, 14.7, None, 28.3, "12 of 43", "ref run"),
    "h_ref_dispatch": (67, 7.5, None, 12.3, "8 of 43", "ref run"),
    "h_ref_ack": (57, 6.0, None, 12.0, "3 of 43", "ref run"),
    "h_ref_cancel": (129, 11.1, None, 19.9, "6 of 43", "ref run"),
    "h_ref_settle": (273, 14.7, None, 28.6, "5 of 43", "ref run"),
    "h_ref_pause": (59, 6.0, None, 12.0, "3 of 43", "ref run"),
    "h_ref_observe": (80, 8.6, None, 16.8, "6 of 43", "ref run"),
}
# Mutant 31 (W10 review, 6b120fd): job seconds, peak (W7, db957d4), killer, property.
W7_MUTANTS = {
    "31": (161, 9.8, "h_ref_cancel", "REF-1 guard exactness: the core's result is the TLA+ error operator's",
           "6b120fd"),
}
# The plain verdict of every mutant: run.sh mutants at 6b120fd (W10), rc 0.
PLAIN_REF = "6b120fd"


def sh(*args):
    r = subprocess.run(list(args), capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit("results-tables.py: %s: exit %d: %s" % (" ".join(args), r.returncode, (r.stderr or r.stdout).strip()))
    return r.stdout


def run_local():
    """run-local.py as a module (its job list and estimates), without bytecode."""
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("run_local", os.path.join(HERE, "run-local.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def matrices():
    """{job name: CI job}, {CI job: [(entry, gb, timeout)]}, [name listed twice] from the workflow's
    four Kani matrices. A matrix that cannot be read is an error, not an empty matrix."""
    where, entries, twice = {}, {}, []
    for ci in ("kani", "kani-large", "kani-mutants", "kani-mutants-large"):
        out = sh(sys.executable, "-I", CHECK, "matrix", ci)
        for line in out.splitlines():
            if not line.strip():
                continue
            entries.setdefault(ci, []).append(tuple(line.split("\t")))
            for n in line.split("\t")[0].split():
                key = n if ci in ("kani", "kani-large") else "mutant-" + n
                if key in where:
                    twice.append(key)
                where[key] = ci
        if not entries.get(ci):
            sys.exit("results-tables.py: the workflow's %s matrix is empty" % ci)
    return where, entries, twice


def v10_table():
    """{harness: (seconds, peak)} of v1.0.0's Kani results table (a072a2d), or {}."""
    r = subprocess.run(["git", "-C", ROOT, "show", "a072a2dec788:projects/shared-pool/kani/RESULTS.md"],
                       capture_output=True, text=True)
    out = {}
    for m in re.finditer(r"^\| `(h_\w+)` \| (\S+) \| (\d+) \| ([\d.]+) \|", r.stdout, re.M):
        out.setdefault(m.group(1), (int(m.group(3)), float(m.group(4))))
    return out


def clauses(log):
    """The last CNF size in a Kani log, in millions, or None."""
    try:
        with open(log, errors="replace") as f:
            found = re.findall(r"^\d+ variables, (\d+) clauses$", f.read(), re.M)
    except OSError:
        return None
    return int(found[-1]) / 1e6 if found else None


def violated(log):
    """The description of the first violated property in a Kani log (old output format), or None."""
    try:
        with open(log, errors="replace") as f:
            text = f.read()
    except OSError:
        return None
    m = re.search(r"^Violated property:\n[^\n]*\n\s*\"([^\n]*)\"", text, re.M)
    if m:
        return m.group(1)
    m = re.search(r"^Check \d+: [^\n]*\n\s*- Status: FAILURE\n\s*- Description: \"([^\n]*)\"", text, re.M)
    return m.group(1) if m else None


def read_runs(runs):
    """{job: measurement} from the RUN directories, later ones winning; {label: {job: its jobs.json entry}}
    of every RUN (its schedule); the labels of the RUNs still going."""
    got, sched, going = {}, {}, []
    for label, d in runs:
        d = os.path.expanduser(d)
        if not os.path.isdir(d):
            print("results-tables.py: %s: no directory %s (skipped)" % (label, d), file=sys.stderr)
            continue
        try:
            with open(os.path.join(d, "jobs.json")) as f:
                sched[label] = {j["name"]: j for j in json.load(f)}
        except (OSError, ValueError):
            sched[label] = {}
        # A run still going (progress.log ends in neither ALL DONE nor a stop)
        # has its unfinished jobs pending; an ended run's are not.
        try:
            with open(os.path.join(d, "progress.log")) as f:
                last = ([l.strip() for l in f if l.strip()] or [""])[-1]
        except OSError:
            last = ""
        if not re.search(r"ALL DONE|not complete|stopped", last):
            going.append(label)
        try:
            with open(os.path.join(d, "results.tsv")) as f:
                rows = [dict(zip(COLS, l.rstrip("\n").split("\t"))) for l in f if l.strip()]
        except OSError:
            rows = []
        for r in rows:
            if len(r) != len(COLS):
                continue
            name, ld = r["job"], os.path.join(d, "logs", r["job"])
            try:
                with open(os.path.join(ld, "stdout.txt")) as f:
                    so = f.read()
            except OSError:
                so = ""
            m = dict(run=label, secs=r["secs"], rc=r["rc"], verdict=r["verdict"], cbmc=r["peak_cbmc_gb"],
                     tree=r["peak_tree_gb"], start=r["start_utc"])
            if r["kind"] == "harness":
                line = re.search(r"^%s\s+(\S+)\s+(\d+)\s+(.*)$" % re.escape(name), so, re.M)
                m["result"] = line.group(1) if line else r["verdict"]
                m["seconds"] = line.group(2) if line else "-"
                cov = line.group(3).strip() if line else "-"
                m["covers"] = re.sub(r" cover properties satisfied$", "", cov) or "-"
                m["clauses"] = clauses(os.path.join(ld, name + ".log"))
                try:
                    with open(os.path.join(ld, "covers.tsv")) as f:
                        m["cover_rows"] = [l.rstrip("\n").split("\t") for l in f if l.strip()]
                except OSError:
                    m["cover_rows"] = []
            else:
                legs = sorted(f for f in os.listdir(ld) if f.endswith(".log") and not f.endswith(".plain.log")) \
                    if os.path.isdir(ld) else []
                m["legs"] = [f.rsplit(".", 2)[1] for f in legs]
                m["property"] = None
                for f in legs:
                    p = violated(os.path.join(ld, f))
                    if p:
                        m["killer"], m["property"] = f.rsplit(".", 2)[1], p
            prev = got.get(name)
            ok = r["rc"] == "0"
            if prev is None or ok or prev["rc"] != "0":
                got[name] = m
    return got, sched, going


def declared():
    """{patch name: reason} of kani/local/not-run.tsv (mutant<TAB>reason), {} without it."""
    try:
        with open(NOT_RUN) as f:
            rows = [l.rstrip("\n").split("\t") for l in f if l.strip()]
    except OSError:
        return {}
    return {r[0]: r[1] for r in rows[1:] if len(r) == 2}


def patch_info(patch):
    """(what it changes: the first sentence of the patch's description, expected)."""
    with open(os.path.join(KANI, "mutants", patch + ".patch")) as f:
        text = f.read()
    desc = []
    for line in text.splitlines():
        if not line.startswith("#"):
            break
        if re.match(r"^# (killed-by|checked-by|fix-revert|applies-to):|^# must-survive$", line):
            continue
        desc.append(line[1:].strip())
    d = " ".join(desc)
    m = re.match(r"(.*?\.)(\s|$)", d)
    d = (m.group(1) if m else d).replace("|", "\\|")
    survive = re.search(r"^# must-survive$", text, re.M) is not None
    return d, ("survive" if survive else "killed")


def peak_gb(m):
    """A measured job's peak in whole GB as the split counts it: max(cbmc, tree), rounded up."""
    vals = [float(v) for v in (m.get("cbmc"), m.get("tree")) if v not in (None, "", "-")]
    return int(math.ceil(max(vals) - 1e-9)) if vals else 0


def fmt_gb(x):
    return "-" if x in (None, "", "-") else "%.1f" % float(x)


def tables(args):
    rl = run_local()
    commit = sh("git", "-C", ROOT, "rev-parse", "--verify", args.rev + "^{commit}").strip()
    jobs, hest = rl.jobs_all(commit)
    runs = []
    for r in args.runs:
        if "=" not in r:
            sys.exit("results-tables.py: %r: want LABEL=DIR" % r)
        runs.append(tuple(r.split("=", 1)))
    for label, d in runs:
        p = os.path.join(os.path.expanduser(d), "results.tsv")
        if os.path.exists(p):
            rl.preload(jobs, hest, p)
    got, sched, going = read_runs(runs)
    # {label: schedule} of the RUNs still going, whose unfinished jobs are
    # "pending (LABEL)". --final: the runs are over, a job no RUN finished is
    # not pending in one, even if a run's progress.log does not show its end
    # (SIGKILL).
    listed = {} if args.final else {l: sched[l] for l in going}
    # A harness whose estimate is still v1.0.0's (or unknown) takes the
    # latest RUN's schedule where that had a better basis, running or ended
    # (run 3's --estimates-from holds h_full_256 at 30 GB, not v1.0.0's
    # 0.8 GB); the mutants are re-estimated from the harnesses.
    weak = ("v1.0.0", "unknown")
    for j in jobs:
        if j["kind"] != "harness" or j.get("basis") not in weak:
            continue
        for label in reversed(list(sched)):
            e = sched[label].get(j["name"])
            if e and e.get("basis") not in weak and e.get("peak") is not None:
                j["peak"], j["est"], j["basis"] = e["peak"], e.get("est", j["est"]), "%s's schedule" % label
                hest[j["name"]] = (j["est"], j["peak"], j["basis"])
                break
    for j in jobs:
        if j["kind"] == "mutant" and j.get("basis") != "W7":
            j["est"], j["peak"], j["basis"], j["source"] = rl.mutant_estimate(j["leg"], j["survive"], hest)
    extra = []
    for c in args.covers:
        if "=" not in c:
            sys.exit("results-tables.py: --covers %r: want LABEL=FILE" % c)
        label, path = c.split("=", 1)
        with open(os.path.expanduser(path)) as f:
            extra += [(label, l.rstrip("\n").split("\t")) for l in f if l.strip()]
    ci, entries, twice = matrices()
    decl = declared()
    v10 = v10_table()
    order = sh("bash", os.path.join(KANI, "run.sh"), "list").split()
    byname = {j["name"]: j for j in jobs}

    def pending(name, patch=None, survive=False):
        """The Result (Kani) cell of a job no RUN finished and W7 did not measure."""
        for label, names in reversed(list(listed.items())):
            if name in names:
                return "pending (%s)" % label
        if patch in decl:
            return "not run at v1.1.0 (declared in `not-run.tsv`)"
        if ci.get(name) in ("kani", "kani-mutants"):
            return ("CI `%s` (not measured here)" if args.final else "pending (CI `%s`)") % ci[name]
        if not args.final:
            return "pending"
        if patch:
            return "not run at v1.1.0 (plain mode: %s, CI `mutants`)" % ("survives" if survive else "killed")
        return "not run"

    def est(j):
        """The estimate of a job (peak and basis): run-local.py's from the RUNs' results, or a RUN's
        schedule where it had a better basis than v1.0.0's (above)."""
        basis = {"v1.0.0": "v1.0.0 x1.25", "harnesses": "its harnesses", "measured": "measured"}.get(
            j.get("basis"), j.get("basis") or "?")
        return "est. %.1f GB (%s)" % (j.get("peak") or 0.0, basis)

    # Harnesses.
    h_rows, h_stats = [], dict(measured=0, w7=0, pending=0, bad=[], secs=0)
    cover_rows = {}
    for h in order:
        j = byname.get(h, dict(peak=None))
        m = got.get(h)
        want = "ok-FAILED" if h == "h_canary" else "ok"
        old = v10.get(h)
        old = "%d s, %.1f GB" % old if old else "new"
        where = "`%s`" % ci[h] if h in ci else "-"
        if m:
            h_stats["measured"] += 1
            if m["rc"] != "0" or m["result"] != want:
                h_stats["bad"].append("%s %s" % (h, m["result"]))
            if m["seconds"].isdigit():
                h_stats["secs"] += int(m["seconds"])
            cl = "%.1f" % m["clauses"] if m["clauses"] is not None else "-"
            h_rows.append("| `%s` | %s | %s | %s | %s | %s | %s | %s | %s | %s |"
                          % (h, m["result"], m["seconds"], fmt_gb(m["cbmc"]), fmt_gb(m["tree"]), cl, m["covers"],
                             m["run"], where, old))
            for c in m["cover_rows"]:
                if len(c) == 3:
                    cover_rows.setdefault(c[0], {}).setdefault(c[1], set()).add((c[2], h))
        elif h in W7:
            s, gb, tree, cl, cov, label = W7[h]
            h_stats["w7"] += 1
            h_rows.append("| `%s` | ok | %d | %.1f | %s | %.1f | %s | W7 %s | %s | %s |"
                          % (h, s, gb, fmt_gb(tree), cl, cov, label, where, old))
        else:
            h_stats["pending"] += 1
            h_rows.append("| `%s` | %s | - | - | - | - | - | %s | %s | %s |" % (h, pending(h), est(j), where, old))
    hdr = ["| Harness | Result | Time (s) | Peak (GB) | Tree (GB) | Clauses (M) | Covers SATISFIED (this harness) "
           "| Run | CI job | v1.0.0 (time, peak) |",
           "|---|---|---:|---:|---:|---:|---|---|---|---|"]
    n = len(order)
    summary = ("%d harnesses (`run.sh list` at `%s`): %d measured in the runs (Run column%s), %d by W7 only, %d not "
               "measured here. Sum of the measured harnesses' times: %d s (%.1f h)."
               % (n, commit[:7], h_stats["measured"],
                  "; all as required" if not h_stats["bad"] else "; NOT as required: %s" % ", ".join(h_stats["bad"]),
                  h_stats["w7"], h_stats["pending"], h_stats["secs"], h_stats["secs"] / 3600))
    harness_block = "\n".join([summary, ""] + hdr + h_rows) + "\n"

    # Mutants.
    m_rows, m_stats = [], dict(measured=0, w7=0, pending=0, bad=[])
    for j in sorted((j for j in jobs if j["kind"] == "mutant"), key=lambda j: j["patch"]):
        name, patch = j["name"], j["patch"]
        nn = name.split("-", 1)[1]
        what, exp = patch_info(patch)
        m = got.get(name)
        where = "`%s`" % ci[name] if name in ci else "-"
        plain_ref = ("killed" if exp == "killed" else "survived") + " (%s)" % PLAIN_REF
        if m:
            m_stats["measured"] += 1
            v = m["verdict"].split("/")
            e, p, k = (v + ["?", "?", "?"])[:3]
            if m["rc"] != "0" or (e, p, k) not in (("killed",) * 3, ("survive", "survived", "survived")):
                m_stats["bad"].append("%s %s" % (name, m["verdict"]))
            legs = m.get("legs") or []
            if exp == "killed":
                kh = m.get("killer") or (legs[-1] if legs else "-")
                prop = m.get("property")
                kani = "%s (`%s`%s)" % (k, kh, ': "%s"' % prop.replace("|", "\\|") if prop else "")
            else:
                kani = "%s (%s)" % (k, ", ".join("`%s`" % l for l in legs) or "-")
            m_rows.append("| `%s` | %s | %s | %s | %s | %s | %s | %s | %s |"
                          % (patch, what, e, p, kani, m["secs"], fmt_gb(m["cbmc"]), m["run"], where))
        elif nn in W7_MUTANTS:
            s, gb, kh, prop, label = W7_MUTANTS[nn]
            m_stats["w7"] += 1
            m_rows.append('| `%s` | %s | %s | %s | killed (`%s`: "%s") | %d | %.1f | W10 at %s | %s |'
                          % (patch, what, exp, plain_ref, kh, prop, s, gb, label, where))
        else:
            m_stats["pending"] += 1
            leg = ", ".join("`%s`" % h for h in j.get("leg") or []) or "-"
            m_rows.append("| `%s` | %s | %s | %s | %s; leg %s | - | - | %s | %s |"
                          % (patch, what, exp, plain_ref, pending(name, patch, exp == "survive"), leg, est(j), where))
    nm = len([j for j in jobs if j["kind"] == "mutant"])
    msum = ("%d mutants (`mutants/*.patch` at `%s`): %d run under Kani in the runs (Run column%s), %d earlier "
            "(W10) only, %d not run under Kani here. Plain mode: every one as expected at `%s` (`run.sh mutants`), and CI's "
            "`mutants` job reruns all of them."
            % (nm, commit[:7], m_stats["measured"],
               "; all as expected" if not m_stats["bad"] else "; NOT as expected: %s" % ", ".join(m_stats["bad"]),
               m_stats["w7"], m_stats["pending"], PLAIN_REF))
    mhdr = ["| Mutant | What it changes | Expected | Plain | Kani (harness: violated property) | Time (s) | Peak (GB) "
            "| Run | CI job |",
            "|---|---|---|---|---|---:|---:|---|---|"]
    mutant_block = "\n".join([msum, ""] + mhdr + m_rows) + "\n"

    # Cover families, from the measured harnesses' covers.tsv.
    fam_h = {}
    for h in order:
        fam_h.setdefault(family(h), []).append(h)
    extra_by = {}
    for label, c in extra:
        if len(c) >= 3:
            cover_rows.setdefault(c[0], {}).setdefault(c[1], set()).add((c[2], label))
            extra_by.setdefault(c[0], set()).add(label)
    c_rows = []
    for f in sorted(fam_h, key=lambda f: order.index(fam_h[f][0])):
        hs = fam_h[f]
        meas = [h for h in hs if h in got]
        covs = cover_rows.get(f, {})
        sat = [c for c, st in covs.items() if any(s == "SATISFIED" for s, _ in st)]
        uns = sorted(c for c in covs if c not in sat)
        w7h = [h for h in hs if h not in got and h in W7]
        state = "%d of %d in the runs" % (len(meas), len(hs))
        if w7h:
            state += ", %d by W7 only" % len(w7h)
        if f in extra_by:
            state += "; with %s's covers.tsv" % ", ".join(sorted(extra_by[f]))
        done = len(meas) + len(w7h) == len(hs) and (not w7h or f in extra_by)
        if not covs:
            unsat = "-"
        elif not uns:
            unsat = "none"
        elif done:
            unsat = "; ".join('"%s"' % c.replace("|", "\\|") for c in uns)
        else:
            rest = [h for h in hs if h not in meas and h not in w7h]
            if not rest:
                # Every harness read, some only by W7, whose covers.tsv was not given.
                unsat = "%d in these logs (W7's covers.tsv not given: `--covers`)" % len(uns)
            else:
                unsat = "%d in these logs (%s: %s)" % (
                    len(uns), "not measured here" if args.final else "pending",
                    ", ".join("`%s`" % h for h in rest) if len(rest) <= 3 else "%d harnesses" % len(rest))
        c_rows.append("| `%s` | %s | %s | %s | %s |"
                      % (f, len(covs) if covs else "-", len(sat) if covs else "-", unsat, state))
    chdr = ["| Family | Covers (in the logs read) | SATISFIED | Not satisfied | Harnesses read |",
            "|---|---:|---:|---|---|"]
    cover_block = "\n".join(chdr + c_rows) + "\n"

    # Summary.
    def by(names):
        c = {}
        for x in names:
            c[got[x]["run"]] = c.get(got[x]["run"], 0) + 1
        return ", ".join("%s: %d" % kv for kv in sorted(c.items()))

    def running(x):
        return any(x in names for names in listed.values())

    bad = {b.split()[0] for b in h_stats["bad"] + m_stats["bad"]}
    h_ok = [h for h in order if h in got and h not in bad]
    h_w7 = [h for h in order if h not in got and h in W7]
    h_rest = [h for h in order if h not in got and h not in W7]
    h_run = [h for h in h_rest if running(h)]
    h_hosted = [h for h in h_rest if not running(h) and ci.get(h) == "kani"]
    h_other = [h for h in h_rest if not running(h) and ci.get(h) != "kani"]
    mj = sorted((j for j in jobs if j["kind"] == "mutant"), key=lambda j: j["patch"])
    m_ok = [j["name"] for j in mj if j["name"] in got and j["name"] not in bad]
    m_w7 = [j["name"] for j in mj if j["name"] not in got and j["name"].split("-", 1)[1] in W7_MUTANTS]
    m_rest = [j for j in mj if j["name"] not in got and j["name"].split("-", 1)[1] not in W7_MUTANTS]
    m_run = [j for j in m_rest if running(j["name"])]
    m_decl = [j for j in m_rest if not running(j["name"]) and j["patch"] in decl]
    m_hosted = [j for j in m_rest if not running(j["name"]) and j["patch"] not in decl
                and ci.get(j["name"]) == "kani-mutants"]
    m_other = [j for j in m_rest if not running(j["name"]) and j["patch"] not in decl
               and ci.get(j["name"]) != "kani-mutants"]

    every = list(order) + [j["name"] for j in mj]
    hosted_got = [x for x in every if x in got and ci.get(x) in ("kani", "kani-mutants")]
    over = [x for x in hosted_got if peak_gb(got[x]) > IN_MEMORY_GB]
    # Placement: every job in exactly one matrix, every matrix name a job.
    unplaced = [x for x in every if x not in ci]
    stale = sorted(x for x in ci if x not in set(every))
    split_bad = (["`%s` in `%s` peaked %d GB" % (x, ci[x], peak_gb(got[x])) for x in over]
                 + ["`%s` is in no matrix" % x for x in unplaced]
                 + ["`%s` is in a matrix but not a job at `%s`" % (x, commit[:7]) for x in stale]
                 + ["`%s` is listed twice in the matrices" % x for x in twice])

    def nums(parts):
        return "; ".join(p for p in parts if p)

    if bad:
        verdict = "not verified: a job failed (the Failures line)"
    elif not args.final:
        verdict = "open: these tables are not final (written without `--final`%s)" % (
            "; %d harnesses still running or queued" % len(h_run) if h_run else "")
    elif h_other:
        verdict = "not verified: %d harnesses not run (%s)" % (len(h_other), ", ".join("`%s`" % h for h in h_other))
    else:
        verdict = ("verified within the bounds, provided CI's `kani` and `kani-record` jobs pass on the commit: no "
                   "failure, and every harness verified on this machine (%d), by W7 on the same Kani harness code "
                   "(%d) or left to CI's hosted `kani` job (%d)" % (len(h_ok), len(h_w7), len(h_hosted)))
    lines = [
        "**Verdict** (the Status rule; the Kani half of a declared mutant's verdict is a gap, not a "
        "failure): %s." % verdict,
        "**Harnesses** (%d, `run.sh list` at `%s`): %s." % (n, commit[:7], nums([
            "%d verified as required on this machine (%s)" % (len(h_ok), by(h_ok)) if h_ok else
            "none verified in the runs",
            "%d only in W7's probes and `run.sh ref` on the same Kani harness code" % len(h_w7) if h_w7 else "",
            "%d still running or queued (%s)" % (len(h_run), ", ".join(sorted(listed))) if h_run else "",
            "%d not measured here, CI's hosted `kani` job runs them on every change" % len(h_hosted)
            if h_hosted else "",
            "%d %s: %s" % (len(h_other), "not run" if args.final else "pending",
                           ", ".join("`%s`" % h for h in h_other)) if h_other else "",
            "%d NOT as required: %s" % (len(h_stats["bad"]), ", ".join(h_stats["bad"])) if h_stats["bad"] else ""])),
        "**Mutants** (%d, `mutants/*.patch` at `%s`): plain mode as expected for all %d at `%s` (`run.sh "
        "mutants`; CI's `mutants` job reruns every one). Under Kani: %s." % (len(mj), commit[:7], len(mj), PLAIN_REF,
                                                                            nums([
            "%d as expected on this machine (%s)" % (len(m_ok), by(m_ok)) if m_ok else "none in the runs",
            "%d earlier (W10, mutant %s)" % (len(m_w7), ", ".join(x.split("-", 1)[1] for x in m_w7)) if m_w7 else "",
            "%d still running or queued" % len(m_run) if m_run else "",
            "%d not run here, CI's hosted `kani-mutants` job's (manual runs)" % len(m_hosted) if m_hosted else "",
            "%d declared not run at v1.1.0 (`kani/local/not-run.tsv`; plain mode only): %s"
            % (len(m_decl), ", ".join(j["patch"].split("-", 1)[0] for j in m_decl)) if m_decl else "",
            "%d %s: %s" % (len(m_other), "not run" if args.final else "pending",
                           ", ".join(j["patch"].split("-", 1)[0] for j in m_other)) if m_other else "",
            "%d NOT as expected: %s" % (len(m_stats["bad"]), ", ".join(m_stats["bad"])) if m_stats["bad"] else ""])),
        "**Split** (the workflow's matrices; hosted runner: `ubuntu-latest`, 16 GB): %s." % "; ".join(
            ("`%s` %d harnesses in %d groups" % (c, sum(len(e[0].split()) for e in entries.get(c, [])),
                                                  len(entries.get(c, []))) if c in ("kani", "kani-large") else
             "`%s` %d mutants" % (c, len(entries.get(c, []))))
            for c in ("kani", "kani-large", "kani-mutants", "kani-mutants-large")),
        "**Split check** (each job in one matrix; a job measured here, peak the larger of CBMC's and the tree's, "
        "rounded up, against the 13 GB line): %s." % (
            "; ".join(split_bad) if split_bad else
            "every job of `run.sh list` and `mutants/*.patch` is in exactly one matrix and every matrix name is "
            "one of them; the %d measured here that are in `kani` or `kani-mutants` peaked at 13 GB or less"
            % len(hosted_got)),
        "**Failures**: %s." % ("none: every harness measured here as required and every mutant run here as "
                               "expected" if not bad else "; ".join(h_stats["bad"] + m_stats["bad"])),
    ]
    summary_block = "\n\n".join(lines) + "\n"
    return dict(summary=summary_block, harnesses=harness_block, mutants=mutant_block, covers=cover_block)


def family(h):
    """run.sh's family() (one cover family per body)."""
    f = h
    for s in ("_tx", "_post", "_inv", "_cov", "_exact"):
        if f.endswith(s):
            f = f[:-len(s)]
    for p, fam in (("h_step_", "h_step"), ("h_version_cap_", "h_version_cap"), ("h_version_max_", "h_version_max"),
                   ("h_wf_", "h_wf"), ("h_full_", "h_full"), ("h_bmc_from_init_", "h_bmc_from_init"),
                   ("h_slot_cycle_", "h_slot_cycle"), ("h_ref_", "h_ref")):
        if f.startswith(p):
            return fam
    return f


def write(blocks):
    with open(RESULTS_MD) as f:
        text = f.read()
    for name, body in blocks.items():
        begin = "<!-- kani-results-%s: begin (kani/local/results-tables.py writes this block; do not edit) -->" % name
        end = "<!-- kani-results-%s: end -->" % name
        m = re.search(r"^<!-- kani-results-%s: begin[^\n]*\n.*?^%s$" % (name, re.escape(end)), text, re.S | re.M)
        if not m:
            sys.exit("results-tables.py: %s has no %s block" % (RESULTS_MD, name))
        text = text[:m.start()] + begin + "\n" + body + end + text[m.end():]
    with open(RESULTS_MD, "w") as f:
        f.write(text)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("runs", nargs="*", metavar="LABEL=DIR")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--final", action="store_true")
    ap.add_argument("--rev", default="HEAD")
    ap.add_argument("--covers", action="append", default=[], metavar="LABEL=FILE")
    args = ap.parse_args()
    blocks = tables(args)
    if args.write:
        write(blocks)
        print("results-tables.py: wrote %s (%s)" % (RESULTS_MD, ", ".join(blocks)))
    else:
        for name, body in blocks.items():
            print("## %s\n\n%s" % (name, body))
    return 0


if __name__ == "__main__":
    sys.exit(main())
