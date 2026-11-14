#!/usr/bin/env python3
"""Generates prolog/extracted/code_facts.pl from the pinned pool-core and shell.

PROP: C-1 C-2 C-3 C-4 (code side of the requirement checks)

Sources (relative to projects/shared-pool/):
  source-mirror/crates/core/src/lib.rs      per function: error sites, fallible
                                             calls, early Ok returns, caller
                                             comparisons, status tests and writes,
                                             the conditions of Full and Arithmetic ifs;
                                             role comparisons in Config::validate
  source-mirror/crates/contract/src/lib.rs  #[near] methods: attributes,
                                             receiver, core calls, caller source
  source-mirror/crates/core/src/fixtures.rs fixture names (order) and the
                                             account names of owner, actors, executor
  tla/SharedPool.tla (only if it exists)    GuardOrder -> extracted/tla_facts.pl
  tla/Props.tla      (if it exists)         LIFE-1 Edges -> extracted/tla_facts.pl

Nothing here is hand-maintained: the Prolog checks compare these facts with
the requirements transcribed from docs/spec.md and docs/api.md. The parser is
deliberately narrow; a construct it does not understand is an error, so a
code change that moves outside its grammar fails loudly instead of being
silently skipped.

usage: python3 -I extract.py [--check] [--core FILE] [--shell FILE] [--fixtures FILE] [--out FILE]
  --check   regenerate in memory and fail if the checked-in files differ
  --core/--shell/--out  used by run.sh for code mutants (temporary copies)
"""
import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PROJ = os.path.dirname(HERE)
CORE = os.path.join(PROJ, "source-mirror", "crates", "core", "src", "lib.rs")
SHELL = os.path.join(PROJ, "source-mirror", "crates", "contract", "src", "lib.rs")
FIXTURES = os.path.join(PROJ, "source-mirror", "crates", "core", "src", "fixtures.rs")
TLA = os.path.join(PROJ, "tla", "SharedPool.tla")
TLA_PROPS = os.path.join(PROJ, "tla", "Props.tla")
OUT = os.path.join(HERE, "extracted", "code_facts.pl")
TLA_OUT = os.path.join(HERE, "extracted", "tla_facts.pl")

# Internal functions whose Err propagates through `?` (or a tail call).
FALLIBLE = ["energy_terms", "check_bounds", "candidate", "validate", "envelope",
            "release", "energy", "record", "index", "bump_new", "bump", "add"]
STATUS_FNS = ["dispatch", "acknowledge", "cancel", "settle", "release", "submit"]


class ExtractError(Exception):
    pass


def snake(name):
    return re.sub(r"(?<!^)([A-Z])", r"_\1", name).lower()


def blank(src, keep_strings=False):
    """Replace comments (and string contents unless keep_strings) by spaces,
    keeping offsets and newlines, so positions map back to source lines."""
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            if not keep_strings:
                for k in range(i + 1, j):
                    if out[k] != "\n":
                        out[k] = " "
            i = j + 1
        elif c == "'" and i + 2 < n and (src[i + 2] == "'" or src[i + 1] == "\\"):
            # char literal ('x' or '\n'); lifetimes ('a) are left alone
            j = src.find("'", i + 1 if src[i + 1] != "\\" else i + 2)
            for k in range(i + 1, j):
                out[k] = " "
            i = j + 1
        else:
            i += 1
    return "".join(out)


def line_of(src, pos):
    return src.count("\n", 0, pos) + 1


def match_close(text, pos, open_ch, close_ch):
    """pos is at open_ch; returns the index of the matching close_ch."""
    depth = 0
    for i in range(pos, len(text)):
        if text[i] == open_ch:
            depth += 1
        elif text[i] == close_ch:
            depth -= 1
            if depth == 0:
                return i
    raise ExtractError(f"unbalanced {open_ch} at offset {pos}")


def functions(text):
    """Yields (name, fn_pos, params, body_start, body_end) for every fn."""
    for m in re.finditer(r"\bfn\s+(\w+)\s*(<[^>]*>)?\s*\(", text):
        lp = m.end() - 1
        rp = match_close(text, lp, "(", ")")
        lb = text.find("{", rp)
        semi = text.find(";", rp)
        if lb < 0 or (0 <= semi < lb):
            continue  # declaration without body
        rb = match_close(text, lb, "{", "}")
        yield m.group(1), m.start(), text[lp + 1:rp], lb, rb


def prev_word(text, pos):
    m = re.search(r"(\w+)\s*$", text[:pos])
    return m.group(1) if m else ""


# --------------------------------------------------------------------------
# Conditions over a record's status and the call's flags (C-3)

def tokenize(cond):
    toks = re.findall(r"matches!|&&|\|\||==|!=|!|\(|\)|,|\||[A-Za-z_][\w.]*(?:\[[^\]]*\])?(?:\.\w+)*(?:::\w+)?", cond)
    rebuilt = "".join(toks)
    if re.sub(r"\s+", "", cond) != rebuilt:
        raise ExtractError(f"cannot tokenize condition: {cond!r}")
    return toks


class CondParser:
    def __init__(self, cond):
        self.t = tokenize(cond)
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else None

    def eat(self, tok=None):
        t = self.peek()
        if tok is not None and t != tok:
            raise ExtractError(f"expected {tok!r}, got {t!r} in {' '.join(self.t)}")
        self.i += 1
        return t

    def parse(self):
        e = self.expr()
        if self.peek() is not None:
            raise ExtractError(f"trailing tokens in condition: {self.t[self.i:]}")
        return e

    def expr(self):
        e = self.term()
        while self.peek() == "||":
            self.eat()
            e = f"or({e}, {self.term()})"
        return e

    def term(self):
        e = self.factor()
        while self.peek() == "&&":
            self.eat()
            e = f"and({e}, {self.factor()})"
        return e

    def factor(self):
        t = self.peek()
        if t == "!":
            self.eat()
            return f"not({self.factor()})"
        if t == "(":
            self.eat()
            e = self.expr()
            self.eat(")")
            return e
        if t == "matches!":
            self.eat()
            self.eat("(")
            self.status_path(self.eat())
            self.eat(",")
            alts = [self.status_value(self.eat())]
            while self.peek() == "|":
                self.eat()
                alts.append(self.status_value(self.eat()))
            self.eat(")")
            return f"in([{', '.join(alts)}])"
        lhs = self.eat()
        if self.peek() in ("==", "!="):
            op = self.eat()
            self.status_path(lhs)
            rhs = self.eat()
            val = "next" if rhs == "next" else self.status_value(rhs)
            return f"{'eq' if op == '==' else 'ne'}({val})"
        if lhs in ("completed", "accepted"):
            return f"flag({lhs})"
        raise ExtractError(f"unsupported condition atom {lhs!r}")

    @staticmethod
    def status_path(tok):
        if not re.fullmatch(r"(c|self\.records\[id as usize\])\.status|c\.status", tok.replace(" ", "")) and \
                tok not in ("c.status",) and not tok.endswith(".status"):
            raise ExtractError(f"unsupported status path {tok!r}")

    @staticmethod
    def status_value(tok):
        m = re.fullmatch(r"Status::(\w+)", tok)
        if not m:
            raise ExtractError(f"unsupported status value {tok!r}")
        return snake(m.group(1))


def parse_cond(cond):
    # `self.records[id as usize].status` contains spaces: normalise it first.
    cond = re.sub(r"self\.records\[id as usize\]\.status", "c.status", cond)
    return CondParser(cond).parse()


# --------------------------------------------------------------------------
# Conditions of the Full and version-cap guards (C-2). A condition is split at
# its top-level `&&`; one with a top-level `||` stays whole. Each conjunct
# over the pool's window or version becomes a term; any other conjunct is
# kept as text, so a changed condition is visible to the comparison.

def conjuncts(cond):
    parts, depth, start, i = [], 0, 0, 0
    has_or = False
    while i < len(cond):
        ch = cond[i]
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif depth == 0 and cond.startswith("&&", i):
            parts.append(cond[start:i].strip())
            i += 2
            start = i
            continue
        elif depth == 0 and cond.startswith("||", i):
            has_or = True
        i += 1
    parts.append(cond[start:].strip())
    return [cond.strip()] if has_or else parts


def cond_atom(c):
    s = re.sub(r"\s+", "", c)
    cmp_name = {">=": "ge", ">": "gt"}
    if (m := re.fullmatch(r"self\.records\.len\(\)(>=|>)(\w+)", s)):
        return f"{cmp_name[m.group(1)]}(retained, '{m.group(2)}')"
    if (m := re.fullmatch(r"self\.version(>=|>)(\w+)", s)):
        return f"{cmp_name[m.group(1)]}(version, '{m.group(2)}')"
    if s == "open(&self.records[0].status)":
        return "open(front)"
    return "text('" + c.replace("\\", "\\\\").replace("'", "\\'") + "')"


# --------------------------------------------------------------------------
# Constant expressions (v1.1.0: `pub const VERSION_CAP: u32 = u32::MAX - 3 *
# MAX_RECORDS as u32;`). Integer literals, earlier constants, `T::MAX`/`T::MIN`,
# `+ - *`, parentheses and `as T` (binding tighter than `*`, as in Rust); every
# intermediate and the result must fit its type, as the compiler requires.

INT_RANGE = {"u32": (0, 2**32 - 1), "u64": (0, 2**64 - 1), "usize": (0, 2**64 - 1),
             "i64": (-2**63, 2**63 - 1), "i32": (-2**31, 2**31 - 1)}


def eval_const(expr, consts, name):
    toks = re.findall(r"\d[\d_]*(?:u32|u64|i64|usize)?|\w+::\w+|\w+|[-+*()]|\S", expr)
    if re.sub(r"\s+", "", expr) != "".join(toks):
        raise ExtractError(f"constant {name}: cannot tokenize {expr!r}")
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def eat():
        t = peek()
        pos[0] += 1
        return t

    def atom():
        t = eat()
        if t == "(":
            v = add()
            if eat() != ")":
                raise ExtractError(f"constant {name}: unbalanced parentheses in {expr!r}")
            return v
        if t is not None and re.fullmatch(r"\d[\d_]*(?:u32|u64|i64|usize)?", t):
            return int(re.sub(r"(u32|u64|i64|usize)$", "", t).replace("_", ""))
        if t is not None and (m := re.fullmatch(r"(\w+)::(MAX|MIN)", t)) and m.group(1) in INT_RANGE:
            lo, hi = INT_RANGE[m.group(1)]
            return hi if m.group(2) == "MAX" else lo
        if t in consts:
            return consts[t]
        raise ExtractError(f"constant {name}: unsupported term {t!r} in {expr!r}")

    def cast():
        v = atom()
        while peek() == "as":
            eat()
            ty = eat()
            if ty not in INT_RANGE or not INT_RANGE[ty][0] <= v <= INT_RANGE[ty][1]:
                raise ExtractError(f"constant {name}: {v} as {ty} does not fit")
        return v

    def mul():
        v = cast()
        while peek() == "*":
            eat()
            v = v * cast()
        return v

    def add():
        v = mul()
        while peek() in ("+", "-"):
            op = eat()
            v = v + mul() if op == "+" else v - mul()
        return v

    v = add()
    if peek() is not None:
        raise ExtractError(f"constant {name}: trailing tokens in {expr!r}")
    return v


# --------------------------------------------------------------------------
# Core

def core_facts(raw):
    text = blank(raw)
    facts = []
    fns = {}
    for name, fpos, params, lb, rb in functions(text):
        fns.setdefault(name, (fpos, params, lb, rb))

    # Constants (literals or constant expressions, evaluated)
    consts = {}
    for m in re.finditer(r"\bconst (\w+): (\w+) = ([^;]+);", text):
        ty = m.group(2)
        if ty not in INT_RANGE:
            raise ExtractError(f"constant {m.group(1)}: unsupported type {ty}")
        v = eval_const(m.group(3), consts, m.group(1))
        if not INT_RANGE[ty][0] <= v <= INT_RANGE[ty][1]:
            raise ExtractError(f"constant {m.group(1)} = {v} does not fit {ty}")
        consts[m.group(1)] = v
        if not text[:m.start()].rstrip().endswith("pub"):
            raise ExtractError(f"constant {m.group(1)} is not pub")
        facts.append(f"code_constant('{m.group(1)}', {v}, {line_of(text, m.start())}).")

    # Enums (data! macro)
    for enum in ("Error", "Status", "Shape"):
        m = re.search(r"pub enum %s \{([^}]*)\}" % enum, text)
        if not m:
            raise ExtractError(f"enum {enum} not found")
        vs = [v.strip() for v in m.group(1).split(",") if v.strip()]
        facts.append(f"code_enum('{enum}', [{', '.join(repr(v) for v in vs)}], {line_of(text, m.start())}).")

    # Struct field types (data! macro): integer widths matter for C-4.
    for m in re.finditer(r"pub struct (\w+) \{([^}]*)\}", text):
        for fm in re.finditer(r"pub (\w+): ([\w<>()]+)", m.group(2)):
            if fm.group(2) in ("u32", "u64", "i64", "bool"):
                facts.append(f"code_field('{m.group(1)}', {fm.group(1)}, {fm.group(2)}).")
            elif (vm := re.fullmatch(r"Vec<(u32|u64|i64)>", fm.group(2))):
                facts.append(f"code_field('{m.group(1)}', {fm.group(1)}, vec({vm.group(1)})).")

    # Role distinctness in Config::validate (F-4): comparisons between two of
    # a recipe's actor/executor and the owner. Always emitted (an empty list is
    # the finding: validate lets one account hold two roles).
    if "validate" not in fns:
        raise ExtractError("Config::validate not found")
    _, _, vlb, vrb = fns["validate"]
    vbody = text[vlb + 1:vrb]
    role = {"r.actor": "actor", "r.executor": "executor", "self.owner": "owner"}
    pairs = []
    for m in re.finditer(r"(r\.actor|r\.executor|self\.owner)\s*(==|!=)\s*(r\.actor|r\.executor|self\.owner)\b", vbody):
        a, b = sorted([role[m.group(1)], role[m.group(3)]])
        pairs.append(f"{a}-{b}")
    facts.append(f"code_role_checks(validate, [{', '.join(sorted(set(pairs)))}]).")

    order = sorted(fns.items(), key=lambda kv: kv[1][0])
    for name, (fpos, params, lb, rb) in order:
        body = text[lb + 1:rb]
        base = lb + 1
        recv = "mut" if "&mut self" in params else ("ref" if "&self" in params else "none")
        facts.append(f"code_fn({name}, {line_of(text, fpos)}, {recv}).")
        events = []  # (sort position, term)
        # error sites
        for m in re.finditer(r"Error::(\w+)", body):
            events.append((m.start(), f"e('{m.group(1)}', {line_of(text, base + m.start())})"))
        # fallible calls
        for m in re.finditer(r"(?<![\w.])(?:self\.|config\.)?(%s)\s*\(" % "|".join(FALLIBLE), body):
            callee = m.group(1)
            # skip the method-call form on other receivers (e.g. x.checked_add) and definitions
            pre = body[:m.start()]
            if re.search(r"\bfn\s*$", pre):
                continue
            lp = m.end() - 1
            rp = match_close(body, lp, "(", ")")
            after = body[rp + 1:].lstrip()
            if after.startswith("?"):
                qpos = body.index("?", rp)
                events.append((qpos, f"c({callee}, {line_of(text, base + m.start())})"))
            elif prev_word(body, m.start()) == "return" or after == "" :
                events.append((m.start(), f"t({callee}, {line_of(text, base + m.start())})"))
            # otherwise the result is captured (.ok(), let x = ...): not a guard
        # early Ok returns
        for m in re.finditer(r"\breturn\s+Ok\s*\(", body):
            events.append((m.start(), f"noop({line_of(text, base + m.start())})"))
        events.sort(key=lambda e: e[0])
        facts.append(f"code_events({name}, [{', '.join(t for _, t in events)}]).")

        # checked increments of a Pool field: `self.<field>.checked_add(..)`
        # in the statement of an Error::Arithmetic site (C-2 discharges the
        # site by the invariant that bounds the field).
        for m in re.finditer(r"Error::Arithmetic", body):
            st = max(body.rfind(";", 0, m.start()), body.rfind("{", 0, m.start()),
                     body.rfind("}", 0, m.start())) + 1
            for fm in re.finditer(r"\bself\s*\.\s*(\w+)\s*\.\s*checked_add\s*\(", body[st:m.start()]):
                facts.append(f"code_checked_add({name}, {line_of(text, base + m.start())}, {fm.group(1)}).")

        # Conditions of the Full and Arithmetic `if` guards, as the list of the
        # condition's top-level `&&` conjuncts, at the line of the Error site:
        #   if <cond> { return Err(Error::Full|Arithmetic); }
        # C-2 compares the record bound (Full) and the version cap with the
        # docs' conditions (pool_req guard_condition/4).
        for m in re.finditer(r"\bif\s+((?:(?!\bif\b)[^{;])*?)\{\s*return\s+Err\((Error::(Full|Arithmetic))\)\s*;\s*\}", body, re.S):
            conjs = [cond_atom(c) for c in conjuncts(" ".join(m.group(1).split()))]
            facts.append(f"code_error_cond({name}, {line_of(text, base + m.start(2))}, '{m.group(3)}', [{', '.join(conjs)}]).")

        # caller comparisons: if <cond on caller> { return Err(Error::X); }
        for m in re.finditer(r"\bif\s+((?:(?!\bif\b)[^{;])*?\bcaller\b[^{]*?)\{\s*return\s+Err\(Error::(\w+)\)", body, re.S):
            cond = " ".join(m.group(1).split())
            parts = re.split(r"\s*(&&|\|\|)\s*", cond)
            cmps = parts[0::2]
            conns = set(parts[1::2])
            allowed = []
            for c in cmps:
                cm = re.fullmatch(r"caller\s*!=\s*([\w.]+)", c)
                if not cm:
                    raise ExtractError(f"{name}: unsupported caller test {c!r}")
                who = {"r.actor": "actor", "r.executor": "executor", "self.config.owner": "owner"}.get(cm.group(1))
                if who is None:
                    raise ExtractError(f"{name}: unknown principal {cm.group(1)!r}")
                allowed.append(who)
            conn = "and" if conns <= {"&&"} else "mixed"
            facts.append(f"code_caller_check({name}, {line_of(text, base + m.start())}, {conn}, [{', '.join(allowed)}], '{m.group(2)}').")

        # integer literals other than 0 and 1 (code-only constants)
        for m in re.finditer(r"(?<![\w.])(\d[\d_]*)(?:u64|u32|i64|usize)?(?![\w.])", body):
            v = int(m.group(1).replace("_", ""))
            if v > 1:
                facts.append(f"code_literal({name}, {v}, {line_of(text, base + m.start())}).")

        if name in STATUS_FNS:
            # status tests: if <cond> { return <outcome>; }
            for m in re.finditer(r"\bif\s+((?:(?!\bif\b)[^{;])*?)\{\s*return\s+([^;]*?);\s*\}", body, re.S):
                cond = " ".join(m.group(1).split())
                if not re.search(r"\bstatus\b|\bcompleted\b|\baccepted\b", cond):
                    continue
                ret = " ".join(m.group(2).split())
                if ret.startswith("Ok("):
                    outcome = "noop"
                elif (em := re.fullmatch(r"Err\(Error::(\w+)\)", ret)):
                    outcome = f"err('{em.group(1)}')"
                elif (tm := re.fullmatch(r"self\.(\w+)\(id\)", ret)):
                    outcome = f"tail({tm.group(1)})"
                else:
                    raise ExtractError(f"{name}: unsupported return {ret!r}")
                facts.append(f"code_status_guard({name}, {line_of(text, base + m.start())}, {parse_cond(cond)}, {outcome}).")
            # status selection and writes
            for m in re.finditer(r"let next = if (\w+) \{\s*Status::(\w+)\s*\} else \{\s*Status::(\w+)\s*\};", body):
                facts.append(f"code_status_next({name}, {line_of(text, base + m.start())}, {m.group(1)}, {snake(m.group(2))}, {snake(m.group(3))}).")
            for m in re.finditer(r"(?:\.status\s*=|\bstatus\s*:)\s*(Status::(\w+)|next)\s*[;,]", body):
                what = "next" if m.group(1) == "next" else snake(m.group(2))
                facts.append(f"code_status_write({name}, {line_of(text, base + m.start())}, {what}).")
    return facts


# --------------------------------------------------------------------------
# Shell

def shell_facts(raw):
    text = blank(raw)
    keep = blank(raw, keep_strings=True)
    facts = []
    m = re.search(r"#\[near\]\s*impl Contract \{", text)
    if not m:
        raise ExtractError("#[near] impl Contract not found")
    lb = text.index("{", m.start())
    rb = match_close(text, lb, "{", "}")
    block = text[lb + 1:rb]
    base = lb + 1
    for name, fpos, params, flb, frb in functions(block):
        # attributes between the previous item and this fn
        prev_end = max(block.rfind("}", 0, fpos), block.rfind("{", 0, fpos), 0)
        attrs = re.findall(r"#\[(\w+)\]", block[prev_end:fpos])
        attrs = sorted(a for a in attrs if a != "allow")
        recv = "mut" if "&mut self" in params else ("ref" if "&self" in params else "none")
        body = block[flb + 1:frb]
        bbase = base + flb + 1
        facts.append(f"shell_method({name}, {line_of(text, base + fpos)}, [{', '.join(attrs)}], {recv}).")
        for cm in re.finditer(r"(?:self\.pool\.|Pool::)(\w+)\s*\(", body):
            lp = cm.end() - 1
            rp = match_close(body, lp, "(", ")")
            args = " ".join(body[lp + 1:rp].split())
            first = args.split(",")[0].strip()
            src = {"&caller()": "predecessor", "actor.as_str()": "argument(actor)",
                   "&executor": "recipe_executor"}.get(first, "none")
            if src == "recipe_executor" and not re.search(r"let executor = [\s\S]*?\.executor\.clone\(\)", body):
                src = "unknown"
            facts.append(f"shell_core_call({name}, {cm.group(1)}, {src}, {line_of(text, bbase + cm.start())}).")
        for lm in re.finditer(r"\.min\((\d+)\)", body):
            facts.append(f"shell_literal({name}, {lm.group(1)}, {line_of(text, bbase + lm.start())}).")
    # helper functions outside the #[near] impl
    for name, fpos, params, flb, frb in functions(keep):
        if name not in ("caller", "now", "must"):
            continue
        body = " ".join(keep[flb + 1:frb].split())
        if name == "caller":
            ok = "env::predecessor_account_id()" in body
            facts.append(f"shell_helper(caller, {'predecessor_account_id' if ok else 'other'}, {line_of(keep, fpos)}).")
        elif name == "now":
            mm = re.search(r"env::block_timestamp\(\) / ([\d_]+)", body)
            div = int(mm.group(1).replace("_", "")) if mm else 0
            facts.append(f"shell_helper(now, block_timestamp_div({div}), {line_of(keep, fpos)}).")
        elif name == "must":
            ok = "env::panic_str(&e.to_string())" in body
            facts.append(f"shell_helper(must, {'panic_with_error_string' if ok else 'other'}, {line_of(keep, fpos)}).")
    return facts


def split_args(s):
    """Top-level comma split of a call's argument text."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def fixture_facts(raw):
    text = blank(raw, keep_strings=True)
    facts = []
    m = re.search(r'pub const NAMES: \[&str; \d+\] = \[([^\]]*)\];', text)
    if not m:
        raise ExtractError("fixtures::NAMES not found")
    names = re.findall(r'"(\w+)"', m.group(1))
    facts.append(f"fixture_names([{', '.join(names)}], {line_of(text, m.start())}).")
    fns = {n: (lb, rb) for n, _, _, lb, rb in functions(text)}
    lb, rb = fns["recipe"]
    rbody = text[lb + 1:rb]
    am = re.search(r'actor: format!\("\{actor\}([^"]*)"\)', rbody)
    xm = re.search(r'executor: "([^"]*)"\.into\(\)', rbody)
    if not am or not xm:
        raise ExtractError("fixtures::recipe: actor/executor pattern not found")
    suffix = am.group(1)
    facts.append(f"fixture_executor('{xm.group(1)}', {line_of(text, lb + 1 + xm.start())}).")
    lb, rb = fns["config"]
    cbody = text[lb + 1:rb]
    om = re.search(r'owner: "([^"]*)"\.into\(\)', cbody)
    if not om:
        raise ExtractError("fixtures::config: owner not found")
    facts.append(f"fixture_owner('{om.group(1)}', {line_of(text, lb + 1 + om.start())}).")
    arms = list(re.finditer(r'((?:"\w+"\s*\|\s*)*"\w+")\s*=>\s*\{', cbody))
    arm_actors, derived = {}, []
    for i, arm in enumerate(arms):
        alb = cbody.index("{", arm.end() - 1)
        arb = match_close(cbody, alb, "{", "}")
        body = cbody[alb + 1:arb]
        loops = {v: re.findall(r'"([^"]*)"', lst)
                 for v, lst in re.findall(r"for \(\w+, (\w+)\) in \[([^\]]*)\]", body)}
        # v1.1.0: an arm may start from another fixture (`c = config("gpu")?;`,
        # the station) and rename its recipes' actors with
        # `r.actor = format!("{}<suffix>", <loop var>.to_lowercase());` over a
        # literal list. Its actors are then a superset: the base fixture's
        # actors plus the renamed ones (fixture_actors is used as "every
        # recipe's actor is among these"). Any other actor write is an error.
        dm = re.search(r'\bc = config\("(\w+)"\)\?;', body)
        renamed = set()
        for am2 in re.finditer(r"\.actor\s*=\s*([^;]*);", body):
            fm2 = re.fullmatch(r'format!\("\{\}([^"]*)",\s*(\w+)\.to_lowercase\(\)\)', " ".join(am2.group(1).split()))
            if not dm or not fm2 or fm2.group(2) not in loops:
                raise ExtractError(f"fixtures: unsupported actor assignment {am2.group(0)!r}")
            renamed.update(x.lower() + fm2.group(1) for x in loops[fm2.group(2)])
        if dm:
            if re.search(r"(?<![\w.])recipe\(|c\.recipes\s*=", body):
                raise ExtractError(f"fixtures: derived arm {arm.group(1)} also builds recipes")
            for nm in re.findall(r'"(\w+)"', arm.group(1)):
                derived.append((nm, dm.group(1), line_of(text, lb + 1 + alb + 1 + dm.start()), renamed))
            continue
        actors, ordered = set(), []
        for cm in re.finditer(r"(?<![\w.])recipe\(", body):
            lp = cm.end() - 1
            args = split_args(body[lp + 1:match_close(body, lp, "(", ")")])
            a = args[1]
            if a.startswith('"'):
                actors.add(a.strip('"') + suffix)
                ordered.append(a.strip('"'))
            elif a in loops:
                actors.update(x + suffix for x in loops[a])
                ordered = None if ordered is None else ordered + [None]
            else:
                raise ExtractError(f"fixtures: unresolved actor argument {a!r}")
        if not actors:
            raise ExtractError(f"fixtures: arm {arm.group(1)} has no recipe() call and no derivation")
        for nm in re.findall(r'"(\w+)"', arm.group(1)):
            arm_actors[nm] = actors
            facts.append(f"fixture_actors({nm}, [{', '.join(repr(x) for x in sorted(actors))}]).")
            # Recipe order by actor, only where the recipes are a literal list
            # (no loop): the recipe's index is its position here.
            lit = re.search(r"c\.recipes = vec!\[", body)
            if lit:
                lb2 = lit.end() - 1
                inner = body[lb2 + 1:match_close(body, lb2, "[", "]")]
                lits = []
                for cm in re.finditer(r"(?<![\w.])recipe\(", inner):
                    lp = cm.end() - 1
                    a = split_args(inner[lp + 1:match_close(inner, lp, "(", ")")])[1]
                    if not a.startswith('"'):
                        raise ExtractError(f"fixtures: non-literal actor {a!r} in a recipe list")
                    lits.append(a.strip('"'))
                if len(lits) != len(ordered):
                    raise ExtractError(f"fixtures: {nm}: recipes outside the literal list")
                facts.append(f"fixture_recipe_actors({nm}, [{', '.join(repr(x) for x in lits)}]).")
    for nm, basename, line, renamed in derived:
        if basename not in arm_actors:
            raise ExtractError(f"fixtures: {nm} derives from {basename!r}, which builds no recipes")
        facts.append(f"fixture_derived({nm}, {basename}, {line}).")
        facts.append(f"fixture_actors({nm}, [{', '.join(repr(x) for x in sorted(arm_actors[basename] | renamed))}]).")
    return facts


def tla_facts(raw, props_raw):
    m = re.search(r"GuardOrder\s*==\s*\[(.*?)\n\s*\]", raw, re.S)
    if not m:
        return None
    out = []
    for op, seq in re.findall(r"(\w+)\s*\|->\s*<<(.*?)>>", m.group(1), re.S):
        errs = re.findall(r'"(\w+)"', seq)
        out.append(f"tla_guard_order({op}, [{', '.join(repr(e) for e in errs)}]).")
    # LIFE-1's status graph (Props.tla `Edges ==`): explicit pairs plus at most
    # one comprehension { <<s, t>> : s \in {..}, t \in {..} }.
    if props_raw is not None:
        em = re.search(r"\nEdges\s*==(.*?)\n(?=\S)", props_raw, re.S)
        if em:
            body = em.group(1)
            edges = set()
            comp = re.search(r"\{\s*<<\s*(\w+)\s*,\s*(\w+)\s*>>\s*:\s*(\w+)\s*\\in\s*\{([^}]*)\}\s*,\s*(\w+)\s*\\in\s*\{([^}]*)\}\s*\}", body)
            if comp:
                xs = re.findall(r'"(\w+)"', comp.group(4))
                ys = re.findall(r'"(\w+)"', comp.group(6))
                if (comp.group(1), comp.group(2)) != (comp.group(3), comp.group(5)):
                    raise ExtractError("Props.tla Edges: unexpected comprehension shape")
                edges |= {(x, y) for x in xs for y in ys}
                body = body[:comp.start()] + body[comp.end():]
            edges |= set(re.findall(r'<<\s*"(\w+)"\s*,\s*"(\w+)"\s*>>', body))
            for a, b in sorted(edges):
                out.append(f"tla_status_edge({snake(a)}, {snake(b)}).")
    return out


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main(argv):
    def opt(flag, default):
        return argv[argv.index(flag) + 1] if flag in argv else default
    core, shell, out = opt("--core", CORE), opt("--shell", SHELL), opt("--out", OUT)
    fixtures = opt("--fixtures", FIXTURES)
    rel = lambda p: os.path.relpath(p, PROJ) if p.startswith(PROJ) else os.path.basename(p)
    lines = ["%% GENERATED by prolog/extract.py (python3 -I extract.py). Do not edit.",
             f"%% core:  {rel(core)} sha256 {sha(core)}",
             f"%% shell: {rel(shell)} sha256 {sha(shell)}",
             f"%% fixtures: {rel(fixtures)} sha256 {sha(fixtures)}",
             "%% PROP: C-1 C-2 C-3 C-4",
             ":- module(code_facts, [code_constant/3, code_enum/3, code_fn/3, code_events/2,",
             "    code_caller_check/5, code_literal/3, code_status_guard/4, code_status_next/5,",
             "    code_status_write/3, code_field/3, code_role_checks/2, shell_method/4, shell_core_call/4, shell_literal/3, shell_helper/3,",
             "    fixture_names/2, fixture_executor/2, fixture_owner/2, fixture_actors/2, fixture_recipe_actors/2,",
             "    code_checked_add/3, code_error_cond/4, fixture_derived/3]).",
             ":- discontiguous(code_fn/3).", ":- discontiguous(code_events/2).",
             ":- discontiguous(code_caller_check/5).", ":- discontiguous(code_literal/3).",
             ":- discontiguous(code_status_guard/4).", ":- discontiguous(code_status_next/5).",
             ":- discontiguous(code_status_write/3).", ":- discontiguous(shell_core_call/4).",
             ":- discontiguous(shell_method/4).", ":- discontiguous(shell_literal/3).",
             ":- discontiguous(fixture_actors/2).", ":- discontiguous(fixture_recipe_actors/2).",
             ":- discontiguous(code_checked_add/3).", ":- discontiguous(code_error_cond/4).",
             ":- dynamic(code_error_cond/4).", ":- dynamic(fixture_derived/3).", ""]
    try:
        lines += core_facts(open(core).read())
        lines += shell_facts(open(shell).read())
        lines += fixture_facts(open(fixtures).read())
    except ExtractError as e:
        sys.exit(f"extract.py: {e}")
    text = "\n".join(lines) + "\n"
    outputs = {out: text}
    if out == OUT and not os.path.exists(TLA):
        outputs[TLA_OUT] = "\n".join(
            ["%% GENERATED by prolog/extract.py: tla/SharedPool.tla does not exist,",
             "%% so C-2/C-3's TLA legs are a recorded gap. Do not edit.",
             "%% PROP: C-2 C-3",
             ":- module(tla_facts, [tla_guard_order/2, tla_status_edge/2, tla_source/1]).",
             ":- dynamic(tla_guard_order/2).", ":- dynamic(tla_status_edge/2).", "",
             "tla_source(absent)."]) + "\n"
    if out == OUT and os.path.exists(TLA):
        props_raw = open(TLA_PROPS).read() if os.path.exists(TLA_PROPS) else None
        tf = tla_facts(open(TLA).read(), props_raw)
        if tf is None:
            sys.exit("extract.py: tla/SharedPool.tla has no GuardOrder")
        else:
            hdr = [f"%% tla: {rel(TLA)} sha256 {sha(TLA)}"]
            if props_raw is not None:
                hdr.append(f"%% tla props: {rel(TLA_PROPS)} sha256 {sha(TLA_PROPS)}")
            outputs[TLA_OUT] = "\n".join(
                ["%% GENERATED by prolog/extract.py from GuardOrder in tla/SharedPool.tla",
                 "%% and Edges in tla/Props.tla. Do not edit."] + hdr +
                ["%% PROP: C-2 C-3",
                 ":- module(tla_facts, [tla_guard_order/2, tla_status_edge/2, tla_source/1]).",
                 ":- dynamic(tla_guard_order/2).", ":- dynamic(tla_status_edge/2).", "",
                 "tla_source(present)."] + tf) + "\n"
    if "--check" in argv:
        bad = [p for p, t in outputs.items() if not os.path.exists(p) or open(p).read() != t]
        if bad:
            sys.exit("extract.py --check: stale: " + ", ".join(bad) + " (run python3 -I prolog/extract.py)")
        print("extract.py --check: extracted facts are current")
        return
    for p, t in outputs.items():
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, "w").write(t)
        print(f"wrote {rel(p)}")


if __name__ == "__main__":
    main(sys.argv[1:])
