# Design revisions

## 2026-11-07 - Round-3 findings G1-G5

- Input design SHA256:
  BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D
- Revised design SHA256:
  3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC
- Read-only yardsticks: this unit's prd.md; parent design section 6;
  .trellis/spec/wrapper/backend/{index,process-lifecycle,dependency-sources}.md;
  baseline c08f398 (src/main.rs, src/lib.rs, Cargo.toml).
- Sections touched: 4, 5, 6, 7, 8, 10, 11, 12. Section 0 is unchanged.

### G1 - Manifest-validated, inactive-owner staging cleanup

Sections: 4, 8, 12. Basis: prd requirement 3, parent section 6, and the
wrapper process-lifecycle rule to stop writers before owned-path cleanup
and never infer ownership from age.

The marker is explicitly insufficient deletion authority. Removed the
cross-boot sweep rather than inventing a crash/live detection protocol.
Automatic removal requires current-invocation provenance, a contained
non-symlink/reparse-point staging child, stopped/awaited writers and child
processes, an abandoned attempt, and a schema-v1 manifest matching the
resolved plan/build under step 4a. Unknown/live ownership and partial or
invalid manifests retain the directory with a diagnostic. Removal IO
errors remain structured. Failure and publication-race cleanup now use
the same rule; another boot's staging cannot be swept. This preserves,
rather than relaxes, the inherited cleanup boundary, at the cost of
retaining crash residue.

New assumption: [ASSUMPTION A7] is inline in section 8. Its section 12
entry is:

> A7: automatic staging removal is limited to this invocation's owned,
> manifest-validated directory after all writers/children stop and the
> attempt is abandoned. No cross-invocation/boot stale sweep; unknown
> liveness, partial staging, or invalid/missing manifests are retained.
> This trades crash-residue reclamation for the inherited cleanup safety
> boundary without adding locks, leases, or age-based deletion. This new
> assumption awaits owner confirmation at review.

### G2 - Existing-runtime corruption acceptance checks

Section: 11. Basis: prd AC3 and requirement 8; section 8 step 4.

Added separate retained-Python-archive corruption and manifest.runtime_id
mismatch cases, each after a successful fixture install. Both retry the
unchanged original plan, sources, flags, and worker build, assert the
original derived id, RUNTIME_INTEGRITY_FAILURE, no returned paths, and
unchanged fetch/staging counters. Post-corruption/pre-retry snapshots
must equal post-rejection snapshots, including corrupt bytes, entry
types, permissions, and modification times. No repair or new versions
entry is allowed. The runtime does not re-hash the extracted Python tree.

### G3 - One failure code and retry class per condition

Sections: 5, 6, 7, 8, 10. Basis: prd requirements 4 and 6 and the existing
section 10 code names, as required by the reviser brief.

Moved the section 5 missing-tool body clauses to the table's existing
RUNTIME_CURL_MISSING for both curl and tar, identifying the tool in the
detail; neither is retryable. Moved the duplicate worker-version clause
out of the table's RUNTIME_CONFIG_INVALID row to agree with section 7's
RUNTIME_VERSION_MISMATCH, also non-retryable. Only
RUNTIME_DOWNLOAD_FAILURE gets the existing three-attempt/2s/4s policy.

Defined usable SHA256 values and made an absent/unusable digest on the
single selected wheel fail closed with RUNTIME_RESOLVE_FAILURE. The same
code covers binary API/SUMS checksum exhaustion and missing PBS API
digests without a plan; absent/unusable plan hashes instead use
RUNTIME_CONFIG_INVALID. A usable digest that disagrees with downloaded
bytes remains RUNTIME_INTEGRITY_FAILURE. The table now distinguishes
metadata transfer failures from unusable metadata; missing binary SUMS
(HTTP 404) is checksum exhaustion, while other transfer failures use
RUNTIME_DOWNLOAD_FAILURE. No absent-checksum case installs unverified
bytes or falls through to a conflicting table row.

### G4 - AC1 single-commit history procedure

Section: 11. Basis: prd requirement 1 and AC1; section 2's change list;
dependency-sources.md's matching-lockfile rule; baseline c08f398.

Added an offline Git ancestry/history/parent-to-commit inspection binding
the installer module and crate wiring, sole boot replacement,
LibraryInstaller/import/helper removal, dependency/patch removal, and
matching lockfile delta to the owner's one delivery commit C. Earlier
partial delivery in the inspected history cannot be excused by the
final-tree metadata checks. The owner records C after implementation
review/gates; the reviewer runs the procedure after that commit and before
final acceptance/push. The reviser creates no commit and runs no Cargo
command. The existing future metadata check explicitly uses offline,
locked mode.

### G5 - Restore the wheel-closure assumption at its use site

Section: 5 (matched against unchanged section 12 A6). Basis: prd
requirements 5-7 and the already-listed A6 assumption.

Restored exactly one inline A6 marker in the wheel/RECORD paragraph and
spelled out its pinned-wheel closure dependency. The owner amendment's
prohibition on hand-written RECORD parsing remains intact. No new
assumption is introduced for this finding.

### Revision checks and owner handoff

- Document-only checks: sections 0-13 remain in order; section 0 is
  byte-identical; original UTF-8 without BOM and LF endings are preserved.
- A1-A6 each have exactly one inline marker; new A7 also has exactly one
  inline marker and a matching section 12 entry. A7 is the only new
  assumption and requires owner confirmation.
- No open-questions.md was needed or written. No PRD ambiguity required
  escalation; the new cleanup policy stays within its safety boundary.
- No Cargo command or test suite ran. No commit, push, merge, or Git-state
  write occurred. The pre-existing dirty implementation was not modified.
- Only design.md and this revision record were edited. A content-hash
  comparison of the 257 tracked/untracked files outside the allowed
  design/review paths found no changes; Git HEAD/index were unchanged.
- approval.md was not edited; its design_sha256 is now stale. The owner
  must review A7 and re-lock the revised design hash.

## 2026-11-07 - Owner amendment: one fixed installed runtime (S1-S4)

This entry supersedes the earlier revision's layout, reuse-failure, and
A7 cleanup conclusions. The preceding entry and round-4 snapshot remain
unchanged historical evidence, not approval of this amended design.

- Input design SHA256:
  3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC
- Revised design SHA256:
  DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59
- Authority: design-single-runtime-brief.md, amended prd.md, and
  review/owner-decisions.md S1-S4. approval.md explicitly withdraws the
  old implementation lock. Source baseline for citations/AC1 is now
  775bd5fa974f1008bb24e71ea92f674515706948, before installer changes.
- Sections changed: 0, 1, 2, 4, 6, 7, 8, 9, 10, 11, 12, 13, plus the
  document title/baseline introduction. Sections 3 and 5 are unchanged.

### Scope and explicit parent overrides

Sections 0, 1, 2, 4, 6, 7, 9, 13 now describe one LIBS_DIR/runtime and
fixed backend paths. Missing, mismatched, or corrupt installed content
requires a verified staged replacement, not a second identity-addressed
directory or a terminal reuse-only rejection. Matching checks configured
yt-dlp, Python/release, platform, full artifact digests, and embedded worker
identity; runtime_id/generation_id remain metadata for existing contracts.
The abbreviated id is not sufficient evidence for reuse.

Per S3, parent section 6's versions/<id> collection and universal
manifest-before-temporary-cleanup rule are explicitly overridden. Parent
section 7's retained versions, switching, retention/GC, and pointer layer
are excluded, not reserved future infrastructure. Parent section 8's
publication requirement is concretized as staged replacement with the
documented non-atomic gap, not rename over a nonempty directory. All
unrelated API/backend/pool/protocol and same-commit dependency-removal
contracts remain unchanged. The earlier prototype layout is not silently
migrated or swept; deployment uses a private suitable root.

One manifest and retained runtime/artifacts/ archives remain the integrity
policy, with mature sha2/zip and separate CLI binary/Python closure. Section
5's source resolution, missing-tool/digest mappings, and download behavior
are byte-identical to the input. Extracted Python trees are still not
re-hashed on reuse; the design explicitly states that detection limit.

### Replacement, cleanup, and failure boundaries

Section 8 separates preparation from retirement/publication. Until the
candidate is verified and its manifest synced, runtime/ is not modified.
Preparation failure returns a structured error with no old-runtime paths
or backend start. This invocation stops and awaits its writers/children
and removes its created, contained staging even when no manifest exists.
Other invocations' and crashed boots' staging is never swept. Uncertain
quiescence/ownership or cleanup failure is diagnosed and prevents retry.

The proposed minimal replacement creates no old-directory backup. Under
exclusive deployment, invalidate the old manifest before removing other
old files, remove runtime/, and rename the candidate only to the absent
fixed path. Unix directory sync points and Windows sharing/permission
failures are distinguished. Delete + rename is not an atomic transaction.
Once retirement starts, an error/death can leave an invalid remainder or
no install; boot fails and recovery is later staged reinstallation, not
restore or an old-version fallback. The old-manifest invalidation avoids
treating a partially deleted tree as ready after process death.

Sections 8 and 10 distinguish candidate/preparation errors, publication
phase errors, post-publication integrity failure, and cleanup/quiescence
errors. Cleanup failure takes precedence with the original cause retained.
After successful rename the staging guard is disarmed. A post-rename sync
failure returns no paths even if the complete candidate is present. No
portable power-loss old-or-new transaction is claimed. Subsequent boots
validate only runtime/ against their own request and ignore prior staging.

### Assumptions retired and proposed

- A7 is retired by S2/prd req 3, not approved. Current-invocation creation,
  containment, and stopped writers authorize failed temporary cleanup
  without requiring a completed manifest. There is no active inline A7.
- A8 is new, marked inline in section 8 and listed in section 12: one
  operator-serialized private libs-dir for a wrapper's full lifetime,
  all prior consumers/children stopped before startup, reserved runtime/
  content, and ordinary runtime/staging directories on one local
  filesystem. This is a deployment precondition, not an implemented lock
  or distributed liveness service. Shared/rolling deployment needs
  separate roots. Owner confirmation remains outstanding.
- A9 is new, marked inline in section 8 and listed in section 12: after
  verification, invalidate/remove the old tree then rename the candidate
  to absent runtime/, without backup/restore. The availability gap and
  possible old-install loss after retirement starts require owner
  confirmation. Preparation failure still preserves old bytes exactly.
- A1-A6 are carried forward and remain subject to owner confirmation;
  S1-S4 is not approval of their defaults/trust assumptions. Each active
  assumption A1-A6/A8-A9 has one inline marker and a section 12 entry.
- No unresolved requirement interpretation prevents this proposed design;
  no open-questions.md was needed or written. A8/A9 must be resolved in
  review before a new implementation lock; they are not silent policy.

### Validation and implementation-plan impacts

Section 11 replaces the old coexistence/reject-only tests with fixed-path
replacement assertions. It covers empty install and no-fetch/no-staging
reuse; separate version/Python/artifact/worker identity changes; manifest
mismatch and retained-archive/binary/worker corruption; successful
replacement without old sentinels/history; failed preparation preserving
old snapshots and rejecting boot; no-old-runtime failure; manifest-free
owned cleanup with foreign staging untouched; publication/cleanup errors;
and restart states after interrupted removal or publication. Native
Windows/Unix filesystem evidence is required; branch injection stays
module-local under cfg(test), with no production test hooks. Restart
fixtures do not claim to simulate power-loss durability.

AC1 retains the owner-created single-delivery-commit procedure using the
new source baseline. Package-scoped cargo tree or full cargo metadata is
allowed, with an explicit local-cache prerequisite for offline execution;
no claim is made that cargo tree has wider coverage. Frozen backend gates
are retained. No Cargo command, build, or test suite was executed this turn.

After fresh design review and owner decisions, implement.md and the
implementation brief will need to replace version-directory planning,
the early corruption-rejection outcome, manifest-gated cleanup, and
concurrent-publication race handling with the fixed paths, full matching,
owned cleanup, A8/A9 transition, phase-specific failures, and revised
offline assertions. Deployment documentation must carry the maintenance
precondition and post-retirement availability limit. Those files and the
pre-existing dirty implementation were not changed by this revision.

### Document checks and handoff

- Sections 0-13 remain in order. UTF-8 without BOM and LF line endings
  are preserved. The earlier revision record remains an unchanged prefix.
- Content hashes of all 267 tracked/untracked read-only paths outside
  design.md and this record are unchanged, as are Git HEAD/index/refs.
- Exact edited files: .trellis/tasks/11-06-immutable-runtime-install/design.md
  and .trellis/tasks/11-06-immutable-runtime-install/review/design-revisions.md.
- No source/tests/Cargo/spec/parent/PRD/approval/plan edits, builds, tests,
  dependency changes, subagents, commits, pushes, or merges occurred.
- This is a proposed amended design, not a new review verdict, fresh
  design lock, or implementation-readiness certification. approval.md
  remains withdrawn pending the owner-managed review/plan/lock sequence.
