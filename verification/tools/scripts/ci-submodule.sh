#!/usr/bin/env bash
# CI only: checks out one public submodule without credentials. Other
# submodules stay uninitialised. Fork PRs use the same anonymous checkout.
#
# usage: tools/scripts/ci-submodule.sh <submodule path>
#
# SOURCE_REF, if set (40 hex chars), is checked out after the gitlink.
# The nightly uses it to verify upstream when contracts/approval moved
# past the pin. Empty means the gitlink, which is the pin.
set -euo pipefail
path=${1:?usage: ci-submodule.sh <submodule path>}
git submodule update --init --depth 1 -- "$path"
if [ -n "${SOURCE_REF:-}" ]; then
  [[ $SOURCE_REF =~ ^[0-9a-f]{40}$ ]] || {
    echo "ci-submodule: SOURCE_REF must be a 40-char sha, got ${SOURCE_REF}" >&2
    exit 2
  }
  have=$(git -C "$path" rev-parse HEAD)
  if [ "$have" != "$SOURCE_REF" ]; then
    git -C "$path" fetch --depth 1 origin "$SOURCE_REF"
    git -C "$path" checkout --detach FETCH_HEAD
  fi
fi
