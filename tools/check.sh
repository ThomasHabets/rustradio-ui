#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt -- --check
cargo test --locked --lib
cargo clippy --locked --target wasm32-unknown-unknown --all-features --all-targets --no-deps -- -D warnings
./tools/check-channels.sh
./tools/check-channels.sh --all-features
