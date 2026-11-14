#!/usr/bin/env bash
# Downloads the pinned Lean 4 toolchain (tools/versions.env) into
# LEAN_CACHE (default tools/.cache) and checks its sha256. Prints the
# toolchain's bin directory (lean, lake). Idempotent. Checks that every
# lean-toolchain file under projects/ names the same version.
#
# usage: tools/scripts/get-lean.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
. "$root/tools/versions.env"
cache=${LEAN_CACHE:-$root/tools/.cache}
mkdir -p "$cache"

for f in "$root"/projects/*/lean/lean-toolchain; do
  [ -f "$f" ] || continue
  if [ "$(cat "$f")" != "leanprover/lean4:v$LEAN" ]; then
    echo "$f says $(cat "$f"), tools/versions.env says v$LEAN" >&2
    exit 1
  fi
done

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) plat=linux; sha=$LEAN_LINUX_SHA256 ;;
  Darwin-arm64) plat=darwin_aarch64; sha=$LEAN_DARWIN_AARCH64_SHA256 ;;
  *) echo "no pinned Lean build for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

hash() {
  if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | awk '{print $1}'
}

name="lean-$LEAN-$plat"
dest="$cache/$name-${sha:0:12}"
# Jobs can share LEAN_CACHE (a self-hosted runner's tool cache), so the zip
# is fetched and unpacked in a per-process temp directory and the toolchain
# is installed by renaming the complete tree to $dest: no job ever sees a
# partial one. Temp directories older than an hour are left-overs of
# cancelled jobs.
find "$cache" -maxdepth 1 -name "$name.??????" -mmin +60 -exec rm -rf {} +
if [ ! -x "$dest/bin/lake" ]; then
  tmp=$(mktemp -d "$cache/$name.XXXXXX")
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL --retry 4 --retry-all-errors -o "$tmp/$name.zip" \
    "https://github.com/leanprover/lean4/releases/download/v$LEAN/$name.zip"
  if [ "$(hash "$tmp/$name.zip")" != "$sha" ]; then
    echo "sha256 mismatch for $name.zip (got $(hash "$tmp/$name.zip"), pinned $sha)" >&2
    exit 1
  fi
  unzip -q "$tmp/$name.zip" -d "$tmp"
  # If another job installed $dest first, keep its tree: either the rename
  # fails, or mv moves this tree inside $dest under its unique temp name,
  # from where it is removed.
  uniq=$(basename "$tmp")
  mv "$tmp/$name" "$tmp/$uniq"
  if [ ! -e "$dest" ]; then
    mv "$tmp/$uniq" "$dest" 2>/dev/null || true
    rm -rf "${dest:?}/$uniq"
  fi
  [ -x "$dest/bin/lake" ] || { echo "could not install $dest" >&2; exit 1; }
fi
echo "$dest/bin"
