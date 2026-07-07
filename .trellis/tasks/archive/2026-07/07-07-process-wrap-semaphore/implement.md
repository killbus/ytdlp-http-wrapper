# Implementation Plan: Performance & Concurrency Refactor

## Execution Order

### Step 1: Add dependency

**Files:** `Cargo.toml`

- Add `process-wrap = { version = "9.1.0", features = ["tokio1"] }` to `[dependencies]`

**Verify:** `cargo check` passes (may auto-upgrade tokio)

---

### Step 2: Introduce AppState in routes.rs

**Files:** `src/routes.rs`

- Add `AppState` struct with `binary_path: PathBuf` and `semaphore: Arc<Semaphore>`
- Change `app()` signature from `app(binary_path: PathBuf) -> Router` to `app(state: Arc<AppState>) -> Router`
- Refactor route handlers from closures to separate async functions using `State<Arc<AppState>>` extractor
- Import `axum::extract::State`

**Verify:** `cargo check` (will fail at main.rs — expected, Step 3 fixes it)

---

### Step 3: Initialize AppState in main.rs

**Files:** `src/main.rs`

- Import `Arc`, `Semaphore`, `routes::AppState`
- Read `MAX_CONCURRENT_DOWNLOADS` env var with `available_parallelism()` fallback
- Create `Arc<AppState>` and pass to `routes::app(state)`

**Verify:** `cargo check` passes

---

### Step 4: Refactor executor.rs with process-wrap + Semaphore

**Files:** `src/executor.rs`

- Import `process_wrap::tokio::{CommandWrap, KillOnDrop, Wrap}` and platform-specific wrappers
- Change `execute(payload, binary_path)` to `execute(payload, state: Arc<AppState>)`
- Add `let _permit = state.semaphore.clone().acquire_owned().await` at function entry
- Replace `cmd.spawn()` → `CommandWrap::new(cmd).wrap(KillOnDrop)` + platform wraps + `.spawn()`
- Remove `cmd.kill_on_drop(true)` and `cmd.creation_flags(...)`
- Simplify timeout branch: manual `child.kill().await` + `child.wait().await` stays (belt-and-suspenders)

**Verify:** `cargo build --release` + `cargo clippy`

---

### Step 5: Verify

**Commands:**

```bash
cargo build --release
cargo clippy --all-targets
cargo test
```

## Rollback

If a step fails compilation:
- `git diff` to identify changed files
- `git checkout -- <file(s)>` to revert
- If Cargo.lock changed, restore via `git checkout -- Cargo.lock`

Full rollback: `git checkout -- .` + `cargo generate-lockfile`
