#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build.sh
cargo test -p pool-sandbox --locked -- --nocapture
