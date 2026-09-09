//! Optional local-host sensors. These never describe a remote inference host.
use serde_json::Value;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Memory {
    pub used_gib: f64,
    pub total_gib: f64,
}
impl Memory {
    pub fn percent(&self) -> f64 {
        if self.total_gib > 0.0 {
            (self.used_gib / self.total_gib * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct GpuTelemetry {
    pub usage: Option<f64>,
    pub temp: Option<f64>,
    pub power: Option<f64>,
    pub vram: Option<Memory>,
}
#[derive(Clone, Debug, Default)]
pub struct HardwareTelemetry {
    pub memory: Option<Memory>,
    pub gpu: Option<GpuTelemetry>,
    pub note: String,
}

/// Bound both process runtime and captured output; drain stdout concurrently to avoid pipe deadlock.
fn command_output(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut output = String::new();
        let result = stdout.take(65537).read_to_string(&mut output);
        let _ = tx.send(
            result
                .ok()
                .filter(|_| output.len() <= 65536)
                .map(|_| output),
        );
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    rx.recv_timeout(Duration::from_millis(50)).ok().flatten()
                } else {
                    None
                }
            }
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn parse_meminfo(text: &str) -> Option<Memory> {
    let value = |key: &str| -> Option<f64> {
        text.lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|s| s.split_whitespace().next())?
            .parse()
            .ok()
    };
    let total = value("MemTotal:")?;
    let available = value("MemAvailable:")?;
    memory(total - available, total, 1024.0 * 1024.0)
}
fn memory(used: f64, total: f64, divisor: f64) -> Option<Memory> {
    (used.is_finite() && total.is_finite() && total > 0.0 && used >= 0.0 && used <= total)
        .then_some(Memory {
            used_gib: used / divisor,
            total_gib: total / divisor,
        })
}

pub fn parse_vm_stat(total: &str, stats: &str) -> Option<Memory> {
    let total = total.trim().parse::<f64>().ok()?;
    let page_size = stats
        .lines()
        .next()?
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()?;
    let pages = |key: &str| -> Option<f64> {
        stats
            .lines()
            .find_map(|line| line.strip_prefix(key))?
            .trim()
            .trim_end_matches('.')
            .parse()
            .ok()
    };
    let available =
        (pages("Pages free:")? + pages("Pages inactive:")? + pages("Pages speculative:")?)
            * page_size;
    memory(total - available, total, 1024.0_f64.powi(3))
}

fn number(value: Option<&Value>) -> Option<f64> {
    let n = match value? {
        Value::String(s) => s.parse().ok()?,
        Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    (n.is_finite() && n >= 0.0).then_some(n)
}
pub fn parse_rocm(text: &str) -> Option<GpuTelemetry> {
    let value: Value = serde_json::from_str(text).ok()?;
    let card = value.get("card0")?.as_object()?;
    let usage = number(card.get("GPU use (%)")).map(|n| n.min(100.0));
    let temp = number(card.get("Temperature (Sensor edge) (C)"));
    let power = number(card.get("Current Socket Graphics Package Power (W)"))
        .or_else(|| number(card.get("Average Graphics Package Power (W)")));
    let vram = number(card.get("VRAM Total Used Memory (B)"))
        .zip(number(card.get("VRAM Total Memory (B)")))
        .and_then(|(used, total)| memory(used, total, 1024.0_f64.powi(3)));
    (usage.is_some() || temp.is_some() || power.is_some() || vram.is_some()).then_some(
        GpuTelemetry {
            usage,
            temp,
            power,
            vram,
        },
    )
}

pub fn sample(no_gpu: bool) -> HardwareTelemetry {
    let memory = if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| parse_meminfo(&s))
    } else if cfg!(target_os = "macos") {
        command_output("sysctl", &["-n", "hw.memsize"], Duration::from_millis(500)).and_then(
            |total| {
                command_output("vm_stat", &[], Duration::from_millis(500))
                    .and_then(|stats| parse_vm_stat(&total, &stats))
            },
        )
    } else {
        None
    };
    let gpu = if no_gpu {
        None
    } else {
        command_output("rocm-smi", &["-a", "--json"], Duration::from_millis(800))
            .and_then(|s| parse_rocm(&s))
    };
    let note = if no_gpu {
        "GPU probes disabled by --no-gpu."
    } else if gpu.is_none() {
        "AMD GPU unavailable. rocm-smi is optional; inference monitoring still works."
    } else {
        "Local AMD card0 / rocm-smi"
    }
    .into();
    HardwareTelemetry { memory, gpu, note }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_timeout_is_bounded() {
        if cfg!(unix) {
            let start = Instant::now();
            assert!(command_output("sleep", &["2"], Duration::from_millis(50)).is_none());
            assert!(start.elapsed() < Duration::from_secs(1));
        }
    }
    #[test]
    fn large_output_does_not_deadlock() {
        if cfg!(unix) {
            let start = Instant::now();
            assert!(command_output("yes", &[], Duration::from_millis(100)).is_none());
            assert!(start.elapsed() < Duration::from_secs(1));
        }
    }
}
