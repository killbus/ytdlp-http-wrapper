# ytdlp-http-wrapper

Lightweight HTTP wrapper that executes `yt-dlp` commands and returns JSON results. Self-bootstraps the yt-dlp binary on startup — no manual installation needed.

## Quick Start

```bash
docker run -d -p 8080:8080 ghcr.io/killbus/ytdlp-http-wrapper

# download audio from YouTube
curl -s -X POST http://localhost:8080/run \
  -H "Content-Type: application/json" \
  -d '{"args": ["-f", "bestaudio", "https://www.youtube.com/watch?v=dQw4w9WgXcQ"]}'
```

## API

### `POST /run` | `GET /run`

| Field | Type | Required | Default |
|---|---|---|---|
| `args` | `string[]` | yes | — |
| `timeout_seconds` | `int` | no | `30` |

Response `200 OK`:
```json
{ "exit_code": 0, "stdout": "...", "stderr": "" }
```

## Configuration

All options accept CLI flags (local dev) or environment variables (Docker). CLI flags take precedence.

| Flag | Short | Env | Default | Description |
|---|---|---|---|---|
| `--host` | — | `HOST` | `127.0.0.1` | Bind address |
| `--port` | `-p` | `PORT` | `8080` | Listen port |
| `--libs-dir` | `-l` | `LIBS_DIR` | `libs` | yt-dlp download directory |
| `--denied-args` | — | `DENIED_ARGS` | *(built-in list)* | JSON blocklist; `[]` to allow all |
| | | `RUST_LOG` | `info` | Tracing level (EnvFilter) |

```bash
# with CLI flags
cargo run -- --host 0.0.0.0 -p 3000 -l /tmp/libs

# with env vars (Docker style)
HOST=0.0.0.0 PORT=3000 cargo run
```

## Installation

### Docker (recommended)

```bash
docker run -d -p 8080:8080 ghcr.io/killbus/ytdlp-http-wrapper
```

### Binary (Linux only)

Download from [GitHub Releases](https://github.com/killbus/ytdlp-http-wrapper/releases):

```bash
curl -fsSL https://github.com/killbus/ytdlp-http-wrapper/releases/latest/download/ytdlp-http-wrapper-<tag>-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz
./ytdlp-http-wrapper
```

macOS/Windows users should use the Docker image.

## Development

```bash
scripts/dev.sh          # run locally
scripts/ci.sh           # fmt + clippy + test
```

## Build

```bash
cargo build --release
docker build -t ytdlp-http-wrapper .
```

See [SPECS.md](SPECS.md) for full technical specification.
