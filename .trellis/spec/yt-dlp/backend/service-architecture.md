# Service Architecture: ytdlp-http-wrapper

> Status: 🏗️ Specifying
> Target: Define the complete structural architecture of the HTTP wrapper service, covering module boundaries, request lifecycle, configuration, error model, and defense-in-depth.

---

## 1. System Overview

```
                    ┌──────────────────────────────┐
                    │      External Client          │
                    │  (curl, browser, app, etc.)   │
                    └──────────────┬───────────────┘
                                   │ HTTP (GET/POST /run)
                                   ▼
                    ┌──────────────────────────────┐
                    │   Axum HTTP Server            │
                    │   (tokio async runtime)       │
                    │   bind: HOST:PORT             │
                    └──────────────┬───────────────┘
                                   │
                                   ▼
                    ┌──────────────────────────────┐
                    │   Router + TraceLayer         │
                    │   routes.rs                   │
                    │   - /run (GET, POST)          │
                    └──────────────┬───────────────┘
                                   │ State(Arc<AppState>)
                                   ▼
                    ┌──────────────────────────────┐
                    │   Executor                    │
                    │   executor.rs                 │
                    │   - arg validation            │
                    │   - Semaphore acquire         │
                    │   - process-wrap chain        │
                    │   - stdout/stderr drain       │
                    │   - timeout + collection      │
                    └──────────────┬───────────────┘
                                   │ spawn
                                   ▼
                    ┌──────────────────────────────┐
                    │   OS Subprocesses             │
                    │   yt-dlp → ffmpeg / aria2c    │
                    │   (managed by process-wrap)   │
                    └──────────────────────────────┘
```

### Container Boundary

```
┌──────────────────────────────────────────────┐
│  Docker Container (debian:bookworm-slim)      │
│                                               │
│  ┌──────────┐                                 │
│  │ dumb-init│  PID 1                          │
│  │ (reaper) │                                 │
│  └────┬─────┘                                 │
│       │ SIGTERM forward                       │
│       ▼                                       │
│  ┌────────────────┐                           │
│  │ ytdlp-http-    │  PID >1                   │
│  │ wrapper        │                           │
│  │ (tokio+axum)   │                           │
│  └───┬────┬────┬──┘                           │
│      │    │    │  yt-dlp / ffmpeg             │
│      │    │    │  (process-wrap group)        │
│      ▼    ▼    ▼                              │
│  ┌─────────────────────────────────────┐      │
│  │  Ephemeral Subprocesses             │      │
│  │  (killed on Drop / timeout)          │      │
│  └─────────────────────────────────────┘      │
└──────────────────────────────────────────────┘
```

---

## 2. Module Boundaries & Responsibilities

### `src/main.rs` — Entrypoint & Wiring

| Responsibility | Detail |
|---|---|
| CLI parsing | `clap::Parser` for `--host`, `--port`, `--libs-dir`, `--denied-args` |
| yt-dlp bootstrap | `LibraryInstaller` with exponential backoff retry (3 attempts) |
| Semaphore init | Read `MAX_CONCURRENT_DOWNLOADS` env var, fallback to `CPU*2` |
| Server start | `axum::serve(listener, app).await` |
| Graceful shutdown | Implicit via tokio signal handling (can be enhanced) |

### `src/routes.rs` — HTTP Routing & State

| Responsibility | Detail |
|---|---|
| Route registration | `/run` GET + POST |
| State carrier | `AppState { binary_path, semaphore }` via `axum::extract::State` |
| Middleware | `TraceLayer` for request logging |
| Handler dispatch | Decompresses query/body into `RunRequest`, delegates to `executor::execute` |

### `src/executor.rs` — Core Execution Engine

| Responsibility | Detail |
|---|---|
| Arg validation | `reject_denied_args()` — block dangerous yt-dlp flags |
| Arg redaction | `redact_args()` — mask secrets in logs |
| Concurrency control | `Semaphore::acquire_owned()` — queuing/backpressure |
| Process lifecycle | `process-wrap` chain: `KillOnDrop` + `ProcessGroup`/`JobObject` |
| I/O management | Async drain of stdout/stderr via `tokio::spawn` + `take(10MB)` |
| Timeout handling | `tokio::time::timeout` + explicit kill |
| Response assembly | `RunResponse { exit_code, stdout, stderr }` |

### `src/models.rs` — Data Types

| Type | Purpose |
|---|---|
| `RunRequest` | Input: `args`, `timeout_seconds` (query string or JSON body) |
| `RunResponse` | Output: `exit_code`, `stdout`, `stderr` |
| `ErrorResponse` | Error: `error`, `code` |

---

## 3. Request Lifecycle (Full Path)

```
Phase 1: CONNECTION
  Client → TCP connect → Axum accept → HTTP parse
  → TraceLayer span created

Phase 2: ROUTING
  Router matches /run
  → GET: Query<RunRequest> extracted
  → POST: Json<RunRequest> extracted
  → State<Arc<AppState>> injected

Phase 3: SEMAPHORE (backpressure gate)
  semaphore.acquire_owned().await
  ├── Permit available → proceed immediately
  └── Permit exhausted → Future yields, parked in FIFO queue
       └── Client disconnects → Future Drop → acquire cancelled → 0 leak

Phase 4: VALIDATION
  reject_denied_args(payload.args)
  ├── Rejected → return 422 ARG_REJECTED, permit dropped (released)
  └── OK → proceed

Phase 5: SPAWN
  CommandWrap::new(raw_cmd)
    .wrap(KillOnDrop)                          # lifecycle interceptor
    ┌── #[cfg(unix)] .wrap(ProcessGroup::leader())  # setpgid(0,0)
    └── #[cfg(windows)] .wrap(JobObject)             # CREATE_SUSPENDED + Job bind
  → wrap.spawn()

Phase 6: STREAM
  child.stdout.take() → tokio::spawn drain (limit 10MB)
  child.stderr.take() → tokio::spawn drain (limit 10MB)
  → Concurrent: child.wait() + drain tasks

Phase 7: COMPLETION
  tokio::time::timeout(duration, child.wait()).await
  ├── Ok(Ok(status)) → normal exit → disarm → collect output → 200 OK
  ├── Ok(Err(e))     → io error      → collect output → 500 COLLECT_FAILURE
  └── Err(_)         → timeout       → kill + wait    → 200 OK (exit=-1)

Phase 8: CLEANUP (RAII, unconditional on scope exit)
  ┌── Semaphore permit dropped → permit returned to pool
  └── Child dropped
      ├── Unix: KillOnDrop → ProcessGroupChild → kill(-pgid, SIGKILL)
      └── Windows: KillOnDrop → JobObjectChild → CloseHandle(job) → kernel kills all
```

### Client Disconnect (Mid-Request)

```
At any .await point in Phase 3-7:
  Client drops TCP connection
  → Axum cancels handler Future
  → Phase 8 CLEANUP fires immediately
  → No orphan processes, no leaked permits
```

---

## 4. Configuration Surface

| Env / CLI | Default | Description |
|---|---|---|
| `HOST` / `--host` | `127.0.0.1` | Bind address |
| `PORT` / `-p` | `8080` | Bind port |
| `LIBS_DIR` / `-l` | `libs` | yt-dlp download directory |
| `DENIED_ARGS` / `--denied-args` | *(built-in list)* | JSON array of blocked yt-dlp args |
| `MAX_CONCURRENT_DOWNLOADS` | `CPU*2` | Max parallel subprocesses (via Semaphore) |
| `RUST_LOG` | `info` | Tracing/logging level |

Built-in denied args: `--exec`, `--exec-before-download`, `--alias`, `--config-locations`, `--load-info-json`, `--plugin-dirs`, `--ffmpeg-location`, `--downloader-args`, `--postprocessor-args`

---

## 5. Error Model

| HTTP Status | `code` | Scenario |
|---|---|---|
| 422 | `ARG_REJECTED` | Client passed a denied argument |
| 500 | `SPAWN_FAILURE` | OS refused to spawn process (FD exhaustion, etc.) |
| 500 | `COLLECT_FAILURE` | IO error reading stdout/stderr after spawn |
| 200 | *(exit=-1)* | Timeout: process killed, partial output returned |
| 200 | *(exit=0..255)* | Normal: process exit code + full output |

All errors are returned as JSON `{ "error": "...", "code": "..." }`.

---

## 6. Defense-in-Depth Matrix

| # | Disaster (from 001) | Primary Defense | Module | Secondary Defense | Mechanism |
|---|---|---|---|---|---|
| 1 | **Orphan zombie storm** (subprocess leak) | `process-wrap` chain: KillOnDrop + ProcessGroup/JobObject | executor.rs | `dumb-init` PID 1 reaper | Unix: `kill(-pgid, SIGKILL)`. Windows: `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |
| 2 | **Concurrency/memory avalanche** (OOM) | `tokio::sync::Semaphore` (MAX_CONCURRENT_DOWNLOADS) | executor.rs + main.rs | `take(10MB)` output cap | RAII permit: Future drop → permit auto-return |
| 3 | **Pipe buffer deadlock** (64KB) | Async concurrent drain via `tokio::spawn` | executor.rs | `take(MAX_OUTPUT_BYTES)` | stdout/stderr drained independently of `child.wait()` |
| 4 | **Tokio runtime starvation** | Semaphore limits concurrent `fork()`; `spawn_blocking` for CPU work | executor.rs | N/A | Max parallel forks ≤ MAX_CONCURRENT; CPU work off main thread |
| 5 | **Heap memory ballooning** | `take(10MB)` + bounded `Vec<u8>` | executor.rs | *(future: jemalloc)* | Per-request cap prevents unbounded allocation |

---

## 7. Graceful Shutdown Sequence

```
SIGTERM (from Docker/K8s)
  │
  ▼
dumb-init (PID 1) forwards SIGTERM to ytdlp-http-wrapper (PID >1)
  │
  ├── Axum initiates graceful shutdown (stops accepting new connections)
  ├── Waits for in-flight requests (up to K8s terminationGracePeriodSeconds, default 30s)
  │
  ├── In-flight requests complete normally
  │   └── RAII Drop releases permits + kills process groups
  │
  └── Grace period expires
      └── Axum forces remaining Future cancellation
          └── KillOnDrop Drop → kills all remaining yt-dlp/ffmpeg via pgid/job
          └── Semaphore permits dropped (already draining)
```

**Invariant:** At container shutdown, **zero orphan processes remain**. Either they completed naturally, were killed by KillOnDrop, or are reaped by dumb-init before exit.

---

## 8. Architecture Convergence Status & Change Map

### Current
- ✅ Async pipe draining (Disaster 3)
- ✅ Output size cap (Disaster 5)
- ✅ Arg validation + redaction
- ✅ Structured logging (tracing + json)
- ✅ `dumb-init` PID 1 reaper (Disaster 1 mitigation)
- ⏳ `process-wrap` chain + Semaphore → tracked in `todos/002-performance-incidents/009-final-architecture-blueprint.md`

### Decision Log (P0)

| Decision | Rationale | Source |
|---|---|---|
| `unsafe_code = "forbid"` → reject self-implemented FFI | Project-level lint prevents `unsafe` usage, even if correct | `Cargo.toml:22-24` |
| `process-wrap` over `command-group` | `command-group` is archived/abandoned | 004-industrial-comparison |
| `process-wrap` over self-implemented | 0 additional `unsafe` in project code; inherits production-tested edge cases | 003-independent-audit, 008-TEAM-B |
| `KillOnDrop` before `ProcessGroup`/`JobObject` | JobObject reads `KillOnDrop` flag from wrapper chain to activate `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` | process-wrap source code |
| Semaphore `acquire_owned` (queue) over `try_acquire` (429) | Backward-compatible; preserves existing timeout-based behavior | design.md |
| CPU*2 as default MAX_CONCURRENT | Conservative enough to avoid `fork()` blocking Tokio worker pool | 005-concurrency-evaluation |

### Out of Scope (Future)

- jemalloc allocator (Disaster 5 hardening)
- `/health` endpoint with Semaphore saturation metric
- SSE/WebSocket streaming (alternative to buffered response)
- Connection pooling / per-IP rate limiting (beyond process Semaphore)
