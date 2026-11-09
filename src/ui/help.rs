//! The `?` field guide overlay.
use super::panel;
use crate::app::AppState;
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::Line,
    widgets::{Clear, Padding, Paragraph, Wrap},
    Frame,
};

impl AppState {
    pub(super) fn draw_help(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let width = area.width.saturating_sub(4).min(76);
        let height = area.height.saturating_sub(2).min(23);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        // Most important first: a small terminal truncates from the bottom.
        let keys: &[&str] = if width >= 72 {
            &[
                "Tab / ↓ / j     Next slot       Shift-Tab / ↑ / k   Previous",
                "Space           Freeze view    t                   Theme",
                "r               Retry poll     c                   Clear events",
                "q / Ctrl-C      Quit safely    ? / Esc             Close help",
            ]
        } else {
            &[
                "Tab ↓ j next slot · Shift-Tab ↑ k previous",
                "Space freeze · t theme · r retry · c clear",
                "q / Ctrl-C quit · ? / Esc close help",
            ]
        };
        let mut lines = vec![Line::from("YOUR INFERENCE FIELD GUIDE").bold().fg(p.accent)];
        lines.extend(keys.iter().map(|k| Line::from(*k)));
        lines.extend([
            Line::from(format!(
                "SERVER  {}",
                if self.demo {
                    "simulation; no requests"
                } else {
                    &self.endpoint
                }
            ))
            .fg(p.muted),
            Line::from(format!(
                "BUILD   {}",
                self.props
                    .as_ref()
                    .and_then(|p| p.build_info.as_deref())
                    .unwrap_or("--")
            ))
            .fg(p.muted),
        ]);
        if height >= 20 {
            lines.extend([
                Line::from(""),
                Line::from("READ THE SIGNAL").bold().fg(p.secondary),
                Line::from("Prefill (no token yet) → Decoding → Idle. Labels never rely on color."),
                Line::from("PROMPT counts ingested prompt tokens, including reused cache."),
                Line::from("CONTEXT is tokens held over slot size; -- means missing, not zero."),
                Line::from("TOK/S is a sampled token delta for the same task, not a benchmark."),
                Line::from("Charts have independent scales and a 60-second time window."),
                Line::from(
                    "Freeze holds the view, not collection. Resume takes the latest sample.",
                ),
                Line::from("Local sensors describe THIS machine, even with a remote --url."),
                Line::from(""),
                Line::from("docs/guide.md · docs/architecture.md").fg(p.muted),
            ]);
        }
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().fg(p.text).bg(p.panel))
                .block(
                    panel(" HELP / ? OR ESC TO CLOSE ", p)
                        .border_style(Style::default().fg(p.accent))
                        .padding(Padding::horizontal(1)),
                )
                .wrap(Wrap { trim: true }),
            popup,
        );
    }
}
