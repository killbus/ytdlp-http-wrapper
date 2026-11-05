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
if worker_id == "slow-start":
    # Delay hello so both boot and lazy replacement pay the handshake cost.
    time.sleep(2.0)
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


def sleep_arg(args: list[str]) -> float | None:
    """Read the controllable per-scenario sleep from the request args.

    The slow-request pool scenarios drive the duration with a leading
    `sleep:<seconds>` argument, so concurrency and saturation tests can pick
    their timing margins without new fixture scenarios.
    """
    for argument in args:
        if argument.startswith("sleep:"):
            return float(argument[len("sleep:"):])
    return None


def run_default(request_id: str, args: list[str]) -> None:
    """Write a default-scenario result with the worker_id in-band.

    The result frame's stdout field is the in-band channel the Rust pool
    tests read to learn which worker served the request (never plain-text
    stdout, which is the binary protocol channel). Request args optionally
    drive a `sleep:<seconds>` delay and a `crash:eof` abort (immediate EOF
    on the result, the same semantics as the crash-eof worker_id scenario).
    """
    for argument in args:
        if argument == "crash:eof":
            sys.exit(17)
    delay = sleep_arg(args)
    if delay is not None:
        time.sleep(delay)
    write_frame(
        "result",
        request_id,
        {
            "exit_code": 0,
            "stdout": f"{worker_id}:{':'.join(args)}\n",
            "stderr": "",
        },
    )


def next_default_frame() -> dict | None:
    """Read the next frame while a default-scenario worker is alive.

    Mirrors the real worker's protocol loop: a shutdown frame is answered
    with its acknowledgement and ends the worker. A fresh worker that never
    handled a run request therefore receives shutdown as its first request
    and still shuts down cleanly.
    """
    frame = read_frame()
    if frame is None or frame["message_type"] != "run":
        if frame is not None and frame["message_type"] == "shutdown":
            write_frame("result", str(frame["request_id"]), {"shutdown": True})
        return None
    return frame


def run_args(frame: dict) -> list[str]:
    return [str(a) for a in frame["payload"]["args"]]


if request["message_type"] == "shutdown":
    # A fresh worker whose first frame is a shutdown (it never handled a run
    # request) is still answered, mirroring the real worker's protocol loop.
    write_frame("result", str(request["request_id"]), {"shutdown": True})
else:
    # The first request is `request`; its request_id may have been reassigned
    # by a scenario above (e.g. request-id-mismatch), so it is not re-derived
    # from the frame. Further run frames keep the warm worker serving serial
    # requests.
    run_default(request_id, run_args(request))
    while (frame := next_default_frame()) is not None:
        run_default(str(frame["request_id"]), run_args(frame))
