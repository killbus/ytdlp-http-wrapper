use axum::{http::StatusCode, response::IntoResponse, Json};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn};

use crate::models::{ErrorResponse, RunRequest};
use crate::routes::AppState;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    format!(
        "request-{}",
        NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn default_denied_args() -> Vec<String> {
    vec![
        "--exec",
        "--exec-before-download",
        "--alias",
        "--config-locations",
        "--load-info-json",
        "--plugin-dirs",
        "--ffmpeg-location",
        "--downloader-args",
        "--postprocessor-args",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn denied_args() -> &'static Vec<String> {
    static DENIED: OnceLock<Vec<String>> = OnceLock::new();
    DENIED.get_or_init(|| match env::var("DENIED_ARGS") {
        Ok(val) => serde_json::from_str(&val).unwrap_or_else(|_| default_denied_args()),
        Err(_) => default_denied_args(),
    })
}

fn reject_denied_args(args: &[String]) -> Result<(), String> {
    let denied = denied_args();
    if denied.is_empty() {
        return Ok(());
    }
    for arg in args {
        let key = arg.split('=').next().unwrap_or(arg);
        if denied.iter().any(|d| d == key) {
            return Err(format!(
                "Argument '{}' is not allowed by DENIED_ARGS policy",
                key
            ));
        }
    }
    Ok(())
}

pub async fn execute(payload: RunRequest, state: Arc<AppState>) -> impl IntoResponse {
    let request_id = next_request_id();
    let request_started = Instant::now();

    if let Err(message) = reject_denied_args(&payload.args) {
        warn!(
            request_id,
            backend = %state.backend.name(),
            log_type = "audit",
            result = "argument_rejected",
            "{}",
            message
        );
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "ARG_REJECTED", message);
    }
    let duration = Duration::from_secs(payload.timeout_seconds.unwrap_or(30).clamp(1, 300));
    let update_requested = payload
        .args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| {
            matches!(arg.as_str(), "--update" | "-U" | "--update-to")
                || arg.starts_with("--update-to=")
        });
    if update_requested {
        if payload.args.len() != 1 || !matches!(payload.args[0].as_str(), "--update" | "-U") {
            return failure(StatusCode::UNPROCESSABLE_ENTITY, "UPDATE_ARGUMENTS", "Use --update or -U alone; combined update/download requests and --update-to are not supported".into());
        }
        if state.updater.is_none() {
            return failure(
                StatusCode::SERVICE_UNAVAILABLE,
                "UPDATE_UNAVAILABLE",
                "Service runtime updates are not configured".into(),
            );
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let update_state = state.clone();
        state.tasks.spawn(async move {
            if let Some(updater) = &update_state.updater {
                updater.run(update_state.clone(), duration, sender).await;
            }
        });
        return match receiver.await {
            Ok(Ok(response)) => (StatusCode::OK, Json(serde_json::json!(response))),
            Ok(Err(error)) => failure(StatusCode::INTERNAL_SERVER_ERROR, error.code, error.message),
            Err(error) => failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "UPDATE_FAILURE",
                error.to_string(),
            ),
        };
    }
    // The tracked supervisor owns this lease through process/temp cleanup, even
    // if the HTTP handler disappears. An update cannot replace in-use files.
    let runtime_lease = if let Some(updater) = &state.updater {
        Some(tokio::select! {
            lease = updater.active.clone().read_owned() => lease,
            _ = state.shutdown.cancelled() => return failure(StatusCode::SERVICE_UNAVAILABLE, "SHUTTING_DOWN", "Service is shutting down".into()),
        })
    } else {
        None
    };
    let state = runtime_lease
        .as_ref()
        .map(|lease| lease.state.clone())
        .unwrap_or(state);
    let queue_started = Instant::now();
    let permit = match state.semaphore.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            info!(
                request_id,
                backend = %state.backend.name(),
                queue_ms = queue_started.elapsed().as_millis() as u64,
                response_ready_ms = request_started.elapsed().as_millis() as u64,
                result = "shutting_down",
                "Request rejected before dispatch"
            );
            return failure(
                StatusCode::SERVICE_UNAVAILABLE,
                "SHUTTING_DOWN",
                "Service is shutting down".into(),
            );
        }
    };
    let queue_ms = queue_started.elapsed().as_millis() as u64;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    match state.backend.clone() {
        // The CLI backend stays the default; selection happens before dispatch.
        crate::backend::Backend::Cli => {
            let run_state = state.clone();
            let run_request_id = request_id.clone();
            state.tasks.spawn(async move {
                crate::supervisor::supervise(
                    run_request_id,
                    payload.args,
                    duration,
                    queue_ms,
                    run_state,
                    permit,
                    sender,
                )
                .await;
                drop(runtime_lease);
            });
        }
        crate::backend::Backend::Resident(_) => {
            // Anchor the absolute deadline at request start (design.md section
            // 5) so semaphore queue time and the pool's slot wait both count
            // against it; the pool then hands the worker only the remaining
            // duration.
            let deadline = request_started + duration;
            let run_state = state.clone();
            let run_request_id = request_id.clone();
            state.tasks.spawn(async move {
                crate::backend::reside(
                    run_request_id,
                    payload.args,
                    deadline,
                    queue_ms,
                    run_state,
                    permit,
                    sender,
                )
                .await;
                drop(runtime_lease);
            });
        }
    }
    let response = match receiver.await {
        Ok(Ok(response)) => (StatusCode::OK, Json(serde_json::json!(response))),
        Ok(Err(e)) => failure(
            // SHUTTING_DOWN and pool saturation are both "cannot serve this
            // request right now" rejections, so both map to 503 (design.md
            // section 5: WORKER_POOL_SATURATED gets the same treatment).
            if e.code == "SHUTTING_DOWN" || e.code == "WORKER_POOL_SATURATED" {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            e.code,
            e.message,
        ),
        Err(e) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SUPERVISOR_FAILURE",
            e.to_string(),
        ),
    };
    info!(
        request_id,
        backend = %state.backend.name(),
        queue_ms,
        response_ready_ms = request_started.elapsed().as_millis() as u64,
        status = response.0.as_u16(),
        "Request response ready"
    );
    response
}

fn failure(
    status: StatusCode,
    code: impl Into<String>,
    error: String,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!(ErrorResponse {
            error,
            code: code.into()
        })),
    )
}
