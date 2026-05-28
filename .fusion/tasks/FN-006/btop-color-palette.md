# btop Default Theme — Color Palette Reference

> **Upstream Source:** `https://raw.githubusercontent.com/aristocratos/btop/main/src/btop_theme.cpp`
> **Canonical Repo:** `https://github.com/aristocratos/btop`
> **Upstream Release:** `https://github.com/aristocratos/btop/releases/latest`
> **Version:** v1.4.7 (main branch as of 2026-05-27)
>
> The `Default_theme` map is a `std::unordered_map<string, string>` containing **48** color key-value pairs.
> The `TTY_theme` map provides ANSI escape code fallbacks for 256-color / TTY mode — also **48** entries.
>
> **2-digit hex expansion:** btop uses compact hex (e.g. `#cc` = `#cccccc`). The `hex_to_dec` function in
> `btop_theme.cpp` expands a 2-digit hex string `XY` to `{stoi(XY, 16), stoi(XY, 16), stoi(XY, 16)}`,
> producing a grayscale RGB triplet. For example, `#00` → `{0, 0, 0}` → `#000000` (black), `#cc` → `{204, 204, 204}` → `#cccccc`.

---

## Section A — Base Color Reference

### Section A1 — Non-gradient base colors (15 entries)

| # | Key | btop Hex | Expanded RGB | Semantic Role | Visual Description |
|---|-----|----------|-------------|---------------|-------------------|
| 1 | `main_bg` | `#00` | `#000000` (0, 0, 0) | Panel / Screen background | Black by default. Transparent only when config `theme_background=false` triggers override to `\x1b[49m` (reset background escape code). |
| 2 | `main_fg` | `#cc` | `#cccccc` (204, 204, 204) | Primary text | Light gray — main body text, metric values, labels |
| 3 | `title` | `#ee` | `#eeeeee` (238, 238, 238) | Panel titles | Very light gray — section headers like "GPU LOAD:", "MEMORY:" |
| 4 | `hi_fg` | `#b54040` | `#b54040` (181, 64, 64) | Highlight / alert text | Red — highlighted rows, important indicators, warning text |
| 5 | `selected_bg` | `#6a2f2f` | `#6a2f2f` (106, 47, 47) | Selected row background | Dark red — active/selected list row background |
| 6 | `selected_fg` | `#ee` | `#eeeeee` (238, 238, 238) | Selected row text | Very light gray — text rendered on selected row |
| 7 | `inactive_fg` | `#40` | `#404040` (64, 64, 64) | Inactive / decorative | Dark gray — borders, separators, dimmed text, idle indicators |
| 8 | `graph_text` | `#60` | `#606060` (96, 96, 96) | Axis labels | Medium-dark gray — graph axis tick labels |
| 9 | `meter_bg` | `#40` | `#404040` (64, 64, 64) | Meter fill background | Dark gray — unfilled portion of progress meters; falls back to `inactive_fg` if not defined |
| 10 | `proc_misc` | `#0de756` | `#0de756` (13, 231, 86) | Process / misc highlights | Bright green — process names, notable values |
| 11 | `cpu_box` | `#556d59` | `#556d59` (85, 109, 89) | CPU panel accent | Muted olive green — CPU panel border / title accent |
| 12 | `mem_box` | `#6c6c4b` | `#6c6c4b` (108, 108, 75) | Memory panel accent | Muted olive/tan — memory panel border / title accent |
| 13 | `net_box` | `#5c588d` | `#5c588d` (92, 88, 141) | Network panel accent | Muted purple — network panel border / title accent |
| 14 | `proc_box` | `#805252` | `#805252` (128, 82, 82) | Process panel accent | Muted rose — process panel border / title accent |
| 15 | `div_line` | `#30` | `#303030` (48, 48, 48) | Panel dividers / borders | Very dark gray — box drawing lines, panel borders, table cell borders |

### Section A2 — Process-specific banner colors (6 entries)

| # | Key | btop Hex | Expanded RGB | Semantic Role | Visual Description |
|---|-----|----------|-------------|---------------|-------------------|
| 16 | `proc_pause_bg` | `#b54040` | `#b54040` (181, 64, 64) | Process pause banner background | Red — shown when process list is paused |
| 17 | `proc_follow_bg` | `#4040b5` | `#4040b5` (64, 64, 181) | Process follow banner background | Blue — shown when process following is active |
| 18 | `proc_banner_bg` | `#7b407b` | `#7b407b` (123, 64, 123) | Process combined banner background | Purple — shown when both paused AND following |
| 19 | `proc_banner_fg` | `#ee` | `#eeeeee` (238, 238, 238) | Process banner text | Very light gray — text color on any process banner |
| 20 | `followed_bg` | `#4040b5` | `#4040b5` (64, 64, 181) | Followed process background | Blue — highlights the process being followed in the list |
| 21 | `followed_fg` | `#ee` | `#eeeeee` (238, 238, 238) | Followed process text | Very light gray — text color on followed process row |

### Section A1+A2 Summary

**21 non-gradient entries** documented above (15 base + 6 banner).

---

## Section B — Gradient Colors (27 entries in 9 triplets)

btop gradients use **3 colors** (start, mid, end) per triplet. The `generateGradients()` function creates **101 color values** (indexed 0–100) per gradient via the algorithm documented in Section D.

| # | Gradient Name | Start Hex | Start RGB | Mid Hex | Mid RGB | End Hex | End RGB | Semantic Use |
|---|--------------|-----------|-----------|---------|---------|---------|---------|-------------|
| 22–24 | `temp` (temperature) | `#4897d4` | (72, 151, 212) | `#5474e8` | (84, 116, 232) | `#ff40b6` | (255, 64, 182) | Temperature gauge — blue → violet → pink |
| 25–27 | `cpu` (CPU load) | `#77ca9b` | (119, 202, 155) | `#cbc06c` | (203, 192, 108) | `#dc4c4c` | (220, 76, 76) | CPU usage bar — green → yellow → red |
| 28–30 | `free` (free memory) | `#384f21` | (56, 79, 33) | `#b5e685` | (181, 230, 133) | `#dcff85` | (220, 255, 133) | Free memory gauge — dark green → light green → pale |
| 31–33 | `cached` (cached memory) | `#163350` | (22, 51, 80) | `#74e6fc` | (116, 230, 252) | `#26c5ff` | (38, 197, 255) | Cached memory gauge — dark blue → cyan → blue |
| 34–36 | `available` (available memory) | `#4e3f0e` | (78, 63, 14) | `#ffd77a` | (255, 215, 122) | `#ffb814` | (255, 184, 20) | Available memory gauge — dark amber → gold → yellow |
| 37–39 | `used` (used memory) | `#592b26` | (89, 43, 38) | `#d9626d` | (217, 98, 109) | `#ff4769` | (255, 71, 105) | Used memory gauge — dark red → rose → red |
| 40–42 | `download` (network down) | `#291f75` | (41, 31, 117) | `#4f43a3` | (79, 67, 163) | `#b0a9de` | (176, 169, 222) | Download speed gauge — dark purple → muted purple → light purple |
| 43–45 | `upload` (network up) | `#620665` | (98, 6, 101) | `#7d4180` | (125, 65, 128) | `#dcafde` | (220, 175, 222) | Upload speed gauge — dark magenta → muted purple → light magenta |
| 46–48 | `process` (process list) | `#80d0a3` | (128, 208, 163) | `#dcd179` | (220, 209, 121) | `#d45454` | (212, 84, 84) | Process list bar — green → yellow → red |

**27 gradient entries** documented above (9 triplets × 3 values).

### Total Count Verification

| Category | Entries |
|----------|---------|
| Section A1 — Non-gradient base colors | 15 |
| Section A2 — Process banner colors | 6 |
| Section B — Gradient triplets (3 values each) | 27 |
| **Default_theme total** | **48** ✓ |

---

## Section C — TTY Fallback Colors (48 entries)

When `truecolor=false` in btop config, or when running in actual TTY mode, the `TTY_theme` map provides ANSI escape code fallbacks. These replace 24-bit RGB colors with basic ANSI sequences.

| Key | ANSI Escape Code | ANSI Name | Meaning |
|-----|-----------------|-----------|---------|
| `main_bg` | `\x1b[0;40m` | Black background | Background |
| `main_fg` | `\x1b[37m` | Bright white | Primary text |
| `title` | `\x1b[97m` | Bright white | Panel titles |
| `hi_fg` | `\x1b[91m` | Bright red | Highlight/alert text |
| `selected_bg` | `\x1b[41m` | Red background | Selected row background |
| `selected_fg` | `\x1b[97m` | Bright white | Selected row text |
| `inactive_fg` | `\x1b[90m` | Bright black (gray) | Inactive/decorative elements |
| `graph_text` | `\x1b[90m` | Bright black (gray) | Axis labels |
| `meter_bg` | `\x1b[90m` | Bright black (gray) | Meter fill background |
| `proc_misc` | `\x1b[92m` | Bright green | Process/misc highlights |
| `cpu_box` | `\x1b[32m` | Green | CPU panel accent |
| `mem_box` | `\x1b[33m` | Yellow | Memory panel accent |
| `net_box` | `\x1b[35m` | Magenta | Network panel accent |
| `proc_box` | `\x1b[31m` | Red | Process panel accent |
| `div_line` | `\x1b[90m` | Bright black (gray) | Panel dividers |
| `temp_start` | `\x1b[94m` | Bright blue | Temperature gauge start |
| `temp_mid` | `\x1b[96m` | Bright cyan | Temperature gauge mid |
| `temp_end` | `\x1b[95m` | Bright magenta | Temperature gauge end |
| `cpu_start` | `\x1b[92m` | Bright green | CPU load gauge start |
| `cpu_mid` | `\x1b[93m` | Bright yellow | CPU load gauge mid |
| `cpu_end` | `\x1b[91m` | Bright red | CPU load gauge end |
| `free_start` | `\x1b[32m` | Green | Free memory gauge start |
| `free_mid` | *(empty, skip)* | — | Same as `free_start` (empty → no mid split) |
| `free_end` | `\x1b[92m` | Bright green | Free memory gauge end |
| `cached_start` | `\x1b[36m` | Cyan | Cached memory gauge start |
| `cached_mid` | *(empty, skip)* | — | Same as `cached_start` (empty → no mid split) |
| `cached_end` | `\x1b[96m` | Bright cyan | Cached memory gauge end |
| `available_start` | `\x1b[33m` | Yellow | Available memory gauge start |
| `available_mid` | *(empty, skip)* | — | Same as `available_start` (empty → no mid split) |
| `available_end` | `\x1b[93m` | Bright yellow | Available memory gauge end |
| `used_start` | `\x1b[31m` | Red | Used memory gauge start |
| `used_mid` | *(empty, skip)* | — | Same as `used_start` (empty → no mid split) |
| `used_end` | `\x1b[91m` | Bright red | Used memory gauge end |
| `download_start` | `\x1b[34m` | Blue | Download gauge start |
| `download_mid` | *(empty, skip)* | — | Same as `download_start` (empty → no mid split) |
| `download_end` | `\x1b[94m` | Bright blue | Download gauge end |
| `upload_start` | `\x1b[35m` | Magenta | Upload gauge start |
| `upload_mid` | *(empty, skip)* | — | Same as `upload_start` (empty → no mid split) |
| `upload_end` | `\x1b[95m` | Bright magenta | Upload gauge end |
| `process_start` | `\x1b[32m` | Green | Process list bar start |
| `process_mid` | `\x1b[33m` | Yellow | Process list bar mid |
| `process_end` | `\x1b[31m` | Red | Process list bar end |
| `proc_pause_bg` | `\x1b[41m` | Red background | Process pause banner |
| `proc_follow_bg` | `\x1b[44m` | Blue background | Process follow banner |
| `proc_banner_bg` | `\x1b[45m` | Magenta background | Combined process banner |
| `proc_banner_fg` | `\x1b[97m` | Bright white | Process banner text |
| `followed_bg` | `\x1b[44m` | Blue background | Followed process background |
| `followed_fg` | `\x1b[97m` | Bright white | Followed process text |

### TTY Theme Gradient Behavior

The TTY theme's `generateTTYColors()` function generates gradients from 2 or 3 ANSI colors:

```cpp
int split = colors.at(base_name + "_mid").empty() ? 50 : 33;
for (int i : iota(0, 101)) {
    gradients[base_name][i] = colors.at(base_name + section);
    if (i == split) {
        section = (split == 33) ? "_mid" : "_end";
        split *= 2;
    }
}
```

- **3-color gradients (start/mid/end):** `split = 33`, doubles to `66` — first 33 values use start, values 33–65 use mid, values 66–100 use end.
- **2-color gradients (start/end, mid empty):** `split = 50` — first 50 values use start, values 50–100 use end.

Note: This differs from the truecolor `Default_theme` behavior documented in Section D.

### TTY Theme Count Verification

| Category | Entries |
|----------|---------|
| Base colors (no gradient suffix) | 15 |
| Gradient triplets | 27 (9 × 3) |
| Banner colors | 6 |
| **TTY_theme total** | **48** ✓ |

---

## Section D — Gradient Interpolation Logic (Default Theme)

btop generates **101 gradient values** (indexed 0–100) per gradient triplet using the algorithm from `generateGradients()`:

### Algorithm

```cpp
int current_range = (input_colors[1][0] >= 0) ? 50 : 100;
for (int rgb : iota(0, 3)) {
    int start = 0, offset = 0;
    int end = (current_range == 50) ? 1 : 2;
    for (int i : iota(0, 101)) {
        output_colors[i][rgb] = input_colors[start][rgb]
            + (i - offset) * (input_colors[end][rgb] - input_colors[start][rgb])
            / current_range;
        if (i == current_range) { ++start; ++end; offset = 50; }
    }
}
```

### Behavior

| Gradient Config | Split Point | Range | Interpolation Path |
|----------------|-------------|-------|--------------------|
| **3 colors** (start + mid + end) | Index 50 | 0–50: start→mid, 50–100: mid→end | Two linear segments, each 50 values |
| **2 colors** (start + end, no mid) | Index 100 | 0–100: start→end | Single linear segment |
| **1 color** (start only) | N/A | All 101 values = start color | Constant |

### Critical: The 50/50 Split

For 3-color gradients (the default case in `Default_theme`), btop uses a **50/50 split** — **NOT** 33/66.

- Values 0–50 interpolate from start color to mid color (51 values, including index 50 at both ends)
- Values 50–100 interpolate from mid color to end color (51 values, including index 50)
- Index 50 is the shared point where both segments meet at the mid color

This is a **50/50 gradient**, not a 33/33/33 threshold-based color scheme. The gradient smoothly transitions through three colors with the midpoint occurring at exactly 50% of the bar.

### Visual Example: CPU Gradient

| Bar Position | RGB (approx) | Color |
|-------------|-------------|-------|
| 0% (start) | (119, 202, 155) | `#77ca9b` — light green |
| 25% | (161, 197, 132) | Midway green→yellow |
| 50% (mid) | (203, 192, 108) | `#cbc06c` — yellow |
| 75% | (212, 134, 92) | Midway yellow→red |
| 100% (end) | (220, 76, 76) | `#dc4c4c` — red |

### Visual Example: Temperature Gradient

| Bar Position | RGB (approx) | Color |
|-------------|-------------|-------|
| 0% (start) | (72, 151, 212) | `#4897d4` — blue |
| 25% | (79, 141, 228) | Midway blue→violet |
| 50% (mid) | (84, 116, 232) | `#5474e8` — violet |
| 75% | (170, 90, 214) | Midway violet→pink |
| 100% (end) | (255, 64, 182) | `#ff40b6` — pink |

---

## Section E — Tars Adaptation Guide

### Section E1 — Color Mappings: btop → Tars Inference Monitor

Mapping btop palette keys to Tars Inference Monitor (Textual TUI) UI elements derived from `monitor.py`:

| Tars UI Element | btop Key(s) | Suggested btop Hex | Notes |
|-----------------|-------------|-------------------|-------|
| Screen background | `main_bg` | `#0a0a0a` | Project spec overrides btop's `#000000`; see Section E2 note |
| Panel borders / dividers | `div_line` | `#303030` | Box-drawing separator lines, DataTable borders |
| Panel titles (e.g. "GPU LOAD:", "MEMORY:") | `title` | `#eeeeee` | Section headers, bold |
| Primary text (labels, metric names) | `main_fg` | `#cccccc` | Body text |
| Metric values (current readings, bold) | `main_fg` | `#cccccc` | Current readings in widgets |
| Idle / inactive elements (IDLE status) | `inactive_fg` | `#404040` | Dimmed text, IDLE slot status |
| Graph axis labels | `graph_text` | `#606060` | Axis tick labels on graphs |
| GPU load bar (0–100%) | `cpu` gradient | `#77ca9b` → `#cbc06c` → `#dc4c4c` | Green → Yellow → Red |
| Temperature gauge (30–100°C) | `temp` gradient | `#4897d4` → `#5474e8` → `#ff40b6` | Blue → Violet → Pink |
| Power draw panel | `cpu_box` accent | `#556d59` | Panel border/title accent |
| Memory panel | `mem_box` accent | `#6c6c4b` | Panel border/title accent |
| Memory used gauge (0–100%) | `used` gradient | `#592b26` → `#d9626d` → `#ff4769` | Dark red → Rose → Red |
| Memory free gauge (0–100%) | `free` gradient | `#384f21` → `#b5e685` → `#dcff85` | Dark green → Light green → Pale |
| Slot status BUSY | `hi_fg` | `#b54040` | Red — active/in-progress slots |
| Slot status IDLE | `inactive_fg` | `#404040` | Dim gray — idle slots |
| Selected / highlighted table row | `selected_bg` + `selected_fg` | `#6a2f2f` / `#eeeeee` | Active DataTable row highlight |
| DataTable borders | `div_line` | `#303030` | Table cell borders |
| Log timestamps | `inactive_fg` | `#404040` | Dimmed timestamps in log entries |
| Log info text | `main_fg` | `#cccccc` | Standard log messages |
| Log warn text | `hi_fg` | `#b54040` | Warning log messages |
| Process / notable value highlights | `proc_misc` | `#0de756` | Bright green for notable values |
| Slot card borders | `div_line` / `proc_box` | `#303030` | SlotDetail card borders |
| Footer background | — | `#1a1a1a` | Derived from `div_line` but slightly lighter |
| Footer text | `inactive_fg` | `#666666` | Approximation of current footer gray |

### Section E2 — Unmapped / Project-Specific Colors

The following btop palette entries are **not mapped** to Tars UI elements in this task:

| btop Key | Suggested Hex | Reason for Exclusion |
|----------|--------------|---------------------|
| `net_box` | `#5c588d` | No network panel in current Tars monitor (no download/upload gauges) |
| `proc_box` | `#805252` | No dedicated process panel in current Tars monitor |

The following project-specific colors exist in `monitor.tcss` but are **not part of** btop's Default theme:

| Project Element | Hex | Source |
|----------------|-----|--------|
| Header background | `#0047ab` | `monitor.tcss` — project-specific blue |
| Header text | `white` (default) | `monitor.tcss` — high contrast on header |
| Footer background | `#1a1a1a` | `monitor.tcss` — slightly lighter than panel bg |

These project-specific colors may be re-evaluated when the btop palette is fully adopted in a future task.

---

## Section F — Tars-Specific Threshold Adaptation

> **Important:** btop's native gradients use **50/50 gradient interpolation** (Section D), not fixed percentage thresholds. The thresholds below are **Tars-adapted** values — they represent the project's operational ranges for GPU/ROCm systems and are **not** btop's native behavior.

| Metric | Low Range | Mid Range | High Range | Color Mapping |
|--------|-----------|-----------|------------|---------------|
| GPU Load | 0–33% | 34–66% | 67–100% | `cpu` gradient: `#77ca9b` → `#cbc06c` → `#dc4c4c` |
| Temperature | 0–40°C | 41–65°C | 66–100°C | `temp` gradient: `#4897d4` → `#5474e8` → `#ff40b6` |
| Memory Used | 0–50% | 51–80% | 81–100% | `used` gradient: `#592b26` → `#d9626d` → `#ff4769` |
| Power Draw | 0–70% of max TGP | 71–90% of max TGP | 91–100% of max TGP | `cpu` gradient (same as GPU Load) |

These thresholds should be applied in the Tars widget engine to provide threshold-aware color transitions independent of the gradient interpolation. For example, a GPU load of 45% would display the `cpu_mid` color (`#cbc06c`) regardless of the gradient's 50% interpolation midpoint.

---

## Appendix — Source Code Evidence

### `Default_theme` Map (verbatim from source)

```cpp
const std::unordered_map<string, string> Default_theme = {
    { "main_bg", "#00" },
    { "main_fg", "#cc" },
    { "title", "#ee" },
    { "hi_fg", "#b54040" },
    { "selected_bg", "#6a2f2f" },
    { "selected_fg", "#ee" },
    { "inactive_fg", "#40" },
    { "graph_text", "#60" },
    { "meter_bg", "#40" },
    { "proc_misc", "#0de756" },
    { "cpu_box", "#556d59" },
    { "mem_box", "#6c6c4b" },
    { "net_box", "#5c588d" },
    { "proc_box", "#805252" },
    { "div_line", "#30" },
    { "temp_start", "#4897d4" },
    { "temp_mid", "#5474e8" },
    { "temp_end", "#ff40b6" },
    { "cpu_start", "#77ca9b" },
    { "cpu_mid", "#cbc06c" },
    { "cpu_end", "#dc4c4c" },
    { "free_start", "#384f21" },
    { "free_mid", "#b5e685" },
    { "free_end", "#dcff85" },
    { "cached_start", "#163350" },
    { "cached_mid", "#74e6fc" },
    { "cached_end", "#26c5ff" },
    { "available_start", "#4e3f0e" },
    { "available_mid", "#ffd77a" },
    { "available_end", "#ffb814" },
    { "used_start", "#592b26" },
    { "used_mid", "#d9626d" },
    { "used_end", "#ff4769" },
    { "download_start", "#291f75" },
    { "download_mid", "#4f43a3" },
    { "download_end", "#b0a9de" },
    { "upload_start", "#620665" },
    { "upload_mid", "#7d4180" },
    { "upload_end", "#dcafde" },
    { "process_start", "#80d0a3" },
    { "process_mid", "#dcd179" },
    { "process_end", "#d45454" },
    { "proc_pause_bg", "#b54040" },
    { "proc_follow_bg", "#4040b5" },
    { "proc_banner_bg", "#7b407b" },
    { "proc_banner_fg", "#ee" },
    { "followed_bg", "#4040b5" },
    { "followed_fg", "#ee" },
};
```

### `hex_to_dec` Function (2-digit hex expansion logic)

```cpp
array<int, 3> hex_to_dec(string hexa) {
    if (hexa.size() > 1) {
        hexa.erase(0, 1);  // Strip leading '#'
        if (hexa.size() == 2) {
            int h_int = stoi(hexa, nullptr, 16);
            return array{h_int, h_int, h_int};  // Grayscale
        } else if (hexa.size() == 6) {
            return array{
                stoi(hexa.substr(0, 2), nullptr, 16),
                stoi(hexa.substr(2, 2), nullptr, 16),
                stoi(hexa.substr(4, 2), nullptr, 16)
            };
        }
    }
    return {-1, -1, -1};
}
```

### `main_bg` Transparency Note

From `generateColors()`:
```cpp
if (name == "main_bg" and not Config::getB("theme_background")) {
    colors[name] = "\x1b[49m";  // Reset background escape code
    rgbs[name] = {-1, -1, -1};
    continue;
}
```

When `theme_background=false` in btop config, `main_bg` is overridden from `#000000` (black) to `\x1b[49m` — the ANSI escape code that resets the background color. This allows the terminal's native background to show through. **This is NOT the default behavior** — the default is solid black (`#000000`).

---

## Summary Statistics

| Metric | Count |
|--------|-------|
| Default_theme entries | 48 |
| TTY_theme entries | 48 |
| Non-gradient colors (A1) | 15 |
| Process banner colors (A2) | 6 |
| Gradient triplets (B) | 9 (27 individual values) |
| Gradient colors with 3 stops | 7 (temp, cpu, free, cached, available, used, process) |
| Gradient colors with 3 stops (TTY) | 4 (temp, cpu, process + 1 more) |
| Gradient colors with 2 stops (TTY, mid empty) | 5 (free, cached, available, used, download, upload) |
| Non-gradient TTY entries | 15 |
| Process banner TTY entries | 6 |
