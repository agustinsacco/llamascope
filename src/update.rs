//! `llamascope update`: replace this binary with a verified GitHub release build.
//!
//! The release must contain `llamascope-<target>` and a `SHA256SUMS` file. The download
//! is checksum-verified, test-run (`--version`), then atomically renamed over the
//! running executable. Nothing else on the system is changed.
use reqwest::{blocking::Client, header};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

pub const REPOSITORY: &str = "agustinsacco/llamascope";
pub const API: &str = "https://api.github.com";
pub const USAGE: &str = "USAGE
  llamascope update [--check] [--version X.Y.Z]

  --check          Report whether a newer release exists; change nothing
  --version X.Y.Z  Install that release (also allows downgrades)

Private repository? Set GH_TOKEN or GITHUB_TOKEN, or sign in with gh.";

/// Release binaries are statically linked on Linux, so any Linux host uses musl builds.
pub fn target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => return None,
    })
}

/// `1.2.3` or `v1.2.3`; pre-release or build suffixes are ignored for ordering.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|n| n.parse().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// The checksum for `asset` in `sha256sum` output (`<hex>  <name>` or `<hex> *<name>`).
pub fn checksum_for<'a>(sums: &'a str, asset: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        (name == asset && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then_some(hash)
    })
}

#[derive(Debug, Default)]
pub struct Options {
    pub check: bool,
    pub version: Option<String>,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--check" => options.check = true,
                "--version" => {
                    let version = args.next().ok_or("--version requires X.Y.Z.")?;
                    parse_version(&version).ok_or("--version must look like 1.2.3.")?;
                    options.version = Some(version.trim_start_matches('v').to_owned());
                }
                _ => return Err(format!("Unknown update option.\n\n{USAGE}")),
            }
        }
        Ok(options)
    }
}

/// Where to look and what to replace. `main` fills this from the real environment.
pub struct Source<'a> {
    pub api: &'a str,
    pub repository: &'a str,
    pub token: Option<String>,
    pub target: &'a str,
    pub current: &'a str,
    pub executable: PathBuf,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    url: String,
}

const LIMIT: u64 = 128 * 1024 * 1024;

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("llamascope/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| "Could not initialize the HTTP client.".into())
}

fn get(client: &Client, source: &Source, url: &str, accept: &str) -> Result<Vec<u8>, String> {
    let mut request = client
        .get(url)
        .header(header::ACCEPT, accept)
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = &source.token {
        // reqwest drops Authorization when a redirect leaves the API host.
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|_| "Cannot reach GitHub. Check the network and retry.".to_owned())?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "Release not found at {}. If the repository is private, set GH_TOKEN or sign in with gh.",
            source.repository
        ));
    }
    if !status.is_success() {
        return Err(format!("GitHub returned HTTP {}.", status.as_u16()));
    }
    let mut body = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut body)
        .map_err(|_| "The download was interrupted. Retry.".to_owned())?;
    if body.len() as u64 > LIMIT {
        return Err("The download is larger than expected; refusing it.".into());
    }
    Ok(body)
}

/// What happened, for the caller to print.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    UpToDate(String),
    Available(String),
    Updated(String),
}

pub fn run(options: &Options, source: &Source) -> Result<Outcome, String> {
    let client = client()?;
    let base = format!("{}/repos/{}/releases", source.api, source.repository);
    let url = match &options.version {
        Some(version) => format!("{base}/tags/v{version}"),
        None => format!("{base}/latest"),
    };
    let release: Release =
        serde_json::from_slice(&get(&client, source, &url, "application/vnd.github+json")?)
            .map_err(|_| "Unexpected release metadata from GitHub.".to_owned())?;
    let latest = release.tag_name.trim_start_matches('v').to_owned();
    let newer = parse_version(&latest).ok_or("The release tag is not a version.")?
        > parse_version(source.current).ok_or("The running version is not a version.")?;
    if options.version.is_none() && !newer {
        return Ok(Outcome::UpToDate(latest));
    }
    if options.check {
        return Ok(Outcome::Available(latest));
    }
    let name = format!("llamascope-{}", source.target);
    let asset = |wanted: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name == wanted)
            .ok_or_else(|| format!("Release {latest} has no {wanted} asset."))
    };
    let binary = get(
        &client,
        source,
        &asset(&name)?.url,
        "application/octet-stream",
    )?;
    let sums = get(
        &client,
        source,
        &asset("SHA256SUMS")?.url,
        "application/octet-stream",
    )?;
    let sums = String::from_utf8(sums).map_err(|_| "SHA256SUMS is not text.".to_owned())?;
    let expected = checksum_for(&sums, &name).ok_or("SHA256SUMS does not list this build.")?;
    let actual = format!("{:x}", Sha256::digest(&binary));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("Checksum mismatch; the download was discarded.".into());
    }
    install(&binary, &source.executable, &latest)?;
    Ok(Outcome::Updated(latest))
}

/// Stage next to the target (same filesystem), prove it runs, then rename over it.
fn install(binary: &[u8], executable: &Path, version: &str) -> Result<(), String> {
    let dir = executable
        .parent()
        .ok_or("Cannot locate the installed binary.")?;
    let staged = dir.join(format!(".llamascope-update-{}", std::process::id()));
    let result = (|| {
        fs::write(&staged, binary).map_err(|e| {
            format!(
                "Cannot write to {} ({}). Re-run the installer or update with permission to that directory.",
                dir.display(),
                e.kind()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
                .map_err(|_| "Cannot mark the download executable.".to_owned())?;
        }
        let output = Command::new(&staged)
            .arg("--version")
            .output()
            .map_err(|_| "The downloaded build does not run on this machine.".to_owned())?;
        let reported = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || reported.trim() != format!("LlamaScope {version}") {
            return Err("The downloaded build did not report the expected version.".into());
        }
        fs::rename(&staged, executable)
            .map_err(|e| format!("Cannot replace {} ({}).", executable.display(), e.kind()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}
