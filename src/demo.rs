//! Deterministic, explicitly labeled simulation. No HTTP or hardware probes.
use crate::{
    hardware::{GpuTelemetry, HardwareTelemetry, Memory},
    telemetry::{ConnectionState, LlamaProps, LlamaSlot, TelemetryData},
};
use std::time::{Duration, Instant};

pub fn sample(tick: u64, epoch: Instant) -> (TelemetryData, HardwareTelemetry) {
    let wave = (tick as f64 / 9.0).sin();
    let slots = vec![
        LlamaSlot {
            id: 0,
            busy: true,
            task_id: Some("demo-1042".into()),
            n_ctx: Some(8192),
            n_prompt_tokens: Some(2048),
            n_prompt_tokens_processed: Some(2048),
            n_decoded: Some(600 + tick % 500),
            n_remain: Some(-1),
            tps: Some(38.0 + wave * 7.0),
        },
        LlamaSlot {
            id: 1,
            busy: true,
            task_id: Some(format!("demo-prefill-{}", tick / 180)),
            n_ctx: Some(16384),
            n_prompt_tokens: Some(12000),
            n_prompt_tokens_processed: Some(1500 + tick % 180 * 50),
            n_decoded: Some(0),
            n_remain: Some(1024),
            tps: Some(0.0),
        },
        LlamaSlot {
            id: 2,
            n_ctx: Some(8192),
            ..LlamaSlot::default()
        },
        LlamaSlot {
            id: 3,
            busy: true,
            task_id: Some("demo-1044".into()),
            n_ctx: Some(8192),
            n_prompt_tokens: Some(6400),
            n_prompt_tokens_processed: Some(6400),
            n_decoded: Some(520 + tick % 400),
            n_remain: Some(512),
            tps: Some(29.0 + (tick as f64 / 6.0).cos() * 5.0),
        },
    ];
    let data = TelemetryData {
        connection: ConnectionState::Live,
        detail: "Simulated telemetry. No server requests or hardware probes.".into(),
        llama_slots: Some(slots),
        llama_props: Some(LlamaProps {
            model_name: "Llama-3.1-8B-Instruct-Q4_K_M.gguf".into(),
            total_slots: 4,
            queue_size: Some(2),
            build_info: Some("demo-build".into()),
            is_sleeping: false,
        }),
        sampled_at: epoch + Duration::from_millis(tick * 500),
        latency: Duration::from_millis(8),
    };
    let hardware = HardwareTelemetry {
        memory: Some(Memory {
            used_gib: 18.2 + wave * 0.3,
            total_gib: 64.0,
        }),
        gpu: Some(GpuTelemetry {
            usage: Some(76.0 + wave * 10.0),
            temp: Some(62.0 + wave * 2.0),
            power: Some(148.0 + wave * 12.0),
            vram: Some(Memory {
                used_gib: 9.4 + wave * 0.2,
                total_gib: 24.0,
            }),
        }),
        note: "Simulated AMD card0 / demo".into(),
    };
    (data, hardware)
}
