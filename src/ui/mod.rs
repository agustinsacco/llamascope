//! Responsive terminal presentation of [`AppState`]. Every cell is painted with theme
//! colors so the terminal's own background never shows through.
mod help;
mod sensors;
mod slots;

use crate::{
    app::{AppState, RollingHistory},
    telemetry::{safe_text, ConnectionState, LlamaSlot},
    theme::Palette,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Chart, Dataset, Gauge, GraphType, Padding, Paragraph, Wrap,
    },
    Frame,
};

impl AppState {
    pub fn draw(&self, frame: &mut Frame) {
        let p = self.theme.palette();
        let area = frame.area();
        frame.render_widget(
            Block::default().style(Style::default().fg(p.text).bg(p.base)),
            area,
        );
        if area.width < 60 || area.height < 18 {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("LLAMASCOPE").bold().fg(p.accent),
                    Line::from("Give your observatory a little more room."),
                    Line::from(format!(
                        "{} x {} now · minimum 60 x 18",
                        area.width, area.height
                    )),
                    Line::from("Resize to continue · q / Ctrl-C quit"),
                ])
                .block(panel(" COMPACT TERMINAL ", p))
                .wrap(Wrap { trim: true }),
                area,
            );
            return;
        }
        let [header, summary, body, footer] = area.layout(&Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(2),
        ]));
        self.draw_header(frame, header);
        self.draw_summary(frame, summary);
        if area.width >= 110 && area.height >= 32 {
            let [sensors, main] = body.layout(&Layout::horizontal([
                Constraint::Percentage(28),
                Constraint::Fill(1),
            ]));
            self.draw_hardware(frame, sensors);
            self.draw_main(frame, main);
        } else {
            self.draw_main(frame, body);
        }
        let footer_lines = vec![
            Line::from(vec![
                Span::styled(" Tab/↑↓ ", Style::default().fg(p.accent).bold()),
                Span::raw("slot  "),
                Span::styled("Space", Style::default().fg(p.accent).bold()),
                Span::raw(" freeze  "),
                Span::styled("t", Style::default().fg(p.accent).bold()),
                Span::raw(" theme  "),
                Span::styled("?", Style::default().fg(p.accent).bold()),
                Span::raw(" help  "),
                Span::styled("q", Style::default().fg(p.accent).bold()),
                Span::raw(" quit"),
            ]),
            Line::from(format!(
                " {} · {} · {}",
                self.theme.name(),
                if self.demo {
                    "SIMULATED DATA"
                } else {
                    "READ ONLY"
                },
                if self.paused_at.is_some() {
                    "View frozen; collection continues"
                } else {
                    "60s history · -- means unavailable"
                }
            ))
            .fg(p.muted),
        ];
        frame.render_widget(Paragraph::new(footer_lines).bg(p.base), footer);
        if self.help {
            self.draw_help(frame, area);
        }
    }
    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let (status, color) = self.status();
        let activity = if matches!(
            self.connection,
            ConnectionState::Connecting | ConnectionState::Loading
        ) && self.paused_at.is_none()
        {
            ["◐", "◓", "◑", "◒"][(self.started_at.elapsed().as_millis() / 150 % 4) as usize]
        } else {
            "●"
        };
        let age = self
            .last_slots_at
            .map(|at| {
                format!(
                    "last sample {:.1}s ago",
                    self.now().saturating_duration_since(at).as_secs_f64()
                )
            })
            .unwrap_or_else(|| "awaiting first sample".into());
        let model = self
            .props
            .as_ref()
            .map_or("Waiting for model metadata", |p| p.model_name.as_str());
        let lines = vec![
            Line::from(vec![
                Span::styled(" LLAMASCOPE ", Style::default().fg(p.accent).bold()),
                Span::styled(
                    if area.width >= 100 {
                        " /  INFERENCE OBSERVATORY"
                    } else {
                        " /  OBSERVATORY"
                    },
                    Style::default().fg(p.muted),
                ),
                Span::styled(
                    format!("   {activity} {status}"),
                    Style::default().fg(color).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled(format!(" {}", safe_text(model)), Style::default().bold()),
                Span::styled(
                    if area.width >= 110 {
                        format!(
                            "  ·  {}",
                            if self.demo {
                                "local simulation"
                            } else {
                                &self.endpoint
                            }
                        )
                    } else {
                        String::new()
                    },
                    Style::default().fg(p.muted),
                ),
            ]),
        ];
        let [identity, status_area] = area.layout(&Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
        ]));
        frame.render_widget(Paragraph::new(lines), identity);
        frame.render_widget(
            Paragraph::new(format!(" {age} · {}", self.detail))
                .fg(color)
                .wrap(Wrap { trim: true }),
            status_area,
        );
    }
    fn draw_summary(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let cols = Layout::horizontal([Constraint::Percentage(25); 4]).split(area);
        let active: Vec<_> = self.slots.iter().filter(|s| s.busy).collect();
        let tps = if self.last_slots_at.is_none() || active.iter().any(|s| s.tps.is_none()) {
            "--".into()
        } else {
            format!(
                "{:.1}",
                active
                    .iter()
                    .filter_map(|s| s.tps)
                    .fold(0.0, |sum, rate| sum + rate)
            )
        };
        let slots = if self.last_slots_at.is_some() {
            format!("{} / {}", active.len(), self.slots.len())
        } else {
            "-- / --".into()
        };
        let kv = self
            .slots
            .iter()
            .filter_map(LlamaSlot::context_percent)
            .reduce(f64::max);
        let items = [
            (
                " THROUGHPUT ",
                format!("{tps} t/s"),
                "observed decode",
                p.good,
            ),
            (" BUSY SLOTS ", slots, "active / total", p.accent),
            (" CONTEXT ", fmt(kv, "%"), "fullest slot", p.secondary),
            (
                " POLL CYCLE ",
                if self.connection == ConnectionState::Connecting {
                    "-- ms".into()
                } else {
                    format!("{} ms", self.latency.as_millis())
                },
                "HTTP round trip",
                p.warning,
            ),
        ];
        for (area, (title, value, label, color)) in cols.iter().zip(items) {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(value).fg(color).bold(),
                    Line::from(label).fg(p.muted),
                ])
                .block(panel(title, p).padding(Padding::horizontal(1))),
                *area,
            );
        }
    }
}

fn panel(title: &str, p: Palette) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(p.muted).bold())
        .border_style(Style::default().fg(p.border))
        .style(Style::default().fg(p.text).bg(p.panel))
}
fn fmt(value: Option<f64>, unit: &str) -> String {
    value
        .filter(|n| n.is_finite())
        .map_or_else(|| "--".into(), |n| format!("{n:.1}{unit}"))
}
/// A token count, compact above ten thousand: `812`, `4096`, `45.1k`.
fn tokens(value: Option<u64>) -> String {
    match value {
        None => "--".into(),
        Some(n) if n >= 10_000 => format!("{:.1}k", n as f64 / 1000.0),
        Some(n) => n.to_string(),
    }
}
fn draw_gauge(frame: &mut Frame, area: Rect, value: f64, color: Color, p: Palette) {
    let ratio = if value.is_finite() {
        (value / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .gauge_style(Style::default().fg(color).bg(p.raised))
            .label(format!("{:.0}%", ratio * 100.0)),
        area,
    );
}
fn draw_chart(
    frame: &mut Frame,
    area: Rect,
    history: &RollingHistory,
    title: &str,
    color: Color,
    max: Option<f64>,
    p: Palette,
) {
    if area.height < 3 || area.width < 12 {
        return;
    }
    if history.data().len() < 2 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(title).fg(color).bold(),
                Line::from(if history.data().is_empty() {
                    " Awaiting reported metrics"
                } else {
                    " Collecting history..."
                })
                .fg(p.muted),
            ])
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let top = max.unwrap_or_else(|| {
        (history.data().iter().map(|p| p.1).fold(0.0, f64::max) * 1.2).max(10.0)
    });
    let data = Dataset::default()
        .marker(Marker::Braille)
        .style(Style::default().fg(color))
        .graph_type(GraphType::Line)
        .data(history.data());
    let chart = Chart::new(vec![data])
        .style(Style::default().bg(p.panel))
        .block(Block::default().title(Line::from(title).fg(color).bold()))
        .x_axis(
            Axis::default()
                .bounds(history.bounds())
                .labels([Line::from("−60s"), Line::from("now")])
                .style(Style::default().fg(p.muted)),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, top])
                .labels([Line::from("0"), Line::from(format!("{top:.0}"))])
                .style(Style::default().fg(p.muted)),
        );
    frame.render_widget(chart, area);
}
