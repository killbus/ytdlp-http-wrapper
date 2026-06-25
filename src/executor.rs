use axum::{http::StatusCode, response::IntoResponse, Json};
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::models::{ErrorResponse, RunRequest, RunResponse};

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

async fn read_pipe<R>(mut reader: R) -> String
where
    R: AsyncRead + Unpin,
{
    let mut buf = String::new();
    let _ = reader.read_to_string(&mut buf).await;
    buf
}

pub async fn execute(payload: RunRequest, binary_path: &PathBuf) -> impl IntoResponse {
    let start = Instant::now();

    if let Err(msg) = reject_denied_args(&payload.args) {
        warn!(
            log_type = "audit",
            args = ?redact_args(&payload.args),
            timeout_seconds = payload.timeout_seconds,
            "{}", msg
        );
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                serde_json::to_value(ErrorResponse {
                    error: msg,
                    code: "ARG_REJECTED",
                })
                .unwrap_or_else(|e| {
                    error!(error = %e, "Failed to serialize ErrorResponse");
                    serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                }),
            ),
        );
    }

    let timeout_duration = Duration::from_secs(payload.timeout_seconds.unwrap_or(30).clamp(1, 300));

    let mut cmd = Command::new(binary_path);
    cmd.args(&payload.args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to spawn yt-dlp");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::to_value(ErrorResponse {
                        error: format!("Failed to spawn yt-dlp process: {}", e),
                        code: "SPAWN_FAILURE",
                    })
                    .unwrap_or_else(|e| {
                        error!(error = %e, "Failed to serialize ErrorResponse");
                        serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                    }),
                ),
            );
        }
    };

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let result = timeout(timeout_duration, child.wait()).await;
    let elapsed = start.elapsed();

    match result {
        Ok(Ok(status)) => {
            let stdout = match stdout_handle {
                Some(reader) => read_pipe(reader).await,
                None => String::new(),
            };
            let stderr = match stderr_handle {
                Some(reader) => read_pipe(reader).await,
                None => String::new(),
            };
            let exit_code = status.code().unwrap_or(-1);

            info!(
                exit_code,
                duration_ms = elapsed.as_millis() as u64,
                stdout_len = stdout.len(),
                stderr_len = stderr.len(),
                args = ?redact_args(&payload.args),
                "yt-dlp completed"
            );

            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(RunResponse {
                        exit_code,
                        stdout,
                        stderr,
                    })
                    .unwrap_or_else(|e| {
                        error!(error = %e, "Failed to serialize RunResponse");
                        serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                    }),
                ),
            )
        }
        Ok(Err(e)) => {
            error!(
                error = %e,
                duration_ms = elapsed.as_millis() as u64,
                "Failed to collect yt-dlp output"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::to_value(ErrorResponse {
                        error: format!("Failed to collect process output: {}", e),
                        code: "COLLECT_FAILURE",
                    })
                    .unwrap_or_else(|e| {
                        error!(error = %e, "Failed to serialize ErrorResponse");
                        serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                    }),
                ),
            )
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;

            let stdout = match stdout_handle {
                Some(reader) => read_pipe(reader).await,
                None => String::new(),
            };
            let stderr = match stderr_handle {
                Some(reader) => read_pipe(reader).await,
                None => String::new(),
            };

            warn!(
                duration_ms = elapsed.as_millis() as u64,
                exit_code = -1,
                args = ?redact_args(&payload.args),
                "yt-dlp timed out"
            );
            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(RunResponse {
                        exit_code: -1,
                        stdout,
                        stderr,
                    })
                    .unwrap_or_else(|e| {
                        error!(error = %e, "Failed to serialize RunResponse");
                        serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                    }),
                ),
            )
        }
    }
}
