#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};
use ytdlp_http_wrapper::worker_protocol::{
    read_frame, write_frame, Frame, MessageType, PROTOCOL_VERSION,
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

fn spawn_worker(python: &Path) -> Child {
    Command::new(python)
        .arg(worker_path())
        .env("YTDLP_GENERATION_ID", "generation-fixture")
        .env("YTDLP_WORKER_ID", "worker-fixture")
        .env("PYTHONPATH", fixture_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python fixture worker must start")
}

#[tokio::test]
async fn rust_and_python_exchange_worker_frames() {
    let Some(python) = find_python() else {
        eprintln!("skipping worker interop test because Python is unavailable");
        return;
    };
    let request_temp = tempfile::tempdir().expect("tempdir for interop request");
    let mut child = spawn_worker(&python);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let hello = read_frame(&mut stdout).await.unwrap().unwrap();
    assert_eq!(hello.protocol_version, PROTOCOL_VERSION);
    assert_eq!(hello.message_type, MessageType::Hello);
    assert_eq!(hello.request_id, "startup");
    assert_eq!(hello.payload["yt_dlp_version"], "fixture-version");
    assert_eq!(hello.payload["generation_id"], "generation-fixture");
    assert_eq!(hello.payload["worker_id"], "worker-fixture");

    write_frame(
        &mut stdin,
        &Frame::new(MessageType::Ready, "startup", json!({})).unwrap(),
    )
    .await
    .unwrap();
    write_frame(
        &mut stdin,
        &Frame::new(
            MessageType::Run,
            "request-1",
            json!({
                "args": ["first", "cookie=fixture-secret"],
                "request_temp": request_temp.path().to_string_lossy()
            }),
        )
        .unwrap(),
    )
    .await
    .unwrap();

    let result = read_frame(&mut stdout).await.unwrap().unwrap();
    result.validate_request_id("request-1").unwrap();
    assert_eq!(result.message_type, MessageType::Result);
    assert_eq!(result.payload["exit_code"], 0);
    assert_eq!(
        result.payload["stdout"],
        "fixture-instance-1:first,cookie=fixture-secret:first,cookie=fixture-secret\n"
    );

    write_frame(
        &mut stdin,
        &Frame::new(
            MessageType::Run,
            "request-2",
            json!({
                "args": ["second"],
                "request_temp": request_temp.path().to_string_lossy()
            }),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    let second_result = read_frame(&mut stdout).await.unwrap().unwrap();
    second_result.validate_request_id("request-2").unwrap();
    assert_eq!(second_result.message_type, MessageType::Result);
    assert_eq!(second_result.payload["exit_code"], 0);
    assert_eq!(
        second_result.payload["stdout"],
        "fixture-instance-2:second:second\n"
    );
    assert!(!second_result.payload["stdout"]
        .as_str()
        .unwrap()
        .contains("fixture-secret"));

    write_frame(
        &mut stdin,
        &Frame::new(MessageType::Shutdown, "shutdown-1", json!({})).unwrap(),
    )
    .await
    .unwrap();
    let shutdown = read_frame(&mut stdout).await.unwrap().unwrap();
    shutdown.validate_request_id("shutdown-1").unwrap();
    assert_eq!(shutdown.message_type, MessageType::Result);
    assert_eq!(shutdown.payload["shutdown"], true);

    drop(stdin);
    let status = child.wait().await.unwrap();
    let mut diagnostics = String::new();
    stderr.read_to_string(&mut diagnostics).await.unwrap();
    assert!(status.success(), "worker failed: {diagnostics}");
    assert!(
        diagnostics.is_empty(),
        "worker emitted diagnostics: {diagnostics}"
    );
}

#[tokio::test]
async fn python_worker_fails_closed_on_request_id_mismatch() {
    let Some(python) = find_python() else {
        eprintln!("skipping worker interop test because Python is unavailable");
        return;
    };
    let mut child = spawn_worker(&python);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let hello = read_frame(&mut stdout).await.unwrap().unwrap();
    assert_eq!(hello.message_type, MessageType::Hello);
    write_frame(
        &mut stdin,
        &Frame::new(MessageType::Ready, "wrong-startup-id", json!({})).unwrap(),
    )
    .await
    .unwrap();
    drop(stdin);

    let status = child.wait().await.unwrap();
    let mut diagnostics = String::new();
    stderr.read_to_string(&mut diagnostics).await.unwrap();
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        vec!["worker protocol failure"]
    );
}

#[tokio::test]
async fn python_worker_rejects_truncated_frames() {
    let Some(python) = find_python() else {
        eprintln!("skipping worker interop test because Python is unavailable");
        return;
    };
    let mut child = spawn_worker(&python);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let hello = read_frame(&mut stdout).await.unwrap().unwrap();
    assert_eq!(hello.message_type, MessageType::Hello);
    stdin.write_all(&20_u32.to_be_bytes()).await.unwrap();
    stdin.write_all(b"short").await.unwrap();
    drop(stdin);

    let status = child.wait().await.unwrap();
    let mut diagnostics = String::new();
    stderr.read_to_string(&mut diagnostics).await.unwrap();
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        vec!["worker protocol failure"]
    );
}

#[tokio::test]
async fn python_worker_treats_controller_eof_as_protocol_failure() {
    let Some(python) = find_python() else {
        eprintln!("skipping worker interop test because Python is unavailable");
        return;
    };
    let mut child = spawn_worker(&python);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let hello = read_frame(&mut stdout).await.unwrap().unwrap();
    assert_eq!(hello.message_type, MessageType::Hello);
    write_frame(
        &mut stdin,
        &Frame::new(MessageType::Ready, "startup", json!({})).unwrap(),
    )
    .await
    .unwrap();
    drop(stdin);

    let status = child.wait().await.unwrap();
    let mut diagnostics = String::new();
    stderr.read_to_string(&mut diagnostics).await.unwrap();
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        vec!["worker protocol failure"]
    );
}
