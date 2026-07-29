use std::collections::HashMap;
use std::time::{Instant, Duration};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum LogLevel {
    Info,
    Warn,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GpuTelemetry {
    pub usage: f64,
    pub temp: f64,
    pub power: f64,
    pub mem_total: f64,
    pub mem_used: f64,
    pub vram_total: f64,
    pub vram_used: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlamaSlot {
    pub id: u32,
    pub busy: bool,
    pub task_id: String,
    pub n_ctx: u32,
    pub n_prompt_tokens: u32,
    pub n_prompt_tokens_processed: u32,
    pub n_prompt_tokens_cache: u32,
    pub n_decoded: u32,
    pub n_remain: u32,
    pub tps: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LlamaProps {
    pub model_name: String,
    pub total_slots: u32,
    pub queue_size: u32,
    pub build_info: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TelemetryData {
    pub gpu: Option<GpuTelemetry>,
    pub llama_slots: Option<Vec<LlamaSlot>>,
    pub llama_props: Option<LlamaProps>,
}

#[derive(Deserialize, Debug)]
struct RawSlotToken {
    n_decoded: u32,
    n_remain: u32,
}

#[derive(Deserialize, Debug)]
struct RawSlot {
    id: u32,
    is_processing: bool,
    id_task: Option<serde_json::Value>,
    n_ctx: u32,
    n_prompt_tokens: Option<u32>,
    n_prompt_tokens_processed: Option<u32>,
    n_prompt_tokens_cache: Option<u32>,
    next_token: Option<Vec<RawSlotToken>>,
}

#[derive(Deserialize, Debug)]
struct RawProps {
    total_slots: Option<u32>,
    model_path: Option<String>,
    queue_size: Option<u32>,
    build_info: Option<String>,
}

fn parse_json_f64(val: Option<&serde_json::Value>) -> f64 {
    match val {
        Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn get_gpu_telemetry() -> Option<GpuTelemetry> {
    // 1. Run rocm-smi -a --json
    let rocm_res = std::process::Command::new("rocm-smi")
        .args(["-a", "--json"])
        .output()
        .ok()?;

    if !rocm_res.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&rocm_res.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    let card0 = v.get("card0")?;

    let usage = parse_json_f64(card0.get("GPU use (%)"));
    let temp = parse_json_f64(card0.get("Temperature (Sensor edge) (C)"));
    let power = parse_json_f64(card0.get("Current Socket Graphics Package Power (W)"));

    // 2. Run free -b
    let free_res = std::process::Command::new("free")
        .arg("-b")
        .output()
        .ok()?;

    let free_stdout = String::from_utf8_lossy(&free_res.stdout);
    let lines: Vec<&str> = free_stdout.lines().collect();
    let mut mem_total = 0.0;
    let mut mem_used = 0.0;
    if lines.len() > 1 {
        let parts: Vec<&str> = lines[1].split_whitespace().collect();
        if parts.len() > 2 {
            let total_bytes = parts[1].parse::<u64>().unwrap_or(0);
            let used_bytes = parts[2].parse::<u64>().unwrap_or(0);
            mem_total = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            mem_used = used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        }
    }

    // 3. Run rocm-smi --showmeminfo vram --json
    let mut vram_total = 0.0;
    let mut vram_used = 0.0;
    if let Ok(vram_res) = std::process::Command::new("rocm-smi")
        .args(["--showmeminfo", "vram", "--json"])
        .output()
    {
        if vram_res.status.success() {
            let vram_stdout = String::from_utf8_lossy(&vram_res.stdout);
            if let Ok(vv) = serde_json::from_str::<serde_json::Value>(&vram_stdout) {
                if let Some(card_vram) = vv.get("card0") {
                    let total_bytes = parse_json_f64(card_vram.get("VRAM Total Memory (B)"));
                    let used_bytes = parse_json_f64(card_vram.get("VRAM Total Used Memory (B)"));
                    vram_total = total_bytes / (1024.0 * 1024.0 * 1024.0);
                    vram_used = used_bytes / (1024.0 * 1024.0 * 1024.0);
                }
            }
        }
    }

    Some(GpuTelemetry {
        usage,
        temp,
        power,
        mem_total,
        mem_used,
        vram_total,
        vram_used,
    })
}

fn get_llama_telemetry(
    client: &reqwest::blocking::Client,
    api_url: &str,
    last_counts: &mut HashMap<u32, (u32, Instant)>,
) -> (Option<Vec<LlamaSlot>>, Option<LlamaProps>) {
    let slots_res = client.get(format!("{}/slots", api_url)).send();
    let props_res = client.get(format!("{}/props", api_url)).send();

    let raw_slots: Vec<RawSlot> = match slots_res {
        Ok(res) => res.json().unwrap_or_default(),
        Err(_) => return (None, None),
    };

    let raw_props: RawProps = match props_res {
        Ok(res) => res.json().unwrap_or_else(|_| RawProps {
            total_slots: None,
            model_path: None,
            queue_size: None,
            build_info: None,
        }),
        Err(_) => return (None, None),
    };

    let now = Instant::now();
    let mut slots = Vec::new();

    for s in raw_slots {
        let n_decoded = s.next_token.as_ref()
            .and_then(|v| v.first())
            .map(|t| t.n_decoded)
            .unwrap_or(0);
        let n_remain = s.next_token.as_ref()
            .and_then(|v| v.first())
            .map(|t| t.n_remain)
            .unwrap_or(0);

        let tps = if let Some(&(prev_count, prev_time)) = last_counts.get(&s.id) {
            let dt = now.duration_since(prev_time).as_secs_f64();
            if dt > 0.0 && n_decoded > prev_count {
                (n_decoded - prev_count) as f64 / dt
            } else {
                0.0
            }
        } else {
            0.0
        };

        last_counts.insert(s.id, (n_decoded, now));

        let task_id = match s.id_task {
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::String(st)) => st,
            _ => "N/A".to_string(),
        };

        slots.push(LlamaSlot {
            id: s.id,
            busy: s.is_processing,
            task_id,
            n_ctx: s.n_ctx,
            n_prompt_tokens: s.n_prompt_tokens.unwrap_or(0),
            n_prompt_tokens_processed: s.n_prompt_tokens_processed.unwrap_or(0),
            n_prompt_tokens_cache: s.n_prompt_tokens_cache.unwrap_or(0),
            n_decoded,
            n_remain,
            tps,
        });
    }

    let model_path = raw_props.model_path.unwrap_or_else(|| "N/A".to_string());
    let model_name = std::path::Path::new(&model_path)
        .file_name()
        .map(|os_str| os_str.to_string_lossy().into_owned())
        .unwrap_or_else(|| "N/A".to_string());

    let props = LlamaProps {
        model_name,
        total_slots: raw_props.total_slots.unwrap_or(0),
        queue_size: raw_props.queue_size.unwrap_or(0),
        build_info: raw_props.build_info.unwrap_or_else(|| "N/A".to_string()),
    };

    (Some(slots), Some(props))
}

pub fn start_polling_thread(
    api_url: String,
    tx: std::sync::mpsc::Sender<TelemetryData>,
) {
    std::thread::spawn(move || {
        let mut last_counts = HashMap::new();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(15000))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        loop {
            let gpu = get_gpu_telemetry();
            let (llama_slots, llama_props) = get_llama_telemetry(&client, &api_url, &mut last_counts);

            let data = TelemetryData {
                gpu,
                llama_slots,
                llama_props,
            };

            if tx.send(data).is_err() {
                break; // Receiver hung up
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_f64() {
        // String cases
        let val_str = serde_json::Value::String("45.2".to_string());
        assert_eq!(parse_json_f64(Some(&val_str)), 45.2);

        let val_invalid_str = serde_json::Value::String("invalid".to_string());
        assert_eq!(parse_json_f64(Some(&val_invalid_str)), 0.0);

        // Number cases
        let val_num = serde_json::Value::Number(serde_json::Number::from_f64(120.5).unwrap());
        assert_eq!(parse_json_f64(Some(&val_num)), 120.5);

        // Null case
        assert_eq!(parse_json_f64(None), 0.0);
    }

    #[test]
    fn test_gpu_telemetry_default() {
        let t = GpuTelemetry::default();
        assert_eq!(t.usage, 0.0);
        assert_eq!(t.temp, 0.0);
        assert_eq!(t.mem_total, 0.0);
    }

    #[test]
    fn test_model_name_extraction() {
        let model_path = "/home/stark/models/test-model-q8_0.gguf";
        let model_name = std::path::Path::new(model_path)
            .file_name()
            .map(|os_str| os_str.to_string_lossy().into_owned())
            .unwrap_or_else(|| "N/A".to_string());
        assert_eq!(model_name, "test-model-q8_0.gguf");
    }
}

