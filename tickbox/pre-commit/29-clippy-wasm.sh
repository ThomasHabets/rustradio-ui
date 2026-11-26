#!/usr/bin/env bash
set -ueo pipefail
cd "$TICKBOX_TEMPDIR/work"
export CARGO_TARGET_DIR="$TICKBOX_CWD/target/${TICKBOX_BRANCH}.clippy"
exec cargo clippy --locked --target wasm32-unknown-unknown --all-features --all-targets -- -D warnings
