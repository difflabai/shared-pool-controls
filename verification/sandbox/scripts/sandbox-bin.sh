#!/usr/bin/env bash
# Prints the path of a sha256-checked near-sandbox 2.13.4 binary (the default
# of near-sandbox 0.3.16 / near-workspaces 0.23.0, upstream crates/sandbox's
# pin), downloading it once into $SANDBOX_BIN_CACHE. run.sh exports it as
# NEAR_SANDBOX_BIN_PATH, so near-workspaces neither downloads (its build script
# skips the ureq download when the variable is set) nor uses an unchecked copy.
# Hashes: the same pins as projects/near-approval/sandbox/scripts/sandbox-bin.sh
# (trust-framework scripts/near/sandbox.sh, recorded 2026-10-19).
set -euo pipefail
VER=2.13.4
CACHE=${SANDBOX_BIN_CACHE:-"$HOME/.cache/formalver/near-sandbox"}
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) plat=Darwin-arm64; sha=158cedb02e5a83d51b47c16e13cd04a84abf493f1f38688ee257cb0263450631 ;;
  Linux:x86_64) plat=Linux-x86_64; sha=5690a4635172263cf026683f75965cac09dd916912584f12709974fed620163f ;;
  Linux:aarch64 | Linux:arm64) plat=Linux-aarch64; sha=edd77d6d28b960852243fc6f45656c97a7976fc2b3cfe53cda89324a6804bd30 ;;
  *) echo "near-sandbox has no build for $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac
sha256_hex() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{ print $1 }'
  else shasum -a 256 "$1" | awk '{ print $1 }'; fi
}
bin="$CACHE/near-sandbox-$VER-$plat/near-sandbox"
if [ ! -x "$bin" ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  url="https://s3-us-west-1.amazonaws.com/build.nearprotocol.com/nearcore/$plat/$VER/near-sandbox.tar.gz"
  echo "Downloading near-sandbox $VER ($plat)" >&2
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$tmp/near-sandbox.tar.gz" "$url"
  got=$(sha256_hex "$tmp/near-sandbox.tar.gz")
  [ "$got" = "$sha" ] || { echo "sha256 mismatch for $url: expected $sha, got $got" >&2; exit 1; }
  mkdir -p "$tmp/x"
  tar -xzf "$tmp/near-sandbox.tar.gz" -C "$tmp/x"
  f=$(find "$tmp/x" -type f -name near-sandbox | head -n 1)
  [ -n "$f" ] || { echo "near-sandbox binary not found in the tarball" >&2; exit 1; }
  mkdir -p "$(dirname "$bin")"
  install -m 0755 "$f" "$bin"
fi
printf '%s\n' "$bin"
