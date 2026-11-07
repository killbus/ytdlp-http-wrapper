"""Deterministic local fixture shared by both backends in `tests/backend_contract.rs`.

Design section 9 requires the resident-vs-CLI comparison to run *the same* local
deterministic fixture arguments through both backends. This script is that
fixture, and it is the single source of truth for the expected bytes:

- the CLI backend spawns it directly (`binary_path` = python, args = [script, mode])
- the resident backend reaches it through
  `tests/fixtures/contract_runtime/yt_dlp/__init__.py`, which spawns the very
  same script with the very same arguments and returns its output

so the two responses can only differ where the wrapper itself differs.

Output is written through the binary buffers with explicit `b"\n"`: a text-mode
write would be translated to `b"\r\n"` on Windows by the CLI backend's pipe
capture, while the resident side re-decodes captured bytes. Writing bytes keeps
the comparison exact on every platform.

modes:
- `success` one stdout line, one stderr line, exit 0
- `hang`    mark itself ready in the owned request directory, then sleep far
           longer than any test deadline
- `fail`    one stderr line, exit 1
"""

from __future__ import annotations

import os
import pathlib
import sys
import time


def owned_request_dir() -> pathlib.Path:
    """The per-request temp directory both backends hand to the fixture.

    The CLI supervisor exports it as TMPDIR/TMP/TEMP without changing cwd; the
    resident worker exports the same variables and also chdirs into it.
    """
    for key in ("TMPDIR", "TMP", "TEMP"):
        value = os.environ.get(key)
        if value:
            return pathlib.Path(value)
    return pathlib.Path.cwd()


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else ""

    if mode == "hang":
        # Lets the test wait until the request is genuinely executing instead of
        # racing process start-up.
        try:
            (owned_request_dir() / "ready").write_text("ready", encoding="utf-8")
        except OSError:
            pass
        time.sleep(120)
        return 0
    if mode == "fail":
        sys.stderr.buffer.write(b"contract business failure\n")
        sys.stderr.buffer.flush()
        return 1

    sys.stdout.buffer.write(b"contract stdout\n")
    sys.stdout.buffer.flush()
    sys.stderr.buffer.write(b"contract stderr\n")
    sys.stderr.buffer.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
