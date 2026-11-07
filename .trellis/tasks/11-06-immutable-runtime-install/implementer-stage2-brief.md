# Implementer brief -- single runtime, Stage 2

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Branch: feature/resident-worker-runtime-upgrade
Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948
Design SHA256: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

## Role and admission

You are the Implementer, not the designer or acceptance reviewer. Implement
only within the locked design. Do not issue PASS/FAIL acceptance verdicts.
This brief fully supplies your role and rules; profile notes are not injected.
The user temporarily selects remote-magpie/auto/gpt-6-astra for every role;
other Implementer settings remain auto-review/xhigh. Do not edit profiles.

This dispatch authorizes Stage 2 ONLY of implement.md. It supersedes the
Stage-1-only initial assignment and final stop instruction in the older
implementer-brief.md; its ownership/admission rules remain binding. Stage 1
is already implemented; do not restart or replace it wholesale. Stage 3,
independent review, deployment and the owner delivery commit remain later work.

Read these actual files before edits, using bounded windows:
- approval.md, prd.md, all design.md sections, implement.md in the task.
- review/owner-decisions.md S1-S5; S5 accepts A8/A9 without changing the
  reviewed design hash. A1-A6 retain their production-rollout confirmation
  requirement; A7 is retired. The design's older A8/A9 pending text is
  superseded only by S5, not a license to rewrite it.
- implement.jsonl and every applicable spec/research file it references;
  .trellis/spec/wrapper/backend/index.md, process-lifecycle.md and
  dependency-sources.md, and .trellis/spec/guides/paseo-dispatch-sop.md.
- review/implementation-start.json, review/implement-progress.md,
  review/responses-stage1.json, current review/responses.json.
- Current installer/tests and their actual production callers, read-only
  outside the two authorized code files.

Verify design SHA256 and approval. Missing input -> review/blocked.md and
stop. Hash mismatch, active blocked.md/spec-gap.md or unexpected concurrent
changes -> stop and report. Do not treat authorized pre-existing dirtiness
as a new blocker. A worktree-content lock is explicit in approval.md; no
pre-implementation docs commit is claimed or authorized.

## Starting checkpoint

Stage 1 changed only src/runtime_installer.rs, tests/runtime_install.rs and
permitted reports. All 35 frozen/preserved hashes in implementation-start.json
were rechecked unchanged by the coordinator before this dispatch.
Current code SHA256:
- src/runtime_installer.rs:
  70A57347824A1AE3135C91059B6CDDA3AE8F59CBA8933E6F558E9DEDE1ECAC44
- tests/runtime_install.rs:
  78A849CABA9FDB4E147B0A50C4E3DD025B25C88129267248E7B883A1946BD2E2

The checkpoint has fixed runtime/, full-identity reuse, verified staging,
manifest invalidation/removal/rename-to-absent publication, invocation-owned
manifest-free cleanup and no public fetch-counter test API. Preserve these.
Windows focused evidence: fmt exit 0, 11 integration tests exit 0, 3 local
installer tests exit 0. This is checkpoint evidence, not final acceptance.

Existing Cargo.toml/lock and src/lib.rs/main.rs/backend.rs changes predate
Stage 1 and are preserved this stage. All unrelated dirty/untracked paths
remain untouched, including todos/, response.json, .claude/, .opencode/,
.trellis/spec/mpd/, .trellis/spec/myMPD/, 00-bootstrap-guidelines,
11-03 task/review records, docs/superpowers/, examples/, .tmp-*.log,
target-gate/ and generated caches. Existing frozen-file whitespace warnings
are recorded in review/follow-ups.md; do not repair them.

## Stage 2 work

1. Complete section-11 identity and repair coverage. Independently vary
   yt-dlp version, Python version/release, artifact digest and embedded-worker
   identity; independently corrupt manifest runtime_id, platform, Python
   identity, artifact digest and worker hash. Cover retained Python archive,
   binary copy and worker-copy corruption. Assert fixed paths, correct full
   identity/bytes, removal of old-only sentinel, and no history/backup/staging.
   Worker-identity inputs belong to PRIVATE module-local tests of the same
   logic used by production include_bytes!, not public API or env toggles.
2. For BOTH mismatched and corrupt prior installs, exercise failed source,
   hash and extraction preparation. Snapshot after intentional damage and
   before retry; require exact prior paths/types/bytes/permissions/mtime,
   no returned runtime paths, and correct cleanup. Cover no-prior-runtime
   failure/recovery without exposing partial installed paths.
3. Cover section-8/11 publication and cleanup boundaries: candidate manifest
   sync; old-manifest invalidation; retired-tree removal; rename-to-absent;
   post-rename sync where applicable; post-publication validation; cleanup
   failure precedence and retained primary cause; uncertain child quiescence;
   successful rename disarming the staging guard. Use real native filesystem
   locks/denials where deterministic, and only private cfg(test) injection for
   remaining branches. No generic updater framework or production test seam.
4. Cover restart states with fixtures: old runtime plus foreign partial
   staging, invalid manifest-free remainder, absent runtime, complete published
   candidate. Never sweep another invocation's paths. Verify failed writers
   stop and are awaited before owned cleanup using local child fixtures/private
   preparation logic; do not infer process-tree guarantees from child.wait.
5. Complete actual production source-resolution, download, tool error and
   retry fidelity per sections 5-8/10. Check all missing/unusable metadata and
   digest paths, and only retry pre-retirement DOWNLOAD_FAILURE after successful
   cleanup. Tests must exercise the real logic rather than separate test-only
   replicas. Keep defaults/channels/dependency boundaries unchanged.
6. Correct inherited AC1 commentary: package-scoped cargo tree is allowed,
   not wider than full cargo metadata; --offline needs cached prerequisites.
   Include --locked and retain graph/manifest/lockfile assertions. Delivery
   history acceptance remains pending the owner's single commit.

Coordinator inspection leads, NOT formal review findings: extract_tar currently
passes ExtractFailure as both run_command error classes, while missing tar must
be CURL_MISSING with tool=tar; binary SUMS curl handling appears to classify
HTTP 404 as DOWNLOAD_FAILURE rather than RESOLVE_FAILURE. Verify directly
before fixing. Other transfer failures remain download errors; unusable wheel
digests must fail resolution. Do not widen into unrelated resolver refactoring.

## Ownership and execution rules

- Only writable code: src/runtime_installer.rs and tests/runtime_install.rs,
  including module-local tests. No Cargo, README, boot/backend/protocol/pool or
  existing contract-suite edits. No deleting the installer to rewrite later.
- prd/design/approval/implement plans, every brief, owner/revision records,
  snapshots, parent documents and all .trellis/spec/ files are READ-ONLY.
- A design conflict, omission or infeasibility -> stop the affected part and
  write review/spec-gap.md with clause, problem and blocked work. Never revise
  the yardstick or silently choose a new policy to make tests green.
- No new versions store, backups/history, fallback, generations, retention/GC,
  cross-boot sweeps, locks/leases or handwritten crypto/compression/archive code.
  Keep mature sha2/zip. Do not invent speculative defense layers.
- Module boundaries beat green tests: no public mock seams, exposed internals,
  production test-only branches, reverse dependencies or abstraction bypasses.
  Private cfg(test) fault injection is permitted by section 11 only.
- Out-of-scope real issues: append a located note to review/follow-ups.md;
  do not fix/refactor. Never stage/commit/push/merge/reset/revert or deploy.
- You are already the bounded implementation worker. No native Trellis helper
  exists in this checkout's .agents/.codex; implement directly and record that
  fallback. Do not spawn recursive Paseo roles or install helper infrastructure.
  No planning/brainstorm/update-spec/finish-work. Acceptance review is separate.
- Use rg and 50-100 line reading windows; concise command output. Make coherent
  patches and checkpoint before context exhaustion. If Stage 2 cannot finish
  in one turn, explicitly list unfinished matrix rows; never claim completion.
- Use isolated fixture roots only, never a real libs/deployment root. On Windows
  verify resolved absolute deletion containment, use literal paths in one shell,
  stop/await writers first. No cross-shell deletion or visible helper windows.
- Follow sandbox approvals for denied cache/network access; no bypass through
  alternate tools. Tests need no live upstream artifacts. Do not push for CI.
- Run fmt plus focused installer integration/module-local tests after coherent
  changes. Full Clippy/Rust/Python gates belong to Stage 3, not each patch.
  Record commands/exits and OS limits; Windows tests do not prove Linux behavior.
- Recheck all frozen/preserved hashes before reporting. Use UTF-8 without BOM
  for JSON. rtk proxy only when normal output is unusable. Do not poll agents.

## Reporting and convergence

Writable task reports ONLY: append review/implement-progress.md; overwrite
review/responses.json; write review/blocked.md or review/spec-gap.md as needed;
append review/follow-ups.md. Do not edit findings or archived checkpoint reports.
Append a Stage 2 section with exact section-11-to-test mapping, commands/exits,
actual OS evidence, source hashes, and outstanding Stage 2/3/native Linux gates.

No CODE review/findings.json exists, so implementation round=1 and responses
are empty; do not derive the round from design-findings.json. If code findings
appear unexpectedly, stop and coordinate instead of inventing a review cycle.
In a later assigned repair round each finding needs ACCEPTED (minimal fix),
REBUTTED (read production reachability/clause evidence), or SPEC_CONFLICT
(spec-gap and stop). Tests alone never rebut architecture clauses; unresolved
round-3 disputes go to the owner, not an endless repair loop.

Write exactly this responses.json schema:
```json
{
  "round": 1,
  "scope_summary": "Stage 2 work completed; remaining work and non-goals explicit",
  "changes": [{"file": "", "intent": "", "spec_clause": "design.md section x"}],
  "test_notes": "Actual commands/exits, behaviors, OS evidence and missing gates",
  "finding_responses": []
}
```

Final reply: two or three factual sentences on progress, tests and deferred work,
no acceptance verdict. Stop before Stage 3 even if Stage 2 finishes early.
