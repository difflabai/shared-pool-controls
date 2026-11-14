#!/usr/bin/env bash
# Builds the two wasm files under test with the documented build: upstream
# scripts/build.sh, run in the pinned source mirror. v1.1.0's build.sh sets
# RUSTFLAGS="--cfg near" (so near-sdk calls panic_utf8 and every failure
# carries its error name; finding SB-1 of the v1.0.0 record), unsets
# CARGO_ENCODED_RUSTFLAGS, runs
#   cargo build -p pool-contract -p pool-executor \
#     --target wasm32-unknown-unknown --release --locked
# into the mirror's target/ (gitignored upstream), checks the import section
# of both files against its reviewed list, and only then writes
# artifacts/build.json (also gitignored, so the mirror stays clean).
#
# The environment is cleared of CARGO_TARGET_DIR, RUSTFLAGS and
# CARGO_ENCODED_RUSTFLAGS so nothing outside build.sh changes the build;
# RUSTUP_TOOLCHAIN is POOL_TOOLCHAIN when set (run.sh sets it to the channel
# of the mirror's rust-toolchain.toml), else unset, so rustup reads that file.
#
# Then: build.json must say dirty false, source_commit = the mirror's HEAD and
# rustflags "--cfg near"; both wasm files are copied to
# target/pool/wasm32-unknown-unknown/release/ (where tests/common reads them)
# and the copies must hash to build.json's sha256. Writes results/wasm.json
# (sha256, NEAR code_hash, size, rustc, flags, and build.json itself).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
mirror="$(cd "$here/../source-mirror" && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
tc=()
if [ -n "${POOL_TOOLCHAIN:-}" ]; then tc=(RUSTUP_TOOLCHAIN="$POOL_TOOLCHAIN"); fi

# build.sh never deletes an old build.json, so remove it: the checks below
# must read this run's manifest.
rm -f "$mirror/artifacts/build.json"
(cd "$mirror" && env -u CARGO_TARGET_DIR -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS -u RUSTUP_TOOLCHAIN \
    "${tc[@]+"${tc[@]}"}" ./scripts/build.sh) >&2

out="$here/target/pool/wasm32-unknown-unknown/release"
mkdir -p "$out"
cp "$mirror/target/wasm32-unknown-unknown/release/pool_contract.wasm" \
   "$mirror/target/wasm32-unknown-unknown/release/pool_executor.wasm" "$out/"
rustc_v="$(cd "$mirror" && env -u RUSTUP_TOOLCHAIN "${tc[@]+"${tc[@]}"}" rustc --version)"
head="$(git -C "$mirror" rev-parse HEAD)"
python3 -I - "$here" "$mirror" "$head" "$rustc_v" <<'PY'
import hashlib, json, pathlib, sys
here, mirror, head, rustc = sys.argv[1:]
here, mirror = pathlib.Path(here), pathlib.Path(mirror)
A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def b58(d):
    n = int.from_bytes(d, "big"); s = ""
    while n:
        n, r = divmod(n, 58); s = A[r] + s
    return "1" * (len(d) - len(d.lstrip(b"\0"))) + s
build = json.loads((mirror / "artifacts/build.json").read_text())
errors = []
if build["dirty"] is not False:
    errors.append("build.json: dirty is %r" % build["dirty"])
if build["source_commit"] != head:
    errors.append("build.json: source_commit %s != mirror HEAD %s" % (build["source_commit"], head))
if build["rustflags"] != "--cfg near":
    errors.append("build.json: rustflags %r" % build["rustflags"])
wasm = {}
for name in ["pool_contract", "pool_executor"]:
    p = here / "target/pool/wasm32-unknown-unknown/release" / f"{name}.wasm"
    b = p.read_bytes()
    d = hashlib.sha256(b).digest()
    want = build["wasm"].get(f"target/wasm32-unknown-unknown/release/{name}.wasm", {}).get("sha256")
    if d.hex() != want:
        errors.append("%s: copy sha256 %s != build.json %s" % (name, d.hex(), want))
    wasm[name] = {"path": str(p.relative_to(here)), "sha256": d.hex(), "code_hash": b58(d), "bytes": len(b)}
if errors:
    sys.exit("build-wasm.sh: " + "; ".join(errors))
manifest = {
    "source_commit": head,
    "source_dirty": build["dirty"],
    "rustc": rustc,
    "build": "upstream scripts/build.sh (RUSTFLAGS=\"--cfg near\"; cargo build -p pool-contract -p pool-executor --target wasm32-unknown-unknown --release --locked)",
    "profile": "mirror [profile.release]: codegen-units=1, opt-level=z, lto, panic=abort, overflow-checks=true",
    "variant": "upstream",
    "wasm": wasm,
    "build_json": build,
}
(here / "results").mkdir(exist_ok=True)
(here / "results/wasm.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest, indent=2))
PY
