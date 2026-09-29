# Planning Review: `_MEI*` Residue

- Date: 2026-09-29
- Scope: latest task `09-29-mei-tmp-residue`, current wrapper source and deployment documentation, with the July process-wrap task as background.
- Method: static review and Trellis context validation. No production access, Linux runtime reproduction, or new build/test run.
- Verdict: the investigation identifies a credible mechanism, but the proposed acceptance criteria and lifecycle assumptions need revision before implementation.

## Current state

The task is `planning`. PRD and research exist; `design.md` and `implement.md` do not. Both context manifests contain two real entries and pass `task.py validate`. That command checks JSONL syntax and referenced paths, not technical correctness or implementation readiness. The seed comment rows are valid.

The executor still immediately kills on timeout (`src/executor.rs:225-229`). No task implementation is present in the working-tree diff. No Compose/deployment file was found, and the README Docker examples do not configure tmpfs. The new task directory was already untracked at review start.

## Findings

### 1. [P1] tmpfs does not provide ongoing residue cleanup

References: `prd.md:20-22`, `research/mei-tmp-residue-root-cause.md:64-73`.

tmpfs prevents new files from occupying the container writable layer and bounds filesystem capacity. It does not delete abandoned directories while the container continues running. The preserved hard-kill/cancellation paths can therefore fill it over repeated requests, even with low concurrency. At the reported 23 MB per directory, a 512 MB mount holds only about 22 abandoned extractions before accounting for live processes and other temporary files. Limiting concurrency does not limit historical residue.

tmpfs pages also consume memory and are normally charged to the container's memory cgroup. Memory pressure/OOM can occur before the filesystem reaches its configured capacity; ENOSPC is not the only failure mode.

Required planning change: describe tmpfs as containment, specify memory and temporary-storage budgets together, and define residue recovery or reclamation. Include a repeated hard-kill test without container restarts. Identify the actual deployment configuration to change.

### 2. [P1] unconditional zero-residue acceptance conflicts with SIGKILL fallback

References: `prd.md:23-29`, `prd.md:45-49`.

If the bootloader ignores/cannot finish SIGTERM cleanup within the grace period, the planned SIGKILL still prevents its cleanup code from completing. Sending SIGTERM first can reduce residue but cannot guarantee that every timed-out request leaves no `_MEI*` directory. Mounting tmpfs does not make such residue immediately disappear.

Required planning change: distinguish successful graceful cleanup from forced termination. Require no residue for a verified cooperative-exit case; require bounded termination and a defined residue policy for forced exits. If zero residue after every timeout is mandatory, design explicit ownership and post-termination cleanup of per-request temporary storage, including cancellation handling. Validate the deployed yt-dlp/PyInstaller artifact on Linux.

### 3. [P2] container termination is incorrectly attributed to Rust Drop

References: `research/mei-tmp-residue-root-cause.md:29-37`, `src/main.rs:126`, `src/executor.rs:145-148`, `Dockerfile:78`.

The timeout branch explicitly calls `start_kill()`. Dropping a live child handle can invoke KillOnDrop. However, those are different from the wrapper itself being terminated by the OS: default SIGTERM termination and SIGKILL do not unwind Rust destructors. `ProcessGroup::leader()` creates a process group; it does not install parent-death or shutdown handling.

The wrapper calls `axum::serve(listener, app).await` without a registered shutdown signal or `with_graceful_shutdown`. Therefore the existing architecture document's automatic Axum shutdown narrative (`.trellis/spec/yt-dlp/backend/service-architecture.md:222-241`) is not implemented. Actual container teardown is governed by dumb-init and the container runtime and must be verified separately.

Required planning change: separate request timeout, handler cancellation, graceful service shutdown, and abrupt service/container death. Explicitly state which paths this task improves. Do not assume client disconnection always cancels a handler without checking the HTTP behavior.

### 4. [P2] matching extraction timestamps do not establish a simultaneous failure

Reference: `research/mei-tmp-residue-root-cause.md:41-47`.

The production summary lists directory/file timestamps. These are not process death timestamps; extracted files may also carry packaged timestamps. This evidence alone cannot establish that all processes died together or rule out gradual accumulation. The later caveat about missing logs is appropriate, but contradicts the categorical earlier finding.

Required planning change: keep a burst incident as a hypothesis until correlated with request timeout logs, container restart/OOM events, and identification of the timestamp fields inspected.

### 5. [P2] task context routes to the wrong package and lacks useful quality guidance

References: `task.json:9`, `implement.jsonl`, `check.jsonl`, `.trellis/config.yaml`, `.trellis/spec/yt-dlp/backend/index.md`.

The task package is `mpd`, while the intended change is in the root Rust wrapper. Both manifests point to the yt-dlp backend index, whose linked quality/error/logging guides are still templates. The configured `yt-dlp` package itself points to `third_party/yt-dlp`, so blindly switching the package name would also leave the root-wrapper scope ambiguous.

Required planning change: select or define the correct wrapper scope, load substantive project constraints directly, and complete design/implementation documents before starting this lifecycle change. The existing service architecture also names `MAX_CONCURRENT_DOWNLOADS`, whereas the actual CLI uses `MAX_CONCURRENT_PROCESSES` (`src/main.rs:43-48`); capacity guidance must use the implemented setting.

## Suggested acceptance coverage

| Scenario | Required evidence |
| --- | --- |
| Normal completion | Existing response and output-cap behavior remains intact. |
| Cooperative timeout | Deployed Linux artifact receives SIGTERM, exits within grace, and removes its extraction directory; response retains timeout semantics. |
| SIGTERM ignored | SIGKILL follows bounded grace; all relevant descendants terminate; pipe drains and semaphore release complete; residue follows the declared policy. |
| Cancellation during grace | Safety-net cleanup still runs; no live child or permit is leaked. |
| Repeated forced exits | Space and memory remain within the documented operating budget, with observable and tested recovery behavior. |
| Windows timeout | Immediate JobObject termination remains unchanged; verify on Windows. |
| Container stop/restart | Verify actual signal behavior, tmpfs lifecycle, and writable-layer usage independently of request timeout. |

The existing test suite only exercises GET query deserialization and spawn failure, not process lifecycle (`tests/query_deserialization_test.rs`). Build and clippy success alone cannot validate this change. CI also requires formatting and treats clippy warnings as errors (`.github/workflows/ci.yml`).

The design should define total response latency after the execution timeout, signal-error handling, process-group ownership, behavior when descendants keep pipes open, and timing metrics. Currently `elapsed` is captured before kill/wait/drain (`src/executor.rs:223`), so adding a grace period without adjusting metrics would conceal that additional latency.

## Recommended order

1. Correct the research claims and split cooperative versus forced-exit acceptance.
2. Specify deployment containment, capacity, and recovery, including explicit concurrency configuration.
3. Write the Unix termination design and verification plan; confirm the process-wrap 9.1.0 signal API before choosing a new direct dependency.
4. Implement only after planning review, preserving Windows behavior and the existing safety net.

The July process-wrap task is archived and has a corresponding implementation commit (`2afc892`), but its acceptance checkboxes remain empty and the local journal contains no recorded sessions. This is a validation-record gap, not proof that its checks failed. The current task should record concrete runtime evidence when completed.

## Checks performed

- Read task metadata, PRD, research, manifests, workflow, relevant specs, executor/server/Docker source, tests, CI, and recent git history.
- `python -B .trellis/scripts/task.py validate .trellis/tasks/09-29-mei-tmp-residue`: passed; two real entries in each manifest.
- No application source, deployment configuration, task status, or existing task artifacts were changed by this review. This report is the only added artifact.
