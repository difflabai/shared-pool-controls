#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo test --locked
cargo test -p pool-core --no-default-features --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
node --check ui/app.js
node --check ui/live.js
