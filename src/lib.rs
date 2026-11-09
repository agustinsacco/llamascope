//! LlamaScope: read-only inference telemetry and a terminal-native observatory.
/// The release version. CI sets `LLAMASCOPE_VERSION` (`<major>.<minor>.<commits>`);
/// local builds report `Cargo.toml`'s version, which is always older than a release.
pub const VERSION: &str = match option_env!("LLAMASCOPE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

pub mod app;
pub mod config;
pub mod demo;
pub mod hardware;
pub mod telemetry;
pub mod theme;
pub mod ui;
pub mod update;
