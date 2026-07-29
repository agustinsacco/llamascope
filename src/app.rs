use std::collections::HashMap;
use ratatui::layout::{Constraint, Layout, Rect, Alignment};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::Frame;
use ratatui::widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType, List, ListItem, Paragraph, Row, Table, Gauge};
use crate::telemetry::{TelemetryData, LogLevel};

pub struct RollingHistory {
    data: Vec<(f64, f64)>,
    max_len: usize,
    counter: f64,
}

impl RollingHistory {
    pub fn new(max_len: usize) -> Self {
        Self {
            data: Vec::new(),
            max_len,
            counter: 0.0,
        }
    }

    pub fn push(&mut self, val: f64) {
        self.data.push((self.counter, val));
        self.counter += 1.0;
        if self.data.len() > self.max_len {
            self.data.remove(0);
        }
    }

    pub fn get_data(&self) -> &[(f64, f64)] {
        &self.data
    }

    pub fn x_bounds(&self) -> [f64; 2] {
        if self.data.is_empty() {
            [0.0, self.max_len as f64]
        } else {
            let first = self.data.first().unwrap().0;
            let last = self.data.last().unwrap().0;
            [first, first.max(last).max(first + self.max_len as f64 - 1.0)]
        }
    }
}

pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

pub struct AppState {
    pub ram_history: RollingHistory,
    pub vram_history: RollingHistory,
    pub gpu_load_history: RollingHistory,
    pub gpu_temp_history: RollingHistory,
    pub gpu_power_history: RollingHistory,
    
    // slot_id -> (tps_history, kv_history)
    pub slot_histories: HashMap<u32, (RollingHistory, RollingHistory)>,
    
    pub logs: Vec<LogEntry>,
    pub current_data: Option<TelemetryData>,
    pub selected_slot_index: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            ram_history: RollingHistory::new(120),
            vram_history: RollingHistory::new(120),
            gpu_load_history: RollingHistory::new(120),
            gpu_temp_history: RollingHistory::new(120),
            gpu_power_history: RollingHistory::new(120),
            slot_histories: HashMap::new(),
            logs: Vec::new(),
            current_data: None,
            selected_slot_index: 0,
        }
    }

    pub fn log_info(&mut self, msg: String) {
        let time_str = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogEntry {
            timestamp: time_str,
            level: LogLevel::Info,
            message: msg,
        });
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    pub fn log_warn(&mut self, msg: String) {
        let time_str = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogEntry {
            timestamp: time_str,
            level: LogLevel::Warn,
            message: msg,
        });
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    pub fn update(&mut self, data: TelemetryData) {
        // Detect server connection transitions
        if data.llama_slots.is_none() {
            if self.current_data.as_ref().and_then(|d| d.llama_slots.as_ref()).is_some() {
                self.log_warn("Llama server disconnected / unreachable.".to_string());
            }
        } else {
            if self.current_data.is_none() || self.current_data.as_ref().and_then(|d| d.llama_slots.as_ref()).is_none() {
                self.log_info("Successfully connected to llama-server.".to_string());
            }
        }

        // Update current raw data
        self.current_data = Some(data.clone());

        // Update system telemetry history
        if let Some(ref gpu) = data.gpu {
            self.ram_history.push(gpu.mem_used);
            self.vram_history.push(gpu.vram_used);
            self.gpu_load_history.push(gpu.usage);
            self.gpu_temp_history.push(gpu.temp);
            self.gpu_power_history.push(gpu.power);
        }

        // Update slot histories
        let mut existing_ids = std::collections::HashSet::new();
        if let Some(ref slots) = data.llama_slots {
            for s in slots {
                existing_ids.insert(s.id);
                let hists = self.slot_histories.entry(s.id)
                    .or_insert_with(|| (RollingHistory::new(120), RollingHistory::new(120)));
                hists.0.push(s.tps);
                let kv_pct = if s.n_ctx > 0 {
                    ((s.n_prompt_tokens_processed + s.n_decoded) as f64 / s.n_ctx as f64) * 100.0
                } else {
                    0.0
                };
                hists.1.push(kv_pct);
            }
        }

        // Retain histories for slots that still exist on the server
        self.slot_histories.retain(|id, _| existing_ids.contains(id));

        // Adjust selected slot index if out of range of existing slots
        let total_slots_count = data.llama_slots.as_ref().map(|s| s.len()).unwrap_or(0);
        if total_slots_count > 0 && self.selected_slot_index >= total_slots_count {
            self.selected_slot_index = total_slots_count - 1;
        }
    }

    pub fn cycle_selected_slot(&mut self) {
        if let Some(ref data) = self.current_data {
            if let Some(ref slots) = data.llama_slots {
                if !slots.is_empty() {
                    self.selected_slot_index = (self.selected_slot_index + 1) % slots.len();
                }
            }
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let size = frame.area();

        // 1. Core vertical split: Top Bar, Content Body, Footer
        let layout_vertical = Layout::vertical([
            Constraint::Length(3), // Top bar
            Constraint::Fill(1),   // Main content body
            Constraint::Length(1), // Help Footer
        ]);
        let [top_area, body_area, footer_area] = size.layout(&layout_vertical);

        // 2. Draw Top Bar (Model Info Summary Bar)
        self.draw_top_bar(frame, top_area);

        // 3. Draw Footer
        let footer_text = Line::from(vec![
            "[q]".bold().cyan(), " Exit  ".into(),
            "[c]".bold().cyan(), " Clear Logs  ".into(),
            "[Tab]".bold().cyan(), " Cycle Active Slot Graph ".into(),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(footer_text).bg(Color::Rgb(17, 17, 27)), footer_area);

        // 4. Split body horizontally: Sidebar (35%) vs Main Panel (65%)
        let layout_horizontal = Layout::horizontal([
            Constraint::Percentage(35),
            Constraint::Percentage(65),
        ]);
        let [sidebar_area, main_area] = body_area.layout(&layout_horizontal);

        // 5. Render Sidebar (Memory & GPU)
        self.draw_sidebar(frame, sidebar_area);

        // 6. Render Main Content (Slots Table, Selected Slot Detail Graph, Log panel)
        self.draw_main_panel(frame, main_area);
    }

    fn draw_top_bar(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(137, 180, 250)))
            .title_top(" ⚡ LLAMASCOPE INFERENCE DASHBOARD ")
            .title_alignment(Alignment::Center)
            .bg(Color::Rgb(24, 24, 37));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let cols = Layout::horizontal([
            Constraint::Percentage(40), // Model Name
            Constraint::Percentage(15), // Status
            Constraint::Percentage(15), // Active Slots
            Constraint::Percentage(15), // Queue size
            Constraint::Percentage(15), // Llama build info
        ]).split(inner_area);

        let model_text = if let Some(ref data) = self.current_data {
            if let Some(ref props) = data.llama_props {
                format!("MODEL: {}", props.model_name)
            } else {
                "MODEL: [Offline]".to_string()
            }
        } else {
            "MODEL: [Offline]".to_string()
        };

        let status_span = if let Some(ref data) = self.current_data {
            if data.llama_props.is_some() {
                Span::styled("● ONLINE", Style::default().fg(Color::Rgb(166, 227, 161)).bold())
            } else {
                Span::styled("○ OFFLINE", Style::default().fg(Color::Rgb(243, 139, 168)).bold())
            }
        } else {
            Span::styled("○ OFFLINE", Style::default().fg(Color::Rgb(243, 139, 168)).bold())
        };

        let active_slots = if let Some(ref data) = self.current_data {
            if let Some(ref slots) = data.llama_slots {
                let active = slots.iter().filter(|s| s.busy).count();
                format!("SLOTS: {}/{}", active, slots.len())
            } else {
                "SLOTS: N/A".to_string()
            }
        } else {
            "SLOTS: N/A".to_string()
        };

        let queue_size = if let Some(ref data) = self.current_data {
            if let Some(ref props) = data.llama_props {
                format!("QUEUE: {}", props.queue_size)
            } else {
                "QUEUE: N/A".to_string()
            }
        } else {
            "QUEUE: N/A".to_string()
        };

        let build_version = if let Some(ref data) = self.current_data {
            if let Some(ref props) = data.llama_props {
                format!("BUILD: {}", props.build_info)
            } else {
                "BUILD: N/A".to_string()
            }
        } else {
            "BUILD: N/A".to_string()
        };

        frame.render_widget(Paragraph::new(model_text).bold(), cols[0]);
        frame.render_widget(Paragraph::new(Line::from(vec![status_span])), cols[1]);
        frame.render_widget(Paragraph::new(active_slots).bold().fg(Color::Rgb(203, 166, 247)), cols[2]);
        frame.render_widget(Paragraph::new(queue_size).bold().fg(Color::Rgb(250, 179, 135)), cols[3]);
        frame.render_widget(Paragraph::new(build_version).bold().fg(Color::Rgb(245, 194, 231)), cols[4]);
    }

    fn draw_sidebar(&self, frame: &mut Frame, area: Rect) {
        let splits = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).split(area);

        // Sidebar 1: Memory block
        let mem_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .title(" MEMORY STATUS ")
            .bg(Color::Rgb(24, 24, 37));
        let mem_inner = mem_block.inner(splits[0]);
        frame.render_widget(mem_block, splits[0]);

        // Sidebar 2: GPU Performance block
        let gpu_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .title(" GPU PERFORMANCE ")
            .bg(Color::Rgb(24, 24, 37));
        let gpu_inner = gpu_block.inner(splits[1]);
        frame.render_widget(gpu_block, splits[1]);

        if let Some(ref data) = self.current_data {
            if let Some(ref gpu) = data.gpu {
                // Render RAM/VRAM details & gauges
                let mem_layout = Layout::vertical([
                    Constraint::Length(1), // RAM title
                    Constraint::Length(1), // RAM gauge
                    Constraint::Length(1), // VRAM title
                    Constraint::Length(1), // VRAM gauge
                    Constraint::Fill(1),   // Graph
                ]).split(mem_inner);

                let ram_pct = if gpu.mem_total > 0.0 { (gpu.mem_used / gpu.mem_total * 100.0) as u16 } else { 0 };
                let vram_pct = if gpu.vram_total > 0.0 { (gpu.vram_used / gpu.vram_total * 100.0) as u16 } else { 0 };

                frame.render_widget(Paragraph::new(format!("System RAM: {:.1} / {:.1} GB", gpu.mem_used, gpu.mem_total)).bold(), mem_layout[0]);
                frame.render_widget(
                    Gauge::default()
                        .percent(ram_pct)
                        .gauge_style(Style::default().fg(Color::Rgb(245, 194, 231)).bg(Color::Rgb(49, 50, 68)))
                        .label(format!("{}%", ram_pct)),
                    mem_layout[1]
                );

                frame.render_widget(Paragraph::new(format!("GPU VRAM: {:.1} / {:.1} GB", gpu.vram_used, gpu.vram_total)).bold(), mem_layout[2]);
                frame.render_widget(
                    Gauge::default()
                        .percent(vram_pct)
                        .gauge_style(Style::default().fg(Color::Rgb(166, 227, 161)).bg(Color::Rgb(49, 50, 68)))
                        .label(format!("{}%", vram_pct)),
                    mem_layout[3]
                );

                // Draw RAM / VRAM line chart
                let ram_slice = self.ram_history.get_data();
                let vram_slice = self.vram_history.get_data();
                let datasets = vec![
                    Dataset::default()
                        .name("RAM")
                        .marker(Marker::Braille)
                        .style(Style::default().fg(Color::Rgb(245, 194, 231)))
                        .graph_type(GraphType::Line)
                        .data(ram_slice),
                    Dataset::default()
                        .name("VRAM")
                        .marker(Marker::Braille)
                        .style(Style::default().fg(Color::Rgb(166, 227, 161)))
                        .graph_type(GraphType::Line)
                        .data(vram_slice),
                ];
                let max_y = gpu.mem_total.max(gpu.vram_total).max(8.0);
                let mem_chart = Chart::new(datasets)
                    .x_axis(Axis::default().bounds(self.ram_history.x_bounds()).labels(vec![Line::from("-60s"), Line::from("now")]).style(Style::default().fg(Color::Rgb(88, 91, 112))))
                    .y_axis(Axis::default().bounds([0.0, max_y]).labels(vec![Line::from("0 GB"), Line::from(format!("{:.0} GB", max_y))]).style(Style::default().fg(Color::Rgb(88, 91, 112))));
                frame.render_widget(mem_chart, mem_layout[4]);

                // Render GPU Compute & power details
                let gpu_layout = Layout::vertical([
                    Constraint::Length(1), // GPU Load label
                    Constraint::Length(1), // GPU Load gauge
                    Constraint::Length(1), // Power & Temp labels
                    Constraint::Fill(1),   // Graph
                ]).split(gpu_inner);

                let usage_pct = gpu.usage as u16;
                frame.render_widget(Paragraph::new("GPU Compute Load:").bold(), gpu_layout[0]);
                frame.render_widget(
                    Gauge::default()
                        .percent(usage_pct)
                        .gauge_style(Style::default().fg(Color::Rgb(137, 180, 250)).bg(Color::Rgb(49, 50, 68)))
                        .label(format!("{}%", usage_pct)),
                    gpu_layout[1]
                );

                let extra_stats = format!("Power: {:.0} W | Core Temp: {:.0} °C", gpu.power, gpu.temp);
                frame.render_widget(Paragraph::new(extra_stats).bold().fg(Color::Rgb(250, 179, 135)), gpu_layout[2]);

                // Draw GPU Load line chart
                let load_slice = self.gpu_load_history.get_data();
                let gpu_dataset = Dataset::default()
                    .name("GPU Compute")
                    .marker(Marker::Braille)
                    .style(Style::default().fg(Color::Rgb(137, 180, 250)))
                    .graph_type(GraphType::Line)
                    .data(load_slice);

                let gpu_chart = Chart::new(vec![gpu_dataset])
                    .x_axis(Axis::default().bounds(self.gpu_load_history.x_bounds()).labels(vec![Line::from("-60s"), Line::from("now")]).style(Style::default().fg(Color::Rgb(88, 91, 112))))
                    .y_axis(Axis::default().bounds([0.0, 105.0]).labels(vec![Line::from("0%"), Line::from("100%")]).style(Style::default().fg(Color::Rgb(88, 91, 112))));
                frame.render_widget(gpu_chart, gpu_layout[3]);
            }
        }
    }

    fn draw_main_panel(&self, frame: &mut Frame, area: Rect) {
        let main_splits = Layout::vertical([
            Constraint::Length(8), // Slots Table
            Constraint::Fill(1),   // Selected Slot Performance chart
            Constraint::Length(7), // Diagnostics Logs
        ]).split(area);

        // Block wrappers
        let table_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .title(" ACTIVE INFERENCE SLOTS ")
            .bg(Color::Rgb(24, 24, 37));
        let table_inner = table_block.inner(main_splits[0]);
        frame.render_widget(table_block, main_splits[0]);

        let graph_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .title(" ACTIVE SLOT GRAPH ")
            .bg(Color::Rgb(24, 24, 37));
        let graph_inner = graph_block.inner(main_splits[1]);
        frame.render_widget(graph_block, main_splits[1]);

        let logs_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .title(" SYSTEM DIAGNOSTIC LOGS ")
            .bg(Color::Rgb(17, 17, 27));
        let logs_inner = logs_block.inner(main_splits[2]);
        frame.render_widget(logs_block, main_splits[2]);

        // 1. Draw Table
        let mut rows = Vec::new();
        let mut active_slots = Vec::new();

        if let Some(ref data) = self.current_data {
            if let Some(ref slots) = data.llama_slots {
                for s in slots {
                    if s.busy {
                        active_slots.push(s.clone());
                    }

                    let status_span = if s.busy {
                        if s.n_prompt_tokens_processed < s.n_prompt_tokens {
                            Span::styled("● Prefill", Style::default().fg(Color::Rgb(250, 179, 135)).bold())
                        } else {
                            Span::styled("● Decoding", Style::default().fg(Color::Rgb(166, 227, 161)).bold())
                        }
                    } else {
                        Span::styled("○ Idle", Style::default().fg(Color::Rgb(108, 112, 134)))
                    };

                    let task_id = if s.busy { s.task_id.clone() } else { "-".to_string() };
                    let progress = if s.busy {
                        format!("{}/{}", s.n_decoded, s.n_decoded + s.n_remain)
                    } else {
                        "-".to_string()
                    };

                    let prompt_ingest = if s.busy && s.n_prompt_tokens > 0 {
                        let pct = (s.n_prompt_tokens_processed as f64 / s.n_prompt_tokens as f64 * 100.0) as u16;
                        format!("{}% ({}/{})", pct, s.n_prompt_tokens_processed, s.n_prompt_tokens)
                    } else {
                        "-".to_string()
                    };

                    let tps = if s.busy { format!("{:.1} t/s", s.tps) } else { "-".to_string() };
                    let kv_pct = if s.n_ctx > 0 {
                        let pct = ((s.n_prompt_tokens_processed + s.n_decoded) as f64 / s.n_ctx as f64) * 100.0;
                        format!("{:.1}%", pct)
                    } else {
                        "-".to_string()
                    };

                    rows.push(Row::new(vec![
                        s.id.to_string(),
                        status_span.content.to_string(),
                        task_id,
                        progress,
                        prompt_ingest,
                        tps,
                        kv_pct,
                    ]));
                }
            }
        }

        let headers = vec!["ID", "Status", "Task ID", "Progress", "Prompt Ingest", "TPS", "KV Cache %"];
        let header_row = Row::new(headers).bold().fg(Color::Rgb(137, 180, 250));
        let widths = [
            Constraint::Percentage(5),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(15),
        ];

        let table = Table::new(rows, widths)
            .header(header_row)
            .row_highlight_style(Style::default().bg(Color::Rgb(49, 50, 68)))
            .bg(Color::Rgb(24, 24, 37));

        frame.render_widget(table, table_inner);

        // 2. Draw Slot Graph
        let all_slots = self.current_data.as_ref()
            .and_then(|d| d.llama_slots.as_ref());

        if let Some(slots) = all_slots {
            if !slots.is_empty() {
                let idx = self.selected_slot_index.min(slots.len() - 1);
                let s = &slots[idx];

                if let Some((tps_hist, kv_hist)) = self.slot_histories.get(&s.id) {
                    let tps_data = tps_hist.get_data();
                    let kv_data = kv_hist.get_data();

                    let datasets = vec![
                        Dataset::default()
                            .name("TPS (t/s)")
                            .marker(Marker::Braille)
                            .style(Style::default().fg(Color::Rgb(249, 226, 175)))
                            .graph_type(GraphType::Line)
                            .data(tps_data),
                        Dataset::default()
                            .name("KV Cache %")
                            .marker(Marker::Braille)
                            .style(Style::default().fg(Color::Rgb(203, 166, 247)))
                            .graph_type(GraphType::Line)
                            .data(kv_data),
                    ];

                    let max_tps_in_hist = tps_data.iter().map(|&(_, y)| y).fold(0.0f64, |a, b| a.max(b));
                    let max_y = 100.0f64.max(max_tps_in_hist + 10.0);

                    let slot_chart = Chart::new(datasets)
                        .x_axis(Axis::default().bounds(tps_hist.x_bounds()).labels(vec![Line::from("-60s"), Line::from("now")]).style(Style::default().fg(Color::Rgb(88, 91, 112))))
                        .y_axis(Axis::default().bounds([0.0, max_y]).labels(vec![Line::from("0"), Line::from(format!("{:.0}", max_y))]).style(Style::default().fg(Color::Rgb(88, 91, 112))))
                        .block(Block::default().title(format!(" Graph for Slot {} | Status: {} | Current TPS: {:.1} t/s | KV Used: {:.1}% ", 
                            s.id,
                            if s.busy { "Busy" } else { "Idle" },
                            s.tps,
                            if s.n_ctx > 0 { ((s.n_prompt_tokens_processed + s.n_decoded) as f64 / s.n_ctx as f64) * 100.0 } else { 0.0 }
                        ).bold().fg(Color::Rgb(249, 226, 175))));

                    frame.render_widget(slot_chart, graph_inner);
                } else {
                    let no_history_msg = Paragraph::new("\n\nNo telemetry history available for the selected slot.")
                        .alignment(Alignment::Center)
                        .fg(Color::Rgb(108, 112, 134));
                    frame.render_widget(no_history_msg, graph_inner);
                }
            } else {
                let no_slots_msg = Paragraph::new("\n\nNo slots are available.")
                    .alignment(Alignment::Center)
                    .fg(Color::Rgb(108, 112, 134));
                frame.render_widget(no_slots_msg, graph_inner);
            }
        } else {
            let offline_msg = Paragraph::new("\n\nLlama server is offline.")
                .alignment(Alignment::Center)
                .fg(Color::Rgb(108, 112, 134));
            frame.render_widget(offline_msg, graph_inner);
        }

        // 3. Draw Logs
        let log_items: Vec<ListItem> = self.logs.iter().map(|log| {
            let lvl_span = match log.level {
                LogLevel::Info => Span::styled("INFO", Style::default().fg(Color::Rgb(137, 220, 235))),
                LogLevel::Warn => Span::styled("WARN", Style::default().fg(Color::Rgb(249, 226, 175))),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", log.timestamp), Style::default().fg(Color::Rgb(88, 91, 112))),
                lvl_span,
                Span::raw(format!(" {}", log.message)),
            ]))
        }).collect();

        // Render the last N lines that fit into the log viewport (logs_inner.height)
        let log_list = List::new(log_items)
            .bg(Color::Rgb(17, 17, 27));
        frame.render_widget(log_list, logs_inner);
    }
}
