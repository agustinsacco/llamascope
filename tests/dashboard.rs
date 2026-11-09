use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use llamascope::{
    app::{Action, AppState, RollingHistory},
    demo,
    hardware::{GpuTelemetry, HardwareTelemetry, Memory},
    telemetry::{ConnectionState, LlamaSlot, TelemetryData},
    theme::Theme,
};
use ratatui::{backend::TestBackend, style::Color, Terminal};
use std::time::{Duration, Instant};

fn sample_state() -> AppState {
    let mut state = AppState::new();
    demo::seed(&mut state);
    state
}
fn render(state: &AppState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| state.draw(frame)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Every distinct screen: connecting, demo, loading, offline, degraded, empty, help,
/// frozen and missing sensors.
fn every_state(theme: Theme) -> Vec<AppState> {
    (0..9)
        .map(|mode| {
            let mut state = if mode == 0 {
                AppState::new()
            } else {
                sample_state()
            };
            state.theme = theme;
            match mode {
                2 => state.update(TelemetryData::status(
                    ConnectionState::Loading,
                    "Model is loading",
                )),
                3 => state.update(TelemetryData::status(
                    ConnectionState::Offline,
                    "Server unreachable; r retries",
                )),
                4 => state.update(TelemetryData::status(
                    ConnectionState::Degraded,
                    "Access denied; set LLAMA_API_KEY",
                )),
                5 => {
                    let mut data = TelemetryData::status(ConnectionState::Live, "No slots");
                    data.llama_slots = Some(vec![]);
                    state.update(data);
                }
                6 => state.help = true,
                7 => state.paused_at = Some(Instant::now()),
                8 => state.update_hardware(HardwareTelemetry::default(), Instant::now()),
                _ => {}
            }
            state
        })
        .collect()
}
const SIZES: [(u16, u16); 9] = [
    (0, 0),
    (1, 1),
    (40, 10),
    (59, 17),
    (60, 18),
    (80, 24),
    (110, 32),
    (140, 42),
    (200, 60),
];

#[test]
fn every_state_and_theme_renders_at_tiny_compact_and_wide_sizes() {
    for theme in [Theme::Mocha, Theme::Latte] {
        for state in every_state(theme) {
            for (width, height) in SIZES {
                let text = render(&state, width, height);
                if width >= 60 && height >= 18 {
                    let mode = if state.demo {
                        "SIMULATED DATA"
                    } else {
                        "READ ONLY"
                    };
                    assert!(text.contains("LLAMASCOPE") && text.contains(mode));
                    assert!(!text.contains("COMPACT TERMINAL"));
                } else if width >= 20 && height >= 6 {
                    assert!(text.contains("COMPACT TERMINAL"), "{width}x{height}");
                }
            }
        }
    }
}
#[test]
fn help_shows_server_and_build_even_at_the_minimum_size() {
    let mut state = sample_state();
    state.help = true;
    let text = render(&state, 60, 18);
    assert!(text.contains("SERVER") && text.contains("BUILD   demo-build"));
}
#[test]
fn loading_offline_and_empty_states_explain_what_to_do() {
    for (status, expected) in [
        (ConnectionState::Connecting, "Establishing a connection"),
        (ConnectionState::Loading, "The model is warming up"),
        (ConnectionState::Offline, "Telemetry needs attention"),
        (ConnectionState::Live, "No slots reported"),
    ] {
        let mut state = AppState::new();
        state.update(TelemetryData::status(status, "Check URL; r retries"));
        assert!(render(&state, 100, 30).contains(expected));
    }
}
#[test]
fn selected_slot_survives_reordering_and_removal_of_other_slots() {
    let mut state = sample_state();
    state.select(true);
    assert_eq!(state.selected_slot, Some(1));
    let mut data = demo::telemetry(121, state.started_at);
    data.llama_slots.as_mut().unwrap().reverse();
    state.update(data.clone());
    assert_eq!(state.selected_slot, Some(1));
    data.llama_slots.as_mut().unwrap().retain(|s| s.id != 0);
    state.update(data);
    assert_eq!(state.selected_slot, Some(1));
    assert!(render(&state, 140, 42).contains("SLOT 01 / PREFILL"));
}
#[test]
fn selection_scrolls_to_slots_beyond_the_viewport() {
    let mut state = AppState::new();
    let mut data = TelemetryData::status(ConnectionState::Live, "Connected");
    data.llama_slots = Some(
        (0..100)
            .map(|id| LlamaSlot {
                id,
                ..Default::default()
            })
            .collect(),
    );
    state.update(data);
    state.select(false);
    assert_eq!(state.selected_slot, Some(99));
    let text = render(&state, 140, 42);
    assert!(text
        .lines()
        .any(|line| line.contains("› 99") && line.contains("Idle")));
    assert!(text.contains("SLOT 99 / IDLE"));
}
#[test]
fn failed_polls_retain_history_and_mark_last_known_values() {
    let mut state = sample_state();
    state.demo = false;
    let count = state.histories[&0].tps.data().len();
    state.update(TelemetryData::status(
        ConnectionState::Degraded,
        "Invalid JSON/schema",
    ));
    assert_eq!(state.histories[&0].tps.data().len(), count);
    assert_eq!(state.slots.len(), 4);
    let text = render(&state, 140, 42);
    assert!(text.contains("DEGRADED") && text.contains("LAST KNOWN"));
}
#[test]
fn slot_rate_and_context_history_reset_on_new_task() {
    let mut state = sample_state();
    let mut data = demo::telemetry(122, state.started_at);
    data.llama_slots.as_mut().unwrap()[0].task_id = Some("new-task".into());
    state.update(data);
    assert_eq!(state.histories[&0].tps.data().len(), 1);
    assert_eq!(state.histories[&0].context.data().len(), 1);
}
#[test]
fn empty_success_clears_selection_and_obsolete_histories() {
    let mut state = sample_state();
    let mut data = TelemetryData::status(ConnectionState::Live, "Connected");
    data.llama_slots = Some(vec![]);
    state.update(data);
    assert!(state.slots.is_empty() && state.histories.is_empty() && state.selected_slot.is_none());
    let text = render(&state, 140, 42);
    assert!(text.contains("0.0 t/s"));
    assert!(
        !text.contains("-0.0"),
        "an empty float sum must not render negative zero"
    );
}
#[test]
fn stale_threshold_respects_the_configured_poll_interval() {
    let mut state = sample_state();
    state.demo = false;
    state.last_slots_at = Some(Instant::now() - Duration::from_secs(5));
    assert!(render(&state, 140, 42).contains("STALE"));
    state.stale_after = Duration::from_secs(30);
    assert!(!render(&state, 140, 42).contains("STALE"));
}
#[test]
fn latest_events_are_visible_and_repeated_failures_are_deduplicated() {
    let mut state = sample_state();
    state.logs.clear();
    let data = TelemetryData::status(ConnectionState::Offline, "Offline");
    state.update(data.clone());
    state.update(data);
    assert_eq!(state.logs.len(), 1);
    for n in 0..150 {
        state.log(false, format!("event-{n:03}"));
    }
    assert_eq!(state.logs.len(), 100);
    let text = render(&state, 140, 42);
    assert!(text.contains("event-149"));
    assert!(!text.contains("event-050"));
}
#[test]
fn keybindings_freeze_theme_help_selection_retry_and_quit() {
    let mut state = sample_state();
    state.handle_key(key(KeyCode::Tab));
    assert_eq!(state.selected_slot, Some(1));
    state.handle_key(key(KeyCode::BackTab));
    assert_eq!(state.selected_slot, Some(0));
    state.handle_key(key(KeyCode::Char('t')));
    assert_eq!(state.theme, Theme::Latte);
    state.handle_key(key(KeyCode::Char('?')));
    assert!(render(&state, 100, 30).contains("YOUR INFERENCE FIELD GUIDE"));
    state.handle_key(key(KeyCode::Esc));
    assert!(!state.help);
    state.handle_key(key(KeyCode::Char(' ')));
    assert!(render(&state, 100, 30).contains("FROZEN"));
    state.handle_key(key(KeyCode::Char(' ')));
    assert!(state.paused_at.is_none());
    assert_eq!(state.handle_key(key(KeyCode::Char('r'))), Action::None);
    assert!(state
        .logs
        .back()
        .unwrap()
        .message
        .contains("no server to retry"));
    state.demo = false;
    assert_eq!(state.handle_key(key(KeyCode::Char('r'))), Action::Retry);
    state.handle_key(key(KeyCode::Char('c')));
    assert_eq!(state.logs.len(), 1);
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Action::Quit
    );
    assert_eq!(state.handle_key(key(KeyCode::Char('q'))), Action::Quit);
    let mut release = key(KeyCode::Char('q'));
    release.kind = KeyEventKind::Release;
    assert_eq!(state.handle_key(release), Action::None);
}
#[test]
fn history_is_time_based_bounded_and_rejects_nonfinite_values() {
    let mut history = RollingHistory::default();
    for tick in 0..2000 {
        history.push(tick as f64 / 4.0, 20.0);
    }
    assert!(history.data().len() <= 241);
    let end = history.data().last().unwrap().0;
    assert_eq!(history.bounds(), [end - 60.0, end]);
    assert!(history.data().first().unwrap().0 >= end - 60.0);
    let count = history.data().len();
    for value in [f64::NAN, f64::INFINITY, -1.0] {
        history.push(end, value);
    }
    assert_eq!(history.data().len(), count);
}
#[test]
fn invalid_sensor_values_cannot_panic_gauges() {
    for value in [f64::NAN, f64::INFINITY, -10.0, 200.0] {
        let mut state = sample_state();
        state.update_hardware(
            HardwareTelemetry {
                memory: Some(Memory {
                    used_gib: value,
                    total_gib: 1.0,
                }),
                gpu: Some(GpuTelemetry {
                    usage: Some(value),
                    ..Default::default()
                }),
                note: String::new(),
            },
            Instant::now(),
        );
        render(&state, 140, 42);
    }
}
#[test]
fn missing_gpu_has_guidance_not_an_empty_panel() {
    let mut state = sample_state();
    state.update_hardware(
        HardwareTelemetry {
            note: "rocm-smi unavailable".into(),
            ..Default::default()
        },
        Instant::now(),
    );
    let text = render(&state, 140, 42);
    assert!(text.contains("GPU telemetry is optional"));
    assert!(text.contains("Host memory unavailable"));
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(r, g, b) = color else {
        panic!("theme must use explicit RGB tokens")
    };
    let linear = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}
#[test]
fn text_tokens_have_at_least_4_5_to_1_contrast_on_every_surface() {
    for theme in [Theme::Mocha, Theme::Latte] {
        let p = theme.palette();
        for fg in [
            p.text,
            p.muted,
            p.accent,
            p.good,
            p.warning,
            p.error,
            p.secondary,
        ] {
            for bg in [p.base, p.panel, p.raised] {
                let (a, b) = (luminance(fg), luminance(bg));
                let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                assert!(ratio >= 4.5, "{theme:?}: {fg:?} on {bg:?} = {ratio:.2}");
            }
        }
    }
}
fn contrast(fg: Color, bg: Color) -> f64 {
    let (a, b) = (luminance(fg), luminance(bg));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
/// Checks the cells actually drawn, not just palette tokens: every cell paints its own
/// background (a light or dark terminal never shows through), text reaches 4.5:1, and
/// chart dots, bars and borders reach 3:1.
#[test]
fn every_rendered_cell_paints_its_background_and_is_legible() {
    let mut failures = std::collections::BTreeSet::new();
    for theme in [Theme::Mocha, Theme::Latte] {
        for (index, state) in every_state(theme).into_iter().enumerate() {
            for (width, height) in [(60, 18), (80, 24), (140, 42)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|frame| state.draw(frame)).unwrap();
                let buffer = terminal.backend().buffer();
                for y in 0..height {
                    for x in 0..width {
                        let cell = &buffer[(x, y)];
                        let at = format!("{theme:?} state {index} {width}x{height} ({x},{y})");
                        assert!(
                            matches!(cell.bg, Color::Rgb(..)),
                            "{at}: unpainted background"
                        );
                        let symbol = cell.symbol();
                        if symbol.trim().is_empty() {
                            continue;
                        }
                        assert!(
                            matches!(cell.fg, Color::Rgb(..)),
                            "{at}: default foreground"
                        );
                        let graphic = symbol.chars().all(|c| {
                            matches!(c, '\u{2500}'..='\u{259f}' | '\u{2800}'..='\u{28ff}' | '•' | '·' | '›')
                        });
                        let minimum = if graphic { 3.0 } else { 4.5 };
                        let ratio = contrast(cell.fg, cell.bg);
                        if ratio < minimum {
                            failures.insert(format!(
                                "{theme:?} {} {:?} on {:?} = {ratio:.2}",
                                if graphic { "graphic" } else { "text" },
                                cell.fg,
                                cell.bg
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
#[test]
fn from_config_carries_endpoint_theme_demo_and_staleness() {
    let config = llamascope::config::Config::parse(
        [
            "--interval",
            "2000",
            "--theme",
            "latte",
            "--url",
            "http://h:1/",
        ]
        .map(String::from),
        |_| None,
    )
    .unwrap();
    let state = AppState::from_config(&config);
    assert_eq!(state.endpoint, "http://h:1");
    assert_eq!(state.theme, Theme::Latte);
    assert_eq!(state.stale_after, Duration::from_secs(6));
    assert!(!state.demo);
    let fast = llamascope::config::Config::parse(["--interval", "250"].map(String::from), |_| None)
        .unwrap();
    assert_eq!(
        AppState::from_config(&fast).stale_after,
        Duration::from_secs(3)
    );
}
#[test]
fn frozen_view_holds_samples_and_applies_the_latest_on_resume() {
    let mut state = sample_state();
    state.handle_key(key(KeyCode::Char(' ')));
    let mut data = demo::telemetry(130, state.started_at);
    data.llama_props.as_mut().unwrap().model_name = "after-resume.gguf".into();
    state.receive(data);
    state.receive_hardware(HardwareTelemetry::default(), Instant::now());
    assert!(!render(&state, 140, 42).contains("after-resume.gguf"));
    assert!(state.hardware.as_ref().unwrap().memory.is_some());
    state.handle_key(key(KeyCode::Char(' ')));
    assert!(render(&state, 140, 42).contains("after-resume.gguf"));
    assert!(state.hardware.as_ref().unwrap().memory.is_none());
}
#[test]
fn stopped_sensors_are_reported_and_server_monitoring_continues() {
    let mut state = sample_state();
    state.sensors_stopped("collector ended unexpectedly");
    let text = render(&state, 140, 42);
    assert!(text.contains("Local sensors stopped"));
    assert!(text.contains("demo-1042"), "slots still render");
}
#[test]
fn long_repeated_details_are_logged_once() {
    let mut state = AppState::new();
    let long = "x".repeat(400);
    for _ in 0..3 {
        state.update(TelemetryData::status(ConnectionState::Offline, &long));
    }
    assert_eq!(state.logs.len(), 1);
}
#[test]
fn gpu_panel_names_its_source_and_shows_gtt() {
    let mut state = sample_state();
    state.demo = false;
    state.update_hardware(
        HardwareTelemetry {
            gpu: llamascope::hardware::parse_rocm(
                r#"{"card0":{"GPU use (%)":"90","VRAM Total Memory (B)":"2147483648","VRAM Total Used Memory (B)":"1073741824","GTT Total Memory (B)":"137438953472","GTT Total Used Memory (B)":"68719476736"}}"#,
            ),
            ..Default::default()
        },
        Instant::now(),
    );
    let text = render(&state, 140, 42);
    assert!(text.contains("VRAM 1.0/2.0 GiB") && text.contains("GTT 64.0/128.0 GiB"));
    assert!(text.contains("AMD card0 · rocm-smi"));
    assert!(!text.contains("Server queue"));
}
