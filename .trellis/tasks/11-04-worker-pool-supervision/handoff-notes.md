# Handoff notes for the pool implementer (owner session; updated 2026-11-05 AM, round 6)

## Disk state (verified by owner)
- src/resident_worker_manager.rs — MODIFIED (uncommitted): now a thin facade over the pool
  (49 insertions / 95 deletions). It `use`s `resident_worker_pool::ResidentWorkerPool`
  (see line 13), builds it via `ResidentWorkerPool::for_config(&config, 1)` (line 35),
  and has `dispatch_for_test` (line 111). READ IT FIRST: it defines the exact pool API
  surface you must provide. Its public API (execute, execute_with_timeout, status,
  ResidentWorkerManagerStatus incl. ShuttingDown) must stay behaviorally identical to c08f398.
- src/resident_worker_pool.rs — MISSING (deleted mid-refactor by a prior agent).
  You must RECREATE it. The spec is design.md sections 2-10; the required API is what
  the facade consumes (read manager.rs for the signatures).
- backend.rs, executor.rs, lib.rs, main.rs, fixtures, tests — UNTOUCHED, still c08f398.
  lib.rs currently has NO `pub mod resident_worker_pool;` — add it.
- approval.md locks design.md (sha256 C46FAB1B...6FC278F). Design/prd/spec FROZEN.
- impl-audit-notes.md — read for known constraints (no .expect in production, keep
  Notify model, design section 5/6 tension resolution).

## What to do (in order)
1. Windowed reads (rg -n + 50-100 line windows, never whole files): src/resident_worker_manager.rs
   (the facade — defines your API contract), design.md sections 2-10 + 10 (tests per AC),
   prd.md AC1-AC7, impl-audit-notes.md, baseline src/resident_worker.rs (worker lifecycle API you
   reuse), src/backend.rs, src/executor.rs, src/main.rs, tests/resident_backend.rs,
   tests/fixtures/resident_worker_runtime.py.
2. Recreate src/resident_worker_pool.rs per design sections 2-10 matching the facade's expected API:
   - PoolSlot/Slot state enum with per-slot retired_reason: Option<&'static str>; four-state
     derivation (live->Idle/Busy; no worker+reason->Retired; neither->Absent; successful
     replacement clears reason)
   - allocation: first-available in slot index order, single policy
   - dispatch: absolute deadline Instant (request start + timeout); pass REMAINING duration to
     execute_with_timeout; backpressure wait loop per section 5 step 3 using tokio::sync::Notify
     (arm notified() BEFORE scan -> scan -> await -> re-arm -> rescan; immediate-lease preference,
     no FIFO, NO Semaphore); queued past deadline -> WORKER_POOL_SATURATED
   - boot: strict fail-fast, any slot start failure fails whole boot before admission
   - retire/replace: reason set on every retirement, retained on replacement-start failure,
     cleared on successful start; per-slot retires/replacements counters
   - health per section 7: per-slot state + retired_reason (Some iff Retired) + counters;
     do NOT add pool-wide retiring_reason_last; expose what the facade needs for status()
     incl. shutting-down state
   - no .expect/.unwrap in production code (clippy gate)
3. Wire the rest: backend.rs (config: --worker-pool-size flag + WORKER_POOL_SIZE env via main.rs,
   default 1; below-1/non-numeric fails at boot before admission; resident backend constructs the
   pool-based manager), lib.rs (pub mod), executor.rs (WORKER_POOL_SATURATED -> 503 mapping),
   main.rs (flag + env + boot validation).
4. Fixture: worker_id in-band — stderr marker or result frame stdout field, never plain-text
   stdout; extend tests/fixtures/resident_worker_runtime.py scenarios as needed.
5. Tests per design section 10 for AC1-AC6, style of tests/resident_backend.rs.
6. Gates, ALL green: cargo fmt --check; cargo clippy --all-targets -- -D warnings;
   cargo test (truncate: 2>&1 | Select-Object -Last 40); python -m unittest tests.test_ytdlp_worker -v.
7. ONE commit staging ONLY: src/, tests/, .trellis/tasks/11-04-worker-pool-supervision/.
   Never stage todos/, response.json, .opencode/, .claude/, .trellis/spec/mpd/,
   .trellis/spec/myMPD/, 00-bootstrap-guidelines, 11-03-bump-yt-dlp-2-10, docs/superpowers/,
   examples/, .tmp-*.log, __pycache__/. No amend/push/PR.
8. Report max 25 lines: commit hash, gate results, AC->test map, deviations with justification. STOP.

## Hard rules
CLI backend default; resident only via explicit config; no silent fallback; production dispatch
always bounded execute_with_timeout; preserve RunResponse shape; tests/run_contract.rs unchanged
and green; no new Cargo deps. If design contradicts baseline and design.md-wins cannot resolve:
record the exact contradiction and stop with BLOCKED. Do not audit, do not ask questions, do not
stop to offer options.
