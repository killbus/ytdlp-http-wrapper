# Implementation plan -- single installed runtime

Locked design SHA256:
DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59
Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948.
Authority: approval.md and review/owner-decisions.md S1-S5. A8/A9 accepted;
A7 retired. Old plans/responses are archived under review/*before-single-runtime*.
They are not completion evidence for this scope. The task remains in_progress.

Read implementer-brief.md, approval, owner decisions, PRD, complete design in
bounded windows, and applicable .trellis/spec/wrapper/backend guidelines before
code edits. The design is the authority, not the old dirty prototype.

## Stage 1 -- Core lifecycle and basic regressions

Scope: src/runtime_installer.rs and tests/runtime_install.rs. Implement this
stage first and return a coherent checkpoint; do not start Stage 2 in the same
dispatch. Preserve mature sha2/zip and unrelated source-resolution logic.

- Fixed runtime/ instead of versions/<id>. Identity is metadata, not directory
  selection. Match full versions/platform/artifacts/worker and required paths;
  reuse a valid match without fetching/staging; missing/mismatched/corrupt
  runtime prepares a replacement (design sections 4, 8).
- Preserve old bytes throughout preparation. Accepted A8/A9 transition:
  contained ordinary paths, exclusive root precondition, closed handles and
  stopped children; invalidate old manifest, remove old tree, rename verified
  staging to an absent target, sync where specified and validate publication.
  No backup, copy-over, generation layer, lock/lease, or old-runtime fallback.
- Replace marker-only removal and cross-boot sweeping with current-invocation
  creation, containment and stopped writers. Cleanup needs no completed
  manifest, never follows links outside owned paths, never touches foreign
  staging and cannot delete runtime after publication. Surface cleanup failure
  with primary cause; retry only pre-retirement download errors after cleanup
  succeeds (sections 8, 10).
- Replace obsolete layout/coexistence assertions. Focused tests: empty install;
  reuse with fixture sources unavailable; fixed-path replacement removes an
  old-only sentinel; corruption reinstalls; failed preparation preserves old
  bytes; failed staging without manifest is reclaimed; foreign staging intact.
- Run cargo fmt --check and focused installer integration/module-local tests.
  Record command exits and explicit Stage 2/3 work. Do not claim full coverage.

## Stage 2 -- Complete acceptance and failure fidelity

Scope: the same installer and tests, after the Stage 1 checkpoint.

- Complete section-11 identity matrix: yt-dlp, Python version/release, digest,
  embedded worker; manifest platform/identity independent of abbreviated id;
  retained archive/binary/worker corruption and repair.
- For both mismatched and corrupt installs, failed source/hash/extraction
  preserves exact old snapshots; no prior runtime exposes no partial paths.
- Exercise publication/cleanup errors and restart states: pre-retirement
  preservation, invalid/absent state after retirement, post-rename sync/check
  failure and cleanup-error precedence. Native filesystem denials/locks where
  deterministic; remaining fault injection only private cfg(test) per section
  11. No production test APIs or generic pluggable-updater abstraction.
- Complete actual sections 5-8/10 resolution/download error and retry mapping.
  Keep defaults, channels and general-purpose dependency boundaries unchanged.
- Correct AC1 commentary: cargo tree is allowed, not wider than full metadata;
  offline commands require local cache. Keep dependency/lockfile assertions.
- Focused tests and fmt; append exact section-11-to-test map, OS evidence and
  remaining gaps. Windows success is not Linux runtime evidence.

## Stage 3 -- Boot, documentation and final gates

Scope: src/main.rs, src/backend.rs, src/lib.rs only as required for data-only
wiring; README.md for the operational contract; necessary installer/test fixes.
Do not change worker protocol, pool, request lifecycle or frozen test suites.

- Confirm installation precedes backend startup/listener binding; both backends
  receive fixed paths and existing generation_id is metadata only. Preserve
  section-7 explicit overrides and CLI default, without fallback.
- Document layout/versions, stop-await-start and private root (A8), same-volume
  ordinary directories, no history/backup, publication gap/reinstall (A9), and
  no automatic migration or sweeping of legacy files. No whole-libs deletion
  instructions for users.
- Preserve already-uncommitted crate/lofty removal and sha2/zip dependencies.
  No Cargo edits unless necessary under the locked design; no new downloader,
  HTTP client, hand-written crypto/compression/archive implementation.
- Final gates, with actual exit evidence: cargo fmt --check; cargo clippy
  --locked --all-targets -- -D warnings; cargo test --locked;
  python -m unittest tests.test_ytdlp_worker -v. Run focused checks during edits,
  not the full suite after every patch. Existing CI matrix owns native
  Linux/Windows and release/container gates; if new evidence is unavailable,
  leave it explicitly pending. No push/CI trigger is authorized solely to
  obtain that evidence. No live upstream download is needed by fixture tests.
- Verify frozen inputs/suites unchanged from implementation-start.json. Write
  responses.json, then obtain code review. AC1 commit-history acceptance only
  follows the owner's single delivery commit; never claim an unmade commit.

## Ownership, reporting and recovery

- Only the dispatched stage is authorized. Append progress after each stage;
  stop between stages to avoid context exhaustion. A context-limited checkpoint
  must identify what is incomplete, not silently return unfinished code as done.
- Missing precondition -> review/blocked.md. Design conflict/gap ->
  review/spec-gap.md with clause and blocked work; do not edit frozen documents.
  Out-of-scope observations -> review/follow-ups.md, not extra fixes.
- Design/PRD/approval/plan/briefs/owner decisions/specs are read-only to workers.
  No git stage/commit/push/merge/reset/revert, and no deletion of existing work.
- Tests use isolated temporary fixture roots, never the real libs installation.
  Check resolved deletion targets stay within those roots; stop writers first.
- Never stage/touch unrelated todos/, response.json, .opencode/, .claude/,
  .trellis/spec/mpd/ or myMPD/, 00-bootstrap-guidelines, docs/superpowers/,
  examples/, .tmp-*.log, target-gate/ or 11-03 task/review changes. Generated
  __pycache__ is not a manual edit/cleanup target.
- Report behaviors, commands/exits, files and remaining work, not an acceptance
  verdict. The reviewer owns architecture/implementation acceptance.
