//! Resident-vs-CLI contract comparison required by design section 9 before the
//! resident backend can become the default: the same local, deterministic
//! fixture arguments are dispatched through both backends and the responses are
//! compared field by field.
//!
//! The approved divergence is recorded in
//! `.trellis/tasks/11-03-resident-worker-runtime-upgrade/review/owner-decisions.md`
//! (round-1 review finding 2, owner decision option (2)). These tests assert it;
//! neither backend's behaviour is changed here.
//!
//! - success: an identical `RunResponse` on both backends
//! - timeout: CLI `200` with `exit_code: -1`; resident `500` with
//!   `WORKER_TIMEOUT` and the worker retired
//! - business error: an unstructured yt-dlp failure keeps an identical
//!   `RunResponse`; a *structured* worker error has no CLI analogue and
//!   surfaces as resident `500` plus the worker's verbatim code
//!
//! Both backends run the same fixture script, `tests/fixtures/contract_echo.py`,
//! with the same argument list. The CLI backend spawns it directly; the resident
//! backend reaches it through `tests/fixtures/contract_runtime/yt_dlp`, which
//! spawns the same script with the same arguments. That single source of truth is
//! what makes the comparison meaningful.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use serde_json::{json, Value};
use std::{fs, path::Path, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::time::{sleep, timeout};
use ytdlp_http_wrapper::{
    backend::Backend,
    executor,
    models::RunRequest,
    resident_worker::ResidentWorkerConfig,
    resident_worker_manager::{ResidentWorkerManager, ResidentWorkerManagerStatus},
    routes::AppState,
};

/// Expected bytes for the `success` fixture mode. Both backends produce exactly
/// this because both run `tests/fixtures/contract_echo.py`.
const SUCCESS_STDOUT: &str = "contract stdout\n";
const SUCCESS_STDERR: &str = "contract stderr\n";
/// Expected stderr for the `fail` fixture mode (exit code 1).
const FAIL_STDERR: &str = "contract business failure\n";

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type Dispatched = Result<(StatusCode, Value), Box<dyn std::error::Error + Send + Sync>>;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// The fixture both backends execute, resolved to an absolute path so the
/// argument lists are literally identical.
fn contract_script() -> String {
    fixture_dir()
        .join("contract_echo.py")
        .to_string_lossy()
        .into_owned()
}

fn fixture_args(mode: &str) -> Vec<String> {
    vec![contract_script(), mode.to_owned()]
}

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

/// CLI backend state. `python` is the supervised executable and the shared
/// fixture script is the first argument, so the CLI backend spawns exactly the
/// same process the resident worker's `yt_dlp` stand-in spawns.
fn cli_state(parent: &TempDir, python: &Path) -> Arc<AppState> {
    Arc::new(AppState::new(
        python.to_path_buf(),
        1,
        Some(parent.path().to_path_buf()),
    ))
}

/// Resident backend state running the real worker
/// (`worker/ytdlp_worker.py`) against the deterministic `yt_dlp` stand-in in
/// `tests/fixtures/contract_runtime`, which shells out to the same fixture
/// script with the same arguments.
fn resident_state(parent: &TempDir, python: PathBuf) -> Arc<AppState> {
    let manager = Arc::new(ResidentWorkerManager::for_config(ResidentWorkerConfig {
        python_path: python,
        worker_path: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("worker")
            .join("ytdlp_worker.py"),
        python_path_entries: vec![fixture_dir().join("contract_runtime")],
        generation_id: "generation-contract".into(),
        worker_id: "worker-contract".into(),
        expected_yt_dlp_version: "fixture-version".into(),
        temp_dir: Some(parent.path().to_path_buf()),
    }));
    Arc::new(
        AppState::new(
            PathBuf::from("non-existent-yt-dlp"),
            1,
            Some(parent.path().to_path_buf()),
        )
        .with_backend(Backend::Resident(Some(manager))),
    )
}

/// Resident backend state driven by the scripted runtime fixture, used for the
/// resident-only half of the business-error comparison (structured error frames
/// exist only in the worker protocol, never in the CLI backend).
fn scripted_resident_state(parent: &TempDir, python: PathBuf, worker_id: &str) -> Arc<AppState> {
    let manager = Arc::new(ResidentWorkerManager::for_config(ResidentWorkerConfig {
        python_path: python,
        worker_path: fixture_dir().join("resident_worker_runtime.py"),
        python_path_entries: vec![fixture_dir()],
        generation_id: "generation-contract".into(),
        worker_id: worker_id.into(),
        expected_yt_dlp_version: "fixture-version".into(),
        temp_dir: Some(parent.path().to_path_buf()),
    }));
    Arc::new(
        AppState::new(
            PathBuf::from("non-existent-yt-dlp"),
            1,
            Some(parent.path().to_path_buf()),
        )
        .with_backend(Backend::Resident(Some(manager))),
    )
}

fn resident_manager(state: &Arc<AppState>) -> Arc<ResidentWorkerManager> {
    match &state.backend {
        Backend::Resident(Some(manager)) => manager.clone(),
        _ => panic!("expected a resident backend with a manager"),
    }
}

/// Runs one request through the executor and returns the HTTP status plus the
/// response body, so both backends are compared at the same boundary.
async fn dispatch(state: Arc<AppState>, args: Vec<String>, seconds: u64) -> Dispatched {
    let response = executor::execute(
        RunRequest {
            args,
            timeout_seconds: Some(seconds),
        },
        state,
    )
    .await
    .into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 25 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body)?))
}

/// Every per-request temporary directory both backends own under `parent`:
/// `ytdlp-request-*` for the CLI supervisor and `ytdlp-worker-request-*` for the
/// resident controller.
fn request_dirs(parent: &TempDir) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(parent.path())? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("ytdlp-request-") || name.starts_with("ytdlp-worker-request-") {
            found.push(entry.path());
        }
    }
    Ok(found)
}

/// Waits until a fixture marks itself ready inside its owned request directory,
/// so the timeout comparison does not race process start-up.
async fn wait_for_ready_marker(parent: &TempDir) -> TestResult {
    timeout(Duration::from_secs(20), async {
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

async fn wait_for_status(
    manager: &ResidentWorkerManager,
    expected: ResidentWorkerManagerStatus,
) -> ResidentWorkerManagerStatus {
    loop {
        let status = manager.status().await;
        if status == expected {
            return status;
        }
        sleep(Duration::from_millis(10)).await;
    }
}

/// Design section 9: a successful local fixture request returns the same
/// `RunResponse` on both backends. No field is normalized -- stdout, stderr and
/// exit code are deterministic on both sides by construction.
#[tokio::test]
async fn success_is_an_identical_run_response_on_both_backends() -> TestResult {
    let Some(python) = find_python() else {
        eprintln!("skipping backend contract test because Python is unavailable");
        return Ok(());
    };
    let parent = tempfile::tempdir()?;
    let args = fixture_args("success");

    let cli = cli_state(&parent, &python);
    let (cli_status, cli_body) = dispatch(cli, args.clone(), 45).await?;

    let resident = resident_state(&parent, python);
    resident.backend.start().await?;
    let (resident_status, resident_body) = dispatch(resident.clone(), args, 45).await?;

    assert_eq!(cli_status, StatusCode::OK, "{cli_body}");
    assert_eq!(resident_status, StatusCode::OK, "{resident_body}");
    assert_eq!(
        cli_body, resident_body,
        "a successful fixture request must produce an identical RunResponse"
    );
    assert_eq!(
        cli_body,
        json!({
            "exit_code": 0,
            "stdout": SUCCESS_STDOUT,
            "stderr": SUCCESS_STDERR,
        }),
        "the shared fixture defines the expected bytes"
    );

    // A successful request keeps the resident worker warm and leaves no owned
    // residue on either side.
    assert_eq!(
        resident_manager(&resident).status().await,
        ResidentWorkerManagerStatus::Ready
    );
    assert!(request_dirs(&parent)?.is_empty());

    resident.backend.shutdown().await;
    Ok(())
}

/// Owner-approved divergence (finding 2, option (2)): the same fixture arguments
/// that make the CLI backend return `200` with `exit_code: -1` make the resident
/// backend return `500` with `WORKER_TIMEOUT`. The CLI shape is the documented
/// spec error-matrix row; the resident shape separates infrastructure failure
/// from yt-dlp failure instead of overloading a magic exit code.
#[tokio::test]
async fn timeout_diverges_exactly_as_the_owner_approved() -> TestResult {
    let Some(python) = find_python() else {
        eprintln!("skipping backend contract test because Python is unavailable");
        return Ok(());
    };
    let parent = tempfile::tempdir()?;

    // CLI: the request deadline interrupts the supervised process tree and the
    // captured (empty) output is returned with exit_code -1.
    let cli = cli_state(&parent, &python);
    let cli_dispatch = tokio::spawn(dispatch(cli, fixture_args("hang"), 1));
    wait_for_ready_marker(&parent).await?;
    // Three unwraps: the timeout, the spawned task's join, then the dispatch.
    let (cli_status, cli_body) = timeout(Duration::from_secs(45), cli_dispatch).await???;
    assert_eq!(cli_status, StatusCode::OK, "{cli_body}");
    assert_eq!(
        cli_body,
        json!({ "exit_code": -1, "stdout": "", "stderr": "" }),
        "CLI timeout must keep the documented 200/exit_code:-1 contract"
    );
    timeout(Duration::from_secs(30), async {
        while !request_dirs(&parent)?.is_empty() {
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), std::io::Error>(())
    })
    .await??;

    // Resident: the same deadline retires the worker and reports an
    // infrastructure failure instead.
    let resident = resident_state(&parent, python);
    let manager = resident_manager(&resident);
    resident.backend.start().await?;
    let (resident_status, resident_body) = timeout(
        Duration::from_secs(90),
        dispatch(resident.clone(), fixture_args("hang"), 1),
    )
    .await??;
    assert_eq!(
        resident_status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{resident_body}"
    );
    assert_eq!(resident_body["code"], "WORKER_TIMEOUT");
    assert!(
        resident_body["error"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "resident timeout must carry a diagnostic message: {resident_body}"
    );
    assert_eq!(
        timeout(
            Duration::from_secs(20),
            wait_for_status(&manager, ResidentWorkerManagerStatus::Empty)
        )
        .await?,
        ResidentWorkerManagerStatus::Empty,
        "a timed-out resident worker must be retired, not left for the next request"
    );
    assert!(request_dirs(&parent)?.is_empty());

    resident.backend.shutdown().await;
    Ok(())
}

/// Owner-approved divergence (finding 2, option (2)) for business errors.
///
/// An *unstructured* yt-dlp failure is a process exit code on both backends, so
/// the same fixture arguments still yield an identical `RunResponse`. The
/// divergence is in the resident backend's *structured* worker error channel,
/// which has no CLI analogue: the closest CLI shape for the same arguments is
/// the `200`/`exit_code: 1` asserted in the first half, which is exactly the
/// lossy shape the owner approved replacing with `500` plus the worker's
/// verbatim code on the resident backend.
#[tokio::test]
async fn business_error_shapes_diverge_exactly_as_the_owner_approved() -> TestResult {
    let Some(python) = find_python() else {
        eprintln!("skipping backend contract test because Python is unavailable");
        return Ok(());
    };
    let parent = tempfile::tempdir()?;
    let args = fixture_args("fail");

    let cli = cli_state(&parent, &python);
    let (cli_status, cli_body) = dispatch(cli, args.clone(), 45).await?;
    assert_eq!(cli_status, StatusCode::OK, "{cli_body}");
    assert_eq!(
        cli_body,
        json!({ "exit_code": 1, "stdout": "", "stderr": FAIL_STDERR }),
        "CLI business failures keep the RunResponse contract"
    );

    let resident = resident_state(&parent, python.clone());
    resident.backend.start().await?;
    let (resident_status, resident_body) = dispatch(resident.clone(), args, 45).await?;
    assert_eq!(resident_status, StatusCode::OK, "{resident_body}");
    assert_eq!(
        cli_body, resident_body,
        "an unstructured yt-dlp failure stays comparable across both backends"
    );
    assert_eq!(
        resident_manager(&resident).status().await,
        ResidentWorkerManagerStatus::Ready,
        "a business error must not retire the worker"
    );
    resident.backend.shutdown().await;

    // Resident-only half: a structured worker error frame is surfaced verbatim
    // as an infrastructure failure, with no CLI-side counterpart.
    let scripted = scripted_resident_state(&parent, python, "business-error");
    let scripted_manager = resident_manager(&scripted);
    scripted.backend.start().await?;
    let (status, body) = dispatch(scripted.clone(), fixture_args("fail"), 45).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(
        body["code"], "FIXTURE_FAILURE",
        "the worker's structured business code must pass through verbatim"
    );
    assert_eq!(
        scripted_manager.status().await,
        ResidentWorkerManagerStatus::Ready
    );
    assert!(request_dirs(&parent)?.is_empty());
    scripted.backend.shutdown().await;
    Ok(())
}
