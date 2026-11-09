//! Local host sensors: RAM and the optional GPU panel.
use super::{draw_chart, draw_gauge, fmt, panel};
use crate::{app::AppState, hardware::Memory};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Paragraph, Wrap},
    Frame,
};

fn gib(name: &str, memory: Option<&Memory>) -> String {
    memory.map_or_else(
        || format!(" {name} --"),
        |m| format!(" {name} {:.1}/{:.1} GiB", m.used_gib, m.total_gib),
    )
}

impl AppState {
    pub(super) fn draw_hardware(&self, frame: &mut Frame, area: Rect) {
        let p = self.theme.palette();
        let [memory_area, gpu_area] = area.layout(&Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]));
        let mem_block = panel(" LOCAL HOST / RAM ", p);
        let mem_inner = mem_block.inner(memory_area);
        frame.render_widget(mem_block, memory_area);
        let gpu_block = panel(" LOCAL GPU ", p);
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
        } else {
            frame.render_widget(Paragraph::new(" Host memory unavailable.\n Supported on Linux and macOS.\n Server telemetry is unaffected.").fg(p.muted).wrap(Wrap { trim: true }), mem_inner);
        }
        if let Some(gpu) = &hardware.gpu {
            let parts = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(2 + u16::from(gpu.gtt.is_some())),
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
                    Line::from(gib("VRAM", gpu.vram.as_ref())).fg(p.muted),
                    Line::from(
                        gpu.gtt
                            .as_ref()
                            .map_or_else(String::new, |m| gib("GTT", Some(m))),
                    )
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
                Paragraph::new(vec![
                    Line::from(format!(" {}", gpu.source)),
                    Line::from(if self.demo {
                        " Not real hardware"
                    } else {
                        " This machine, not the server"
                    }),
                ])
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
}
