#!/usr/bin/env python3
"""REF-5: convert conformance traces of the real pool-core into a TLA+ module.

usage: python3 -I trace2tla.py TRACES OUT.tla [--module NAME] [--first N] [--count K]

TRACES is projects/shared-pool/conformance/traces/core.txt (format documented
at the top of conformance/src/main.rs). OUT.tla defines

    Traces == << [cfg |-> .., t0 |-> .., err |-> .., init |-> State,
                  steps |-> << Step, .. >>], .. >>

with State = [values, version, observedAt, paused, records, firstId, doneU]
(the core's s(Values, Version, ObservedAt, Paused, Records, FirstId,
CompletedUnits); records keep no id: record k has id firstId + k - 1) and one
Step per call: [op, c, i, r, u, v, exp, now, b, vals, res, id, post]. Fields
an op does not use hold neutral defaults. Results are the core's error names
(snake_case -> CamelCase) or "Ok"; id is submit's Ok(id), else -1. err is ""
for a trace whose config Pool::new accepted; a trace recorded as
`init_error(E)` (Pool::new refused the config) has err = E in CamelCase, no
calls, and a placeholder init state that TraceCheck does not read.

One abstraction, reported on stderr: TLC integers are 32-bit, and 7 submit
calls carry request version u32::MAX (from version.wrapping_sub(1) at
version 0). Request versions >= 2^31 become -1. The model reads a request
version only through `v = version`, and every state version in the traces is
checked below 2^31, so the outcome (Stale) is the same. Any other integer
outside TLC's range is an error.

No dependencies; run with python3 -I.

PROP: REF-5
"""
import re
import sys

TLC_MAX = 2**31 - 1
TOKEN = re.compile(r"\s*(?:(-?\d+)|([a-z][A-Za-z0-9_]*)|(\S))")


def tokens(s):
    pos = 0
    out = []
    while pos < len(s):
        m = TOKEN.match(s, pos)
        if not m or m.end() == pos:
            if s[pos:].strip() == "":
                break
            raise ValueError(f"bad token at {pos}: {s[pos:pos+20]!r}")
        pos = m.end()
        if m.group(1) is not None:
            out.append(("int", int(m.group(1))))
        elif m.group(2) is not None:
            out.append(("atom", m.group(2)))
        elif m.group(3) is not None:
            out.append(("p", m.group(3)))
    return out


class Parser:
    def __init__(self, s):
        self.t = tokens(s)
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else ("eof", None)

    def eat(self, kind, val=None):
        k, v = self.peek()
        if k != kind or (val is not None and v != val):
            raise ValueError(f"expected {kind} {val}, got {k} {v}")
        self.i += 1
        return v

    def term(self):
        k, v = self.peek()
        if k == "int":
            self.i += 1
            return v
        if k == "atom":
            self.i += 1
            if self.peek() == ("p", "("):
                self.i += 1
                args = [self.term()]
                while self.peek() == ("p", ","):
                    self.i += 1
                    args.append(self.term())
                self.eat("p", ")")
                return (v, args)
            return v
        if (k, v) == ("p", "["):
            self.i += 1
            items = []
            if self.peek() != ("p", "]"):
                items.append(self.term())
                while self.peek() == ("p", ","):
                    self.i += 1
                    items.append(self.term())
            self.eat("p", "]")
            return ("[]", items)
        raise ValueError(f"unexpected {k} {v}")

    def clause(self):
        t = self.term()
        self.eat("p", ".")
        if self.peek()[0] != "eof":
            raise ValueError("trailing input")
        return t


def parse_line(line):
    return Parser(line).clause()


def lst(t):
    assert isinstance(t, tuple) and t[0] == "[]", t
    return t[1]


def cmp(t, name, n):
    assert isinstance(t, tuple) and t[0] == name and len(t[1]) == n, (name, t)
    return t[1]


class Ctx:
    def __init__(self):
        self.wrapped_versions = 0
        self.max_state_version = 0


def num(x):
    assert isinstance(x, int), x
    if not -TLC_MAX - 1 <= x <= TLC_MAX:
        raise ValueError(f"integer {x} outside TLC's range")
    return str(x)


def boolean(x):
    assert x in ("true", "false"), x
    return "TRUE" if x == "true" else "FALSE"


def string(x):
    assert isinstance(x, str) and re.fullmatch(r"[a-z][A-Za-z0-9_]*", x), x
    return f'"{x}"'


def seq(items):
    return "<<" + ", ".join(items) + ">>" if items else "<<>>"


def ints(t):
    return seq([num(x) for x in lst(t)])


def camel(snake):
    return "".join(p[:1].upper() + p[1:] for p in snake.split("_"))


STATUS = {s: camel(s) for s in ("reserved", "dispatched", "active", "unknown", "completed", "cancelled")}


def config(t):
    owner, max_age, ms, ts, rs = cmp(t, "config", 5)
    metrics = []
    for m in lst(ms):
        lo, hi, init = cmp(m, "m", 3)
        metrics.append(f"[min |-> {num(lo)}, max |-> {num(hi)}, init |-> {num(init)}]")
    terms = []
    for x in lst(ts):
        metric, target, weight, shape = cmp(x, "t", 4)
        assert shape in ("target", "excess", "deficit")
        terms.append(f'[metric |-> {num(metric)}, target |-> {num(target)}, weight |-> {num(weight)}, shape |-> "{shape}"]')
    recipes = []
    for r in lst(rs):
        actor, executor, reserve, complete, max_units, requires = cmp(r, "r", 6)
        req = [f"<<{num(p)}, {num(u)}>>" for p, u in (cmp(q, "p", 2) for q in lst(requires))]
        recipes.append(
            f"[actor |-> {string(actor)}, executor |-> {string(executor)}, reserve |-> {ints(reserve)}, "
            f"complete |-> {ints(complete)}, maxUnits |-> {num(max_units)}, requires |-> {seq(req)}]"
        )
    return (
        f"[owner |-> {string(owner)}, maxAge |-> {num(max_age)}, metrics |-> {seq(metrics)}, "
        f"terms |-> {seq(terms)}, recipes |-> {seq(recipes)}]"
    )


def state(t, ctx):
    vals, version, observed_at, paused, recs, first_id, done = cmp(t, "s", 7)
    ctx.max_state_version = max(ctx.max_state_version, version)
    records = []
    for c in lst(recs):
        recipe, units, exp, status = cmp(c, "c", 4)
        records.append(f'[recipe |-> {num(recipe)}, units |-> {num(units)}, exp |-> {num(exp)}, status |-> "{STATUS[status]}"]')
    return (
        f"[values |-> {ints(vals)}, version |-> {num(version)}, observedAt |-> {num(observed_at)}, "
        f"paused |-> {boolean(paused)}, records |-> {seq(records)}, firstId |-> {num(first_id)}, "
        f"doneU |-> {ints(done)}]"
    )


# The init of a trace whose config Pool::new refused (never compared).
NO_STATE = ("[values |-> <<>>, version |-> 0, observedAt |-> 0, paused |-> FALSE, "
            "records |-> <<>>, firstId |-> 0, doneU |-> <<>>]")


def request_version(v, ctx):
    if v > TLC_MAX:
        assert v <= 2**32 - 1, v
        ctx.wrapped_versions += 1
        return "-1"
    return num(v)


def call(t, ctx):
    op, res, post = cmp(t, "call", 3)
    name, args = op
    f = {"c": '""', "i": "0", "r": "0", "u": "0", "v": "0", "exp": "0", "now": "0", "b": "FALSE", "vals": "<<>>"}
    if name == "submit":
        c, r, u, v, exp, now = args
        f.update(c=string(c), r=num(r), u=num(u), v=request_version(v, ctx), exp=num(exp), now=num(now))
    elif name == "dispatch":
        c, i, now = args
        f.update(c=string(c), i=num(i), now=num(now))
    elif name == "ack":
        i, a = args
        f.update(i=num(i), b=boolean(a))
    elif name == "cancel":
        c, i, now = args
        f.update(c=string(c), i=num(i), now=num(now))
    elif name == "settle":
        c, i, done = args
        f.update(c=string(c), i=num(i), b=boolean(done))
    elif name == "pause":
        c, b = args
        f.update(c=string(c), b=boolean(b))
    elif name == "observe":
        c, vals, now = args
        f.update(c=string(c), vals=ints(vals), now=num(now))
    else:
        raise ValueError(f"unknown op {name}")
    rid = "-1"
    if res == "ok":
        r = '"Ok"'
    elif isinstance(res, tuple) and res[0] == "ok":
        assert name == "submit"
        r = '"Ok"'
        rid = num(res[1][0])
    else:
        (e,) = cmp(res, "err", 1)
        r = f'"{camel(e)}"'
    fields = ", ".join(f"{k} |-> {v}" for k, v in f.items())
    return f'[op |-> "{name}", {fields}, res |-> {r}, id |-> {rid}, post |-> {state(post, ctx)}]'


def traces(path):
    """Yield (n, config term, init now, init state term, [call terms])."""
    cur = None
    with open(path) as fh:
        for ln, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            try:
                t = parse_line(line)
            except ValueError as e:
                raise SystemExit(f"{path}:{ln}: {e}")
            if isinstance(t, tuple) and t[0] == "trace":
                assert cur is None, ln
                cur = {"n": t[1][0], "calls": []}
            elif t == "end":
                yield cur
                cur = None
            elif t[0] == "config":
                cur["config"] = t
            elif t[0] == "init":
                cur["init"] = t
            elif t[0] == "init_error":
                cur["init_error"] = t
            elif t[0] == "call":
                cur["calls"].append(t)
            else:
                raise SystemExit(f"{path}:{ln}: unexpected {t[0]}")
    assert cur is None, "unterminated trace"


def main(argv):
    args = [a for a in argv[1:]]
    opts = {"--module": None, "--first": "0", "--count": None}
    pos = []
    while args:
        a = args.pop(0)
        if a in opts:
            opts[a] = args.pop(0)
        else:
            pos.append(a)
    if len(pos) != 2:
        raise SystemExit(__doc__)
    src, out = pos
    module = opts["--module"] or re.sub(r".*/|\.tla$", "", out)
    first = int(opts["--first"])
    ctx = Ctx()
    entries = []
    total_calls = 0
    init_errors = 0
    for k, tr in enumerate(traces(src)):
        if k < first or (opts["--count"] is not None and k >= first + int(opts["--count"])):
            continue
        if "init_error" in tr:
            if "init" in tr or tr["calls"]:
                raise SystemExit(f"trace({tr['n']}): init_error with an init or calls")
            (e,) = cmp(tr["init_error"], "init_error", 1)
            init_errors += 1
            entries.append(
                f"  \\* trace({tr['n']}): init_error\n  [cfg |-> {config(tr['config'])},\n   t0 |-> 0,\n"
                f"   err |-> \"{camel(e)}\",\n   init |-> {NO_STATE},\n   steps |-> <<>>]"
            )
            continue
        now0, init = cmp(tr["init"], "init", 2)
        steps = [call(c, ctx) for c in tr["calls"]]
        total_calls += len(steps)
        entries.append(
            f"  \\* trace({tr['n']})\n  [cfg |-> {config(tr['config'])},\n   t0 |-> {num(now0)},\n"
            f"   err |-> \"\",\n   init |-> {state(init, ctx)},\n   steps |-> <<\n     "
            + ",\n     ".join(steps)
            + ">>]"
        )
    if ctx.max_state_version > TLC_MAX // 2:
        raise SystemExit("state versions too large for the -1 abstraction")
    with open(out, "w") as fh:
        fh.write(f"---- MODULE {module} ----\nEXTENDS Integers\n")
        fh.write(f"\\* Generated by trace2tla.py from {src.split('/')[-1]}: {len(entries)} traces "
                 f"({init_errors} init_error), {total_calls} calls.\n")
        fh.write("Traces == <<\n" + ",\n".join(entries) + "\n>>\n====\n")
    print(
        f"trace2tla: {len(entries)} traces ({init_errors} init_error), {total_calls} calls; "
        f"{ctx.wrapped_versions} request versions >= 2^31 mapped to -1 (max state version {ctx.max_state_version})",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main(sys.argv)
