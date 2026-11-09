//! Deterministic, explicitly labeled simulation. No HTTP or hardware probes.
use crate::{
    app::AppState,
    hardware::{GpuTelemetry, HardwareTelemetry, Memory},
    telemetry::{ConnectionState, LlamaProps, LlamaSlot, TelemetryData},
};
use std::time::{Duration, Instant};

/// Simulated samples arrive on the same cadence as the default poll interval.
pub const TICK: Duration = Duration::from_millis(500);
const SEED_TICKS: u64 = 120;

fn wave(tick: u64) -> f64 {
    (tick as f64 / 9.0).sin()
}

/// The tick whose sample is current at `now`, given the simulation epoch.
pub fn tick_at(epoch: Instant, now: Instant) -> u64 {
    (now.saturating_duration_since(epoch).as_millis() / TICK.as_millis()) as u64
}

/// Slot counters follow current llama-server semantics: `n_prompt_tokens` counts every
/// token held in the slot, so it grows with generation.
pub fn telemetry(tick: u64, epoch: Instant) -> TelemetryData {
    let decoding = |id, task: &str, cached, processed, decoded, remain, tps| LlamaSlot {
        id,
        busy: true,
        task_id: Some(task.into()),
        n_ctx: Some(8192),
        n_prompt_tokens: Some(cached + processed + decoded),
        n_prompt_tokens_processed: Some(processed),
        n_prompt_tokens_cache: Some(cached),
        n_decoded: Some(decoded),
        n_remain: Some(remain),
        tps: Some(tps),
    };
    let prefilled = 1500 + tick % 180 * 50;
    let slots = vec![
        decoding(
            0,
            "demo-1042",
            1024,
            1024,
            600 + tick % 500,
            -1,
            38.0 + wave(tick) * 7.0,
        ),
        LlamaSlot {
            id: 1,
            busy: true,
            task_id: Some(format!("demo-prefill-{}", tick / 180)),
            n_ctx: Some(16384),
            n_prompt_tokens: Some(prefilled),
            n_prompt_tokens_processed: Some(prefilled),
            n_prompt_tokens_cache: Some(0),
            n_decoded: Some(0),
            n_remain: Some(1024),
            tps: Some(0.0),
        },
        LlamaSlot {
            id: 2,
            n_ctx: Some(8192),
            ..LlamaSlot::default()
        },
        decoding(
            3,
            "demo-1044",
            4096,
            2304,
            520 + tick % 400,
            512,
            29.0 + (tick as f64 / 6.0).cos() * 5.0,
        ),
    ];
    TelemetryData {
        connection: ConnectionState::Live,
        detail: "Simulated telemetry. No server requests or hardware probes.".into(),
        llama_slots: Some(slots),
        llama_props: Some(LlamaProps {
            model_name: "Llama-3.1-8B-Instruct-Q4_K_M.gguf".into(),
            build_info: Some("demo-build".into()),
        }),
        sampled_at: epoch + TICK * tick as u32,
        latency: Duration::from_millis(8),
    }
}

pub fn hardware(tick: u64) -> HardwareTelemetry {
    let wave = wave(tick);
    HardwareTelemetry {
        memory: Some(Memory {
            used_gib: 18.2 + wave * 0.3,
            total_gib: 64.0,
        }),
        gpu: Some(GpuTelemetry {
            source: "Simulated GPU \u{b7} no probes",
            usage: Some(76.0 + wave * 10.0),
            temp: Some(62.0 + wave * 2.0),
            power: Some(148.0 + wave * 12.0),
            vram: Some(Memory {
                used_gib: 9.4 + wave * 0.2,
                total_gib: 24.0,
            }),
            gtt: None,
        }),
        note: "Simulated GPU / demo".into(),
    }
}

/// Mark `state` as a demo and backfill one minute of history so charts start populated.
/// Returns the simulation epoch for later ticks.
pub fn seed(state: &mut AppState) -> Instant {
    let now = Instant::now();
    // Shortly after boot the monotonic clock may not reach back a full minute.
    state.started_at = now.checked_sub(TICK * SEED_TICKS as u32).unwrap_or(now);
    state.demo = true;
    for tick in 0..SEED_TICKS {
        let data = telemetry(tick, state.started_at);
        let at = data.sampled_at;
        state.update(data);
        state.update_hardware(hardware(tick), at);
    }
    state.started_at
}
