//! The main column: slot table, selected-slot charts, and the event stream.
use super::{draw_chart, fmt, panel, tokens};
use crate::{
    app::AppState,
    telemetry::{safe_text, ConnectionState, Phase},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Cell, List, ListItem, Padding, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

impl AppState {
    pub(super) fn draw_main(&self, frame: &mut Frame, area: Rect) {
        let logs_height = if area.height >= 23 { 6 } else { 0 };
        let table_height = (self.slots.len().min(7) as u16 + 3)
            .clamp(6, 10)
            .min(area.height);
        let [table, graph, logs] = area.layout(&Layout::vertical([
            Constraint::Length(table_height),
            Constraint::Min(0),
            Constraint::Length(logs_height),
        ]));
        self.draw_slots(frame, table);
        self.draw_graph(frame, graph);
        if logs_height > 0 {
            self.draw_logs(frame, logs);
        }
    }
    fn draw_slots(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let block = panel(
            if self.stale() && !self.slots.is_empty() {
                " INFERENCE SLOTS / LAST KNOWN "
            } else {
                " INFERENCE SLOTS "
            },
            p,
        )
        .title_bottom(
            Line::from(" Tab / arrows select · selected row follows the charts ").fg(p.muted),
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if self.slots.is_empty() {
            let (title, hint) = match self.connection {
                ConnectionState::Connecting => ("Establishing a connection", "Checking /health, /slots, and /props. Your dashboard will appear automatically."),
                ConnectionState::Loading => ("The model is warming up", "llama-server is loading weights. Keep this view open; monitoring resumes automatically."),
                ConnectionState::Live => ("Connected. No slots reported.", "Check llama-server parallel-slot configuration. No inference requests are sent by LlamaScope."),
                _ => ("Telemetry needs attention", self.detail.as_str()),
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(title).bold().fg(p.accent),
                    Line::from(hint).fg(p.muted),
                ])
                .wrap(Wrap { trim: true })
                .block(Block::default().padding(Padding::horizontal(1))),
                inner,
            );
            return;
        }
        let wide = inner.width >= 85;
        let rows: Vec<_> = self
            .slots
            .iter()
            .map(|s| {
                let phase = s.phase();
                let phase_color = match phase {
                    Phase::Idle => p.muted,
                    Phase::Prefill => p.warning,
                    Phase::Decoding | Phase::Working => p.good,
                };
                let mut cells = vec![
                    Cell::from(format!("{:02}", s.id)),
                    Cell::from(phase.label()).fg(phase_color),
                    Cell::from(fmt(s.tps.filter(|_| s.busy), "")),
                    Cell::from(tokens(s.prompt_tokens().filter(|_| s.busy))),
                    Cell::from(fmt(s.context_percent(), "%")),
                ];
                if wide {
                    let generated = s
                        .n_decoded
                        .filter(|_| s.busy)
                        .map(|n| match s.n_remain {
                            Some(r) if r >= 0 => format!("{n}/{}", n.saturating_add(r as u64)),
                            Some(_) => format!("{n}/∞"),
                            None => n.to_string(),
                        })
                        .unwrap_or_else(|| "--".into());
                    cells.push(Cell::from(generated));
                    cells.push(Cell::from(
                        s.task_id
                            .as_deref()
                            .map(safe_text)
                            .unwrap_or_else(|| "--".into()),
                    ));
                }
                Row::new(cells)
            })
            .collect();
        let mut widths = vec![
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
        ];
        let mut headers = vec!["SLOT", "PHASE", "TOK/S", "PROMPT", "CONTEXT"];
        if wide {
            widths.extend([Constraint::Length(13), Constraint::Fill(1)]);
            headers.extend(["GENERATED", "TASK"]);
        }
        let selected = self
            .slots
            .iter()
            .position(|s| Some(s.id) == self.selected_slot);
        let mut state = TableState::default().with_selected(selected).with_offset(
            selected
                .unwrap_or(0)
                .saturating_sub(inner.height.saturating_sub(2) as usize / 2),
        );
        frame.render_stateful_widget(
            Table::new(rows, widths)
                .header(Row::new(headers).fg(p.muted).bold())
                .column_spacing(1)
                .row_highlight_style(Style::default().bg(p.raised).bold())
                .highlight_symbol("› "),
            inner,
            &mut state,
        );
    }
    fn draw_graph(&self, frame: &mut Frame, area: Rect) {
        if area.height < 3 {
            return;
        }
        let p = self.theme.palette();
        let title = self.selected().map_or_else(
            || " SLOT EXPLORER ".into(),
            |s| format!(" SLOT {:02} / {} ", s.id, s.phase().label().to_uppercase()),
        );
        let block = panel(&title, p).border_style(Style::default().fg(p.accent));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let Some(slot) = self.selected() else {
            frame.render_widget(Paragraph::new("A request’s rhythm, not just a number.\nSlot throughput and estimated context will appear here.").fg(p.muted).wrap(Wrap { trim: true }).block(Block::default().padding(Padding::horizontal(1))), inner);
            return;
        };
        if let Some(history) = self.histories.get(&slot.id) {
            let parts = Layout::horizontal([
                Constraint::Fill(11),
                Constraint::Length(2),
                Constraint::Fill(9),
            ])
            .split(inner);
            draw_chart(
                frame,
                parts[0],
                &history.tps,
                " DECODE · tok/s ",
                p.good,
                None,
                p,
            );
            draw_chart(
                frame,
                parts[2],
                &history.context,
                " CONTEXT · % ",
                p.secondary,
                Some(100.0),
                p,
            );
        }
    }
    fn draw_logs(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let block = panel(" EVENT STREAM ", p)
            .title_bottom(Line::from(" c clear · r retry · newest events below ").fg(p.muted));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let items: Vec<_> = self
            .logs
            .iter()
            .skip(self.logs.len().saturating_sub(inner.height as usize))
            .map(|log| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", log.timestamp), Style::default().fg(p.muted)),
                    Span::styled(
                        if log.warning { "WARN  " } else { "INFO  " },
                        Style::default().fg(if log.warning { p.warning } else { p.accent }),
                    ),
                    Span::raw(&log.message),
                ]))
            })
            .collect();
        frame.render_widget(List::new(items), inner);
    }
}
