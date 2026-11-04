"""Deterministic `yt_dlp` stand-in for the resident-side contract fixtures.

`tests/backend_contract.rs` puts this directory on `PYTHONPATH` so the real
worker (`worker/ytdlp_worker.py`) imports it instead of the shared
`tests/fixtures/yt_dlp` module.

Single source of truth: instead of hand-writing a second copy of the expected
output, this module runs the very same fixture script the CLI backend spawns
(`tests/fixtures/contract_echo.py`) and returns its stdout, stderr and exit
code. Both backends therefore receive the same request arguments and produce
the same bytes by construction, so any difference the comparison test sees
comes from the wrapper, not from the fixtures.

The one exception is the `hang` mode, which blocks inside this module instead of
delegating to the script. A hanging request has no output to compare on either
side, and blocking in-process keeps the retirement path identical to the already
proven `request-timeout` fixture instead of adding a grandchild process tree that
only has to be reaped on one platform.
"""

from __future__ import annotations

import subprocess
import sys
import time
from types import SimpleNamespace

version = SimpleNamespace(__version__="fixture-version")


def parse_options(args):
    return SimpleNamespace(ydl_opts={"contract_args": list(args)}, urls=list(args))


class YoutubeDL:
    def __init__(self, options):
        self.urls = list(options.get("contract_args", []))

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        return False

    def download(self, urls):
        requested = list(urls)
        if "hang" in requested:
            time.sleep(120)
            return 0
        if not requested:
            raise ValueError("contract fixture expects the echo script path first")
        completed = subprocess.run(
            [sys.executable, *requested],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        # The worker captures these through redirected StringIO streams, which
        # never translate newlines. The echo script writes explicit b"\n", so
        # decoding preserves exactly what the CLI backend's pipe captures.
        sys.stdout.write(completed.stdout.decode("utf-8", "replace"))
        sys.stderr.write(completed.stderr.decode("utf-8", "replace"))
        return completed.returncode
