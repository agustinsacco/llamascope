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

    # Release install, piped into sh as `curl ... | sh` does, against loopback release downloads.
    system, machine = platform.system(), platform.machine()
    target = {("Linux", "x86_64"): "x86_64-unknown-linux-musl", ("Linux", "aarch64"): "aarch64-unknown-linux-musl",
              ("Darwin", "x86_64"): "x86_64-apple-darwin", ("Darwin", "arm64"): "aarch64-apple-darwin"}[(system, machine)]
    binary = b"#!/bin/sh\necho 'LlamaScope 1.2.3'\n"
    sums = f"{hashlib.sha256(binary).hexdigest()}  llamascope-{target}\n".encode()
    seen = []

    class Releases(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(self.path)
            if self.path.startswith(("/latest/download/", "/download/v1.2.3/")):
                # GitHub answers release downloads with a redirect to its asset host.
                self.send_response(302)
                self.send_header("Location", "/assets/" + self.path.rsplit("/", 1)[1])
                self.end_headers()
                return
            body = {"/assets/SHA256SUMS": sums, f"/assets/llamascope-{target}": binary}.get(self.path)
            self.send_response(404 if body is None else 200)
            self.send_header("Content-Length", str(len(body or b"")))
            self.end_headers()
            self.wfile.write(body or b"")

        def log_message(self, *_): pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Releases)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    release_env = {**env, "LLAMASCOPE_RELEASES": f"http://127.0.0.1:{server.server_port}"}
    script = (ROOT / "install.sh").read_bytes()
    installed = home / "custom prefix" / "bin" / "llamascope"
    install = lambda *args: subprocess.run(["sh", "-s", "--", *args], input=script, cwd=home, env=release_env, capture_output=True)
    try:
        result = install()
        assert result.returncode == 0, result.stderr
        assert installed.read_bytes() == binary and os.access(installed, os.X_OK)
        assert seen[0] == "/latest/download/SHA256SUMS", seen
        assert install("--version", "1.2.3").returncode == 0
        assert "/download/v1.2.3/SHA256SUMS" in seen
        result = install("--version", "9.9.9")
        assert result.returncode != 0 and b"cannot find release 9.9.9" in result.stderr, result.stderr
        sums = f"{'0' * 64}  llamascope-{target}\n".encode()
        installed.write_bytes(b"old")
        result = install()
        assert result.returncode != 0 and b"checksum mismatch" in result.stderr, result.stderr
        assert installed.read_bytes() == b"old"
        assert list(installed.parent.iterdir()) == [installed], "no staged leftovers"
    finally:
        server.shutdown()
print("PASS scripts: arbitrary working directory, argument boundaries, custom prefix, no shell edits, failure propagation")
print("PASS curl | sh install: latest and pinned releases, redirects, checksum refusal, missing release")
