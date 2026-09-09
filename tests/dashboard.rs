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
    state.started_at = Instant::now() - Duration::from_secs(60);
    state.demo = true;
    for tick in 0..120 {
        let (data, hardware) = demo::sample(tick, state.started_at);
        let at = data.sampled_at;
        state.update(data);
        state.update_hardware(hardware, at);
    }
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

#[test]
fn every_state_and_theme_renders_at_tiny_compact_and_wide_sizes() {
    for theme in [Theme::Mocha, Theme::Latte] {
        for mode in 0..9 {
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
            for (width, height) in [
                (0, 0),
                (1, 1),
                (40, 10),
                (59, 17),
                (60, 18),
                (80, 24),
                (110, 32),
                (140, 42),
                (200, 60),
            ] {
                let text = render(&state, width, height);
                if width >= 60 && height >= 18 {
                    assert!(text.contains("LLAMASCOPE"));
                }
            }
        }
    }
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
    let mut data = demo::sample(121, state.started_at).0;
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
    let mut data = demo::sample(122, state.started_at).0;
    data.llama_slots.as_mut().unwrap()[0].task_id = Some("new-task".into());
    state.update(data);
    assert_eq!(state.histories[&0].tps.data().len(), 1);
    assert_eq!(state.histories[&0].kv.data().len(), 1);
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
