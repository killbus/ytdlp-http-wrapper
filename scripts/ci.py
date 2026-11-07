#!/usr/bin/env python3
"""Repository-owned fixture gates; no runtime artifact downloads or installation."""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINARY = "ytdlp-http-wrapper"
BINARY_ENV = f"CARGO_BIN_EXE_{BINARY}"


def run(args, *, env=None, capture=False):
    print("+ " + " ".join(map(str, args)), flush=True)
    return subprocess.run(
        args,
        cwd=ROOT,
        env=env,
        check=True,
        text=True,
        encoding="utf-8",
        stdout=subprocess.PIPE if capture else None,
    )


def require_tool(name):
    if shutil.which(name) is None:
        raise RuntimeError(
            f"Required tool '{name}' is missing from PATH; install it before CI."
        )


def host_target():
    require_tool("cargo")
    require_tool("rustc")
    output = run(["rustc", "-vV"], capture=True).stdout
    hosts = [
        line.removeprefix("host: ").strip()
        for line in output.splitlines()
        if line.startswith("host: ")
    ]
    if len(hosts) != 1 or not hosts[0]:
        raise RuntimeError("rustc -vV did not report exactly one host target.")
    return hosts[0]


def require_permission_denial():
    if not sys.platform.startswith("linux"):
        return
    if os.geteuid() == 0:
        raise RuntimeError(
            "Run fixture gates as an ordinary non-root user, without sudo."
        )
    # The Rust permission tests use the same temp filesystem and otherwise skip
    # when capabilities or mount semantics allow writes through mode 0555.
    with tempfile.TemporaryDirectory(prefix="ytdlp-ci-permissions-") as directory:
        probe = Path(directory)
        try:
            probe.chmod(0o555)
            try:
                (probe / "denied").write_bytes(b"probe")
            except PermissionError:
                return
            raise RuntimeError(
                "Runner bypasses directory permissions; use an unprivileged user "
                "and a local temp filesystem honoring chmod for Linux denial tests."
            )
        finally:
            probe.chmod(0o700)


def fixture_prerequisites():
    require_permission_denial()
    for tool in ("tar", "curl", "uv"):
        require_tool(tool)
        run([tool, "--version"])
    # Match the frozen Rust fixtures' interpreter search, rather than assuming
    # the interpreter used to launch this script is discoverable by those tests.
    python = next((name for name in ("python", "python3") if shutil.which(name)), None)
    if python is None:
        raise RuntimeError(
            "Put Python 3.10+ on PATH as python or python3 for the Rust fixtures."
        )
    run(
        [
            python,
            "-c",
            "import sys; assert sys.version_info >= (3, 10), 'Python 3.10+ required'",
        ]
    )


def fresh_binary(target):
    output = run(
        [
            "cargo",
            "build",
            "--locked",
            "--bin",
            BINARY,
            "--target",
            target,
            "--message-format=json-render-diagnostics",
        ],
        capture=True,
    ).stdout
    executables = []
    for line in output.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("target", {}).get("name") == BINARY
            and message.get("target", {}).get("kind") == ["bin"]
            and not message.get("profile", {}).get("test")
            and message.get("executable")
        ):
            executables.append(Path(message["executable"]))
    if len(executables) != 1:
        raise RuntimeError(
            "Current Cargo build did not report exactly one wrapper executable."
        )
    executable = executables[0]
    if (
        not executable.is_absolute()
        or not executable.is_file()
        or not os.access(executable, os.X_OK)
    ):
        raise RuntimeError(
            "Cargo's reported wrapper executable is missing or not executable."
        )
    return executable.resolve()


def script_tests():
    run([sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests", "-v"])


def check():
    fixture_prerequisites()
    target = host_target()
    run(["cargo", "fmt", "--check"])
    run(
        [
            "cargo",
            "clippy",
            "--locked",
            "--all-targets",
            "--target",
            target,
            "--",
            "-D",
            "warnings",
        ]
    )
    executable = fresh_binary(target)
    print(f"Current host executable: {executable}", flush=True)
    env = os.environ.copy()
    env[BINARY_ENV] = str(executable)
    run(["cargo", "test", "--locked", "--target", target], env=env)
    run([sys.executable, "-m", "unittest", "tests.test_ytdlp_worker", "-v"])
    script_tests()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "gate", nargs="?", default="check", choices=("check", "release", "script-tests")
    )
    args = parser.parse_args(argv)
    try:
        if args.gate == "check":
            check()
        elif args.gate == "release":
            run(["cargo", "build", "--locked", "--release", "--target", host_target()])
        else:
            script_tests()
    except subprocess.CalledProcessError as error:
        print(f"CI command exited {error.returncode}; stopping.", file=sys.stderr)
        return error.returncode if error.returncode > 0 else 1
    except (OSError, ValueError, RuntimeError) as error:
        print(f"CI precondition/orchestration error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
