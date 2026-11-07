# Implementer brief -- single runtime, Stage 3

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Branch: feature/resident-worker-runtime-upgrade
Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948
Design SHA256: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

## Role, authority and admission

You are the Implementer, already the bounded implementation worker. Implement
the approved design, not a replacement design. You do not judge acceptance;
do not issue PASS/FAIL verdicts about your own output. This brief supplies your
role, ownership and output schema in full; no hidden profile notes are needed.
The user's temporary model override is remote-magpie/auto/gpt-6-astra with
Implementer auto-review/xhigh settings. Do not edit global profiles.

This dispatch authorizes Stage 3 ONLY of implement.md and supersedes earlier
Stage-1/2-only scope and stop instructions. Their ownership principles remain.
Stage 2 is checkpointed, not independently accepted. Preserve that work.
Stop after Stage 3 for independent code review; do not dispatch a reviewer,
stage, commit, push, deploy or change task lifecycle status.

Read actual files in bounded windows before editing:
- Task approval.md, prd.md, complete design.md, implement.md, implement.jsonl
  and check.jsonl with their applicable spec/research references.
- review/owner-decisions.md S1-S5: A8/A9 accepted by S5, superseding only their
  pending-confirmation wording in the reviewed document. A7 retired. A1-A6
  still have rollout confirmation requirements; do not claim upstream pins
  were verified by fixture tests.
- .trellis/spec/wrapper/backend/index.md, process-lifecycle.md,
  dependency-sources.md and .trellis/spec/guides/paseo-dispatch-sop.md.
- review/implementation-start.json, review/stage3-start.json,
  review/implement-progress.md, review/responses-stage2.json, responses.json.
- Actual main/backend/lib/installer code, README and relevant existing tests;
  request/worker/pool code is read-only context, not a refactoring invitation.

Recompute the design hash and verify approval. Missing approval/design ->
review/blocked.md and stop. Active blocked.md/spec-gap.md, hash mismatch or
unexpected concurrent changes -> stop/report; do not repair the inputs.
The approval is explicitly a worktree-content lock, not a claim that docs
were already committed. Do not manufacture that commit.

## Starting facts

The coordinator checked all 35 original frozen/preserved hashes unchanged
after Stage 2. Sources match the Stage 2 report:
- src/runtime_installer.rs:
  8DB4CAFD6649FF81D45E7EFDB0B2B3974B5D8C901BE6892C530E1755C69139D2
- tests/runtime_install.rs:
  F9134DB10EDD62185A0AD5E66695EA1C35745D0B536BA000416121FEF255200B

Stage 2 reports Windows fmt exit 0, 24 module-local tests exit 0 (includes
a child-fixture entrypoint), 15 integration tests exit 0. Its coverage map
is in implement-progress.md. No Windows matrix row is intentionally deferred;
native Unix sync/permission/link cases are unexecuted. This is reported
evidence, not your own execution evidence or an acceptance verdict.

Cargo.toml/lock and main/backend/lib already have authorized, uncommitted
crate removal and boot-wiring edits from before Stage 1. Preserve and audit
them rather than reverting to the Git baseline. The new installer/tests are
untracked; ordinary git diff does not include them. Read those files directly.
There are no active code findings.json, blocked.md or spec-gap.md at dispatch.

## Stage 3 work

1. Audit the real boot chain against sections 2, 3, 7, 8 and 9. Installation
   must complete before backend start and listener binding, with one installer
   path for both backends. CLI remains default and consumes the installed
   standalone binary, NOT python -m yt_dlp. Resident consumes interpreter,
   worker copy and package paths as data. Runtime-id/generation_id is metadata,
   not a version store. No backend-to-installer reverse dependency or fallback.
2. Verify section-7 flag/env defaults and explicit override precedence:
   resident worker interpreter/script overrides win; runtime package paths and
   existing worker-python-path behavior remain; expected worker version defaults
   to the runtime pin, equal explicit values work, differing values return
   RUNTIME_VERSION_MISMATCH before admission/retry. Preserve existing facade
   call signatures and frozen behavior. Add narrow private boot/config tests
   in allowed files only where needed; no public introspection/mock hooks.
   Record file:line anchors for the audited production handoffs.
3. Update README for the actual fixed layout and operations. Explain the two
   distinct installed artifacts without implying two CLI launch modes: the
   official standalone yt-dlp binary serves CLI; pinned Python plus the
   yt_dlp wheel serves resident. The worker script is embedded from the wrapper
   build and copied locally, NOT downloaded upstream. Document relevant flags
   with exact defaults and overrides, without claiming live upstream availability.
4. Document libs/runtime/ plus owned staging, full-identity valid reuse,
   mismatch/corruption preparation and replacement, no history/backup/rollback,
   and A8 private canonical root for one instance's entire serving lifetime.
   Deployment is stop -> await all old consumers/children -> start, with same
   local filesystem and ordinary runtime/staging directories. No automatic
   locking or multi-instance coexistence is promised.
5. Document A9: preparation failure preserves old bytes but rejects boot;
   verified candidate publication invalidates old manifest, removes old tree
   and renames into absent runtime/. Failure can leave absent/invalid runtime;
   fail boot and reinstall next startup, no old-version fallback. Do not claim
   atomic delete-plus-rename or old-or-new crash durability. Legacy flat files,
   old versions/ stores and foreign/crashed staging are never swept. Explain
   manual handling while all consumers are stopped or using a fresh private
   root; never recommend blanket deletion of libs/.
6. Preserve the existing crate/lofty removal and direct sha2/zip dependencies.
   No handwritten hashing/compression/archive format code, new downloader/HTTP
   framework, retention/generation/lease system or speculative cleanup layer.
   Keep changes small; do not rewrite completed installer/test matrices for style.
7. Run final gates on the resulting code, in this order, recording command,
   actual exit, summary counts and prerequisites:
   - cargo fmt --check
   - cargo clippy --locked --all-targets -- -D warnings
   - cargo test --locked
   - python -m unittest tests.test_ytdlp_worker -v
   Fix only in-scope issues in writable files, then rerun affected checks and
   ensure final-tree evidence is current. Do not repeatedly run the full suite
   after each small patch. Do not weaken/skip assertions to make gates green.
   If a boot test needs an executable path, supply the freshly built binary
   through its existing harness mechanism and record it; do not alter the
   frozen test. No live-source artifact fetch is required by these tests.

Native Linux, CI matrix, release/container and live-default validity evidence
must remain explicitly pending unless actually executed in an authorized
environment. No push or CI trigger is authorized just to acquire evidence.
Do not connect to another host, deploy or clean a live root in this assignment.
Windows success does not prove Unix behavior. AC1's delivery-history procedure
is pending the owner's ONE delivery commit after review; final-tree dependency
checks cannot establish that history and you must not create the commit.

## Exact write boundaries and safety

- Writable code/docs: src/main.rs, src/backend.rs, src/lib.rs only for needed
  boot/data wiring; README.md for the stated operations; src/runtime_installer.rs
  and tests/runtime_install.rs for necessary in-scope fixes. Module-local tests
  in these files are allowed. No other source, tests or fixtures may change.
- This stage releases ONLY main/backend/lib/README from the previous preserved
  list. review/stage3-start.json records their starting bytes and the remaining
  31 immutable file hashes. Do not edit that snapshot or implementation-start.
- Cargo.toml/lock stay read-only this dispatch; if a design-required dependency
  change truly becomes necessary, report the precise need rather than adding it
  silently. No CI, Docker, scripts, worker protocol/pool or request-lifecycle edits.
- All task PRD/design/approval/plan/briefs/owner decisions/revision and finding
  records, parent documents and .trellis/spec/ files are read-only. No changes
  even for a clarification or to satisfy a test. Design conflict/omission ->
  stop the affected part and write review/spec-gap.md with clause and blocked work.
- Green tests/lint are behavioral/mechanical evidence, not architecture proof.
  No public testing seam, production test toggles, exposed internals or bypassed
  boundaries. Existing module-local cfg(test) faults remain test-only.
- Out-of-scope real issue -> located review/follow-ups.md entry, not a fix.
  Do not touch unrelated dirtiness: todos/, response.json, .claude/, .opencode/,
  .trellis/spec/mpd/ or myMPD/, 00-bootstrap-guidelines, docs/superpowers/,
  examples/, .tmp-*.log, target-gate/, 11-03 task/review records or caches.
  Frozen whitespace warnings are pre-existing, recorded and not yours to fix.
- No git stage/commit/push/merge/reset/revert. Never delete work to recreate it.
  Observe permissions; denied cache/network writes require escalation, not bypass.
- The checkout has no native .agents/.codex Trellis helpers. Implement directly,
  report this fallback, and do not spawn another Paseo role or install helpers.
  No planning/brainstorm/update-spec/finish-work. Reviewer dispatch is coordinator-owned.
- Use bounded 50-100 line reads and rg. On context pressure write a coherent
  progress checkpoint with unfinished items, never imply completion. Do not poll.
- Use isolated test roots only. Windows delete/move targets must be resolved,
  contained and checked; use literal paths and one shell end-to-end. Stop/await
  writers first. Background helpers must not open visible windows.
- JSON must be UTF-8 without BOM. Use rtk proxy only for unusable command output.
  Do not treat truncated/old logs as new evidence.

## Reporting and review contract

Writable reports ONLY: append review/implement-progress.md; overwrite
review/responses.json; write review/blocked.md or review/spec-gap.md if needed;
append review/follow-ups.md for located out-of-scope issues. Other archives,
snapshots, coordinator dispatch records and findings are read-only.

Append Stage 3 evidence: exact changed files/reasons, boot call-chain anchors,
configuration/README coverage, gate commands/exits and counts, all 31 immutable
hash comparisons, final writable hashes, pending Linux/CI/release/container and
rollout/history evidence. Separate observed facts from remaining limitations.

No CODE review findings.json exists, so round remains 1; design-review rounds
are a different cycle. If a code review unexpectedly appears, stop/coordinate.
In later assigned repair rounds every finding needs ACCEPTED with minimal fix,
REBUTTED with read production-reachability/clause evidence, or SPEC_CONFLICT
with spec-gap and stop. Tests passing is not a rebuttal. Round-3 unresolved
disputes escalate rather than creating endless repair rounds.

Use exactly this responses.json schema:
```json
{
  "round": 1,
  "scope_summary": "Stage 3 completed or checkpointed; remaining evidence explicit",
  "changes": [{"file": "", "intent": "", "spec_clause": "design.md section x"}],
  "test_notes": "Actual commands/exits, behavior, OS evidence and pending checks",
  "finding_responses": []
}
```

Final reply only two or three factual sentences on changes, verification and
remaining work; no acceptance verdict. Stop for independent code review.
