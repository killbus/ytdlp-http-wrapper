#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Deterministic pool acceptance tests for AC1-AC6 of the worker-pool unit
//! (task 11-04-worker-pool-supervision, design.md section 10). The
//! single-worker regression surface (AC6) is covered by the existing
//! `resident_backend` / `resident_worker_manager` / `resident_worker`
//! suites running green with the default capacity of 1; this file adds the
//! dedicated multi-slot assertions.
//!
//! The fixture's default scenario reports its `YTDLP_WORKER_ID` in-band on
//! every result (the result frame's `stdout` field) and supports
//! `sleep:<seconds>` / `crash:eof` request args, so concurrency, saturation,
//! and retirement tests pick their timing margins through request args
//! without new worker-id scenarios.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use ytdlp_http_wrapper::{
    backend::Backend,
    resident_worker::{ResidentWorkerConfig, ResidentWorkerError},
    resident_worker_manager::ResidentWorkerManager,
    resident_worker_pool::{SlotHealth, SlotState, WORKER_POOL_SATURATED},
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

/// All pool scenarios run against the scripted runtime fixture.
fn base_config(python: PathBuf) -> ResidentWorkerConfig {
    ResidentWorkerConfig {
        python_path: python,
        worker_path: fixture_path().join("resident_worker_runtime.py"),
        python_path_entries: vec![fixture_path()],
        generation_id: "generation-fixture".into(),
        worker_id: "worker-1".into(),
        expected_yt_dlp_version: "fixture-version".into(),
        temp_dir: None,
    }
}

async fn pool_with(python: PathBuf, capacity: usize) -> Arc<ResidentWorkerManager> {
    let manager = Arc::new(ResidentWorkerManager::for_pool_size(
        base_config(python),
        capacity,
    ));
    manager.ensure_started().await.unwrap();
    manager
}

/// The fixture's default-scenario result stdout is
/// `"<worker_id>:<arg>:<arg>...\n"` — the in-band worker identity
/// (design.md section 10: the worker id must never travel as plain-text
/// stdout, which is the binary protocol channel).
fn served_by(stdout: &str, arg: &str) -> String {
    let suffix = format!(":{arg}\n");
    assert!(
        stdout.ends_with(&suffix),
        "unexpected fixture result stdout: {stdout:?}"
    );
    stdout
        .strip_suffix(&suffix)
        .expect("worker id prefix")
        .to_owned()
}

/// Bounded, absolute-deadline dispatch (the production path shape): the
/// deadline is anchored at call time, so queue time counts against it.
/// Returns the in-band serving worker id plus the wall time.
async fn dispatch_at(
    manager: &Arc<ResidentWorkerManager>,
    request_id: &str,
    arg: &str,
    deadline: Duration,
) -> (String, Duration) {
    let started = std::time::Instant::now();
    let response = manager
        .pool()
        .dispatch(
            request_id,
            vec![arg.to_owned()],
            tokio::time::Instant::now() + deadline,
        )
        .await
        .unwrap();
    (served_by(&response.stdout, arg), started.elapsed())
}

/// Runs a boot probe with a real timeout: the probe must exit on its own
/// (a validation failure exits before the server binds), so a hang means the
/// process reached admission, which is exactly the regression this guards.
fn run_boot_check(mut command: Command) -> Output {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(command.output());
    });
    receiver
        .recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|_| {
            panic!("boot probe did not exit within 30s; boot validation must fail before admission")
        })
        .unwrap()
}

#[tokio::test]
async fn pool_size_zero_or_non_numeric_fails_boot_validation_before_admission() {
    let Some(python) = find_python() else {
        eprintln!("skipping pool size boot validation test because Python is unavailable");
        return;
    };
    let Some(exe) = std::env::var_os("CARGO_BIN_EXE_ytdlp-http-wrapper") else {
        eprintln!("skipping pool size boot validation test because the binary is unavailable");
        return;
    };
    let exe = PathBuf::from(exe);
    let fixture = fixture_path().join("resident_worker_runtime.py");
    let common_args: &[&str] = &[
        "--host",
        "127.0.0.1",
        "--port",
        "1",
        "--backend",
        "resident",
        "--worker-python",
        python.to_str().unwrap(),
        "--worker-path",
        fixture.to_str().unwrap(),
        "--worker-yt-dlp-version",
        "fixture-version",
    ];

    // AC1 (flag, value 0): the boot gate rejects a below-1 pool size with an
    // actionable error before any request is admitted (the fixture paths keep
    // the resident configuration otherwise valid, so the pool size is the
    // only failure).
    let mut probe = Command::new(&exe);
    probe
        .args(common_args)
        .args(["--worker-pool-size", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_boot_check(probe);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("worker-pool-size must be at least 1"),
        "the boot error must name the offending configuration: {stderr:?}"
    );
    assert!(
        !stderr.contains("HTTP server started"),
        "validation must fail before the server admits requests: {stderr:?}"
    );

    // AC1 (env WORKER_POOL_SIZE, value 0): the same gate via the env variable.
    let mut probe = Command::new(&exe);
    probe
        .args(common_args)
        .env("WORKER_POOL_SIZE", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_boot_check(probe);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("worker-pool-size must be at least 1"),
        "the env variable must reach the same boot gate: {stderr:?}"
    );

    // AC1 (non-numeric): rejected at CLI parse time, before serve() runs.
    let mut probe = Command::new(&exe);
    probe
        .args(common_args)
        .args(["--worker-pool-size", "abc"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_boot_check(probe);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid value"),
        "non-numeric pool sizes must be rejected: {stderr:?}"
    );

    // AC1 (default): an unconfigured resident backend is a pool of capacity 1
    // that preserves the base config's single-worker identity.
    let backend = Backend::from_config(
        "resident",
        python,
        fixture,
        vec![fixture_path()],
        "fixture-version",
        None,
    )
    .unwrap();
    let Backend::Resident(Some(manager)) = &backend else {
        panic!("expected the resident backend");
    };
    assert_eq!(manager.pool().capacity(), 1);
    assert_eq!(manager.worker_id(), "worker-1");
}

#[tokio::test]
async fn pool_size_two_runs_requests_concurrently_with_per_slot_serialization() {
    let Some(python) = find_python() else {
        eprintln!("skipping pool concurrency test because Python is unavailable");
        return;
    };
    let manager = pool_with(python, 2).await;
    let pool = manager.pool();
    assert_eq!(pool.capacity(), 2);

    // AC2: two concurrent requests complete on two DISTINCT worker ids (read
    // via the in-band fixture channel) and overlap across slots. The third
    // request starts while both slots are busy, so it must queue behind a
    // worker instead of running on it concurrently (per-slot
    // serialization): its wall time covers the queue wait plus its own full
    // 1.5s sleep, far above one single sleep.
    let manager_c = manager.clone();
    let (first, second, queued) = tokio::join!(
        dispatch_at(&manager, "request-a", "sleep:1.5", Duration::from_secs(10)),
        dispatch_at(&manager, "request-b", "sleep:1.5", Duration::from_secs(10)),
        async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            dispatch_at(
                &manager_c,
                "request-c",
                "sleep:1.5",
                Duration::from_secs(10),
            )
            .await
        }
    );

    assert_eq!(first.0, "worker-1");
    assert_eq!(second.0, "worker-2");
    // Each holder ran its full 1.5s sleep, and the pair overlapped: each
    // stayed well under the 3.0s two serial slots would take.
    assert!(
        first.1 >= Duration::from_millis(1500),
        "first request did not run its full sleep: {:?}",
        first.1
    );
    assert!(
        second.1 >= Duration::from_millis(1500),
        "second request did not run its full sleep: {:?}",
        second.1
    );
    assert!(
        first.1.max(second.1) < Duration::from_millis(2500),
        "the two concurrent requests must overlap across slots: first {:?}, second {:?}",
        first.1,
        second.1
    );
    // Per-slot serialization: the queued request waited for a busy worker to
    // finish, then ran its own full sleep on top (>= (1.5 - 0.4) + 1.5 = 2.6s
    // in the fast case; assert a 1.9s floor so a few milliseconds of instant
    // skew between the join arms cannot flip it, while it stays far above a
    // single 1.5s sleep).
    assert!(
        matches!(queued.0.as_str(), "worker-1" | "worker-2"),
        "the queued request must run on one of the pool workers: {:?}",
        queued.0
    );
    assert!(
        queued.1 >= Duration::from_millis(1900),
        "the queued request must wait out a busy slot before running its own sleep: {:?}",
        queued.1
    );

    // The contention retired nothing; both slots are idle again.
    let health = pool.health();
    assert_eq!(
        health
            .slots
            .iter()
            .map(|slot| slot.state)
            .collect::<Vec<_>>(),
        vec![SlotState::Idle, SlotState::Idle]
    );
    assert_eq!(health.retires, 0);
    assert_eq!(health.replacements, 0);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn pool_size_two_backpressures_third_request_within_its_deadline() {
    // AC3, deterministic "slot frees within its deadline" branch: the third
    // request's 5s deadline is generous relative to the 1.2s slot contention,
    // so it MUST proceed after a slot frees instead of being rejected.
    let Some(python) = find_python() else {
        eprintln!("skipping pool backpressure test because Python is unavailable");
        return;
    };
    let manager = pool_with(python, 2).await;
    let pool = manager.pool();

    let (first, second, third) = tokio::join!(
        dispatch_at(&manager, "request-1", "sleep:1.2", Duration::from_secs(5)),
        dispatch_at(&manager, "request-2", "sleep:1.2", Duration::from_secs(5)),
        dispatch_at(&manager, "request-3", "sleep:1.2", Duration::from_secs(5)),
    );

    // All three completed: the third was backpressured, not rejected. With
    // two slots and three requests, exactly two distinct workers serve, one
    // of them serving twice (the backpressured request reusing a freed slot).
    let distinct: BTreeSet<String> = [first.0.clone(), second.0.clone(), third.0.clone()]
        .into_iter()
        .collect();
    assert_eq!(
        distinct,
        BTreeSet::from(["worker-1".to_owned(), "worker-2".to_owned()]),
        "three requests across two slots must use exactly the two pool workers"
    );
    // Each request ran its full 1.2s sleep; the backpressured third one also
    // queued first, so it took at least queue + sleep.
    assert!(
        first.1 >= Duration::from_millis(1200),
        "first request did not run its full sleep: {:?}",
        first.1
    );
    assert!(
        second.1 >= Duration::from_millis(1200),
        "second request did not run its full sleep: {:?}",
        second.1
    );
    assert!(
        third.1 >= Duration::from_millis(2000),
        "the third request must queue behind a busy slot before its own sleep: {:?}",
        third.1
    );
    // Two of the three sleeps serialized on one slot, so the whole batch
    // covers at least 2 x 1.2s (sleeps never run short); the floor keeps a
    // 50ms margin for the per-arm instant skew, far above the ~1.2s a fully
    // parallel (non-serialized) batch would take.
    let batch = first.1.max(second.1.max(third.1));
    assert!(
        batch >= Duration::from_millis(2350),
        "with two slots, three 1.2s requests must serialize at least two sleeps: {:?}",
        batch
    );

    let health = pool.health();
    assert_eq!(
        health
            .slots
            .iter()
            .map(|slot| slot.state)
            .collect::<Vec<_>>(),
        vec![SlotState::Idle, SlotState::Idle]
    );
    assert_eq!(health.retires, 0);
    assert_eq!(health.replacements, 0);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn pool_size_two_saturates_third_request_when_deadline_expires_queued() {
    // AC3, deterministic "deadline expires while queued" branch: the third
    // request's 500ms deadline expires long before either 1.5s slot holder
    // releases, so it MUST be rejected with WORKER_POOL_SATURATED (HTTP 503
    // in the executor) without ever running.
    let Some(python) = find_python() else {
        eprintln!("skipping pool saturation test because Python is unavailable");
        return;
    };
    let manager = pool_with(python, 2).await;
    let pool = manager.pool();

    let manager_c = manager.clone();
    let (first, second, (third, third_elapsed)) = tokio::join!(
        dispatch_at(&manager, "request-1", "sleep:1.5", Duration::from_secs(10)),
        dispatch_at(&manager, "request-2", "sleep:1.5", Duration::from_secs(10)),
        async move {
            let started = std::time::Instant::now();
            let result = manager_c
                .pool()
                .dispatch(
                    "request-3",
                    vec!["sleep:1.5".to_owned()],
                    tokio::time::Instant::now() + Duration::from_millis(500),
                )
                .await;
            (result, started.elapsed())
        }
    );

    let saturated: ResidentWorkerError = third
        .err()
        .expect("the third request must be rejected while queued");
    assert_eq!(saturated.code(), WORKER_POOL_SATURATED);
    // The rejection is the bounded wait: it arrives at its 500ms deadline,
    // long before the 1.5s slot holders finish.
    assert!(
        third_elapsed < Duration::from_millis(1400),
        "the saturation rejection must come from the bounded queue wait, not a slow rejection: {:?}",
        third_elapsed
    );
    // Both holders complete on distinct workers with their full sleeps.
    assert_eq!(first.0, "worker-1");
    assert_eq!(second.0, "worker-2");
    assert!(
        first.1 >= Duration::from_millis(1500),
        "first holder did not run its full sleep: {:?}",
        first.1
    );
    assert!(
        second.1 >= Duration::from_millis(1500),
        "second holder did not run its full sleep: {:?}",
        second.1
    );

    // The rejection retires and replaces nothing.
    let health = pool.health();
    assert_eq!(
        health
            .slots
            .iter()
            .map(|slot| slot.state)
            .collect::<Vec<_>>(),
        vec![SlotState::Idle, SlotState::Idle]
    );
    assert_eq!(health.retires, 0);
    assert_eq!(health.replacements, 0);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn pool_replaces_retired_slot_and_reports_per_slot_health() {
    // AC4 + AC5: a crash (EOF on the result) retires exactly the crashed
    // slot; the next dispatch replenishes it lazily in place; health reports
    // per-slot state, reason, and counters throughout the sequence.
    let Some(python) = find_python() else {
        eprintln!("skipping pool replenishment test because Python is unavailable");
        return;
    };
    let manager = pool_with(python, 2).await;
    let pool = manager.pool();

    // Both slots are live from boot. Crash the first-available slot (slot 0 /
    // worker-1 — selection is deterministic, slot index order) with the in-band
    // EOF abort: the result frame never arrives.
    let crashed = manager
        .pool()
        .dispatch(
            "crash-1",
            vec!["crash:eof".to_owned()],
            tokio::time::Instant::now() + Duration::from_secs(10),
        )
        .await
        .err()
        .expect("the crashed worker must fail the request");
    assert_eq!(crashed.code(), "WORKER_PROTOCOL_FAILURE");

    // AC5 (retired state): slot 0 is Retired with the protocol-failure
    // reason, slot 1 is still the warm worker-2, retires == 1, and no
    // replacement has happened yet.
    let health = pool.health();
    assert_eq!(health.capacity, 2);
    assert_eq!(
        health.slots,
        vec![
            SlotHealth {
                worker_id: "worker-1".to_owned(),
                state: SlotState::Retired,
                retired_reason: Some("WORKER_PROTOCOL_FAILURE"),
            },
            SlotHealth {
                worker_id: "worker-2".to_owned(),
                state: SlotState::Idle,
                retired_reason: None,
            },
        ]
    );
    assert_eq!(health.retires, 1);
    assert_eq!(health.replacements, 0);

    // AC4: the next dispatch replenishes the retired slot in place (lazy
    // replacement before the execution deadline is re-measured) and the
    // replacement reuses the slot's stable worker id.
    let (served, _) = dispatch_at(
        &manager,
        "after-crash",
        "post-crash",
        Duration::from_secs(10),
    )
    .await;
    assert_eq!(served, "worker-1");

    // AC5 (replenished state): the reason is cleared, the slot is Idle,
    // replacements == 1 while retires stays 1.
    let health = pool.health();
    assert_eq!(
        health.slots,
        vec![
            SlotHealth {
                worker_id: "worker-1".to_owned(),
                state: SlotState::Idle,
                retired_reason: None,
            },
            SlotHealth {
                worker_id: "worker-2".to_owned(),
                state: SlotState::Idle,
                retired_reason: None,
            },
        ]
    );
    assert_eq!(health.retires, 1);
    assert_eq!(health.replacements, 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn default_pool_size_one_preserves_single_worker_identity() {
    // AC6 (dedicated minimal assertion): the default capacity is 1, the base
    // worker id is preserved verbatim (the single-worker compatibility
    // surface), and the one slot serves under that id. The broader
    // single-worker regression baseline (status semantics, error codes, lazy
    // replacement, existing suites) stays covered by the unchanged tests.
    let Some(python) = find_python() else {
        eprintln!("skipping pool default capacity test because Python is unavailable");
        return;
    };
    let manager = pool_with(python, 1).await;
    let pool = manager.pool();

    assert_eq!(pool.capacity(), 1);
    assert_eq!(manager.worker_id(), "worker-1");
    let (served, _) = dispatch_at(&manager, "single-1", "solo", Duration::from_secs(10)).await;
    assert_eq!(served, "worker-1");

    let health = pool.health();
    assert_eq!(health.capacity, 1);
    assert_eq!(
        health.slots,
        vec![SlotHealth {
            worker_id: "worker-1".to_owned(),
            state: SlotState::Idle,
            retired_reason: None,
        }]
    );
    assert_eq!(health.retires, 0);
    assert_eq!(health.replacements, 0);
    manager.shutdown().await.unwrap();
}
