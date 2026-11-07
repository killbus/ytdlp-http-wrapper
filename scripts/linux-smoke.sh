#!/usr/bin/env bash
set -euo pipefail
# Supply a verified official yt-dlp_linux binary; this script performs no downloads.
: "${YTDLP_SMOKE_BINARY:?Set YTDLP_SMOKE_BINARY to the official yt-dlp_linux executable}"
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    echo 'Official yt-dlp_linux smoke requires native x86_64 Linux.' >&2
    exit 1
fi
if [[ $YTDLP_SMOKE_BINARY != /* || ! -x $YTDLP_SMOKE_BINARY ]]; then
    echo 'YTDLP_SMOKE_BINARY must be an absolute path to a verified executable.' >&2
    exit 1
fi
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
host=$(rustc -vV | awk '$1 == "host:" { print $2 }')
if [[ ! $host =~ ^x86_64-unknown-linux-[a-z0-9]+$ ]]; then
    echo 'Smoke tests require a native x86_64 Linux Rust host toolchain.' >&2
    exit 1
fi
"$YTDLP_SMOKE_BINARY" --version
cargo test --locked --target "$host" --test linux_ytdlp_smoke -- --ignored --nocapture
