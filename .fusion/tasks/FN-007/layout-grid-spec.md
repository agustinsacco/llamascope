# Layout Grid Specification — Tars Inference Monitor

> **Task:** FN-007 | **Size:** S | **Review Level:** 1 (Plan Only)
> **Created:** 2026-05-27
> **Dependencies:** FN-005 (btop Rendering Architecture), FN-006 (btop Color Palette)

---

## Section 1: Current Layout Reference

### 1.1 Existing Layout Architecture

The current Tars Inference Monitor uses a **horizontal split layout** with two main columns:

```
┌────────────────────────────────────────────────────────────┐
│ Header (3 rows, docked top, #0047ab blue background)      │
├──────────┬─────────────────────────────────────────────────┤
│          │                                                 │
│ #sidebar │              #main-content                      │
│  30%     │                 70%                             │
│          │                                                 │
│ ┌──────┐ │ ┌───────────────────────────────────────────┐  │
│ │MEM   │ │ │ DataTable (10 rows fixed height)          │  │
│ │graph │ │ │ ID │ Status │ Task │ Progress │ TPS │ KV  │  │
│ └──────┘ │ └───────────────────────────────────────────┘  │
│          │                                                 │
│ ┌──────┐ │ ┌───────────────────────────────────────────┐  │
│ │GPU   │ │ │ #active-slots (1fr, scrollable)           │  │
│ │graph │ │ │ SlotDetail cards with TPS/KV mini-graphs   │  │
│ └──────┘ │ └───────────────────────────────────────────┘  │
│          │                                                 │
│ ┌──────┐┌──────┐                                         │
│ │POWER ││TEMP  │                                         │
│ │graph ││graph │                                         │
│ └──────┘└──────┘                                         │
│          │                                                 │
│          │ ┌───────────────────────────────────────────┐  │
│          │ │ #logs (RichLog, 8 rows, docked bottom)   │  │
│          │ └───────────────────────────────────────────┘  │
├────────────────────────────────────────────────────────────┤
│ Footer (1 row, docked bottom, #1a1a1a background)          │
└────────────────────────────────────────────────────────────┘
```

### 1.2 Layout Details from `monitor.tcss`

| Element | CSS Properties | Dimensions |
|---------|---------------|------------|
| `Header` | `dock: top`, `height: 3`, `background: #0047ab`, `color: white`, `text-style: bold` | 3 rows fixed, full width |
| `#sidebar` | `width: 30%`, `height: 100%`, `dock: left`, `border: panel #333` | 30% width, full height |
| `#main-content` | `width: 70%`, `height: 100%`, `border: panel #333` | 70% width, full height |
| `#logs` | `height: 8`, `dock: bottom`, `border-top: panel #333`, `background: #050505` | 8 rows fixed |
| `DataTable` | `height: 10`, `border: round #222` | 10 rows fixed |
| `#active-slots` | `height: 1fr`, `overflow-y: scroll` | Flexible remaining height |
| `#sidebar .widget-container` | `height: auto`, `margin-bottom: 1`, `padding: 1`, `border: round #222` | Auto height, 1-cell padding |
| `#sidebar .half-width` | `width: 50%` | 50% of parent |
| `Footer` | `dock: bottom`, `height: 1`, `background: #1a1a1a`, `color: #666` | 1 row fixed |

### 1.3 Widget Assignments

**Sidebar (#sidebar, docked left, 30% width):**
- `Static` "MEMORY: X.X / X.X GB" text label
- `GraphWidget` `mem_plot` — Unified Memory (GB) line graph
- `Static` "GPU LOAD: X%" text label
- `GraphWidget` `gpu_load_plot` — GPU Compute Load (%) line graph
- `GraphWidget` `power_plot` — Power Draw (W) line graph
- `GraphWidget` `temp_plot` — Core Temperature (°C) line graph

**Main Content (#main-content, remaining 70% width):**
- `DataTable` (id="slots-table") — Inference slot status table, 10 rows, 6 columns (ID, Status, Task, Progress, TPS, KV)
- `#active-slots` — Container for dynamic `SlotDetail` widgets, each containing:
  - `Static` slot title (e.g., "Slot 0 Metrics")
  - `GraphWidget` TPS time-series graph
  - `GraphWidget` KV Cache time-series graph
- `RichLog` (id="logs") — Scrolling log output, 8 rows, docked at bottom

**Header & Footer:**
- `Header` (Textual built-in) — 3 rows, blue background (#0047ab), white bold text
- `Footer` (Textual built-in) — 1 row, dark background (#1a1a1a), dim text

### 1.4 Border Style

- Widget containers: `round #222` (rounded corners, dark gray)
- Sidebar/main panels: `panel #333` (panel borders, slightly lighter)
- Logs panel top border: `panel #333`
- DataTable: `round #222` (rounded corners)
- Internal spacing: `padding: 1`, `margin-bottom: 1`

### 1.5 Color Palette (Current, from `monitor.tcss`)

| Element | Color | Source |
|---------|-------|--------|
| Screen background | `#0a0a0a` | Project theme |
| Header background | `#0047ab` | Project-specific blue |
| Header text | `white` | High contrast |
| Footer background | `#1a1a1a` | Derived from div_line |
| Footer text | `#666` | Approximation |
| Sidebar/main border | `#333` | Panel borders |
| Widget container border | `#222` | Darker container borders |
| Logs background | `#050505` | Darker than screen |
| Logs border | `#333` | Panel borders |

> **Note:** The btop-style redesign will replace this palette with the FN-006 color scheme. The current colors serve only as baseline documentation.

---

## Section 2: Target Layout at ≥52 Rows

### 2.1 Design Philosophy

The target layout replaces the current 30/70 sidebar/main-content split with a **full-width three-row panel structure** inspired by btop's dashboard panels:

```
Row 1 — Arc Gauges Panel  (4 GPU metrics as large arc gauges)
Row 2 — Utilization Bars  (GPU core utilization + per-slot TPS)
Row 3 — Table + Log Strip (ExtendedDataTable + RichLog)
```

This structure provides a clear visual hierarchy:
1. **Primary metrics** (gauges) — immediately visible at a glance
2. **Secondary details** (bars) — granular per-resource utilization
3. **Tertiary data** (table + logs) — slot-level status and diagnostic output

### 2.2 Full Terminal Layout at Ideal Size (120 columns × 52 rows)

```
┌──────────────────────────────────────────────────────────────────────┐
│ Header: Tars Inference Monitor — btop-style TUI Redesign            │  (3 rows)
├──────────────────────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐               │
│ │  GPU     │ │ MEMORY   │ │  POWER   │ │  TEMP    │               │  (12 rows)
│ │  LOAD    │ │          │ │          │ │          │               │
│ │  arc     │ │  arc     │ │  arc     │ │  arc     │               │
│ │  gauge   │ │  gauge   │ │  gauge   │ │  gauge   │               │
│ │          │ │          │ │          │ │          │               │
│ │  ┌───┐   │ │  ┌───┐   │ │  ┌───┐   │ │  ┌───┐   │               │
│ │  │ ~ │   │ │  │ ~ │   │ │  │ ~ │   │ │  │ ~ │   │               │
│ │  └───┘   │ │  └───┘   │ │  └───┘   │ │  └───┘   │               │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘               │
├──────────────────────────────────────────────────────────────────────┤
│ GPU Core Utilization          │ Slot TPS Activity                   │  (20 rows)
│ ──────────────────────────    │ ─────────────────────               │
│ Core 0: ■■■■■■■■■■ 85%        │ Slot 0: ■■■■■■■ 42 TPS              │
│ Core 1: ■■■■■■■■░░ 72%        │ Slot 1: ■■■■■ 28 TPS                │
│ Core 2: ■■■■■░░░░░ 45%        │ Slot 2: ■■ 12 TPS                   │
│ Core 3: ■■■■■■■■■■ 91%        │                                     │
│ ...                           │ (dynamic, grows with busy slots)    │
│                               │                                     │
├──────────────────────────────────────────────────────────────────────┤
│ Inference Slot Table                                          │  (8 rows)
│ ──────────────────────────────────────────────────────────────    │
│ ID │ Status  │ Task      │ Progress │ TPS  │ KV     │             │
│  0 │ BUSY    │ prompt... │  142/512 │ 42.3 │ 89.1k │             │
│  1 │ BUSY    │ embed...  │   88/256 │ 28.1 │ 45.2k │             │
│  2 │ IDLE    │ —         │    0/128 │  0.0 │  0.0k │             │
│ ... │ ... │ ...     │  ...   │ ... │ ... │             │
├──────────────────────────────────────────────────────────────────────┤
│ Logs (timestamp + level + message, scrolling)                     │  (8 rows)
├──────────────────────────────────────────────────────────────────────┤
│ Footer: Keyboard shortcuts (btop-style)                           │  (1 row)
└──────────────────────────────────────────────────────────────────────┘
```

### 2.3 Row Definitions (Ideal Size — ≥52 rows)

**Row 1 — Arc Gauge Panel (12 rows)**
- Purpose: Display 4 GPU metrics as arc gauges — GPU Compute Load (%), Unified Memory (GB), Power Draw (W), Core Temperature (°C)
- Layout: 4 arc gauges arranged horizontally in a 1×4 row, each occupying equal width
- Each arc gauge includes: metric label (top), numeric value (center, large bold), unit label (bottom), gradient arc fill, and a small time-series mini-graph below the arc
- Terminal height: **12 rows**

**Row 2 — Utilization Bars Panel (20 rows)**
- Purpose: Display per-GPU-core utilization bars (horizontal) and per-slot TPS bars
- Layout: Left 60% = GPU core utilization bars. Right 40% = Per-slot TPS bars. If no GPU core data available, use full width for TPS bars.
- Each utilization bar: label (left), `■` bar fill (gradient from FN-006 `cpu` gradient), value percentage (right)
- Terminal height: **20 rows**

**Row 3 — Table + Log Strip Panel (16 rows total)**
- Purpose: Display inference slot status table and scrolling log messages
- Layout: Top portion = ExtendedDataTable. Bottom portion = Log Strip (scrolling RichLog)
- Table height: **8 rows**
- Log Strip height: **8 rows** (docked at very bottom of terminal, above footer)

**Full terminal height at ideal size: 3 + 12 + 20 + 8 + 8 + 1 = 52 rows** ✓

---

## Section 3: Panel Dimensions by Breakpoint

### 3.1 Breakpoint A — Standard: 120 columns × 48 rows

| Panel | Height | Width | Details |
|-------|--------|-------|---------|
| Header | 3 rows | Full | Title text, centered |
| Row 1: Arc Gauges | 12 rows | Full | 4 gauges, each 28 columns wide |
| Row 2: Utilization Bars | **16 rows** | Split 70 + 1 + 48 = 119 | GPU bars (70 cols) + 1-col gap + TPS bars (48 cols) |
| Row 3a: Table | 8 rows | Full | ExtendedDataTable |
| Row 3b: Log Strip | **7 rows** | Full | RichLog docked bottom |
| Footer | 1 row | Full | Keyboard shortcuts |
| **Total** | **47 rows** | | Fits in 48 rows with 1-row margin |

**Row 1 Gauge Layout (120 columns):**
- 4 gauges × 28 cols + 3 gaps × 1 col = 115 cols used
- 5 cols remaining → 2 left margin + 2 right margin + 1 center offset
- Each gauge internal layout: 3 cols label + 14 cols arc + 3 cols value + 0 gap + 6 cols mini-graph + 2 cols padding = **28 cols** ✓

**Row 2 Layout (120 columns):**
- GPU bars panel: 70 cols wide (60% of 119)
- 1 column gap between sub-panels
- TPS bars panel: 48 cols wide (40% of 119)
- Total: 70 + 1 + 48 = 119 cols, 1 col remaining

### 3.2 Breakpoint B — Minimum: 80 columns × 30 rows

| Panel | Height | Width | Details |
|-------|--------|-------|---------|
| Header | **2 rows** | Full | Reduced from 3 to fit minimum |
| Row 1: Arc Gauges | **8 rows** | 2×2 grid | 2 gauges per row × 2 rows, each 36 cols |
| Row 2: Utilization Bars | **8 rows** | Split 42 + 1 + 36 = 79 | GPU bars (42 cols) + 1-col gap + TPS bars (36 cols) |
| Row 3a: Table | **5 rows** | Full | Minimum usable DataTable (3+ header + 2 data) |
| Row 3b: Log Strip | **3 rows** | Full | Minimum usable RichLog |
| Footer | 1 row | Full | Keyboard shortcuts |
| **Total** | **27 rows** | | Fits in 30 rows with 3-row margin |

**Row 1 Gauge Layout (80 columns):**
- 2×2 grid: 2 gauges per row
- 2 gauges × 36 cols + 1 gap × 1 col = 73 cols used
- 7 cols remaining → 3 left margin + 3 right margin + 1 center offset
- Each gauge internal layout: 3 cols label + 24 cols arc + 3 cols value + 0 gap + 2 cols padding = **32 cols** (fits in 36-col slot) ✓
- Vertical: 8 rows for gauge + mini-graph (reduced from 12)

**Row 2 Layout (80 columns):**
- GPU bars panel: 42 cols wide (50% of 79)
- 1 column gap
- TPS bars panel: 36 cols wide (50% of 79)
- Total: 42 + 1 + 36 = 79 cols, 1 col remaining

### 3.3 Breakpoint C — Large: 160 columns × 60 rows

| Panel | Height | Width | Details |
|-------|--------|-------|---------|
| Header | 3 rows | Full | Standard |
| Row 1: Arc Gauges | **14 rows** | Full | 4 gauges, each 38 cols wide (wider arcs) |
| Row 2: Utilization Bars | **28 rows** | Split 96 + 1 + 62 = 159 | GPU bars (96 cols) + 1-col gap + TPS bars (62 cols) |
| Row 3a: Table | **10 rows** | Full | Standard DataTable (2 header + 8 data) |
| Row 3b: Log Strip | **10 rows** | Full | Expanded log strip |
| Footer | 1 row | Full | Keyboard shortcuts |
| **Total** | **66 rows** | | Exceeds 60 rows → responsive shrinkage applies |

**Row 1 Gauge Layout (160 columns):**
- 4 gauges × 38 cols + 3 gaps × 1 col = 155 cols used
- 5 cols remaining → 2 left margin + 2 right margin + 1 center offset
- Each gauge internal layout: 3 cols label + 18 cols arc + 3 cols value + 0 gap + 6 cols mini-graph + 2 cols padding = **32 cols** (fits in 38-col slot with extra padding) ✓
- Extra 6 cols provide additional padding for larger display

**Row 2 Layout (160 columns):**
- GPU bars panel: 96 cols wide (60% of 159)
- 1 column gap
- TPS bars panel: 62 cols wide (40% of 159)
- Total: 96 + 1 + 62 = 159 cols, 1 col remaining

### 3.4 Summary Table (All Totals Arithmetically Verified)

| Panel | 80×30 (Min) | 120×48 (Std) | 160×60 (Large) |
|-------|-------------|--------------|----------------|
| Header | 2 rows | 3 rows | 3 rows |
| Row 1: Arc Gauges | 8 rows (2×2 grid) | 12 rows (1×4 row) | 14 rows (1×4 row) |
| Row 2: Utilization Bars | 8 rows (50/50 split) | 16 rows (60/40 split) | 28 rows (60/40 split) |
| Row 3a: Table | 5 rows | 8 rows | 10 rows |
| Row 3b: Log Strip | 3 rows | 7 rows | 10 rows |
| Footer | 1 row | 1 row | 1 row |
| **Total** | **27 rows** | **47 rows** | **66 rows** |

### 3.5 Arithmetic Verification

| Checkpoint | Calculation | Result | Status |
|------------|-------------|--------|--------|
| Ideal (≥52 rows) | 3 + 12 + 20 + 8 + 8 + 1 | **52 rows** | ✓ |
| Breakpoint A (48 rows) | 3 + 12 + 16 + 8 + 7 + 1 | **47 rows** | ✓ (47 ≤ 48) |
| Breakpoint B (30 rows) | 2 + 8 + 8 + 5 + 3 + 1 | **27 rows** | ✓ (27 ≤ 30) |
| Breakpoint C (60 rows) | 3 + 14 + 28 + 10 + 10 + 1 | **66 rows** | > 60 → responsive shrink |
| Gauge width (Std) | 3 + 14 + 3 + 0 + 6 + 2 | **28 cols/gauge** | ✓ |
| Gauge width (Min) | 3 + 24 + 3 + 0 + 2 | **32 cols/gauge** | ✓ (fits in 36-col slot) |
| Gauge width (Large) | 3 + 18 + 3 + 0 + 6 + 2 | **32 cols/gauge** | ✓ (fits in 38-col slot) |

---

## Section 4: Spacing & Border Conventions

### 4.1 Panel Borders (Unicode Box-Drawing)

Borders are implemented using characters from FN-005's `Symbols` namespace:

| Element | Unicode Characters | Source |
|---------|-------------------|--------|
| Sharp corners | `┌` `┐` `└` `┘` | `Symbols::left_up`, `right_up`, `left_down`, `right_down` |
| Rounded corners | `╭` `╮` `╰` `╯` | `Symbols::round_left_up`, `round_right_up`, `round_left_down`, `round_right_down` |
| Horizontal line | `─` | `Symbols::h_line` (U+2500) |
| Vertical line | `│` | `Symbols::v_line` (U+2502) |
| Title left | `┐` | `Symbols::title_left` (same as `right_up`) |
| Title right | `┌` | `Symbols::title_right` (same as `left_up`) |

**Corner Style:**
- Rounded corners are the **default** (matching btop's `Config::getB("rounded_corners")` setting)
- Use `╭` / `╮` / `╰` / `╯` for all panel corners by default
- In TTY mode or when `rounded_corners=false`, fall back to sharp corners `┌` / `┐` / `└` / `┘`

**Border Color:**
- All panel borders use `div_line` = **`#303030`** (very dark gray) from FN-006's `Default_theme`

### 4.2 `createBox()` Integration

Panel borders should be rendered using a `createBox()` function pattern (from FN-005), which assembles a complete panel frame:

```
createBox(x, y, width, height, title="Panel Title", rounded=true)
```

The function should:
1. Draw top border: `╭` + `─` × (width-2) + `╮`
2. Draw title bar: `│` + padding + title (bold, `title` color `#eeeeee`) + `┐` + `─` × remaining + `┌`
3. Draw body: `│` + content + `│`
4. Draw bottom border: `╰` + `─` × (width-2) + `╯`

See FN-005's `createBox()` algorithm for the full implementation details (source: `src/btop_draw.cpp`).

### 4.3 Internal Spacing

| Element | Spacing | Notes |
|---------|---------|-------|
| Panel padding | 1 character cell on all sides | Matches btop's inner padding convention |
| Row gaps | 1 row between Row 1↔Row 2, Row 2↔Row 3a | Visual separation between major panels |
| Sub-panel gaps | 1 column between horizontal sub-panels within Row 2 | e.g., GPU bars ↔ TPS bars |
| Gauge label margin | 2 columns left | Space for metric label text |
| Gauge value margin | 1 column right | Space for numeric value display |
| Bar label margin | 1 column left | Space for bar label text |
| Bar value margin | 1 column right | Space for percentage value |

### 4.4 Text Layout & Colors (from FN-006 Palette)

| Element | Weight | Color | Hex | Source Key |
|---------|--------|-------|-----|------------|
| Panel titles | Bold | Light gray | `#eeeeee` | `title` |
| Metric values (arc gauges) | Bold, large | Light gray | `#cccccc` | `main_fg` |
| Bar labels | Regular | Light gray | `#cccccc` | `main_fg` |
| Bar values | Regular | Light gray | `#cccccc` | `main_fg` |
| Table headers | Bold, uppercase | Light gray | `#eeeeee` | `title` |
| Log timestamps | Regular, dim | Dark gray | `#404040` | `inactive_fg` |
| Log info text | Regular | Light gray | `#cccccc` | `main_fg` |
| Log warn text | Regular | Red | `#b54040` | `hi_fg` |
| Log error text | Regular, bold | Bright red | `#b54040` | `hi_fg` |
| BUSY status | Bold | Red | `#b54040` | `hi_fg` |
| IDLE status | Regular, dim | Dark gray | `#404040` | `inactive_fg` |
| Selected row bg | — | Dark red | `#6a2f2f` | `selected_bg` |
| Selected row fg | — | Light gray | `#eeeeee` | `selected_fg` |
| Panel borders | — | Very dark gray | `#303030` | `div_line` |
| Meter fill bg (empty) | — | Dark gray | `#404040` | `meter_bg` |

### 4.5 Gradient Colors for Gauge/Bar Fills (from FN-006)

| Panel/Metric | Gradient Key | Start | Mid | End |
|--------------|-------------|-------|-----|-----|
| GPU Compute Load | `cpu` | `#77ca9b` | `#cbc06c` | `#dc4c4c` |
| Memory Used | `used` | `#592b26` | `#d9626d` | `#ff4769` |
| Power Draw | `cpu` | `#77ca9b` | `#cbc06c` | `#dc4c4c` |
| Temperature | `temp` | `#4897d4` | `#5474e8` | `#ff40b6` |
| Per-slot TPS | `cpu` | `#77ca9b` | `#cbc06c` | `#dc4c4c` |

All gradients use btop's 50/50 interpolation algorithm (0→50: start→mid, 50→100: mid→end), generating 101 color values per gradient as documented in FN-006's Section D.

---

## Section 5: Responsive Rules

### 5.1 Resize Trigger

On terminal resize (`SIGWINCH` → Textual's `on_resize` event), all panel dimensions are recalculated proportionally. This mirrors btop's `Term::refresh()` → `Draw::calcSizes()` pipeline.

### 5.2 Row Height Allocation (Vertical)

| Panel | Behavior | Notes |
|-------|----------|-------|
| Header | **Fixed** — 3 rows (standard/large), 2 rows (minimum) | Never shrinks below 2 rows |
| Footer | **Fixed** — 1 row | Never changes |
| Row 3a (Table) | **Minimum 5 rows**, grows proportionally | If space < 5 rows, enable DataTable scroll |
| Row 3b (Log Strip) | **Minimum 3 rows**, grows proportionally | If space < 3 rows, enable RichLog scroll |
| Row 2 (Utilization Bars) | **Flexible** — takes remaining space | Shrinks first on overflow, min 6 rows |
| Row 1 (Arc Gauges) | **Fixed** — 12 rows (standard), 8 rows (minimum) | Never hidden or shrunk below 8 rows |

### 5.3 Breakpoint Transitions (Column-Based)

| Column Range | Layout Mode | Gauge Grid | GPU/TPS Split | Notes |
|-------------|-------------|------------|---------------|-------|
| 80–99 cols | **Minimum** | 2×2 grid | 50/50 | Compact layout |
| 100–139 cols | **Standard** | 1×4 row | 60/40 | Default target |
| 140+ cols | **Large** | 1×4 row (wider) | 60/40 | Expanded detail |

**Transition behavior:**
- When columns cross from 99 → 100: gauge grid collapses from 2×2 to 1×4
- When columns cross from 139 → 140: gauge arc width increases from 14 to 18 cols, Row 2 grows from 16 to 20 rows, Table grows from 8 to 10 rows, Log Strip grows from 7 to 8 rows
- Transitions are **instant** (no animation) — recalculated on every resize event

### 5.4 Overflow Handling

When total calculated row height exceeds available terminal rows:

1. **Shrink Row 2 (Utilization Bars) first** — takes the hit, but never below 6 rows
2. **Then shrink Row 3b (Log Strip)** — reduce below 7 rows if needed, but never below 3 rows
3. **Then shrink Row 3a (Table)** — reduce below 8 rows if needed, but never below 5 rows
4. **Enable scrolling** — if Row 2 < 6 rows, enable vertical scroll within Row 2's container (like current `#active-slots`)
5. **Never hide** — Header, Footer, and Row 1 (arc gauges) are always visible

**Example: 60-row terminal (Breakpoint C)**
- Ideal total: 66 rows, terminal has 60 → 6-row deficit
- Shrink Row 2: 28 → 22 rows (-6)
- Total: 3 + 14 + 22 + 10 + 10 + 1 = 60 ✓

**Example: 48-row terminal (Breakpoint A)**
- Standard total: 47 rows, terminal has 48 → 1-row surplus
- No shrinkage needed — fits with 1-row margin
- Row 2: 16 rows (not full 20), Row 3b: 7 rows (not full 8)

### 5.5 Textual Implementation Notes

| Concept | Textual Mapping |
|---------|----------------|
| Panel containers | `Container` with `dock`, `height`, `width` properties |
| Row gaps | 1-row empty `Static` spacer between rows |
| Flexible height (Row 2) | CSS `1fr` or Textual `height: auto` with flex layout |
| Fixed height (Header, Row 1, Footer) | CSS `height: N` (e.g., `height: 3`) |
| Resize handler | Override `on_resize` to recalculate panel dimensions |
| Terminal size access | `self.size` in Textual App (returns `(width, height)` tuple) |
| Full-width panels | `width: 100%` or `width: 1fr` on horizontal containers |
| Sub-panel split | `Horizontal` container with `Grid` or proportional widths |

---

## Section 6: Widget-to-Panel Mapping

### 6.1 Panel-to-Widget Mapping

| Panel | Existing Widget(s) | Replacement/New Widget(s) |
|-------|-------------------|--------------------------|
| Header | `Header` (Textual built-in) | `Header` (unchanged, docked top) |
| Row 1 Arc Gauges | `GraphWidget` (×4), `Static` labels | **`ArcGauge`** custom widget |
| Row 2 Utilization Bars | None (new feature) | **`MeterBar`** custom widget |
| Row 3a Table | `DataTable` | **`ExtendedDataTable`** (enhanced) |
| Row 3b Log Strip | `RichLog` | `RichLog` (unchanged, docked bottom) |
| Footer | `Footer` (Textual built-in) | `Footer` (unchanged, docked bottom) |

### 6.2 New Widget Specifications (High-Level)

Detailed specifications for new widgets belong in FN-008 (Design Spec Document). This section provides high-level parameters only.

**`ArcGauge` Custom Widget**

| Parameter | Type | Description |
|-----------|------|-------------|
| `title` | `str` | Metric label (e.g., "GPU LOAD", "MEMORY") |
| `value` | `float` | Current metric value (e.g., 85.0, 12.3) |
| `min` | `float` | Minimum scale value (e.g., 0.0, 0.0) |
| `max` | `float` | Maximum scale value (e.g., 100.0, 64.0) |
| `unit` | `str` | Unit label (e.g., "%", "GB", "W", "°C") |
| `gradient_key` | `str` | Gradient key from FN-006 (e.g., "cpu", "used", "temp") |

**Rendering:** Semi-circular arc fill using Unicode arc-drawing characters, bold numeric value display, unit label, and small time-series mini-graph below the arc. Specific Unicode characters for arc rendering to be defined in FN-008.

**`MeterBar` Custom Widget**

| Parameter | Type | Description |
|-----------|------|-------------|
| `label` | `str` | Bar label (e.g., "Core 0", "Slot 0") |
| `value` | `float` | Current value (0.0–100.0 percentage) |
| `max` | `float` | Maximum scale value (default: 100.0) |
| `gradient_key` | `str` | Gradient key from FN-006 (default: "cpu") |

**Rendering:** Horizontal bar using `■` (U+25A0 FULL SQUARE) characters, gradient fill per FN-006 gradient triplets, value percentage displayed to the right.

**`ExtendedDataTable` (Enhanced DataTable)**

Extends Textual's `DataTable` with:
- **Column color logic:** Status column uses `hi_fg` (`#b54040`) for BUSY, `inactive_fg` (`#404040`) for IDLE
- **Row selection highlighting:** Uses `selected_bg` (`#6a2f2f`) + `selected_fg` (`#eeeeee`)
- **Keyboard navigation:** Arrow keys for cell/row navigation, Enter to select

### 6.3 Widget Container Hierarchy (Textual `compose` Structure)

```
App (TarsMonitor)
├── Header (docked top)
├── Container (id="grid-main")
│   ├── Container (id="row1-gauges")          ← Arc Gauge Panel
│   │   ├── Container (id="gauge-load")       ← ArcGauge: GPU Load
│   │   ├── Container (id="gauge-memory")     ← ArcGauge: Memory
│   │   ├── Container (id="gauge-power")      ← ArcGauge: Power
│   │   └── Container (id="gauge-temp")       ← ArcGauge: Temperature
│   ├── Container (id="row2-bars")            ← Utilization Bars Panel
│   │   ├── Container (id="gpu-bars")         ← Left: GPU Core MeterBars
│   │   └── Container (id="tps-bars")         ← Right: Slot TPS MeterBars
│   └── Container (id="row3-data")            ← Table + Log Strip Panel
│       ├── ExtendedDataTable (id="slots-table") ← Slot status table
│       └── RichLog (id="logs")                ← Scrolling log output
└── Footer (docked bottom)
```

This hierarchy replaces the current `Horizontal(#sidebar + #main-content)` structure with a **full-width three-row vertical stack**.

---

## Appendix A: Quick Reference

### A.1 Panel Dimensions Summary

| Panel | 80×30 (Min) | 120×48 (Std) | 160×60 (Large) |
|-------|-------------|--------------|----------------|
| Header | 2 rows | 3 rows | 3 rows |
| Row 1: Arc Gauges (1×4 or 2×2) | 8 rows (2×2) | 12 rows (1×4) | 14 rows (1×4) |
| Row 2: Utilization Bars | 8 rows | 16 rows | 22–28 rows* |
| Row 3a: Table | 5 rows | 8 rows | 8–10 rows* |
| Row 3b: Log Strip | 3 rows | 7 rows | 7–10 rows* |
| Footer | 1 row | 1 row | 1 row |
| **Total** | **27** | **47** | **66 (shrunk)** |

*Row 2/3a/3b heights vary at large breakpoint based on overflow handling.

### A.2 Gauge Internal Layout by Breakpoint

| Breakpoint | Columns/Gauge | Label | Arc | Value | Gap | Mini-graph | Padding |
|------------|---------------|-------|-----|-------|-----|------------|---------|
| 80×30 (Min) | 36 | 3 cols | 24 cols | 3 cols | 0 | 2 cols | — |
| 120×48 (Std) | 28 | 3 cols | 14 cols | 3 cols | 0 | 6 cols | 2 cols |
| 160×60 (Large) | 38 | 3 cols | 18 cols | 3 cols | 0 | 6 cols | 2 cols |

### A.3 Color Palette Quick Reference (FN-006)

| Key | Hex | Usage |
|-----|-----|-------|
| `div_line` | `#303030` | Panel borders |
| `main_bg` | `#0a0a0a` | Screen/panel background |
| `main_fg` | `#cccccc` | Primary text, metric values |
| `title` | `#eeeeee` | Panel titles, table headers |
| `hi_fg` | `#b54040` | BUSY status, warn/error logs |
| `inactive_fg` | `#404040` | IDLE status, log timestamps |
| `selected_bg` | `#6a2f2f` | Selected row background |
| `meter_bg` | `#404040` | Empty meter bar cells |

**Gradients:**
| Key | Start | Mid | End |
|-----|-------|-----|-----|
| `cpu` | `#77ca9b` | `#cbc06c` | `#dc4c4c` |
| `temp` | `#4897d4` | `#5474e8` | `#ff40b6` |
| `used` | `#592b26` | `#d9626d` | `#ff4769` |

### A.4 Unicode Characters Reference (FN-005 `Symbols`)

| Purpose | Character | Symbol Key |
|---------|-----------|------------|
| Rounded corners | `╭ ╮ ╰ ╯` | `round_left_up` etc. |
| Sharp corners | `┌ ┐ └ ┘` | `left_up` etc. |
| Horizontal line | `─` | `h_line` |
| Vertical line | `│` | `v_line` |
| Meter fill | `■` | `meter` (U+25A0) |
| Title connectors | `┐ ┌` | `title_left` / `title_right` |
| Dividers | `├ ┤ ┬ ┴` | `div_left` etc. |

---

## Cross-References

| Reference | Source | Content |
|-----------|--------|---------|
| `Symbols` namespace | FN-005, Section 4 | All Unicode box-drawing characters used for panel borders |
| `createBox()` function | FN-005, Section 5 | Panel border assembly algorithm |
| `div_line` color | FN-006, Section A1 | `#303030` — panel borders and dividers |
| `main_bg` color | FN-006, Section A1 | `#0a0a0a` — screen/panel background (Tars-adapted from btop's `#000000`) |
| `main_fg` color | FN-006, Section A1 | `#cccccc` — primary text and metric values |
| `title` color | FN-006, Section A1 | `#eeeeee` — panel titles and table headers |
| `cpu` gradient | FN-006, Section B | `#77ca9b` → `#cbc06c` → `#dc4c4c` — GPU load bars |
| `temp` gradient | FN-006, Section B | `#4897d4` → `#5474e8` → `#ff40b6` — temperature gauge |
| `used` gradient | FN-006, Section B | `#592b26` → `#d9626d` → `#ff4769` — memory gauge |
| `hi_fg` color | FN-006, Section A1 | `#b54040` — BUSY status, warnings |
| `inactive_fg` color | FN-006, Section A1 | `#404040` — IDLE status, timestamps |
| `selected_bg` color | FN-006, Section A1 | `#6a2f2f` — selected row background |
| `meter_bg` color | FN-006, Section A1 | `#404040` — empty meter bar cells |
| 50/50 gradient interpolation | FN-006, Section D | Algorithm for generating 101 gradient values from 3-stop triplets |

---

*Document generated by FN-007 (Layout Grid Specification). All dimensions arithmetically verified. No application code changes — specification document only.*
