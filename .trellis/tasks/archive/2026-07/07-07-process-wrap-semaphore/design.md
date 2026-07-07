# Design: Performance & Concurrency Refactor

## Architecture Overview

### Before (Current)

```
[Axum Route (closure)] → execute(RunRequest, &PathBuf)
                              │
                              ├── cmd.kill_on_drop(true)      // kills direct child only
                              ├── cmd.creation_flags(...)      // Windows manual mgmt
                              ├── cmd.spawn()
                              ├── tokio::spawn drain stdout/stderr
                              └── timeout + child.kill() + child.wait()
```

Issues: no concurrency throttling; `ffmpeg` subprocesses escape on timeout/cancellation; routes use closure capture, hard to extend

### After (Target)

```
[Axum Route (State)] → acquire Semaphore Permit (queue/throttle)
                              │
                      process-wrap Chain
                              │
                    ┌─ KillOnDrop (lifecycle interceptor)
                    ├─ ProcessGroup::leader() [unix] / JobObject [windows]
                    │
                    ├── wrap.spawn()
                    ├── tokio::spawn drain stdout/stderr
                    └── timeout or Drop → KillOnDrop cascade terminate
```

## Component Boundary & Contract

### New `routes::AppState`

```rust
pub struct AppState {
    pub binary_path: PathBuf,
    pub semaphore: Arc<Semaphore>,
}
```
- Initialized in `main.rs`, injected into `routes::app()` via `Router::with_state()`
- Handler signature changes from closure capture to `extract::State<Arc<AppState>>`

### process-wrap chain (executor.rs)

Fixed order:
1. `CommandWrap::new(raw_cmd)`
2. `.wrap(KillOnDrop)` — sets kill_on_drop flag; `JobObject` depends on this to activate `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
3. `#[cfg(unix)] .wrap(ProcessGroup::leader())` — runs `setpgid(0, 0)` in `pre_exec`
4. `#[cfg(windows)] .wrap(JobObject)` — unit struct, handles `CREATE_SUSPENDED` + Job binding internally

Removed from `executor.rs`:
- `cmd.kill_on_drop(true)` — replaced by KillOnDrop
- `#[cfg(windows)] cmd.creation_flags(0x08000000)` — replaced by JobObject

### Semaphore throttling

```rust
// main.rs initialization
let max_concurrent = env::var("MAX_CONCURRENT_PROCESSES")
    .ok().and_then(|v| v.parse().ok())
    .unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get() * 2)
            .unwrap_or(8)
    });

// executor.rs entry
let _permit = state.semaphore.clone().acquire_owned().await?;
// _permit auto-returns at end of scope, including when Future is dropped
```

### Error & exception handling

| Scenario | Behavior |
|---|---|
| Client disconnects while queued on Semaphore | Future dropped, `acquire_owned` auto-cancels, 0 leak |
| process-wrap spawn fails | Returns `500 SPAWN_FAILURE`, permit released |
| Timeout triggered | KillOnDrop cascade terminates, permit released |
| Windows JobObject creation fails | `warn!` degrades gracefully, process continues (loses subprocess kill capability but does not block service) |
| Child exits normally | After `wait()` succeeds, disarm state machine — no erroneous kill on Drop |

## Data Flow

```
HTTP Request
  → Axum Router route resolution
  → handler: semaphore.acquire_owned().await (queue)
  → CommandWrap construct + spawn
  → tokio::spawn drain stdout
  → tokio::spawn drain stderr
  → timeout(child.wait()).await
  → timeout/normal → permit Drop → semaphore released
  → HTTP Response

Client disconnect (Future Drop):
  → permit Drop → semaphore released
  → Child Drop → KillOnDrop → Unix: kill(-pgid, SIGKILL) / Windows: CloseHandle(JobObject)
```

## Dependencies

- `process-wrap = { version = "9.1.0", features = ["tokio1"] }`
- Transitive deps: `windows ^0.62.2` (Windows only), `nix ^0.31.1` (Unix only)
- MSRV: 1.87.0 (Docker `rust:1.96-alpine` ok; local dev needs `rustup update`)

## Trade-offs

| Option | Trade-off |
|---|---|
| process-wrap vs custom FFI | process-wrap adds external deps to Cargo tree, but avoids `unsafe_code = "forbid"` conflict and inherits industrial test suite |
| Semaphore.try_acquire vs acquire_owned | `try_acquire` can return 429 immediately, but changes existing endpoint behavior; `acquire_owned` queues more smoothly — v1 uses queue |
| Default MAX_CONCURRENT value | CPU*2 is conservative, guarantees `fork` blocking won't flood Tokio worker threads |
