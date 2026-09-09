//! Stateful dashboard model and responsive terminal presentation.
use crate::{
    hardware::HardwareTelemetry,
    telemetry::{safe_text, ConnectionState, LlamaProps, LlamaSlot, TelemetryData},
    theme::{Palette, Theme},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Cell, Chart, Clear, Dataset, Gauge, GraphType, List, ListItem,
        Padding, Paragraph, Row, Table, TableState, Wrap,
    },
    Frame,
};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

/// At most 241 samples and 60 seconds; x is elapsed wall time, not a sample index.
#[derive(Default)]
pub struct RollingHistory {
    data: Vec<(f64, f64)>,
}
impl RollingHistory {
    pub fn push(&mut self, seconds: f64, value: f64) {
        if !seconds.is_finite() || !value.is_finite() || value < 0.0 {
            return;
        }
        self.data.retain(|(x, _)| *x >= seconds - 60.0);
        if self.data.len() >= 241 {
            self.data.remove(0);
        }
        self.data.push((seconds, value));
    }
    pub fn data(&self) -> &[(f64, f64)] {
        &self.data
    }
    pub fn bounds(&self) -> [f64; 2] {
        let end = self.data.last().map_or(60.0, |p| p.0);
        [end - 60.0, end]
    }
}
#[derive(Default)]
pub struct SlotHistory {
    pub tps: RollingHistory,
    pub kv: RollingHistory,
    task: Option<String>,
}
#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub warning: bool,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Retry,
}

pub struct AppState {
    pub started_at: Instant,
    pub theme: Theme,
    pub endpoint: String,
    pub demo: bool,
    pub paused_at: Option<Instant>,
    pub help: bool,
    pub connection: ConnectionState,
    pub detail: String,
    pub slots: Vec<LlamaSlot>,
    pub props: Option<LlamaProps>,
    pub selected_slot: Option<u32>,
    pub last_slots_at: Option<Instant>,
    pub stale_after: Duration,
    last_attempt_had_slots: bool,
    pub latency: Duration,
    pub hardware: Option<HardwareTelemetry>,
    pub hardware_at: Option<Instant>,
    pub histories: HashMap<u32, SlotHistory>,
    pub ram_history: RollingHistory,
    pub gpu_history: RollingHistory,
    pub logs: VecDeque<LogEntry>,
}
impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            theme: Theme::default(),
            endpoint: "http://127.0.0.1:8086".into(),
            demo: false,
            paused_at: None,
            help: false,
            connection: ConnectionState::Connecting,
            detail: "Opening a read-only connection. Waiting for the first sample...".into(),
            slots: Vec::new(),
            props: None,
            selected_slot: None,
            last_slots_at: None,
            stale_after: Duration::from_secs(3),
            last_attempt_had_slots: false,
            latency: Duration::ZERO,
            hardware: None,
            hardware_at: None,
            histories: HashMap::new(),
            ram_history: RollingHistory::default(),
            gpu_history: RollingHistory::default(),
            logs: VecDeque::new(),
        }
    }
    pub fn log(&mut self, warning: bool, message: impl Into<String>) {
        if self.logs.len() >= 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            warning,
            message: safe_text(&message.into()),
        });
    }
    pub fn update(&mut self, data: TelemetryData) {
        if self.connection != data.connection || self.detail != data.detail {
            self.log(
                matches!(
                    data.connection,
                    ConnectionState::Degraded | ConnectionState::Offline
                ),
                &data.detail,
            );
        }
        self.connection = data.connection;
        self.detail = safe_text(&data.detail);
        self.latency = data.latency;
        self.last_attempt_had_slots = data.llama_slots.is_some();
        if let Some(props) = data.llama_props {
            self.props = Some(props);
        }
        if let Some(mut slots) = data.llama_slots {
            slots.sort_by_key(|s| s.id);
            let elapsed = data
                .sampled_at
                .saturating_duration_since(self.started_at)
                .as_secs_f64();
            for slot in &slots {
                let history = self.histories.entry(slot.id).or_default();
                if history.task != slot.task_id {
                    *history = SlotHistory {
                        task: slot.task_id.clone(),
                        ..SlotHistory::default()
                    };
                }
                if let Some(tps) = slot.tps {
                    history.tps.push(elapsed, tps);
                }
                if let Some(kv) = slot.kv_percent() {
                    history.kv.push(elapsed, kv);
                }
            }
            let ids: std::collections::HashSet<_> = slots.iter().map(|s| s.id).collect();
            self.histories.retain(|id, _| ids.contains(id));
            if !slots.iter().any(|s| Some(s.id) == self.selected_slot) {
                self.selected_slot = slots.first().map(|s| s.id);
            }
            self.slots = slots;
            self.last_slots_at = Some(data.sampled_at);
        }
    }
    pub fn update_hardware(&mut self, hardware: HardwareTelemetry, at: Instant) {
        let elapsed = at.saturating_duration_since(self.started_at).as_secs_f64();
        if let Some(memory) = &hardware.memory {
            self.ram_history.push(elapsed, memory.percent());
        }
        if let Some(load) = hardware.gpu.as_ref().and_then(|g| g.usage) {
            self.gpu_history.push(elapsed, load);
        }
        self.hardware = Some(hardware);
        self.hardware_at = Some(at);
    }
    pub fn select(&mut self, forward: bool) {
        if self.slots.is_empty() {
            return;
        }
        let index = self
            .slots
            .iter()
            .position(|s| Some(s.id) == self.selected_slot)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % self.slots.len()
        } else {
            (index + self.slots.len() - 1) % self.slots.len()
        };
        self.selected_slot = Some(self.slots[next].id);
    }
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None;
        }
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Quit
            }
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Esc => self.help = false,
            KeyCode::Char('t') => self.theme.toggle(),
            KeyCode::Char(' ') => {
                self.paused_at = if self.paused_at.is_some() {
                    None
                } else {
                    Some(Instant::now())
                };
                self.log(
                    false,
                    if self.paused_at.is_some() {
                        "View frozen. Collection continues; Space resumes with the latest sample."
                    } else {
                        "Live view resumed."
                    },
                );
            }
            KeyCode::Char('c') => {
                self.logs.clear();
                self.log(
                    false,
                    "Events cleared. New connection changes will appear here.",
                );
            }
            KeyCode::Char('r') => {
                self.log(
                    false,
                    "Retry requested. An in-flight request completes first.",
                );
                return Action::Retry;
            }
            KeyCode::Tab | KeyCode::Down | KeyCode::Right | KeyCode::Char('j') => self.select(true),
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left | KeyCode::Char('k') => {
                self.select(false)
            }
            _ => {}
        }
        Action::None
    }
    pub fn selected(&self) -> Option<&LlamaSlot> {
        self.slots.iter().find(|s| Some(s.id) == self.selected_slot)
    }
    fn now(&self) -> Instant {
        self.paused_at.unwrap_or_else(Instant::now)
    }
    fn stale(&self) -> bool {
        self.last_slots_at.is_some_and(|at| {
            !self.last_attempt_had_slots
                || self.now().saturating_duration_since(at) > self.stale_after
        })
    }
    fn status(&self) -> (&'static str, Color) {
        let p = self.theme.palette();
        if self.paused_at.is_some() {
            return ("FROZEN", p.warning);
        }
        if self.demo {
            return ("DEMO", p.secondary);
        }
        match self.connection {
            ConnectionState::Connecting => ("CONNECTING", p.accent),
            ConnectionState::Loading => ("MODEL LOADING", p.warning),
            ConnectionState::Live if self.stale() => ("STALE", p.warning),
            ConnectionState::Live => ("LIVE", p.good),
            ConnectionState::Degraded => ("DEGRADED", p.warning),
            ConnectionState::Offline => ("OFFLINE", p.error),
        }
    }
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
            .filter_map(LlamaSlot::kv_percent)
            .reduce(f64::max);
        let items = [
            (
                " THROUGHPUT ",
                format!("{tps} t/s"),
                "observed decode",
                p.good,
            ),
            (" BUSY SLOTS ", slots, "active / total", p.accent),
            (" CONTEXT EST. ", fmt(kv, "%"), "highest known", p.secondary),
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
    fn draw_main(&self, frame: &mut Frame, area: Rect) {
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
                let phase_color = match s.phase() {
                    "Idle" => p.muted,
                    "Prefill" => p.warning,
                    _ => p.good,
                };
                let mut cells = vec![
                    Cell::from(format!("{:02}", s.id)),
                    Cell::from(s.phase()).fg(phase_color),
                    Cell::from(fmt(s.tps.filter(|_| s.busy), "")),
                    Cell::from(fmt(s.prompt_percent().filter(|_| s.busy), "%")),
                    Cell::from(fmt(s.kv_percent(), "%")),
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
        let mut headers = vec!["SLOT", "PHASE", "TOK/S", "PREFILL", "CTX EST."];
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
            |s| format!(" SLOT {:02} / {} ", s.id, s.phase().to_uppercase()),
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
                &history.kv,
                " CONTEXT EST. · % ",
                p.secondary,
                Some(100.0),
                p,
            );
        }
    }
    fn draw_hardware(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let [memory_area, gpu_area] = area.layout(&Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]));
        let mem_block = panel(" LOCAL HOST / RAM ", p);
        let mem_inner = mem_block.inner(memory_area);
        frame.render_widget(mem_block, memory_area);
        let gpu_block = panel(" LOCAL GPU / AMD ", p);
        let gpu_inner = gpu_block.inner(gpu_area);
        frame.render_widget(gpu_block, gpu_area);
        let Some(hardware) = &self.hardware else {
            for inner in [mem_inner, gpu_inner] {
                frame.render_widget(Paragraph::new(" Discovering local sensors...\n Optional; server monitoring is independent.").fg(p.muted).wrap(Wrap { trim: true }), inner);
            }
            return;
        };
        if let Some(memory) = &hardware.memory {
            let parts = Layout::vertical([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(mem_inner);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!(
                        " {:.1} / {:.1} GiB",
                        memory.used_gib, memory.total_gib
                    ))
                    .bold(),
                    Line::from(" Local machine, not remote server").fg(p.muted),
                ]),
                parts[0],
            );
            draw_gauge(frame, parts[1], memory.percent(), p.accent, p);
            draw_chart(
                frame,
                parts[2],
                &self.ram_history,
                " RAM · % ",
                p.accent,
                Some(100.0),
                p,
            );
            let queue = self
                .props
                .as_ref()
                .and_then(|p| p.queue_size)
                .map_or_else(|| "--".into(), |n| n.to_string());
            frame.render_widget(
                Paragraph::new(format!(" Server queue: {queue} (if reported)")).fg(p.muted),
                parts[3],
            );
        } else {
            frame.render_widget(Paragraph::new(" Host memory unavailable.\n Supported on Linux and macOS.\n Server telemetry is unaffected.").fg(p.muted).wrap(Wrap { trim: true }), mem_inner);
        }
        if let Some(gpu) = &hardware.gpu {
            let parts = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(gpu_inner);
            frame.render_widget(
                Paragraph::new(format!(" Compute {}", fmt(gpu.usage, "%"))).bold(),
                parts[0],
            );
            if let Some(usage) = gpu.usage {
                draw_gauge(frame, parts[1], usage, p.good, p);
            }
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!(
                        " {}   {}",
                        fmt(gpu.temp, "°C"),
                        fmt(gpu.power, "W")
                    ))
                    .fg(p.warning),
                    Line::from(gpu.vram.as_ref().map_or_else(
                        || " VRAM --".into(),
                        |m| format!(" VRAM {:.1}/{:.1} GiB", m.used_gib, m.total_gib),
                    ))
                    .fg(p.muted),
                ]),
                parts[2],
            );
            draw_chart(
                frame,
                parts[3],
                &self.gpu_history,
                " COMPUTE · % ",
                p.good,
                Some(100.0),
                p,
            );
            frame.render_widget(
                Paragraph::new(if self.demo {
                    " Simulated hardware\n No probes are running"
                } else {
                    " Local card0 only\n Not remote-host telemetry"
                })
                .fg(p.muted),
                parts[4],
            );
        } else {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(" GPU telemetry is optional").fg(p.accent).bold(),
                    Line::from(""),
                    Line::from(format!(" {}", hardware.note)).fg(p.muted),
                    Line::from(""),
                    Line::from(" CPU-only inference? You’re ready.").fg(p.good),
                ])
                .wrap(Wrap { trim: true }),
                gpu_inner,
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
    fn draw_help(&self, frame: &mut Frame, area: Rect) {
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
        let lines = vec![
            Line::from("YOUR INFERENCE FIELD GUIDE").bold().fg(p.accent),
            Line::from(""),
            Line::from("Tab / ↓ / j     Next slot       Shift-Tab / ↑ / k   Previous"),
            Line::from("Space           Freeze view    t                   Theme"),
            Line::from("r               Retry poll     c                   Clear events"),
            Line::from("q / Ctrl-C      Quit safely    ? / Esc             Close help"),
            Line::from(""),
            Line::from("READ THE SIGNAL").bold().fg(p.secondary),
            Line::from("Prefill → Working / Decoding → Idle. Labels never rely on color."),
            Line::from("TOK/S is a sampled token delta for the same task, not a benchmark."),
            Line::from("Context is a token-count estimate; -- means missing data, not zero."),
            Line::from("Charts have independent scales and a 60-second time window."),
            Line::from("Freeze holds the view, not collection. Resume takes the latest sample."),
            Line::from("Local sensors describe THIS machine, even with a remote --url."),
            Line::from(""),
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
            Line::from("docs/guide.md · docs/architecture.md").fg(p.muted),
        ];
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
