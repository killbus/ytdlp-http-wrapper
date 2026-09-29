# Implementation Progress — 2026-09-29

Local implementation and Windows validation are ready for review. Task remains
in_progress; Linux runtime acceptance and archive are outstanding. The user approved
the local commit scope with "提交" on 2026-09-29.
No deployment, push, or cleanup of production files has occurred.

## Delivered

- Tracked supervisor owns child group/job, request TempDir and semaphore permit.
- Timeout, handler cancellation and shutdown terminate descendants before removing
  owned files; excess output is drained after the 10 MiB per-stream retention cap.
- Cleanup failures report an error and close admission. A Windows locked-file
  regression verifies this behavior. No shared/age-based temporary-file sweeper.
- Child-only temporary environment, validated startup parent configuration, explicit
  SIGTERM/Ctrl-C handling and supervisor drainage at shutdown.
- Compose and Docker examples specify executable 512 MiB tmpfs, concurrency 4,
  1 GiB memory and 15-second stop grace, with operations and migration guidance.
- CI matrix for Windows/Linux, verified official Linux onefile smoke test, and
  a container probe for non-root tmpfs permissions, capacity and mount flags.
- Root wrapper package/spec and corrected incident research. Production root-cause
  attribution remains a hypothesis; dependency behavior was inspected locally.

## Validation evidence

- Windows baseline and implementation compile passed with locked/offline Cargo.
- Final `cargo build --locked --offline --release` passed after all review
  corrections. The resulting binary's help and invalid-config checks passed;
  zero concurrency and a file as temp parent fail before dependency bootstrap.
- Final `cargo test --locked --offline` passed 10 Windows lifecycle cases plus
  the existing GET deserialization test. Six ignored native fixtures were launched
  by those cases. Includes shutdown-between-admission-and-spawn regression.
- Final `cargo clippy --locked --offline --all-targets -- -D warnings` and
  `cargo fmt --check` passed.
- Bash syntax validation passed for both smoke scripts. PyYAML parsed CI/Compose;
  assertions verified OS matrix, tmpfs options, concurrency, memory and stop grace.
- Trellis implement/check JSONLs each validate with 3 real context entries.
- Tracked diff whitespace check passed after normalizing only task-edited files.

## Inline final review

Reviewed the request -> admission -> tracked supervisor -> cleanup -> response
flow, actual process-wrap 9.1.0 APIs, error propagation, temp ownership, concurrent
pipe drains, cancellation and failure paths. Review corrections explicitly clean
up after temp canonicalization failure, validate startup settings before downloads,
distinguish termination reasons in logs, and return HTTP 503 when shutdown wins
after admission but before process spawn. This is an inline Trellis review, not an
independent external audit. No audit-service timeout was encountered in this work.

Windows process-wrap can retain an uncancellable blocking completion-port waiter;
the timeout path retains its handle, closes admission and bounds executable runtime
teardown. Unix waits for group disappearance and requires a working init reaper.

## Acceptance gaps / deployment boundary

- No usable local Docker daemon or configured WSL Linux runtime. Linux-only source
  and tests have not been compiled/executed locally. The Windows Linux-smoke target
  runs zero tests and is not Linux evidence.
- CI changes have not been pushed, so no remote CI result is claimed. Linux
  SIGTERM/fallback, real PyInstaller extraction reclamation, image build, Compose
  runtime probe and actual service signal shutdown still require Linux execution.
- The container probe validates the mount, not workload memory sufficiency or
  actual HTTP shutdown. Representative concurrency/memory testing remains a rollout
  check. Filesystem stalls and HTTP response delivery are not hard bounded.
- Intentionally detached process sessions, abrupt bare-metal wrapper death and
  temporary paths overridden by programs/flags remain outside owned-dir cleanup.
- Existing unrelated untracked files are preserved; commit scope is explicit in
  commit-plan.md. Task archive must wait for remaining acceptance and workflow steps.
