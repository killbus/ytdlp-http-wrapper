import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("ci", ROOT / "scripts/ci.py")
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)
HOST = "x86_64-test-host"


def artifact(executable):
    return json.dumps(
        {
            "reason": "compiler-artifact",
            "target": {"name": ci.BINARY, "kind": ["bin"]},
            "profile": {"test": False},
            "executable": str(executable),
        }
    )


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ytdlp-ci-unit-")
        self.addCleanup(self.temporary.cleanup)
        self.binary = Path(self.temporary.name) / "custom target" / "wrapper.exe"
        self.binary.parent.mkdir()
        self.binary.write_bytes(b"stub executable: never run")
        self.binary.chmod(0o700)

    def output(self, args, **kwargs):
        text = artifact(self.binary) if args[:2] == ["cargo", "build"] else ""
        return subprocess.CompletedProcess(args, 0, stdout=text)

    def test_shared_sequence_uses_current_artifact_and_child_environment(self):
        with (
            patch.object(ci, "fixture_prerequisites"),
            patch.object(ci, "host_target", return_value=HOST),
            patch.object(ci, "run", side_effect=self.output) as run,
            patch.dict(
                os.environ,
                {
                    ci.BINARY_ENV: "stale.exe",
                    "CARGO_TARGET_DIR": "custom target",
                    "CARGO_BUILD_TARGET": "not-the-host",
                },
            ),
        ):
            self.assertEqual(ci.main(["check"]), 0)
            commands = [call.args[0] for call in run.call_args_list]
            self.assertEqual(
                commands,
                [
                    ["cargo", "fmt", "--check"],
                    [
                        "cargo",
                        "clippy",
                        "--locked",
                        "--all-targets",
                        "--target",
                        HOST,
                        "--",
                        "-D",
                        "warnings",
                    ],
                    [
                        "cargo",
                        "build",
                        "--locked",
                        "--bin",
                        ci.BINARY,
                        "--target",
                        HOST,
                        "--message-format=json-render-diagnostics",
                    ],
                    ["cargo", "test", "--locked", "--target", HOST],
                    [sys.executable, "-m", "unittest", "tests.test_ytdlp_worker", "-v"],
                    [
                        sys.executable,
                        "-m",
                        "unittest",
                        "discover",
                        "-s",
                        "scripts/tests",
                        "-v",
                    ],
                ],
            )
            env = run.call_args_list[3].kwargs["env"]
            self.assertEqual(env[ci.BINARY_ENV], str(self.binary.resolve()))
            self.assertEqual(env["CARGO_TARGET_DIR"], "custom target")
            self.assertEqual(os.environ[ci.BINARY_ENV], "stale.exe")

    def test_every_gate_failure_stops_without_running_later_commands(self):
        for index in range(6):
            with self.subTest(index=index):
                seen = []

                def fail_one(args, seen=seen, index=index, **kwargs):
                    seen.append(args)
                    if len(seen) == index + 1:
                        raise subprocess.CalledProcessError(17, args)
                    return self.output(args, **kwargs)

                with (
                    patch.object(ci, "fixture_prerequisites"),
                    patch.object(ci, "host_target", return_value=HOST),
                    patch.object(ci, "run", side_effect=fail_one),
                ):
                    self.assertEqual(ci.main(["check"]), 17)
                self.assertEqual(len(seen), index + 1)

    def test_missing_or_ambiguous_artifact_never_uses_stale_environment(self):
        outputs = [
            "",
            "not JSON",
            artifact("relative/wrapper.exe"),
            artifact(self.binary.parent / "missing.exe"),
            artifact(self.binary) + "\n" + artifact(self.binary),
        ]
        for output in outputs:
            with self.subTest(output=output), patch.object(ci, "run") as run:
                run.return_value.stdout = output
                with (
                    patch.dict(os.environ, {ci.BINARY_ENV: str(self.binary)}),
                    self.assertRaises((RuntimeError, ValueError)),
                ):
                    ci.fresh_binary(HOST)

    def test_runner_uses_repository_cwd_and_argument_lists(self):
        with patch.object(ci.subprocess, "run") as run:
            ci.run(["tool", "argument with spaces"], env={"key": "value"}, capture=True)
        run.assert_called_once_with(
            ["tool", "argument with spaces"],
            cwd=ROOT,
            env={"key": "value"},
            check=True,
            text=True,
            encoding="utf-8",
            stdout=subprocess.PIPE,
        )

    def test_release_gate_is_locked_and_host_targeted(self):
        with (
            patch.object(ci, "host_target", return_value=HOST),
            patch.object(ci, "run") as run,
        ):
            self.assertEqual(ci.main(["release"]), 0)
        run.assert_called_once_with(
            ["cargo", "build", "--locked", "--release", "--target", HOST]
        )

    def test_missing_tools_stop_before_gates(self):
        with (
            patch.object(ci, "require_permission_denial"),
            patch.object(ci.shutil, "which", return_value=None),
            patch.object(ci, "run") as run,
        ):
            self.assertEqual(ci.main(["check"]), 1)
        run.assert_not_called()

    def test_python_must_be_discoverable_under_fixture_names(self):
        with (
            patch.object(ci, "require_permission_denial"),
            patch.object(ci, "require_tool"),
            patch.object(ci.shutil, "which", return_value=None),
            patch.object(ci, "run"),
            self.assertRaisesRegex(RuntimeError, "python or python3"),
        ):
            ci.fixture_prerequisites()

    def test_root_and_permission_bypass_are_not_permission_evidence(self):
        with (
            patch.object(ci.sys, "platform", "linux"),
            patch.object(ci.os, "geteuid", return_value=0, create=True),
            self.assertRaisesRegex(RuntimeError, "non-root"),
        ):
            ci.require_permission_denial()
        with (
            patch.object(ci.sys, "platform", "linux"),
            patch.object(ci.os, "geteuid", return_value=1000, create=True),
            patch.object(Path, "write_bytes", return_value=5),
            self.assertRaisesRegex(RuntimeError, "bypasses"),
        ):
            ci.require_permission_denial()

    def test_permission_denial_probe_accepts_only_a_real_denial(self):
        with (
            patch.object(ci.sys, "platform", "linux"),
            patch.object(ci.os, "geteuid", return_value=1000, create=True),
            patch.object(Path, "write_bytes", side_effect=PermissionError),
        ):
            ci.require_permission_denial()

    def test_host_target_must_be_present_and_unambiguous(self):
        for output in ("", "host: \n", "host: first\nhost: second"):
            with (
                self.subTest(output=output),
                patch.object(ci, "require_tool"),
                patch.object(ci, "run") as run,
            ):
                run.return_value.stdout = output
                with self.assertRaises(RuntimeError):
                    ci.host_target()


if __name__ == "__main__":
    unittest.main()
