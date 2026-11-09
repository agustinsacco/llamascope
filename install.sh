#!/bin/sh
# LlamaScope installer. Installs one binary into $PREFIX/bin (default ~/.local/bin).
# Never edits shell configuration and never installs a toolchain.
#
#   curl -fsSL https://github.com/agustinsacco/llamascope/releases/latest/download/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version 1.1.42
#   ./install.sh --source        # build this checkout with Cargo instead
#
# Later, `llamascope update` replaces the binary with the newest release.
set -eu

REPO="${LLAMASCOPE_REPO:-agustinsacco/llamascope}"
RELEASES="${LLAMASCOPE_RELEASES:-https://github.com/$REPO/releases}"
PREFIX="${PREFIX:-$HOME/.local}"
VERSION=""
SOURCE=0

die() { printf 'llamascope install: %s\n' "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --source) SOURCE=1 ;;
        --version) [ $# -ge 2 ] || die "--version requires X.Y.Z"; VERSION="${2#v}"; shift ;;
        -h|--help) printf 'Usage: install.sh [--version X.Y.Z | --source]\nPREFIX=%s\n' "$PREFIX"; exit 0 ;;
        *) die "unknown option $1 (try --help)" ;;
    esac
    shift
done

finish() {
    printf '\nInstalled. Try: %s/bin/llamascope --demo\n' "$PREFIX"
    case ":$PATH:" in
        *":$PREFIX/bin:"*) ;;
        *) printf 'Add this directory to your PATH if desired: %s/bin\nNo shell configuration was changed.\n' "$PREFIX" ;;
    esac
}

if [ "$SOURCE" = 1 ]; then
    APP_DIR="$(cd "$(dirname "$0")" && pwd)"
    [ -f "$APP_DIR/Cargo.toml" ] || die "--source must run from a LlamaScope checkout"
    PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo >/dev/null 2>&1 || die "Rust/Cargo is required. Install it from https://rustup.rs, then retry."
    printf 'Building LlamaScope into %s/bin (replaces an existing LlamaScope binary)...\n' "$PREFIX"
    cargo install --locked --path "$APP_DIR" --root "$PREFIX" --bin llamascope --force
    finish
    exit 0
fi

case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) TARGET=x86_64-unknown-linux-musl ;;
    Linux/aarch64|Linux/arm64) TARGET=aarch64-unknown-linux-musl ;;
    Darwin/x86_64) TARGET=x86_64-apple-darwin ;;
    Darwin/arm64) TARGET=aarch64-apple-darwin ;;
    *) die "no release build for $(uname -s)/$(uname -m); clone the repository and run ./install.sh --source" ;;
esac
command -v curl >/dev/null 2>&1 || die "curl is required"
if command -v sha256sum >/dev/null 2>&1; then SHA="sha256sum"; else SHA="shasum -a 256"; fi
if [ -n "$VERSION" ]; then BASE="$RELEASES/download/v$VERSION"; else BASE="$RELEASES/latest/download"; fi
ASSET="llamascope-$TARGET"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/llamascope-install.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT INT TERM

printf 'Downloading %s from %s...\n' "$ASSET" "$BASE"
curl -fsSL --retry 2 -o "$WORK/SHA256SUMS" "$BASE/SHA256SUMS" \
    || die "cannot find release ${VERSION:-latest} at $RELEASES"
curl -fsSL --retry 2 -o "$WORK/$ASSET" "$BASE/$ASSET" || die "this release has no $ASSET build"
EXPECTED="$(awk -v name="$ASSET" '$2 == name || $2 == "*" name { print $1; exit }' "$WORK/SHA256SUMS")"
ACTUAL="$(cd "$WORK" && $SHA "$ASSET" | awk '{ print $1 }')"
[ -n "$EXPECTED" ] && [ "$EXPECTED" = "$ACTUAL" ] || die "checksum mismatch; nothing was installed"

chmod 755 "$WORK/$ASSET"
REPORTED="$("$WORK/$ASSET" --version 2>/dev/null || true)"
case "$REPORTED" in
    "LlamaScope ${VERSION:-}"*) ;;
    *) die "the downloaded build does not run here or is not ${VERSION:-a LlamaScope release}" ;;
esac
[ -z "$VERSION" ] || [ "$REPORTED" = "LlamaScope $VERSION" ] || die "the downloaded build is not $VERSION"
mkdir -p "$PREFIX/bin"
# Stage on the destination filesystem, then rename: a running copy keeps working.
cp "$WORK/$ASSET" "$PREFIX/bin/.llamascope-install.$$"
mv -f "$PREFIX/bin/.llamascope-install.$$" "$PREFIX/bin/llamascope"
printf '%s is in %s/bin. Later, run: llamascope update\n' "$REPORTED" "$PREFIX"
finish
