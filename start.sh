#!/bin/bash
# Tars Inference Monitor Launcher v1.3
# Refactored for Textual TUI with real-time Plotext graphs.

APP_DIR="/home/stark/.tars/apps/inference-monitor"
VENV="$APP_DIR/venv"

echo "📡 Initializing Tars Inference Monitor (Textual Edition)..."

# Ensure venv exists
if [ ! -d "$VENV" ]; then
    echo "📦 Creating virtual environment..."
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install textual textual-plotext requests rich --quiet
fi

# Step 1: Run Logic Validation
echo "🧪 Running Logic Validation..."
cd "$APP_DIR"
"$VENV/bin/python3" validate_monitor.py
if [ $? -ne 0 ]; then
    echo "❌ Error: Logic validation failed. Monitor update blocked."
    exit 1
fi
echo "✅ Validation Passed."

# Step 2: Launch TUI
# Note: Using python3 from venv
"$VENV/bin/python3" monitor.py
