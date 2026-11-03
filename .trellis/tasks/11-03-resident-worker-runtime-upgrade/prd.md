# Resident yt-dlp worker and runtime generation upgrade

## Goal

Define the requirements baseline for replacing per-request PyInstaller yt-dlp processes with resident Python workers, while supporting zero-downtime yt-dlp upgrades through immutable worker generations, controlled traffic switching, graceful draining, and rollback.

## Requirements

- Keep the Rust HTTP service as the control plane for admission, validation, concurrency, timeouts, cancellation, observability, routing, and worker supervision.
- Introduce resident Python workers that import yt-dlp once and create a fresh `YoutubeDL` instance for every request.
- Allow one active request per worker. If cancellation or timeout cannot safely stop yt-dlp, retire and replace that worker.
- Isolate request options, headers, cookies, proxy settings, output, downloads, and temporary files.
- Define a versioned framed protocol between Rust and Python. Do not use unframed stdout as the protocol channel.
- Manage yt-dlp versions as immutable installations. Never upgrade packages in place inside an active worker environment.
- Implement runtime upgrades as generation-based rolling replacement:
  - prepare and verify a candidate version;
  - launch a candidate worker generation;
  - run health, protocol, fixture, and extractor smoke checks;
  - optionally route canary requests;
  - atomically promote the candidate;
  - drain the previous generation;
  - retain the previous stable generation for rollback.
- Keep the Rust service available during generation changes. In-flight requests remain assigned to their original generation.
- Require owner approval by default before promoting a candidate generation to active production traffic.
- Retain the external CLI backend as a fallback until the resident backend passes compatibility, isolation, cancellation, stability, and performance validation.
- Remove the Rust `yt-dlp` crate instead of bumping it. Replace its remaining `LibraryInstaller` usage with project-owned runtime installation and version-management code so the resident-worker architecture does not retain a transitional dependency.
- Remove dependency-specific workarounds that become unused with the crate, including the `lofty` patch, after verifying they have no independent consumers.
- During detailed design and implementation, a depth-1 clone of the upstream `yt-dlp` crate repository may be used as read-only reference material for mature installer, platform, integrity, and release-handling practices. Do not vendor, fork, or retain the upstream crate as a production dependency.
- Treat this as a complex task covering detailed design and implementation. Implementation begins only after `prd.md`, `design.md`, and `implement.md` are reviewed and the Trellis task is explicitly started.

## Acceptance Criteria

- [ ] The architecture boundary assigns HTTP control, supervision, and routing to Rust and extraction work to Python.
- [ ] Every worker reports `protocol_version`, `yt_dlp_version`, `python_version`, `generation_id`, and `worker_id` during startup.
- [ ] The control plane rejects workers whose protocol or yt-dlp version does not match the generation manifest.
- [ ] Every request receives a fresh `YoutubeDL` instance and cannot inherit mutable state from an earlier request.
- [ ] Timeout, cancellation, worker crash, protocol corruption, and excessive output have explicit failure and worker-replacement behavior.
- [ ] Candidate installation cannot mutate or break the active generation.
- [ ] Failed candidate validation leaves the active generation unchanged.
- [ ] Promotion switches only new requests; in-flight requests complete on their original generation.
- [ ] Rollback can reactivate the retained previous stable generation without reinstalling yt-dlp.
- [ ] Upgrade validation covers installation integrity, protocol compatibility, local fixture contracts, representative extractor smoke tests, repeated-request stability, RSS growth, and request-state isolation.
- [ ] The existing CLI backend remains available as a controlled fallback during migration.
- [ ] The Rust `yt-dlp` crate and its transitive-only dependency workarounds are absent from `Cargo.toml` and `Cargo.lock`.
- [ ] Project-owned runtime management preserves the required download, platform selection, integrity verification, retry, executable-permission, and failure-reporting behavior without depending on the removed crate.
- [ ] Any upstream source clone used for research is shallow, read-only reference material and is not committed, vendored, or required at build or runtime.
- [ ] Performance measurements distinguish process startup, worker readiness, request execution, first response, and cleanup.

## Notes

- Runtime hot upgrade means starting a new immutable worker generation and switching traffic without restarting the Rust service. It does not mean `pip install -U`, `importlib.reload()`, or replacing modules inside a live Python process.
- Production defaults may automatically discover, download, and validate candidates, but full promotion requires owner approval.
- Retain at least the active and previous stable generations. Cleanup policy and storage limits belong in the later design task.
- The dependency removal must land as part of the reviewed implementation rather than leaving a deprecated compatibility layer or a follow-up cleanup debt.
- The abandoned `.trellis/tasks/11-03-bump-yt-dlp-2-10/` directory is outside this task and must not be committed without a separate cleanup decision.
