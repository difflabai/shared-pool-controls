#!/usr/bin/env python3
"""The call classes of a conformance core.txt (strategy.md section 10).

usage: python3 coverage.py [--quiet] CORE_TXT

Reads a core.txt in either format, v1.0.0's (State = s/5) or v1.1.0's
(s/7, init_error; the header of src/main.rs), recomputes from the terms
alone the class of every call (`Op:Result`) and the derived classes of the
v1.1.0 contract, and applies the REQUIRED and FORBIDDEN lists:

  submit:prune               submit ok and FirstId grew
  submit:full_open_front     submit full, 256 records retained, the oldest open
  submit:prereq_after_prune  submit ok of a recipe with a prerequisite that the
                             retained Completed records alone do not meet
  submit:recovery_admitted   submit ok in a disturbed pre-state (a value
                             outside its metric's range)
  <op>:not_found_pruned      dispatch, ack, cancel or settle not_found on an
                             id below FirstId
  observe:refresh            observe ok with a record open
  init:invalid_config        a refused configuration (init_error)

A v1.0.0 state has FirstId 0. It also checks that no state has a version at
or above VERSION_CAP (TLC's integers; the contract keeps cap states out of
core.txt).

This is a second implementation of the generator's classification
(`classes` in src/main.rs), written from the trace contract, not from that
code. The two lists are the generator's; when src/main.rs is next to this
file, they must be equal (else exit 2).

Exit 0: every required class occurs, no forbidden one does, no state at the
cap. Exit 1: otherwise (the classes missing or forbidden are listed). Exit 2:
the file does not parse, or the lists differ from src/main.rs.

On the v1.0.0 corpus (traces/core.txt at commit a072a2d, the v1.0.0
verification's head) it exits 1; on the v1.1.0 corpus, 0. Standard library
only.
"""

import re
import sys
from collections import Counter
from pathlib import Path

MAX_RECORDS = 256
VERSION_CAP = 2**32 - 1 - 3 * MAX_RECORDS

REQUIRED = [
    "submit:ok", "submit:arithmetic", "submit:full", "submit:paused", "submit:stale", "submit:expired",
    "submit:snapshot_expired", "submit:invalid_recipe", "submit:unauthorized", "submit:invalid_units",
    "submit:prerequisite", "submit:bounds", "submit:energy", "dispatch:ok", "dispatch:not_found",
    "dispatch:unauthorized", "dispatch:paused", "dispatch:invalid_state", "dispatch:expired",
    "dispatch:snapshot_expired", "ack:ok", "ack:not_found", "cancel:ok", "cancel:not_found",
    "cancel:unauthorized", "cancel:pending", "cancel:invalid_state", "settle:ok", "settle:not_found",
    "settle:unauthorized", "settle:invalid_state", "pause:ok", "pause:unauthorized", "observe:ok",
    "observe:unauthorized", "observe:pending", "observe:bounds",
    "submit:prune", "submit:full_open_front", "submit:prereq_after_prune", "submit:recovery_admitted",
    "dispatch:not_found_pruned", "ack:not_found_pruned", "cancel:not_found_pruned", "settle:not_found_pruned",
    "observe:refresh", "init:invalid_config",
]
FORBIDDEN = ["cancel:bounds", "cancel:arithmetic", "settle:bounds", "settle:arithmetic"]

OPEN = {"reserved", "dispatched", "active", "unknown"}


class ParseError(Exception):
    pass


# --- terms ----------------------------------------------------------------
# An atom is a str, an integer an int, a list a Python list, a compound
# term a tuple (name, arg, ...).

TOKEN = re.compile(r"\s*(?:(-?\d+)|([a-z][A-Za-z0-9_]*)|([\[\](),.]))")


def parse_line(line):
    """The one term of a line `Term.`."""
    toks = []
    pos = 0
    while pos < len(line):
        m = TOKEN.match(line, pos)
        if not m:
            if line[pos:].strip() == "":
                break
            raise ParseError(f"bad character at column {pos + 1}")
        pos = m.end()
        if m.group(1) is not None:
            toks.append(("int", int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(("atom", m.group(2)))
        else:
            toks.append(("punct", m.group(3)))
    if not toks or toks[-1] != ("punct", "."):
        raise ParseError("no terminating '.'")
    term, i = parse_term(toks, 0)
    if i != len(toks) - 1:
        raise ParseError("text after the term")
    return term


def parse_term(toks, i):
    kind, v = toks[i]
    if kind == "int":
        return v, i + 1
    if kind == "atom":
        if i + 1 < len(toks) and toks[i + 1] == ("punct", "("):
            args, i = parse_seq(toks, i + 2, ")")
            return (v, *args), i
        return v, i + 1
    if v == "[":
        if toks[i + 1] == ("punct", "]"):
            return [], i + 2
        args, i = parse_seq(toks, i + 1, "]")
        return args, i
    raise ParseError(f"unexpected {v!r}")


def parse_seq(toks, i, close):
    args = []
    while True:
        t, i = parse_term(toks, i)
        args.append(t)
        if toks[i] == ("punct", ","):
            i += 1
        elif toks[i] == ("punct", close):
            return args, i + 1
        else:
            raise ParseError(f"expected ',' or {close!r}")


def functor(t):
    return (t[0], len(t) - 1) if isinstance(t, tuple) else (t, 0)


# --- states ---------------------------------------------------------------


class State:
    def __init__(self, t):
        if functor(t) not in (("s", 5), ("s", 7)):
            raise ParseError(f"not a state: {functor(t)}")
        self.values, self.version, self.observed_at, self.paused, recs = t[1:6]
        self.records = [r[1:] for r in recs]  # (recipe, units, expires_at, status)
        self.first_id = t[6] if len(t) == 8 else 0
        self.done = t[7] if len(t) == 8 else None


class Config:
    def __init__(self, t):
        if functor(t) != ("config", 5):
            raise ParseError("expected config/5")
        _, self.owner, self.max_age, ms, _ts, rs = t
        self.metrics = [(m[1], m[2]) for m in ms]  # (min, max)
        self.requires = [[(p[1], p[2]) for p in r[6]] for r in rs]


def disturbed(cfg, st):
    return any(v < lo or v > hi for v, (lo, hi) in zip(st.values, cfg.metrics))


def has_open(st):
    return any(r[3] in OPEN for r in st.records)


def result_name(res):
    if res == "ok" or functor(res) == ("ok", 1):
        return "ok"
    if functor(res) == ("err", 1):
        return res[1]
    raise ParseError(f"bad result {res!r}")


def classes(cfg, op, res, a, b):
    """The classes of one call: op, result, pre-state a, post-state b."""
    name = op[0]
    r = result_name(res)
    ks = [f"{name}:{r}"]
    if name == "submit" and r == "ok":
        if b.first_id > a.first_id:
            ks.append("submit:prune")
        if disturbed(cfg, a):
            ks.append("submit:recovery_admitted")
        recipe = op[2]

        def retained(q):
            return sum(u for (rq, u, _e, s) in a.records if rq == q and s == "completed")

        if any(retained(q) < u for (q, u) in cfg.requires[recipe]):
            ks.append("submit:prereq_after_prune")
    elif name == "submit" and r == "full":
        if len(a.records) >= MAX_RECORDS and a.records[0][3] in OPEN:
            ks.append("submit:full_open_front")
    elif name in ("dispatch", "ack", "cancel", "settle") and r == "not_found":
        rid = op[1] if name == "ack" else op[2]
        if rid < a.first_id:
            ks.append(f"{name}:not_found_pruned")
    elif name == "observe" and r == "ok" and has_open(a):
        ks.append("observe:refresh")
    return ks


# --- the file -------------------------------------------------------------


def scan(path):
    """(hits, where, problems): class counts, the first places of each class
    (trace, call), and contract problems (states at the cap)."""
    hits = Counter()
    where = {}
    problems = []
    traces = 0
    calls = 0
    n = cfg = st = None
    with open(path) as f:
        for lineno, line in enumerate(f, 1):
            if not line.strip():
                continue
            try:
                t = parse_line(line)
                fn = functor(t)
                if fn == ("trace", 1):
                    n, cfg, st, k = t[1], None, None, 0
                    traces += 1
                elif fn == ("config", 5):
                    cfg = Config(t)
                elif fn == ("init", 2):
                    st = State(t[2])
                elif fn == ("init_error", 1):
                    key = f"init:{t[1]}"
                    hits[key] += 1
                    where.setdefault(key, []).append((n, 0))
                elif fn == ("call", 3):
                    k += 1
                    calls += 1
                    b = State(t[3])
                    for key in classes(cfg, t[1], t[2], st, b):
                        hits[key] += 1
                        if len(where.setdefault(key, [])) < 5:
                            where[key].append((n, k))
                    st = b
                    if st.version >= VERSION_CAP:
                        problems.append(f"trace {n} call {k}: version {st.version} >= VERSION_CAP")
                elif fn == ("end", 0):
                    pass
                else:
                    raise ParseError(f"unexpected {fn[0]}/{fn[1]}")
            except (ParseError, IndexError, TypeError, ValueError) as e:
                raise ParseError(f"{path}:{lineno}: {e}") from None
    return hits, where, problems, traces, calls


def rust_list(src, name):
    m = re.search(r"const " + name + r": \[&str; \d+\] = \[(.*?)\];", src, re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else None


def lists_agree():
    main = Path(__file__).resolve().parent / "src" / "main.rs"
    if not main.exists():
        return True
    src = main.read_text()
    ok = True
    for name, mine in (("REQUIRED", REQUIRED), ("FORBIDDEN", FORBIDDEN)):
        theirs = rust_list(src, name)
        if theirs is None:
            print(f"coverage.py: no const {name} in {main}", file=sys.stderr)
            ok = False
        elif theirs != mine:
            only_here = [k for k in mine if k not in theirs]
            only_there = [k for k in theirs if k not in mine]
            print(f"coverage.py: {name} differs from {main}: only here {only_here}, "
                  f"only there {only_there}" + ("" if only_here or only_there else " (order)"),
                  file=sys.stderr)
            ok = False
    return ok


def main(argv):
    args = [a for a in argv[1:] if a != "--quiet"]
    quiet = "--quiet" in argv[1:]
    if len(args) != 1:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    if not lists_agree():
        return 2
    try:
        hits, where, problems, traces, calls = scan(args[0])
    except (OSError, ParseError) as e:
        print(f"coverage.py: {e}", file=sys.stderr)
        return 2
    if not quiet:
        for k in sorted(hits):
            print(f"{k:32} {hits[k]}")
    print(f"{args[0]}: {traces} traces, {calls} calls")
    missing = [k for k in REQUIRED if k not in hits]
    forbidden = [k for k in FORBIDDEN if k in hits]
    for k in missing:
        print(f"missing (required): {k}")
    for k in forbidden:
        at = ", ".join(f"trace {n} call {c}" for n, c in where[k])
        print(f"forbidden: {k} x{hits[k]} (first at {at})")
    for p in problems[:10]:
        print(f"at the cap: {p}")
    if missing or forbidden or problems:
        print(f"coverage: FAIL ({len(missing)} missing, {len(forbidden)} forbidden, {len(problems)} at the cap)")
        return 1
    print("coverage: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
