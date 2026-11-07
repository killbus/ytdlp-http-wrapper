import io
import json
import os
import struct
import sys
import shutil
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "worker"))

import ytdlp_worker


def encoded_frame(message_type, request_id, payload, *, version=1):
    encoded = json.dumps(
        {
            "protocol_version": version,
            "message_type": message_type,
            "request_id": request_id,
            "payload": payload,
        },
        separators=(",", ":"),
    ).encode("utf-8")
    return struct.pack(">I", len(encoded)) + encoded


def decode_all(output):
    output.seek(0)
    frames = []
    while True:
        frame = ytdlp_worker.read_frame(output)
        if frame is None:
            return frames
        frames.append(frame)


class FakeRuntime:
    version = "fixture-version"

    def __init__(self):
        self.calls = []

    def execute(self, args):
        self.calls.append(args)
        return ytdlp_worker.ExecutionResult(
            exit_code=0,
            stdout=f"call-{len(self.calls)}",
            stderr="",
        )


class WorkerTests(unittest.TestCase):
    def run_worker(self, incoming, runtime=None):
        output = io.BytesIO()
        diagnostics = io.StringIO()
        selected_runtime = runtime or FakeRuntime()
        worker = ytdlp_worker.Worker(
            io.BytesIO(incoming),
            output,
            diagnostics,
            lambda: selected_runtime,
            generation_id="generation-1",
            worker_id="worker-1",
        )
        status = worker.run()
        return status, decode_all(output), diagnostics.getvalue(), selected_runtime

    def test_handshake_two_requests_ping_and_shutdown(self):
        request_temp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, request_temp, True)
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame(
                    "run", "request-1", {"args": ["--version"], "request_temp": request_temp}
                ),
                encoded_frame("ping", "ping-1", {}),
                encoded_frame(
                    "run", "request-2", {"args": ["example:test"], "request_temp": request_temp}
                ),
                encoded_frame("shutdown", "shutdown-1", {}),
            ]
        )

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 0)
        self.assertEqual(diagnostics, "")
        self.assertEqual(
            [frame["message_type"] for frame in frames],
            ["hello", "result", "pong", "result", "result"],
        )
        self.assertEqual(
            frames[0]["payload"]["yt_dlp_version"], "fixture-version"
        )
        self.assertEqual(frames[0]["payload"]["generation_id"], "generation-1")
        self.assertEqual(frames[0]["payload"]["worker_id"], "worker-1")
        self.assertEqual(
            runtime.calls,
            [("--version",), ("example:test",)],
        )
        self.assertEqual(frames[1]["payload"]["stdout"], "call-1")
        self.assertEqual(frames[3]["payload"]["stdout"], "call-2")

    def test_bad_ready_fails_closed_without_echoing_payload(self):
        secret = "cookie=secret-token"
        incoming = encoded_frame("run", secret, {"args": [secret]})

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 2)
        self.assertEqual(len(frames), 1)
        self.assertEqual(frames[0]["message_type"], "hello")
        self.assertEqual(runtime.calls, [])
        self.assertEqual(diagnostics, "worker protocol failure\n")
        self.assertNotIn(secret, diagnostics)

    def test_ready_requires_an_empty_payload(self):
        incoming = encoded_frame("ready", "startup", {"unexpected": True})

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 2)
        self.assertEqual(len(frames), 1)
        self.assertEqual(runtime.calls, [])
        self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_version_mismatch_and_truncated_payload_fail_closed(self):
        cases = [
            encoded_frame("ready", "startup", {}, version=2),
            struct.pack(">I", 20) + b"short",
        ]

        for incoming in cases:
            with self.subTest(incoming=incoming):
                status, frames, diagnostics, _ = self.run_worker(incoming)
                self.assertEqual(status, 2)
                self.assertEqual(len(frames), 1)
                self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_invalid_run_payload_fails_closed(self):
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame("run", "request-1", {"args": "not-a-list"}),
            ]
        )

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 2)
        self.assertEqual(len(frames), 1)
        self.assertEqual(runtime.calls, [])
        self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_ping_and_shutdown_require_empty_payloads(self):
        for message_type in ("ping", "shutdown"):
            with self.subTest(message_type=message_type):
                incoming = b"".join(
                    [
                        encoded_frame("ready", "startup", {}),
                        encoded_frame(
                            message_type,
                            f"{message_type}-1",
                            {"unexpected": True},
                        ),
                    ]
                )

                status, frames, diagnostics, runtime = self.run_worker(incoming)

                self.assertEqual(status, 2)
                self.assertEqual(len(frames), 1)
                self.assertEqual(runtime.calls, [])
                self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_eof_after_handshake_fails_closed(self):
        incoming = encoded_frame("ready", "startup", {})

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 2)
        self.assertEqual(len(frames), 1)
        self.assertEqual(runtime.calls, [])
        self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_execution_exception_becomes_sanitized_error(self):
        class FailingRuntime(FakeRuntime):
            def execute(self, args):
                raise RuntimeError("cookie=secret-token")

        request_temp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, request_temp, True)
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame(
                    "run",
                    "request-1",
                    {"args": ["cookie=secret-token"], "request_temp": request_temp},
                ),
                encoded_frame("shutdown", "shutdown-1", {}),
            ]
        )

        status, frames, diagnostics, _ = self.run_worker(
            incoming, FailingRuntime()
        )

        self.assertEqual(status, 0)
        self.assertEqual(frames[1]["message_type"], "error")
        self.assertEqual(
            frames[1]["payload"]["code"], "EXECUTION_FAILURE"
        )
        self.assertEqual(frames[2]["message_type"], "result")
        self.assertEqual(frames[2]["request_id"], "shutdown-1")
        self.assertNotIn("secret-token", json.dumps(frames))
        self.assertNotIn("secret-token", diagnostics)
        self.assertEqual(os.listdir(request_temp), [])

    def test_request_temp_directory_is_borrowed_and_restored(self):
        class TrackingRuntime(FakeRuntime):
            def execute(self, args):
                # While executing, the worker must have switched into the owned
                # request directory and pointed the temp variables at it.
                temp_dir = os.environ["TEMP"]
                self.observed_cwd = os.getcwd()
                marker = os.path.join(temp_dir, "owned-file.txt")
                with open(marker, "w", encoding="utf-8") as handle:
                    handle.write("owned\n")
                self.observed_temp = temp_dir
                return ytdlp_worker.ExecutionResult(
                    exit_code=0, stdout="tracked", stderr=""
                )

        request_temp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, request_temp, True)
        runtime = TrackingRuntime()
        original_cwd = os.getcwd()
        original_temp = os.environ.get("TEMP")
        self.addCleanup(os.chdir, original_cwd)
        self.addCleanup(
            lambda: os.environ.pop("TEMP", None)
            if original_temp is None
            else os.environ.__setitem__("TEMP", original_temp)
        )
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame(
                    "run",
                    "request-1",
                    {"args": ["--version"], "request_temp": request_temp},
                ),
                encoded_frame("shutdown", "shutdown-1", {}),
            ]
        )

        status, frames, diagnostics, _ = self.run_worker(incoming, runtime)

        self.assertEqual(status, 0)
        self.assertEqual(diagnostics, "")
        self.assertEqual(frames[1]["message_type"], "result")
        self.assertEqual(frames[1]["payload"]["stdout"], "tracked")
        # While executing the request the worker was inside the owned directory
        # with the temp variables pointing at it.
        self.assertEqual(os.path.abspath(runtime.observed_cwd), os.path.abspath(request_temp))
        self.assertEqual(os.path.abspath(runtime.observed_temp), os.path.abspath(request_temp))
        # The worker wrote into the owned directory; the controller removes it,
        # so the file is still present after the worker restores state.
        self.assertTrue(os.path.isfile(os.path.join(request_temp, "owned-file.txt")))
        # The worker restored its working directory and environment afterward.
        self.assertEqual(os.getcwd(), original_cwd)
        self.assertEqual(os.environ.get("TEMP"), original_temp)

    def test_empty_request_temp_fails_closed(self):
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame(
                    "run", "request-1", {"args": ["--version"], "request_temp": ""}
                ),
            ]
        )

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 2)
        self.assertEqual(len(frames), 1)
        self.assertEqual(frames[0]["message_type"], "hello")
        self.assertEqual(runtime.calls, [])
        self.assertEqual(diagnostics, "worker protocol failure\n")

    def test_unusable_request_temp_returns_structured_error(self):
        request_temp = str(Path(tempfile.gettempdir()) / "ytdlp-missing-parent")
        self.assertFalse(os.path.isdir(request_temp))
        incoming = b"".join(
            [
                encoded_frame("ready", "startup", {}),
                encoded_frame(
                    "run",
                    "request-1",
                    {"args": ["--version"], "request_temp": request_temp},
                ),
                encoded_frame("shutdown", "shutdown-1", {}),
            ]
        )

        status, frames, diagnostics, runtime = self.run_worker(incoming)

        self.assertEqual(status, 0)
        self.assertEqual(runtime.calls, [])
        self.assertEqual(frames[1]["message_type"], "error")
        self.assertEqual(
            frames[1]["payload"]["code"], "WORKER_REQUEST_TEMP_FAILURE"
        )
        self.assertEqual(frames[2]["message_type"], "result")


class FramingTests(unittest.TestCase):
    def test_partial_reader_and_unknown_fields(self):
        class PartialReader(io.BytesIO):
            def read(self, size=-1):
                if size > 1:
                    size = 1
                return super().read(size)

        frame = encoded_frame("ping", "ping-1", {})
        decoded = ytdlp_worker.read_frame(PartialReader(frame))
        self.assertEqual(decoded["message_type"], "ping")

        invalid = json.dumps(
            {
                "protocol_version": 1,
                "message_type": "ping",
                "request_id": "ping-1",
                "payload": {},
                "unexpected": True,
            }
        ).encode("utf-8")
        with self.assertRaises(ytdlp_worker.ProtocolError):
            ytdlp_worker.read_frame(
                io.BytesIO(struct.pack(">I", len(invalid)) + invalid)
            )

    def test_oversized_frame_is_rejected_before_payload_read(self):
        incoming = struct.pack(">I", ytdlp_worker.MAX_FRAME_BYTES + 1)
        with self.assertRaises(ytdlp_worker.ProtocolError):
            ytdlp_worker.read_frame(io.BytesIO(incoming))

    def test_empty_frame_unknown_message_and_non_object_payload_are_rejected(self):
        invalid_frames = [
            struct.pack(">I", 0),
            encoded_frame("unknown", "request-1", {}),
            encoded_frame("ping", "request-1", []),
        ]

        for incoming in invalid_frames:
            with self.subTest(incoming=incoming):
                with self.assertRaises(ytdlp_worker.ProtocolError):
                    ytdlp_worker.read_frame(io.BytesIO(incoming))


if __name__ == "__main__":
    unittest.main()
