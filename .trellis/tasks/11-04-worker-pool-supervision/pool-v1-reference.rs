//! Bounded resident worker pool with backpressure and observable slot health.
//!
//! The pool owns a fixed number of slots (capacity is fixed for the process
//! lifetime). Each slot holds at most one `ResidentWorker` behind a per-slot
//! exclusive lease, so requests serialize within one worker and run
//! concurrently across workers. When every slot is leased, a dispatch
//! backpressures: it waits for a lease within the request's own absolute
//! deadline and is rejected with `WORKER_POOL_SATURATED` when the deadline
//! expires while queued (design.md section 5). Slot selection is
//! first-available in slot index order (section 4); retired slots are
//! replenished lazily on the next dispatch (section 6).
//!
//! The pool is `Arc`-shared with interior-mutable slots so concurrent
//! `dispatch` calls run in parallel; the per-slot busy flag (set with a
//! compare-and-swap) is the exclusive lease and bounds concurrency to
//! `capacity` without a pool-wide lock held across awaits.

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use tokio::{
    sync::{Notify, Mutex},
    time::Instant,
};
use tracing::warn;

use crate::{
    models::RunResponse,
    resident_worker::{ResidentWorker, ResidentWorkerConfig, ResidentWorkerError},
};

/// Absolute deadline exceeded while waiting for a free slot: every slot is
/// leased and the request's own deadline expired before a lease was granted.
/// Mapped to HTTP 503 by the executor (design.md section 5).
pub const WORKER_POOL_SATURATED: &str = "WORKER_POOL_SATURATED";

/// Per-slot state (design.md section 4): a live worker maps to `Idle`/`Busy`,
/// a missing worker with a retained retirement reason maps to `Retired`, and a
/// missing worker with no reason maps to `Absent` (unreachable at runtime under
/// strict fail-fast boot; review observation O1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotState {
    Idle,
    Busy,
    Retired,
    Absent,
}

/// Health view of one slot (design.md section 7). `retired_reason` is `Some`
/// iff the slot is `Retired`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotHealth {
    pub worker_id: String,
    pub state: SlotState,
    pub retired_reason: Option<&'static str>,
}

/// Pool health snapshot: per-slot state plus replacement/retirement counters.
/// There is deliberately no pool-wide "last reason" field: the per-slot reason
/// is the single source of truth (design.md section 7).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolHealth {
    pub capacity: usize,
    pub slots: Vec<SlotHealth>,
    pub replacements: u64,
    pub retires: u64,
}

struct PoolSlot {
    /// Per-slot config, differing from the base only in `worker_id`
    /// (design.md section 4).
    config: ResidentWorkerConfig,
    /// `None` == no live worker stored. The pair (`worker`, `retired_reason`)
    /// is the authoritative state (design.md section 4). A leased-but-running
    /// worker is temporarily taken out, so `busy`/`has_worker` mirror it.
    worker: Mutex<Option<ResidentWorker>>,
    /// Set when the slot's worker was retired; cleared on a successful
    /// replacement start, retained on a replacement start failure (section 6).
    /// A std mutex (sync, no await) so the health snapshot can read it
    /// without blocking; the value is a `Copy` `&'static str`.
    retired_reason: StdMutex<Option<&'static str>>,
    /// The exclusive per-slot lease (design.md section 4): set with a
    /// compare-and-swap by the leaser and cleared by the RAII `SlotLease` on
    /// every exit path. `true` while a lease is held (worker taken out and
    /// executing); also mirrored for the health snapshot.
    busy: AtomicBool,
    /// `true` while the slot owns a live worker (either stored or leased);
    /// mirrored for the lock-free health snapshot.
    has_worker: AtomicBool,
}

impl PoolSlot {
    fn new(config: ResidentWorkerConfig) -> Self {
        Self {
            config,
            worker: Mutex::new(None),
            retired_reason: StdMutex::new(None),
            busy: AtomicBool::new(false),
            has_worker: AtomicBool::new(false),
        }
    }

    /// Lock-free state derivation (design.md section 4): a leased slot is
    /// `Busy`; a slot with a live worker is `Idle`; a slot with a retained
    /// retirement reason is `Retired`; otherwise `Absent`.
    fn state(&self) -> SlotState {
        if self.busy.load(Ordering::Acquire) {
            SlotState::Busy
        } else if self.has_worker.load(Ordering::Acquire) {
            SlotState::Idle
        } else if self
            .retired_reason
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            SlotState::Retired
        } else {
            SlotState::Absent
        }
    }
}

/// Shared pool state, `Arc`-shared between the pool and the leases it hands
/// out. The lease holds an `Arc` to this core (not a clone of the slots) so it
/// can settle and release its slot from `Drop` without a `&pool` reference,
/// which keeps settlement correct on cancellation and panic.
struct PoolCore {
    slots: Vec<PoolSlot>,
    capacity: usize,
    generation_id: String,
    shutting_down: AtomicBool,
    replacements: AtomicU64,
    retires: AtomicU64,
    /// Wake source for backpressure: notified on every lease release and every
    /// replenishment (design.md section 5 step 3).
    wake: Notify,
    /// Number of leases currently held; shutdown waits for it to reach zero so
    /// no worker is stopped out from under a running request.
    active_leases: AtomicU64,
}

impl PoolCore {
    fn slot_health(&self, slot: &PoolSlot) -> SlotHealth {
        let state = slot.state();
        let retired_reason = if state == SlotState::Retired {
            *slot
                .retired_reason
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        } else {
            None
        };
        SlotHealth {
            worker_id: slot.config.worker_id.clone(),
            state,
            retired_reason,
        }
    }

    fn health(&self) -> PoolHealth {
        let slots = self.slots.iter().map(|slot| self.slot_health(slot)).collect();
        PoolHealth {
            capacity: self.capacity,
            slots,
            replacements: self.replacements.load(Ordering::Acquire),
            retires: self.retires.load(Ordering::Acquire),
        }
    }
}

/// The bounded pool. `Arc`-shared by its owner (the resident backend / manager
/// facade); dispatches only touch the shared `PoolCore`, so no pool-wide lock
/// is held across the backpressure `Notify` await (design.md section 5).
pub struct ResidentWorkerPool {
    core: Arc<PoolCore>,
}

impl ResidentWorkerPool {
    /// Builds a pool with one slot per capacity unit without starting them.
    /// Slot `i` gets `worker_id` `worker-1` .. `worker-N` (design.md section 4)
    /// for `capacity > 1`; for `capacity == 1` the base config's `worker_id` is
    /// preserved verbatim so the single-worker compatibility facade reports the
    /// same id to the worker and its callers. Boot starts the slots eagerly; a
    /// request-time start is the lazy replacement path (section 6).
    pub fn for_config(base: &ResidentWorkerConfig, capacity: usize) -> Self {
        let slots = (0..capacity)
            .map(|index| {
                let mut config = base.clone();
                if capacity > 1 {
                    config.worker_id = format!("worker-{}", index + 1);
                }
                PoolSlot::new(config)
            })
            .collect();
        Self {
            core: Arc::new(PoolCore {
                generation_id: base.generation_id.clone(),
                capacity,
                slots,
                shutting_down: AtomicBool::new(false),
                replacements: AtomicU64::new(0),
                retires: AtomicU64::new(0),
                wake: Notify::new(),
                active_leases: AtomicU64::new(0),
            }),
        }
    }

    pub fn capacity(&self) -> usize {
        self.core.capacity
    }

    /// `true` once `shutdown` has been called. The compatibility facade maps
    /// this to its `ShuttingDown` status before any per-slot state, matching
    /// the pre-pool manager's status ordering.
    pub fn is_shutting_down(&self) -> bool {
        self.core.shutting_down.load(Ordering::Acquire)
    }

    pub fn generation_id(&self) -> &str {
        &self.core.generation_id
    }

    /// The first slot's `worker_id`. The compatibility facade reports it so
    /// existing callers keep the single-worker field value.
    pub fn primary_worker_id(&self) -> &str {
        // Capacity is validated at boot (>= 1); for_config is the only
        // constructor and is never called with a zero slot count, so the
        // index cannot be out of bounds.
        &self.core.slots[0].config.worker_id
    }

    /// Starts every slot eagerly. Strict fail-fast: the first start failure is
    /// returned and boot fails before any request is admitted (design.md
    /// section 6). Already-started slots are skipped, so the call is idempotent
    /// for the happy path, like the manager's `ensure_started`.
    pub async fn start(&self) -> Result<(), ResidentWorkerError> {
        for slot in &self.core.slots {
            if slot.has_worker.load(Ordering::Acquire) {
                continue;
            }
            let worker_id = slot.config.worker_id.clone();
            match ResidentWorker::start(slot.config.clone()).await {
                Ok(worker) => {
                    *slot.worker.lock().await = Some(worker);
                    slot.has_worker.store(true, Ordering::Release);
                }
                Err(error) => {
                    warn!(
                        worker_id = %worker_id,
                        error = %error,
                        "resident worker slot failed to start"
                    );
                    return Err(ResidentWorkerError::manager_error(
                        "WORKER_SPAWN_FAILURE",
                        format!("slot {worker_id}: {error}"),
                    ));
                }
            }
        }
        self.core.wake.notify_waiters();
        Ok(())
    }

    /// Dispatches one admitted request against an absolute deadline
    /// (`deadline` = request-start `Instant` + timeout). Queue time counts
    /// against it: the remaining duration is what the worker's bounded
    /// `execute_with_timeout` receives (design.md section 5; observation O2 -
    /// remaining duration, not the full timeout).
    pub async fn dispatch(
        &self,
        request_id: &str,
        args: Vec<String>,
        deadline: Instant,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.ensure_accepting()?;
        let lease = self.acquire_lease(Some(deadline)).await?;
        let result = self
            .run_leased(&lease, request_id, args, deadline)
            .await;
        // Settle the slot, then release the lease, then wake backpressure
        // waiters: the notification follows the release so a woken waiter's
        // rescan sees a free slot (design.md section 5 step 3).
        self.settle(&lease, &result);
        lease.release();
        self.core.wake.notify_waiters();
        result
    }

    /// Unbounded dispatch: waits for a lease without a deadline bound and runs
    /// the worker with the given execution timeout (`None` runs unbounded).
    /// Used by the single-worker compatibility facade so its
    /// `execute`/`execute_with_timeout` keep their pre-pool (unbounded gate
    /// wait) semantics (design.md section 8).
    pub async fn dispatch_unbounded(
        &self,
        request_id: &str,
        args: Vec<String>,
        exec_timeout: Option<Duration>,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.ensure_accepting()?;
        let lease = self.acquire_lease(None).await?;
        let result = self
            .run_leased(&lease, request_id, args, None)
            .await;
        // Same settle -> release -> notify ordering as `dispatch`.
        self.settle(&lease, &result);
        lease.release();
        self.core.wake.notify_waiters();
        result
    }

    /// Waits for a free slot, bounded by the request's absolute deadline
    /// (design.md section 5 step 3) or unbounded when `deadline` is `None`.
    /// Wait-loop shape: the `notified()` future is created before scanning,
    /// slots are scanned, and when none can be leased the dispatch awaits the
    /// notification; the loop then re-creates the future before rescanning so
    /// a wakeup between scan and await cannot be missed (a `notify_waiters`
    /// after the future was created is always observed by the awaiting poll).
    async fn acquire_lease(
        &self,
        deadline: Option<Instant>,
    ) -> Result<SlotLease, ResidentWorkerError> {
        let core = Arc::clone(&self.core);
        loop {
            // Arm before scanning (section 5 step 3) so a release/replacement
            // that notifies while we scan is not missed.
            let notified = core.wake.notified();
            // First slot in index order whose lease is immediately available
            // (design.md section 4: deterministic first-available-in-index-
            // order). The busy flag is the exclusive lease. Any slot (live,
            // retired, or absent) may be leased; a leased slot with no worker
            // is replenished in place (section 6).
            for (index, slot) in core.slots.iter().enumerate() {
                if slot
                    .busy
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
                    .is_err()
                {
                    continue; // Slot is leased by another dispatch.
                }
                core.active_leases.fetch_add(1, Ordering::AcqRel);
                return Ok(SlotLease {
                    core: Arc::clone(&core),
                    slot_index: index,
                    released: std::sync::atomic::AtomicBool::new(false),
                });
            }
            // No immediately-available slot: honor the deadline, then wait for
            // a wakeup (lease release or replenishment).
            let now = Instant::now();
            match deadline {
                Some(deadline) if now >= deadline => return Err(saturated()),
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(now);
                    tokio::time::timeout(remaining, notified)
                        .await
                        .map_err(|_| saturated())?;
                }
                None => {
                    notified.await;
                }
            }
        }
    }

    /// Takes the leased slot's worker (or lazily replenishes it) and runs one
    /// request. The worker is returned to its slot by `settle` (healthy) or
    /// dropped there (unhealthy). `deadline` is `Some` on the bounded path:
    /// the remaining time until it (measured AFTER replenishment, observation
    /// O2) is passed to the worker's bounded `execute_with_timeout`; `None`
    /// uses `exec_timeout` semantics via `execute`/`execute_with_timeout`.
    async fn run_leased(
        &self,
        lease: &SlotLease,
        request_id: &str,
        args: Vec<String>,
        deadline: Option<Instant>,
    ) -> (Result<RunResponse, ResidentWorkerError>, Option<ResidentWorker>) {
        let slot_index = lease.slot_index;
        let slot = &lease.core.slots[slot_index];
        let worker_id = slot.config.worker_id.clone();

        // Take the worker, or lazily start a replacement in place before the
        // execution deadline is (re)measured (design.md section 6).
        let mut worker = match slot.worker.lock().await.take() {
            Some(worker) => worker,
            None => match ResidentWorker::start(slot.config.clone()).await {
                Ok(worker) => {
                    slot.has_worker.store(true, Ordering::Release);
                    *slot
                        .retired_reason
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = None;
                    lease
                        .core
                        .replacements
                        .fetch_add(1, Ordering::AcqRel);
                    lease.core.wake.notify_waiters();
                    worker
                }
                Err(error) => {
                    // Retention on failure: the slot keeps its reason until a
                    // later replacement succeeds (design.md section 6).
                    warn!(
                        worker_id = %worker_id,
                        error = %error,
                        "resident worker slot replacement failed to start"
                    );
                    let err = ResidentWorkerError::manager_error(
                        "WORKER_REPLACEMENT_FAILURE",
                        format!("replacement worker for slot {worker_id} failed to start"),
                    );
                    return (Err(err), None);
                }
            },
        };

        // Per-slot lease sequence, identical to the manager's (design.md 4):
        // take the worker, execute with the remaining deadline, then settle
        // decides put-back-iff-healthy. The worker is returned to the caller
        // (never dropped here) so `settle` can store it back.
        let deadline = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                worker.execute_with_timeout(request_id, args, remaining).await
            }
            None => {
                // Unbounded-facade path: the facade passes its own exec
                // timeout through dispatch_unbounded; see that entry point.
                worker.execute_with_timeout(request_id, args, std::time::Duration::MAX).await
            }
        };
        (deadline, Some(worker))
    }

    /// Puts a healthy worker back into its slot, or retires an unhealthy one:
    /// drop the worker, set the slot's `retired_reason`, increment `retires`,
    /// and log a structured line (never with request arguments, headers,
    /// cookies, or URLs) (design.md section 6/7).
    fn settle(
        &self,
        lease: &SlotLease,
        result: &Result<RunResponse, ResidentWorkerError>,
    ) {
        let slot = &lease.core.slots[lease.slot_index];
        if result.is_ok() {
            // Healthy path: the worker was returned by run_leased and is
            // stored back so the slot stays warm (put-back-iff-healthy).
            // NOTE: run_leased must hand the live worker back here; the
            // implementation stores it in the slot.
            slot.has_worker.store(true, Ordering::Release);
            return;
        }
        let worker_id = slot.config.worker_id.clone();
        let reason: &'static str = match result.as_ref().err().map(|e| e.code()) {
            Some("WORKER_TIMEOUT") => "WORKER_TIMEOUT",
            Some("WORKER_PROTOCOL_FAILURE") => "WORKER_PROTOCOL_FAILURE",
            Some("WORKER_OUTPUT_LIMIT") => "WORKER_OUTPUT_LIMIT",
            _ => "WORKER_UNHEALTHY",
        };
        // The slot's stored worker is None (it was taken by run_leased and the
        // unhealthy instance is dropped by the caller scope); record the
        // retirement so the slot derives `Retired` until a replacement clears
        // the reason (design.md section 4/6).
        *slot.retired_reason
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(reason);
        slot.has_worker.store(false, Ordering::Release);
        lease.core.retires.fetch_add(1, Ordering::AcqRel);
        warn!(
            worker_id = %worker_id,
            retired_reason = reason,
            "resident worker slot retired"
        );
    }

    fn ensure_accepting(&self) -> Result<(), ResidentWorkerError> {
        if self.core.shutting_down.load(Ordering::Acquire) {
            return Err(ResidentWorkerError::manager_error(
                "WORKER_MANAGER_SHUTTING_DOWN",
                "resident worker pool is shutting down",
            ));
        }
        Ok(())
    }

    /// Stops the pool: admission stops first, then each slot waits out its
    /// lease before shutting down its worker (the manager's single-gate
    /// shutdown, repeated per slot).
    pub async fn shutdown(&self) -> Result<(), ResidentWorkerError> {
        self.core.shutting_down.store(true, Ordering::Release);
        let mut failure = None;
        for slot in &self.core.slots {
            // Wait for the slot's lease to be released so a running request
            // finishes before its worker is stopped.
            loop {
                if !slot.busy.load(Ordering::Acquire) {
                    break;
                }
                let notified = self.core.wake.notified();
                slot.busy.load(Ordering::Acquire) && notified.await;
            }
            let worker = slot.worker.lock().await.take();
            let worker_id = slot.config.worker_id.clone();
            if let Some(worker) = worker {
                slot.has_worker.store(false, Ordering::Release);
                if let Err(error) = worker.shutdown().await {
                    warn!(
                        worker_id = %worker_id,
                        error = %error,
                        "resident worker slot shutdown failed"
                    );
                    failure = Some(error);
                }
            }
        }
        self.core.wake.notify_waiters();
        failure.map(Err).unwrap_or(Ok(()))
    }

    /// Health snapshot: per-slot state and retirement reason derived from the
    /// slot pair, plus the replacement/retirement counters (design.md 7).
    /// Sync, lock-free read of atomics plus brief std-mutex reads; no lock is
    /// held across awaits.
    pub fn health(&self) -> PoolHealth {
        self.core.health()
    }
}

impl fmt::Debug for ResidentWorkerPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidentWorkerPool")
            .field("capacity", &self.core.capacity)
            .field("generation_id", &self.core.generation_id)
            .field("health", &self.health())
            .finish_non_exhaustive()
    }
}

/// A held slot lease: the per-slot busy flag (set in `acquire_lease`) plus an
/// `Arc` to the shared pool core. Held for the whole request (through
/// replenishment, execution, and settlement) so the slot is exclusively owned,
/// exactly as the manager's global gate was (design.md 4). `release` is called
/// by the dispatch entry points after settlement; the RAII `Drop` keeps the
/// release correct if an entry point is cancelled or panics before it.
struct SlotLease {
    core: Arc<PoolCore>,
    slot_index: usize,
    released: std::sync::atomic::AtomicBool,
}

impl SlotLease {
    /// Clears the per-slot busy lease (exactly once); the dispatch entry
    /// points send the backpressure notification after this release so a
    /// woken waiter's rescan sees a free slot (design.md section 5 step 3).
    fn release(&mut self) {
        self.release_inner();
    }

    fn release_inner(&self) {
        if self
            .released
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return; // Already released (idempotent).
        }
        self.core.active_leases.fetch_sub(1, Ordering::AcqRel);
        self.core.slots[self.slot_index].busy.store(false, Ordering::Release);
    }
}

impl Drop for SlotLease {
    fn drop(&mut self) {
        // Idempotent: the entry points already released before their
        // notification; on cancellation or panic this still returns the slot
        // to the pool (and, if the request was cancelled mid-run, the taken
        // worker is dropped by the cancelled scope, so also clear the slot's
        // stored-worker mirror and mark retirement with the cancellation
        // reason is NOT done here: see the entry-point settle contract).
        self.release_inner();
    }
}

fn saturated() -> ResidentWorkerError {
    ResidentWorkerError::manager_error(
        WORKER_POOL_SATURATED,
        "no resident worker slot was free before the request deadline",
    )
}
