//! Render the real Ratatui buffer as SVG: cargo run --example snapshot -- demo mocha 140 42
use llamascope::{
    app::AppState,
    demo,
    telemetry::{ConnectionState, TelemetryData},
    theme::Theme,
};
use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier},
    Terminal,
};
use std::time::Instant;

fn color(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "transparent".into(),
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace(' ', "&#160;")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = args.first().map_or("demo", String::as_str);
    let theme = match args.get(1).map_or("mocha", String::as_str) {
        "mocha" => Theme::Mocha,
        "latte" => Theme::Latte,
        _ => return Err("theme: mocha or latte".into()),
    };
    let width: u16 = args.get(2).map_or(Ok(140), |s| s.parse())?;
    let height: u16 = args.get(3).map_or(Ok(42), |s| s.parse())?;
    if !(1..=300).contains(&width) || !(1..=100).contains(&height) {
        return Err("snapshot bounds: 1–300 columns, 1–100 rows".into());
    }
    let mut state = AppState::new();
    state.theme = theme;
    if matches!(
        mode,
        "demo" | "help" | "frozen" | "degraded" | "hardware-missing"
    ) {
        demo::seed(&mut state);
        state.log(
            false,
            "Explore the signal. Tab selects a slot; ? opens the field guide.",
        );
    }
    match mode {
        "connecting" | "demo" => {},
        "help" => state.help = true,
        "frozen" => state.paused_at = Some(Instant::now()),
        "loading" => state.update(TelemetryData::status(ConnectionState::Loading, "Model is loading. Waiting for llama-server; no action needed.")),
        "offline" => state.update(TelemetryData::status(ConnectionState::Offline, "Cannot reach /health. Check the server URL and network; r retries.")),
        "degraded" => {
            state.demo = false;
            state.endpoint = "simulation (no server)".into();
            state.update(TelemetryData::status(ConnectionState::Degraded, "SIMULATED ERROR: invalid /slots schema. Retained readings are demo data; r retries."));
            state.update_hardware(llamascope::hardware::HardwareTelemetry { note: "Simulated unavailable sensor; no probes were run.".into(), ..Default::default() }, Instant::now());
        }
        "empty" => {
            let mut data = TelemetryData::status(ConnectionState::Live, "Connected. The server reports an empty slot list.");
            data.llama_slots = Some(Vec::new());
            state.update(data);
        }
        "hardware-missing" => state.update_hardware(llamascope::hardware::HardwareTelemetry { note: "rocm-smi is not installed; GPU telemetry is optional.".into(), ..Default::default() }, Instant::now()),
        _ => return Err("state: demo, help, frozen, connecting, loading, offline, degraded, empty, hardware-missing".into()),
    }
    for log in &mut state.logs {
        log.timestamp = "09:41:00".into();
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| state.draw(frame))?;
    let buffer = terminal.backend().buffer();
    println!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" role=\"img\"><title>LlamaScope — {mode}, {} theme</title><desc>Actual terminal rendering. Demo readings are simulated, not benchmark results.</desc><rect width=\"100%\" height=\"100%\" fill=\"{}\"/><g font-family=\"Menlo,DejaVu Sans Mono,monospace\" font-size=\"13\" xml:space=\"preserve\">", width * 8, height * 18, width * 8, height * 18, theme.name(), color(theme.palette().base));
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let start = x;
            let cell = &buffer[(x, y)];
            let mut text = String::new();
            let mut positions = Vec::new();
            while x < width && buffer[(x, y)].style() == cell.style() {
                text.push_str(buffer[(x, y)].symbol());
                positions.push((x * 8).to_string());
                x += 1;
            }
            println!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"18\" fill=\"{}\"/>",
                start * 8,
                y * 18,
                (x - start) * 8,
                color(cell.bg)
            );
            if !text.trim().is_empty() {
                println!(
                    "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-weight=\"{}\">{}</text>",
                    positions.join(" "),
                    y * 18 + 14,
                    color(cell.fg),
                    if cell.modifier.contains(Modifier::BOLD) {
                        "700"
                    } else {
                        "400"
                    },
                    escape(&text)
                );
            }
        }
    }
    println!("</g></svg>");
    Ok(())
}
