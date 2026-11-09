#!/usr/bin/env python3
"""Check launcher/installer contracts with a fake Cargo; never modifies the real HOME."""
import hashlib
import http.server
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="llamascope-scripts-") as directory:
    home = Path(directory)
    tools = home / "tools"
    tools.mkdir()
    cargo = tools / "cargo"
    cargo.write_text(f"#!{sys.executable}\nimport json, os, sys\nfrom pathlib import Path\nPath(os.environ['CAPTURE']).write_text(json.dumps(sys.argv[1:]))\nsys.exit(int(os.environ.get('FAKE_EXIT', '0')))\n")
    cargo.chmod(0o755)
    capture = home / "args.json"
    env = {**os.environ, "HOME": str(home), "PATH": f"{tools}:/usr/bin:/bin", "CAPTURE": str(capture), "PREFIX": str(home / "custom prefix")}
    for shell_file in [".zshrc", ".bashrc"]:
        (home / shell_file).write_text("# user configuration; do not change\n")

    subprocess.run(["bash", str(ROOT / "start.sh"), "--url", "https://example.com/a path", "--demo"], cwd=home, env=env, check=True)
    args = json.loads(capture.read_text())
    assert args == ["run", "--locked", "--release", "--manifest-path", str(ROOT / "Cargo.toml"), "--", "--url", "https://example.com/a path", "--demo"]
    subprocess.run(["sh", str(ROOT / "install.sh"), "--source"], cwd=home, env=env, check=True, capture_output=True)
    args = json.loads(capture.read_text())
    assert args == ["install", "--locked", "--path", str(ROOT), "--root", str(home / "custom prefix"), "--bin", "llamascope", "--force"]
    for shell_file in [".zshrc", ".bashrc"]:
        assert (home / shell_file).read_text() == "# user configuration; do not change\n"
    for script in [["bash", "start.sh"], ["sh", "install.sh", "--source"]]:
        result = subprocess.run([script[0], str(ROOT / script[1]), *script[2:]], cwd=home, env={**env, "FAKE_EXIT": "17"}, capture_output=True)
        assert result.returncode != 0, f"{script} swallowed a Cargo failure"
        assert b"Installed. Try" not in result.stdout

    # Release install, piped into sh as `curl ... | sh` does, against a loopback GitHub API.
    system, machine = platform.system(), platform.machine()
    target = {("Linux", "x86_64"): "x86_64-unknown-linux-musl", ("Linux", "aarch64"): "aarch64-unknown-linux-musl",
              ("Darwin", "x86_64"): "x86_64-apple-darwin", ("Darwin", "arm64"): "aarch64-apple-darwin"}[(system, machine)]
    binary = b"#!/bin/sh\necho 'LlamaScope 1.2.3'\n"
    files = {"/asset/1": binary, "/asset/2": f"{hashlib.sha256(binary).hexdigest()}  llamascope-{target}\n".encode()}
    seen = []

    class GitHub(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append((self.path, self.headers.get("Authorization")))
            base = f"http://127.0.0.1:{self.server.server_port}"
            release = {"url": f"{base}/repos/o/r/releases/1", "tag_name": "v1.2.3", "body": "notes, with commas", "assets": [
                {"url": f"{base}/repos/o/r/releases/assets/{n}", "name": name, "uploader": {"url": f"{base}/users/x"}}
                for n, name in [(1, f"llamascope-{target}"), (2, "SHA256SUMS")]]}
            path = self.path.replace("/repos/o/r/releases/assets", "/asset")
            body = files.get(path) or json.dumps(release, indent=2).encode()
            self.send_response(200 if path in files or path.endswith(("/latest", "/tags/v1.2.3")) else 404)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_): pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), GitHub)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    release_env = {**env, "LLAMASCOPE_REPO": "o/r", "LLAMASCOPE_API": f"http://127.0.0.1:{server.server_port}", "GH_TOKEN": "t0ken"}
    script = (ROOT / "install.sh").read_bytes()
    try:
        subprocess.run(["sh"], input=script, cwd=home, env=release_env, check=True, capture_output=True)
        installed = home / "custom prefix" / "bin" / "llamascope"
        assert installed.read_bytes() == binary and os.access(installed, os.X_OK)
        assert seen[0] == ("/repos/o/r/releases/latest", "Bearer t0ken"), seen
        subprocess.run(["sh", "-s", "--", "--version", "1.2.3"], input=script, cwd=home, env=release_env, check=True, capture_output=True)
        assert seen[-3][0] == "/repos/o/r/releases/tags/v1.2.3", seen
        files["/asset/2"] = f"{'0' * 64}  llamascope-{target}\n".encode()
        installed.write_bytes(b"old")
        result = subprocess.run(["sh"], input=script, cwd=home, env=release_env, capture_output=True)
        assert result.returncode != 0 and b"checksum mismatch" in result.stderr, result.stderr
        assert installed.read_bytes() == b"old"
        result = subprocess.run(["sh", "-s", "--", "--version", "9.9.9"], input=script, cwd=home, env=release_env, capture_output=True)
        assert result.returncode != 0 and b"cannot read release" in result.stderr, result.stderr
    finally:
        server.shutdown()
print("PASS scripts: arbitrary working directory, argument boundaries, custom prefix, no shell edits, failure propagation")
print("PASS curl | sh install: latest and pinned releases, token header, checksum refusal, missing release")
