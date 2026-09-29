# Process Lifecycle

- Each admitted request owns a randomly named `tempfile::TempDir`, a process group (Unix) or JobObject (Windows), and a semaphore permit. Set only the child's TMPDIR/TMP/TEMP; never change the wrapper's environment globally.
- Hold resources in a tracked supervisor, independently of the HTTP handler. A closed reply channel requests cancellation; do not assume every network disconnect cancels the handler.
- Unix termination uses process-wrap's safe `ChildWrapper::signal` API, first SIGTERM then SIGKILL after a bounded grace period. Windows uses immediate JobObject termination.
- process-wrap 9.1.0 `KillOnDrop` only sets Tokio's direct-child kill-on-drop on Unix. A separate guard must invoke group termination if supervisor cleanup is interrupted.
- Retain at most 10 MiB per pipe while continuing to drain excess output. Include pipe completion in deadlines; a descendant can keep a pipe open after its parent exits.
- Terminate remaining group/job members before removing the owned temporary directory and returning the permit. Never sweep unrelated `_MEI*` directories by age.
- SIGKILL/default fatal signals do not run Rust destructors. Use container tmpfs for abrupt-death containment; it does not perform ongoing garbage collection and consumes memory.
- Shutdown explicitly cancels queued and active work, stops admission, and awaits tracked cleanup. The HTTP timeout response remains `200` with `exit_code: -1`; grace/cleanup can add latency.

## 1. Scope / Trigger
Root wrapper subprocess, cancellation, temp storage, admission and deployment changes.

## 2. Signatures
- AppState::new(binary_path: PathBuf, max_concurrent: usize, temp_dir: Option<PathBuf>)
- AppState::begin_shutdown(); async AppState::wait_for_cleanup()
- execute(RunRequest, Arc<AppState>) -> impl IntoResponse
- CLI --temp-dir / YTDLP_TEMP_DIR: existing parent directory, canonicalized at startup.

## 3. Contracts
GET/POST /run continues to accept args and optional timeout_seconds. Timeout is
clamped to 1..300 (default 30), excludes queueing and includes pipe completion.
HTTP 200 returns exit_code/stdout/stderr; timeout/cancellation uses exit_code -1.
Only the child's TMPDIR/TMP/TEMP change. Retain 10 MiB of raw bytes per stream,
discard excess while draining, and preserve the captured prefix across grace.
SIGTERM grace is 3 seconds; post-kill process/pipe cleanup deadline is 2 seconds.
Filesystem cleanup is explicit but not hard bounded. Total supervisor logs
include cleanup time. Hold the permit until that cleanup returns.

## 4. Validation & Error Matrix
| Condition | HTTP / code | Resource behavior |
|---|---|---|
| Denied argument | 422 ARG_REJECTED | No permit/process/temp directory |
| Admission closed or shutdown before spawn | 503 SHUTTING_DOWN | No process |
| Temp creation/resolution fails | 500 TEMP_FAILURE | No process; permit returned |
| Spawn fails | 500 SPAWN_FAILURE | Owned directory explicitly removed |
| Output collection fails | 500 COLLECT_FAILURE | Terminate and clean before reporting |
| Process/pipe cleanup uncertain | 500 CLEANUP_FAILURE | Preserve temp, close admission |
| Directory removal fails | 500 TEMP_CLEANUP_FAILURE | Log path, close admission |
| Supervisor panic | 500 SUPERVISOR_FAILURE | Guard requests kill, preserves temp, closes admission |
| Timeout, cleanup succeeds | 200 exit_code=-1 | Captured output returned; no owned residue |

## 5. Good / Base / Bad Cases
Good: concurrent requests use different directories; cancelling one leaves the
other's files intact. Base: normal exit and spawn error both remove owned temp.
Bad: Windows file sharing prevents removal; fail closed and retain the error/path.
A fatal wrapper signal cannot run cleanup: tmpfs is the abrupt-death fallback.

## 6. Tests Required
Use tests/process_lifecycle.rs native subprocess fixtures for lifecycle changes.
Assert response, directory reclamation, permit lifetime and stopped descendant
heartbeats. Windows locked-file regression proves cleanup errors close admission.
Unix cases must prove SIGTERM cooperation, ignored SIGTERM fallback, and cancellation
while grace is active. tests/linux_ytdlp_smoke.rs observes real _MEI extraction
before asserting three timeout cycles reclaim it. CI runs both OSes; never infer
Linux behavior from a Windows pass.

## 7. Wrong vs Correct
Wrong: handler owns TempDir, Drop triggers deletion immediately; read.take(limit)
stops draining; assume KillOnDrop kills the whole Unix group or Axum handles SIGTERM.
Correct: tracked supervisor owns process/temp/permit, explicit signal handling and
bounded termination precede deletion, and excess output is drained to EOF.

Dependency caveat: do not call process-wrap group/job wait during cancellable
execution. Unix polls group disappearance using safe killpg(None). Windows waits
on the JobObject only after successful termination. If that wait times out,
preserve the wrapper/handle because process-wrap may have a raw-handle blocking
waiter. Close admission; the executable bounds runtime blocking-thread teardown.
A functioning Unix init reaper is required, including for zombie group members.
