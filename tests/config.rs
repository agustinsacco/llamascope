use llamascope::{config::Config, theme::Theme};
use std::{process::Command, time::Duration};

fn parse(args: &[&str]) -> Result<Config, String> {
    Config::parse(args.iter().map(|s| s.to_string()), |_| None)
}
#[test]
fn defaults_and_cli_overrides_are_predictable() {
    let config = parse(&[]).unwrap();
    assert_eq!(config.api_url, "http://127.0.0.1:8086");
    assert_eq!(config.interval, Duration::from_millis(500));
    let config = Config::parse(
        [
            "--url",
            "https://example.com/proxy/",
            "--theme",
            "latte",
            "--interval",
            "750",
            "--no-gpu",
            "--demo",
        ]
        .map(str::to_string),
        |name| match name {
            "LLAMA_API_URL" => Some("http://old.invalid".into()),
            "LLAMASCOPE_THEME" => Some("invalid-env-overridden".into()),
            _ => None,
        },
    )
    .unwrap();
    assert_eq!(config.api_url, "https://example.com/proxy");
    assert_eq!(config.theme, Theme::Latte);
    assert_eq!(config.interval, Duration::from_millis(750));
    assert!(config.no_gpu && config.demo);
}
#[test]
fn bad_urls_are_rejected_without_echoing_credentials() {
    for url in [
        "garbage",
        "file:///tmp/server",
        "http://user:secret@example.com",
        "http://example.com?key=secret",
        "http://example.com#secret",
    ] {
        let error = parse(&["--url", url]).err().unwrap();
        assert!(!error.contains("secret"));
    }
}
#[test]
fn invalid_options_fail_before_terminal_setup() {
    for args in [
        &["--interval", "0"][..],
        &["--interval", "249"],
        &["--interval", "10001"],
        &["--interval", "NaN"],
        &["--url"],
        &["--theme", "unknown"],
        &["--unknown"],
    ] {
        assert!(parse(args).is_err());
    }
}
#[test]
fn api_key_header_injection_is_rejected_without_echoing_it() {
    let error = Config::parse(Vec::new(), |name| {
        (name == "LLAMA_API_KEY").then(|| "secret\r\nHeader: injected".into())
    })
    .err()
    .unwrap();
    assert!(!error.contains("secret"));
    assert!(error.contains("header"));
}
#[test]
fn help_and_version_work_without_a_tty() {
    for arg in ["--help", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_llamascope"))
            .arg(arg)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("LlamaScope"));
        assert!(!output.stdout.contains(&0x1b));
    }
}
#[test]
fn noninteractive_startup_exits_cleanly_without_escape_sequences() {
    let output = Command::new(env!("CARGO_BIN_EXE_llamascope"))
        .arg("--demo")
        .env_remove("LLAMA_API_URL")
        .env_remove("LLAMA_API_KEY")
        .env_remove("LLAMASCOPE_THEME")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    assert!(!output.stdout.contains(&0x1b));
}
