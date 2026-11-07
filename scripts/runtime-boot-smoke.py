#!/usr/bin/env python3
"""CI-only real Docker boot, CLI dispatch, offline resident reuse and shutdown."""

import argparse
import json
import subprocess
import sys
import time
import uuid
from pathlib import Path, PurePosixPath


LIBS_DIR = "/downloads/staging"


def docker(*args, check=True, timeout=40, input=None):
    return subprocess.run(
        ["docker", *args], check=check, capture_output=True, text=True,
        encoding="utf-8", timeout=timeout, input=input,
    )


def run_arguments(image, name, volume, backend, version):
    args = [
        "run", "--detach", "--name", name, "--label", "ytdlp.boot-smoke=" + volume,
        "--mount", f"type=volume,src={volume},dst=/downloads",
        "--tmpfs", "/tmp:rw,exec,nosuid,nodev,size=512m,mode=1777",
        "--memory", "1g", "--env", "YTDLP_TEMP_DIR=/tmp",
        "--env", "MAX_CONCURRENT_PROCESSES=4", "--env", "RUST_LOG=info",
    ]
    if backend == "resident":
        args += ["--network", "none"]
    args += [image, "--libs-dir", LIBS_DIR, "--backend", backend]
    if version is not None:
        args += ["--runtime-yt-dlp-version", version]
    if backend == "resident":
        args += ["--runtime-uv", "/missing-uv-reuse-must-not-resolve"]
    return args


def ready(name, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if docker("inspect", "--format", "{{.State.Running}}", name).stdout.strip() != "true":
            raise RuntimeError("wrapper exited before admission; inspect installer logs")
        response = docker(
            "exec", name, "curl", "--silent", "--show-error", "--fail",
            "--max-time", "5", "--header", "Content-Type: application/json",
            "--data", '{"args":["--version"],"timeout_seconds":10}',
            "http://127.0.0.1:8080/run", check=False,
        )
        if response.returncode == 0:
            result = json.loads(response.stdout)
            if result["exit_code"] != 0:
                raise RuntimeError(f"--version request failed: {result}")
            return result["stdout"].strip()
        time.sleep(2)
    raise RuntimeError(f"boot deadline ({timeout}s) exceeded; no successful /run response")


def stop(name):
    docker("stop", "--time", "15", name)
    state = json.loads(docker("inspect", "--format", "{{json .State}}", name).stdout)
    if state["Running"] or state["ExitCode"] != 0:
        raise RuntimeError(f"wrapper did not shut down cleanly: {state}")


def check_ejs(name):
    script = Path(__file__).with_name("runtime-ejs-smoke.py").read_text(encoding="utf-8")
    result = docker(
        "exec", "--interactive", name, LIBS_DIR + "/runtime/python/bin/python",
        "-I", "-B", "-", input=script,
    )
    print(result.stdout, end="", flush=True)


def smoke(image, timeout, version):
    volume = "ytdlp-boot-smoke-" + uuid.uuid4().hex
    names = [volume + "-cli", volume + "-resident"]
    created = []
    docker("volume", "create", "--label", "ytdlp.boot-smoke=" + volume, volume)
    try:
        manifest = None
        for name, backend in zip(names, ("cli", "resident")):
            # No --entrypoint: exercise dumb-init and the real application boot.
            created.append(name)
            docker(*run_arguments(image, name, volume, backend, version))
            observed = ready(name, timeout)
            text = docker("exec", name, "cat", LIBS_DIR + "/runtime/manifest.json").stdout
            current = json.loads(text)
            for key in ("base_executable", "base_prefix"):
                base = PurePosixPath(current["plan"]["environment"][key])
                if not base.is_relative_to(PurePosixPath(LIBS_DIR) / "uv-python"):
                    raise RuntimeError(f"{backend} {key} is not in stable managed storage: {base}")
            if observed != current["plan"]["yt_dlp_version"]:
                raise RuntimeError(f"{backend} returned wrong explicit yt-dlp version: {observed}")
            if manifest is not None:
                if text != manifest or "runtime_reused" not in docker("logs", name).stdout:
                    raise RuntimeError("offline resident restart did not reuse unchanged runtime")
                check_ejs(name)
            manifest = text
            stop(name)
            print(f"Verified {backend}: {observed}, runtime {current['runtime_id']}", flush=True)
    finally:
        quiescent = True
        for name in created:
            logs = docker("logs", name, check=False)
            print(logs.stdout + logs.stderr, flush=True)
            stopped = docker("stop", "--time", "15", name, check=False)
            if stopped.returncode == 0:
                docker("rm", name)
            else:
                quiescent = False
                print(f"Preserving {name} and {volume}: stop failed", file=sys.stderr)
        if quiescent:
            docker("volume", "rm", volume)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--yt-dlp-version", help="Explicit deployment pin; no implicit latest or fallback")
    args = parser.parse_args(argv)
    if not 1 <= args.timeout <= 1800:
        parser.error("--timeout must be in 1..1800 seconds")
    try:
        smoke(args.image, args.timeout, args.yt_dlp_version)
    except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Real boot smoke failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
