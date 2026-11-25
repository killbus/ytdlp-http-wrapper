"""Exercise the real binary and public sources in a disposable local runtime."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

repo = Path(__file__).resolve().parents[3]
binary = repo / "target" / "debug" / ("ytdlp-http-wrapper.exe" if os.name == "nt" else "ytdlp-http-wrapper")
root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(tempfile.mkdtemp(prefix="online-update-", dir=repo / "target"))
assert root.is_relative_to(repo / "target"), root
(repo / "target" / "http-tmp").mkdir(exist_ok=True)
print(f"Isolated verification root: {root}", flush=True)
pin = "2026.10.26"


def call(port, args, *, success=True):
    data = json.dumps({"args": args, "timeout_seconds": 300}).encode()
    request = urllib.request.Request(f"http://127.0.0.1:{port}/run", data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=340) as response:
        result = json.load(response)
    print(json.dumps({"args": args, "result": result}, ensure_ascii=False), flush=True)
    assert (result["exit_code"] == 0) == success, result
    return result["stdout"].strip()


def phase(backend, *, update=False, expected=None, failure=False):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    args = [str(binary), "--host", "127.0.0.1", "--port", str(port), "--libs-dir", str(root / "libs"),
            "--temp-dir", str(repo / "target" / "http-tmp"),
            "--runtime-yt-dlp-version", pin, "--runtime-python", sys.executable, "--backend", backend]
    if expected:
        args += ["--runtime-uv", str(root / "missing-uv-reuse-only")]
    path = root / f"{backend}-{'update' if update else 'restart'}.log"
    env = {k: v for k, v in os.environ.items() if k not in {"RUNTIME_PLAN", "WORKER_PYTHON", "WORKER_PATH", "WORKER_PYTHONPATH", "WORKER_YTDLP_VERSION"}}
    with path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT, env=env,
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        try:
            deadline = time.monotonic() + 300
            while True:
                if process.poll() is not None:
                    raise RuntimeError(path.read_text(encoding="utf-8")[-12000:])
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=1):
                        break
                except OSError:
                    if time.monotonic() > deadline:
                        raise TimeoutError(path.read_text(encoding="utf-8")[-12000:])
                    time.sleep(0.3)
            version = call(port, ["--version"])
            assert version == (expected or pin), version
            if failure:
                call(port, ["--update"], success=False)
                assert call(port, ["--version"]) == pin
            if update:
                call(port, ["--update"])
                version = call(port, ["--version"])
                assert version != pin, "fixture pin must precede latest stable for a real update check"
                assert "already up to date" in call(port, ["-U"])
                manifest = json.loads((root / "libs/runtime/manifest.json").read_text())
                assert manifest["plan"]["yt_dlp_version"] == version
                assert manifest["online_update_from"] == pin
                assert not (root / "libs/update-backup").exists()
            elif expected and not failure:
                assert "already up to date" in call(port, ["-U"])
            return version
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    stopped = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True)
                    if stopped.returncode and process.poll() is None:
                        process.kill()
                else:
                    process.send_signal(signal.SIGINT)
                process.wait(timeout=20)


if len(sys.argv) > 1:
    version = json.loads((root / "libs/runtime/manifest.json").read_text())["plan"]["yt_dlp_version"]
else:
    phase("resident")
    phase("resident", expected=pin, failure=True)
    version = phase("resident", update=True)
phase("resident", expected=version)
phase("cli", expected=version)
print(f"PASS: resident update, no-op alias, resident/CLI restart persistence ({version}); logs retained at {root}", flush=True)
