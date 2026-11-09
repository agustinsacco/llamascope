#!/bin/sh
# LlamaScope installer. Installs one binary into $PREFIX/bin (default ~/.local/bin).
# Never edits shell configuration and never installs a toolchain.
#
#   curl -fsSL https://raw.githubusercontent.com/agustinsacco/llamascope/main/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version 1.1.0
#   ./install.sh --source        # build this checkout with Cargo instead
#
# Private repository: export GH_TOKEN (or sign in with gh) first.
set -eu

REPO="${LLAMASCOPE_REPO:-agustinsacco/llamascope}"
API="${LLAMASCOPE_API:-https://api.github.com}"
PREFIX="${PREFIX:-$HOME/.local}"
VERSION=""
SOURCE=0

die() { printf 'llamascope install: %s\n' "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --source) SOURCE=1 ;;
        --version) [ $# -ge 2 ] || die "--version requires X.Y.Z"; VERSION="${2#v}"; shift ;;
        -h|--help) sed -n '2,9p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
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

TOKEN="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
if [ -z "$TOKEN" ] && command -v gh >/dev/null 2>&1; then
    TOKEN="$(gh auth token 2>/dev/null || true)"
fi

# fetch URL ACCEPT OUTPUT. curl drops the token if GitHub redirects to its download host.
fetch() {
    if [ -n "$TOKEN" ]; then
        curl -fsSL --retry 2 -H "Accept: $2" -H "Authorization: Bearer $TOKEN" -o "$3" "$1"
    else
        curl -fsSL --retry 2 -H "Accept: $2" -o "$3" "$1"
    fi
}

WORK="$(mktemp -d "${TMPDIR:-/tmp}/llamascope-install.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT INT TERM

if [ -n "$VERSION" ]; then RELEASE="tags/v$VERSION"; else RELEASE="latest"; fi
fetch "$API/repos/$REPO/releases/$RELEASE" application/vnd.github+json "$WORK/release.json" \
    || die "cannot read release $RELEASE of $REPO. If the repository is private, export GH_TOKEN or sign in with gh."

# Each asset lists its API "url" before its "name"; no jq required.
asset_url() {
    tr ',{}' '\n\n\n' < "$WORK/release.json" | awk -v want="\"name\":\"$1\"" '
        { gsub(/^[ \t]+|[ \t]+$/, ""); gsub(/": "/, "\":\"") }
        /^"url":".*\/releases\/assets\/[0-9]+"$/ { url = substr($0, 8, length($0) - 8) }
        $0 == want && url != "" { print url; exit }'
}
TAG="$(tr ',' '\n' < "$WORK/release.json" | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)"
BINARY_URL="$(asset_url "llamascope-$TARGET")"
SUMS_URL="$(asset_url SHA256SUMS)"
[ -n "$TAG" ] && [ -n "$BINARY_URL" ] && [ -n "$SUMS_URL" ] || die "release $RELEASE has no llamascope-$TARGET build"

printf 'Downloading LlamaScope %s for %s...\n' "$TAG" "$TARGET"
fetch "$BINARY_URL" application/octet-stream "$WORK/llamascope-$TARGET" || die "download failed"
fetch "$SUMS_URL" application/octet-stream "$WORK/SHA256SUMS" || die "checksum download failed"
EXPECTED="$(awk -v name="llamascope-$TARGET" '$2 == name || $2 == "*" name { print $1; exit }' "$WORK/SHA256SUMS")"
ACTUAL="$(cd "$WORK" && $SHA "llamascope-$TARGET" | awk '{ print $1 }')"
[ -n "$EXPECTED" ] && [ "$EXPECTED" = "$ACTUAL" ] || die "checksum mismatch; nothing was installed"

chmod 755 "$WORK/llamascope-$TARGET"
[ "$("$WORK/llamascope-$TARGET" --version)" = "LlamaScope $TAG" ] || die "the downloaded build did not report version $TAG"
mkdir -p "$PREFIX/bin"
# Stage on the destination filesystem, then rename: a running copy keeps working.
cp "$WORK/llamascope-$TARGET" "$PREFIX/bin/.llamascope-install.$$"
mv -f "$PREFIX/bin/.llamascope-install.$$" "$PREFIX/bin/llamascope"
printf 'LlamaScope %s is in %s/bin. Later, run: llamascope update\n' "$TAG" "$PREFIX"
finish
