# Consolidated Design Spec — btop-style Tars Inference Monitor TUI Redesign

**Task:** FN-015 (consolidates FN-005, FN-006, FN-007)
**Created:** 2026-05-28
**Status:** Authoritative reference for all downstream implementers

---

## Section 1 — Executive Summary

### Mission

This document provides the consolidated design specification for the btop-style TUI redesign of the Tars Inference Monitor application. It replaces the current sidebar/main-content split (`monitor.tcss`: 30% sidebar / 70% main) with a full-width three-row panel grid inspired by btop's dashboard aesthetic — dark background, Unicode box-drawing panel borders, color-coded data elements, and gradient-based value visualization.

### Target Visual Design

The target is a dark dashboard featuring:

- **Arc gauges** (Row 1) — Semi-circular arc displays for GPU load, memory, power, and temperature metrics
- **Horizontal meter bars** (Row 2) — Gradient-filled utilization bars for GPU core usage and per-slot TPS
- **Color-coded table** (Row 3a) — Extended DataTable with status indicators and btop-style keyboard navigation
- **Real-time log strip** (Row 3b) — Scrolling telemetry event log with btop palette colors
- **btop-style panel borders** — Unicode box-drawing characters with rounded corners
- **Gradient/threshold coloring** — Green→Yellow→Red severity scales matching btop's approach

> **⚠️ Critical Note — Arc Gauges Are Tars-Specific:**
> btop does NOT render arc gauges. btop renders horizontal bar meters using `■` (U+25A0) characters. Arc gauges are a **Tars-specific visual enhancement** layered on top of btop's panel layout philosophy. This distinction is important for understanding what btop natively provides versus what the Tars redesign adds as an enhancement.

### Consolidated Research Sources

This spec synthesizes three sibling research deliverables:

| Sibling Task | Deliverable | Role in This Spec |
|---|---|---|
| **FN-005** | `research-btop-rendering.md` | Rendering research: Unicode `Symbols` namespace character names, btop's `Draw::Meter` pipeline (horizontal bar meters), `Draw::Graph` structure (braille interpolation), `createBox()` panel border system |
| **FN-006** | `btop-color-palette.md` | Complete btop Default theme palette: all 48 `Default_theme` color keys with expanded hex codes, gradient interpolation logic (50/50 split), TTY fallback codes, Tars adaptation mappings |
| **FN-007** | Layout Grid Specification (from PROMPT.md, Steps 1–7) | Three-row grid layout: panel dimensions at three breakpoints, spacing conventions, responsive rules, widget-to-panel mapping, container hierarchy |

---

## Section 2 — Widget Behavior Specifications

### ArcGauge Widget (Row 1 — Arc Gauge Panel)

**Purpose:** Display GPU metrics (load, memory, power, temperature) as semi-circular arc gauges.

**Layout:** 4 gauges in Row 1, equal width, each in its own panel with btop-style border.

**Parameters:**
- `title` (str) — Metric name (e.g., "GPU Load", "Memory", "Power", "Temperature")
- `value` (float) — Current metric value
- `min` (float) — Minimum scale value
- `max` (float) — Maximum scale value
- `unit` (str) — Unit label (e.g., "%", "GB", "W", "°C")
- `gradient_key` (str) — Gradient key from FN-006 palette

**Display Elements:**
1. Metric label (top of gauge)
2. Numeric value (center, large bold, `main_fg` color)
3. Unit label (bottom of gauge area)
4. Gradient arc fill (semi-circular arc using Tars-specific Unicode fill characters)
5. Mini time-series graph below arc (braille interpolation from FN-005's `Draw::Graph`)

**Rendering Approach:** Override Textual `render()` to draw arc using Unicode characters. Arc outline uses horizontal/vertical line characters (`─` U+2500, `│` U+2502) from FN-005's `Symbols` namespace. Arc fill uses the Tars-specific block characters (`▂`/`▃`/`▄`/`▅`/`▆`/`▇`/`█`, U+2580–U+258F) — **these are NOT from btop's `Symbols` namespace** (btop does not render arc gauges).

**Color Mapping:**
| Arc Gauge | Gradient Key | Gradient (FN-006) |
|---|---|---|
| GPU Load | `cpu` | `#77ca9b` → `#cbc06c` → `#dc4c4c` |
| Memory | `used` | `#592b26` → `#d9626d` → `#ff4769` |
| Power | `cpu` | `#77ca9b` → `#cbc06c` → `#dc4c4c` |
| Temperature | `temp` | `#4897d4` → `#5474e8` → `#ff40b6` |

**Size:**
- Standard (120-column): 12 rows
- Minimum (80-column): 8 rows (arranged in 2×2 grid)

**References:** FN-005 (rendering approach, Unicode character reference), FN-006 (color gradient triplets), FN-007 (panel placement, dimensions)

---

### MeterBar Widget (Row 2 — Utilization Bars Panel)

**Purpose:** Display horizontal bar meters for GPU core utilization and per-slot TPS.

**Layout:** Two sub-panels in Row 2 — GPU cores (left, ~60% width) and per-slot TPS (right, ~40% width).

**Parameters:**
- `label` (str) — Bar label (e.g., "Core 0", "Slot 0")
- `value` (float) — Current value
- `max` (float) — Maximum scale value
- `gradient_key` (str) — Gradient key from FN-006 palette
- `max_bars` (int) — Maximum number of bars to display

**Rendering Approach:** Override Textual `render()` to draw horizontal bar using `■` (U+25A0 FULL SQUARE) from FN-005's `Symbols::meter`. Fill portion uses gradient color from FN-006 palette; empty portion uses `meter_bg` color (`#404040`).

**Text Layout:**
- Label on left, value percentage on right
- Both use `main_fg` color (`#cccccc`)

**Size:**
- Standard (120-column): 16 rows
- Minimum (80-column): 8 rows (scrollable if bars exceed visible area)

**Gradient Assignment Justification:** Both GPU core bars and TPS bars use the `cpu` gradient (`#77ca9b` → `#cbc06c` → `#dc4c4c`) because:
1. btop uses the `cpu` gradient for CPU usage bars representing "utilization" — the same semantic domain as GPU core utilization
2. TPS is also a "throughput/utilization" metric that benefits from the green→yellow→red severity scale
3. Using a single gradient for both bars maintains visual consistency in Row 2
4. The `cpu_box` accent (`#556d59`) is used for the panel header background

**References:** FN-005 (btop `Draw::Meter` pipeline, Unicode `■` character), FN-006 (gradient colors, `meter_bg`), FN-007 (panel placement, dimensions)

---

### ExtendedDataTable Widget (Row 3a — Table Panel)

**Purpose:** Extended `DataTable` with btop-style color-coded rows, status indicators, and keyboard navigation.

**Columns:** ID, Status (color-coded), Task, Progress, TPS, KV Cache

**Color Coding:**
| Status | Color Key | Hex | Visual |
|---|---|---|---|
| BUSY | `hi_fg` | `#b54040` | Red text for active/busy slots |
| IDLE | `inactive_fg` | `#404040` | Dim text for idle slots |
| Selected Row | `selected_bg` | `#6a2f2f` | Dark red background highlight |
| Selected Row Text | `selected_fg` | `#eeeeee` | White text on selected row |

**Keyboard Navigation:** btop-style arrow key navigation, enter to select, q to quit. Specific key bindings:
- `↑`/`↓` — Navigate rows
- `←`/`→` — Navigate columns
- `Enter` — Select current row, show slot detail
- `q` — Quit application
- `c` — Clear logs (existing binding)

**Size:**
- Standard (120-column): **15 rows**
- Minimum (80-column): 8 rows

**References:** FN-006 (status color codes), FN-007 (panel placement, dimensions)

---

### RichLog Widget (Row 3b — Log Strip)

**Purpose:** Real-time telemetry event logging — **unchanged structure but updated color styling** (current app uses `[dim]`/`[yellow]` markup; new spec uses btop palette keys).

**Position:** Right of table in Row 3, docked at bottom of terminal within a shared `Horizontal` container.

**Size:**
- Standard (120-column): **15 rows** (shared height with ExtendedDataTable)
- Minimum (80-column): 8 rows (shared height with ExtendedDataTable)

**Text Styling:**
| Message Type | Color Key | Hex |
|---|---|---|
| Timestamps | `inactive_fg` | `#404040` |
| Info messages | `main_fg` | `#cccccc` |
| Warnings/Errors | `hi_fg` | `#b54040` |
| Axis labels / secondary info | `graph_text` | `#606060` |

**References:** FN-006 (color codes), FN-007 (panel placement)

---

### Header & Footer (unchanged from current app)

**Header:**
- **3 rows at standard/large** terminal sizes
- **2 rows at minimum (80-column) breakpoint**
- Docked top, Textual built-in `Header` widget
- **Responsive height adjustment:** The minimum-breakpoint header (2 rows) requires programmatic height adjustment via `on_resize` since Textual's current `Header` widget height is fixed in CSS (`monitor.tcss` sets `Header { height: 3; }`). Implement either a Textual `@media (min-width: 80)` query or override `on_resize` to call `self.query_one(Header).height = 2` when terminal is below standard size.

**Footer:**
- 1 row, docked bottom, Textual built-in `Footer` widget

---

## Section 3 — Data Binding Architecture

### Data Flow Overview

```
TelemetryEngine.update_telemetry() [2 Hz]
  ├── get_gpu_data() → GPU metrics
  │     ├── usage    → ArcGauge (GPU Load, gradient_key=`cpu`)
  │     ├── temp     → ArcGauge (Temperature, gradient_key=`temp`)
  │     ├── power    → ArcGauge (Power, gradient_key=`cpu`)
  │     └── mem_used → ArcGauge (Memory, gradient_key=`used`)
  ├── get_inference_data() → Inference slot data
  │     ├── Per-slot TPS → MeterBar widgets (gradient_key=`cpu`)
  │     └── Slot list    → ExtendedDataTable (columns: ID, Status, Task, Progress, TPS, KV)
  └── Event logging    → RichLog (info/warn/error messages)
```

### Data Types Per Widget

| Widget | Data Type | Source |
|---|---|---|
| ArcGauge | `float` value (usage, temperature, power, memory used); gradient key from FN-006 palette | `TelemetryEngine.get_gpu_data()` returns `{usage, temp, power, mem_total, mem_used}` |
| MeterBar | `{label: str, value: float, max: float, gradient_key: str}` for GPU core bars | `TelemetryEngine.get_gpu_data()` for core utilization |
| MeterBar | `{label: str, value: float (TPS), max: float, gradient_key: str}` for slot TPS bars | `TelemetryEngine.get_inference_data()` returns `{slots: [{id, busy, task, decoded, remain, tps, kv}], queue}` |
| ExtendedDataTable | List of slot dicts from `get_inference_data()` | Fields: `id`, `busy`, `task`, `decoded`, `remain`, `tps`, `kv` |
| RichLog | `{level: str, message: str}` formatted messages | `log_info()` and `log_warn()` methods |

### Update Frequency

- 2 Hz (0.5s interval) via `set_interval(1/REFRESH_RATE, self.update_telemetry)`
- Matches current `REFRESH_RATE = 2.0` in `monitor.py`

### Data Buffer

- GPU metrics maintain 60-second history (120 samples) for mini-graph display in ArcGauge
- Port from existing `GraphWidget.data` deque in `monitor.py` (already uses `deque(maxlen=MAX_BUFFER)` where `MAX_BUFFER = 120`)

### Error Handling

- Per FN-005 research, poll failures return `None` and log warning
- UI must handle `None` gracefully — show `--` or dimmed values instead of crashing
- Existing pattern in `monitor.py`: `if gpu:` check before updating widgets; else log warning

---

## Section 4 — Animation & Refresh Expectations

### Gauge Arc Fill Animation

Arc gauges should smoothly transition between values — no instant snap. Use interpolation over ~200ms for visual continuity. **This is a Tars-specific enhancement** — btop does not animate arc gauges because it does not render them.

### Meter Bar Animation

Horizontal bars should smoothly expand/shrink as values change. Use Textual `animate` or `set_timer` for smooth value transitions. **This is a Tars-specific enhancement** — btop's native meter bars update instantly on each refresh cycle.

### Refresh Cycle

- All widgets update on the 2 Hz telemetry refresh cycle
- The `on_resize` handler (btop's `Term::refresh()` equivalent) recalculates panel dimensions and triggers widget repaints
- Implement `on_resize` in `TarsMonitor` to recalculate panel dimensions and trigger repaints

### Mini-Graph Update

- ArcGauge mini-graphs update every tick (2 Hz)
- Maintain a deque buffer of 120 data points (60 seconds at 2 Hz)
- Match existing `GraphWidget` approach from `monitor.py`
- Use braille interpolation for smooth line rendering (FN-005's `Draw::Graph` approach)

### Table Scroll

- When table rows exceed visible area, DataTable handles scrolling natively in Textual

### Log Strip

- RichLog auto-scrolls to newest entries
- Does NOT auto-scroll on manual navigation

---

## Section 5 — Terminal Compatibility Requirements

### Terminal Sizes

| Breakpoint | Columns | Rows | Layout Mode |
|---|---|---|---|
| **A — Minimum** | 80 | 30 | 2×2 gauge grid, 50/50 GPU/TPS split |
| **B — Standard** | 120 | 48 | 1×4 gauge row, 60/40 GPU/TPS split |
| **C — Large** | 160+ | 60+ | Wider gauges, more rows in utilization/table |

### Unicode Font Support

Terminal must support:
- Unicode box-drawing characters (U+2500–U+257F)
- Full block character (U+25A0) — FN-005's `Symbols::meter`
- Braille patterns (U+2800–U+28FF) — for mini-graphs
- Arc-fill characters (U+2580–U+258F, Tars-specific) — not from btop

### True Color Support

Required for btop palette rendering (24-bit colors). Detect via `$TERM` environment variable and `TERM_PROGRAM` for fallback to ANSI mode if true color unavailable.

### Terminal Resize Handling

- Implement `on_resize` in `TarsMonitor` to recalculate panel dimensions and trigger repaints
- Equivalent to btop's `Term::refresh()` in `btop.cpp`
- Do NOT crash on resize — gracefully adapt layout
- Per FN-007 responsive rules: Row 2 (utilization bars) is the only panel that compresses when total height exceeds terminal size

### C0/C1 Control Character Support

Terminal must handle ANSI escape codes for color (e.g., `\x1b[38;2;R;G;Bm` for 24-bit color, `\x1b[0m` for reset). Textual handles this automatically on modern terminals.

---

## Section 6 — Textual Implementation Guide

### Container Hierarchy

```
App (TarsMonitor)
├── Header (docked top, 3 rows / 2 rows min)
├── Container (id="grid-main")
│   ├── Container (id="row1-gauges")
│   │   ├── Container (id="gauge-load")   → ArcGauge
│   │   ├── Container (id="gauge-memory")  → ArcGauge
│   │   ├── Container (id="gauge-power")   → ArcGauge
│   │   └── Container (id="gauge-temp")    → ArcGauge
│   ├── Container (id="row2-bars")
│   │   ├── Container (id="gpu-bars")      → GPU core MeterBar widgets
│   │   └── Container (id="tps-bars")      → Slot TPS MeterBar widgets
│   └── Horizontal (id="row3-data")        ← Horizontal container for side-by-side layout
│       ├── DataTable (id="slots-table")   → ExtendedDataTable
│       └── RichLog (id="logs")
└── Footer (docked bottom, 1 row)
```

**Key change from FN-007:** Row 3 uses a `Horizontal` container to place the DataTable and RichLog side-by-side (table ~60% width, logs ~40%). Both widgets share the same height (15 rows at standard, 8 rows at minimum). This resolves the FN-007 ambiguity where the two widgets' separate heights (8 rows + 7 rows) didn't add up to a single container height.

### CSS Architecture Recommendations

- Use CSS custom properties (`--btop-*`) on `:root` or `Screen` for the btop palette (from FN-006)
- Panel borders use Textual `border` property with Unicode characters or `border-style: round`
- Use `fr` (fractional) units for flexible-width panels, `height: N` for fixed-height panels
- Gap spacing between panels: use `margin` or `gap` properties in Textual containers
- **WCAG AA compliance:** All new border colors must meet ≥3:1 contrast ratio for non-text elements. Use `#666` for outer panel borders on `#0a0a0a` background (per FN-007 contrast requirements, not btop's native `#303030` which fails WCAG). See `.fusion/memory/MEMORY.md` WCAG Contrast Audit for details.

### Textual Widget Implementation Notes

- Custom widgets (`ArcGauge`, `MeterBar`) must override `render()` to draw Unicode characters cell-by-cell
- Use `self.set_timer()` or `self.animate()` for smooth value transitions
- Use `Container.compose()` for nested panel structure
- Monitor terminal size via `self.size` in `TarsMonitor.on_resize()`
- **Responsive header height:** The minimum-breakpoint header (2 rows) requires programmatic height adjustment via `on_resize` since Textual's current `Header` widget height is fixed in CSS (`monitor.tcss` sets `Header { height: 3; }`).

---

## Section 7 — Integration Notes & Known Discrepancies

### Sibling Task Integration

**FN-005 → Design Spec:** Rendering research provides the Unicode character set (`Symbols` namespace: `h_line`, `v_line`, `left_up`, `right_up`, `left_down`, `right_down`, `round_left_up`, `round_right_up`, `round_left_down`, `round_right_down`, `meter`, `graph_symbols`), `■` U+25A0 from `Symbols::meter`, meter bar rendering pipeline (btop's `Draw::Meter` approach), graph rendering approach (braille interpolation via `Draw::Graph`), and panel border system (`createBox()`). The spec's widget implementations must use these Unicode characters and rendering patterns.

**FN-006 → Design Spec:** Color palette provides all hex codes applied to widgets: `div_line` for borders, `main_bg` for backgrounds, `main_fg` for primary text, `title` for panel headers, gradient triplets (`cpu`, `temp`, `used`, `free`) for gauge fills, `hi_fg` for status highlights, `inactive_fg` for idle elements, `graph_text` for axis labels, `cpu_box` for panel accents. The spec maps these palette keys to specific widget elements.

**FN-007 → Design Spec:** Layout grid provides panel structure (3-row grid), exact dimensions at breakpoints, spacing conventions, responsive rules, and container hierarchy. The spec integrates these dimensional values into widget size specifications.

### Known Discrepancies (FN-015 Resolutions)

| FN-007 Claim | FN-015 Resolution | Rationale |
|---|---|---|
| Row 3a (table) = 8 rows, Row 3b (log) = 7 rows, separate containers | Row 3 container = 15 rows, both widgets share height inside `Horizontal` | FN-007 showed table and log as "side by side" but listed separate heights. Side-by-side widgets in a shared container must have equal height. 8+7=15 rows total for the combined Row 3 panel at standard breakpoint. |
| Minimum Row 3a = 5 rows, Row 3b = 3 rows | Minimum Row 3 container = 8 rows, both widgets share height | Same resolution: both widgets at 8 rows in a `Horizontal` container at minimum breakpoint. |
| Breakpoint C total height = 66 rows in a 60-row terminal | Breakpoint C: Row 2 compresses to 22 rows (from 28) when terminal is 60 rows. Total = 3+14+22+10+10+1 = 60 rows exactly. | The spec's responsive rules state that Row 2 (utilization bars) is the only panel that compresses when total height exceeds terminal. Row 2 supports scrolling anyway, so 22 rows with vertical scroll is the fallback. |
| Header is 3 rows at all breakpoints | Header is 3 rows at standard/large, 2 rows at minimum | Per FN-007 Breakpoint A, the header shrinks to 2 rows. This spec adopts that adaptation. |

---

## Section 8 — Quick Reference Appendix

### Widget Summary

| Widget | Panel | Std Height | Min Height | Std Width | Min Width | Gradient Key(s) | Key Unicode |
|---|---|---|---|---|---|---|---|
| ArcGauge | Row 1 | 12 rows | 8 rows | ~27 cols (1/4 row) | ~36 cols (1/2 col in 2×2) | cpu, temp, cpu, used | Arc: U+2580–U+258F (Tars) |
| MeterBar | Row 2 sub-panel | 16 rows | 8 rows (scrollable) | 70 cols (GPU), 48 cols (TPS) | 42 cols, 36 cols | cpu (both bars) | ■ U+25A0 (FN-005) |
| ExtendedDataTable | Row 3a | 15 rows | 8 rows | ~60% of row 3 | ~60% of row 3 | hi_fg, inactive_fg, selected_bg | — (uses Textual DataTable) |
| RichLog | Row 3b | 15 rows (shared) | 8 rows (shared) | ~40% of row 3 | ~40% of row 3 | inactive_fg, main_fg, hi_fg, graph_text | — (uses Textual RichLog) |
| Header | Top | 3 rows | 2 rows | full | full | — | — |
| Footer | Bottom | 1 row | 1 row | full | full | — | — |

### Panel Dimension Summary (Standard 120×48 terminal)

| Panel | Height | Width | Notes |
|---|---|---|---|
| Header | 3 rows | full | Top docked |
| Row 1: Arc Gauges | 12 rows | full | 4 gauges, 27 cols each + 1-col gaps |
| Row 2: Utilization Bars | 16 rows | full | GPU bars (70 cols) + TPS bars (48 cols) + 1-col gap |
| Row 3: Table + Log | 15 rows | full | Horizontal container: DataTable (60%) + RichLog (40%) |
| Footer | 1 row | full | Bottom docked |
| **Total** | **47 rows** | | Fits in 48-row terminal (1 row margin) |

### Complete Color Palette Reference (all 48 Default_theme keys with expanded hex)

#### Base Colors (15 entries)

| Palette Key | btop Hex | Expanded Hex | Semantic Role | Used By |
|---|---|---|---|---|
| `main_bg` | `#00` | `#000000` | Panel/Screen background | All panels (project adapts to `#0a0a0a` per monitor.tcss baseline) |
| `main_fg` | `#cc` | `#cccccc` | Primary text | All text, metric values, panel content |
| `title` | `#ee` | `#eeeeee` | Panel titles | Panel headers, widget titles |
| `hi_fg` | `#b54040` | `#b54040` | Highlight/alert text | Warning status, busy slots, error log messages |
| `selected_bg` | `#6a2f2f` | `#6a2f2f` | Selected row background | ExtendedDataTable selected row |
| `selected_fg` | `#ee` | `#eeeeee` | Selected row text | ExtendedDataTable selected row text |
| `inactive_fg` | `#40` | `#404040` | Inactive/decorative | Idle status, log timestamps, dimmed text |
| `graph_text` | `#60` | `#606060` | Axis labels, dim text | Log axis labels, secondary info text (btop expands `#60` → `#606060` via 2-digit hex repetition) |
| `meter_bg` | `#40` | `#404040` | Meter fill background | Empty portion of MeterBar widgets |
| `proc_misc` | `#0de756` | `#0de756` | Process/misc highlights | Bright green for notable values (future use) |
| `cpu_box` | `#556d59` | `#556d59` | CPU panel accent | Panel header backgrounds in Row 2 |
| `mem_box` | `#6c6c4b` | `#6c6c4b` | Memory panel accent | Memory-related panel headers (future use) |
| `net_box` | `#5c588d` | `#5c588d` | Network panel accent | Network panel headers (future use, not in current scope) |
| `proc_box` | `#805252` | `#805252` | Process panel accent | Process panel headers (future use, not in current scope) |
| `div_line` | `#30` | `#303030` | Panel dividers | Panel borders (project adapts to `#666` for WCAG compliance) |

#### Process Banner Colors (6 entries)

| Palette Key | btop Hex | Expanded Hex | Semantic Role | Used By |
|---|---|---|---|---|
| `proc_pause_bg` | `#b54040` | `#b54040` | Process list pause banner bg | Not used (future process panel) |
| `proc_follow_bg` | `#4040b5` | `#4040b5` | Process list follow banner bg | Not used (future process panel) |
| `proc_banner_bg` | `#7b407b` | `#7b407b` | Process combined banner bg | Not used (future process panel) |
| `proc_banner_fg` | `#ee` | `#eeeeee` | Process banner text | Not used (future process panel) |
| `followed_bg` | `#4040b5` | `#4040b5` | Followed process background | Not used (future process panel) |
| `followed_fg` | `#ee` | `#eeeeee` | Followed process text | Not used (future process panel) |

#### Gradient Triplets (9 triplets = 27 entries)

| Gradient | Start Hex | Mid Hex | End Hex | Semantic Use | Used By |
|---|---|---|---|---|---|
| `cpu` (CPU/utilization) | `#77ca9b` | `#cbc06c` | `#dc4c4c` | Green→Yellow→Red | GPU load ArcGauge, GPU core MeterBar, TPS MeterBar |
| `temp` (temperature) | `#4897d4` | `#5474e8` | `#ff40b6` | Blue→Violet→Pink | Temperature ArcGauge |
| `free` (free memory) | `#384f21` | `#b5e685` | `#dcff85` | Dark→Light green→Pale green | Memory free gauge (future use) |
| `cached` (cached memory) | `#163350` | `#74e6fc` | `#26c5ff` | Dark→Light blue→Cyan | Cached memory gauge (future use) |
| `available` (available memory) | `#4e3f0e` | `#ffd77a` | `#ffb814` | Dark yellow→Golden→Gold | Available memory gauge (future use) |
| `used` (used memory) | `#592b26` | `#d9626d` | `#ff4769` | Dark red→Rose→Red | Memory used ArcGauge |
| `download` (network down) | `#291f75` | `#4f43a3` | `#b0a9de` | Dark→Medium→Light purple | Network gauge (future use, not in current scope) |
| `upload` (network up) | `#620665` | `#7d4180` | `#dcafde` | Dark→Medium→Light magenta | Network gauge (future use, not in current scope) |
| `process` (process list) | `#80d0a3` | `#dcd179` | `#d45454` | Green→Yellow→Red | Process bar (future use, not in current scope) |

> **Note on gradient interpolation:** btop uses a 50/50 split — values 0–50 interpolate start→mid, values 50–100 interpolate mid→end. This is btop's native behavior.

**Traceability note:** All 48 `Default_theme` color keys and hex values in this table are sourced from FN-006's `btop-color-palette` task document. If any value appears questionable, cross-reference FN-006's source (btop `src/btop_theme.cpp` at `https://raw.githubusercontent.com/aristocratos/btop/main/src/btop_theme.cpp`) for authoritative values.

**Gradient keys currently NOT used in the btop-style redesign (excluded by design):** `free`, `cached`, `available`, `download`, `upload`, `process` — these 6 gradients are not mapped to any current Tars UI element. They may be used in future tasks that add memory breakdown panels, network monitoring, or process-detail panels.

### Braille Character Set for Mini-Graphs (from btop `Symbols::graph_symbols`)

| Symbol Key | Characters | Purpose |
|---|---|---|
| `braille_up` | U+2800–U+28FF (25 cells) | Braille line interpolation for upward graph segments |
| `braille_down` | U+2800–U+28FF (25 cells) | Braille line interpolation for downward graph segments |
| `block_up` | U+2580–U+258F (25 cells) | Upper-half block characters for upward segments |
| `block_down` | U+2580–U+258F (25 cells) | Lower-half block characters for downward segments |
| `tty_up` | 25 control characters | TTY-mode fallback for upward segments |
| `tty_down` | 25 control characters | TTY-mode fallback for downward segments |

ArcGauge mini-graphs use `block_up`/`block_down` sets for smooth line rendering across adjacent data points. The 25-cell braille interpolation maps pairs of adjacent values to select one of 25 characters (result[0] * 5 + result[1]).

### Unicode Character Reference

| Character | Unicode | Name | Source | Used For |
|---|---|---|---|---|
| `■` | U+25A0 | FULL SQUARE | FN-005 `Symbols::meter` | Meter bar fill (btop-native) |
| `─` | U+2500 | BOX DRAWINGS LIGHT HORIZONTAL | FN-005 `Symbols::h_line` | Arc outline, panel borders |
| `│` | U+2502 | BOX DRAWINGS LIGHT VERTICAL | FN-005 `Symbols::v_line` | Arc outline, panel borders |
| `▂` | U+2582 | UPPER HALF BLOCK | Tars-specific | Arc fill (Tars enhancement) |
| `▃` | U+2583 | THREE-QUARTERS UPPER HALF BLOCK | Tars-specific | Arc fill |
| `▄` | U+2584 | LOWER HALF BLOCK | Tars-specific | Arc fill |
| `▅` | U+2585 | THREE-QUARTERS LOWER HALF BLOCK | Tars-specific | Arc fill |
| `▆` | U+2586 | FIVE-EIGHTHS BLOCK | Tars-specific | Arc fill |
| `▇` | U+2587 | SEVEN-EIGHTHS BLOCK | Tars-specific | Arc fill |
| `█` | U+2588 | FULL BLOCK | Tars-specific | Arc fill (100% filled arc) |
| `┌` `┐` `└` `┘` | U+250C–U+2510 | BOX DRAWINGS CORNERS | FN-005 `Symbols::left_up` etc. | Panel border corners |
| `╭` `╮` `╰` `╯` | U+256D–U+2570 | ROUNDED CORNERS | FN-005 `Symbols::round_left_up` etc. | Rounded panel borders |

---

*End of Consolidated Design Spec. This document is the authoritative reference for all downstream implementers building the btop-style Tars Inference Monitor interface.*
