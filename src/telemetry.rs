//! Read-only llama-server protocol adapter and bounded polling worker.
use crate::config::Config;
use reqwest::{blocking::Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionState {
    #[default]
    Connecting,
    Loading,
    Live,
    Degraded,
    Offline,
}

#[derive(Clone, Debug, Default)]
pub struct LlamaSlot {
    pub id: u32,
    pub busy: bool,
    pub task_id: Option<String>,
    pub n_ctx: Option<u64>,
    pub n_prompt_tokens: Option<u64>,
    pub n_prompt_tokens_processed: Option<u64>,
    pub n_decoded: Option<u64>,
    pub n_remain: Option<i64>,
    pub tps: Option<f64>,
}

impl LlamaSlot {
    pub fn phase(&self) -> &'static str {
        if !self.busy {
            "Idle"
        } else if matches!((self.n_prompt_tokens_processed, self.n_prompt_tokens), (Some(a), Some(b)) if a < b)
        {
            "Prefill"
        } else if self.n_decoded.is_some_and(|n| n > 0) {
            "Decoding"
        } else {
            "Working"
        }
    }
    /// A token-count estimate, not measured physical KV memory allocation.
    pub fn kv_percent(&self) -> Option<f64> {
        let ctx = self.n_ctx.filter(|n| *n > 0)?;
        let used = self
            .n_prompt_tokens_processed?
            .saturating_add(self.n_decoded?);
        Some((used as f64 / ctx as f64 * 100.0).clamp(0.0, 100.0))
    }
    pub fn prompt_percent(&self) -> Option<f64> {
        let total = self.n_prompt_tokens.filter(|n| *n > 0)?;
        Some((self.n_prompt_tokens_processed? as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LlamaProps {
    #[serde(default, rename = "model_path")]
    pub model_name: String,
    pub total_slots: u32,
    pub queue_size: Option<u32>,
    pub build_info: Option<String>,
    #[serde(default)]
    pub is_sleeping: bool,
}

#[derive(Clone, Debug)]
pub struct TelemetryData {
    pub connection: ConnectionState,
    pub detail: String,
    pub llama_slots: Option<Vec<LlamaSlot>>,
    pub llama_props: Option<LlamaProps>,
    pub sampled_at: Instant,
    pub latency: Duration,
}

impl TelemetryData {
    pub fn status(connection: ConnectionState, detail: impl Into<String>) -> Self {
        Self {
            connection,
            detail: detail.into(),
            llama_slots: None,
            llama_props: None,
            sampled_at: Instant::now(),
            latency: Duration::ZERO,
        }
    }
}

#[derive(Deserialize)]
struct RawToken {
    n_decoded: Option<u64>,
    n_remain: Option<i64>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum NextToken {
    Object(RawToken),
    Legacy(Vec<RawToken>),
}
#[derive(Deserialize)]
struct RawSlot {
    id: u32,
    is_processing: bool,
    id_task: Option<Value>,
    n_ctx: Option<u64>,
    n_prompt_tokens: Option<u64>,
    n_prompt_tokens_processed: Option<u64>,
    next_token: Option<NextToken>,
}

struct Counter {
    task: Option<String>,
    decoded: u64,
    at: Instant,
}
#[derive(Default)]
pub struct SlotTracker {
    counts: HashMap<u32, Counter>,
}

impl SlotTracker {
    pub fn parse(&mut self, json: &str, now: Instant) -> Result<Vec<LlamaSlot>, String> {
        let raw: Vec<RawSlot> = serde_json::from_str(json)
            .map_err(|_| "Invalid JSON/schema from /slots; check your llama-server version.")?;
        if raw.len() > 1024 {
            return Err("/slots exceeded the 1024-slot safety limit.".into());
        }
        let mut ids = std::collections::HashSet::new();
        if raw.iter().any(|s| !ids.insert(s.id)) {
            return Err("Duplicate slot IDs in /slots response.".into());
        }
        let mut slots = Vec::with_capacity(raw.len());
        for s in raw {
            let token = match s.next_token {
                Some(NextToken::Object(t)) => Some(t),
                Some(NextToken::Legacy(v)) => v.into_iter().next(),
                None => None,
            };
            let decoded = token.as_ref().and_then(|t| t.n_decoded);
            let task = match s.id_task {
                Some(Value::Number(n)) => Some(n.to_string()),
                Some(Value::String(s)) => Some(s),
                _ => None,
            };
            let tps = if s.is_processing {
                self.counts.get(&s.id).and_then(|prev| {
                    // Missing identity cannot prove continuity across slot reuse.
                    let elapsed = now.saturating_duration_since(prev.at).as_secs_f64();
                    let count = decoded?;
                    (task.is_some() && prev.task == task && count >= prev.decoded && elapsed > 0.0)
                        .then(|| (count - prev.decoded) as f64 / elapsed)
                })
            } else {
                Some(0.0)
            };
            if let Some(count) = decoded.filter(|_| s.is_processing) {
                self.counts.insert(
                    s.id,
                    Counter {
                        task: task.clone(),
                        decoded: count,
                        at: now,
                    },
                );
            } else {
                self.counts.remove(&s.id);
            }
            slots.push(LlamaSlot {
                id: s.id,
                busy: s.is_processing,
                task_id: task,
                n_ctx: s.n_ctx,
                n_prompt_tokens: s.n_prompt_tokens,
                n_prompt_tokens_processed: s.n_prompt_tokens_processed,
                n_decoded: decoded,
                n_remain: token.and_then(|t| t.n_remain),
                tps,
            });
        }
        self.counts.retain(|id, _| ids.contains(id));
        slots.sort_by_key(|s| s.id);
        Ok(slots)
    }
}

/// Prevent server-provided text from injecting terminal controls or unbounded labels.
pub fn safe_text(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(160)
        .collect()
}

struct EndpointError {
    state: ConnectionState,
    message: String,
}

pub struct Poller {
    client: Client,
    config: Config,
    tracker: SlotTracker,
    props: Option<LlamaProps>,
    props_at: Option<Instant>,
}

impl Poller {
    pub fn new(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = &config.api_key {
            let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))?;
            value.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_secs(1))
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .user_agent(concat!("llamascope/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            config,
            tracker: SlotTracker::default(),
            props: None,
            props_at: None,
        })
    }

    fn get(&self, endpoint: &str) -> Result<String, EndpointError> {
        let response = self.client.get(format!("{}{endpoint}", self.config.api_url)).send().map_err(|e| EndpointError {
            state: ConnectionState::Offline,
            message: if e.is_timeout() { format!("{endpoint} timed out after 2s. Retrying automatically; r retries sooner.") }
                else { format!("Cannot reach {endpoint}. Check the server URL, network, and TLS certificate; r retries.") },
        })?;
        let status = response.status();
        // Do not retain prompts, response bodies, or credential-bearing errors in diagnostics.
        const LIMIT: u64 = 2 * 1024 * 1024;
        let mut body = String::new();
        response
            .take(LIMIT + 1)
            .read_to_string(&mut body)
            .map_err(|_| EndpointError {
                state: ConnectionState::Degraded,
                message: format!("Could not read {endpoint}; response must be UTF-8 and complete."),
            })?;
        if body.len() as u64 > LIMIT {
            return Err(EndpointError {
                state: ConnectionState::Degraded,
                message: format!("{endpoint} exceeded the 2 MiB response limit."),
            });
        }
        if status == StatusCode::SERVICE_UNAVAILABLE
            && endpoint == "/health"
            && serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .is_some_and(|s| s.eq_ignore_ascii_case("loading model"))
        {
            return Err(EndpointError {
                state: ConnectionState::Loading,
                message: "Model is loading. Waiting for llama-server; no action needed.".into(),
            });
        }
        if !status.is_success() {
            let hint = match status.as_u16() {
                401 | 403 => "Access denied. Set LLAMA_API_KEY and check server permissions.",
                404 | 501 if endpoint == "/slots" => {
                    "Slot monitoring unavailable. Remove --no-slots (older builds: enable --slots)."
                }
                404 => "Endpoint not found. Check the base URL and llama-server version.",
                300..=399 => "Redirect refused. Use the final server URL directly.",
                _ => "Server returned an error. Retrying automatically.",
            };
            return Err(EndpointError {
                state: ConnectionState::Degraded,
                message: format!("{endpoint}: HTTP {}. {hint}", status.as_u16()),
            });
        }
        Ok(body)
    }

    pub fn sample(&mut self) -> TelemetryData {
        let started = Instant::now();
        let mut data = TelemetryData::status(
            ConnectionState::Live,
            "Telemetry is live. Read-only monitoring.",
        );
        let result = self.sample_inner(&mut data);
        if let Err(error) = result {
            if data.llama_slots.is_none() {
                self.tracker.counts.clear();
            }
            self.props_at = None;
            data.connection = error.state;
            data.detail = error.message;
        }
        data.latency = started.elapsed();
        data
    }

    fn sample_inner(&mut self, data: &mut TelemetryData) -> Result<(), EndpointError> {
        let health = self.get("/health")?;
        if serde_json::from_str::<Value>(&health)
            .ok()
            .and_then(|v| v.get("status").and_then(Value::as_str).map(str::to_owned))
            .as_deref()
            != Some("ok")
        {
            return Err(EndpointError {
                state: ConnectionState::Degraded,
                message: "Invalid /health response; expected status: ok.".into(),
            });
        }
        let slots = self.get("/slots")?;
        data.sampled_at = Instant::now();
        data.llama_slots = Some(self.tracker.parse(&slots, data.sampled_at).map_err(
            |message| EndpointError {
                state: ConnectionState::Degraded,
                message,
            },
        )?);
        if self
            .props_at
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(30))
        {
            let props_json = self.get("/props")?;
            let mut props: LlamaProps =
                serde_json::from_str(&props_json).map_err(|_| EndpointError {
                    state: ConnectionState::Degraded,
                    message: "Invalid JSON/schema from /props; slot readings remain available."
                        .into(),
                })?;
            props.model_name = safe_text(
                props
                    .model_name
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("Unknown model"),
            );
            if props.model_name.is_empty() {
                props.model_name = "Unknown model".into();
            }
            props.build_info = props.build_info.map(|s| safe_text(&s));
            self.props = Some(props);
            self.props_at = Some(Instant::now());
        }
        data.llama_props = self.props.clone();
        Ok(())
    }
}

/// A capacity-one mailbox prevents UI stalls from growing telemetry memory.
pub fn deliver<T>(tx: &SyncSender<T>, value: T) -> bool {
    !matches!(tx.try_send(value), Err(TrySendError::Disconnected(_)))
}

pub struct Worker {
    stopping: Arc<AtomicBool>,
    control: SyncSender<()>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn spawn<T: Send + 'static>(
        interval: Duration,
        mut sample: impl FnMut() -> T + Send + 'static,
    ) -> (Self, Receiver<T>) {
        let (tx, rx) = mpsc::sync_channel(1);
        let (control, commands) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stopping);
        let handle = thread::spawn(move || loop {
            if stopped.load(Ordering::Acquire) {
                break;
            }
            let started = Instant::now();
            if !deliver(&tx, sample()) {
                break;
            }
            match commands.recv_timeout(interval.saturating_sub(started.elapsed())) {
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                _ => continue,
            }
        });
        (
            Self {
                stopping,
                control,
                handle: Some(handle),
            },
            rx,
        )
    }
    pub fn retry(&self) {
        let _ = self.control.try_send(());
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let _ = self.control.try_send(());
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
        // Close control before joining. Callers restore the terminal before dropping workers.
        let (replacement, _) = mpsc::sync_channel(1);
        drop(std::mem::replace(&mut self.control, replacement));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
