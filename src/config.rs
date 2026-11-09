//! Validated startup configuration. Secrets are never included in diagnostics.
use crate::theme::Theme;
use std::time::Duration;

pub const HELP: &str = "LlamaScope — a calm observatory for local inference

USAGE
  llamascope [OPTIONS]

OPTIONS
  --url URL        llama-server base URL (default http://127.0.0.1:8086)
  --interval MS    Poll interval, 250–10000 milliseconds (default 500)
  --theme THEME    mocha (dark) or latte (light)
  --demo           Explore deterministic simulated telemetry; no network or probes
  --no-gpu         Disable GPU probes; keep local host memory
  -h, --help       Show this guide without opening the terminal UI
  -V, --version    Print version

ENVIRONMENT
  LLAMA_API_URL    Base URL; --url takes precedence
  LLAMA_API_KEY    Optional bearer token; never passed on the command line
  LLAMASCOPE_THEME  mocha or latte; --theme takes precedence

KEYS
  q / Ctrl-C  quit      Tab / arrows / j,k  select slot
  Space       freeze   t  theme   ?  help   c  clear events   r  retry

Start here: llamascope --demo
Documentation: https://github.com/agustinsacco/llamascope#readme";

#[derive(Clone)]
pub struct Config {
    pub api_url: String,
    pub api_key: Option<String>,
    pub interval: Duration,
    pub theme: Theme,
    pub demo: bool,
    pub no_gpu: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:8086".into(),
            api_key: None,
            interval: Duration::from_millis(500),
            theme: Theme::Mocha,
            demo: false,
            no_gpu: false,
        }
    }
}

impl Config {
    pub fn parse(
        args: impl IntoIterator<Item = String>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        // An exported-but-empty variable means "unset" for every setting, consistently.
        let var = |name: &str| env(name).filter(|value| !value.trim().is_empty());
        let mut config = Self::default();
        if let Some(url) = var("LLAMA_API_URL") {
            config.api_url = url;
        }
        config.api_key = var("LLAMA_API_KEY");
        let mut theme = var("LLAMASCOPE_THEME").unwrap_or_else(|| "mocha".into());
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => config.demo = true,
                "--no-gpu" => config.no_gpu = true,
                "--url" | "--theme" | "--interval" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{arg} requires a value. See --help."))?;
                    match arg.as_str() {
                        "--url" => config.api_url = value,
                        "--theme" => theme = value,
                        _ => {
                            let ms: u64 = value
                                .parse()
                                .map_err(|_| "--interval must be an integer from 250 to 10000.")?;
                            if !(250..=10000).contains(&ms) {
                                return Err("--interval must be between 250 and 10000 ms.".into());
                            }
                            config.interval = Duration::from_millis(ms);
                        }
                    }
                }
                _ => {
                    return Err(
                        "Unknown option. Run llamascope --help for supported options.".into(),
                    )
                }
            }
        }
        config.theme = match theme.as_str() {
            "mocha" => Theme::Mocha,
            "latte" => Theme::Latte,
            _ => return Err("Theme must be mocha or latte.".into()),
        };
        let url = reqwest::Url::parse(&config.api_url)
            .map_err(|_| "Invalid server URL. Use http://host:port or https://host:port.")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("Server URL must use HTTP or HTTPS and include a host.".into());
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("URL credentials, query strings, and fragments are not supported. Use LLAMA_API_KEY for authentication.".into());
        }
        config.api_url = url.to_string().trim_end_matches('/').to_string();
        if let Some(key) = &config.api_key {
            if reqwest::header::HeaderValue::from_str(&format!("Bearer {key}")).is_err() {
                return Err("LLAMA_API_KEY is not a valid HTTP header value.".into());
            }
        }
        Ok(config)
    }
}
