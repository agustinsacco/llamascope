use llamascope::{
    config::Config,
    hardware::{parse_meminfo, parse_rocm, parse_vm_stat, GpuTelemetry},
    telemetry::{safe_text, ConnectionState, LlamaSlot, Poller, SlotTracker, Worker},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

const CURRENT: &str = include_str!("fixtures/slots-current.json");
const LEGACY: &str = include_str!("fixtures/slots-legacy.json");
const PROPS: &str = include_str!("fixtures/props.json");
const HEALTH: &str = r#"{"status":"ok"}"#;

fn raw(task: u64, count: u64, busy: bool) -> String {
    format!(
        r#"[{{"id":0,"id_task":{task},"is_processing":{busy},"n_ctx":8192,"next_token":{{"n_decoded":{count},"n_remain":-1}}}}]"#
    )
}

#[test]
fn current_object_and_unlimited_generation_are_supported() {
    let slots = SlotTracker::default()
        .parse(CURRENT, Instant::now())
        .unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].n_decoded, Some(136));
    assert_eq!(slots[0].n_remain, Some(-1));
    assert_eq!(slots[0].tps, None);
    assert_eq!(
        slots[0].kv_percent(),
        None,
        "missing prompt metrics must not become zero"
    );
}
#[test]
fn legacy_array_retains_prefill_and_context() {
    let slots = SlotTracker::default()
        .parse(LEGACY, Instant::now())
        .unwrap();
    assert_eq!(slots[0].phase(), "Prefill");
    assert_eq!(slots[0].prompt_percent(), Some(50.0));
    assert_eq!(slots[0].kv_percent(), Some(25.0));
}
#[test]
fn sampled_tps_uses_elapsed_time_and_task_identity() {
    let mut tracker = SlotTracker::default();
    let now = Instant::now();
    tracker.parse(&raw(1, 10, true), now).unwrap();
    let slots = tracker
        .parse(&raw(1, 30, true), now + Duration::from_millis(500))
        .unwrap();
    assert_eq!(slots[0].tps, Some(40.0));
    let slots = tracker
        .parse(&raw(2, 90, true), now + Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        slots[0].tps, None,
        "higher count on a NEW task is not a delta"
    );
}
#[test]
fn counter_reset_idle_and_missing_identity_reset_baselines() {
    let mut tracker = SlotTracker::default();
    let now = Instant::now();
    for (index, (count, busy, expected)) in [
        (20, true, None),
        (10, true, None),
        (10, true, Some(0.0)),
        (100, false, Some(0.0)),
        (200, true, None),
    ]
    .into_iter()
    .enumerate()
    {
        let slot = tracker
            .parse(
                &raw(1, count, busy),
                now + Duration::from_secs(index as u64),
            )
            .unwrap()
            .remove(0);
        assert_eq!(slot.tps, expected);
    }
    let no_task = r#"[{"id":0,"is_processing":true,"next_token":{"n_decoded":300}}]"#;
    assert_eq!(
        tracker
            .parse(no_task, now + Duration::from_secs(6))
            .unwrap()[0]
            .tps,
        None
    );
}
#[test]
fn removed_slots_do_not_leave_rate_baselines() {
    let now = Instant::now();
    let mut tracker = SlotTracker::default();
    tracker.parse(&raw(1, 1, true), now).unwrap();
    tracker.parse("[]", now).unwrap();
    assert_eq!(
        tracker
            .parse(&raw(1, 100, true), now + Duration::from_secs(1))
            .unwrap()[0]
            .tps,
        None
    );
}
#[test]
fn malformed_or_duplicate_slots_are_errors_not_empty_success() {
    for json in [
        "not JSON",
        "{}",
        r#"{"error":"denied"}"#,
        r#"[{"id":1}]"#,
        r#"[{"id":0,"is_processing":true,"next_token":{"n_decoded":-1}}]"#,
        r#"[{"id":0,"is_processing":false},{"id":0,"is_processing":false}]"#,
    ] {
        assert!(
            SlotTracker::default().parse(json, Instant::now()).is_err(),
            "accepted {json}"
        );
    }
    assert!(SlotTracker::default()
        .parse("[]", Instant::now())
        .unwrap()
        .is_empty());
}
#[test]
fn context_math_is_overflow_safe_and_bounded() {
    let slot = LlamaSlot {
        n_ctx: Some(2),
        n_prompt_tokens_processed: Some(u64::MAX),
        n_prompt_tokens: Some(1),
        n_decoded: Some(u64::MAX),
        ..Default::default()
    };
    assert_eq!(slot.kv_percent(), Some(100.0));
    assert_eq!(slot.prompt_percent(), Some(100.0));
    assert_eq!(
        LlamaSlot {
            n_ctx: Some(0),
            ..slot
        }
        .kv_percent(),
        None
    );
}
#[test]
fn slot_count_is_bounded_before_allocating_histories() {
    let slots: Vec<_> = (0..1025)
        .map(|id| serde_json::json!({"id": id, "is_processing": false}))
        .collect();
    let json = serde_json::to_string(&slots).unwrap();
    assert!(SlotTracker::default()
        .parse(&json, Instant::now())
        .unwrap_err()
        .contains("1024-slot"));
}

#[test]
fn invalid_library_auth_config_is_rejected_not_silently_dropped() {
    assert!(Poller::new(Config {
        api_key: Some("invalid\r\nheader".into()),
        ..Default::default()
    })
    .is_err());
}

#[test]
fn terminal_text_is_bounded_and_cannot_inject_control_codes() {
    let text = safe_text(&format!("\x1b[2J\n\t\u{202e}{}", "x".repeat(1000)));
    assert!(!text.chars().any(char::is_control));
    assert!(!text.contains('\u{202e}'));
    assert_eq!(text.chars().count(), 160);
}

/// Loopback-only fixtures; every expected request has a deadline and is asserted to be GET.
fn server(
    replies: Vec<(&'static str, u16, String)>,
    delay: Duration,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for (path, status, body) in replies {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    Instant::now() < deadline,
                    "expected {path} was never requested"
                );
                thread::sleep(Duration::from_millis(2));
            };
            // macOS inherits the listener's nonblocking flag on accepted sockets.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.ends_with(b"\r\n\r\n") {
                let size = stream.read(&mut buffer).unwrap();
                assert!(size > 0 && request.len() < 8192);
                request.extend_from_slice(&buffer[..size]);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(
                request.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
                "{request}"
            );
            requests.push(request);
            thread::sleep(delay);
            let redirect = if (300..400).contains(&status) {
                "Location: http://127.0.0.1:1/should-not-follow\r\n"
            } else {
                ""
            };
            let _ = write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{redirect}Connection: close\r\n\r\n{body}", body.len());
        }
        requests
    });
    (url, handle)
}
fn reply(path: &'static str, status: u16, text: &str) -> (&'static str, u16, String) {
    (path, status, text.into())
}
fn poller(url: String) -> Poller {
    Poller::new(Config {
        api_url: url,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn test_model_name_extraction_and_metadata_cache() {
    let (url, server) = server(
        vec![
            reply("/health", 200, HEALTH),
            reply("/slots", 200, CURRENT),
            reply("/props", 200, PROPS),
            reply("/health", 200, HEALTH),
            reply("/slots", 200, CURRENT),
        ],
        Duration::ZERO,
    );
    let mut poller = poller(url);
    let first = poller.sample();
    assert_eq!(first.connection, ConnectionState::Live);
    let props = first.llama_props.unwrap();
    assert_eq!(props.model_name, "test-model-q8_0.gguf");
    assert_eq!(
        props.queue_size, None,
        "stock /props does not report a queue"
    );
    assert_eq!(poller.sample().connection, ConnectionState::Live);
    assert_eq!(server.join().unwrap().len(), 5, "props should be cached");
}
#[test]
fn auth_failure_is_actionable_and_does_not_echo_secrets_or_bodies() {
    let (url, server) = server(
        vec![
            reply("/health", 200, HEALTH),
            reply("/slots", 401, "fixture-private-body"),
        ],
        Duration::ZERO,
    );
    let mut poller = Poller::new(Config {
        api_url: url,
        api_key: Some("fixture-test-token".into()),
        ..Default::default()
    })
    .unwrap();
    let data = poller.sample();
    assert_eq!(data.connection, ConnectionState::Degraded);
    assert!(data.detail.contains("LLAMA_API_KEY"));
    assert!(!data.detail.contains("fixture-"));
    assert!(data.llama_slots.is_none());
    assert!(server.join().unwrap().iter().all(|r| r
        .to_lowercase()
        .contains("authorization: bearer fixture-test-token")));
}
#[test]
fn model_loading_is_distinct_from_an_arbitrary_503() {
    for (body, expected) in [
        (
            r#"{"error":{"code":503,"message":"Loading model","type":"unavailable_error"}}"#,
            ConnectionState::Loading,
        ),
        (
            r#"{"error":{"message":"Unavailable"}}"#,
            ConnectionState::Degraded,
        ),
    ] {
        let (url, server) = server(vec![reply("/health", 503, body)], Duration::ZERO);
        assert_eq!(poller(url).sample().connection, expected);
        server.join().unwrap();
    }
}
#[test]
fn invalid_health_cannot_masquerade_as_live() {
    let (url, server) = server(vec![reply("/health", 200, "{}")], Duration::ZERO);
    assert_eq!(poller(url).sample().connection, ConnectionState::Degraded);
    server.join().unwrap();
}
#[test]
fn slots_disabled_and_invalid_json_are_not_empty_slots() {
    for (status, body, hint) in [
        (404, "{}", "--no-slots"),
        (200, "{broken", "Invalid JSON/schema"),
    ] {
        let (url, server) = server(
            vec![reply("/health", 200, HEALTH), reply("/slots", status, body)],
            Duration::ZERO,
        );
        let data = poller(url).sample();
        assert_eq!(data.connection, ConnectionState::Degraded);
        assert!(data.llama_slots.is_none());
        assert!(data.detail.contains(hint));
        server.join().unwrap();
    }
}
#[test]
fn partial_props_failure_preserves_live_slot_rates() {
    let (url, server) = server(
        vec![
            reply("/health", 200, HEALTH),
            reply("/slots", 200, &raw(1, 10, true)),
            reply("/props", 500, "{}"),
            reply("/health", 200, HEALTH),
            reply("/slots", 200, &raw(1, 20, true)),
            reply("/props", 200, "{}"),
        ],
        Duration::ZERO,
    );
    let mut poller = poller(url);
    let first = poller.sample();
    assert_eq!(first.connection, ConnectionState::Degraded);
    assert!(first.llama_slots.is_some());
    let second = poller.sample();
    assert_eq!(second.connection, ConnectionState::Degraded);
    assert!(second.llama_slots.unwrap()[0].tps.unwrap() > 0.0);
    server.join().unwrap();
}
#[test]
fn reconnect_does_not_average_tps_across_an_outage() {
    let (url, server) = server(
        vec![
            reply("/health", 200, HEALTH),
            reply("/slots", 200, &raw(1, 10, true)),
            reply("/props", 200, PROPS),
            reply("/health", 500, "{}"),
            reply("/health", 200, HEALTH),
            reply("/slots", 200, &raw(1, 100, true)),
            reply("/props", 200, PROPS),
        ],
        Duration::ZERO,
    );
    let mut poller = poller(url);
    assert_eq!(poller.sample().connection, ConnectionState::Live);
    assert_eq!(poller.sample().connection, ConnectionState::Degraded);
    let restored = poller.sample();
    assert_eq!(restored.connection, ConnectionState::Live);
    assert_eq!(restored.llama_slots.unwrap()[0].tps, None);
    server.join().unwrap();
}
#[test]
fn redirects_are_not_followed() {
    let (url, server) = server(vec![reply("/health", 302, "{}")], Duration::ZERO);
    assert!(poller(url).sample().detail.contains("Redirect refused"));
    server.join().unwrap();
}
#[test]
fn oversized_responses_are_rejected() {
    let (url, server) = server(
        vec![reply("/health", 200, &"x".repeat(2 * 1024 * 1024 + 1))],
        Duration::ZERO,
    );
    assert!(poller(url).sample().detail.contains("2 MiB"));
    server.join().unwrap();
}
#[test]
fn request_timeout_returns_an_offline_state() {
    let (url, server) = server(
        vec![reply("/health", 200, HEALTH)],
        Duration::from_millis(2200),
    );
    let start = Instant::now();
    let data = poller(url).sample();
    assert_eq!(data.connection, ConnectionState::Offline);
    assert!(data.detail.contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(3));
    server.join().unwrap();
}
#[test]
fn worker_retry_wakes_the_poll_and_drop_does_not_wait_for_interval() {
    let (worker, rx) = Worker::spawn(Duration::from_secs(30), || 42);
    assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 42);
    worker.retry();
    assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 42);
    let start = Instant::now();
    drop(worker);
    assert!(start.elapsed() < Duration::from_secs(1));
}
#[test]
fn stopping_with_a_pending_retry_does_not_start_another_sample() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (entered, started) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (worker, _rx) = Worker::spawn(Duration::from_secs(30), move || {
        observed.fetch_add(1, Ordering::SeqCst);
        let _ = entered.send(());
        let _ = blocked.recv_timeout(Duration::from_secs(2));
    });
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.retry();
    worker.stop();
    release.send(()).unwrap();
    drop(worker);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn worker_mailbox_is_bounded() {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    for n in 0..1000 {
        assert!(llamascope::telemetry::deliver(&tx, n));
    }
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![0]);
    drop(rx);
    assert!(!llamascope::telemetry::deliver(&tx, 1001));
}

#[test]
fn test_parse_json_f64() {
    let gpu = parse_rocm(r#"{"card0":{"GPU use (%)":"45.2","Temperature (Sensor edge) (C)":120.5,"Current Socket Graphics Package Power (W)":"invalid"}}"#).unwrap();
    assert_eq!(gpu.usage, Some(45.2));
    assert_eq!(gpu.temp, Some(120.5));
    assert_eq!(gpu.power, None, "unavailable readings are not zero watts");
    assert!(parse_rocm(r#"{"card0":{"GPU use (%)":"NaN"}}"#).is_none());
}
#[test]
fn test_gpu_telemetry_default() {
    let gpu = GpuTelemetry::default();
    assert!(gpu.usage.is_none() && gpu.temp.is_none() && gpu.vram.is_none());
}
#[test]
fn rocm_values_are_clamped_and_memory_is_gib() {
    let gpu = parse_rocm(r#"{"card0":{"GPU use (%)":"120","VRAM Total Memory (B)":"2147483648","VRAM Total Used Memory (B)":"1073741824"}}"#).unwrap();
    assert_eq!(gpu.usage, Some(100.0));
    let memory = gpu.vram.unwrap();
    assert_eq!(memory.used_gib, 1.0);
    assert_eq!(memory.percent(), 50.0);
}
#[test]
fn linux_memory_does_not_depend_on_a_gpu() {
    let memory = parse_meminfo("MemTotal: 8388608 kB\nMemAvailable: 2097152 kB\n").unwrap();
    assert_eq!(memory.total_gib, 8.0);
    assert_eq!(memory.used_gib, 6.0);
    assert!(parse_meminfo("MemTotal: 1 kB\nMemAvailable: 2 kB").is_none());
    assert!(parse_meminfo("MemTotal: NaN kB\nMemAvailable: 0 kB").is_none());
}
#[test]
fn macos_memory_uses_the_reported_page_size() {
    let memory = parse_vm_stat("1073741824", "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 16384.\nPages inactive: 16384.\nPages speculative: 0.\n").unwrap();
    assert_eq!(memory.total_gib, 1.0);
    assert_eq!(memory.used_gib, 0.5);
}
