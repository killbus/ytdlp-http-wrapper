//! Thin compatibility facade over the bounded worker pool (design.md section
//! 2): `ResidentWorkerManager` is a pool of capacity 1. Its public API and
//! `ResidentWorkerManagerStatus` semantics stay identical to the pre-pool
//! manager for existing callers and tests; per-request deadline clamping stays
//! in the executor, so this facade keeps the pre-pool (unbounded gate wait)
//! dispatch semantics (design.md section 8).

use tokio::time::{Duration, Instant};

use crate::{
    models::RunResponse,
    resident_worker::{ResidentWorkerConfig, ResidentWorkerError},
    resident_worker_pool::ResidentWorkerPool,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidentWorkerManagerStatus {
    Ready,
    Empty,
    Busy,
    ShuttingDown,
}

pub struct ResidentWorkerManager {
    pool: ResidentWorkerPool,
}

impl ResidentWorkerManager {
    /// Builds a manager without starting its worker. Used by the resident
    /// backend so the spawn + handshake happen at boot (`ensure_started`),
    /// letting a bad resident configuration fail before any request is
    /// admitted.
    pub fn for_config(config: ResidentWorkerConfig) -> Self {
        Self::for_pool_size(config, 1)
    }

    /// Builds the facade over a pool of `capacity` slots. The resident
    /// backend uses this so `--worker-pool-size` reaches the pool
    /// (design.md section 2/8); the public API still delegates to the pool
    /// unchanged for existing callers and tests.
    pub fn for_pool_size(config: ResidentWorkerConfig, capacity: usize) -> Self {
        Self {
            pool: ResidentWorkerPool::for_config(&config, capacity),
        }
    }

    /// The underlying pool. The resident backend dispatches against it with
    /// the request-start-anchored absolute deadline so queue time counts
    /// against it (design.md section 5); the facade's own entry points keep
    /// the pre-pool unbounded wait semantics for existing callers and tests
    /// (section 8).
    pub fn pool(&self) -> &ResidentWorkerPool {
        &self.pool
    }

    /// Starts the worker if it has not started yet. Idempotent for the happy
    /// path; a start failure leaves the manager empty so the next request
    /// retries the start (single-worker replacement behavior).
    pub async fn ensure_started(&self) -> Result<(), ResidentWorkerError> {
        self.pool.start().await
    }

    pub fn generation_id(&self) -> &str {
        self.pool.generation_id()
    }

    pub fn worker_id(&self) -> &str {
        self.pool.primary_worker_id()
    }

    pub async fn start(config: ResidentWorkerConfig) -> Result<Self, ResidentWorkerError> {
        let manager = Self::for_config(config);
        manager.pool.start().await?;
        Ok(manager)
    }

    pub async fn execute(
        &self,
        request_id: &str,
        args: Vec<String>,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.pool.dispatch_unbounded(request_id, args, None).await
    }

    pub async fn execute_with_timeout(
        &self,
        request_id: &str,
        args: Vec<String>,
        duration: Duration,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.pool
            .dispatch_unbounded(request_id, args, Some(duration))
            .await
    }

    pub async fn status(&self) -> ResidentWorkerManagerStatus {
        // Shutting-down wins before any per-slot state, matching the pre-pool
        // manager's status ordering (its `ShuttingDown` was checked first).
        if self.pool.is_shutting_down() {
            return ResidentWorkerManagerStatus::ShuttingDown;
        }
        match self.pool.health().slots[0].state {
            crate::resident_worker_pool::SlotState::Busy => ResidentWorkerManagerStatus::Busy,
            crate::resident_worker_pool::SlotState::Idle => ResidentWorkerManagerStatus::Ready,
            crate::resident_worker_pool::SlotState::Retired => ResidentWorkerManagerStatus::Empty,
            crate::resident_worker_pool::SlotState::Absent => ResidentWorkerManagerStatus::Empty,
        }
    }

    pub async fn shutdown(&self) -> Result<(), ResidentWorkerError> {
        self.pool.shutdown().await
    }

    /// Test-only: a bounded, absolute-deadline dispatch against the underlying
    /// pool. Production dispatch goes through the backend, which owns the
    /// deadline anchor.
    #[doc(hidden)]
    pub async fn dispatch_for_test(
        &self,
        request_id: &str,
        args: Vec<String>,
        deadline: Instant,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.pool.dispatch(request_id, args, deadline).await
    }
}
