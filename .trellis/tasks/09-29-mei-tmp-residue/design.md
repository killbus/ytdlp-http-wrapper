# Design: Owned temporary storage and supervised termination

## Authorization and scope

The user requested implementation of mature practices after reviewing the planning assessment. This authorizes local code, deployment examples, tests, and documentation. Production deployment and Git commits are separate actions. The root wrapper package is now explicitly mapped in Trellis.

## Resource ownership

`AppState` supplies a shutdown CancellationToken and TaskTracker alongside the existing semaphore and binary path. The HTTP executor validates arguments and acquires a permit before spawning a tracked supervisor. A oneshot reply channel connects the handler to that supervisor. If the receiver is dropped, the supervisor terminates its process; the semaphore permit remains held until cleanup finishes. No supervisor is created while a request is still queued.

The supervisor creates an exclusive, random `ytdlp-request-*` TempDir under the configured temporary parent (OS default otherwise). It sets TMPDIR/TMP/TEMP only on the child. This directs PyInstaller extraction into request-owned storage, without moving downloads or libraries. After process cleanup it explicitly closes the TempDir and reports errors. No global glob or age-based deletion is used. Abrupt wrapper death cannot run this cleanup: the container's `/tmp` tmpfs is the second boundary, removed on container stop. Bare-metal abrupt-death leftovers require operator cleanup while the service is stopped.

## Termination

Normal completion waits for the direct child and concurrent pipe EOF. The execution timeout covers both; buffers retain 10 MiB while excess bytes are drained without retention. Completion, timeout, reply-channel closure, and service cancellation are explicit outcomes.

On Unix timeout/cancellation: send SIGTERM using process-wrap `ChildWrapper::signal`, allow 3 seconds for the child and pipes to finish, then terminate the remaining process group with SIGKILL. Windows skips SIGTERM. A guard retains KillOnDrop/JobObject and explicitly requests group/job kill on unexpected supervisor drop. Direct-child wait avoids process-wrap's group wait spawning an uncancellable blocking reaper. Cleanup wait/drain is bounded (2 seconds); failure closes admission and preserves owned storage instead of claiming successful reclamation. Cleanup failures are logged and surfaced.

Always terminate remaining group/job members before reclaiming storage, including when the direct child exits normally with descendants still alive. Process groups are cooperative containment: intentionally detached sessions need stronger container isolation.

## Shutdown

Register SIGTERM and Ctrl-C explicitly. Cancel the shared token, close semaphore admission, use Axum graceful shutdown, and await tracked supervisors before runtime exit. No reliance on OS signals running Drop. Set container stop grace to 15 seconds, exceeding normal 3-second graceful + 2-second forced cleanup.

## Deployment

Provide Compose and matching Docker examples with `/tmp:rw,exec,nosuid,nodev,size=512m,mode=1777`, explicit concurrency 4, and a documented 1 GiB example memory budget. Capacity includes live extraction, other child temp use, and application/output memory. Values are starting points, not hard upper bounds for arbitrary yt-dlp flags. Restart/recreate reclaims abrupt-death residue; monitoring must cover memory and temp filesystem usage.

## Compatibility and rollback

Keep `/run` JSON and timeout `exit_code: -1`, argument denial, Windows JobObject, and output bounds. Execution timeout excludes semaphore queueing as before. Added cleanup latency is documented and included in logs. New infrastructure errors use the existing ErrorResponse shape. Roll back only this task's code/config changes; never delete shared temp trees.

## Evidence

process-wrap 9.1.0 source confirms public `signal(i32)` on ChildWrapper, killpg in ProcessGroupChild, and direct-child-only KillOnDrop on Unix. tempfile 3.27.0 provides owned random directories; tokio-util 0.7.18 supplies CancellationToken and TaskTracker. These are already in Cargo.lock. Linux execution is unavailable on the current Windows host (no configured WSL or Docker); add Linux CI and a reproducible real-artifact smoke test and report that runtime limit accurately.
