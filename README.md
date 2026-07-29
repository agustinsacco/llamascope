# 📡 LlamaScope

A high-performance, terminal-native developer dashboard and visualization extension for `llama-server`. Built in Rust using [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm).

LlamaScope helps developers inspect, analyze, and optimize their local LLM inference pipelines by visualizing scheduling slots, prompt pre-fills, token generation speed (TPS), and resource utilization in real-time.

---

## ✨ Features

- **🚀 Deep Inference Telemetry**:
  - Track parallel slot utilization.
  - Monitor prompt ingestion progress (`[Pre-fill]` stage: count / percentage).
  - Track real-time token-per-second (`[Decoding]` speed) for each request slot.
  - Monitor slot-level KV Cache capacity usage (`(prompt_processed + decoded) / n_ctx`).
- **📈 Advanced System & GPU Metrics**:
  - Unified view of Host RAM vs GPU VRAM history plots.
  - Live GPU compute load history plots.
  - Real-time power draw and core temperature tracking (via `rocm-smi`).
- **🎯 Interactive Multi-Slot View**:
  - Real-time double line charts plotting TPS and KV Cache history for active requests.
  - Cycle between active slot graphs instantly with the `Tab` key.
- **🖥️ Server Metadata Bar**: Live summary showing current model alias, connection status, slot configuration, queue size, and llama.cpp build.
- **📋 Diagnostics Console**: Real-time warning and diagnostic logging info with clean ANSI coloration.
- **🎨 Catppuccin Mocha Aesthetics**: Modern slate/lavender theme with clean rounded borders.

---

## ⚙️ How LlamaScope Helps Optimize Inference

1. **Pre-fill Ingestion Bottlenecks**: See if prompt pre-fill ingestion is taking too long due to heavy prompt lengths, helping you optimize context-shifting.
2. **Context Bounds & Limits**: View the exact percentage of your slot's KV Cache context usage. Know when your request is close to exceeding `n_ctx`.
3. **Queue Length**: Monitor the queue size to see if requests are piling up, suggesting you need to allocate more slot count.
4. **Thermal / Power Throttling**: Track GPU core temp and power draw alongside TPS to spot hardware throttling.

---

## 🛠️ Build & Installation

### Prerequisites
- Rust and Cargo (installed automatically if running launcher)
- Active `llama-server` (defaults to `http://127.0.0.1:8086`)
- `rocm-smi` utility (for AMD GPU telemetry)

### Installation

To compile and install LlamaScope globally to your user bin directory (`~/.local/bin`), run the automated installer script:

```bash
chmod +x install.sh
./install.sh
```

This will compile LlamaScope in release mode, copy the binary to your path, and update your shell config (`.bashrc` / `.zshrc`) if necessary.

### Manual Compilation

Alternatively, you can compile LlamaScope directly using Cargo:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/llamascope`.

---

## 🚀 Running LlamaScope

### Option 1: Automatic Launcher Script
Run the launcher script to compile and run LlamaScope automatically:

```bash
chmod +x start.sh
./start.sh
```

### Option 2: Running Directly
Set the server URL using the `LLAMA_API_URL` environment variable:

```bash
export LLAMA_API_URL="http://127.0.0.1:8086"
./target/release/llamascope
```

---

## 🎮 Controls & Keybindings

| Key | Action |
| --- | --- |
| `q` | Safe Exit / Quit |
| `c` | Clear Diagnostics Log Panel |
| `Tab` | Cycle Selected Slot performance graph |
