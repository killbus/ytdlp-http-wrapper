# Errata for pool-v1-reference.rs (owner-annotated, 2026-11-05)

pool-v1-reference.rs is a STRUCTURAL baseline recovered from a prior implementer's
deleted draft: the Notify backpressure model, RAII SlotLease, four-state derivation,
counters, and the exact API surface the manager facade consumes (verified: for_config,
start, generation_id, primary_worker_id, dispatch, dispatch_unbounded, is_shutting_down,
health, SlotState, shutdown). Its design is right; the code has these KNOWN BREAKAGES
that you must fix (do not ship the file as-is):

1. COMPILER: run_leased returns `(Result<RunResponse, ResidentWorkerError>,
   Option<ResidentWorker>)` but dispatch/dispatch_unbounded bind it as a plain
   `result` and pass `&result` to settle (which expects `&Result<...>`). Unpack the
   tuple at both call sites and hand the worker to settle.
2. LOGIC (the core bug of the original draft): settle's healthy path never stores the
   worker back into the slot (it only sets has_worker=true), so a healthy warm worker
   is dropped after EVERY request. settle must store it back: `*slot.worker.lock()...
   = Some(worker)` (make settle async, or switch the stored-worker field to StdMutex
   whose critical section is await-free).
3. LOGIC: dispatch_unbounded's `exec_timeout: Option<Duration>` is ignored. The
   facade's execute_with_timeout(duration) must actually bound execution: convert to
   deadline `Instant::now() + duration` and pass it through (None stays unbounded-ish
   via a very large bounded timeout, as the reference's None branch does).
4. CANCELLATION CONTRACT: if the dispatch future is cancelled mid-run, the taken
   worker is dropped by the cancelled scope and the slot must record retirement.
   Design section 6's reason vocabulary has no cancel code; pick the behavior that
   best fits section 4/6 (e.g. treat as WORKER_PROTOCOL_FAILURE with a comment, or
   add a code) and DOCUMENT it as a deviation in your final report. The SlotLease
   Drop already re-arms the busy flag idempotently; the worker/retired_reason
   settlement on the cancel path still needs handling (e.g. settle in the entry
   points via a helper that runs on every exit path, not just the happy path).

Also: no .expect/.unwrap in production code (clippy gate). Keep the Notify model
(never a Semaphore). The file must match the facade in src/resident_worker_manager.rs
byte-for-byte on the API surface it consumes.
