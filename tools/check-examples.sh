#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

for example in iq-waterfall rtlsdr-fm; do
    (
        cd "examples/$example"
        cargo fmt -- --check
        cargo check --locked --target wasm32-unknown-unknown --all-features --lib
    )
done
