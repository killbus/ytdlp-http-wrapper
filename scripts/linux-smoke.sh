#!/usr/bin/env bash
set -euo pipefail
# Supply a verified official yt-dlp_linux binary; this script performs no downloads.
: "${YTDLP_SMOKE_BINARY:?Set YTDLP_SMOKE_BINARY to the official yt-dlp_linux executable}"
"$YTDLP_SMOKE_BINARY" --version
cargo test --locked --test linux_ytdlp_smoke -- --ignored --nocapture
