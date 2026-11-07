# Design Reviser -- single-runtime scope amendment

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper
Branch: feature/resident-worker-runtime-upgrade
Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948 (before installer changes).

## Role, authority, and inputs

You are the Design Reviser. Produce a design for the newly confirmed scope;
do not implement code or certify your own result. This is a requirements
amendment, NOT another closure round for G1-G5. The previous design and its
round-4 PASS are historical evidence only. This brief supersedes both the
previous reviser brief and its prohibition on editing section 0.

Read in order:
1. review/owner-decisions.md, S1-S4, under the active task.
2. The amended prd.md, including requirements 2-4 and AC2-AC4.
3. approval.md's current status: old implementation lock withdrawn.
4. design.md and review/design-revisions.md for existing structure/context.
5. Parent .trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md,
   sections 6-8, plus .trellis/spec/wrapper/backend/{index,
   process-lifecycle,dependency-sources}.md.

The revised PRD and recorded owner decisions supersede the specifically named
inherited version-directory and cleanup rules. Do not require those superseded
rules to be reintroduced. All unrelated contracts and dependency rules stand.
The dirty source is an earlier implementation, NOT design authority.

## Ownership rules

- Write ONLY this task's design.md and review/design-revisions.md. If a real
  unresolved user decision prevents design, write review/open-questions.md and
  stop that part instead of silently choosing policy.
- PRD, owner-decisions, approval, implementation plan/briefs, parent documents,
  all specs, source, tests, Cargo files, and git state are READ-ONLY.
- No builds, tests, dependency changes, commits, pushes, merges, or subagents.
- Never revert/clean the pre-existing dirty worktree or remove task artifacts.
- Maintain sections 0-13 and update all affected clauses consistently. Changes
  must serve the new scope; do not rewrite unrelated resolution/transport logic.
- Trace decisions to requirements/specs or mark new choices [ASSUMPTION] at
  their use sites and in section 12. Assumptions are not user approvals.

## Required simplification

- Exactly one installed LIBS_DIR/runtime, not versions/<id>. Return fixed
  binary/Python/worker paths. Valid matching installation is reused.
- A missing, mismatched, or corrupt installation is prepared in owned staging
  then installed/replaced before backend start. After successful replacement,
  do not retain a history of usable runtimes or build a generation store.
- Matching is not just a yt-dlp string: account for Python/platform/artifact
  and embedded-worker identity. Existing runtime_id/generation_id metadata may
  remain for the frozen handshake/logs without a generation manager.
- Preparing a candidate must not modify the old install. If download,
  extraction, or verification fails, preserve old install bytes, clean this
  attempt's quiescent owned staging, and fail boot without old-version fallback.
- A completed manifest is NOT required for cleaning this invocation's owned
  temporary work. Replace the previous A7 rather than treating it as approved.
  Do not silently sweep other invocations' or crashed boots' staging.
- Keep one manifest, mature sha2/zip implementations, binary CLI + Python
  resident separation, and existing protocol/pool/HTTP boundaries. Do not
  redesign retained-archive integrity policy unless the new requirement truly
  demands it; update its location from versions/<id> to runtime/.
- Remove future generation, rollback/history, retention/GC and active/previous
  pointer infrastructure from this unit, including claims that today's layout
  reserves it for later. This is a simplification, not a new updater framework.

## Replacement must be implementable

State the successful and failed fixed-directory transitions on Windows and
Unix. Do not assert that std::fs::rename atomically overwrites an existing
nonempty directory on both platforms. Separate guarantees during preparation,
publication, cleanup, and abrupt process death. Partial installs must never be
returned as ready; next startup must not silently use a mismatched runtime.

If a short-lived filesystem transition needs temporary storage for an old
directory, distinguish it from retained selectable versions, constrain its
lifetime/cleanup, and mark any mechanism not established by the PRD as a
proposed assumption. Do not infer approval of multiple installed histories.
Similarly state the deployment/concurrency precondition for a shared libs-dir;
do not remove files under another live backend or invent a distributed lease
system. Prefer the smallest sufficient design and flag unresolved policy.

## Validation and error contract

Update section 11 so its offline assertions exercise:
- Empty install; valid matching reuse without re-download or staging.
- Requested identity change followed by successful fixed-path replacement,
  with no retained history; embedded worker/Python matching remains addressed.
- Corruption/manifest mismatch causes reinstallation, never unchecked reuse.
- Failed preparation preserves the prior installation and rejects boot;
  absence of a prior installation never exposes partial runtime paths.
- This attempt's failed staging is cleaned after writers stop without a
  manifest; unrelated staging is untouched; publication/cleanup failures have
  explicit outcomes without unsupported atomicity claims.
- Dependency removal in one owner delivery commit and frozen backend contracts
  remain covered. cargo tree is allowed by PRD AC1; do not claim it is wider
  than full cargo metadata or that any offline command needs no local cache.

Only add mechanical checks needed for these behaviors. No production test-only
hooks, speculative extension points, or assertions that force the old layout.
Align error mappings across sections 5-8 and 10 for the changed paths; preserve
the previously resolved non-conflicting mappings.

## Deliverables

1. Revised design.md, consistent with the amended PRD throughout, including
   section 0, layout/wiring, replacement/reuse/cleanup, compatibility, errors,
   tests, assumptions, and rejected alternatives.
2. Append to review/design-revisions.md: this owner-amendment revision, touched
   sections, assumptions retired/added, explicit parent overrides, and the
   implementation-plan impacts. Do NOT edit implement.md or approval.md.
3. Return the new SHA256, sections changed, unresolved questions/assumptions,
   and exact edited-file list. No PASS/FAIL verdict on your own work.

Read in bounded windows using rg first. Preserve UTF-8 without BOM and existing
line endings. Do not claim a fresh design lock or implementation acceptance.
