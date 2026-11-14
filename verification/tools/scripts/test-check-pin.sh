#!/usr/bin/env bash
# Self-test for check-pin.sh, mainly its identity check ([project] identity =
# [{ label, commit, paths }]): on scratch repositories, no network. An
# identical commit that the mirror lacks is fetched by SHA and passes; an
# entry whose commit is the pin passes without a fetch; a path that differs
# (content or mode), a path missing on either side, a commit that cannot be
# fetched, a malformed entry and the old verified_release_commit /
# identical_paths form each fail.
#
# usage: tools/scripts/test-check-pin.sh
# Needs git and python3 >= 3.11 (tomllib), as check-pin.sh does.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0

# The user's git configuration (signing, hooks, default branch) stays out.
export HOME="$tmp/home" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.invalid GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.invalid
mkdir -p "$HOME"
g() { git -c init.defaultBranch=main -c commit.gpgsign=false -c protocol.file.allow=always "$@"; }

# Upstream: pin; `same` (only README differs from the pin); `diff` (core/lib.rs
# differs); `mode` (core/lib.rs executable, same content). Only the pin's
# branch is cloned into the mirror, so the other three must be fetched.
up="$tmp/up"
g init -q "$up"
mkdir -p "$up/core" "$up/docs"
echo 'fn a() {}' >"$up/core/lib.rs"
echo 'spec' >"$up/docs/spec.md"
echo 'readme' >"$up/README.md"
printf '[profile.release]\noverflow-checks = true\n' >"$up/Cargo.toml"
g -C "$up" add -A && g -C "$up" commit -qm pin
pin=$(g -C "$up" rev-parse HEAD)
g -C "$up" branch -q pinned
echo 'readme 2' >"$up/README.md"; g -C "$up" commit -qam same
same=$(g -C "$up" rev-parse HEAD)
g -C "$up" checkout -q -b other "$pin"
echo 'fn b() {}' >"$up/core/lib.rs"; g -C "$up" commit -qam diff
diff=$(g -C "$up" rev-parse HEAD)
g -C "$up" checkout -q -b modebit "$pin"
chmod +x "$up/core/lib.rs"; g -C "$up" commit -qam mode
mode=$(g -C "$up" rev-parse HEAD)
g -C "$up" checkout -q main
g -C "$up" config uploadpack.allowAnySHA1InWant true

# A repository laid out like this one: the script, and projects/p with the
# mirror as a gitlink at the pin.
root="$tmp/root"
mkdir -p "$root/tools/scripts" "$root/projects/p"
cp "$here/check-pin.sh" "$root/tools/scripts/check-pin.sh"
g init -q "$root"
mirror="$root/projects/p/source-mirror"
g clone -q --no-local --single-branch --branch pinned "file://$up" "$mirror"
g -C "$root" add tools projects/p/source-mirror 2>/dev/null   # the mirror becomes a gitlink
g -C "$root" commit -qm root
if g -C "$mirror" cat-file -e "$same^{commit}" 2>/dev/null; then
  echo "FAIL: setup: the mirror already has $same, so no case would fetch"; exit 1
fi
lib_sha=$(python3 -I -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$mirror/core/lib.rs")

# expect <0|1> <name> <text the output must contain> <[project] lines>
# (UPSTREAM_PATH and PINNED override the defaults for one case.)
expect() {
  local want=$1 name=$2 needle=$3 extra=$4 key=core/lib.rs
  [ -n "${UPSTREAM_PATH:-}" ] && key=lib.rs   # [source_hashes] is relative to upstream_path
  {
    echo '[project]'
    echo 'name = "p"'
    echo "upstream_path = \"${UPSTREAM_PATH:-}\""
    echo "pinned_commit = \"${PINNED:-$pin}\""
    [ -n "${UPSTREAM_PATH:-}" ] || echo 'release_overflow_checks = true'
    printf '%s\n' "$extra"
    echo '[source_hashes]'
    echo "\"$key\" = \"$lib_sha\""
  } >"$root/projects/p/status.toml"
  "$root/tools/scripts/check-pin.sh" p >"$tmp/out" 2>&1
  local got=$?
  [ "$got" -gt 1 ] && got=2
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name: exit $got, want $want"; sed 's/^/  /' "$tmp/out"; fail=1
  elif ! grep -qF -- "$needle" "$tmp/out"; then
    echo "FAIL: $name: output lacks \"$needle\""; sed 's/^/  /' "$tmp/out"; fail=1
  fi
}

expect 0 "no identity entries" "pin ok" ""
PINNED=$diff expect 1 "pinned_commit is not the gitlink" "!= pinned_commit" ""
expect 0 "identical commit, fetched by SHA" "later (${same:0:7}): core, docs/spec.md, Cargo.toml byte-identical to the pin" \
  "identity = [ { label = \"later\", commit = \"$same\", paths = [\"core\", \"docs/spec.md\", \"Cargo.toml\"] } ]"
if ! g -C "$mirror" cat-file -e "$same^{commit}" 2>/dev/null; then
  echo "FAIL: identical commit: check-pin.sh passed but did not fetch $same"; fail=1
fi
# The pin itself: no fetch (the origin is unreachable for this case).
g -C "$mirror" remote set-url origin "file://$tmp/nowhere"
expect 0 "commit = pin passes without a fetch" "release (${pin:0:7}) is the pin" \
  "identity = [ { label = \"release\", commit = \"$pin\", paths = [\"core\", \"docs\"] } ]"
expect 1 "commit = pin, path missing at the pin" "release: nope is missing at the pin" \
  "identity = [ { label = \"release\", commit = \"$pin\", paths = [\"core\", \"nope\"] } ]"
expect 1 "unfetchable commit" "ghost: cannot fetch 0123456789abcdef0123456789abcdef01234567" \
  "identity = [ { label = \"ghost\", commit = \"0123456789abcdef0123456789abcdef01234567\", paths = [\"core\"] } ]"
g -C "$mirror" remote set-url origin "file://$up"
expect 1 "one path differs" "head: core differs between the pin and ${diff:0:7}" \
  "identity = [ { label = \"head\", commit = \"$diff\", paths = [\"docs\", \"core\"] } ]"
if grep -q 'docs differs' "$tmp/out"; then echo "FAIL: one path differs: the identical path docs was reported"; fail=1; fi
expect 1 "mode differs, content equal" "head: core/lib.rs differs between the pin and ${mode:0:7} (100644 blob" \
  "identity = [ { label = \"head\", commit = \"$mode\", paths = [\"core/lib.rs\"] } ]"
expect 1 "path missing at the commit and the pin" "later: nope is missing at the pin and later" \
  "identity = [ { label = \"later\", commit = \"$same\", paths = [\"core\", \"nope\"] } ]"
expect 0 "two entries, both hold" "later (${same:0:7}): core byte-identical to the pin" \
  "identity = [ { label = \"merge\", commit = \"$pin\", paths = [\"core\"] }, { label = \"later\", commit = \"$same\", paths = [\"core\"] } ]"
expect 1 "two entries, the second differs" "head: core differs" \
  "identity = [ { label = \"later\", commit = \"$same\", paths = [\"core\"] }, { label = \"head\", commit = \"$diff\", paths = [\"core\"] } ]"
UPSTREAM_PATH=core expect 0 "upstream_path joins the paths" "later (${same:0:7}): lib.rs byte-identical to the pin" \
  "identity = [ { label = \"later\", commit = \"$same\", paths = [\"lib.rs\"] } ]"
UPSTREAM_PATH=core expect 1 "upstream_path joins the paths (differs)" "head: core/lib.rs differs" \
  "identity = [ { label = \"head\", commit = \"$diff\", paths = [\"lib.rs\"] } ]"
expect 1 "abbreviated sha" "short: commit '${same:0:7}' is not a 40-character sha" \
  "identity = [ { label = \"short\", commit = \"${same:0:7}\", paths = [\"core\"] } ]"
expect 1 "empty paths" "later: paths must be a non-empty list of paths" \
  "identity = [ { label = \"later\", commit = \"$same\", paths = [] } ]"
expect 1 "identity is not a list" "identity is not a list" \
  "identity = \"$same\""
expect 1 "the old form is refused" "verified_release_commit is no longer read" \
  "verified_release_commit = \"$same\"
identical_paths = [\"core\"]"

if [ "$fail" = 0 ]; then echo "check-pin self-test ok"; fi
exit "$fail"
