# Research: temporary extraction residue (corrected 2026-09-29)

## Reported observation and limits
The production report described dozens of overlay2 /tmp/_MEI* directories,
about 23 MB each, with Python libraries and a Sep 25 18:03 timestamp cluster.
This is consistent with PyInstaller extraction. No production logs, container
inspect output, UID mapping, OOM records or process history were accessed in
this implementation session. Matching timestamps do not establish simultaneous
termination or distinguish timeout, OOM, restart and gradual accumulation.
The host owner label systemd-coredump can be a UID/name mapping effect; its
exact mapping was not verified.

## Verified source facts
- process-wrap 9.1.0 ProcessGroupChild exposes safe ChildWrapper::signal(i32),
  which sends killpg. start_kill sends SIGKILL. Symbolic signals use nix 0.31.
- Unix KillOnDrop configures Tokio's direct-child kill_on_drop; it does not
  independently ensure that the whole process group is killed on Drop.
- A fatal OS signal does not unwind Rust or run its Drop implementations.
  Axum graceful shutdown must be installed explicitly.
- ProcessGroupChild::wait and JobObjectChild::wait can start blocking reapers.
  Ordinary execution waits use the underlying Tokio child instead. Unix group
  completion is polled with killpg(None). Windows job wait starts only after
  successful termination; its handle is preserved on timeout and runtime
  teardown has a bounded blocking-thread wait.
- Official yt-dlp_linux onefile builds use PyInstaller extraction. Graceful
  bootloader cleanup depends on artifact/platform/signal behavior and is not
  an unconditional SIGTERM guarantee. The real-artifact smoke test checks it
  separately from the native fake-process tests.

## Baseline failure paths (before this change)
Timeout explicitly called start_kill. Handler drop could kill only the direct
Unix child, and network disconnect did not necessarily drop the handler.
Container stop sent signals according to its runtime/init setup; it was not
accurate to describe every abnormal exit as process-group SIGKILL. Output
readers stopped at the size cap, and waits after termination were unbounded.
These are plausible contributors; they do not prove the historical cause.

## Implemented controls
Each admitted request owns an exclusive temporary directory and child-only
TMPDIR/TMP/TEMP. A tracked supervisor holds its semaphore permit through cleanup.
Timeout, handler cancellation and shutdown request SIGTERM with 3 seconds of
Unix grace, then SIGKILL; Windows uses JobObject termination. Both pipes retain
bounded prefixes while draining excess. Process/pipe cleanup has a 2-second
post-kill deadline. Failure closes admission and preserves uncertain resources.
No global age-based _MEI sweep is used.

Compose mounts /tmp as rw,exec,nosuid,nodev,size=512m,mode=1777, with explicit
concurrency 4, memory 1 GiB and stop grace 15s. tmpfs is bounded storage, not
ongoing garbage collection. It consumes memory and can produce either ENOSPC
or memory pressure/OOM. Size based on representative workload, not only the
reported 23 MB extraction size. exec permits executable library mappings.

## Operations
Recreate the stopped old container with the new image/configuration while
preserving the downloads volume. Mounting tmpfs over old overlay files hides
them without reclaiming the old writable layer. On bare metal, stop the service,
confirm its processes have exited, then review exact owned paths for cleanup.
Age alone never establishes that an extraction directory is unused.

## Evidence sources
Installed process-wrap 9.1.0 src/tokio/{core,kill_on_drop,process_group,job_object}.rs;
Cargo.lock; src/supervisor.rs; tests/process_lifecycle.rs; tests/linux_ytdlp_smoke.rs;
compose.yaml; docs/process-lifecycle.md. Windows runtime evidence and Linux
verification limitations are recorded in ../progress.md.
