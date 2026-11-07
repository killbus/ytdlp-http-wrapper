#!/usr/bin/env python3
"""CI-only offline load of installed EJS bundles under Deno, not challenge solving."""

import importlib.metadata
import subprocess
import sys


LOAD_MARKER = "YTDLP_EJS_ENTRYPOINT_LOADED"


def entrypoint_script():
    from yt_dlp_ejs.yt import solver

    if not importlib.metadata.version("yt-dlp-ejs"):
        raise RuntimeError("missing EJS distribution version")
    lib = solver.lib()
    core = solver.core()
    for name, bundle in (("lib", lib), ("core", core)):
        if not isinstance(bundle, str) or not bundle.strip():
            raise RuntimeError(f"missing or empty EJS {name} bundle")
    # Match upstream's package-bundle order; no player or challenge is fetched.
    return "\n".join((
        lib,
        "Object.assign(globalThis, lib);",
        core,
        'if (typeof jsc !== "function") { throw new Error("EJS jsc entrypoint missing"); }',
        f'console.log("{LOAD_MARKER}");',
    ))


def check_ejs():
    result = subprocess.run(
        [
            "deno", "run", "--no-config", "--no-lock", "--no-prompt",
            "--no-remote", "--no-npm", "--cached-only", "-",
        ],
        input=entrypoint_script(), check=True, capture_output=True, text=True,
        encoding="utf-8", timeout=30,
    )
    if LOAD_MARKER not in result.stdout.splitlines():
        raise RuntimeError("Deno exited without the EJS entrypoint-load marker")


def main():
    try:
        check_ejs()
    except (ImportError, OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"Offline EJS bundle-load smoke failed: {error}", file=sys.stderr)
        return 1
    print("Verified offline EJS lib/core entrypoint load under Deno (not challenge solving)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
