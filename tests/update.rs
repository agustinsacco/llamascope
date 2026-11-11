use llamascope::update::{self, checksum_for, parse_version, Options, Outcome, Source};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
};

#[test]
fn versions_compare_numerically_and_ignore_suffixes() {
    assert_eq!(parse_version("v1.10.0"), Some((1, 10, 0)));
    assert!(parse_version("1.10.0") > parse_version("1.9.9"));
    assert_eq!(parse_version("2.0.0-rc.1+abc"), Some((2, 0, 0)));
    for bad in ["", "1.2", "1.2.3.4", "one.2.3", "v"] {
        assert_eq!(parse_version(bad), None, "{bad}");
    }
}
#[test]
fn checksums_match_exact_asset_names_only() {
    let hash = "a".repeat(64);
    let sums = format!(
        "{hash}  llamascope-x86_64-unknown-linux-musl\n{} *other\n",
        "b".repeat(64)
    );
    assert_eq!(
        checksum_for(&sums, "llamascope-x86_64-unknown-linux-musl"),
        Some(hash.as_str())
    );
    assert_eq!(checksum_for(&sums, "other"), Some("b".repeat(64).as_str()));
    assert_eq!(checksum_for(&sums, "llamascope"), None);
    assert_eq!(checksum_for("zz  other", "other"), None);
}
#[test]
fn options_accept_check_and_a_pinned_version() {
    let parse = |args: &[&str]| Options::parse(args.iter().map(|s| s.to_string()));
    let options = parse(&["--check", "--version", "v1.2.3"]).unwrap();
    assert!(options.check);
    assert_eq!(options.version.as_deref(), Some("1.2.3"));
    assert!(parse(&["--version"]).is_err());
    assert!(parse(&["--version", "latest"]).is_err());
    assert!(parse(&["--force"]).unwrap_err().contains("USAGE"));
}
#[test]
fn this_platform_has_a_release_target() {
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(update::target().is_some());
    }
}

/// A loopback stand-in for the GitHub API: one release, its binary, and SHA256SUMS.
struct FakeGitHub {
    url: String,
    requests: thread::JoinHandle<Vec<String>>,
}
fn github(tag: &str, binary: &'static [u8], sums: Option<String>, count: usize) -> FakeGitHub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let sums = sums.unwrap_or_else(|| format!("{:x}  llamascope-test\n", Sha256::digest(binary)));
    let release = format!(
        r#"{{"tag_name":"{tag}","assets":[{{"name":"llamascope-test","url":"{url}/asset/1"}},{{"name":"SHA256SUMS","url":"{url}/asset/2"}}]}}"#
    );
    let requests = thread::spawn(move || {
        let mut seen = Vec::new();
        for stream in listener.incoming().take(count) {
            let mut stream = stream.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.ends_with(b"\r\n\r\n") {
                let size = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..size]);
            }
            let request = String::from_utf8(request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap().to_owned();
            let body: Vec<u8> = match path.as_str() {
                "/asset/1" => binary.to_vec(),
                "/asset/2" => sums.clone().into_bytes(),
                _ => release.clone().into_bytes(),
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
            seen.push(request);
        }
        seen
    });
    FakeGitHub { url, requests }
}
fn installed(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llamascope-update-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("llamascope");
    fs::write(&path, "old").unwrap();
    path
}
fn source<'a>(api: &'a str, executable: PathBuf) -> Source<'a> {
    Source {
        api,
        repository: "o/r",
        token: Some("secret-token".into()),
        target: "test",
        current: "1.0.0",
        executable,
    }
}
const NEW: &[u8] = b"#!/bin/sh\necho 'LlamaScope 1.1.0'\n";

#[cfg(unix)]
#[test]
fn update_verifies_test_runs_and_replaces_the_binary() {
    let fake = github("v1.1.0", NEW, None, 3);
    let path = installed("ok");
    let outcome = update::run(&Options::default(), &source(&fake.url, path.clone())).unwrap();
    assert_eq!(outcome, Outcome::Updated("1.1.0".into()));
    assert_eq!(fs::read(&path).unwrap(), NEW);
    let requests = fake.requests.join().unwrap();
    assert!(requests[0].starts_with("GET /repos/o/r/releases/latest "));
    assert!(requests
        .iter()
        .all(|r| r.contains("authorization: Bearer secret-token")));
    assert_eq!(
        fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1,
        "no staged leftovers"
    );
}
#[test]
fn check_and_up_to_date_change_nothing() {
    let fake = github("v1.1.0", NEW, None, 1);
    let path = installed("check");
    let check = Options {
        check: true,
        version: None,
    };
    assert_eq!(
        update::run(&check, &source(&fake.url, path.clone())).unwrap(),
        Outcome::Available("1.1.0".into())
    );
    fake.requests.join().unwrap();
    let fake = github("v1.0.0", NEW, None, 1);
    assert_eq!(
        update::run(&Options::default(), &source(&fake.url, path.clone())).unwrap(),
        Outcome::UpToDate("1.0.0".into())
    );
    assert_eq!(fs::read(&path).unwrap(), b"old");
}
#[test]
fn a_checksum_mismatch_discards_the_download() {
    let fake = github(
        "v1.1.0",
        NEW,
        Some(format!("{}  llamascope-test\n", "0".repeat(64))),
        3,
    );
    let path = installed("mismatch");
    let error = update::run(&Options::default(), &source(&fake.url, path.clone())).unwrap_err();
    assert!(error.contains("Checksum mismatch"));
    assert_eq!(fs::read(&path).unwrap(), b"old");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn a_build_reporting_the_wrong_version_is_not_installed() {
    let fake = github("v1.1.0", b"#!/bin/sh\necho 'LlamaScope 9.9.9'\n", None, 3);
    let path = installed("wrong");
    let error = update::run(&Options::default(), &source(&fake.url, path.clone())).unwrap_err();
    assert!(error.contains("expected version"));
    assert_eq!(fs::read(&path).unwrap(), b"old");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}
#[test]
fn a_pinned_version_requests_that_tag_and_may_downgrade() {
    let fake = github("v0.9.0", b"", None, 1);
    let options = Options {
        check: true,
        version: Some("0.9.0".into()),
    };
    let outcome = update::run(&options, &source(&fake.url, installed("pinned"))).unwrap();
    assert_eq!(outcome, Outcome::Available("0.9.0".into()));
    assert!(fake.requests.join().unwrap()[0].starts_with("GET /repos/o/r/releases/tags/v0.9.0 "));
}
