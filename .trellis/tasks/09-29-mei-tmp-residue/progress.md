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
independent external audit. The initial implementation did not encounter audit
timeouts; the later CI investigation did and retried them as recorded below.

Windows process-wrap can retain an uncancellable blocking completion-port waiter;
the timeout path retains its handle, closes admission and bounds executable runtime
teardown. Unix waits for group disappearance and requires a working init reaper.

## Acceptance gaps / deployment boundary

- No usable local Docker daemon or configured WSL Linux runtime. Linux-only source
  and tests have not been compiled/executed locally. The Windows Linux-smoke target
  runs zero tests and is not Linux evidence.
- PR #1 now exists at commit 7f9f83b9419e4bc399302f5e5186ab942dc09312.
  CI run 36532174155 failed before compilation/tests: all three jobs could not
  fetch the pinned boul2gom/lofty-rs dependency. Linux SIGTERM/fallback, real
  PyInstaller extraction reclamation, image build, Compose runtime probe and
  actual service signal shutdown still require Linux execution.
- The container probe validates the mount, not workload memory sufficiency or
  actual HTTP shutdown. Representative concurrency/memory testing remains a rollout
  check. Filesystem stalls and HTTP response delivery are not hard bounded.
- Intentionally detached process sessions, abrupt bare-metal wrapper death and
  temporary paths overridden by programs/flags remain outside owned-dir cleanup.
- Existing unrelated untracked files are preserved; commit scope is explicit in
  commit-plan.md. Task archive must wait for remaining acceptance and workflow steps.

## PR #1 failure investigation (2026-09-29)

- Authoritative failed run: https://github.com/killbus/ytdlp-http-wrapper/actions/runs/36532174155
- Both Rust jobs failed during dependency fetching in Clippy; Docker failed in
  cargo-chef cook for the same Git URL/revision. No lifecycle assertion failed.
- GitHub API returned 404 for boul2gom/lofty-rs. The local Cargo Git cache
  explains why earlier locked/offline checks passed. Sandbox gh authentication
  failed, but the same read outside the sandbox successfully retrieved CI logs.
- The cached fork is lofty 0.23.3 plus a paste-to-pastey change and fork CI.
  yt-dlp 2.7.2 documents this as mitigation for RUSTSEC-2024-0436. The cached
  registry index marks all lofty 0.23.x releases yanked; removing the patch
  without checking compatibility and preserving its purpose is insufficient.
- Approval review recovered on retry. GitHub API found the identical commit
  d2e41640481a48a95303d95939ba831767afcec8 through Serial-ATA/lofty-rs. A fresh
  empty repository successfully fetched that exact revision from public upstream.
- The fix changes only the Git URL in Cargo.toml and the three corresponding
  Cargo.lock source entries. All dependency versions and the pinned revision
  remain unchanged, including the pastey mitigation. No vendored files or Docker
  source-replacement configuration are needed.
- `cargo fetch --locked` succeeded with a new task-local CARGO_HOME whose Git
  cache did not exist before the command. Only crates.io packages were supplied
  from a temporary local snapshot; the patched Git dependency was fetched over
  the network from public upstream by Cargo itself.
- Structural lockfile comparison verified that only the three source URLs changed
  (lofty, lofty_attr, ogg_pager); all other package data is identical.
- Local fmt, all-target Clippy, 10 Windows lifecycle tests and the GET query test
  passed against the newly fetched source. Release build returned success when
  its session was checked for cancellation after the user requested CI-only heavy
  work. No further local heavy builds/tests will be started.
- User instruction: "走 ci 构建，不在本地进行重任务。" Continue by updating the
  existing PR branch and validating Windows/Linux/Docker in CI.
- Automatic approval review intermittently failed with rate-limit/channel
  errors. These commands were not executed; retry review instead of bypassing it.
