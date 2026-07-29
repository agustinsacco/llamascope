#!/bin/bash
# LlamaScope Launcher
# High-performance Ratatui TUI dashboard for llama-server.

export PATH="$HOME/.cargo/bin:$PATH"
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "📡 Initializing LlamaScope TUI..."

# Compile if binary does not exist
if [ ! -f "$APP_DIR/target/release/llamascope" ]; then
    echo "📦 Building release binary..."
    cd "$APP_DIR"
    cargo build --release
    if [ $? -ne 0 ]; then
        echo "❌ Error: Compilation failed."
        exit 1
    fi
fi

# Launch TUI
"$APP_DIR/target/release/llamascope"

