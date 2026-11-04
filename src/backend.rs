//! Backend abstraction for `/run` execution.
//!
//! Both backends return the existing `RunResponse` contract. The CLI backend is
//! the default; the resident backend is only reachable through explicit,
//! non-default configuration. Once a resident dispatch starts there is no
//! fallback to the CLI backend: silent fallback could duplicate side effects.

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{oneshot, OwnedSemaphorePermit};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::resident_worker::{ResidentWorkerConfig, ResidentWorkerError};
use crate::resident_worker_manager::ResidentWorkerManager;
use crate::routes::AppState;

pub(crate) type Reply = crate::supervisor::Reply;

/// The execution backend selected for `/run`. An enum (not a trait object) so it
/// is cheaply cloneable across the HTTP layer without extra dependencies.
///
/// - `Cli` (default): one supervised process per request.
/// - `Resident`: a single supervised resident worker. `None` means the backend
///   was explicitly requested but its worker failed to start; requests are
///   rejected with 503 rather than falling back to the CLI backend.
#[derive(Clone)]
pub enum Backend {
    Cli,
    Resident(Option<std::sync::Arc<ResidentWorkerManager>>),
}

impl Backend {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Resident(_) => "resident",
        }
    }

    /// Default backend. The CLI backend remains the default until the resident
    /// backend passes its compatibility gates.
    pub fn default_backend() -> Self {
        Self::Cli
    }

    /// Selects the backend from explicit configuration. Only the value
    /// `resident` enables the resident backend; `cli` (and the unset default)
    /// stays on the CLI backend. Unknown values are a startup error rather than
    /// a silent CLI default.
    pub fn from_config(
        value: &str,
        python_path: PathBuf,
        worker_path: PathBuf,
        python_path_entries: Vec<PathBuf>,
        expected_yt_dlp_version: &str,
        temp_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        match value {
            "cli" | "" => Ok(Self::Cli),
            "resident" => {
                if expected_yt_dlp_version.is_empty() {
                    return Err(
                        "backend=resident requires --worker-yt-dlp-version (the pinned yt-dlp \
                         version the worker handshake must report)"
                            .into(),
                    );
                }
                let config = ResidentWorkerConfig {
                    python_path,
                    worker_path,
                    python_path_entries,
                    generation_id: "resident".into(),
                    worker_id: "worker-1".into(),
                    expected_yt_dlp_version: expected_yt_dlp_version.into(),
                    temp_dir,
                };
                Ok(Self::Resident(Some(std::sync::Arc::new(
                    ResidentWorkerManager::for_config(config),
                ))))
            }
            other => Err(format!(
                "unknown backend '{other}' (expected cli or resident)"
            )),
        }
    }

    /// Starts the resident worker (spawn + handshake) so configuration errors
    /// surface at boot, before any request is admitted.
    pub async fn start(&self) -> Result<(), String> {
        if let Self::Resident(manager) = self {
            let Some(manager) = manager else {
                return Ok(());
            };
            manager
                .ensure_started()
                .await
                .map_err(|error| format!("resident worker failed to start: {error}"))
        } else {
            Ok(())
        }
    }

    /// Stops the resident worker gracefully. The CLI backend has nothing to stop.
    pub async fn shutdown(&self) {
        if let Self::Resident(Some(manager)) = self {
            if let Err(error) = manager.shutdown().await {
                warn!(%error, "resident worker shutdown failed");
            }
        }
    }
}

/// Executes one admitted request on the resident backend and returns the
/// `RunResponse` contract (or an infrastructure `Failure`). Spawned as a tracked
/// task by the executor, mirroring `supervisor::supervise` for the CLI backend.
///
/// The clamped deadline is always applied through `execute_with_timeout`; the
/// unbounded `execute` entry point is test-only because it would hold the
/// manager gate for the (unbounded) request duration and deadlock shutdown.
pub(crate) async fn reside(
    request_id: String,
    args: Vec<String>,
    deadline: Duration,
    queue_ms: u64,
    state: std::sync::Arc<AppState>,
    permit: OwnedSemaphorePermit,
    mut reply: oneshot::Sender<Reply>,
) {
    let started = Instant::now();

    // Mirror the CLI supervisor's pre-dispatch cancellation check: a service
    // shutdown or an already-disconnected client stops the request before it
    // leases the single warm worker.
    if state.shutdown.is_cancelled() || reply.is_closed() {
        info!(
            request_id = %request_id,
            backend = "resident",
            queue_ms,
            result = "shutting_down",
            "resident backend acquisition stopped before dispatch"
        );
        let _ = reply.send(Err(crate::supervisor::Failure::new(
            "SHUTTING_DOWN",
            "Request cancelled before dispatch",
        )));
        return;
    }

    let result = match &state.backend {
        Backend::Resident(manager) => match manager {
            Some(manager) => {
                let generation_id = manager.generation_id().to_owned();
                let worker_id = manager.worker_id().to_owned();
                let result = dispatch_with_cancellation(
                    manager,
                    &request_id,
                    args,
                    deadline,
                    &state,
                    &mut reply,
                )
                .await;
                info!(
                    request_id = %request_id,
                    backend = "resident",
                    generation_id = %generation_id,
                    worker_id = %worker_id,
                    queue_ms,
                    supervisor_ms = started.elapsed().as_millis() as u64,
                    result = if result.is_ok() { "completed" } else { "failed" },
                    "resident worker request finished"
                );
                result
            }
            None => Err(crate::supervisor::Failure::new(
                "SHUTTING_DOWN",
                "resident backend is unavailable",
            )),
        },
        // The executor routes the CLI backend to `supervisor::supervise`.
        Backend::Cli => unreachable!("CLI requests are dispatched to the CLI supervisor"),
    };

    let result_kind = match &result {
        Ok(response) if response.exit_code == -1 => "interrupted",
        Ok(_) => "completed",
        Err(failure) => failure.code.as_str(),
    };
    info!(
        request_id = %request_id,
        backend = "resident",
        queue_ms,
        supervisor_ms = started.elapsed().as_millis() as u64,
        result = result_kind,
        success = result.is_ok(),
        "resident backend supervision finished"
    );
    if let Err(failure) = &result {
        error_resident(&request_id, failure);
    }
    // Release the permit before publishing the response, only after cleanup.
    drop(permit);
    let _ = reply.send(result);
}

fn error_resident(request_id: &str, failure: &crate::supervisor::Failure) {
    tracing::error!(
        request_id = %request_id,
        backend = "resident",
        code = failure.code,
        error = %failure.message,
        "resident worker request failed"
    );
}

/// Dispatches the request on the resident backend while observing client and
/// service cancellation, mirroring the CLI supervisor's `reply.closed()` select
/// branch and `state.shutdown.cancelled()` branch.
///
/// The cancellation trigger is handed to the manager, which forwards it to the
/// leased worker. The worker then runs the design section 5 retirement sequence
/// it already uses for timeouts (cooperative cancel frame, bounded grace,
/// `terminate_preserving`, replacement for the next request). Because that
/// sequence must run to completion, the dispatch future is awaited after the
/// trigger fires instead of being dropped: dropping it would kill the worker
/// without reclaiming its process tree or its request temp directory, and would
/// release the admission permit before cleanup finished.
async fn dispatch_with_cancellation(
    manager: &std::sync::Arc<ResidentWorkerManager>,
    request_id: &str,
    args: Vec<String>,
    deadline: Duration,
    state: &std::sync::Arc<AppState>,
    reply: &mut oneshot::Sender<Reply>,
) -> Reply {
    let cancel = CancellationToken::new();
    let dispatch = manager.execute_with_cancellation(request_id, args, deadline, cancel.clone());
    // Pinning keeps the dispatch future alive after the select below: awaiting
    // it to completion is what performs the retirement sequence.
    tokio::pin!(dispatch);

    tokio::select! {
        biased;
        _ = reply.closed() => cancel.cancel(),
        _ = state.shutdown.cancelled() => cancel.cancel(),
        result = &mut dispatch => return map_dispatch(result),
    }
    map_dispatch(dispatch.await)
}

/// Converts the manager's worker-level result into the public `Reply` shape.
fn map_dispatch(result: Result<crate::models::RunResponse, ResidentWorkerError>) -> Reply {
    result.map_err(|error| failure_from_worker_error(&error))
}

/// Maps a resident worker error to the supervisor's public `Failure` shape.
/// The manager's internal shutting-down code is normalized to `SHUTTING_DOWN`
/// so the executor maps it to HTTP 503 like the CLI backend does.
pub(crate) fn failure_from_worker_error(error: &ResidentWorkerError) -> crate::supervisor::Failure {
    let code = match error.code() {
        "WORKER_MANAGER_SHUTTING_DOWN" => "SHUTTING_DOWN".to_owned(),
        other => other.to_owned(),
    };
    crate::supervisor::Failure {
        code,
        message: error.to_string(),
    }
}
