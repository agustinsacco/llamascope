#!/usr/bin/env python3
"""Real-input Unix PTY checks. Standard library only; no real llama-server required."""
import codecs
import fcntl
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import termios
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / "target/debug/llamascope"


class Screen:
    """The small VT subset emitted by Crossterm, used to assert the visible screen."""
    def __init__(self, width=140, height=42):
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.pending = ""
        self.resize(width, height)

    def resize(self, width, height):
        self.width, self.height = width, height
        self.rows = [[" "] * width for _ in range(height)]
        self.x = self.y = 0

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith("\x1b"):
                if len(self.pending) < 2:
                    return
                if self.pending[1] == "[":
                    match = re.match(r"\x1b\[([0-?]*)([ -/]*)([@-~])", self.pending)
                    if not match:
                        return
                    params, _, command = match.groups()
                    self.pending = self.pending[match.end():]
                    values = [int(v or 0) for v in params.split(";")] if not params.startswith("?") else []
                    if command in ("H", "f"):
                        self.y = max(0, (values[0] if values else 1) - 1)
                        self.x = max(0, (values[1] if len(values) > 1 else 1) - 1)
                    elif command == "J" and values == [2]:
                        self.rows = [[" "] * self.width for _ in range(self.height)]
                    elif command == "K" and self.y < self.height:
                        start = 0 if values == [2] else self.x
                        self.rows[self.y][start:] = [" "] * (self.width - start)
                    elif command in ("A", "B", "C", "D"):
                        delta = values[0] if values and values[0] else 1
                        if command == "A": self.y = max(0, self.y - delta)
                        if command == "B": self.y += delta
                        if command == "C": self.x += delta
                        if command == "D": self.x = max(0, self.x - delta)
                    continue
                self.pending = self.pending[2:]
                continue
            char, self.pending = self.pending[0], self.pending[1:]
            if char == "\r": self.x = 0
            elif char == "\n": self.y += 1
            elif char >= " ":
                if self.x < self.width and self.y < self.height:
                    self.rows[self.y][self.x] = char
                self.x += 1

    @property
    def text(self):
        return "\n".join("".join(row) for row in self.rows)


class Terminal:
    def __init__(self, args):
        self.master, self.slave = pty.openpty()
        self.original = termios.tcgetattr(self.slave)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 42, 140, 0, 0))
        self.screen = Screen()
        self.output = bytearray()
        self.finished = False
        self.status_read, status_write = os.pipe()
        release_read, self.release_write = os.pipe()
        self.pid = os.fork()
        if self.pid == 0:
            os.close(self.status_read)
            os.close(self.release_write)
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)
            for fd in (0, 1, 2): os.dup2(self.slave, fd)
            os.close(self.master)
            os.close(self.slave)
            env = {k: v for k, v in os.environ.items() if k not in ("LLAMA_API_URL", "LLAMA_API_KEY", "LLAMASCOPE_THEME")}
            env["TERM"] = "xterm-256color"
            # Keep the session leader alive until the parent inspects restored termios.
            # macOS revokes its PTY when the session leader exits, even with slave FDs open.
            code = subprocess.call([str(BINARY), *args], env=env)
            os.write(status_write, str(code).encode())
            os.read(release_read, 1)
            os._exit(0)
        os.close(status_write)
        os.close(release_read)

    def pump(self, seconds=0.1):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                data = os.read(self.master, 65536)
                if not data: return
                self.output.extend(data)
                self.screen.feed(data)

    def wait(self, text, timeout=5):
        deadline = time.monotonic() + timeout
        while text not in self.screen.text and time.monotonic() < deadline:
            self.pump()
        assert text in self.screen.text, f"Expected {text!r}\n{self.screen.text}"

    def send(self, keys):
        os.write(self.master, keys)
        self.pump(0.25)

    def resize(self, width, height):
        self.screen.resize(width, height)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        os.killpg(self.pid, signal.SIGWINCH)
        self.pump(0.3)

    def quit(self, key=b"q"):
        self.send(key)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if select.select([self.status_read], [], [], 0)[0]:
                code = int(os.read(self.status_read, 32))
                assert code == 0, self.output.decode(errors="replace")
                break
            self.pump()
        else:
            raise AssertionError("App did not exit within its bounded request/probe deadlines")
        assert b"\x1b[?1049l" in self.output, "Alternate screen was not restored"
        assert b"\x1b[?25h" in self.output, "Cursor was not restored"
        assert termios.tcgetattr(self.slave) == self.original, "Terminal attributes/raw mode leaked"
        os.write(self.release_write, b"x")
        os.waitpid(self.pid, 0)
        self.finished = True

    def close(self):
        if not self.finished:
            os.killpg(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        os.close(self.master)
        os.close(self.slave)
        os.close(self.status_read)
        os.close(self.release_write)


def demo_check():
    terminal = Terminal(["--demo"])
    try:
        terminal.wait("DEMO")
        assert not termios.tcgetattr(terminal.slave)[3] & termios.ICANON
        terminal.send(b"\t")
        terminal.wait("SLOT 01 / PREFILL")
        terminal.send(b"t")
        terminal.wait("Latte")
        assert b"48;2;239;241;245" in terminal.output
        terminal.send(b"?")
        terminal.wait("YOUR INFERENCE FIELD GUIDE")
        terminal.send(b"\x1b")
        terminal.send(b" ")
        terminal.wait("FROZEN")
        values = terminal.screen.rows[5].copy()
        terminal.pump(1.1)
        assert terminal.screen.rows[5] == values, "Frozen values continued changing"
        terminal.send(b" ")
        terminal.wait("DEMO")
        terminal.resize(80, 24)
        terminal.wait("INFERENCE SLOTS")
        terminal.resize(40, 10)
        terminal.wait("minimum 60 x 18")
        terminal.resize(140, 42)
        terminal.wait("INFERENCE SLOTS")
        terminal.quit()
        print("PASS demo: navigation, theme, help, freeze/resume, resize, q, terminal restoration")
    finally:
        terminal.close()


def ctrl_c_check():
    terminal = Terminal(["--demo"])
    try:
        terminal.wait("DEMO")
        terminal.quit(b"\x03")
        print("PASS Ctrl-C: clean exit and terminal restoration")
    finally:
        terminal.close()


def recovery_check():
    class Handler(http.server.BaseHTTPRequestHandler):
        mode = "loading"
        count = 0
        requests = []

        def do_GET(self):
            type(self).requests.append(self.path)
            status = 200
            if self.path == "/health":
                if self.mode == "loading":
                    status, body = 503, {"error": {"message": "Loading model"}}
                else: body = {"status": "ok"}
            elif self.path == "/slots":
                type(self).count += 10
                body = "malformed" if self.mode == "malformed" else [{"id": 0, "id_task": 12, "is_processing": True, "n_ctx": 4096, "next_token": {"n_decoded": self.count, "n_remain": -1}}]
            elif self.path == "/props":
                body = {"total_slots": 1, "model_path": "/models/fixture-model.gguf"}
            else:
                raise AssertionError(f"Unexpected endpoint {self.path}")
            payload = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *_): pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    terminal = Terminal(["--url", f"http://127.0.0.1:{server.server_port}", "--no-gpu"])
    thread.start()
    try:
        terminal.wait("MODEL LOADING")
        Handler.mode = "live"
        terminal.wait("LIVE")
        terminal.wait("fixture-model.gguf")
        terminal.wait("GPU probes disabled")
        if sys.platform in ("darwin", "linux"):
            terminal.wait("Local machine, not remote server")
        Handler.mode = "malformed"
        terminal.wait("DEGRADED")
        terminal.wait("LAST KNOWN")
        Handler.mode = "live"
        terminal.send(b"r")
        terminal.wait("LIVE")
        server.shutdown()
        server.server_close()
        terminal.wait("OFFLINE")
        terminal.quit()
        assert set(Handler.requests) == {"/health", "/slots", "/props"}
        print("PASS HTTP recovery: loading → live → malformed/stale → live → offline; read-only requests")
    finally:
        terminal.close()
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


if __name__ == "__main__":
    assert BINARY.is_file(), f"Build the app first: cargo build --locked ({BINARY})"
    demo_check()
    ctrl_c_check()
    recovery_check()
