# btop Rendering Architecture Analysis

> **Task:** FN-005 | **Source:** aristocratos/btop v1.4.7 | **Author:** Fusion Research Agent

---

## 1. btop Rendering Reality Check

### What btop Actually Renders

btop renders a **dashboard of rectangular panels** using Unicode box-drawing characters and direct ANSI escape sequences written to `stdout`. It does NOT use a widget hierarchy or layout engine — it builds an output string per panel and prints the entire terminal buffer each refresh cycle.

### Key Finding: btop Does NOT Render Semi-Circular Arc Gauges

**The existing `.fusion/memory/MEMORY.md` incorrectly states that "btop uses semi-circular arc gauges." This is false.** btop renders:

- **Horizontal bar meters** (`■` U+25A0 FULL SQUARE characters, gradient-colored)
- **Line/bar graphs** using braille characters (`⣿`, `▄`, `▀`, `▌`, `▐` for interpolation)
- **ASCII text labels** for values, units, and percentages
- **Unicode box-drawing borders** for panel containers

Arc gauges are a **Tars-specific enhancement** that does not exist in btop. The Tars Inference Monitor should treat arc gauges as a custom widget requirement, not as something ported from btop.

### btop's Actual Rendering Approach

| Feature | btop Implementation | Tars Target |
|---------|-------------------|-------------|
| Panel containers | Unicode box-drawing chars (`┌`, `─`, `┐`, `│`, `└`, `┘`) | Textual borders or custom widget borders |
| Meters | Horizontal `■` character repeats, gradient-colored | Custom MeterBar Textual widget |
| Graphs | Braille cell interpolation (5×5 sub-grid per character) | Custom LineGraph Textual widget |
| Color thresholds | 101-value color gradient arrays (index 0–100) | CSS custom properties + render() color mapping |
| Panel layout | Grid calculated from terminal dimensions | Textual Container/Horizontal/Vertical nesting |

---

## 2. Meter Bar Rendering Pipeline

### Source Location

- **Declaration:** `src/btop_draw.hpp` — `namespace Draw { class Meter { ... }; }`
- **Implementation:** `src/btop_draw.cpp` — `Meter::Meter(...)`, `Meter::operator()(int value)`

### Class Definition

```cpp
namespace Draw {
    class Meter {
        int width;                          // Number of ■ cells wide
        string color_gradient;              // Gradient name (e.g., "cpu", "temp", "used")
        bool invert;                        // Reverse gradient direction
        array<string, 101> cache;           // Precomputed cache for all 0-100%
    public:
        Meter();
        Meter(const int width, string color_gradient, bool invert = false);
        string operator()(int value);       // Returns formatted string for given value
    };
}
```

### Rendering Algorithm (`operator(int value)`)

```cpp
string Meter::operator()(int value) {
    if (width < 1) return "";
    value = clamp(value, 0, 100);
    if (not cache.at(value).empty()) return cache.at(value);

    auto& out = cache.at(value);
    for (const int& i : iota(1, width + 1)) {
        int y = round((double)i * 100.0 / width);  // Threshold for this cell position
        if (value >= y)
            // Filled cell: use gradient color at threshold position
            out += Theme::g(color_gradient).at(invert ? 100 - y : y) + Symbols::meter;
        else {
            // Empty cell + remaining cells: use meter_bg color
            out += Theme::c("meter_bg") + Symbols::meter * (width + 1 - i);
            break;
        }
    }
    out += Fx::reset;  // Reset color to default
    return out;
}
```

### Rendering Order (Left to Right)

1. **Filled cells** (left): For each cell position `i`, compute threshold `y = ceil(i * 100 / width)`. If `value >= y`, render with `Theme::g(color_gradient)[y]` color + `■` character.
2. **Empty cells** (right): Once `value < y` for a position, render all remaining cells with `Theme::c("meter_bg")` color + `■` character, then break.
3. **Reset**: Append `Fx::reset` to clear color state.

### Unicode Character

- **`Symbols::meter`** = `"■"` — U+25A0 FULL SQUARE
- This is a **single-cell wide** character used for every meter cell position

### Caching Optimization

- `array<string, 101> cache` stores precomputed strings for all 101 possible values (0–100%)
- Each unique `(value, width)` combination is computed once and cached
- Subsequent calls with the same value return the cached string instantly
- This is a critical performance optimization: btop refreshes at ~2Hz and meters are used extensively

### Numerical Readouts Are Separate

**Important:** The Meter class renders ONLY the `■` character bar. The numerical value (e.g., "85%") is rendered by the *calling code*, NOT by Meter itself. For example, in `Cpu::draw()`:

```cpp
out += Mv::to(b_y + 1, b_x + 1) 
    + Theme::c("main_fg") + Fx::b + "CPU " 
    + cpu_meter(safeVal(cpu.cpu_percent, "total"s).back())
    + Theme::g("cpu").at(clamp(safeVal(cpu.cpu_percent, "total"s).back(), 0ll, 100ll))
    + rjust(to_string(safeVal(cpu.cpu_percent, "total"s).back()), 4) 
    + Theme::c("main_fg") + '%';
```

The `cpu_meter(...)` returns only the bar. The percentage number and "%" suffix are appended separately.

### Usage in btop Source

Meters are instantiated in each panel namespace:

```cpp
// In Cpu namespace:
Draw::Meter cpu_meter;          // CPU total meter
vector<Draw::Meter> gpu_meters; // Per-GPU meters

// In Mem namespace:
std::unordered_map<string, Draw::Meter> mem_meters;  // Per-memory-type meters
std::unordered_map<string, Draw::Meter> disk_meters_used;
std::unordered_map<string, Draw::Meter> disk_meters_free;

// In Gpu namespace:
vector<Draw::Meter> gpu_meter_vec;
vector<Draw::Meter> pwr_meter_vec;
vector<Draw::Meter> enc_meter_vec;
```

---

## 3. Graph Rendering Structure

### Source Location

- **Declaration:** `src/btop_draw.hpp` — `namespace Draw { class Graph { ... }; }`
- **Implementation:** `src/btop_draw.cpp` — `Graph::_create()`, `Graph::Graph()`, `Graph::operator()(const deque<long long>&, bool)`, `Graph::operator()()`

### Class Definition

```cpp
namespace Draw {
    class Graph {
        int width, height;
        string color_gradient;
        string out, symbol = "default";
        bool invert, no_zero;
        long long offset, last = 0, max_value = 0;
        bool current = true, tty_mode = false;
        // Double-buffer: two representations to switch between
        std::unordered_map<bool, vector<string>> graphs = { {true, {}}, {false, {}} };
    public:
        Graph();
        Graph(int width, int height, const string& color_gradient,
              const deque<long long>& data, const string& symbol="default",
              bool invert=false, bool no_zero=false,
              long long max_value=0, long long offset=0);
        string& operator()(const deque<long long>& data, bool data_same=false);
        string& operator()();
    };
}
```

### Unicode Character Selection (6 Symbol Types)

btop uses **braille characters** for sub-cell interpolation. The `Symbols::graph_symbols` `unordered_map<string, vector<string>>` contains 6 keys, each with 25 character entries representing all possible 5×5 sub-grid patterns of a braille cell:

| Key | Description | Example Characters |
|-----|-------------|-------------------|
| `braille_up` | Fills top half of braille cell (line graph) | ` `, `⢀`, `⢠`, `⢰`, `⢸`, `⡀`, `⣀`, `⣠`, `⣰`, `⣸`, `⡄`, `⣄`, `⣤`, `⣴`, `⣼`, `⡆`, `⣆`, `⣦`, `⣶`, `⣾`, `⡇`, `⣇`, `⣧`, `⣷`, `⣿` |
| `braille_down` | Fills bottom half of braille cell | ` `, `⠈`, `⠘`, `⠸`, `⢸`, `⠁`, `⠉`, `⠙`, `⠹`, `⢹`, `⠃`, `⠋`, `⠛`, `⠻`, `⢻`, `⠇`, `⠏`, `⠟`, `⠿`, `⢿`, `⡇`, `⡏`, `⡟`, `⡿`, `⣿` |
| `block_up` | Block fills top half | ` `, `▗`, `▐`, `▖`, `▄`, `▟`, `▌`, `▙`, `█` |
| `block_down` | Block fills bottom half | ` `, `▝`, `▐`, `▘`, `▀`, `▜`, `▌`, `▛`, `█` |
| `tty_up` | TTY-mode: ░▒█ block fill up | ` `, `░`, `▒`, `█` |
| `tty_down` | TTY-mode: ░▒█ block fill down | ` `, `░`, `▒`, `█` |

### Data Interpolation Algorithm

The graph maps pairs of adjacent data points to a single braille character using a 5×5 sub-grid:

1. **Two values per cell:** Each braille cell encodes TWO adjacent data values (current and previous)
2. **Height mapping:** For each row `horizon` (0 to `height-1`):
   - `cur_high = round(100.0 * (height - horizon) / height)` — upper bound
   - `cur_low = round(100.0 * (height - (horizon + 1)) / height)` — lower bound
3. **Value quantization:** For each of the two values (previous `last`, current `data_value`):
   - If `value >= cur_high` → `result[ai] = 4` (fully filled)
   - If `value <= cur_low` → `result[ai] = clamp_min` (empty)
   - Otherwise: `result[ai] = clamp(round((value - cur_low) * 4 / (cur_high - cur_low) + 0.1), clamp_min, 4)`
4. **Character selection:** `graph_symbol[(result[0] * 5 + result[1])]` — maps the 5×5 sub-grid index (0-24) to one of 25 braille characters

### Double-Buffering

```cpp
std::unordered_map<bool, vector<string>> graphs = { {true, {}}, {false, {}} };
// Toggle with: current = not current;
```

Two buffers are maintained (`true` and `false`), and the `current` flag toggles between them on each update. This allows btop to render the next graph frame while the current frame is displayed (conceptually).

### Update Algorithm (`operator()(const deque<long long>& data, bool data_same)`)

```cpp
string& Graph::operator()(const deque<long long>& data, bool data_same) {
    if (data_same) return out;  // No-op if data hasn't changed

    // Shift existing characters left (remove first char from each row)
    if (not tty_mode) current = not current;
    for (const int& i : iota(0, height)) {
        if (height == 1 and graphs.at(current).at(i).at(1) == '[') {
            // ANSI escape sequence prefix handling
            if (graphs.at(current).at(i).at(3) == 'C')
                graphs.at(current).at(i).erase(0, 4);
            else
                graphs.at(current).at(i).erase(0, graphs.at(current).at(i).find_first_of('m') + 4);
        }
        else if (graphs.at(current).at(i).at(0) == ' ')
            graphs.at(current).at(i).erase(0, 1);
        else
            graphs.at(current).at(i).erase(0, 3);  // Multi-byte Unicode
    }
    this->_create(data, (int)data.size() - 1);  // Render new data point
    return out;
}
```

### Single-Row vs Multi-Row Graphs

- **Height 1:** Single line of braille characters. Only the first (current) buffer is returned.
- **Height > 1:** Each row renders at a different vertical position. The `out` string uses `Mv::l(width)` + `Mv::d(1)` to wrap between rows.

### Tars Inference Mapping

For the inference monitor:
- `GraphWidget` currently uses Plotext (a Python library) — replacing this requires a custom Textual widget
- Data comes from `deque<long long>` — the Tars engine provides `deque<float>` via `GraphWidget.data`
- The 5×5 braille interpolation approach is the key insight: each cell encodes TWO adjacent data points, effectively doubling horizontal resolution

---

## 4. Unicode Character Reference (Symbols Namespace)

### Source Location

- **All symbols declared:** `src/btop_draw.hpp` — `namespace Symbols { ... }` (inline string literals, header-only)
- **Additional definitions:** `src/btop_draw.cpp` — `Symbols::meter`, `Symbols::superscript`, `Symbols::graph_symbols` (runtime initialization)

### Complete Character Map

**Horizontal/Vertical Lines:**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `h_line` | `"─"` | U+2500 | BOX DRAWINGS LIGHT HORIZONTAL |
| `v_line` | `"│"` | U+2502 | BOX DRAWINGS LIGHT VERTICAL |
| `dotted_v_line` | `"╎"` | U+250E | BOX DRAWINGS LIGHT VERTICAL AND RIGHT |

**Sharp Corners:**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `left_up` | `"┌"` | U+250C | BOX DRAWINGS LIGHT DOWN AND RIGHT |
| `right_up` | `"┐"` | U+2510 | BOX DRAWINGS LIGHT DOWN AND LEFT |
| `left_down` | `"└"` | U+2514 | BOX DRAWINGS LIGHT UP AND RIGHT |
| `right_down` | `"┘"` | U+2518 | BOX DRAWINGS LIGHT UP AND LEFT |

**Rounded Corners:**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `round_left_up` | `"╭"` | U+256D | LEFT SEMICIRCLE |
| `round_right_up` | `"╮"` | U+256E | RIGHT SEMICIRCLE |
| `round_left_down` | `"╰"` | U+2570 | LEFT SEMICIRCLE |
| `round_right_down` | `"╯"` | U+2571 | RIGHT SEMICIRCLE |

**Title Area (for panel headers):**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `title_left` | `"┐"` | U+2510 | Same as right_up |
| `title_right` | `"┌"` | U+250C | Same as left_up |
| `title_left_down` | `"┘"` | U+2518 | Same as right_down |
| `title_right_down` | `"└"` | U+2514 | Same as left_down |

**Dividers:**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `div_right` | `"┤"` | U+2524 | BOX DRAWINGS LIGHT VERTICAL AND LEFT |
| `div_left` | `"├"` | U+251C | BOX DRAWINGS LIGHT VERTICAL AND RIGHT |
| `div_up` | `"┬"` | U+252C | BOX DRAWINGS LIGHT DOWN AND HORIZONTAL |
| `div_down` | `"┴"` | U+2534 | BOX DRAWINGS LIGHT UP AND HORIZONTAL |

**Directional Arrows:**
| Name | Value | Unicode | Description |
|------|-------|---------|-------------|
| `up` | `"↑"` | U+2191 | UPWARDS ARROW |
| `down` | `"↓"` | U+2193 | DOWNWARDS ARROW |
| `left` | `"←"` | U+2190 | LEFTWARDS ARROW |
| `right` | `"→"` | U+2192 | RIGHTWARDS ARROW |
| `enter` | `"↵"` | U+21B5 | DOWNWARDS ARROW WITH CORNER LEFTWARDS |

**Graph Symbols (braille-based):**
| Key | Type | Characters |
|-----|------|------------|
| `braille_up` | 25-element vector | ` `, `⢀`, `⢠`, `⢰`, `⢸`, `⡀`, `⣀`, `⣠`, `⣰`, `⣸`, `⡄`, `⣄`, `⣤`, `⣴`, `⣼`, `⡆`, `⣆`, `⣦`, `⣶`, `⣾`, `⡇`, `⣇`, `⣧`, `⣷`, `⣿` |
| `braille_down` | 25-element vector | ` `, `⠈`, `⠘`, `⠸`, `⢸`, `⠁`, `⠉`, `⠙`, `⠹`, `⢹`, `⠃`, `⠋`, `⠛`, `⠻`, `⢻`, `⠇`, `⠏`, `⠟`, `⠿`, `⢿`, `⡇`, `⡏`, `⡟`, `⡿`, `⣿` |
| `block_up` | 25-element vector | ` `, `▗`, `▗`, `▐`, `▐`, `▖`, `▄`, `▄`, `▟`, `▟`, `▖`, `▄`, `▄`, `▟`, `▟`, `▌`, `▙`, `▙`, `█`, `█`, `▌`, `▙`, `▙`, `█`, `█` |
| `block_down` | 25-element vector | ` `, `▝`, `▝`, `▐`, `▐`, `▘`, `▀`, `▀`, `▜`, `▜`, `▘`, `▀`, `▀`, `▜`, `▜`, `▌`, `▛`, `▛`, `█`, `█`, `▌`, `▛`, `▛`, `█`, `█` |
| `tty_up` | 25-element vector | ` `, `░`, `░`, `▒`, `▒`, `░`, `░`, `▒`, `▒`, `█`, `░`, `▒`, `▒`, `▒`, `█`, `▒`, `▒`, `▒`, `█`, `█`, `▒`, `█`, `█`, `█`, `█` |
| `tty_down` | 25-element vector | ` `, `░`, `░`, `▒`, `▒`, `░`, `░`, `▒`, `▒`, `█`, `░`, `▒`, `▒`, `▒`, `█`, `▒`, `▒`, `▒`, `█`, `█`, `▒`, `█`, `█`, `█`, `█` |

**Meter Character:**
| Name | Value | Unicode |
|------|-------|---------|
| `meter` | `"■"` | U+25A0 FULL SQUARE |

**Superscript Digits (for panel numbering):**
| Index | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|-------|---|---|---|---|---|---|---|---|---|---|
| Char | `⁰` | `¹` | `²` | `³` | `⁴` | `⁵` | `⁶` | `⁷` | `⁸` | `⁹` |

### Characters That Do NOT Exist in Symbols

The following names are **NOT** defined in the btop `Symbols` namespace (do not invent them):
- `cross_h`, `cross_v`
- `header_h`, `header_v`
- `meter_tick`
- `arc_start`, `arc_end`

---

## 5. Panel & Border System

### Source Location

- **`createBox()` function:** `src/btop_draw.cpp`
- **`calcSizes()` function:** `src/btop_draw.cpp` — calculates all panel dimensions and positions
- **Size data structures:** Each panel namespace (`Cpu`, `Mem`, `Net`, `Proc`, `Gpu`) defines `x, y, width, height, min_width, min_height`

### createBox() Function

```cpp
string createBox(
    const int x, const int y, const int width, const int height,
    string line_color = "",
    bool fill = false,
    const std::string_view title = "",
    const std::string_view title2 = "",
    const int num = 0
);
```

### Border Assembly Algorithm

1. **Reset + color:** `out = Fx::reset + line_color`
2. **Top and bottom horizontal lines:**
   - For y-positions `{y, y + height - 1}`: `Mv::to(hpos, x) + h_line * (width - 1)`
3. **Vertical lines (interior rows):**
   - For each `hpos` in `{y+1, ..., y+height-2}`:
     - `Mv::to(hpos, x) + v_line + (fill ? (width-2) spaces : Mv::r(width-2)) + v_line`
   - If `fill=true`, the interior is filled with spaces; otherwise it's blank (no content)
4. **Corners:**
   ```
   Mv::to(y, x) + left_up
   Mv::to(y, x + width - 1) + right_up
   Mv::to(y + height - 1, x) + left_down
   Mv::to(y + height - 1, x + width - 1) + right_down
   ```
5. **Title bar:** If title is provided:
   ```
   Mv::to(y, x + 2) + title_left + Fx::b + numbering + Theme::c("title") + title + Fx::ub + line_color + title_right
   ```
   - Uses `title_left` (`┐`) and `title_right` (`┌`) — **not** corner characters
   - Panel number shown as superscript (`⁰`–`⁹`) before title text
6. **Second title** (bottom of panel): Same format but uses `title_left_down` (`┘`) and `title_right_down` (`└`)
7. **Return cursor position:** `out + Fx::reset + Mv::to(y + 1, x + 1)`

### Rounded Corners Toggle

```cpp
auto rounded = Config::getB("rounded_corners");
const auto& right_up = (tty_mode or not rounded ? Symbols::right_up : Symbols::round_right_up);
const auto& left_up = (tty_mode or not rounded ? Symbols::left_up : Symbols::round_left_up);
const auto& right_down = (tty_mode or not rounded ? Symbols::right_down : Symbols::round_right_down);
const auto& left_down = (tty_mode or not rounded ? Symbols::left_down : Symbols::round_left_down);
```

When `rounded_corners` is true (default), btop uses `╭╮╰╯` instead of `┌┐└┘`. In `tty_mode`, sharp corners are always used.

### Panel Layout Architecture (`calcSizes()`)

Each panel namespace maintains its own position and dimension:

```cpp
namespace Cpu {
    int x = 1, y = 1, width = 20, height;    // Panel bounds
    int b_x, b_y, b_width, b_height;          // Inner stats box bounds
    int b_columns, b_column_size;             // Core column grid
}
namespace Mem {
    int x = 1, y, width = 20, height;
    int mem_width, disks_width, divider;
}
namespace Net {
    int x = 1, y, width = 20, height;
    int b_x, b_y, b_width, b_height;
}
namespace Proc {
    int x, y, width = 20, height;
}
```

Layout is computed dynamically based on:
- Terminal dimensions (`Term::width`, `Term::height`)
- Panel percentage sizes (e.g., `Cpu::width_p = 100`, `Mem::width_p = 45`)
- Minimum panel sizes (e.g., `Cpu::min_width = 60`, `Mem::min_width = 36`)
- Shown panel flags (`Cpu::shown`, `Mem::shown`, etc.)
- Configuration options (`cpu_bottom`, `proc_left`, `mem_below_net`)

The layout is a **grid of full-width panels** stacked vertically by default. With GPU support, GPU panels can be inserted between CPU and other panels.

### Resize Handling

Terminal resize events trigger `SIGWINCH` → `term_resize()` → `Term::refresh()` → `Draw::calcSizes()`. The entire panel layout is recalculated and all panel contents are redrawn.

---

## 6. Table & Threshold Color Logic

### Theme Access Functions

From `btop_theme.hpp`:

```cpp
namespace Theme {
    // Flat color lookup by name
    inline const string& c(const string& name) { return colors.at(name); }

    // Gradient color lookup by name and index (0-100)
    inline const array<string, 101>& g(const string& name) { return gradients.at(name); }

    // RGB decimal lookup by name
    inline const std::array<int, 3>& dec(const string& name) { return rgbs.at(name); }
}
```

### Data Flow for Threshold Colors

1. **`Default_theme`** (`std::unordered_map<string, string>`) — Contains hex color definitions
2. **`generateColors()`** — Converts hex to ANSI escape sequences and RGB decimal arrays
3. **`generateGradients()`** — Creates 101-value gradient arrays from start/mid/end color triplets
4. **`Theme::c(name)`** — Returns pre-formatted ANSI escape for a flat color
5. **`Theme::g(name)`** — Returns array of 101 pre-formatted ANSI escapes indexed 0-100

### Default Theme Keys and Hex Values

**Complete `Default_theme` map (verified from btop_theme.cpp):**

```cpp
// Background & text
"main_bg"       = "#00"       // #000000 (black)
"main_fg"       = "#cc"       // #cccccc (light gray)
"title"         = "#ee"       // #eeeeee (near-white)
"hi_fg"         = "#b54040"   // Bright red (for hotkeys)
"selected_bg"   = "#6a2f2f"   // Dark red (selected row bg)
"selected_fg"   = "#ee"       // Near-white (selected row fg)
"inactive_fg"   = "#40"       // #404040 (dim gray)
"graph_text"    = "#60"       // #606060 (dim gray for axis labels)
"meter_bg"      = "#40"       // #404040 (meter background)
"proc_misc"     = "#0de756"   // Bright green (running process indicator)

// Panel borders
"cpu_box"       = "#556d59"   // Muted green-gray
"mem_box"       = "#6c6c4b"   // Olive drab
"net_box"       = "#5c588d"   // Slate blue
"proc_box"      = "#805252"   // Muted red-brown

// Divider
"div_line"      = "#30"       // #303030 (very dark gray)

// CPU metric gradient (start → mid → end)
"cpu_start"     = "#77ca9b"   // Soft green
"cpu_mid"       = "#cbc06c"   // Yellow-olive
"cpu_end"       = "#dc4c4c"   // Red

// Temperature gradient (start → mid → end)
"temp_start"    = "#4897d4"   // Blue
"temp_mid"      = "#5474e8"   // Purple-blue
"temp_end"      = "#ff40b6"   // Pink/magenta

// Memory gradients
"used_start"    = "#592b26"   // Dark red-brown
"used_mid"      = "#d9626d"   // Rose
"used_end"      = "#ff4769"   // Bright red
"free_start"    = "#384f21"   // Dark green
"free_mid"      = "#b5e685"   // Light green
"free_end"      = "#dcff85"   // Bright yellow-green
"available_start" = "#4e3f0e" // Dark gold
"available_mid"   = "#ffd77a" // Light gold
"available_end"   = "#ffb814" // Bright gold
"cached_start"  = "#163350"   // Dark blue
"cached_mid"    = "#74e6fc"   // Cyan
"cached_end"    = "#26c5ff"   // Bright cyan

// Network gradients
"download_start" = "#291f75"  // Dark blue-violet
"download_mid"   = "#4f43a3"  // Blue-violet
"download_end"   = "#b0a9de"  // Light lavender
"upload_start"   = "#620665"  // Dark magenta
"upload_mid"     = "#7d4180"  // Purple-magenta
"upload_end"     = "#dcafde"  // Light magenta

// Process gradient
"process_start"  = "#80d0a3"  // Mint green
"process_mid"    = "#dcd179"  // Warm yellow
"process_end"    = "#d45454"  // Red

// Process UI backgrounds
"proc_pause_bg"     = "#b54040"   // Red (paused list bg)
"proc_follow_bg"    = "#4040b5"   // Blue (follow mode bg)
"proc_banner_bg"    = "#7b407b"   // Purple (banner bg)
"proc_banner_fg"    = "#ee"       // Near-white
"followed_bg"       = "#4040b5"   // Blue (followed process bg)
"followed_fg"       = "#ee"       // Near-white
```

**Keys that do NOT exist in `Default_theme`:**
- `mem_start`, `mem_mid`, `mem_end`
- `gpu_start`, `gpu_mid`, `gpu_end`
- `net_start`, `net_mid`, `net_end`
- `border_color`

### Gradient Generation Algorithm

```cpp
void generateGradients() {
    // For each color group with _start suffix:
    for (const auto& [name, source_arr] : rgbs) {
        if (not name.ends_with("_start")) continue;
        const string color_name = rtrim(name, "_start");

        // Three color stops: start, mid, end
        const array<array<int, 3>, 3> input_colors = {
            source_arr,                                          // start RGB
            rgbs[color_name + "_mid"],                           // mid RGB (may be {-1,-1,-1})
            rgbs[color_name + "_end"]                            // end RGB (may be {-1,-1,-1})
        };

        array<array<int, 3>, 101> output_colors;

        if (input_colors[2][0] >= 0) {
            // Two-pass interpolation: start→mid (0-50), mid→end (51-100)
            int current_range = (input_colors[1][0] >= 0) ? 50 : 100;
            for (int rgb : iota(0, 3)) {
                int start_idx = 0, end_idx = (current_range == 50) ? 1 : 2;
                int offset = 0;
                for (int i : iota(0, 101)) {
                    output_colors[i][rgb] = input_colors[start_idx][rgb]
                        + (i - offset) * (input_colors[end_idx][rgb] - input_colors[start_idx][rgb]) / current_range;
                    if (i == current_range) { ++start_idx; ++end_idx; offset = 50; }
                }
            }
        }

        // Convert RGB arrays to ANSI escape strings
        array<string, 101> color_gradient;
        for (int y = 0; const auto& [red, green, blue] : output_colors)
            color_gradient[y++] = dec_to_color(red, green, blue, t_to_256);

        gradients[color_name] = std::move(color_gradient);
    }
}
```

### Threshold Color Application

In btop, thresholds are **continuous** — values map to gradient indices:

```cpp
// CPU: 0% = green (gradient[0]), 100% = red (gradient[100])
Theme::g("cpu").at(clamp(value, 0ll, 100ll))

// Temperature: 0°C = blue, max_temp = magenta
Theme::g("temp").at(clamp(value * 100 / safe_cpu_temp_max, 0ll, 100ll))

// Memory used: 0% = dark red, 100% = bright red
Theme::g("used").at(value)
```

There are **no discrete thresholds** (green/yellow/red at fixed values). Instead, each metric has its own 3-stop gradient that smoothly interpolates. The `cpu` gradient (green→olive→red) is the most commonly used because it serves CPU, GPU, and process coloring.

### Per-Metric Color Assignment

| Metric Group | Gradient Key | Purpose |
|-------------|-------------|---------|
| CPU utilization | `cpu` | Green→olive→red (0→50→100%) |
| Temperature | `temp` | Blue→purple→magenta (0°C→max) |
| Memory used | `used` | Dark red→rose→bright red |
| Memory free | `free` | Dark green→light green→bright yellow-green |
| Memory available | `available` | Dark gold→light gold→bright gold |
| Memory cached | `cached` | Dark blue→cyan→bright cyan |
| Network download | `download` | Dark blue-violet→blue-violet→lavender |
| Network upload | `upload` | Dark magenta→purple-magenta→light magenta |
| Process coloring | `process` | Mint green→warm yellow→red |
| Power draw | `cached` | Uses the cached gradient (dark blue→cyan→bright cyan) |

Note: **GPU metrics reuse CPU gradients** — there are no dedicated GPU color keys in `Default_theme`.

---

## 7. Textual Widget Design Mappings

### 7a. Unicode Character Selection for Textual

All btop Unicode characters are standard Unicode and render correctly in Textual terminals that support them:

| btop Character | Unicode | Textual Equivalent | Notes |
|---------------|---------|-------------------|-------|
| `■` (U+25A0) | FULL SQUARE | `■` | Meter bar cells. Cell width = 1 |
| `─` (U+2500) | LIGHT HORIZONTAL | `─` | Horizontal panel borders |
| `│` (U+2502) | LIGHT VERTICAL | `│` | Vertical panel borders |
| `┌┐└┘` | BOX DRAWINGS | Same | Sharp corners |
| `╭╮╰╯` | SEMICIRCLES | Same | Rounded corners |
| `├┤┬┴` | BOX DRAWINGS | Same | Dividers |
| `⣿` etc. | BRAILLE | Same | Graph characters. Each is 1 cell wide |
| `▄▀▌▐` | BLOCK ELEMENTS | Same | TTY-mode graph chars |

**Textual rendering constraints:**
- All characters must be **single-cell wide** (width=1) — Textual's `render()` works with character cells
- Braille characters are single-cell wide but may not render correctly in all terminal fonts
- Terminal font support for Unicode box-drawing and braille varies — some terminals may show squares or question marks
- **No multi-width characters** should be used in meter bars or graph rows

### 7b. MeterBar Widget Concept

```python
from textual.widget import Widget
from textual.css.query import NoMatches
from textual.color import Color

class MeterBar(Widget):
    """Replicates btop's Draw::Meter horizontal bar meter."""

    # CSS default size: height=1, width varies by parent container
    DEFAULT_CSS = """
    MeterBar {
        height: 1;
        width: 1fr;
    }
    """

    def __init__(
        self,
        current_value: float = 0.0,
        min_value: float = 0.0,
        max_value: float = 100.0,
        label: str = "",
        gradient_colors: list[str] | None = None,
        bg_color: str | None = None,
        show_label: bool = False,
        show_value: bool = True,
    ):
        super().__init__()
        self.current_value = current_value
        self.min_value = min_value
        self.max_value = max_value
        self.label = label
        # Gradient colors: [start_hex, mid_hex, end_hex] — maps to btop's start/mid/end
        self.gradient_colors = gradient_colors or ["#77ca9b", "#cbc06c", "#dc4c4c"]  # cpu default
        self.bg_color = bg_color or "#404040"
        self.show_label = show_label
        self.show_value = show_value
        self._cache: dict[int, str] = {}  # value_index -> rendered string (like btop's array<string, 101>)

    def _value_to_index(self) -> int:
        pct = max(0, min(100, (self.current_value - self.min_value) / (self.max_value - self.min_value) * 100))
        return int(round(pct))

    def _get_gradient_color(self, index: int) -> str:
        """Get color at gradient index 0-100 using 3-stop interpolation."""
        if index <= 0:
            return self.gradient_colors[0]
        elif index >= 100:
            return self.gradient_colors[2]
        elif len(self.gradient_colors) == 3:
            mid = 50
            if index <= mid:
                t = index / mid
                return self._blend(self.gradient_colors[0], self.gradient_colors[1], t)
            else:
                t = (index - mid) / 50
                return self._blend(self.gradient_colors[1], self.gradient_colors[2], t)
        else:
            return self.gradient_colors[0]

    def _blend(self, c1: str, c2: str, t: float) -> str:
        """Blend two hex colors."""
        # Implementation: parse hex, interpolate RGB channels, return hex
        pass

    def render(self) -> str:
        """Render the meter bar as a single-line string, like btop's operator()."""
        idx = self._value_to_index()
        if idx in self._cache:
            return self._cache[idx]

        # Get terminal width for this widget
        width = self.content_width
        if width < 1:
            return ""

        parts = []
        if self.show_label and self.label:
            parts.append(f"{self.label}: ")

        filled = 0
        for i in range(1, width + 1):
            threshold = round(i * 100.0 / width)
            if idx >= threshold:
                color = self._get_gradient_color(threshold)
                parts.append(f"\x1b[38;2;{self._hex_to_rgb(color)}m■")
                filled += 1
            else:
                # Remaining cells: background color
                color = self.bg_color
                remaining = width + 1 - i
                parts.append(f"\x1b[38;2;{self._hex_to_rgb(color)}m{'■' * remaining}")
                break

        # Value text
        if self.show_value:
            parts.append(f"\x1b[0m {idx}%")

        result = "".join(parts) + "\x1b[0m"
        self._cache[idx] = result
        return result
```

**Key design decisions vs. btop:**
- Textual's `render()` returns a string (same as btop's `operator()` returning string)
- Cache key is the integer percentage (same as btop's `array<string, 101>`)
- Color interpolation is done in Python (btop uses C++ gradient generation)
- Width comes from Textual's layout system (`self.content_width`), not a fixed parameter

### 7c. LineGraph Widget Concept

```python
class LineGraph(Widget):
    """Replicates btop's Draw::Graph line/bar graph with braille interpolation."""

    DEFAULT_CSS = """
    LineGraph {
        height: 3;  # 3 rows = 3 levels of vertical resolution
        width: 1fr;
    }
    """

    def __init__(
        self,
        title: str = "",
        color_gradient: list[str] | None = None,
        symbol: str = "braille_up",  # "braille_up", "block_up", "tty_up"
        invert: bool = False,
        y_min: float = 0.0,
        y_max: float = 100.0,
        buffer_size: int = 120,  # MAX_BUFFER from monitor.py
    ):
        super().__init__()
        self.title = title
        self.gradient_colors = color_gradient or ["#77ca9b", "#cbc06c", "#dc4c4c"]
        self.symbol = symbol
        self.invert = invert
        self.y_min = y_min
        self.y_max = y_max
        self.data: deque[float] = deque(maxlen=buffer_size)
        self._double_buffer = {True: [], False: []}
        self._current_buffer = True

    def update_data(self, value: float):
        """Add a data point and trigger re-render. Like btop's operator()(data, data_same)."""
        self.data.append(value)
        self.refresh()

    def _braille_symbol_index(self, result0: int, result1: int) -> int:
        """Map two quantized values (0-4) to braille cell index (0-24).
        Like btop: result[0] * 5 + result[1]"""
        return result0 * 5 + result1

    def render(self) -> str:
        """Render the graph. Each cell encodes two adjacent data points."""
        if len(self.data) < 2:
            return ""

        width = self.content_width
        height = self.content_height

        # Get braille character set
        symbols = self._get_braille_symbols()  # 25-char vector

        lines = []
        for row in range(height):
            high = round(100.0 * (height - row) / height)
            low = round(100.0 * (height - (row + 1)) / height)

            row_str = []
            for i in range(0, len(self.data) - 1, 2):
                # Two adjacent values per braille cell
                val0 = self._quantize(self.data[i], low, high)
                val1 = self._quantize(self.data[i + 1], low, high) if i + 1 < len(self.data) else 0
                idx = self._braille_symbol_index(val0, val1)
                row_str.append(symbols[idx])

            lines.append("".join(row_str))

        return "\n".join(lines)

    def _quantize(self, value: float, low: int, high: int) -> int:
        """Quantize value to 0-4 scale for braille sub-cell position."""
        pct = (value - self.y_min) / (self.y_max - self.y_min) * 100
        if pct >= high:
            return 4
        elif pct <= low:
            return 0
        else:
            return int(round((pct - low) * 4 / (high - low) + 0.1))
```

**Key design decisions vs. btop:**
- Textual's `render()` returns a multi-line string (one line per row)
- Browsers need to support braille characters — consider `block_up`/`block_down` as fallback
- Rolling buffer via `deque(maxlen=buffer_size)` matches btop's `deque<long long>` approach
- Double-buffering is handled by Textual's widget system (no manual buffer swap needed)

### 7d. Theme System Design

**btop's theme access patterns → Textual CSS custom properties:**

btop uses two access patterns:
1. `Theme::c("main_fg")` — flat color lookup
2. `Theme::g("cpu").at(85)` — gradient lookup by index

Textual maps to CSS custom properties:

```css
/* In monitor.tcss or a dedicated theme file */
Screen {
    /* Background & text */
    --btop-main-bg: #000000;
    --btop-main-fg: #cccccc;
    --btop-title: #eeeeee;
    --btop-hi-fg: #b54040;
    --btop-inactive: #404040;
    --btop-graph-text: #606060;

    /* Borders */
    --btop-cpu-box: #556d59;
    --btop-mem-box: #6c6c4b;
    --btop-net-box: #5c588d;
    --btop-proc-box: #805252;
    --btop-div-line: #303030;

    /* CPU metric gradient (0% → 100%) */
    --btop-cpu-start: #77ca9b;
    --btop-cpu-mid: #cbc06c;
    --btop-cpu-end: #dc4c4c;

    /* Temperature gradient */
    --btop-temp-start: #4897d4;
    --btop-temp-mid: #5474e8;
    --btop-temp-end: #ff40b6;

    /* Memory gradients */
    --btop-used-start: #592b26;
    --btop-used-mid: #d9626d;
    --btop-used-end: #ff4769;
    --btop-free-start: #384f21;
    --btop-free-mid: #b5e685;
    --btop-free-end: #dcff85;

    /* Meter background */
    --btop-meter-bg: #404040;

    /* Process colors */
    --btop-process-start: #80d0a3;
    --btop-process-mid: #dcd179;
    --btop-process-end: #d45454;
}
```

**Widget rendering consuming CSS variables:**
- `MeterBar` reads `--btop-cpu-start/mid/end` for gradient, `--btop-meter-bg` for empty cells
- `LineGraph` reads `--btop-cpu-start/mid/end` for graph line colors
- Panel borders use `--btop-cpu-box`, `--btop-mem-box`, etc.

### 7e. Layout Grid Concept

**btop's panel grid → Textual container hierarchy:**

```
Screen
├── Header (top-docked)
├── Horizontal (main layout)
│   ├── Vertical (left: sidebar, ~30% width)
│   │   ├── MeterBar + LineGraph (GPU load)
│   │   ├── MeterBar + LineGraph (Memory)
│   │   ├── Horizontal
│   │   │   ├── MeterBar + LineGraph (Power) [half-width]
│   │   │   └── MeterBar + LineGraph (Temperature) [half-width]
│   │   └── Footer
│   └── Vertical (right: main-content, ~70% width)
│       ├── DataTable (slot table)
│       ├── Container (active slots with MeterBar widgets)
│       └── RichLog (bottom logs)
└── Footer (bottom-docked)
```

**Panel borders in Textual:**
- Use Textual's built-in `border` property: `border: round $primary` or `border: panel #333`
- For btop-style Unicode borders (not Textual's rounded corners): Custom widget with `render()` that draws `┌─┐│ │└─┘` borders manually
- Rounded corners: Use `border: heavy $primary` with a custom border render method

**Resize handling:**
- Textual's `on_resize` event → recalculate widget sizes → `self.refresh()`
- Equivalent to btop's `SIGWINCH → term_resize → Draw::calcSizes()`

---

## 8. Integration Recommendations for Tars Inference Monitor

### 8a. Architecture Mapping

| btop Concept | Tars Implementation |
|-------------|-------------------|
| `Runner::run("all")` — periodic full redraw | `set_interval(1/REFRESH_RATE, update_telemetry)` |
| `Term::refresh()` — terminal sync | Textual's `self.refresh()` per widget |
| `Draw::calcSizes()` — layout recalc | Textual's automatic layout system |
| `Draw::Meter` — horizontal bar | Custom `MeterBar(Widget)` class |
| `Draw::Graph` — line graph | Custom `LineGraph(Widget)` class |
| `Cpu::draw()` — panel builder | `GPUMonitor(Widget)` compose + render methods |
| `Mem::draw()` — panel builder | `MemoryMonitor(Widget)` |
| `Symbols::meter` = `■` | Same Unicode character |
| `Symbols::graph_symbols` (braille) | Same Unicode characters |
| `Default_theme` color map | Textual CSS `:root` custom properties |

### 8b. Immediate Next Steps

1. **FN-006 (Color Palette):** Define the Tars-specific color palette derived from btop's `Default_theme`. The mapping should use the same hex values as btop's defaults for consistency.

2. **FN-007 (Layout Grid):** Design the btop-style panel layout using Textual's Container system. Key insight: btop's panels are rectangular with Unicode borders, arranged in a responsive grid.

3. **FN-008 (Design Spec):** Combine the above into a detailed implementation specification with component diagrams and CSS specifications.

### 8c. Tars-Specific Enhancements (Not from btop)

| Feature | Status | Notes |
|---------|--------|-------|
| Semi-circular arc gauges | Tars-specific | NOT in btop. Requires custom widget design |
| GPU inference slot tracking | Tars-specific | btop has process lists but not LLM slot tracking |
| TPS/KV cache visualization | Tars-specific | btop graphs system metrics, not inference throughput |
| Slot activity indicators | Tars-specific | btop has process states but not "BUSY"/"IDLE" for inference |

### 8d. Known Textual Limitations

1. **No direct character-cell manipulation:** Textual's widget model doesn't allow writing arbitrary characters at specific terminal positions like btop's `Mv::to(y, x) + char`. The closest equivalent is `render()` returning a formatted string.

2. **No escape sequence passthrough:** Textual processes all text through its rendering pipeline. Raw ANSI escape codes in `render()` output may be stripped or transformed. **Workaround:** Use Textual's `text-style` CSS and color properties instead of raw escapes.

3. **Braille character rendering:** Braille Unicode characters (⣿, ⢀, etc.) may not display in all terminal fonts. **Workaround:** Implement a symbol selector allowing fallback to block characters (▄, ▀, ▌, ▐).

4. **Double-buffering not needed:** Textual's widget rendering is already double-buffered. The manual buffer swap pattern from btop's `graphs[current]` / `graphs[!current]` is unnecessary.

5. **String concatenation overhead:** btop builds one huge string per refresh. Textual's widget tree is rendered incrementally. The performance model is fundamentally different.

6. **No cursor movement primitives:** Textual doesn't expose `Mv::to(row, col)` cursor positioning. Layout is handled by the Container/Widget hierarchy.

---

## Appendix A: btop Source File Inventory (Verified)

| File | Purpose | Key Content |
|------|---------|-------------|
| `src/btop_draw.hpp` | Drawing declarations | `Draw::Meter`, `Draw::Graph`, `Draw::createBox()`, `Symbols` namespace |
| `src/btop_draw.cpp` | Drawing implementation | `Meter::operator()`, `Graph::_create()`, `createBox()`, `calcSizes()` |
| `src/btop_theme.hpp` | Theme declarations | `Theme::c()`, `Theme::g()`, `Theme::dec()` |
| `src/btop_theme.cpp` | Theme implementation | `Default_theme`, `generateColors()`, `generateGradients()` |
| `src/btop.cpp` | Main program | `Runner::run()`, main loop, `term_resize()`, `btop_main()` |
| `src/main.cpp` | Entry point | `main()` function |
| `src/btop_shared.hpp` | Shared declarations | `Cpu::draw()`, `Mem::draw()`, `Net::draw()`, `Gpu::draw()` signatures |
| `src/btop_shared.cpp` | Shared utilities | `Cpu::trim_name()`, GPU data helpers |
| `src/btop_config.hpp`/`.cpp` | Configuration | `Config::getS()`, `Config::getI()`, `Config::getB()` |
| `src/btop_menu.hpp`/`.cpp` | Menu system | Interactive menu rendering |
| `src/btop_input.hpp`/`.cpp` | Input handling | Keyboard/mouse event processing |
| `src/btop_tools.hpp`/`.cpp` | Utilities | String formatting, time functions, etc. |
| `src/btop_log.hpp`/`.cpp` | Logging | Debug/error logging infrastructure |

---

## Appendix B: Color Hex Reference (Quick Lookup)

```
Backgrounds:
  main_bg     = #000000  (black)
  main_fg     = #cccccc  (light gray)
  inactive_fg = #404040  (dim gray)
  meter_bg    = #404040  (dim gray)

Borders:
  cpu_box     = #556d59  (muted green-gray)
  mem_box     = #6c6c4b  (olive drab)
  net_box     = #5c588d  (slate blue)
  proc_box    = #805252  (muted red-brown)
  div_line    = #303030  (very dark gray)

CPU Gradient (0% → 50% → 100%):
  cpu_start   = #77ca9b  (soft green)
  cpu_mid     = #cbc06c  (yellow-olive)
  cpu_end     = #dc4c4c  (red)

Temperature Gradient (0°C → mid → max):
  temp_start  = #4897d4  (blue)
  temp_mid    = #5474e8  (purple-blue)
  temp_end    = #ff40b6  (pink/magenta)

Memory Used Gradient:
  used_start  = #592b26  (dark red-brown)
  used_mid    = #d9626d  (rose)
  used_end    = #ff4769  (bright red)

Memory Free Gradient:
  free_start  = #384f21  (dark green)
  free_mid    = #b5e685  (light green)
  free_end    = #dcff85  (bright yellow-green)

Memory Available Gradient:
  available_start = #4e3f0e  (dark gold)
  available_mid   = #ffd77a  (light gold)
  available_end   = #ffb814  (bright gold)

Memory Cached Gradient:
  cached_start = #163350  (dark blue)
  cached_mid   = #74e6fc  (cyan)
  cached_end   = #26c5ff  (bright cyan)

Network Download Gradient:
  download_start = #291f75  (dark blue-violet)
  download_mid   = #4f43a3  (blue-violet)
  download_end   = #b0a9de  (light lavender)

Network Upload Gradient:
  upload_start   = #620665  (dark magenta)
  upload_mid     = #7d4180  (purple-magenta)
  upload_end     = #dcafde  (light magenta)

Process Gradient:
  process_start = #80d0a3  (mint green)
  process_mid   = #dcd179  (warm yellow)
  process_end   = #d45454  (red)
```

---

*Document generated from authoritative source analysis of aristocratos/btop v1.4.7 (commit main branch).*
