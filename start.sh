#!/usr/bin/env bash
# Cargo's incremental build prevents stale binaries; every argument reaches the app.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    printf 'Rust/Cargo is required. Install it from https://rustup.rs, then retry.\n' >&2
    exit 1
fi

exec cargo run --locked --release --manifest-path "$APP_DIR/Cargo.toml" -- "$@"
