# Design Reviser brief -- 11-06-immutable-runtime-install (revision of round-3 findings)

THIS DOCUMENT IS SELF-CONTAINED. You are the Design Reviser for this unit.
Your role and rules are inlined below (they are the Paseo profile
"Design Reviser" notes, materialized to disk per the dispatch SOP R1/R2).
Do not look for a separate profile or hidden role file.

## Your role (verbatim)

You revise the design. You cannot ask the user questions; you only close the
gaps listed by the Design Reviewer.

## Your inputs

- `.trellis/tasks/11-06-immutable-runtime-install/review/design-findings.json`
  -- the round-3 findings (verdict FAIL, blocking G1-G5). Close them one by one.
- Read-only yardsticks you may quote from: `prd.md`,
  `.trellis/spec/`, the baseline commit `c08f398`, the parent design
  `.trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md`.

## Rules (binding)

- You may modify ONLY
  `.trellis/tasks/11-06-immutable-runtime-install/design.md`, and you may write
  files under `.trellis/tasks/11-06-immutable-runtime-install/review/`.
  You must NOT modify prd.md, approval.md, .trellis/spec/, any source file, or
  any git state.
- Every addition must come from prd.md, .trellis/spec/ or the baseline. A
  design decision you have to invent must be marked `[ASSUMPTION An]` inline at
  the point where it bites, and listed in design.md section 12. Never present
  an assumption as an established fact.
- Do the minimal addition per finding. Do not widen the design, do not rewrite
  sections that were not flagged, do not restructure numbering (sections 0-13
  stay as they are).
- Do NOT re-open the owner's amendment in design.md section 0. The amended
  dependency rule (`sha2`/`zip` are legitimate direct deps; no hand-written
  crypto/compression/archive code) is owner-authored and stays as written.
- If a gap can only be closed by a user decision (prd.md itself is ambiguous),
  do not guess: write `.trellis/tasks/11-06-immutable-runtime-install/review/open-questions.md`
  with the question and stop.
- No git commit / push / merge.
- Keep the document internally consistent: after your edits, no two sections may
  state different rules for the same condition.

## Context you need

- The design is already implemented in the worktree (uncommitted; gates green).
  Do not design to the implementation and do not read it as authority, but
  prefer reconciliation that does not force gratuitous churn: when a body clause
  and the section 10 error table disagree, prefer the code names the table
  already defines unless a prd.md / spec / parent-design clause decides
  otherwise, and say in your report which side you moved.
- The owner locks the design: after your edits, approval.md's design_sha256 is
  stale. Do not touch approval.md; report the new sha256 so the owner can
  re-lock.
## The five blocking findings to close (round 3)

Close each one. The finding ids, criteria and locators are as the reviewer
wrote them in review/design-findings.json; re-read that file for the exact
wording before editing.

### G1 (D7) -- staging cleanup boundary vs the inherited yardstick

Locators: design.md section 4 staging layout (lines 127-130) and section 8
steps 5, 8 and 9 (lines 368-382); conflicting yardsticks: prd.md requirement 3
("cleanup only touches owned, manifest-validated paths") and parent design
`.trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md` section 6
(lines 194-206).

Gap: section 8 step 5 lets the first boot sweep any older `staging/<*>`
directory on the strength of the `RUNTIME_STAGING_OWNER` marker alone, but the
manifest is only written later, so a partial staging directory has no
manifest-validated ownership. Step 9 also supports two concurrent boots, and
the sweep says nothing about how a crashed boot's marked directory is told
apart from another live boot's marked directory. An implementation that follows
the marker-only rule can delete a path the inherited boundary protects and can
interfere with the publication race the design supports.

Landing point: after the edit, section 8 must specify the ownership validation
and the stale-versus-live determination that precede any staging removal, and a
marker alone must not be able to authorize deletion of an unvalidated or
concurrently active directory. If you intend a deviation from prd requirement 3
or parent section 6, state it and justify it explicitly. Any new mechanism you
have to invent (a lock/lease rule, a boot identity, an age bound) is an
assumption: mark it inline and add it to section 12.

### G2 (D6) -- no offline acceptance check for the AC3 corruption case

Locators: section 11 AC3 rows (lines 465, 467) versus section 8 step 4
(lines 356-363); yardsticks: prd.md AC3 and requirement 8.

Gap: neither AC3 row exercises rejection of a corrupt or manifest-mismatched
existing `versions/<id>`. The wrong-plan-hash row is an initial-install
failure, and the changed-artifact-content row derives a different id and so
never enters the reuse check. Both rows can pass even if reuse never validates
the existing manifest or the retained archives, so the amended
reuse-integrity policy has no offline acceptance check for the central AC3
condition or for leaving the rejected directory unchanged.

Landing point: section 11 must gain explicit offline assertions that (a) first
install a valid runtime, then (b) corrupt a retained archive and, separately,
mismatch the manifest, then (c) retry with the original plan and the expected
id. Specify the required structured rejection and a before/after comparison
proving the existing directory is neither repaired nor mutated, and show that
the retry reaches section 8 step 4 instead of deriving a different id. Do not
require re-hashing the extracted Python tree.

### G3 (D4) -- failure conditions have no single authoritative outcome

Locators: section 5 source 3 and download mechanics (lines 249-279); section 7
worker-version row (line 339); section 8 retry paragraph (lines 387-389);
section 10 error table (lines 434-440). Yardsticks: prd.md requirements 4 and 6
(actionable structured failures, bounded retry).

Gap: section 5 gives a missing curl RUNTIME_DOWNLOAD_FAILURE while the section
10 table gives RUNTIME_CURL_MISSING; section 5 gives a missing tar
RUNTIME_EXTRACT_FAILURE while the table folds tar into RUNTIME_CURL_MISSING; and
a worker-version mismatch is claimed by both RUNTIME_CONFIG_INVALID and
RUNTIME_VERSION_MISMATCH in the table while section 7 selects exactly one. The
curl/tar conflict also changes retry behaviour, because section 8 retries only
download-class errors. In addition, the amended wheel policy makes the archive
SHA256 the integrity anchor, but section 5 source 3 only rejects zero or several
wheels; it does not say what happens for exactly one py3-none-any wheel whose
`digests.sha256` is absent or unusable, and the table does not resolve it.

Landing point: after the edit, each of missing curl, missing tar and
worker-version mismatch has exactly one code and one retry classification, the
body and the table agree, and there is an explicit fail-closed rule for an
absent or unusable wheel digest. Every absent-checksum path (binary after API
and SUMS are exhausted, PBS, wheel, plan) must have exactly one fail-closed
outcome and no conflicting table row.

### G4 (D6) -- AC1's "one commit" requirement has no landing point

Locators: section 2 Cargo and boot change rows (lines 85-90) and section 11
AC1 row (line 468); yardsticks: prd.md requirement 1 and AC1.

Gap: the AC1 landing point checks only the resulting dependency graph and
Cargo.toml text, so it cannot falsify prd AC1's "the change and the installer
ship in one commit". A history that removes the old installer in one commit and
wires the replacement in another satisfies every check currently listed.

Landing point: extend the AC1 landing point with an offline commit-level
procedure and explicit assertions that bind the installer module, the boot
replacement, the LibraryInstaller removal, the Cargo.toml dependency and patch
removal and the matching lockfile change to one commit. State how a split-commit
delivery fails acceptance even though the final tree passes the metadata
checks. The owner performs the commit, so say who runs this procedure and at
what point in the loop.

### G5 (D4) -- the A6 inline marker was lost

Locators: section 5 wheel/RECORD paragraphs (lines 271-279) and section 12 A6
(lines 489-492); yardsticks: prd.md requirements 5-7 plus the D4 rule that a
decision with no prd/spec source must be marked.

Gap: section 12 lists exactly A1-A6, but the amended section 5 RECORD paragraph
carries no inline `[ASSUMPTION A6]` marker; A1-A5 each appear inline once and A6
appears zero times. The claim that one pinned wheel plus ZIP extraction supplies
the whole resident Python closure is not guaranteed by the prd, and its
assumption status is no longer marked where the implementation relies on it.

Landing point: restore exactly one inline `[ASSUMPTION A6]` marker in the
section 5 wheel/RECORD paragraph that relies on the pinned wheel plus ZIP
closure, keeping the amended rule that RECORD parsing is not reimplemented.
After the edit each of A1-A6 appears inline exactly once and matches section 12.

## Output contract

1. Edit `.trellis/tasks/11-06-immutable-runtime-install/design.md` in place.
   Keep sections 0-13 in their current order and numbering, keep the existing
   line-ending convention and the existing wrapping style.
2. Append a revision record to
   `.trellis/tasks/11-06-immutable-runtime-install/review/design-revisions.md`
   (create it if absent): one entry per finding id, naming the sections touched
   and what was added, plus every new `[ASSUMPTION An]` with its section 12
   entry.
3. Do not run cargo or the test suites. You may read sources and baseline blobs
   to check a citation. In this shell `git diff` / `git log` stdout may be
   truncated by a wrapper; if a command misbehaves set
   `$env:CLAUDE_CONFIG_DIR='C:\Users\ying\.claude'` first, or redirect to a file
   and read it.
4. Reply with: the new sha256 of design.md (compute it with
   `(Get-FileHash .trellis/tasks/11-06-immutable-runtime-install/design.md -Algorithm SHA256).Hash`),
   the list of sections touched, whether you had to write open-questions.md, and
   one line confirming no other file changed. No verdict language (no PASS/FAIL).