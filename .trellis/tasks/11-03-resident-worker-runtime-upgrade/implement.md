# Implementation plan

## Preconditions

- [ ] Review and approve `prd.md`, `design.md`, and this plan.
- [ ] Keep the task in `planning` until the review gate passes.
- [ ] Create a temporary feature branch only after the task is started.
- [ ] Snapshot `git status --short` and exclude all unrelated dirty or untracked
      paths from commits.
- [ ] If upstream research is needed, use a depth-1 clone outside committed
      source paths and record only independently written findings.

## Phase 1: Freeze contracts and establish measurement

- [ ] Add deterministic fixtures for the current `/run` request, response, error,
      timeout, output-cap, cancellation, and shutdown behavior.
- [ ] Introduce request correlation IDs and phase timing for queue, acquisition,
      execution, first response, and cleanup without changing the API.
- [ ] Capture a repeatable CLI-backend baseline for cold and warm requests.
- [ ] Confirm Linux and Windows lifecycle expectations remain represented.

Validation:

```text
cargo fmt --check
focused contract and lifecycle tests
source review of public API shapes and 10 MiB caps
```

Rollback point: instrumentation and fixtures can be reverted without changing
runtime selection.

## Phase 2: Define and test the worker protocol

- [x] Add shared Rust protocol types and bounded length-prefixed framing.
- [x] Add the Python worker entry point with handshake, one-request dispatch,
      structured result/error frames, stderr diagnostics, and fresh
      `YoutubeDL` creation.
- [x] Test partial frames, oversized frames, malformed payloads, request-ID
      mismatches, version mismatches, EOF, and stderr behavior.
- [x] Ensure secrets are never emitted through tracing or diagnostics.

Validation:

```text
cargo fmt --check
focused protocol tests
Python worker fixture tests
```

Rollback point: protocol code remains unreachable from production routing.

## Phase 3: Add resident worker supervision

- [x] Add the backend abstraction while preserving `RunResponse`.
- [x] Implement a single-worker resident backend behind an explicit non-default
      configuration.
- [x] Extend process-group/JobObject supervision to resident workers.
- [x] Implement exclusive leases, timeout/cancel retirement, crash handling,
      replacement, and shutdown.
- [ ] Extend to a bounded worker pool with backpressure and observable health.
- [x] Preserve request-specific temporary-directory ownership and output caps.

Validation:

```text
focused worker lifecycle and pool tests
Linux process-group lifecycle tests in CI
Windows JobObject lifecycle tests in CI
resident versus CLI deterministic contract comparison
```

Rollback point: select the CLI backend before dispatch; no silent execution
fallback is introduced.

## Phase 4: Implement immutable runtime management

- [ ] Define runtime and generation manifests with strict decoding.
- [ ] Implement supported platform/architecture resolution.
- [ ] Implement bounded download, retry, integrity verification, executable
      permissions, staging, fsync where supported, and atomic publication.
- [ ] Reuse only installations whose manifests and integrity records validate.
- [ ] Add safe cleanup limited to owned, manifest-validated staging and retained
      runtime paths.
- [ ] Make both resident and CLI backends consume project-owned runtime paths.

Validation:

```text
focused manifest and installer tests
interrupted-download and interrupted-publication tests
wrong checksum and wrong platform tests
offline reuse of a previously verified installation
```

Rollback point: a failed candidate never changes active runtime routing.

## Phase 5: Remove the Rust crate without debt

- [ ] Remove `LibraryInstaller`, `install_with_retry`, and the Rust `yt-dlp`
      dependency.
- [ ] Remove the `lofty` patch after confirming no independent direct consumer.
- [ ] Regenerate `Cargo.lock` and verify `yt-dlp`, `lofty`, and other
      transitive-only entries are absent.
- [ ] Update dependency-source documentation to describe project-owned runtime
      acquisition.
- [ ] Ensure no vendored or cloned upstream source enters the repository.

Validation:

```text
cargo metadata --locked
focused Cargo.toml and Cargo.lock searches
cargo fmt --check
full locked CI matrix
```

Rollback point: revert the coherent dependency-removal commit only if the
project-owned installer cannot pass its gates. Do not retain both installers as
an indefinite compatibility layer.

## Phase 6: Add generation upgrades

- [ ] Implement candidate generation creation and pool startup.
- [ ] Validate handshake, local fixtures, representative extractor smoke tests,
      repeated-request stability, isolation, and RSS.
- [ ] Add an explicit owner promotion gate.
- [ ] Implement atomic routing of new requests to the promoted generation.
- [ ] Drain in-flight leases on the prior generation.
- [ ] Retain active and previous stable generations.
- [ ] Implement rollback without reinstall.
- [ ] Add conservative retention and disk-limit behavior.

Validation:

```text
candidate failure leaves active unchanged
promotion routes only new requests
in-flight requests finish on their original generation
rollback reactivates retained previous stable generation
concurrent promotion and request admission tests
```

Rollback point: atomically restore the previous stable routing pointer and
retire the candidate.

## Phase 7: Operational hardening

- [ ] Update process lifecycle and deployment documentation.
- [ ] Add metrics and structured logs for workers and generations.
- [ ] Exercise long-running repeated requests and verify bounded RSS behavior.
- [ ] Validate container memory, temporary storage, shutdown deadline, and
      runtime disk retention under representative concurrency.
- [ ] Update Linux official artifact smoke coverage for the new runtime path.
- [ ] Verify no sensitive request data appears in logs.

## Final review gates

- [ ] Run the wrapper backend spec quality checklist across every affected layer.
- [ ] Run full CI on Linux and Windows:

```text
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
official Linux runtime smoke
Docker build and container smoke
```

- [ ] Review all changes against `prd.md` and `design.md`.
- [ ] Confirm every commit contains only task-owned files.
- [ ] Keep iterative commits for review, then consolidate commit fragmentation
      only after CI passes and before owner merge decision.
- [ ] Create the PR with migration, rollback, performance, and compatibility
      evidence.
- [ ] Do not merge without owner approval.
