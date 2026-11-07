#!/usr/bin/env python3
"""Resident yt-dlp worker using a length-prefixed JSON protocol."""

from __future__ import annotations

import contextlib
import io
import json
import os
import platform
import struct
import sys
from dataclasses import dataclass
from typing import Any, BinaryIO, Callable, Mapping, Sequence


PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 128 * 1024 * 1024
STARTUP_REQUEST_ID = "startup"
MESSAGE_TYPES = frozenset(
    {
        "hello",
        "ready",
        "run",
        "result",
        "error",
        "cancel",
        "shutdown",
        "ping",
        "pong",
    }
)


class ProtocolError(Exception):
    """Raised when the controller violates the worker protocol."""


def _read_exact(stream: BinaryIO, size: int) -> bytes | None:
    chunks: list[bytes] = []
    remaining = size
    while remaining:
        chunk = stream.read(remaining)
        if not chunk:
            if not chunks:
                return None
            raise ProtocolError("truncated frame")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def read_frame(stream: BinaryIO) -> dict[str, Any] | None:
    prefix = _read_exact(stream, 4)
    if prefix is None:
        return None

    length = struct.unpack(">I", prefix)[0]
    if length == 0:
        raise ProtocolError("frame payload must not be empty")
    if length > MAX_FRAME_BYTES:
        raise ProtocolError("frame exceeds configured limit")

    encoded = _read_exact(stream, length)
    if encoded is None:
        raise ProtocolError("truncated frame payload")

    try:
        frame = json.loads(encoded.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProtocolError("invalid frame JSON") from error

    if not isinstance(frame, dict):
        raise ProtocolError("frame must be a JSON object")
    if set(frame) != {
        "protocol_version",
        "message_type",
        "request_id",
        "payload",
    }:
        raise ProtocolError("frame fields do not match protocol")
    if frame["protocol_version"] != PROTOCOL_VERSION:
        raise ProtocolError("protocol version mismatch")
    if frame["message_type"] not in MESSAGE_TYPES:
        raise ProtocolError("unknown message_type")
    if not isinstance(frame["request_id"], str) or not frame["request_id"]:
        raise ProtocolError("request_id must be a non-empty string")
    if not isinstance(frame["payload"], dict):
        raise ProtocolError("payload must be a JSON object")
    return frame


def write_frame(
    stream: BinaryIO,
    message_type: str,
    request_id: str,
    payload: Mapping[str, Any],
) -> None:
    if message_type not in MESSAGE_TYPES:
        raise ProtocolError("unknown message_type")
    if not request_id:
        raise ProtocolError("request_id must be non-empty")
    if not isinstance(payload, Mapping):
        raise ProtocolError("payload must be a mapping")

    encoded = json.dumps(
        {
            "protocol_version": PROTOCOL_VERSION,
            "message_type": message_type,
            "request_id": request_id,
            "payload": payload,
        },
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(encoded) > MAX_FRAME_BYTES:
        raise ProtocolError("frame exceeds configured limit")

    stream.write(struct.pack(">I", len(encoded)))
    stream.write(encoded)
    stream.flush()


@dataclass(frozen=True)
class ExecutionResult:
    exit_code: int
    stdout: str
    stderr: str


class YtDlpRuntime:
    """Small adapter around yt-dlp to keep worker tests dependency-free."""

    def __init__(self) -> None:
        import yt_dlp

        self._module = yt_dlp

    @property
    def version(self) -> str:
        version_module = getattr(self._module, "version", None)
        return str(getattr(version_module, "__version__", "unknown"))

    def execute(self, args: Sequence[str]) -> ExecutionResult:
        stdout = io.StringIO()
        stderr = io.StringIO()
        exit_code = 0

        # yt-dlp may write through Python stdout/stderr. Keep those streams away
        # from the binary protocol channel for the entire request.
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            try:
                parsed = self._module.parse_options(list(args))
                options = dict(parsed.ydl_opts)
                urls = list(parsed.urls)
                with self._module.YoutubeDL(options) as downloader:
                    exit_code = int(downloader.download(urls) or 0)
            except SystemExit as error:
                exit_code = int(error.code) if isinstance(error.code, int) else 1
            except Exception:
                exit_code = 1
                stderr.write("yt-dlp request failed\n")

        return ExecutionResult(
            exit_code=exit_code,
            stdout=stdout.getvalue(),
            stderr=stderr.getvalue(),
        )


class Worker:
    def __init__(
        self,
        input_stream: BinaryIO,
        output_stream: BinaryIO,
        diagnostic_stream: io.TextIOBase,
        runtime_factory: Callable[[], Any] = YtDlpRuntime,
        *,
        generation_id: str,
        worker_id: str,
    ) -> None:
        self._input = input_stream
        self._output = output_stream
        self._diagnostics = diagnostic_stream
        self._runtime_factory = runtime_factory
        self._generation_id = generation_id
        self._worker_id = worker_id

    def run(self) -> int:
        try:
            runtime = self._runtime_factory()
            write_frame(
                self._output,
                "hello",
                STARTUP_REQUEST_ID,
                {
                    "protocol_version": PROTOCOL_VERSION,
                    "yt_dlp_version": runtime.version,
                    "python_version": platform.python_version(),
                    "generation_id": self._generation_id,
                    "worker_id": self._worker_id,
                },
            )

            ready = read_frame(self._input)
            if (
                ready is None
                or ready["message_type"] != "ready"
                or ready["request_id"] != STARTUP_REQUEST_ID
                or ready["payload"] != {}
            ):
                raise ProtocolError("expected matching ready frame")

            while True:
                frame = read_frame(self._input)
                if frame is None:
                    raise ProtocolError("controller closed the protocol stream")

                message_type = frame["message_type"]
                request_id = frame["request_id"]
                if message_type == "run":
                    self._handle_run(runtime, request_id, frame["payload"])
                elif message_type == "ping":
                    self._require_empty_payload(frame["payload"], "ping")
                    write_frame(self._output, "pong", request_id, {})
                elif message_type == "shutdown":
                    self._require_empty_payload(frame["payload"], "shutdown")
                    write_frame(self._output, "result", request_id, {"shutdown": True})
                    return 0
                else:
                    raise ProtocolError("message is invalid while worker is idle")
        except ProtocolError:
            self._diagnostics.write("worker protocol failure\n")
            self._diagnostics.flush()
            return 2
        except Exception:
            self._diagnostics.write("worker startup failure\n")
            self._diagnostics.flush()
            return 3

    @staticmethod
    def _require_empty_payload(payload: Any, message_type: str) -> None:
        if payload != {}:
            raise ProtocolError(f"{message_type} payload must be empty")

    def _handle_run(
        self, runtime: Any, request_id: str, payload: Any
    ) -> None:
        if not isinstance(payload, dict):
            raise ProtocolError("run payload must be a JSON object")
        if "args" not in payload or set(payload) not in (
            {"args"},
            {"args", "request_temp"},
        ):
            raise ProtocolError("run payload fields do not match protocol")
        args = payload["args"]
        if not isinstance(args, list) or not all(
            isinstance(argument, str) for argument in args
        ):
            raise ProtocolError("run args must be a string array")
        request_temp = payload.get("request_temp")
        if request_temp is not None and (
            not isinstance(request_temp, str) or not request_temp
        ):
            raise ProtocolError("request_temp must be a non-empty path")

        # Per-request temp-directory ownership: the controller (Rust) creates the
        # owned temp directory and removes it after this request, mirroring the
        # CLI supervisor. The worker must not download into the controller's own
        # cwd, so it borrows the owned directory by switching into it and pointing
        # the process temp variables at it, then restores its working directory
        # and environment. Removal is owned by the controller, not the worker.
        previous_cwd = None
        previous_env = {}
        if request_temp is not None:
            previous_cwd = os.getcwd()
            previous_env = {key: os.environ.get(key) for key in ("TMPDIR", "TMP", "TEMP")}
            try:
                os.chdir(request_temp)
            except OSError:
                write_frame(
                    self._output,
                    "error",
                    request_id,
                    {
                        "code": "WORKER_REQUEST_TEMP_FAILURE",
                        "message": "request temporary directory is unavailable",
                    },
                )
                return
            for key in ("TMPDIR", "TMP", "TEMP"):
                os.environ[key] = request_temp

        def _restore() -> None:
            if previous_cwd is None:
                return
            for key, value in previous_env.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
            os.chdir(previous_cwd)

        try:
            result = runtime.execute(tuple(args))
        except Exception:
            _restore()
            write_frame(
                self._output,
                "error",
                request_id,
                {
                    "code": "EXECUTION_FAILURE",
                    "message": "yt-dlp request execution failed",
                },
            )
            return
        finally:
            _restore()

        write_frame(
            self._output,
            "result",
            request_id,
            {
                "exit_code": result.exit_code,
                "stdout": result.stdout,
                "stderr": result.stderr,
            },
        )


def main() -> int:
    generation_id = os.environ.get("YTDLP_GENERATION_ID", "")
    worker_id = os.environ.get("YTDLP_WORKER_ID", "")
    if not generation_id or not worker_id:
        sys.stderr.write("worker identity is not configured\n")
        return 3

    return Worker(
        sys.stdin.buffer,
        sys.stdout.buffer,
        sys.stderr,
        generation_id=generation_id,
        worker_id=worker_id,
    ).run()


if __name__ == "__main__":
    raise SystemExit(main())
