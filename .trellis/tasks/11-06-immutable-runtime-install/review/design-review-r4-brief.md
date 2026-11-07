# Design Reviewer -- round 4 closure review

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper
Branch: feature/resident-worker-runtime-upgrade

## Role and scope

You are the Design Reviewer. Decide whether the revised design can serve as
the implementation and review yardstick. Verify facts; do not design fixes.
This brief is self-contained and supersedes the round-3 brief for this turn.

Review only round-3 G1-G5 closure and new violations caused by their fixes.
Do not reopen unrelated decisions or start a new full review. Round 3 was the
full review of the owner-amended revision; this is its closure pass. If any
blocking gap remains, use ESCALATE with the unresolved conflict explained;
do not start another automatic revision loop.

## Candidate and approval status

- Previous candidate SHA256:
  BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D
- Revised candidate SHA256 (hash the actual file):
  3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC
- Round-3 source baseline remains c08f398 for continuity with those findings.
  The task design names parent baseline 7611b95; distinguish these references
  rather than silently substituting one for the other.
- approval.md still records the previous candidate, deliberately. This is a
  pre-lock review of a revised document, not an implementation admission check.
  Do not reject this review solely because the old approval hash differs.
  If the actual candidate differs from the revised hash above, ESCALATE.
- The design is NOT reapproved for implementation by this dispatch. Do not
  change approval.md or imply owner approval of the new A7 assumption.

## Inputs (read-only)

All task-relative paths below are under the active task directory.

1. review/design-findings.json -- your round-3 findings; read before replacing.
2. review/design-revisions.md -- the reviser's account, NOT closure evidence.
3. design.md -- directly re-read the revised clauses to verify each finding.
4. prd.md -- user intent, requirements, acceptance criteria and scope.
5. approval.md -- historical lock, not approval of the new revision.
6. review/open-questions.md, if present -- ESCALATE and quote the unresolved
   question; do not guess user intent.
7. .trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md, especially
   section 6, and .trellis/spec/wrapper/backend/{index,process-lifecycle,
   dependency-sources}.md -- inherited boundaries.

Missing PRD/design: ESCALATE. Only cite files and lines actually read.
The existing implementation and green tests do not establish design validity.

## Closure checks

- G1 / D7: sections 4, 8 and 12 must preserve owned, manifest-validated
  cleanup and not delete another active invocation's staging directory.
  The proposed A7 removes cross-boot sweeping, retains partial/unvalidated
  staging, and permits cleanup only for this invocation after writers stop.
  Check the policy consistently covers failure and publication-race paths.
  A7 is pending owner confirmation: assess traceability and consistency, list
  the retention tradeoff, and do not treat it as owner-approved.
- G2 / D6: section 11 must exercise retained-archive corruption and manifest
  mismatch after a valid install, retry the unchanged plan at the same id,
  and verify structured rejection without modifying that existing directory.
  Check the proposed observations are feasible without introducing production
  test-only hooks or contradicting module ownership; do not demand implementation.
- G3 / D4: sections 5-8 and 10 must agree on missing curl, missing tar,
  worker-version mismatch, absent/unusable checksums and retry classes.
  Assess semantic consistency, not naming preference. In particular verify
  the newly specified metadata-transfer/SUMS distinctions have no new conflict.
- G4 / D6: section 11 must contain an executable offline procedure to reject
  split-commit delivery of installer introduction, sole boot replacement,
  dependency/patch removal and matching lockfile changes. Check ordering of
  owner commit and final acceptance. No delivery commit exists yet; this review
  verifies the procedure, not that a future commit already passed it.
- G5 / D4: exactly one inline A6 marker at the wheel-closure claim, matching
  section 12; no reversal of the section 0 prohibition on hand-written RECORD
  parsing. Check A1-A6 remain intact and new A7 is explicitly marked/listed.

## D1-D8 rubric

- D1: explicit module boundaries; one principal responsibility per module.
- D2: explicit allowed and forbidden dependency directions; no cycles.
- D3: explicit non-goals.
- D4: decisions trace to PRD, project spec, or explicit [ASSUMPTION] markers.
- D5: stable, addressable section headings or numbers.
- D6: mechanically falsifiable acceptance criteria with design landing points.
- D7: consistency with established boundaries; deliberate changes stated and
  justified, not silently substituted.
- D8: no broad refactor beyond task scope.

Only gaps preventing downstream implementation/review are blocking. No style
opinions, alternative designs, speculative future needs, or unrelated findings.
An assumptions list alone does not decide the verdict. Pending assumptions are
not user approvals; unresolved PRD ambiguity still requires escalation.

## Allowed actions and output

Write ONLY review/design-findings.json under this task. No changes to design,
PRD, approval, source, other review artifacts, or git state. No commits, builds,
Cargo commands, test suites, or subordinate agents. Use rg to locate clauses
and read bounded windows. Write UTF-8 without BOM.

Use this complete JSON shape (replace example values, retain every field):

```json
{
  "done": false,
  "verdict": "PASS | ESCALATE",
  "round": 4,
  "design_sha256": "3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC",
  "base_commit": "c08f398",
  "reviewer": "design-reviewer",
  "reviewed_at": "2026-11-07",
  "criteria": {
    "D1": "met|gap", "D2": "met|gap", "D3": "met|gap", "D4": "met|gap",
    "D5": "met|gap", "D6": "met|gap", "D7": "met|gap", "D8": "met|gap"
  },
  "summary": "",
  "previous_findings_status": [
    {"id": "G1", "status": "closed|open|withdrawn", "verification": ""}
  ],
  "blocking_findings": [
    {"id": "G1", "criterion": "D7", "design_locator": "", "gap": "", "verification": ""}
  ],
  "assumptions_for_human_review": [],
  "escalation_reason": ""
}
```

Include a previous_findings_status entry for EACH of G1-G5 with directly read
clause evidence. Keep unresolved finding IDs; new fix-introduced gaps use G6+.
done=true only for PASS, all criteria met and no blocking findings. ESCALATE
requires done=false and a nonempty escalation_reason. A design PASS is not code
acceptance or approval of A7. Reply with the same JSON followed by done=true or
done=false. Do not cite unrun commands as evidence.
