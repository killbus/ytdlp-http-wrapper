//! The ignored tests are native subprocess fixtures, launched through the executor.
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use std::{fs, io::Write, path::PathBuf, process::Command, sync::Arc, time::Duration};
use tempfile::TempDir;
#[cfg(unix)]
use tokio::time::Instant;
use tokio::time::{sleep, timeout};
use ytdlp_http_wrapper::{executor, models::RunRequest, routes::AppState};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn fixture_temp() -> PathBuf {
    let path = std::env::temp_dir();
    assert!(path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("ytdlp-request-")));
    path
}

fn residue() -> TestResult {
    let temp = fixture_temp();
    assert_eq!(std::env::var_os("TMPDIR"), std::env::var_os("TMP"));
    assert_eq!(std::env::var_os("TMP"), std::env::var_os("TEMP"));
    fs::create_dir_all(temp.join("_MEI_fixture"))?;
    fs::write(temp.join("_MEI_fixture/data"), "extraction")?;
    fs::write(temp.join("ready"), "ready")?;
    Ok(())
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_normal() -> TestResult {
    residue()?;
    println!("fixture stdout");
    eprintln!("fixture stderr");
    Ok(())
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_hang() -> TestResult {
    residue()?;
    std::thread::sleep(Duration::from_secs(60));
    Ok(())
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_output() -> TestResult {
    residue()?;
    let bytes = vec![b'x'; 11 * 1024 * 1024];
    std::io::stdout().write_all(&bytes)?;
    std::io::stderr().write_all(&bytes)?;
    Ok(())
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_child() -> TestResult {
    let temp = fixture_temp();
    let heartbeat = temp
        .parent()
        .ok_or("Missing temp parent")?
        .join("heartbeat");
    fs::write(temp.join("child-ready"), "ready")?;
    for n in 0..3000 {
        fs::write(&heartbeat, n.to_string())?;
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn descendant(quiet: bool) -> TestResult {
    residue()?;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(["--exact", "fixture_child", "--ignored", "--nocapture"]);
    if quiet {
        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    let _child = command.spawn()?;
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while !fixture_temp().join("child-ready").exists() {
        assert!(std::time::Instant::now() < until, "Child did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(()) // Leader exits, but its descendant still runs.
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_pipe_holder() -> TestResult {
    descendant(false)
}

#[test]
#[ignore = "subprocess fixture"]
fn fixture_quiet_descendant() -> TestResult {
    descendant(true)
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess fixture"]
fn fixture_cooperative() -> TestResult {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        residue()?;
        term.recv().await;
        println!("cooperative termination");
        Ok(())
    })
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess fixture"]
fn fixture_ignore_term() -> TestResult {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let _term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        residue()?;
        sleep(Duration::from_secs(60)).await;
        Ok(())
    })
}

fn state(parent: &TempDir, limit: usize) -> Result<Arc<AppState>, std::io::Error> {
    Ok(Arc::new(AppState::new(
        std::env::current_exe()?,
        limit,
        Some(parent.path().to_path_buf()),
    )))
}

async fn run(
    state: Arc<AppState>,
    fixture: &str,
    seconds: u64,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    let response = timeout(
        Duration::from_secs(12),
        executor::execute(
            RunRequest {
                args: vec![
                    "--exact".into(),
                    fixture.into(),
                    "--ignored".into(),
                    "--nocapture".into(),
                ],
                timeout_seconds: Some(seconds),
            },
            state,
        ),
    )
    .await?
    .into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 25 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body)?))
}

fn request_dirs(parent: &TempDir) -> Result<Vec<PathBuf>, std::io::Error> {
    fs::read_dir(parent.path())?
        .filter_map(|entry| match entry {
            Ok(entry)
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("ytdlp-request-") =>
            {
                Some(Ok(entry.path()))
            }
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
        .collect()
}

async fn ready(parent: &TempDir) -> TestResult {
    timeout(Duration::from_secs(8), async {
        loop {
            if request_dirs(parent)?
                .iter()
                .any(|path| path.join("ready").exists())
            {
                return Ok::<(), std::io::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn normal_and_spawn_failure_reclaim_only_owned_files() -> TestResult {
    let parent = tempfile::tempdir()?;
    fs::create_dir(parent.path().join("_MEI_unrelated"))?;
    let state = state(&parent, 1)?;
    let (status, body) = run(state.clone(), "fixture_normal", 10).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["exit_code"], 0);
    assert!(body["stdout"]
        .as_str()
        .is_some_and(|out| out.contains("fixture stdout")));
    assert!(body["stderr"]
        .as_str()
        .is_some_and(|out| out.contains("fixture stderr")));
    assert!(request_dirs(&parent)?.is_empty());
    assert!(parent.path().join("_MEI_unrelated").exists());
    assert_eq!(state.semaphore.available_permits(), 1);
    let missing = Arc::new(AppState::new(
        parent.path().join("missing"),
        1,
        Some(parent.path().into()),
    ));
    let (status, body) = run(missing, "unused", 10).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "SPAWN_FAILURE");
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn repeated_timeouts_leave_no_residue() -> TestResult {
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    for _ in 0..3 {
        let (status, body) = run(state.clone(), "fixture_hang", 1).await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["exit_code"], -1);
        assert!(request_dirs(&parent)?.is_empty());
        assert_eq!(state.semaphore.available_permits(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_requests_have_independent_directories() -> TestResult {
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 2)?;
    let first = tokio::spawn(run(state.clone(), "fixture_hang", 30));
    let second = tokio::spawn(run(state.clone(), "fixture_hang", 30));
    timeout(Duration::from_secs(8), async {
        loop {
            let dirs = request_dirs(&parent)?;
            if dirs.len() == 2 && dirs.iter().all(|p| p.join("ready").exists()) {
                return Ok::<(), std::io::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    first.abort();
    let _ = first.await;
    timeout(Duration::from_secs(8), async {
        while state.semaphore.available_permits() != 1 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let remaining = request_dirs(&parent)?;
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].join("_MEI_fixture/data").exists());
    assert!(!second.is_finished());
    second.abort();
    let _ = second.await;
    timeout(Duration::from_secs(8), state.wait_for_cleanup()).await?;
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn invalid_temp_parent_is_reported_without_spawning() -> TestResult {
    let parent = tempfile::tempdir()?;
    let file = parent.path().join("not-a-directory");
    fs::write(&file, "keep")?;
    let state = Arc::new(AppState::new(
        std::env::current_exe()?,
        1,
        Some(file.clone()),
    ));
    let (status, body) = run(state.clone(), "fixture_normal", 10).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "TEMP_FAILURE");
    assert_eq!(fs::read_to_string(file)?, "keep");
    assert_eq!(state.semaphore.available_permits(), 1);
    assert!(!state.semaphore.is_closed());
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn deletion_failure_is_visible_and_closes_admission() -> TestResult {
    use std::os::windows::fs::OpenOptionsExt;
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    let active = tokio::spawn(run(state.clone(), "fixture_hang", 1));
    ready(&parent).await?;
    let dirs = request_dirs(&parent)?;
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dirs[0].join("_MEI_fixture/data"))?;
    let (status, body) = active.await??;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "TEMP_CLEANUP_FAILURE");
    assert!(state.semaphore.is_closed());
    assert!(dirs[0].exists());
    assert_eq!(
        run(state.clone(), "fixture_normal", 1).await?.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(locked);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_during_grace_retains_permit_until_reclamation() -> TestResult {
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    let active = tokio::spawn(run(state.clone(), "fixture_ignore_term", 1));
    ready(&parent).await?;
    sleep(Duration::from_millis(1200)).await;
    active.abort();
    let _ = active.await;
    assert_eq!(state.semaphore.available_permits(), 0);
    assert_eq!(request_dirs(&parent)?.len(), 1);
    timeout(Duration::from_secs(8), state.wait_for_cleanup()).await?;
    assert_eq!(state.semaphore.available_permits(), 1);
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn output_limit_continues_draining_both_pipes() -> TestResult {
    let parent = tempfile::tempdir()?;
    let (status, body) = run(state(&parent, 1)?, "fixture_output", 10).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["exit_code"], 0);
    for key in ["stdout", "stderr"] {
        assert_eq!(
            body[key].as_str().ok_or("Missing output")?.len(),
            10 * 1024 * 1024
        );
    }
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn handler_cancellation_and_queued_cancellation() -> TestResult {
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    let active = tokio::spawn(run(state.clone(), "fixture_hang", 30));
    ready(&parent).await?;
    assert_eq!(state.semaphore.available_permits(), 0);
    let queued = tokio::spawn(run(state.clone(), "fixture_normal", 30));
    sleep(Duration::from_millis(50)).await;
    queued.abort();
    let _ = queued.await;
    assert_eq!(request_dirs(&parent)?.len(), 1);
    active.abort();
    let _ = active.await;
    timeout(Duration::from_secs(8), state.wait_for_cleanup()).await?;
    assert!(request_dirs(&parent)?.is_empty());
    assert_eq!(state.semaphore.available_permits(), 1);
    Ok(())
}

#[tokio::test]
async fn shutdown_cancels_active_and_rejects_queued_requests() -> TestResult {
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    let active = tokio::spawn(run(state.clone(), "fixture_hang", 30));
    ready(&parent).await?;
    let queued = tokio::spawn(run(state.clone(), "fixture_normal", 30));
    state.begin_shutdown();
    let (status, body) = active.await??;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["exit_code"], -1);
    assert_eq!(queued.await??.0, StatusCode::SERVICE_UNAVAILABLE);
    timeout(Duration::from_secs(8), state.wait_for_cleanup()).await?;
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn shutdown_after_admission_before_spawn_returns_unavailable() -> TestResult {
    use std::{future::Future, task::Poll};
    let parent = tempfile::tempdir()?;
    let state = state(&parent, 1)?;
    let mut request = std::pin::pin!(executor::execute(
        RunRequest {
            args: vec![],
            timeout_seconds: Some(10),
        },
        state.clone(),
    ));
    // On this current-thread runtime, poll admission without yielding to the
    // newly scheduled supervisor. Shutdown must win before any child starts.
    std::future::poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(state.semaphore.available_permits(), 0);
    state.begin_shutdown();
    let response = request.await.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    timeout(Duration::from_secs(8), state.wait_for_cleanup()).await?;
    assert!(request_dirs(&parent)?.is_empty());
    Ok(())
}

#[tokio::test]
async fn exited_leader_descendants_are_terminated_with_or_without_open_pipes() -> TestResult {
    for fixture in ["fixture_pipe_holder", "fixture_quiet_descendant"] {
        let parent = tempfile::tempdir()?;
        let (status, body) = run(state(&parent, 1)?, fixture, 1).await?;
        assert_eq!(status, StatusCode::OK, "{fixture}: {body}");
        assert!(
            body["stdout"]
                .as_str()
                .is_some_and(|out| out.contains("test result: ok")),
            "The leader must exit successfully before its descendant is killed: {body}"
        );
        // On Windows the descendant can inherit additional pipe handles even
        // with its standard streams redirected to null. EOF then needs the kill.
        assert_eq!(
            body["exit_code"],
            if fixture == "fixture_pipe_holder" || cfg!(windows) {
                -1
            } else {
                0
            },
            "{fixture}: {body}"
        );
        assert!(request_dirs(&parent)?.is_empty());
        let heartbeat = fs::read(parent.path().join("heartbeat"))?;
        sleep(Duration::from_millis(150)).await;
        assert_eq!(
            heartbeat,
            fs::read(parent.path().join("heartbeat"))?,
            "Descendant survived cleanup"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_grace_and_force_kill_fallback() -> TestResult {
    for fixture in ["fixture_cooperative", "fixture_ignore_term"] {
        let parent = tempfile::tempdir()?;
        let started = Instant::now();
        let (status, body) = run(state(&parent, 1)?, fixture, 1).await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["exit_code"], -1);
        if fixture == "fixture_cooperative" {
            assert!(body["stdout"]
                .as_str()
                .is_some_and(|out| out.contains("cooperative termination")));
        } else {
            assert!(started.elapsed() >= Duration::from_secs(4));
        }
        assert!(request_dirs(&parent)?.is_empty());
    }
    Ok(())
}
