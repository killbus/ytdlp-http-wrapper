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
| `RUST_LOG` | `info` | Tracing level (EnvFilter) |
| `DENIED_ARGS` | *(built-in list)* | Comma-separated args blacklist, `[]` to allow all |

## Build

```bash
cargo build --release
docker build -t ytdlp-http-wrapper .
```

See [SPECS.md](SPECS.md) for full technical specification.
