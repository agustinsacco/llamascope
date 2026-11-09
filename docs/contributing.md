# The contributor workshop

[← Home](../README.md) · [Architecture](architecture.md) · [Design system](design.md)

**Make the measurement more trustworthy before making it more impressive.** A missing value rendered honestly is better than a polished but invented number.

## Set up

Use a current stable Rust toolchain. CI installs stable Rust with Clippy and rustfmt. Python 3 is needed only for the standard-library-only Unix PTY and script smoke tests—not for the app.

```bash
rustup component add rustfmt clippy
cargo build --locked
cargo run --locked -- --demo
```

An active llama-server, model weights, AMD hardware, `rocm-smi`, tmux, Pillow, and Rich are **not** prerequisites for local development. Do not add them to the basic test path.

## Run the quality gates

From the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
python3 tests/terminal_smoke.py
bash -n start.sh && sh -n install.sh
python3 tests/scripts_smoke.py
```

The [Quality workflow](../.github/workflows/ci.yml) runs these gates on Linux and macOS. Before shipping a binary, also build release mode:

```bash
cargo build --locked --release
./target/release/llamascope --version
python3 tests/terminal_smoke.py target/release/llamascope
```

### What each layer proves

| Layer | Tests | What it catches |
| --- | --- | --- |
| Configuration / CLI | `tests/config.rs` | Precedence, invalid arguments, URL/key rejection, non-TTY safety, help/version. |
| Parsing and math | `tests/protocol.rs` | Object/legacy tokens, signed remaining counts, optional fields, task reuse, resets, saturating context arithmetic. |
| HTTP integration | `tests/protocol.rs` | Real loopback GETs, auth headers, loading vs errors, malformed JSON, partial failures, reconnection, timeouts, response bounds, props cache. |
| State and rendering | `tests/dashboard.rs` | Stable selection, scrolling, retained data, fresh/stale distinction, latest logs, both themes, extreme dimensions. |
| Contrast | `tests/dashboard.rs` | Every semantic text token on every theme surface. |
| Probe lifecycle | `src/hardware.rs` tests | Child timeout and stdout flooding without deadlock. |
| Real input / terminal | `tests/terminal_smoke.py` | Keys, freeze, resizing, loading/recovery, q/Ctrl-C, cursor, alternate screen, exact restored terminal attributes. |
| Shell contracts | `tests/scripts_smoke.py` | Arbitrary working directory, argument forwarding, custom prefix, unchanged shell files, propagated Cargo failures. |

The PTY test uses a small parser for the VT sequences emitted by Crossterm; pure rendering tests use Ratatui’s `TestBackend`. Neither replaces testing a real emulator and real hardware. The Python test keeps the PTY session leader alive while checking attributes because macOS revokes a PTY when that leader exits.

The launcher/installer smoke test uses a fake Cargo in a temporary HOME. It validates script behavior without installing binaries or changing your actual shell configuration. Test the real installer separately with a disposable `PREFIX` when modifying packaging behavior.

## Reproducible screenshots

The exporter renders the actual terminal buffer. SVG is generated directly, so there are no screenshot-library dependencies or developer-specific font paths.

```bash
# state, theme, columns, rows
cargo run --locked --example snapshot -- demo mocha 140 42 > docs/assets/dashboard-mocha.svg
cargo run --locked --example snapshot -- demo latte 140 42 > docs/assets/dashboard-latte.svg

# Review smaller/error views without overwriting the README assets.
cargo run --locked --example snapshot -- loading mocha 80 24 > /tmp/llamascope-loading.svg
cargo run --locked --example snapshot -- offline latte 80 24 > /tmp/llamascope-offline.svg
cargo run --locked --example snapshot -- help mocha 100 30 > /tmp/llamascope-help.svg
```

Supported states: `demo`, `connecting`, `loading`, `offline`, `degraded` (a retained-data outage), `empty`, `help`, `frozen`, and `hardware-missing`. The latter three demo-backed states still contain simulated readings. Dimensions are bounded to 300 columns and 100 rows.

Open SVG files in a browser. The screenshot font uses the viewer’s available monospace font, so glyph appearance may differ from your terminal. Demo values and event timestamps are reproducible; the displayed live age can vary slightly with execution time. Never edit measurements directly in a generated screenshot.

The old `screenshot.py` tmux/Pillow/Rich script has been replaced by this portable exporter.

## Add a protocol feature without guessing

1. Read the corresponding endpoint’s [upstream definition](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md#api-endpoints), and inspect a response from the server version you intend to support.
2. Reduce the response to a sanitized fixture. **Remove prompts, keys, usernames, private paths, and customer data.** Keep a source/version note in `tests/fixtures/README.md`.
3. Decide whether absence means “unavailable” or a real zero. Prefer `Option` unless the protocol defines a default.
4. Add parsing and error-path tests before changing display behavior.
5. Confirm how task reuse, idle states, missing identity, reconnects, and overflow affect the metric.
6. Add the field to the operator’s metric dictionary with units and limitations.
7. Render missing, first-sample, live, and stale states in both palettes.

If a server response is incompatible, improve the explanation; do not quietly accept it as an empty list just to make the dashboard look healthy.

## Design changes should carry evidence

Keep colors in `src/theme.rs`, not beside individual widgets. Review the [layout and interaction contract](design.md) before changing panels. Test keyboard actions in `AppState::handle_key` and through the PTY harness when they affect lifecycle or live collection.

For a visual PR, include:

- A generated screenshot in both themes.
- A compact-width screenshot or exact render-test coverage.
- The affected loading/error/empty states.
- The tests and build commands actually run.
- Any untested operating systems, GPU models, or server versions.

## Keep changes reviewable

Prefer one concern per PR: a protocol fix, a sensor adapter, an interaction improvement, or a documentation correction. Explain the defect and the observed behavior, not just the changed files. Avoid unrelated formatting or dependency upgrades. Commit the lockfile only when dependencies intentionally change.

### Before you say “done”

- [ ] Existing behavior remains supported or an intentional change is documented.
- [ ] Unknown readings still display as unknown; errors do not become success.
- [ ] Network/probe work remains off the UI thread and bounded.
- [ ] New messages cannot reveal credentials or raw response bodies.
- [ ] Render, protocol, CLI, and surrounding quality gates pass.
- [ ] Documentation commands were actually exercised where the environment permits.
- [ ] Real-server and real-hardware limitations are explicitly stated.

## Releasing

There is no manual release step. After every green [Quality](../.github/workflows/ci.yml) run on `main`, the [Release workflow](../.github/workflows/release.yml) builds static Linux (musl, x86_64 and arm64) and macOS binaries from that exact commit and publishes them as the latest GitHub release, with `SHA256SUMS` and `install.sh`. This mirrors Phosphor's continuous release.

- **Version:** `<major>.<minor>` from `Cargo.toml`, patch = number of commits on `main` (for example `1.1.57`). It always increases and needs no bump commit. Edit `Cargo.toml` only to change major or minor. Local builds report `Cargo.toml`'s `X.Y.0`, so `llamascope update` always offers the newest release.
- **Skip a release:** end the merge commit message with the trailer `Skip-Release: true`.
- **Retry:** run the Release workflow by hand on `main`. It republishes only if that commit's Quality run passed and its tag does not exist yet.

`install.sh` and `llamascope update` both read `llamascope-<target>` and `SHA256SUMS` from the release, so keep those asset names stable.
