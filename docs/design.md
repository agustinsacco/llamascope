# The observatory design system

[← Home](../README.md) · [Field guide](guide.md) · [Implementation](../src/theme.rs)

## The idea: calm until something matters

An inference monitor should not compete with the work it observes. LlamaScope borrows the discipline of an instrument panel: a clear state at the top, a small set of summary readings, a selectable subject, then supporting evidence.

**The visual hierarchy**

```text
01  IDENTITY + TRUST      Which model? Which server? Is this live, stale, or simulated?
02  ORIENTATION          Throughput · occupied slots · known context · poll duration
03  ATTENTION            Selected request and its two independent time-series scales
04  CONTEXT              Local sensors and recent operational events
05  DISCOVERY            Keyboard hints; the rest lives behind ?
```

Blank space is useful; blank *states* are not. Every empty surface should say what is happening, whether it needs action, and what the user can do next.

## Two palettes, one vocabulary

Mocha retains the project’s Catppuccin-inspired identity. Latte uses a light ground and darker semantic accents; it is not a simple inversion. All presentation colors originate in `src/theme.rs`.

| Token | Role | Never use it for |
| --- | --- | --- |
| `base` | Canvas and footer | An implied status |
| `panel` | Card and chart ground | Invisible text |
| `raised` | Selected row and gauge track | An unlabeled alert |
| `text` | Primary values and body copy | Subtle grid lines |
| `muted` | Supporting text, axes, timestamps | Disabled/unreadable copy |
| `border` | Low-emphasis boundaries | Essential information |
| `accent` | Brand, focus, keys, host RAM | Success/failure alone |
| `good` | Live state, decoding, throughput | A guarantee of server correctness |
| `warning` | Loading, degraded, frozen, prefill | A catastrophic error |
| `error` | Offline/transport failures | Ordinary idleness |
| `secondary` | Context estimates, demo identity | Measured hardware allocation |

The contrast regression test computes sRGB relative luminance and requires **at least 4.5:1** for every text/semantic token on base, panel, and raised backgrounds. Border colors are decorative and excluded. Terminal emulators and fonts can still alter rendering; this is not a blanket accessibility certification.

## Layout contract

| Terminal / available space | Behavior |
| --- | --- |
| At least `110 × 32` | Local-sensor sidebar and primary inference panel. |
| At least 33 rows | Event stream fits in the main panel; at smaller heights it is hidden, not deleted. |
| Below the sidebar threshold | Full-width inference view; local data still collects but its panels are hidden. |
| Slot-table inner width at least 85 columns | Include generated-token and task columns. |
| Narrower table | Keep slot, phase, TOK/S, prompt, and context. |
| Fewer than `60 × 18` | Show an explicit minimum-size guide; retain state and quit controls. |

A selected row has a distinct background, bold treatment, and `›` marker. The table scrolls to keep selection visible. Selecting by ID preserves focus when the server reorders rows; selecting a removed slot falls back to the first remaining slot.

The two request charts deliberately have different y-scales. Plotting tokens/second and context percentage on one axis suggests comparisons that the numbers do not support. Context stays on a `0–100%` scale; throughput scales to its observed range with headroom.

## Interaction contract

- **Keyboard first:** useful actions always have a visible hint or appear in help.
- **No hover dependency:** the terminal has no hover-only controls or tooltips.
- **Color plus language:** `LIVE`, `OFFLINE`, `Prefill`, and `Idle` remain understandable without hue.
- **Loading is honest:** an activity indicator means work is pending; no invented percentage or ETA is shown for model loading.
- **Freeze is explicit:** the entire view is labeled frozen; collection continues in the background.
- **Errors are actionable:** explain endpoint/status and a next step, rather than dumping opaque exceptions.
- **Missing values are visible:** `--` means unavailable, including the first throughput sample.
- **No surprise persistence:** theme changes, selection, events, and history are session-local.

## Review a UI change in every state

Do not approve only a pretty demo screenshot. Render connecting, model loading, live, empty, degraded/last-known, offline, frozen, help, and missing-sensor views. Test both palettes, a compact terminal, a large terminal, and an unusably small terminal. Exercise keys in a real pseudo-terminal as well as the in-memory renderer.

The [snapshot example and test commands](contributing.md#reproducible-screenshots) make that review repeatable. Documentation screenshots come from the same `AppState::draw` used by the executable.

### Deliberate limits

Braille charts and box-drawing characters assume a capable monospaced terminal font. There is no ASCII-only rendering mode or screen-reader-specific interface yet. Do not claim accessibility beyond the tested contrast, explicit labels, keyboard controls, and resizing behavior.
