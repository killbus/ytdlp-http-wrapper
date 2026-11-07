# Worker pool supervision: backpressure and observable health

## Goal

Extend the resident backend from a single resident worker to a bounded worker
pool with backpressure and observable per-slot health, completing the last
open checkbox of implement.md Phase 3 (parent task
11-03-resident-worker-runtime-upgrade, rollout step 2 of parent design.md).

## Requirements

- The resident backend runs N resident workers, N configurable, default 1.
- Each worker keeps the existing invariant: at most one active request,
  exclusive lease until result delivery and cleanup finish (parent design.md section 5).
- Requests may execute concurrently across different workers; they remain
  serialized within one worker.
- When every worker is busy, new requests are backpressured: they wait within
  the request's own deadline and are rejected with 503 plus a distinct error
  code when the deadline expires while queued. No unbounded in-process queue.
- A retired worker slot is replenished back to pool capacity (lazy
  replenishment on the next dispatch is acceptable, matching existing manager
  semantics).
- The pool exposes a health snapshot: per-slot worker id and state
  (idle/busy/retired/absent), plus replacement and retirement counters with
  retirement reason. Structured logs on retirement events.
- Pool size 1 must preserve today's single-worker behavior exactly
  (regression baseline: all existing resident tests stay green unchanged).
- All Phase 3 hard constraints stay unchanged: CLI backend remains the default;
  the resident backend is reachable only via explicit configuration; no silent
  fallback resident to CLI; production dispatch always uses the bounded-deadline
  execution path; per-request owned temp-directory ownership; the public
  RunResponse shape is preserved.

## Acceptance Criteria

- [ ] Pool size is accepted by the CLI via --worker-pool-size (env WORKER_POOL_SIZE), defaults to 1, and a value below 1 fails at boot with an actionable error before any request is admitted.
- [ ] With pool size 2, two concurrent resident requests complete using two distinct worker ids (fixture reports worker_id), and each worker processes at most one request at a time.
- [ ] A third concurrent request while all slots are busy is backpressured: it either proceeds when a slot frees within its deadline or is rejected with HTTP 503 and a pool-saturation error code when it does not.
- [ ] After one worker is retired by a protocol failure, the next dispatch replenishes the pool to its configured size (deterministic replenishment test, parent design.md section 11).
- [ ] A health query reports per-slot state and replacement/retirement counters consistent with the requests issued in the test.
- [ ] With the default pool size (1), the entire existing test suite passes without modifying any existing test expectation.
- [ ] cargo fmt --check, cargo clippy --all-targets (0 warnings), full cargo test, and python -m unittest tests.test_ytdlp_worker -v are all green.
