#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
#[cfg(unix)]
use tokio::time::sleep;
use tokio::time::Duration;

use ytdlp_http_wrapper::resident_worker::{ResidentWorker, ResidentWorkerConfig};

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

fn worker_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worker")
        .join("ytdlp_worker.py")
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn handshake_fixture_path() -> PathBuf {
    fixture_path().join("resident_worker_handshake.py")
}

fn runtime_fixture_path() -> PathBuf {
    fixture_path().join("resident_worker_runtime.py")
}

fn config(python: PathBuf) -> ResidentWorkerConfig {
    ResidentWorkerConfig {
        python_path: python,
        worker_path: worker_path(),
        python_path_entries: vec![fixture_path()],
        generation_id: "generation-fixture".into(),
        worker_id: "worker-fixture".into(),
        expected_yt_dlp_version: "fixture-version".into(),
        temp_dir: None,
    }
}

async fn runtime_failure(worker_id: &str) -> (ResidentWorker, String) {
    let python = find_python().expect("Python is required by this helper");
    let mut runtime_config = config(python);
    runtime_config.worker_path = runtime_fixture_path();
    runtime_config.worker_id = worker_id.into();
    let mut worker = ResidentWorker::start(runtime_config).await.unwrap();
    let error = worker
        .execute("request-1", vec!["fixture".into()])
        .await
        .err()
        .expect("runtime fixture request should fail");
    (worker, error.code().to_owned())
}

#[tokio::test]
async fn resident_worker_reuses_process_for_serial_requests() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut worker = ResidentWorker::start(config(python)).await.unwrap();
    let process_id = worker.process_id();

    let first = worker
        .execute("request-1", vec!["first".into()])
        .await
        .unwrap();
    let second = worker
        .execute("request-2", vec!["second".into()])
        .await
        .unwrap();

    assert_eq!(worker.process_id(), process_id);
    assert_eq!(first.exit_code, 0);
    assert_eq!(first.stdout, "fixture-instance-1:first:first\n");
    assert_eq!(second.exit_code, 0);
    assert_eq!(second.stdout, "fixture-instance-2:second:second\n");

    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn resident_worker_rejects_generation_mismatch() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.worker_path = handshake_fixture_path();
    mismatched.generation_id = "expected-generation".into();
    mismatched.worker_id = "generation-mismatch".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_rejects_worker_id_mismatch() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.worker_path = handshake_fixture_path();
    mismatched.worker_id = "worker-id-mismatch".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_rejects_non_hello_first_frame() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.worker_path = handshake_fixture_path();
    mismatched.worker_id = "non-hello-first-frame".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_rejects_startup_request_id_mismatch() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.worker_path = handshake_fixture_path();
    mismatched.worker_id = "startup-request-id-mismatch".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_rejects_invalid_hello_payload() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.worker_path = handshake_fixture_path();
    mismatched.worker_id = "invalid-hello-payload".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_rejects_yt_dlp_version_mismatch() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut mismatched = config(python);
    mismatched.expected_yt_dlp_version = "expected-version".into();

    let error = ResidentWorker::start(mismatched).await.unwrap_err();
    assert_eq!(error.code(), "WORKER_HANDSHAKE_FAILURE");
}

#[tokio::test]
async fn resident_worker_retires_after_result_request_id_mismatch() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("request-id-mismatch").await;
    assert_eq!(code, "WORKER_PROTOCOL_FAILURE");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_retires_after_worker_crash_or_eof() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("crash-eof").await;
    assert_eq!(code, "WORKER_PROTOCOL_FAILURE");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_retires_after_unexpected_response_frame() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("unexpected-frame").await;
    assert_eq!(code, "WORKER_PROTOCOL_FAILURE");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_retires_after_invalid_result_payload() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("invalid-result-payload").await;
    assert_eq!(code, "WORKER_PROTOCOL_FAILURE");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_retires_when_worker_writes_stderr() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("stderr-diagnostic").await;
    assert_eq!(code, "WORKER_PROTOCOL_FAILURE");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_retires_when_result_stream_exceeds_limit() {
    if find_python().is_none() {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    }

    let (mut worker, code) = runtime_failure("oversized-stdout").await;
    assert_eq!(code, "WORKER_OUTPUT_LIMIT");
    let retired = worker
        .execute("request-2", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[tokio::test]
async fn resident_worker_timeout_retires_worker() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let mut runtime_config = config(python);
    runtime_config.worker_path = runtime_fixture_path();
    runtime_config.worker_id = "request-timeout".into();
    let mut worker = ResidentWorker::start(runtime_config).await.unwrap();

    let error = worker
        .execute_with_timeout(
            "request-timeout",
            vec!["fixture".into()],
            Duration::from_millis(100),
        )
        .await
        .err()
        .expect("timed-out request should fail");
    assert_eq!(error.code(), "WORKER_TIMEOUT");
    assert!(
        worker.process_id().is_none(),
        "timed-out worker process should be reaped before timeout returns"
    );

    let retired = worker
        .execute("request-after-timeout", Vec::new())
        .await
        .err()
        .expect("retired worker should reject another request");
    assert_eq!(retired.code(), "WORKER_UNHEALTHY");
}

#[cfg(unix)]
#[tokio::test]
async fn resident_worker_timeout_reaps_descendant_process_group() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let temp = tempfile::tempdir().unwrap();
    let heartbeat = temp.path().join("descendant-heartbeat");
    let mut runtime_config = config(python);
    runtime_config.worker_path = runtime_fixture_path();
    runtime_config.worker_id = "descendant-timeout".into();
    let mut worker = ResidentWorker::start(runtime_config).await.unwrap();

    let error = worker
        .execute_with_timeout(
            "descendant-timeout",
            vec![heartbeat.to_string_lossy().into_owned()],
            Duration::from_millis(250),
        )
        .await
        .err()
        .expect("timed-out descendant request should fail");

    assert_eq!(error.code(), "WORKER_TIMEOUT");
    assert!(
        worker.process_id().is_none(),
        "worker leader should be reaped before timeout returns"
    );
    let stopped_value = std::fs::read(&heartbeat).expect("descendant should write heartbeat");
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        stopped_value,
        std::fs::read(&heartbeat).expect("heartbeat should remain readable"),
        "worker descendant survived process-group cleanup"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resident_worker_shutdown_reports_and_reaps_surviving_descendant() {
    let Some(python) = find_python() else {
        eprintln!("skipping resident worker test because Python is unavailable");
        return;
    };

    let temp = tempfile::tempdir().unwrap();
    let heartbeat = temp.path().join("shutdown-descendant-heartbeat");
    let mut runtime_config = config(python);
    runtime_config.worker_path = runtime_fixture_path();
    runtime_config.worker_id = "descendant-shutdown".into();
    let mut worker = ResidentWorker::start(runtime_config).await.unwrap();

    let response = worker
        .execute(
            "descendant-shutdown",
            vec![heartbeat.to_string_lossy().into_owned()],
        )
        .await
        .unwrap();
    assert_eq!(response.exit_code, 0);

    let error = worker
        .shutdown()
        .await
        .expect_err("shutdown with a surviving descendant should fail");
    assert_eq!(error.code(), "WORKER_SHUTDOWN_FAILURE");

    let stopped_value =
        std::fs::read(&heartbeat).expect("shutdown descendant should write heartbeat");
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        stopped_value,
        std::fs::read(&heartbeat).expect("heartbeat should remain readable"),
        "shutdown descendant survived forced process-group cleanup"
    );
}
