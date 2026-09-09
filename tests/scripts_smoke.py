#!/usr/bin/env python3
"""Check launcher/installer contracts with a fake Cargo; never modifies the real HOME."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

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
    subprocess.run(["bash", str(ROOT / "install.sh")], cwd=home, env=env, check=True, capture_output=True)
    args = json.loads(capture.read_text())
    assert args == ["install", "--locked", "--path", str(ROOT), "--root", str(home / "custom prefix"), "--bin", "llamascope", "--force"]
    for shell_file in [".zshrc", ".bashrc"]:
        assert (home / shell_file).read_text() == "# user configuration; do not change\n"
    for script in ["start.sh", "install.sh"]:
        result = subprocess.run(["bash", str(ROOT / script)], cwd=home, env={**env, "FAKE_EXIT": "17"}, capture_output=True)
        assert result.returncode == 17, f"{script} swallowed a Cargo failure"
        assert b"Installed. Try" not in result.stdout
print("PASS scripts: arbitrary working directory, argument boundaries, custom prefix, no shell edits, failure propagation")
