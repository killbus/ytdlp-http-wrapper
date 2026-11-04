#![allow(clippy::expect_used, clippy::unwrap_used)]

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tower::ServiceExt;
use ytdlp_http_wrapper::{
    backend::Backend,
    resident_worker::ResidentWorkerConfig,
    resident_worker_manager::{ResidentWorkerManager, ResidentWorkerManagerStatus},
    routes::{self, AppState},
};

fn find_python() -> Option<PathBuf> {
    ["python", "python3"].into_iter().find_map(|candidate| {
        std::process::Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()
            .filter(|status| status.success())
            .map(|_| PathBuf::from(candidate))
    })
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn worker_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worker")
        .join("ytdlp_worker.py")
}

fn config(python: PathBuf, worker_id: &str) -> ResidentWorkerConfig {
    ResidentWorkerConfig {
        python_path: python,
        worker_path: worker_path(),
        python_path_entries: vec![fixture_path()],
        generation_id: "generation-fixture".into(),
        worker_id: worker_id.into(),
        expected_yt_dlp_version: "fixture-version".into(),
        temp_dir: None,
    }
}

/// The `request-timeout` and `business-error` behaviors exist only in the
/// scripted runtime fixture, so those tests must not use the real worker.
fn runtime_fixture_config(python: PathBuf, worker_id: &str) -> ResidentWorkerConfig {
    let mut config = config(python, worker_id);
    config.worker_path = fixture_path().join("resident_worker_runtime.py");
    config
}

fn state_with(
    manager: Option<Arc<ResidentWorkerManager>>,
    temp_dir: Option<PathBuf>,
) -> Arc<AppState> {
    let state = AppState::new(PathBuf::from("non-existent-yt-dlp"), 2, temp_dir)
        .with_backend(Backend::Resident(manager));
    Arc::new(state)
}

async fn post_run(state: &Arc<AppState>, body: &str) -> (StatusCode, Value) {
    let response = routes::app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/run")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn resident_backend_processes_requests_and_reclaims_temp_dirs() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident backend test because Python is unavailable");
        return;
    };
    let parent = tempfile::tempdir().unwrap();
    let manager = Arc::new(ResidentWorkerManager::for_config(config(
        python,
        "resident-backend",
    )));
    let state = state_with(Some(manager), Some(parent.path().into()));
    state.backend.start().await.unwrap();

    let (status, payload) =
        post_run(&state, r#"{"args":["--version"],"timeout_seconds":45}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["exit_code"], 0);
    assert_eq!(
        payload["stdout"],
        "fixture-instance-1:--version:--version\n"
    );

    // A second request reuses the same worker with a fresh yt-dlp instance.
    let (status, payload) = post_run(&state, r#"{"args":["second"],"timeout_seconds":45}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["stdout"], "fixture-instance-2:second:second\n");

    // The controller-owned per-request temp directories are reclaimed.
    let owned = std::fs::read_dir(parent.path())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        owned.is_empty(),
        "owned request temp directories must be removed after the request"
    );

    state.backend.shutdown().await;
}

#[tokio::test]
async fn resident_backend_timeout_is_an_infrastructure_failure_without_retry() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident backend test because Python is unavailable");
        return;
    };
    let manager = Arc::new(ResidentWorkerManager::for_config(runtime_fixture_config(
        python,
        "request-timeout",
    )));
    let state = state_with(Some(manager.clone()), None);
    state.backend.start().await.unwrap();

    let (status, payload) = post_run(&state, r#"{"args":["fixture"],"timeout_seconds":2}"#).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "WORKER_TIMEOUT");
    // The timed-out worker is retired and replaced only for the next request.
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Empty);
    state.backend.shutdown().await;
}

#[tokio::test]
async fn resident_backend_business_error_keeps_worker_and_surfaces_code() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident backend test because Python is unavailable");
        return;
    };
    let manager = Arc::new(ResidentWorkerManager::for_config(runtime_fixture_config(
        python,
        "business-error",
    )));
    let state = state_with(Some(manager.clone()), None);
    state.backend.start().await.unwrap();

    let (status, payload) = post_run(&state, r#"{"args":["fixture"],"timeout_seconds":45}"#).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(payload["code"], "FIXTURE_FAILURE");
    // A structured business error does not retire the worker.
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Ready);
    state.backend.shutdown().await;
}

#[tokio::test]
async fn unavailable_resident_backend_rejects_without_cli_fallback() {
    let state = state_with(None, None);

    let (status, payload) =
        post_run(&state, r#"{"args":["--version"],"timeout_seconds":45}"#).await;
    // The CLI backend is not used as a fallback (which would report
    // SPAWN_FAILURE here); the request is rejected explicitly.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(payload["code"], "SHUTTING_DOWN");
}

#[tokio::test]
async fn resident_backend_configuration_failures_are_explicit() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident backend test because Python is unavailable");
        return;
    };

    // Unknown backend values are startup errors, not a silent CLI default.
    let unknown = Backend::from_config(
        "bogus",
        python.clone(),
        worker_path(),
        vec![fixture_path()],
        "fixture-version",
        None,
    );
    assert!(unknown.is_err());

    // Resident without a pinned yt-dlp version fails at configuration.
    let missing_version = Backend::from_config(
        "resident",
        python.clone(),
        worker_path(),
        vec![fixture_path()],
        "",
        None,
    );
    assert!(missing_version.is_err());

    // A handshake that reports the wrong yt-dlp version fails at boot,
    // before any request is admitted.
    let mismatched = Backend::from_config(
        "resident",
        python.clone(),
        worker_path(),
        vec![fixture_path()],
        "expected-version",
        None,
    )
    .unwrap();
    assert!(mismatched.start().await.is_err());

    // The default backend stays the CLI backend.
    assert_eq!(Backend::default_backend().name(), "cli");
}
