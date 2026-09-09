#!/usr/bin/env bash
# Explicit, user-local installation. Never downloads a toolchain or edits shell files.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

if ! command -v cargo >/dev/null 2>&1; then
    printf 'Rust/Cargo is required. Install it from https://rustup.rs, then retry.\n' >&2
    exit 1
fi

printf 'Installing LlamaScope to %s/bin (replaces an existing LlamaScope binary)...\n' "$PREFIX"
cargo install --locked --path "$APP_DIR" --root "$PREFIX" --bin llamascope --force
printf '\nInstalled. Try: %s/bin/llamascope --demo\n' "$PREFIX"
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) printf 'Add this directory to your PATH if desired: %s/bin\nNo shell configuration was changed.\n' "$PREFIX" ;;
esac
