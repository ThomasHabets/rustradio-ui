#!/usr/bin/env bash
# The main goal here is actually to update Cargo.lock
set -ueo pipefail
cargo check
for e in iq-waterfall rtlsdr-fm; do
        (cd examples/$e; cargo check)
done
