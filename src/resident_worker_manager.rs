use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::{sync::Mutex, time::Duration};
use tokio_util::sync::CancellationToken;

use crate::{
    models::RunResponse,
    resident_worker::{ResidentWorker, ResidentWorkerConfig, ResidentWorkerError},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidentWorkerManagerStatus {
    Ready,
    Empty,
    Busy,
    ShuttingDown,
}

pub struct ResidentWorkerManager {
    config: ResidentWorkerConfig,
    worker: Arc<Mutex<Option<ResidentWorker>>>,
    execution_gate: Mutex<()>,
    busy: AtomicBool,
    shutting_down: AtomicBool,
}

impl ResidentWorkerManager {
    /// Builds a manager without starting its worker. Used by the resident
    /// backend so the spawn + handshake happen at boot (`ensure_started`),
    /// letting a bad resident configuration fail before any request is admitted.
    pub fn for_config(config: ResidentWorkerConfig) -> Self {
        Self {
            config,
            worker: Arc::new(Mutex::new(None)),
            execution_gate: Mutex::new(()),
            busy: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
        }
    }

    /// Starts the worker if it has not started yet. Idempotent for the happy
    /// path; a start failure leaves the manager empty so the next request
    /// retries the start (single-worker replacement behavior).
    pub async fn ensure_started(&self) -> Result<(), ResidentWorkerError> {
        if self.worker.lock().await.is_some() {
            return Ok(());
        }
        let worker = ResidentWorker::start(self.config.clone()).await?;
        *self.worker.lock().await = Some(worker);
        Ok(())
    }

    pub fn generation_id(&self) -> &str {
        &self.config.generation_id
    }

    pub fn worker_id(&self) -> &str {
        &self.config.worker_id
    }

    pub async fn start(config: ResidentWorkerConfig) -> Result<Self, ResidentWorkerError> {
        let worker = ResidentWorker::start(config.clone()).await?;
        Ok(Self {
            config,
            worker: Arc::new(Mutex::new(Some(worker))),
            execution_gate: Mutex::new(()),
            busy: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
        })
    }

    pub async fn execute(
        &self,
        request_id: &str,
        args: Vec<String>,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.execute_inner(request_id, args, None, CancellationToken::new())
            .await
    }

    pub async fn execute_with_timeout(
        &self,
        request_id: &str,
        args: Vec<String>,
        duration: Duration,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.execute_inner(request_id, args, Some(duration), CancellationToken::new())
            .await
    }

    /// Bounded-deadline dispatch that also observes the caller's cancellation
    /// trigger. The caller cancels when the client disconnects or the service
    /// shuts down; the leased worker then runs the design section 5 retirement
    /// sequence (cooperative cancel frame, bounded grace, terminate, replace).
    pub async fn execute_with_cancellation(
        &self,
        request_id: &str,
        args: Vec<String>,
        duration: Duration,
        cancel: CancellationToken,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.execute_inner(request_id, args, Some(duration), cancel)
            .await
    }

    pub async fn status(&self) -> ResidentWorkerManagerStatus {
        if self.shutting_down.load(Ordering::Acquire) {
            return ResidentWorkerManagerStatus::ShuttingDown;
        }
        if self.busy.load(Ordering::Acquire) {
            return ResidentWorkerManagerStatus::Busy;
        }
        if self.worker.lock().await.is_some() {
            ResidentWorkerManagerStatus::Ready
        } else {
            ResidentWorkerManagerStatus::Empty
        }
    }

    pub async fn shutdown(&self) -> Result<(), ResidentWorkerError> {
        self.shutting_down.store(true, Ordering::Release);
        let _execution = self.execution_gate.lock().await;
        let worker = self.worker.lock().await.take();
        match worker {
            Some(worker) => worker.shutdown().await,
            None => Ok(()),
        }
    }

    async fn execute_inner(
        &self,
        request_id: &str,
        args: Vec<String>,
        duration: Option<Duration>,
        cancel: CancellationToken,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.ensure_accepting()?;
        let _execution = self.execution_gate.lock().await;
        self.ensure_accepting()?;
        self.busy.store(true, Ordering::Release);

        let mut worker = match self.worker.lock().await.take() {
            Some(worker) => worker,
            None => match ResidentWorker::start(self.config.clone()).await {
                Ok(worker) => worker,
                Err(error) => {
                    self.busy.store(false, Ordering::Release);
                    return Err(ResidentWorkerError::manager_error(
                        "WORKER_REPLACEMENT_FAILURE",
                        format!("replacement worker failed to start: {error}"),
                    ));
                }
            },
        };

        let result = match duration {
            Some(duration) => {
                worker
                    .execute_with_cancellation(request_id, args, duration, cancel)
                    .await
            }
            None => worker.execute(request_id, args).await,
        };

        if worker.is_healthy() {
            *self.worker.lock().await = Some(worker);
        }
        self.busy.store(false, Ordering::Release);
        result
    }

    fn ensure_accepting(&self) -> Result<(), ResidentWorkerError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(ResidentWorkerError::manager_error(
                "WORKER_MANAGER_SHUTTING_DOWN",
                "resident worker manager is shutting down",
            ));
        }
        Ok(())
    }
}
