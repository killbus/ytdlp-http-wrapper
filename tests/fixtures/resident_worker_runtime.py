from __future__ import annotations

import json
import os
import platform
import subprocess
import struct
import sys
import time


def read_frame() -> dict[str, object] | None:
    header = sys.stdin.buffer.read(4)
    if not header:
        return None
    size = struct.unpack(">I", header)[0]
    payload = sys.stdin.buffer.read(size)
    if len(payload) != size:
        raise EOFError("truncated frame")
    return json.loads(payload)


def write_frame(
    message_type: str,
    request_id: str,
    payload: dict[str, object],
) -> None:
    encoded = json.dumps(
        {
            "protocol_version": 1,
            "message_type": message_type,
            "request_id": request_id,
            "payload": payload,
        },
        separators=(",", ":"),
    ).encode("utf-8")
    sys.stdout.buffer.write(struct.pack(">I", len(encoded)))
    sys.stdout.buffer.write(encoded)
    sys.stdout.buffer.flush()


worker_id = os.environ["YTDLP_WORKER_ID"]
write_frame(
    "hello",
    "startup",
    {
        "protocol_version": 1,
        "yt_dlp_version": "fixture-version",
        "python_version": platform.python_version(),
        "generation_id": os.environ["YTDLP_GENERATION_ID"],
        "worker_id": worker_id,
    },
)

ready = read_frame()
if ready is None or ready["message_type"] != "ready":
    raise RuntimeError("missing ready frame")

request = read_frame()
if request is None:
    raise RuntimeError("missing run frame")

request_id = str(request["request_id"])
if worker_id == "request-id-mismatch":
    request_id = "unexpected-request"
elif worker_id == "crash-eof":
    sys.exit(17)
elif worker_id == "unexpected-frame":
    write_frame("pong", request_id, {})
    sys.exit(0)
elif worker_id == "invalid-result-payload":
    write_frame(
        "result",
        request_id,
        {"exit_code": "invalid", "stdout": "", "stderr": ""},
    )
    sys.exit(0)
elif worker_id == "stderr-diagnostic":
    sys.stderr.write("fixture diagnostic\n")
    sys.stderr.flush()
    write_frame(
        "result",
        request_id,
        {"exit_code": 0, "stdout": "", "stderr": ""},
    )
    sys.exit(0)
elif worker_id == "oversized-stdout":
    write_frame(
        "result",
        request_id,
        {"exit_code": 0, "stdout": "x" * (10 * 1024 * 1024 + 1), "stderr": ""},
    )
    sys.exit(0)
elif worker_id == "business-error":
    while True:
        write_frame(
            "error",
            request_id,
            {"code": "FIXTURE_FAILURE", "message": "structured fixture failure"},
        )
        request = read_frame()
        if request is None:
            break
        request_id = str(request["request_id"])
        if request["message_type"] == "shutdown":
            write_frame("result", request_id, {"shutdown": True})
            break
elif worker_id == "manager-shutdown":
    if request["message_type"] != "shutdown":
        raise RuntimeError("manager shutdown fixture expected a shutdown frame")
    write_frame("result", request_id, {"shutdown": True})
    sys.exit(0)
elif worker_id == "request-timeout":
    time.sleep(30)
elif worker_id == "request-slow":
    # Long enough that a cancelled client must be what stops the request: the
    # request deadline used by the cancellation test is far shorter. Like the
    # real worker, this fixture does not read frames while it executes, so the
    # cooperative cancel frame cannot be proven to apply and the worker is
    # retired after the bounded grace period.
    time.sleep(120)
elif worker_id == "descendant-timeout":
    heartbeat = str(request["payload"]["args"][0])
    subprocess.Popen(
        [
            sys.executable,
            "-c",
            (
                "import pathlib,sys,time\n"
                "path = pathlib.Path(sys.argv[1])\n"
                "for value in range(3000):\n"
                "    path.write_text(str(value), encoding='utf-8')\n"
                "    time.sleep(0.02)\n"
            ),
            heartbeat,
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 5
    while not os.path.exists(heartbeat):
        if time.monotonic() >= deadline:
            raise RuntimeError("descendant did not start")
        time.sleep(0.01)
    time.sleep(30)
elif worker_id == "descendant-shutdown":
    heartbeat = str(request["payload"]["args"][0])
    write_frame(
        "result",
        request_id,
        {"exit_code": 0, "stdout": "ok\n", "stderr": ""},
    )
    shutdown = read_frame()
    if shutdown is None or shutdown["message_type"] != "shutdown":
        raise RuntimeError("missing shutdown frame")
    subprocess.Popen(
        [
            sys.executable,
            "-c",
            (
                "import pathlib,sys,time\n"
                "path = pathlib.Path(sys.argv[1])\n"
                "for value in range(3000):\n"
                "    path.write_text(str(value), encoding='utf-8')\n"
                "    time.sleep(0.02)\n"
            ),
            heartbeat,
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 5
    while not os.path.exists(heartbeat):
        if time.monotonic() >= deadline:
            raise RuntimeError("descendant did not start")
        time.sleep(0.01)
    write_frame("result", str(shutdown["request_id"]), {"shutdown": True})
    sys.exit(0)

write_frame(
    "result",
    request_id,
    {"exit_code": 0, "stdout": "ok\n", "stderr": ""},
)
