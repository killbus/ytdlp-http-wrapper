# Technical Specification: ytdlp-http-wrapper

Lightweight HTTP wrapper service that bootstraps a `yt-dlp` binary at startup and exposes a `/run` endpoint to execute it with arbitrary arguments, returning JSON results.

---

## 1. Architecture

The service follows a **modular single-binary** pattern:

```
┌─ main.rs ──────────────────────────────────────────────┐
│  tracing_subscriber::fmt().json()                      │
│  LibraryInstaller → download yt-dlp to ./libs/         │
│     retry x3 with exponential backoff                  │
│  routes::app(binary_path) → axum::serve               │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─ routes.rs ───────────────────────────────────────────┐
│  Router::new().route("/run", get + post)               │
│  .layer(TraceLayer::new_for_http())                    │
└────────────────────────────────────────────────────────┘
         │
         ▼
┌─ executor.rs ─────────────────────────────────────────┐
│  reject_denied_args → 422 if blocked                  │
│  Command::new(binary).args(args).spawn()              │
│  stdout/stderr → take before wait                     │
│  timeout(duration, child.wait())                      │
│  on timeout: child.kill() + child.wait() to reap      │
│  redact_args for logs                                 │
│  #[cfg(windows)] creation_flags(0x08000000)           │
│  → (StatusCode, Json<RunResponse | ErrorResponse>)    │
└────────────────────────────────────────────────────────┘
```

### File Layout

| File | Responsibility |
|---|---|
| `src/main.rs` | Entrypoint: subscriber init, binary bootstrap, server start |
| `src/routes.rs` | Router factory with TraceLayer |
| `src/executor.rs` | Arg validation, process spawn, timeout, response construction |
| `src/models.rs` | `RunRequest`, `RunResponse`, `ErrorResponse` |

### Runtime Dependencies (system PATH)

- **Python 3** — yt-dlp script runtime
- **Deno (>= 2.x)** — JS sandbox for signature/decryption challenges
- **ffmpeg** — required by many yt-dlp post-processing operations

---

## 2. API

### `POST /run` | `GET /run`

Accepts both JSON POST and URL-encoded GET with repeatable `args` parameters.

#### Request

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `args` | `string[]` | yes | — | Raw CLI arguments forwarded to yt-dlp |
| `timeout_seconds` | `int` | no | `30` | Max execution time, clamped to `[1, 300]` |

POST:
```json
{
  "args": ["-g", "-f", "bestaudio", "https://www.youtube.com/watch?v=dQw4w9WgXcQ"],
  "timeout_seconds": 15
}
```

GET:
```http
GET /run?args=-g&args=-f&args=bestaudio&args=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DdQw4w9WgXcQ&timeout_seconds=15
```

#### Response (200 OK)

| Field | Type | Description |
|---|---|---|
| `exit_code` | `int` | Process exit code; `-1` on timeout |
| `stdout` | `string` | Captured stdout |
| `stderr` | `string` | Captured stderr |

```json
{ "exit_code": 0, "stdout": "https://rr3---sn-...", "stderr": "" }
```

On timeout:
```json
{ "exit_code": -1, "stdout": "", "stderr": "ERROR: Command execution timed out" }
```

#### Error Response (422 Unprocessable Entity)

Returned when one or more arguments match the `DENIED_ARGS` blocklist.

| `code` | Meaning |
|---|---|
| `ARG_REJECTED` | One or more args matched the blocklist |

```json
{ "error": "Argument '--exec' is not allowed by DENIED_ARGS policy", "code": "ARG_REJECTED" }
```

#### Error Response (500 Internal Server Error)

Returned when process spawn or output collection fails.

| `code` | Meaning |
|---|---|
| `SPAWN_FAILURE` | Failed to spawn yt-dlp process |
| `COLLECT_FAILURE` | Failed to read process output |

```json
{ "error": "Failed to spawn yt-dlp process: ...", "code": "SPAWN_FAILURE" }
```

---

## 3. Security

### 3.1 Argument Blocklist (`DENIED_ARGS`)

Defined at `src/executor.rs:17`. Default list:

| Argument | Reason |
|---|---|
| `--exec` / `--exec-before-download` | Arbitrary shell command execution |
| `--alias` | Command aliasing |
| `--config-locations` | Load external config |
| `--load-info-json` | Deserialize untrusted data |
| `--plugin-dirs` | Load external plugins |
| `--ffmpeg-location` | Override PATH binary |
| `--downloader-args` / `--postprocessor-args` | Pass arbitrary flags to subprocesses |

Configure via environment variable:
```bash
# custom blocklist
DENIED_ARGS='["--exec","--exec-before-download"]'

# allow all (dangerous)
DENIED_ARGS='[]'
```

### 3.2 Argument Redaction in Logs

The following argument prefixes have their value redacted in all log output (`src/executor.rs:57`):

- `--cookies-from-browser`, `--cookies`, `--load-cookies`
- `--add-header`, `--header`
- `--username`, `--password`, `--video-password`
- `--token`, `--api-key`

### 3.3 Network Isolation

- No authentication built in — assumed to sit behind a gateway or internal network
- Default bind: `127.0.0.1:8080`

---

## 4. Observability

### 4.1 Logging

All output is structured JSON via `tracing-subscriber`:

```json
{"level":"INFO","message":"Dependency ready","path":"libs/yt-dlp","target":"ytdlp_http_wrapper"}
```

Audit events carry `log_type = "audit"`:

```json
{"level":"WARN","log_type":"audit","args":["--cookies [REDACTED]"],"timeout_seconds":30,"message":"Args rejected by DENIED_ARGS policy: ..."}
```

### 4.2 Tracing

HTTP request tracing via `TraceLayer` at `debug_span!` level. Production `RUST_LOG=info` omits spans entirely; `RUST_LOG=debug` reveals full URI including query parameters.

| Span field | Source |
|---|---|
| `method` | `request.method()` |
| `uri` | `request.uri()` |
| `status` | Recorded on response |

---

## 5. Dependencies

### 5.1 Cargo.toml

```toml
[package]
name = "ytdlp-http-wrapper"
version = "1.0.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
yt-dlp = "2.7.2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tower-http = { version = "0.5", features = ["trace"] }

[patch.crates-io]
lofty = { git = "https://github.com/boul2gom/lofty-rs", rev = "d2e41640481a48a95303d95939ba831767afcec8" }
```

The `[patch.crates-io]` section pins a git fork of `lofty` (version 0.23.3) to work around a yank of `lofty = "0.23.2"` — a transitive dependency of `yt-dlp = "2.7.2"`. The fork satisfies `^0.23.2` and the `Cargo.lock` pins the exact revision.

### 5.2 yt-dlp Binary Bootstrap

On startup, `install_with_retry()` wraps `LibraryInstaller::install_youtube(None)` with up to 3 retries and exponential backoff (2s, 4s). The binary is downloaded from GitHub Releases, verified by SHA256 (where the release provides a digest), and saved to `./libs/`. This happens before the HTTP server binds.

---

## 6. Docker

### 6.1 Dockerfile

Multi-stage build: `rust:1.96-slim-bookworm` → `alpine:3.21`.

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.96-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release

FROM alpine:3.21
RUN <<'EOF'
adduser -D -h /app -u 1001 app
apk update && apk upgrade -a
apk add --no-cache \
    ca-certificates \
    dumb-init \
    ffmpeg \
    nghttp2 \
    python3 \
    zstd
apk add --no-cache -X https://dl-cdn.alpinelinux.org/alpine/edge/community \
    deno
rm -rf /var/cache/apk/*
mkdir -p /app libs /downloads /app/.cache
chown -R app:app /app /downloads /app/.cache
EOF
COPY --link --from=builder /app/target/release/ytdlp-http-wrapper /app/ytdlp-http-wrapper
USER app:app
WORKDIR /downloads
VOLUME ["/downloads"]
EXPOSE 8080
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    RUST_LOG=info
ENTRYPOINT ["dumb-init", "/app/ytdlp-http-wrapper"]
```

Key design choices:

| Choice | Rationale |
|---|---|
| `alpine:3.21` runtime | Minimal image (~20 MB + deps) |
| `dumb-init` | Proper signal handling for subprocesses |
| `USER app:app` (UID 1001) | Non-root with stable UID for volume mounts |
| `ffmpeg` + `python3` + `deno` | yt-dlp runtime requirements |
| BuildKit cache mounts | Cargo registry + target reuse across builds |
| `COPY --link` | Independent layer, unaffected by preceding RUN |
| ENV consolidated near ENTRYPOINT | Runtime-only, not consumed by build stages |

### 6.2 Caveat: No Deno binary in Alpine (pre-3.22)

The `deno` package is pulled from Alpine's `edge/community` repository. Alpine 3.22+ will include it in the main repository. This is a temporary workaround.

### 6.3 .dockerignore

```
.git
target
.env
third_party
*.md
```

Prevents build context bloat and accidental leakage.

---

## 7. CI/CD

### CI (`ci.yml`)

On push/PR to `main`:

1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`
4. Docker build (BuildKit, GHA cache, load to local)

### Docker Push (`docker.yml`)

On push to `main` or tag `v*`:

| Event | Registry | Tags |
|---|---|---|
| `main` branch | GHCR (always) / Docker Hub (conditional) | `sha-<short>` |
| `v*` tag | GHCR (always) / Docker Hub (conditional) | `semver`, `semver.x.y`, `latest` |

**Flow:** single `docker/build-push-action` pushes to both registries from one build.

| Registry | Image pattern |
|---|---|
| GHCR | `ghcr.io/${{ github.repository }}` |
| Docker Hub | `${{ github.repository }}` |

Requires secrets `DOCKERHUB_USERNAME` and `DOCKERHUB_PASSWORD` for Docker Hub pushes. GHCR uses `secrets.GITHUB_TOKEN`.

---

## 8. Environment Variables

| Variable | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Bind address |
| `PORT` | `8080` | Listen port |
| `RUST_LOG` | `info` | `EnvFilter` directive for tracing |
| `DENIED_ARGS` | *(built-in list)* | JSON array of blocked arguments; `[]` allows all |
| `SSL_CERT_FILE` | `/etc/ssl/certs/ca-certificates.crt` | (Docker only) Path to CA bundle |

---

## 9. Development

```bash
# build and run natively
cargo run

# with custom config
HOST=0.0.0.0 PORT=3000 DENIED_ARGS='[]' RUST_LOG=debug cargo run

# docker build
docker build -t ytdlp-http-wrapper .

# tests
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
