# ytdlp-http-wrapper

Lightweight HTTP wrapper that executes `yt-dlp` commands and returns JSON results. Self-bootstraps the yt-dlp binary on startup — no manual installation needed.

## Quick Start

```bash
docker run -d -p 127.0.0.1:8080:8080 \
  -e HOST=0.0.0.0 -e MAX_CONCURRENT_PROCESSES=4 \
  --tmpfs /tmp:rw,exec,nosuid,nodev,size=512m,mode=1777 \
  --memory 1g --stop-timeout 15 \
  ghcr.io/killbus/ytdlp-http-wrapper

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

Timeouts return HTTP 200 with `exit_code: -1`. The 1–300 second execution
budget excludes queueing and includes output pipe completion. Termination can
add 3 seconds of Unix SIGTERM grace plus up to 2 seconds for forced process
cleanup; filesystem cleanup and response delivery take additional time. Windows
terminates the JobObject immediately. Each output stream retains its first
10 MiB and drains excess bytes without retaining them.

## Configuration

All options accept CLI flags (local dev) or environment variables (Docker). CLI flags take precedence.

| Flag | Short | Env | Default | Description |
|---|---|---|---|---|
| `--host` | — | `HOST` | `127.0.0.1` | Bind address |
| `--port` | `-p` | `PORT` | `8080` | Listen port |
| `--libs-dir` | `-l` | `LIBS_DIR` | `libs` | yt-dlp download directory |
| `--max-concurrent` | — | `MAX_CONCURRENT_PROCESSES` | CPU×2 | Max concurrent yt-dlp processes |
| `--temp-dir` | — | `YTDLP_TEMP_DIR` | OS temp directory | Existing parent for isolated request temporary files |
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
docker run -d -p 127.0.0.1:8080:8080 \
  -e HOST=0.0.0.0 -e MAX_CONCURRENT_PROCESSES=4 \
  --tmpfs /tmp:rw,exec,nosuid,nodev,size=512m,mode=1777 \
  --memory 1g --stop-timeout 15 \
  ghcr.io/killbus/ytdlp-http-wrapper
```

For a local build with the same limits and a downloads volume:

```bash
docker compose up --build -d
```

See [process lifecycle operations](docs/process-lifecycle.md) for memory sizing,
shutdown, residue migration and Linux validation. These settings take effect
when the image/configuration is rebuilt and deployed.

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
