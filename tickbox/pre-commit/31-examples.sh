#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."

# Tickbox's RUSTFLAGS overrides the examples' required WASM flags. Merge the
# warning policy into their target configuration instead.
unset RUSTFLAGS

for example in iq-waterfall rtlsdr-fm; do
    echo "Checking example $example"
    (
        cd "examples/$example"
        cargo fmt -- --check
        cargo check --locked --target wasm32-unknown-unknown --all-features --lib \
            --config 'target.wasm32-unknown-unknown.rustflags=["--deny", "warnings"]'
    )
done
