from __future__ import annotations

import json
import os
import platform
import struct
import sys


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


generation_id = os.environ["YTDLP_GENERATION_ID"]
worker_id = os.environ["YTDLP_WORKER_ID"]

if worker_id == "generation-mismatch":
    generation_id = "actual-generation"
elif worker_id == "worker-id-mismatch":
    worker_id = "actual-worker"

message_type = "result" if worker_id == "non-hello-first-frame" else "hello"
request_id = (
    "unexpected-startup"
    if worker_id == "startup-request-id-mismatch"
    else "startup"
)
payload = {
    "protocol_version": 1,
    "yt_dlp_version": "fixture-version",
    "python_version": platform.python_version(),
    "generation_id": generation_id,
    "worker_id": worker_id,
}
if worker_id == "invalid-hello-payload":
    payload["unexpected"] = True

write_frame(
    message_type,
    request_id,
    payload,
)
