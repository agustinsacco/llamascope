use llamascope::{
    config::Config,
    telemetry::{safe_text, ConnectionState, LlamaSlot, Phase, Poller, SlotTracker, Worker},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

const LIVE: &str = include_str!("fixtures/slots-live.json");
const OBJECT: &str = include_str!("fixtures/slots-object.json");
const PROPS: &str = include_str!("fixtures/props.json");
const HEALTH: &str = r#"{"status":"ok"}"#;

fn raw(task: u64, count: u64, busy: bool) -> String {
    format!(
        r#"[{{"id":0,"id_task":{task},"is_processing":{busy},"n_ctx":8192,"next_token":{{"n_decoded":{count},"n_remain":-1}}}}]"#
    )
}

#[test]
fn live_array_shape_reports_phase_prompt_and_context() {
    let slots = SlotTracker::default().parse(LIVE, Instant::now()).unwrap();
    let [prefill, cached, idle] = &slots[..] else {
        panic!("three slots expected");
    };
    // n_prompt_tokens runs ahead of processed during prefill; that is not a percentage.
    assert_eq!(prefill.phase(), Phase::Prefill);
    assert_eq!(prefill.prompt_tokens(), Some(69632));
    assert_eq!(prefill.context_percent(), Some(71680.0 / 131072.0 * 100.0));
    // Generated tokens also grow n_prompt_tokens; generation is still Decoding.
    assert_eq!(cached.phase(), Phase::Decoding);
    assert_eq!(cached.n_remain, Some(-1));
    assert_eq!(
        cached.prompt_tokens(),
        Some(9088),
        "cache hits count as prompt"
    );
    assert_eq!(cached.context_percent(), Some(9408.0 / 32768.0 * 100.0));
    assert_eq!(idle.phase(), Phase::Idle);
    assert_eq!(idle.task_id.as_deref(), Some("190"));
}
#[test]
fn object_shape_without_prompt_counters_stays_unknown() {
    let slots = SlotTracker::default()
        .parse(OBJECT, Instant::now())
        .unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].phase(), Phase::Decoding);
    assert_eq!(slots[0].n_decoded, Some(136));
    assert_eq!(slots[0].n_remain, Some(-1));
    assert_eq!(slots[0].tps, None);
    assert_eq!(slots[0].prompt_tokens(), None, "missing is not zero");
    assert_eq!(slots[0].context_percent(), None, "missing is not zero");
}
#[test]
fn phase_follows_busy_and_generated_counters() {
    let slot = |busy, n_decoded| LlamaSlot {
        busy,
        n_decoded,
        ..Default::default()
    };
    assert_eq!(slot(false, Some(9)).phase(), Phase::Idle);
    assert_eq!(slot(true, Some(0)).phase(), Phase::Prefill);
    assert_eq!(slot(true, Some(1)).phase(), Phase::Decoding);
    assert_eq!(slot(true, None).phase(), Phase::Working);
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
        n_prompt_tokens: Some(u64::MAX),
        n_prompt_tokens_processed: Some(u64::MAX),
        n_prompt_tokens_cache: Some(u64::MAX),
        ..Default::default()
    };
    assert_eq!(slot.context_percent(), Some(100.0));
    assert_eq!(slot.prompt_tokens(), Some(u64::MAX));
    assert_eq!(
        LlamaSlot {
            n_ctx: Some(0),
            ..slot
        }
        .context_percent(),
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
    let marks = safe_text("a\u{200e}b\u{200f}c\u{061c}d\u{2028}e\u{2066}f");
    assert_eq!(
        marks, "abcdef",
        "direction marks and line separators are stripped"
    );
}
#[test]
fn minimal_props_with_only_a_model_path_is_live_not_degraded() {
    let (url, server) = server(
        vec![
            reply("/health", 200, HEALTH),
            reply("/slots", 200, LIVE),
            reply("/props", 200, r#"{"model_path":"/m/only.gguf"}"#),
        ],
        Duration::ZERO,
    );
    let data = poller(url).sample();
    assert_eq!(data.connection, ConnectionState::Live);
    assert_eq!(data.llama_props.unwrap().model_name, "only.gguf");
    server.join().unwrap();
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
            reply("/slots", 200, LIVE),
            reply("/props", 200, PROPS),
            reply("/health", 200, HEALTH),
            reply("/slots", 200, LIVE),
        ],
        Duration::ZERO,
    );
    let mut poller = poller(url);
    let first = poller.sample();
    assert_eq!(first.connection, ConnectionState::Live);
    let props = first.llama_props.unwrap();
    assert_eq!(props.model_name, "test-model-q8_0.gguf");
    assert_eq!(props.build_info.as_deref(), Some("b12345-test"));
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
            reply("/props", 200, "[]"),
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
fn reverse_proxy_path_prefixes_every_endpoint() {
    let (url, server) = server(
        vec![
            reply("/llama/health", 200, HEALTH),
            reply("/llama/slots", 200, LIVE),
            reply("/llama/props", 200, PROPS),
        ],
        Duration::ZERO,
    );
    let data = poller(format!("{url}/llama")).sample();
    assert_eq!(data.connection, ConnectionState::Live);
    server.join().unwrap();
}
