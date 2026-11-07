import importlib.metadata
import importlib.util
import subprocess
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


SPEC = importlib.util.spec_from_file_location(
    "ejs_smoke", Path(__file__).resolve().parents[1] / "runtime-ejs-smoke.py"
)
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class EjsSmokeTests(unittest.TestCase):
    def setUp(self):
        self.solver = types.ModuleType("yt_dlp_ejs.yt.solver")
        self.solver.lib = Mock(return_value="var lib = {fixture: true};")
        self.solver.core = Mock(return_value="function jsc() {}")
        package = types.ModuleType("yt_dlp_ejs")
        yt = types.ModuleType("yt_dlp_ejs.yt")
        package.yt = yt
        yt.solver = self.solver
        modules = patch.dict(sys.modules, {
            "yt_dlp_ejs": package, "yt_dlp_ejs.yt": yt, "yt_dlp_ejs.yt.solver": self.solver,
        })
        modules.start()
        self.addCleanup(modules.stop)
        metadata = patch.object(smoke.importlib.metadata, "version", return_value="fixture")
        self.metadata = metadata.start()
        self.addCleanup(metadata.stop)

    def test_bundle_order_restricted_stdin_and_explicit_load_result(self):
        with patch.object(smoke.subprocess, "run") as run:
            run.return_value.stdout = smoke.LOAD_MARKER + "\n"
            smoke.check_ejs()
        argv = run.call_args.args[0]
        self.assertEqual(argv[:2], ["deno", "run"])
        self.assertEqual(argv[-1], "-")
        for flag in ("--no-config", "--no-lock", "--no-prompt", "--no-remote", "--no-npm", "--cached-only"):
            self.assertIn(flag, argv)
        self.assertFalse(any(arg == "-A" or arg.startswith("--allow-") for arg in argv))
        options = run.call_args.kwargs
        self.assertTrue(options["check"])
        self.assertEqual(options["timeout"], 30)
        script = options["input"]
        lib = script.index(self.solver.lib.return_value)
        assignment = script.index("Object.assign(globalThis, lib);")
        core = script.index(self.solver.core.return_value)
        assertion = script.index('typeof jsc !== "function"')
        marker = script.index(smoke.LOAD_MARKER)
        self.assertLess(lib, assignment)
        self.assertLess(assignment, core)
        self.assertLess(core, assertion)
        self.assertLess(assertion, marker)
        self.metadata.assert_called_once_with("yt-dlp-ejs")

    def test_bare_package_environment_cannot_reach_deno(self):
        with (
            patch.dict(sys.modules, {"yt_dlp_ejs.yt": None}),
            patch.object(smoke.subprocess, "run") as run,
            patch("sys.stderr"),
        ):
            self.assertEqual(smoke.main(), 1)
            run.assert_not_called()

    def test_missing_distribution_cannot_reach_deno(self):
        self.metadata.side_effect = importlib.metadata.PackageNotFoundError("yt-dlp-ejs")
        with patch.object(smoke.subprocess, "run") as run, patch("sys.stderr"):
            self.assertEqual(smoke.main(), 1)
            run.assert_not_called()

    def test_missing_and_empty_bundles_cannot_reach_deno(self):
        for name in ("lib", "core"):
            loader = getattr(self.solver, name)
            original = loader.return_value
            for value in (None, "", " \n", FileNotFoundError("missing bundle")):
                loader.side_effect = value if isinstance(value, Exception) else None
                loader.return_value = value
                with (
                    self.subTest(bundle=name, value=value),
                    patch.object(smoke.subprocess, "run") as run,
                    patch("sys.stderr"),
                ):
                    self.assertEqual(smoke.main(), 1)
                    run.assert_not_called()
            loader.side_effect = None
            loader.return_value = original

    def test_deno_failures_are_not_load_evidence_even_with_a_marker(self):
        for error in (
            FileNotFoundError("deno"),
            subprocess.CalledProcessError(1, ["deno"], output=smoke.LOAD_MARKER),
            subprocess.TimeoutExpired(["deno"], 30),
        ):
            with (
                self.subTest(error=error),
                patch.object(smoke.subprocess, "run", side_effect=error),
                patch("sys.stderr"),
            ):
                self.assertEqual(smoke.main(), 1)

    def test_zero_exit_without_the_entrypoint_marker_is_not_evidence(self):
        for output in ("", "deno 2.9.8\n", "not-" + smoke.LOAD_MARKER):
            with (
                self.subTest(output=output),
                patch.object(smoke.subprocess, "run") as run,
                patch("sys.stderr"),
            ):
                run.return_value.stdout = output
                self.assertEqual(smoke.main(), 1)


if __name__ == "__main__":
    unittest.main()
