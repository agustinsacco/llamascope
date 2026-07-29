#!/bin/bash
# LlamaScope Installer Script

set -e

echo "📡 Starting LlamaScope Installation..."

# 1. Ensure Rust & Cargo are installed
if ! command -v cargo &> /dev/null; then
    echo "📦 Rust/Cargo not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    echo "✅ Rust/Cargo is already installed."
fi

# 2. Compile in Release Mode
echo "🔨 Compiling LlamaScope..."
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$APP_DIR"
cargo build --release

# 3. Create install destination
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# 4. Copy binary
echo "🚀 Installing binary to $INSTALL_DIR/llamascope..."
cp target/release/llamascope "$INSTALL_DIR/llamascope"

# 5. Shell PATH setup helper
SHELL_RC=""
if [ -n "$SHELL" ]; then
    SHELL_NAME=$(basename "$SHELL")
    if [ "$SHELL_NAME" = "bash" ]; then
        SHELL_RC="$HOME/.bashrc"
    elif [ "$SHELL_NAME" = "zsh" ]; then
        SHELL_RC="$HOME/.zshrc"
    fi
fi

if [ -n "$SHELL_RC" ] && [ -f "$SHELL_RC" ]; then
    if ! grep -q '\.local/bin' "$SHELL_RC"; then
        echo "Updating $SHELL_RC to add \$HOME/.local/bin to PATH..."
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_RC"
        echo "✅ PATH updated. Please reload your shell or run: source $SHELL_RC"
    else
        echo "✅ \$HOME/.local/bin is already in your PATH inside $SHELL_RC"
    fi
fi

echo "🎉 LlamaScope successfully installed! Run it using: llamascope"
