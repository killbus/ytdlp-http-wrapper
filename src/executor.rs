use axum::{http::StatusCode, response::IntoResponse, Json};
use std::env;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::warn;

use crate::models::{ErrorResponse, RunRequest};
use crate::routes::AppState;

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

fn redact_args(args: &[String]) -> Vec<String> {
    let sensitive = [
        "--cookies-from-browser",
        "--cookies",
        "--load-cookies",
        "--add-header",
        "--header",
        "--username",
        "--password",
        "--video-password",
        "--token",
        "--api-key",
    ];
    args.iter()
        .map(|arg| {
            if sensitive.iter().any(|s| arg.starts_with(s)) {
                if let Some(eq_pos) = arg.find('=') {
                    format!("{} [REDACTED]", &arg[..=eq_pos])
                } else {
                    format!("{} [REDACTED]", arg)
                }
            } else {
                arg.clone()
            }
        })
        .collect()
}

pub async fn execute(payload: RunRequest, state: Arc<AppState>) -> impl IntoResponse {
    if let Err(message) = reject_denied_args(&payload.args) {
        warn!(log_type = "audit", args = ?redact_args(&payload.args), "{}", message);
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "ARG_REJECTED", message);
    }
    let duration = Duration::from_secs(payload.timeout_seconds.unwrap_or(30).clamp(1, 300));
    let permit = match state.semaphore.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            return failure(
                StatusCode::SERVICE_UNAVAILABLE,
                "SHUTTING_DOWN",
                "Service is shutting down".into(),
            )
        }
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    state.tasks.spawn(crate::supervisor::supervise(
        payload.args,
        duration,
        state.clone(),
        permit,
        sender,
    ));
    match receiver.await {
        Ok(Ok(response)) => (StatusCode::OK, Json(serde_json::json!(response))),
        Ok(Err(e)) => failure(
            if e.code == "SHUTTING_DOWN" {
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
    }
}

fn failure(
    status: StatusCode,
    code: &'static str,
    error: String,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!(ErrorResponse { error, code })),
    )
}
