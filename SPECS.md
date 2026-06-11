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
|---|---|---|
| `src/main.rs` | Entrypoint: clap config, subscriber init, binary bootstrap, server start |
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
clap = { version = "4", features = ["derive", "env"] }

[patch.crates-io]
lofty = { git = "https://github.com/boul2gom/lofty-rs", rev = "d2e41640481a48a95303d95939ba831767afcec8" }
```

The `[patch.crates-io]` section pins a git fork of `lofty` (version 0.23.3) to work around a yank of `lofty = "0.23.2"` — a transitive dependency of `yt-dlp = "2.7.2"`. The fork satisfies `^0.23.2` and the `Cargo.lock` pins the exact revision.

### 5.2 yt-dlp Binary Bootstrap

On startup, `install_with_retry()` wraps `LibraryInstaller::install_youtube(None)` with up to 3 retries and exponential backoff (2s, 4s). The binary is downloaded from GitHub Releases, verified by SHA256 (where the release provides a digest), and saved to `./libs/`. This happens before the HTTP server binds.

---

## 6. Docker

### 6.1 Dockerfile

Multi-stage build: `rust:1.96-alpine` → `debian:bookworm-slim`.

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.96-alpine AS chef
RUN cargo install cargo-chef --version 0.1.77 --locked

FROM chef AS planner
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/ytdlp-http-wrapper /app/ytdlp-http-wrapper

FROM debian:bookworm-slim
ARG TARGETARCH

RUN <<'EOF'
set -eux
groupadd -r app && useradd -r -g app -d /app -s /sbin/nologin app
apt-get update
apt-get install -y --no-install-recommends \
    ca-certificates curl dumb-init ffmpeg \
    libnghttp2-14 python3 unzip zstd
apt-get clean
rm -rf /var/lib/apt/lists/*

case "$TARGETARCH" in
    amd64) DENO_ARCH="x86_64" ;;
    arm64) DENO_ARCH="aarch64" ;;
    *) echo "unsupported arch: $TARGETARCH" && exit 1 ;;
esac
curl -fsSL "https://github.com/denoland/deno/releases/latest/download/deno-$DENO_ARCH-unknown-linux-gnu.zip" -o /tmp/deno.zip
unzip /tmp/deno.zip -d /usr/local/bin/
rm /tmp/deno.zip

mkdir -p /app /downloads /app/.cache
chown -R app:app /app /downloads /app/.cache
EOF

COPY --link --from=builder /app/ytdlp-http-wrapper /app/ytdlp-http-wrapper
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
| `rust:1.96-alpine` builder | Produces statically-linked musl binary; cargo-chef splits dep/app compilation for CI layer caching |
| `cargo-chef` | Standard Rust Docker caching tool; `recipe.json` derived from Cargo.toml/lock only, dep layer stable across source changes |
| `debian:bookworm-slim` runtime | Native glibc environment for yt-dlp PyInstaller binary (downloaded at runtime via `LibraryInstaller`) |
| Deno from GitHub releases | Not available as a Debian package; TARGETARCH handles multi-arch |
| `dumb-init` | Proper signal handling for subprocesses |
| `USER app:app` (UID 1001) | Non-root with stable UID for volume mounts |
| `ffmpeg` + `python3` + `deno` | yt-dlp runtime requirements |
| BuildKit cache mounts | Cargo registry + target reuse across builds |
| `COPY --link` | Independent layer, unaffected by preceding RUN |
| ENV consolidated near ENTRYPOINT | Runtime-only, not consumed by build stages |

### 6.2 Deno Installation

Deno is downloaded from GitHub releases at build time, extracted from a zipped binary specific to `TARGETARCH`. This avoids depending on distribution-specific package availability.

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

### Binary Release (`release.yml`)

On tag `v*`:

1. `taiki-e/create-gh-release-action` creates a GitHub Release
2. `taiki-e/upload-rust-binary-action` compiles and uploads archives across 5 targets:

| Target | Runner | Archive format |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | `.tar.gz` |
| `x86_64-apple-darwin` | `macos-latest` | `.tar.gz` |
| `aarch64-apple-darwin` | `macos-latest` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` |

Linux arm64 is cross-compiled via `cross`; macOS/Windows targets compile natively on their respective runners. Archives are named `ytdlp-http-wrapper-$tag-$target.{tar.gz,zip}`.

---

## 8. Configuration

Configured via **environment variables** (Docker-friendly) or **CLI flags** (local dev). CLI flags take precedence when both are set.

### 8.1 CLI Flags

| Flag | Short | Env var | Default | Description |
|---|---|---|---|---|
| `--host` | — | `HOST` | `127.0.0.1` | Bind address |
| `--port` | `-p` | `PORT` | `8080` | Listen port |
| `--libs-dir` | `-l` | `LIBS_DIR` | `libs` | yt-dlp download directory |
| `--denied-args` | — | `DENIED_ARGS` | *(built-in list)* | JSON array of blocked args |
| `--help` | `-h` | — | — | Show help and exit |

### 8.2 Environment Variables (Reference)

| Variable | Default | CLI equivalent | Description |
|---|---|---|---|
| `HOST` | `127.0.0.1` | `--host` | Bind address |
| `PORT` | `8080` | `--port` | Listen port |
| `LIBS_DIR` | `libs` | `--libs-dir` | yt-dlp download directory |
| `RUST_LOG` | `info` | — | `EnvFilter` directive for tracing |
| `DENIED_ARGS` | *(built-in list)* | `--denied-args` | JSON array of blocked arguments; `[]` allows all |
| `SSL_CERT_FILE` | `/etc/ssl/certs/ca-certificates.crt` | — | (Docker only) Path to CA bundle |

---

## 9. Development

```bash
# quick start
scripts/dev.sh

# with custom config (env vars)
HOST=0.0.0.0 PORT=3000 LIBS_DIR=/tmp/libs DENIED_ARGS='[]' RUST_LOG=debug scripts/dev.sh

# with custom config (CLI flags)
cargo run -- --host 0.0.0.0 -p 3000 -l /tmp/libs

# CI validation (fmt + clippy + test)
scripts/ci.sh

# docker build
docker build -t ytdlp-http-wrapper .

# natively
cargo run
cargo test
cargo clippy -- -D warnings
```
