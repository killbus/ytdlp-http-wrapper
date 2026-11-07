import importlib.util
import json
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "boot_smoke", Path(__file__).resolve().parents[1] / "runtime-boot-smoke.py"
)
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class BootSmokeTests(unittest.TestCase):
    def test_real_entrypoint_and_offline_reuse_arguments(self):
        first = smoke.run_arguments("image", "name", "owned", "cli", None)
        self.assertNotIn("--entrypoint", first)
        self.assertNotIn("--runtime-yt-dlp-version", first)
        self.assertNotIn("--network", first)
        self.assertEqual(first[first.index("--libs-dir") + 1], "/downloads/staging")
        self.assertIn("type=volume,src=owned,dst=/downloads", first)
        second = smoke.run_arguments("image", "name", "owned", "resident", "pin")
        self.assertEqual(second[second.index("--network") + 1], "none")
        self.assertEqual(second[second.index("--libs-dir") + 1], "/downloads/staging")
        self.assertEqual(second[second.index("--runtime-yt-dlp-version") + 1], "pin")
        self.assertIn("/missing-uv-reuse-must-not-resolve", second)

    def test_named_staging_root_requires_a_stable_managed_base(self):
        for invalid in (None, "base_executable", "base_prefix"):
            environment = {
                "base_executable": "/downloads/staging/uv-python/cpython/bin/python",
                "base_prefix": "/downloads/staging/uv-python/cpython",
            }
            if invalid:
                environment[invalid] = "/downloads/staging/staging/candidate/python"
            manifest = json.dumps({
                "runtime_id": "fixture", "plan": {"yt_dlp_version": "pin", "environment": environment},
            })

            def reply(*args, **kwargs):
                stdout = "runtime_reused" if args[0] == "logs" else manifest
                return subprocess.CompletedProcess(args, 0, stdout, "")

            with (
                self.subTest(invalid=invalid),
                patch.object(smoke, "docker", side_effect=reply) as docker,
                patch.object(smoke, "ready", return_value="pin"),
                patch.object(smoke, "stop"),
                patch.object(smoke, "check_ejs") as ejs,
                patch("builtins.print"),
            ):
                if invalid:
                    with self.assertRaisesRegex(RuntimeError, "not in stable managed storage"):
                        smoke.smoke("image", 1, "pin")
                else:
                    smoke.smoke("image", 1, "pin")
                if invalid:
                    ejs.assert_not_called()
                else:
                    ejs.assert_called_once()
                    self.assertTrue(ejs.call_args.args[0].endswith("-resident"))
                reads = [call.args for call in docker.call_args_list if call.args[0] == "exec"]
                self.assertEqual(len(reads), 1 if invalid else 2)
                for args in reads:
                    self.assertEqual(args[2:], ("cat", "/downloads/staging/runtime/manifest.json"))

    def test_ejs_helper_uses_installed_isolated_python_over_stdin(self):
        with patch.object(smoke, "docker") as docker, patch("builtins.print"):
            smoke.check_ejs("offline-resident")
        docker.assert_called_once_with(
            "exec", "--interactive", "offline-resident",
            "/downloads/staging/runtime/python/bin/python", "-I", "-B", "-",
            input=Path(smoke.__file__).with_name("runtime-ejs-smoke.py").read_text(encoding="utf-8"),
        )

    def test_ejs_failure_propagates_after_cli_shutdown_and_cleans_up(self):
        manifest = json.dumps({
            "runtime_id": "fixture", "plan": {
                "yt_dlp_version": "pin", "environment": {
                    "base_executable": "/downloads/staging/uv-python/cpython/bin/python",
                    "base_prefix": "/downloads/staging/uv-python/cpython",
                },
            },
        })
        events = []

        def reply(*args, **kwargs):
            events.append(args)
            if args[:2] == ("exec", "--interactive"):
                raise subprocess.CalledProcessError(1, args, stderr="EJS load failed")
            stdout = "runtime_reused" if args[0] == "logs" else manifest
            return subprocess.CompletedProcess(args, 0, stdout, "")

        with (
            patch.object(smoke, "docker", side_effect=reply),
            patch.object(smoke, "ready", return_value="pin"),
            patch.object(smoke, "stop", side_effect=lambda name: events.append(("clean-stop", name))),
            patch("builtins.print"),
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                smoke.smoke("image", 1, "pin")
        starts = [event for event in events if event[0] == "run"]
        self.assertEqual(len(starts), 2)
        cli = starts[0][starts[0].index("--name") + 1]
        resident = starts[1][starts[1].index("--name") + 1]
        self.assertLess(events.index(("clean-stop", cli)), events.index(starts[1]))
        self.assertEqual(starts[1][starts[1].index("--network") + 1], "none")
        self.assertIn("/missing-uv-reuse-must-not-resolve", starts[1])
        gate = next(event for event in events if event[:2] == ("exec", "--interactive"))
        self.assertEqual(gate[2], resident)
        self.assertLess(events.index(starts[1]), events.index(gate))
        self.assertNotIn(("clean-stop", resident), events)
        for name in (cli, resident):
            self.assertIn(("stop", "--time", "15", name), events)
            self.assertIn(("rm", name), events)
        self.assertEqual(events[-1][:2], ("volume", "rm"))

    def test_failed_or_killed_shutdown_is_not_reuse_evidence(self):
        with patch.object(smoke, "docker") as docker:
            docker.return_value.stdout = '{"Running":false,"ExitCode":137}'
            with self.assertRaisesRegex(RuntimeError, "shut down cleanly"):
                smoke.stop("owned")

    def test_early_exit_fails_without_waiting_for_deadline(self):
        with patch.object(smoke, "docker") as docker:
            docker.return_value.stdout = "false"
            with self.assertRaisesRegex(RuntimeError, "before admission"):
                smoke.ready("owned", 1)

    def test_main_propagates_failures_and_never_substitutes_a_pin(self):
        with patch.object(smoke, "smoke", side_effect=subprocess.TimeoutExpired("docker", 1)) as run:
            self.assertEqual(smoke.main(["--image", "image"]), 1)
            run.assert_called_once_with("image", 900, None)


if __name__ == "__main__":
    unittest.main()
