#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::time::Duration;
use ytdlp_http_wrapper::{
    resident_worker::ResidentWorkerConfig,
    resident_worker_manager::{ResidentWorkerManager, ResidentWorkerManagerStatus},
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

fn config(python: PathBuf, worker_id: &str) -> ResidentWorkerConfig {
    ResidentWorkerConfig {
        python_path: python,
        worker_path: fixture_path().join("resident_worker_runtime.py"),
        python_path_entries: vec![fixture_path()],
        generation_id: "generation-fixture".into(),
        worker_id: worker_id.into(),
        expected_yt_dlp_version: "fixture-version".into(),
    }
}

#[tokio::test]
async fn manager_reuses_worker_after_structured_business_error() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker manager test because Python is unavailable");
        return;
    };

    let manager = ResidentWorkerManager::start(config(python, "business-error"))
        .await
        .unwrap();
    let first = manager
        .execute("request-1", vec!["first".into()])
        .await
        .err()
        .expect("the fixture should return a structured business error");
    assert_eq!(first.code(), "WORKER_EXECUTION_FAILURE");
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Ready);

    let second = manager
        .execute("request-2", vec!["second".into()])
        .await
        .err()
        .expect("the reused worker should return a structured business error");
    assert_eq!(second.code(), "WORKER_EXECUTION_FAILURE");
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Ready);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn manager_does_not_retry_failed_request_and_replaces_for_next_request() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker manager test because Python is unavailable");
        return;
    };

    let manager = ResidentWorkerManager::start(config(python, "crash-eof"))
        .await
        .unwrap();
    let failed = manager
        .execute("request-1", vec!["first".into()])
        .await
        .err()
        .expect("the fixture worker should fail its first request");
    assert_eq!(failed.code(), "WORKER_PROTOCOL_FAILURE");
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Empty);

    let next = manager
        .execute("request-2", vec!["second".into()])
        .await
        .err()
        .expect("the replacement worker should fail only the next request");
    assert_eq!(next.code(), "WORKER_PROTOCOL_FAILURE");
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Empty);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn manager_retires_timed_out_worker_without_retrying_request() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker manager test because Python is unavailable");
        return;
    };

    let manager = ResidentWorkerManager::start(config(python, "request-timeout"))
        .await
        .unwrap();
    let failed = manager
        .execute_with_timeout(
            "request-timeout",
            vec!["fixture".into()],
            Duration::from_millis(100),
        )
        .await
        .err()
        .expect("the fixture request should time out");
    assert_eq!(failed.code(), "WORKER_TIMEOUT");
    assert_eq!(manager.status().await, ResidentWorkerManagerStatus::Empty);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn manager_rejects_requests_after_shutdown() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker manager test because Python is unavailable");
        return;
    };

    let manager = ResidentWorkerManager::start(config(python, "manager-shutdown"))
        .await
        .unwrap();
    manager.shutdown().await.unwrap();
    assert_eq!(
        manager.status().await,
        ResidentWorkerManagerStatus::ShuttingDown
    );

    let error = manager
        .execute("request-after-shutdown", Vec::new())
        .await
        .err()
        .expect("a shut down manager should reject new requests");
    assert_eq!(error.code(), "WORKER_MANAGER_SHUTTING_DOWN");
}
