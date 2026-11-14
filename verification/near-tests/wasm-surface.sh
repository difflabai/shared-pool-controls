#!/usr/bin/env bash
# Export and import surface of the shared-pool wasm (pool_contract and
# pool_executor) in the documented build, the one the near-tests use:
#   build  upstream scripts/build.sh: RUSTFLAGS="--cfg near" (and no
#          CARGO_ENCODED_RUSTFLAGS), cargo build -p pool-contract
#          -p pool-executor --target wasm32-unknown-unknown --release
#          --locked, from the source-mirror root (its rust-toolchain.toml).
#          This is near-tests' Build::Near; SURF-3(a) checks the bytes.
#
# usage: projects/shared-pool/near-tests/wasm-surface.sh [--update]
#   --update   rewrite projects/shared-pool/wasm-surface.expected from this
#              build instead of comparing against it (SURF-2 and SURF-3
#              still run).
#   POOL_WASM_TARGET_DIR  build directory (default near-tests/target/wasm,
#              shared with the tests: `near` and, for SURF-3(b), `plain`).
#
# Each import line carries a class:
#   promise    promise_* host functions (the shell's receipts)
#   forbidden  build.sh's FORBIDDEN names (deploy, delete_account, add_key,
#              delete_key, gas_key, transfer, stake, create_account,
#              global_contract) and every promise_batch_* function
#   -          everything else
#
# Checks (SURF-n are local names; each tag line names the catalogue IDs the
# check bears on):
#   SURF-1  the surface (exports, imports, classes) == wasm-surface.expected.
#           Pins the exports to the documented methods plus
#           contract_source_metadata (no upgrade, migrate or reset export:
#           SH-11; it bears on ASM-DEPLOY), and records that the build
#           imports panic_utf8, so its panics carry their message (SH-5,
#           NT-F1 fixed in v1.1.0).
#           PROP: SH-5 SH-11 ASM-DEPLOY
#   SURF-2  no import is `forbidden`: the contracts cannot deploy code,
#           delete the account, add or delete keys, transfer, stake or
#           create accounts (the in-contract half of ASM-DEPLOY; NT-F3 fixed
#           in v1.1.0: low-level promise functions only).
#           PROP: SH-12 ASM-DEPLOY
#   SURF-3  build.sh itself.
#           (a) The real scripts/build.sh, run in source-mirror with
#           CARGO_TARGET_DIR, RUSTFLAGS and CARGO_ENCODED_RUSTFLAGS unset,
#           passes its own checker. Its artifacts/build.json has dirty
#           false, source_commit = the pinned commit (the superproject's
#           gitlink) and rustflags "--cfg near", and both sha256 values equal
#           this script's build (the bytes the near-tests run). Each wasm's
#           import set, read by this script's parser, equals its ALLOWED list
#           parsed from build.sh's text (18 and 16 names): no extra and no
#           unused entry. build.sh's FORBIDDEN list equals this script's.
#           (b) Negative controls: build.sh's checker (its heredoc, run with
#           python3 -I in a scratch directory) exits 1 with its message on
#           (i) the wasm built without --cfg near ("does not import
#           env.panic_utf8"), (ii) a synthetic module importing
#           env.promise_batch_action_transfer ("imports host functions for
#           actions other than function calls"), (iii) a synthetic module
#           importing env.promise_yield_create ("has imports that are not in
#           the reviewed list").
#           PROP: SH-5 SH-12
# Exit: 0 all hold; 1 a check fails; 2 a build fails.
# The parser reads only the import (2) and export (7) sections; python3 only.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

here=$(cd "$(dirname "$0")" && pwd)
proj=$(cd "$here/.." && pwd)
src="$proj/source-mirror"
expected="$proj/wasm-surface.expected"
update=0
[ "${1:-}" = --update ] && update=1

work=$(mktemp -d "${TMPDIR:-/tmp}/pool-surface.XXXXXX")
trap 'rm -rf "$work"' EXIT
target=${POOL_WASM_TARGET_DIR:-$here/target/wasm}
rustc_v=$(cd "$src" && env -u RUSTUP_TOOLCHAIN rustc --version)
pin=$(git -C "$proj" ls-files -s source-mirror | awk '{print $2}')
[ -n "$pin" ] || { echo "no source-mirror gitlink in the superproject index" >&2; exit 2; }

# The tests' build command (common::wasm_dir): `near` is build.sh's flags,
# `plain` (no --cfg near) is only SURF-3(b)'s negative control.
build() {
  local b=$1 flags=""
  [ "$b" = near ] && flags="--cfg near"
  (cd "$src" && env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS -u RUSTUP_TOOLCHAIN RUSTFLAGS="$flags" CARGO_TARGET_DIR="$target/$b" \
    cargo build --manifest-path "$src/Cargo.toml" --target wasm32-unknown-unknown --release --locked -p pool-contract -p pool-executor) \
    >"$work/build-$b.log" 2>&1 || { tail -n 30 "$work/build-$b.log"; echo "wasm build ($b) failed" >&2; exit 2; }
}
build near
build plain
wasm_of() { echo "$target/$1/wasm32-unknown-unknown/release/$2.wasm"; }

cat >"$work/surface.py" <<'PY'
import sys
# scripts/build.sh's FORBIDDEN list (SURF-3(a) checks they are equal) and
# every promise_batch_* function.
FORBIDDEN = ["deploy", "delete_account", "add_key", "delete_key", "gas_key", "transfer", "stake", "create_account", "global_contract"]
if sys.argv[1] == "--forbidden":
    print(" ".join(FORBIDDEN))
    sys.exit(0)
path, prefix = sys.argv[1], sys.argv[2]
data = open(path, "rb").read()
if data[:4] != b"\0asm" or data[4:8] != b"\x01\0\0\0":
    sys.exit("not a wasm v1 module: " + path)
def leb(buf, i):
    result = shift = 0
    while True:
        b = buf[i]; i += 1
        result |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return result, i
def name(buf, i):
    n, i = leb(buf, i)
    return buf[i:i + n].decode("utf-8"), i + n
def limits(buf, i):
    flags = buf[i]; i += 1
    _, i = leb(buf, i)
    if flags & 1:
        _, i = leb(buf, i)
    return i
KIND = {0: "func", 1: "table", 2: "memory", 3: "global", 4: "tag"}
imports, exports = [], []
i = 8
while i < len(data):
    sid = data[i]; i += 1
    size, i = leb(data, i)
    body, i = data[i:i + size], i + size
    if sid == 2:
        n, j = leb(body, 0)
        for _ in range(n):
            mod, j = name(body, j)
            field, j = name(body, j)
            kind = body[j]; j += 1
            if kind == 0:
                _, j = leb(body, j)
            elif kind == 1:
                j = limits(body, j + 1)
            elif kind == 2:
                j = limits(body, j)
            elif kind == 3:
                j += 2
            elif kind == 4:
                _, j = leb(body, j + 1)
            else:
                sys.exit("unknown import kind %d" % kind)
            imports.append((KIND[kind], mod, field))
    elif sid == 7:
        n, j = leb(body, 0)
        for _ in range(n):
            nm, j = name(body, j)
            kind = body[j]; j += 1
            _, j = leb(body, j)
            exports.append((KIND.get(kind, str(kind)), nm))
def cls(field):
    if field.startswith("promise_batch_") or any(w in field for w in FORBIDDEN):
        return "forbidden"
    if field.startswith("promise_"):
        return "promise"
    return "-"
lines = sorted("%s export %s %s" % (prefix, k, n) for (k, n) in exports)
lines += sorted("%s import %s %s.%s %s" % (prefix, k, m, f, cls(f)) for (k, m, f) in imports)
print("\n".join(lines))
PY

: >"$work/surface"
for w in pool_contract pool_executor; do
  f=$(wasm_of near $w)
  echo "build $w sha256 $(shasum -a 256 "$f" | cut -d' ' -f1) ($(wc -c <"$f" | tr -d ' ') bytes)"
  python3 -I "$work/surface.py" "$f" "build $w" >>"$work/surface"
done
echo
for w in pool_contract pool_executor; do
  sel() { grep "^build $w import .* $1\$" "$work/surface" | awk '{print $5}' | sed 's/^env\.//' | tr '\n' ' '; }
  echo "build $w: $(grep -c "^build $w export" "$work/surface") exports, $(grep -c "^build $w import" "$work/surface") imports"
  echo "  promise:   $(sel promise)"
  echo "  forbidden: $(sel forbidden)"
  grep -q "^build $w import func env.panic_utf8 " "$work/surface" && p=yes || p=no
  echo "  panic_utf8 imported: $p"
done
echo

status=0
if [ $update -eq 1 ]; then
  {
    echo "# wasm-surface.expected -- exports and imports of the pinned shared-pool wasm"
    echo "# (difflabai/shared-pool-controls v1.1.0, source-mirror ${pin:0:7})."
    echo "# Generated by near-tests/wasm-surface.sh --update; review every change."
    echo "# build: scripts/build.sh (RUSTFLAGS=\"--cfg near\", cargo build --target wasm32-unknown-unknown --release --locked), $rustc_v"
    echo "# Line: build <wasm> export <kind> <name> | build <wasm> import <kind> <module.name> <class>"
    echo "# class: promise (promise_* host functions), forbidden (build.sh's FORBIDDEN names and"
    echo "# promise_batch_*; none since v1.1.0), - (other)."
    cat "$work/surface"
  } >"$expected"
  echo "updated $expected"
elif diff -u <(grep -v '^#' "$expected") "$work/surface"; then
  echo "SURF-1 ok: surface equals wasm-surface.expected"
else
  echo "SURF-1 FAIL: surface differs from wasm-surface.expected" >&2
  status=1
fi

nf=$(grep -c ' forbidden$' "$work/surface" || true)
if [ "$nf" -gt 0 ]; then
  echo "SURF-2 FAIL: $nf forbidden host-function imports:" >&2
  grep ' forbidden$' "$work/surface" >&2
  status=1
else
  echo "SURF-2 ok: no deploy/delete/key/transfer/stake/account/global-contract or promise_batch imports"
fi

# --- SURF-3(a): build.sh itself --------------------------------------------
# build.sh writes artifacts/build.json only when its checker passes and never
# deletes an old one, so remove it first: the check below must read this
# run's manifest, not a stale one. If build.sh then fails (exit 2), the
# mirror is left without build.json (artifacts/ is gitignored upstream); the
# next successful run of this script or of build.sh writes it again.
rm -f "$src/artifacts/build.json"
if ! (cd "$src" && env -u CARGO_TARGET_DIR -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS -u RUSTUP_TOOLCHAIN ./scripts/build.sh) >"$work/buildsh.log" 2>&1; then
  tail -n 30 "$work/buildsh.log"
  echo "SURF-3(a) FAIL: scripts/build.sh failed" >&2
  exit 2
fi
python3 -I "$work/surface.py" --forbidden >"$work/forbidden"
python3 -I - "$src" "$pin" "$(wasm_of near pool_contract)" "$(wasm_of near pool_executor)" "$work/surface" "$work/forbidden" <<'PY' || status=1
import hashlib, json, pathlib, re, sys
src, pin, contract, executor, surface, forbidden = sys.argv[1:]
src = pathlib.Path(src)
errors = []
m = json.loads((src / "artifacts" / "build.json").read_text())
if m["dirty"] is not False:
    errors.append("build.json: dirty is %r" % m["dirty"])
if m["source_commit"] != pin:
    errors.append("build.json: source_commit %s != pinned %s" % (m["source_commit"], pin))
if m["rustflags"] != "--cfg near":
    errors.append("build.json: rustflags %r" % m["rustflags"])
ours = {"pool_contract": contract, "pool_executor": executor}
for n, p in ours.items():
    key = "target/wasm32-unknown-unknown/release/%s.wasm" % n
    got = m["wasm"].get(key, {}).get("sha256")
    want = hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
    print("  %s: build.sh %s, near-tests Build::Near %s" % (n, got, want))
    if got != want:
        errors.append("%s: build.sh's sha256 %s != the near-tests build's %s" % (n, got, want))
text = (src / "scripts" / "build.sh").read_text()
lst = lambda s: re.findall(r"'([^']+)'", s)
common = lst(re.search(r"^COMMON=\[([^\]]*)\]", text, re.M).group(1))
allowed = {k: common + lst(v) for k, v in re.findall(r"'(pool_\w+)':COMMON\+\[([^\]]*)\]", text)}
bforb = lst(re.search(r"^FORBIDDEN=\[([^\]]*)\]", text, re.M).group(1))
if sorted(allowed) != ["pool_contract", "pool_executor"]:
    errors.append("build.sh ALLOWED keys %s" % sorted(allowed))
if bforb != open(forbidden).read().split():
    errors.append("build.sh FORBIDDEN %s != wasm-surface.sh's %s" % (bforb, open(forbidden).read().split()))
imports = {}
for line in open(surface):
    f = line.split()
    if f[2] == "import":
        imports.setdefault(f[1], []).append((f[3], f[4]))
for n, names in allowed.items():
    got = imports.get(n, [])
    if any(k != "func" or not name.startswith("env.") for k, name in got):
        errors.append("%s: a non-function or non-env import: %s" % (n, got))
    got = sorted(name[len("env."):] for _, name in got)
    extra = sorted(set(got) - set(names))
    unused = sorted(set(names) - set(got))
    print("  %s: %d imports, build.sh ALLOWED %d names" % (n, len(got), len(names)))
    if extra or unused or len(names) != len(set(names)):
        errors.append("%s: imports != ALLOWED (extra %s, unused %s)" % (n, extra, unused))
for e in errors:
    print("SURF-3(a) FAIL: " + e, file=sys.stderr)
if errors:
    sys.exit(1)
print("SURF-3(a) ok: build.sh passes; build.json clean at the pin with --cfg near; its hashes are the near-tests wasm; imports == ALLOWED (%s)" % ", ".join("%s %d" % (n, len(v)) for n, v in sorted(allowed.items())))
PY

# --- SURF-3(b): build.sh's checker refuses what it must ----------------------
chk="$here/target/surface/buildcheck.py"
mkdir -p "$(dirname "$chk")"
sed -n "/<<'PY'/,/^PY\$/p" "$src/scripts/build.sh" | sed '1d;$d' >"$chk"
synth() {  # synth <out> <import>...: a wasm with one type and env.<import> functions
  python3 -I - "$@" <<'PY'
import sys
out, names = sys.argv[1], sys.argv[2:]
def leb(n):
    b = bytearray()
    while True:
        x = n & 0x7F; n >>= 7
        b.append(x | (0x80 if n else 0))
        if not n:
            return bytes(b)
def s(t):
    t = t.encode()
    return leb(len(t)) + t
def section(sid, body):
    return bytes([sid]) + leb(len(body)) + body
types = section(1, leb(1) + b"\x60\x00\x00")
imports = section(2, leb(len(names)) + b"".join(s("env") + s(n) + b"\x00" + leb(0) for n in names))
open(out, "wb").write(b"\0asm\x01\0\0\0" + types + imports)
PY
}
neg() {  # neg <label> <expected text> <contract wasm> <executor wasm>
  local d="$work/neg-$1" rc=0
  mkdir -p "$d/target/wasm32-unknown-unknown/release"
  cp "$3" "$d/target/wasm32-unknown-unknown/release/pool_contract.wasm"
  cp "$4" "$d/target/wasm32-unknown-unknown/release/pool_executor.wasm"
  (cd "$d" && python3 -I "$chk") >"$d/out" 2>&1 || rc=$?
  if [ $rc -eq 1 ] && grep -qF "$2" "$d/out" && [ ! -e "$d/artifacts/build.json" ]; then
    echo "SURF-3(b) ok ($1): exit 1, $(head -c 200 "$d/out")"
  else
    echo "SURF-3(b) FAIL ($1): exit $rc, expected 1 with \"$2\": $(cat "$d/out")" >&2
    status=1
  fi
}
synth "$work/transfer.wasm" panic_utf8 promise_batch_action_transfer
synth "$work/yield.wasm" panic_utf8 promise_yield_create
neg plain "does not import env.panic_utf8" "$(wasm_of plain pool_contract)" "$(wasm_of plain pool_executor)"
neg transfer "imports host functions for actions other than function calls" "$work/transfer.wasm" "$(wasm_of near pool_executor)"
neg yield "has imports that are not in the reviewed list" "$work/yield.wasm" "$(wasm_of near pool_executor)"
exit $status
