# Supervise yt-dlp termination and reclaim request temporary files

## Goal
Stop continuous accumulation of PyInstaller extraction residue during normal service operation, and bound container storage after abrupt wrapper death. The observed production cause remains a hypothesis; local dependency source confirms force-kill paths can bypass bootloader cleanup.

## Requirements
1. Each admitted request has exclusive temporary storage, reclaimed after its process group/job terminates. Never delete another request's or another application's files.
2. Timeout, handler cancellation and service shutdown retain ownership through termination and cleanup. Unix allows 3 seconds after SIGTERM before SIGKILL; Windows uses immediate JobObject termination.
3. Output retention remains bounded to 10 MiB per stream while draining excess. Pipe completion and force-kill waits have deadlines. Cleanup failure is visible and stops admission rather than reporting successful reclamation.
4. Preserve /run JSON, argument rejection and HTTP 200 / exit_code -1 on timeout. Execution timeout still excludes queueing. Document additional termination latency.
5. Provide executable tmpfs deployment examples with memory/concurrency sizing, explicit shutdown grace, and safe operations guidance. tmpfs bounds residue and resets on stop; it does not periodically clean itself.
6. Keep unsafe_code forbidden and the Windows process-tree containment path.

## Acceptance
- [x] Lifecycle tests exercise normal exit, spawn failure, timeout, cancellation, shutdown, inherited pipes, descendants, repeated residue cleanup, output limits and isolation.
- [ ] Linux tests distinguish cooperative SIGTERM from forced termination; Windows tests use real native subprocesses.
- [ ] fmt, clippy all targets, tests and release build pass on available host; CI covers Linux/Windows, with reproducible real yt-dlp Linux artifact smoke validation. Unexecuted checks are explicitly reported.
- [x] Compose and Docker examples specify /tmp with rw,exec,nosuid,nodev,size=512m,mode=1777 and document sizing/stop grace. Runtime probe is prepared; Linux execution remains pending.
- [x] Task evidence, corrected research and wrapper code-spec describe actual guarantees and limitations.

## Non-goals
Production deployment, historical incident attribution, repackaging yt-dlp, and containment of intentionally detached sessions. Git commits require separate authorization.
