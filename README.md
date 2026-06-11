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

| Env | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Bind address |
| `PORT` | `8080` | Listen port |
| `LIBS_DIR` | `libs` | yt-dlp download directory |
| `RUST_LOG` | `info` | Tracing level (EnvFilter) |
| `DENIED_ARGS` | *(built-in list)* | Comma-separated args blacklist, `[]` to allow all |

Run with `--help` to print all env vars.

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
