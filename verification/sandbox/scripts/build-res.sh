#!/usr/bin/env bash
# Builds the helper contract relay/ (RT-PRED, RT-PRIVATE, RT-RACE) the same way
# as the contracts under test (plain wasm32 release build, toolchain
# $POOL_TOOLCHAIN, default 1.98, --locked), to
# target/wasm32-unknown-unknown/release/sandbox_relay.wasm.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo "+${POOL_TOOLCHAIN:-1.98}" build -p sandbox-relay --release --target wasm32-unknown-unknown --locked
w=target/wasm32-unknown-unknown/release/sandbox_relay.wasm
echo "$w $(shasum -a 256 "$w" | cut -d' ' -f1)"
