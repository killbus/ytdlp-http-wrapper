"""Exercise orchestration with offline command stubs, not Linux/Docker evidence."""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def find_bash():
    bash = shutil.which("bash")
    if bash:
        return bash
    git = shutil.which("git")
    if os.name == "nt" and git:
        candidate = Path(git).resolve().parents[1] / "bin/bash.exe"
        if candidate.is_file():
            return str(candidate)
    raise RuntimeError(
        "Script tests require Bash (Git for Windows includes bin/bash.exe)."
    )


class ShellTests(unittest.TestCase):
    def setUp(self):
        self.bash = find_bash()
        self.temporary = tempfile.TemporaryDirectory(prefix="ytdlp-shell-unit-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        self.log = self.directory / "commands.log"
        self.stub("uname", 'if [ "$1" = -s ]; then echo Linux; else echo x86_64; fi\n')
        self.stub("rustc", 'echo "host: x86_64-unknown-linux-gnu"\n')
        self.stub("chmod", 'echo chmod >> "$CI_TEST_LOG"\nexec "$REAL_CHMOD" "$@"\n')
        self.stub(
            "cargo", 'echo "cargo $*" >> "$CI_TEST_LOG"\nexit "${CI_CARGO_EXIT:-0}"\n'
        )
        self.stub(
            "gh",
            r"""
echo "gh $*" >> "$CI_TEST_LOG"
if [ "$2" = view ]; then echo 2026.10.26; exit 0; fi
if [ "${CI_SUMS_MODE:-}" = download-error ]; then exit 23; fi
while [ "$1" != --dir ]; do shift; done
shift
cd "$1"
cat > yt-dlp_linux <<'BINARY'
#!/bin/sh
echo version >> "$CI_TEST_LOG"
BINARY
sha256sum yt-dlp_linux > SHA2-256SUMS
case "${CI_SUMS_MODE:-}" in
    missing) : > SHA2-256SUMS ;;
    wrong) printf '%064d  yt-dlp_linux\n' 0 > SHA2-256SUMS ;;
    duplicate) cat SHA2-256SUMS SHA2-256SUMS > duplicates; mv duplicates SHA2-256SUMS ;;
    malformed) printf 'not-a-hash  yt-dlp_linux\n' > SHA2-256SUMS ;;
    unrelated) printf 'other' > other; sha256sum other > SHA2-256SUMS ;;
    absent-binary) rm yt-dlp_linux ;;
esac
""",
        )
        # Exact repository handoff, not a Compose emulator. Supported run flags:
        # https://docs.docker.com/reference/cli/docker/compose/run/
        # https://github.com/docker/compose/blob/v2.40.3/cmd/compose/run.go#L225
        self.stub(
            "docker",
            r"""
echo "docker $*" >> "$CI_TEST_LOG"
if [ "${CI_DOCKER_FAIL:-}" = build ] && [ "$1 $2" = 'buildx build' ]; then exit 29; fi
if [ "$1" = compose ]; then
    shift
    if [ "$1" = -f ]; then shift 2; fi
    if [ "$1" = run ]; then
        shift
        expected=(--rm --no-deps --pull never -T --entrypoint /bin/sh ytdlp -eu)
        for arg in "${expected[@]}"; do
            if [ "${1:-}" != "$arg" ]; then
                echo "Unexpected Compose run argument: ${1:-<missing>}" >&2
                exit 64
            fi
            shift
        done
        [ "$#" -eq 0 ] || exit 64
        cat > "$CI_TEST_INPUT"
        exit "${CI_DOCKER_RUN_EXIT:-0}"
    fi
fi
""",
        )

    def stub(self, name, body):
        script = self.bin / name
        script.write_text(
            "#!/usr/bin/env bash\nset -eu\n" + body, encoding="utf-8", newline="\n"
        )
        script.chmod(0o700)

    def execute(self, script, *args, **env_updates):
        env = os.environ.copy()
        env.update(env_updates)
        # Convert paths before adding stubs to PATH: Windows drive colons are
        # not PATH separators in PowerShell, but are separators inside Bash.
        bootstrap = r"""
set -eu
bin=$1; work=$2; script=$3; shift 3
if command -v cygpath >/dev/null; then
    bin=$(cygpath -u "$bin"); work=$(cygpath -u "$work"); script=$(cygpath -u "$script")
fi
export REAL_CHMOD=$(command -v chmod)
export CI_TEST_LOG="$work/commands.log" CI_TEST_INPUT="$work/container-input"
export TMPDIR="$work" PATH="$bin:$PATH"
exec bash "$script" "$@"
"""
        return subprocess.run(
            [
                self.bash,
                "--noprofile",
                "--norc",
                "-c",
                bootstrap,
                "test-shell",
                str(self.bin),
                str(self.directory),
                str(ROOT / "scripts" / script),
                *args,
            ],
            cwd=self.directory,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )

    def commands(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def test_official_smoke_verifies_before_execution_and_cleans_owned_directory(self):
        result = self.execute("ci-linux-smoke.sh")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        self.assertIn("release view", commands[0])
        self.assertIn("release download 2026.10.26", commands[1])
        self.assertEqual(commands[2:4], ["chmod", "version"])
        self.assertEqual(
            commands[4],
            "cargo test --locked --target x86_64-unknown-linux-gnu --test linux_ytdlp_smoke -- --ignored --nocapture",
        )
        self.assertIn("Verified yt-dlp release 2026.10.26 SHA256", result.stdout)
        self.assertEqual(list(self.directory.glob("ytdlp-ci-smoke.*")), [])

    def test_missing_malformed_duplicate_or_wrong_checksum_never_executes(self):
        for mode in (
            "missing",
            "wrong",
            "duplicate",
            "malformed",
            "unrelated",
            "absent-binary",
            "download-error",
        ):
            with self.subTest(mode=mode):
                self.log.write_text("")
                result = self.execute("ci-linux-smoke.sh", CI_SUMS_MODE=mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(all(line.startswith("gh ") for line in self.commands()))
                self.assertEqual(list(self.directory.glob("ytdlp-ci-smoke.*")), [])

    def test_tag_override_and_smoke_exit_are_preserved(self):
        result = self.execute("ci-linux-smoke.sh", "2026.09.23", CI_CARGO_EXIT="31")
        self.assertEqual(result.returncode, 31, result.stderr)
        self.assertIn("release download 2026.09.23", self.commands()[0])
        self.assertNotIn("release view", "\n".join(self.commands()))
        self.assertEqual(list(self.directory.glob("ytdlp-ci-smoke.*")), [])

    def test_invalid_tag_is_rejected_before_download(self):
        result = self.execute("ci-linux-smoke.sh", "--clobber")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.commands(), [])

    def test_wrong_rust_host_does_not_execute_a_zero_test_smoke(self):
        self.stub("rustc", 'echo "host: x86_64-pc-windows-msvc"\n')
        result = self.execute("ci-linux-smoke.sh")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("version", self.commands())
        self.assertTrue(all(not line.startswith("cargo ") for line in self.commands()))

    def test_ci_shell_wrapper_forwards_arguments_and_exit(self):
        self.stub("python", 'echo "python $*" >> "$CI_TEST_LOG"\nexit 37\n')
        result = self.execute("ci.sh", "script-tests")
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertEqual(len(self.commands()), 1)
        self.assertTrue(self.commands()[0].endswith("/scripts/ci.py script-tests"))

    def test_consumer_refuses_relative_artifact_and_non_linux(self):
        result = self.execute("linux-smoke.sh", YTDLP_SMOKE_BINARY="relative")
        self.assertNotEqual(result.returncode, 0)
        self.stub("uname", "echo Darwin\n")
        result = self.execute("ci-linux-smoke.sh")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.commands(), [])

    def test_container_rejects_unsupported_no_build_option(self):
        scripts = self.directory / "pre-fix" / "scripts"
        scripts.mkdir(parents=True)
        shutil.copyfile(ROOT / "compose.yaml", scripts.parent / "compose.yaml")
        shutil.copyfile(
            ROOT / "scripts/container-smoke.sh", scripts / "container-smoke.sh"
        )
        script = scripts / "ci-container.sh"
        source = (ROOT / "scripts" / "ci-container.sh").read_text()
        self.assertEqual(source.count("--pull never"), 1)
        script.write_text(
            source.replace("--pull never", "--no-build --pull never"),
            encoding="utf-8",
            newline="\n",
        )
        result = self.execute(script)
        self.assertEqual(result.returncode, 64, result.stderr)
        self.assertIn("Unexpected Compose run argument: --no-build", result.stderr)
        self.assertEqual(len(self.commands()), 5)
        self.assertFalse((self.directory / "container-input").exists())

    def test_container_calls_existing_smoke_without_publish_or_actions_cache(self):
        result = self.execute("ci-container.sh")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        self.assertEqual(len(commands), 5)
        self.assertEqual(
            commands[:2], ["docker buildx version", "docker compose version"]
        )
        self.assertTrue(commands[2].endswith("config --quiet"))
        self.assertEqual(
            commands[3], "docker buildx build --load --tag ytdlp-http-wrapper:local ."
        )
        self.assertIn(
            "run --rm --no-deps --pull never -T --entrypoint /bin/sh ytdlp -eu",
            commands[4],
        )
        self.assertEqual(
            (self.directory / "container-input").read_bytes(),
            (ROOT / "scripts/container-smoke.sh").read_bytes(),
        )

    def test_container_run_failure_propagates_after_smoke_input(self):
        for code in (1, 37):
            with self.subTest(code=code):
                self.log.write_text("")
                result = self.execute("ci-container.sh", CI_DOCKER_RUN_EXIT=str(code))
                self.assertEqual(result.returncode, code, result.stderr)
                self.assertEqual(len(self.commands()), 5)
                self.assertEqual(
                    (self.directory / "container-input").read_bytes(),
                    (ROOT / "scripts/container-smoke.sh").read_bytes(),
                )

    def test_container_accepts_only_cache_options_and_stops_on_build_error(self):
        result = self.execute(
            "ci-container.sh",
            "--cache-from=type=gha",
            "--cache-to=type=gha,mode=max",
            CI_DOCKER_FAIL="build",
        )
        self.assertEqual(result.returncode, 29, result.stderr)
        self.assertIn(
            "--cache-from=type=gha --cache-to=type=gha,mode=max", self.commands()[-1]
        )
        self.assertEqual(len(self.commands()), 4)
        self.log.write_text("")
        result = self.execute("ci-container.sh", "--push")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.commands(), [])


if __name__ == "__main__":
    unittest.main()
