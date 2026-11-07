# Worker pool supervision design

Unit of parent task 11-03-resident-worker-runtime-upgrade (rollout step 2,
parent design.md). Parent baseline: commit c08f398. Scope: prd.md of this unit.

## 1. Context

c08f398 ships a single-worker resident backend: `ResidentWorkerManager` holds
one `Option<ResidentWorker>` behind a global `execution_gate` mutex, so all
resident requests serialize even when the HTTP semaphore admits several. This
unit generalizes that manager to N workers with backpressure and observable
health, keeping every Phase 3 constraint intact.

## 2. Module boundaries and responsibilities

| Module | Responsibility | Change |
|---|---|---|
| `src/resident_worker_pool.rs` (new) | Owns N `PoolSlot`s, slot allocation (first-available in slot index order, section 4), backpressure wait, lazy replenishment, health snapshot, retirement/replacement counters | new |
| `src/resident_worker_manager.rs` | Becomes a thin compatibility facade: `ResidentWorkerManager` is a pool of capacity 1; its public API and `ResidentWorkerManagerStatus` semantics stay byte-for-byte identical for existing callers and tests | refactor, API preserved |
| `src/resident_worker.rs` | One worker: handshake, serial execution, temp dir, retirement, process-group/JobObject containment | unchanged (no new surface) |
| `src/backend.rs` | `Backend::Resident` payload becomes `Arc<ResidentWorkerPool>`; boot start, shutdown, `from_config` gain pool size; explicit non-default config and no-fallback rules unchanged | signature change, same rules |
| `src/main.rs` | New `--worker-pool-size` / `WORKER_POOL_SIZE` (default 1); boot-time validation before any request is admitted | additive |
| `src/executor.rs` | Dispatch unchanged in shape: still routes by backend, still `execute_with_timeout` with the clamped deadline; the pool-saturation error code maps to 503 like `SHUTTING_DOWN` | small mapping addition |
| `tests/resident_worker_pool.rs` (new) | Deterministic pool tests (concurrency, backpressure, replenishment, health, pool-size regression) | new |

No changes to `worker/ytdlp_worker.py`, `src/worker_protocol.rs`,
`src/supervisor.rs`, `src/models.rs`, or the CLI backend path.

## 3. Dependency direction

`executor` -> `backend` -> `resident_worker_pool` -> `resident_worker_manager`
(compat facade) -> `resident_worker` -> `worker_protocol`.
Dependencies stay one-directional and within the existing layering
(parent design.md section 2 Rust responsibilities: queueing, concurrency,
deadlines, worker supervision are owned by Rust). The pool never imports
`routes`; `Backend` remains the only seam the executor crosses.
Forbidden: pool code reaching into `supervisor` (CLI) or `executor`.

## 4. Pool data model

```text
ResidentWorkerPool
  slots: Vec<PoolSlot>              // fixed length == capacity
  capacity: usize
  replacements: AtomicU64
  retires: AtomicU64

PoolSlot
  worker: Option<ResidentWorker>    // None == no live worker (never started, or retired and not yet replaced)
  retired_reason: Option<&'static str> // set when a worker was retired; cleared on successful replacement
  worker_id: String                 // "worker-1" .. "worker-N", stable per slot
  gate: Mutex<()>                   // per-slot exclusive lease (replaces the manager's global gate)
  state: Idle | Busy                // mirrored for the health snapshot; authoritative state is the gate + Option
```

- Per-slot state is derivable from the pair (`worker`, `retired_reason`):
  live worker -> `Idle`/`Busy`; no worker + `retired_reason` set -> `Retired`
  (with that reason); no worker + no reason -> `Absent` (never started).
  A successful replacement clears `retired_reason`, so a replenished slot
  returns to `Idle`/`Busy` rather than staying `Retired`. This makes the
  prd's per-slot state vocabulary (idle/busy/retired/absent) and the
  per-slot retirement reason representable; there is no pool-wide
  "last reason" field (section 7).
- Slot identity: `worker_id` per slot is stable; a replacement worker reuses
  its slot's `worker_id` and receives `worker_id` via the existing
  `ResidentWorkerConfig` clone (one config per slot, differing only in
  `worker_id`).
- Leasing: a dispatch takes the slot gate; the worker is `take()`-n, executed
  with the bounded deadline, and put back iff healthy. Identical to today's
  single-worker sequence, repeated per slot (parent design.md section 5
  lease rules).
- Selection: scan slots for the first whose gate is immediately available and
  that holds a live worker; if none, backpressure (section 5). Deterministic
  scan order = slot index order; tests may rely on it. This is
  first-available-in-index-order (not least-recently-occupied) and therefore
  biases load toward low-index slots; that bias is intentional and is the
  same policy named in section 2.

## 5. Dispatch and backpressure

[ASSUMPTION B1] Backpressure = bounded wait against the request's own
deadline (prd.md requirement 4 / AC3; prd-mandated, not a design choice).
The only assumption is that no separate queue-depth configuration is added.

- `dispatch(request_id, args, deadline: Instant)`: the deadline is an
  absolute `Instant` (equivalently request-start `Instant` + timeout), so
  queue time counts against it. Today `reside()` receives only a `Duration`
  and starts the clock at the worker call (`src/backend.rs:125`,
  `src/executor.rs:79`); this unit adds the request-start anchor so the pool
  can subtract elapsed time before handing the remaining duration to
  `execute_with_timeout` (which stays the only execution path).
  1. If shutting down -> `WORKER_MANAGER_SHUTTING_DOWN` (unchanged code,
     keeps the existing 503 mapping).
  2. Try each slot in index order for an immediate lease + live worker. On a
     hit, execute exactly as the manager does today.
  3. No immediate slot: wait for a slot to become free, bounded by the
     remaining time until `deadline`. Wait-loop shape: arm `Notify`
     (`notified()` future created before scanning), scan slots, if none
     await the notification, then **re-arm before retrying the scan** so a
     wakeup between scan and await cannot be missed; repeat until a slot is
     leased or the deadline passes. Wake source: a `tokio::sync::Notify`
     notified on every lease release and every replenishment.
  4. Deadline expires while waiting -> new error code
     `WORKER_POOL_SATURATED` (mapped to HTTP 503 in the executor, the same
     treatment as `SHUTTING_DOWN`).
  5. The request is never retried on a different worker after a failure, and
     there is no fallback to the CLI backend (parent design.md section 3:
     silent fallback is forbidden).
- Fairness: [ASSUMPTION] a newly arrived request may lease a slot that frees
  just before an already-waiting request wakes, i.e. immediate-lease
  preference for new arrivals, with no FIFO queue. This is acceptable here
  because every wait is bounded by the request deadline (no unbounded
  starvation) and no separate queue structure is introduced. The
  alternative (FIFO waiter ordering) is not adopted in this unit.
- Rationale for B1 over immediate rejection: the request deadline already
  bounds the wait, so no new configuration surface is needed (YAGNI), and
  burst traffic that fits within deadlines still succeeds. Immediate
  rejection is the documented alternative if the owner prefers.

## 6. Replenishment

Lazy, matching the existing single-worker semantics (parent design.md section
11 names "pool replenishment" as a required deterministic test):

- A retired slot stays absent until the next dispatch that needs a worker:
  that dispatch starts a replacement in place (spawn + handshake) before the
  execution deadline is (re)measured for the worker call, exactly as the
  current manager's replacement path does.
- On successful start: `replacements += 1` and the slot's `retired_reason`
  is cleared. On any retirement: `retires += 1`, the slot's
  `retired_reason` is set to the retirement reason code, plus a structured
  log line with slot, `worker_id`, and that reason code. On replacement start
  failure: the request fails with `WORKER_REPLACEMENT_FAILURE` (unchanged
  code); the slot stays absent (`retired_reason` retained until a later
  replacement succeeds).
- Boot: `Backend::start` starts all slots eagerly. **Strict fail-fast:** if
  any slot fails to start, `start` returns that error and the whole boot
  fails before any request is admitted. This preserves the current rule
  (`src/main.rs:215` `backend.start().await?`; `src/backend.rs` `start`
  propagates the first `ensure_started` error), which prd.md requires to stay
  unchanged ("a bad resident configuration fails at boot"). Partial-degraded
  boot (some slots live, some absent, boot continues) is rejected because it
  would relax that preserved rule; it is not a design option here.

## 7. Health and observability

```text
PoolHealth {
  capacity: usize,
  slots: [SlotHealth {
    worker_id,
    state: Idle | Busy | Retired | Absent,
    retired_reason: Option<&'static str>,   // Some iff state == Retired
  }],
  replacements: u64,
  retires: u64,
}
```

- Per-slot `state` and `retired_reason` derive from `PoolSlot` (section 4):
  live worker -> `Idle`/`Busy`; `retired_reason` set -> `Retired` with that
  reason; neither -> `Absent`. The pool-wide `retiring_reason_last` field is
  **dropped**: the prd asks for retirement reason at per-slot granularity,
  which the per-slot marker satisfies, and a pool-wide copy would duplicate
  state and could disagree with the slots.
- Exposed as `ResidentWorkerPool::health()` (sync read of atomics + a short
  pool-wide lock) and surfaced through `Backend` so Phase 7 can attach a
  metrics/log endpoint without further API change.
- No new HTTP route is added in this unit (observability endpoint is parent
  `implement.md` Phase 7 "Operational hardening"; parent design.md section 10
  defines the metrics inventory). Structured logs only here.
- Logs must not contain request arguments, headers, cookies, or URLs
  (parent design.md section 10).

## 8. Configuration

- `--worker-pool-size` / `WORKER_POOL_SIZE`, default `1`, parsed as usize.
  `0` or non-numeric -> boot error before the runtime serves (parent design.md
  section 8 style: actionable structured error).
- Capacity is fixed for the process lifetime; no runtime resize.
- With capacity 1 the pool is behaviorally identical to c08f398 for the
  existing test surface and for uncontended requests: same status semantics
  (`Ready`/`Empty`/`Busy`/`ShuttingDown`), same error codes, same single-slot
  lazy replacement. Contended requests differ by design: today's global
  `execution_gate` wait is unbounded and does not count against the request
  deadline (`src/resident_worker_manager.rs:122`), whereas here queue wait
  counts against the deadline (section 5) and the deadline bounds it. That
  bounded wait is prd-mandated ("wait within the request's own deadline",
  "no unbounded in-process queue"), not a regression. This scoped claim is
  the regression baseline (prd AC 6).

## 9. Non-goals

- No runtime pool resize, no per-slot independent configuration.
- No metrics endpoint or histogram; structured logs + in-memory snapshot only
  (endpoint belongs to Phase 7).
- No generation/canary/promotion semantics (Phase 6, parent design.md section
  7); pool configuration inside the generation manifest is a Phase 6 concern.
- No changes to the worker protocol, the Python worker, or the CLI backend.
- No cross-worker request retry (parent design.md section 13 rejects
  concurrency inside one worker; retry across workers is rejected here for the
  same side-effect-duplication reason as silent fallback).

## 10. Validation mapping (prd acceptance criteria -> tests)

- AC1 -> `main.rs` boot validation unit test (0, non-numeric) + `from_config`
  test in `tests/resident_backend.rs` style.
- AC2 -> `tests/resident_worker_pool.rs`: two concurrent requests, assert two
  distinct `worker_id`s and per-slot serialization. The fixture's stdout is
  the length-prefixed frame stream, so `worker_id` must travel **inside the
  result frame payload** (e.g. the result's `stdout` field, which tests can
  read from the frame) or on stderr (the existing `stderr-diagnostic`
  pattern) — never as plain-text stdout. Extend the fixture to report its
  `YTDLP_WORKER_ID` on one of those in-band channels.
- AC3 -> pool size 2, three concurrent slow requests; the third either runs
  after a slot frees within its deadline or returns 503 `WORKER_POOL_SATURATED`.
- AC4 -> retire one slot via the `crash-eof` fixture path, next dispatch
  replenishes to capacity (design.md section 11 of parent task names this).
- AC5 -> health() assertions after the sequence in AC4: assert each slot's
  `state` and, for the retired slot, `retired_reason` (section 4/7), plus the
  `replacements`/`retires` counters matching the requests issued.
- AC6 -> full existing suite green with default config, zero existing-test
  edits.
- AC7 -> fmt/clippy/test/python gates, same as the parent task.
