//! Dashboard state: telemetry merging, bounded history/events, freeze, and key actions.
//! Presentation lives in [`crate::ui`].
use crate::{
    config::Config,
    hardware::HardwareTelemetry,
    telemetry::{safe_text, ConnectionState, LlamaProps, LlamaSlot, TelemetryData},
    theme::{Palette, Theme},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::Color;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

/// At most 241 samples and 60 seconds; x is elapsed wall time, not a sample index.
#[derive(Default)]
pub struct RollingHistory {
    data: Vec<(f64, f64)>,
}
impl RollingHistory {
    pub fn push(&mut self, seconds: f64, value: f64) {
        let backwards = self.data.last().is_some_and(|last| seconds < last.0);
        if !seconds.is_finite() || !value.is_finite() || value < 0.0 || backwards {
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
    pub context: RollingHistory,
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
    /// Latest samples received while frozen; applied on resume.
    pending: Option<TelemetryData>,
    pending_hardware: Option<(HardwareTelemetry, Instant)>,
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
            endpoint: Config::default().api_url,
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
            pending: None,
            pending_hardware: None,
        }
    }
    /// Startup state for a validated configuration. Staleness is three poll intervals,
    /// with a three-second floor.
    pub fn from_config(config: &Config) -> Self {
        Self {
            theme: config.theme,
            endpoint: config.api_url.clone(),
            demo: config.demo,
            stale_after: (config.interval * 3).max(Duration::from_secs(3)),
            ..Self::new()
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
    /// Accept a server sample: applied now, or held (latest only) while the view is frozen.
    pub fn receive(&mut self, data: TelemetryData) {
        if self.paused_at.is_some() {
            self.pending = Some(data);
        } else {
            self.update(data);
        }
    }
    /// Accept a local sensor sample: applied now, or held (latest only) while frozen.
    pub fn receive_hardware(&mut self, hardware: HardwareTelemetry, at: Instant) {
        if self.paused_at.is_some() {
            self.pending_hardware = Some((hardware, at));
        } else {
            self.update_hardware(hardware, at);
        }
    }
    /// The local sensor collector ended; show that instead of its last readings.
    pub fn sensors_stopped(&mut self, reason: &str) {
        let note = format!("Local sensors stopped ({reason}). Server monitoring continues.");
        self.log(true, &note);
        self.hardware = Some(HardwareTelemetry {
            note,
            ..Default::default()
        });
        self.pending_hardware = None;
    }
    pub fn update(&mut self, data: TelemetryData) {
        let detail = safe_text(&data.detail);
        if self.connection != data.connection || self.detail != detail {
            self.log(
                matches!(
                    data.connection,
                    ConnectionState::Degraded | ConnectionState::Offline
                ),
                &detail,
            );
        }
        self.connection = data.connection;
        self.detail = detail;
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
                if let Some(context) = slot.context_percent() {
                    history.context.push(elapsed, context);
                }
            }
            let ids: HashSet<_> = slots.iter().map(|s| s.id).collect();
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
    fn toggle_freeze(&mut self) {
        if self.paused_at.take().is_some() {
            if let Some(data) = self.pending.take() {
                self.update(data);
            }
            if let Some((hardware, at)) = self.pending_hardware.take() {
                self.update_hardware(hardware, at);
            }
            self.log(false, "Live view resumed.");
        } else {
            self.paused_at = Some(Instant::now());
            self.log(
                false,
                "View frozen. Collection continues; Space resumes with the latest sample.",
            );
        }
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
            KeyCode::Char(' ') => self.toggle_freeze(),
            KeyCode::Char('c') => {
                self.logs.clear();
                self.log(
                    false,
                    "Events cleared. New connection changes will appear here.",
                );
            }
            KeyCode::Char('r') if self.demo => {
                self.log(false, "Demo mode has no server to retry.");
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
    /// The instant the view represents: the freeze time while frozen.
    pub(crate) fn now(&self) -> Instant {
        self.paused_at.unwrap_or_else(Instant::now)
    }
    /// Displayed slots are last-known rather than fresh.
    pub fn stale(&self) -> bool {
        self.last_slots_at.is_some_and(|at| {
            !self.last_attempt_had_slots
                || self.now().saturating_duration_since(at) > self.stale_after
        })
    }
    /// The header badge: freeze and demo take precedence over the connection state.
    pub fn status(&self) -> (&'static str, Color) {
        let p: Palette = self.theme.palette();
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
}
