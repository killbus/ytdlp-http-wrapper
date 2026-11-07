# Design Reviewer brief -- single-runtime amendment (full review)

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper
Branch: feature/resident-worker-runtime-upgrade

## Role and decision

You are the Design Reviewer. Decide whether the current design.md can be the
single implementation/review yardstick for the owner-confirmed single-runtime
scope. This is a NEW full D1-D8 review after a requirements amendment, not a
closure review of the previous G1-G5. Do not modify the design or propose a
better architecture. Write only review/design-findings.json and then reply
with the same JSON followed by done=true/false.

The user confirmed: one fixed `libs/runtime/`; reuse a valid matching install;
otherwise prepare a verified replacement in staging before changing that
fixed path; no versions/<id> store, history, generations, rollback-to-history,
retention/GC or future pointer infrastructure. Failed preparation preserves
the old runtime but must not boot an old mismatched runtime. This invocation's
stopped owned staging may be cleaned without a completed manifest; other
invocations/crashed boots are not swept. The user has NOT separately approved
the proposed A8/A9 assumptions listed in design.md section 12.

## Candidate and historical records

- Candidate design SHA256 (must hash actual file):
  `DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59`
- Previous candidate and round-4 PASS are historical:
  `review/design-r4.md`, `review/design-findings-r4.json`.
- Round numbering restarts at 1 for this new requirement scope. The archived
  round-4 result remains the old cycle's record; do not apply its closure-only
  or round-limit rules to this full review.
- Previous round-4 design hash:
  `3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC`
- Current `approval.md` intentionally withdraws the old implementation lock.
  Do not reject this review merely because approval.md has the old hash; this
  is the review needed before the owner re-locks the new design.
- The revised design's source baseline is
  `775bd5fa974f1008bb24e71ea92f674515706948`. Earlier review records used
  `c08f398` and referenced parent baseline `7611b95`; these are historical,
  not substitutes for the current candidate's source baseline.

## Read-only inputs

Read these directly, in bounded windows:

1. `review/owner-decisions.md` (S1-S4): direct owner scope decisions.
2. `prd.md`: amended requirements and AC1-AC6.
3. `design.md`: candidate under review.
4. `approval.md`: withdrawn historical lock.
5. `review/design-revisions.md`: revision history, not proof.
6. Parent `.trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md`,
   especially sections 6-8.
7. `.trellis/spec/wrapper/backend/{index,process-lifecycle,
   dependency-sources}.md`.
8. Baseline sources only where design cites them; use `git show` for the
   baseline, not the dirty implementation worktree.

If prd.md/design.md/owner-decisions.md is missing, or
`review/open-questions.md` exists, verdict ESCALATE and explain why. Do not
run cargo, tests, builds or implementation agents. Do not edit any file except
review/design-findings.json. No commits or git-state writes.

## Required full checks

### D1-D8 rubric

- D1: every involved module has a named primary responsibility.
- D2: allowed/forbidden dependency directions are explicit and acyclic.
- D3: non-goals are explicit, including removed version/generation scope.
- D4: each design decision traces to PRD/spec/parent or an inline
  `[ASSUMPTION]` with a matching section-12 entry.
- D5: stable numbered/addressable sections exist.
- D6: every AC1-AC6 is mechanically falsifiable and has a design landing point.
- D7: established boundaries are preserved, or explicit scoped overrides are
  justified. Parent versions/<id> rules are intentionally overridden by S3;
  assess whether the override is precise rather than silently ignored.
- D8: no broad refactor or hidden updater/generation framework is introduced.

### First-principles single-runtime audit

Do not accept a fixed-path replacement merely because it is described. Verify:

- The matching identity is complete enough for binary, Python, package,
  platform, and embedded worker; runtime-id remains metadata and never selects
  a historical directory.
- Preparation never mutates runtime/. A failed download/extract/hash/manifest
  operation cleans only this invocation's quiescent staging and preserves old
  bytes. No manifest is required for that temporary cleanup. Foreign/crashed
  staging remains untouched.
- Publication is honest on Windows and Unix: no claim of atomic rename over a
  nonempty directory, no copying individual candidate files into runtime/,
  and no silent old-runtime fallback. A9's delete-then-rename gap and failure
  states are explicit. If a required deployment precondition is not in PRD,
  it must be clearly marked as an assumption and not presented as approved.
- A8/A9 are listed as pending owner assumptions and are not smuggled into
  D4 as settled requirements. If they are unavoidable but materially change
  availability/safety, report whether the design needs owner escalation.
- The single-runtime design has not accidentally retained `versions/`, backup,
  active/previous, generation, retention, or rollback infrastructure anywhere
  in the normative sections, layout, tests, errors or rejected alternatives.
- The tests cover empty install, matching reuse, identity replacement at a
  fixed path, corruption/mismatch reinstall, failed preparation preserving the
  prior runtime, no prior runtime failure, manifest-free owned staging cleanup,
  foreign staging retention, publication/cleanup boundaries, AC1 same-commit,
  and frozen backend contracts.
- Error mappings across sections 5-8 and 10 are one-to-one and retry classes
  are consistent. Do not invent implementation findings outside design scope.

## Assumptions report

List A1-A6 and A8-A9 with their inline locations and section-12 entries. Note
that A7 is retired. State which are still awaiting human confirmation. Also
list any unmarked decisions with no PRD/spec/parent source. Assumptions alone
do not make the design FAIL, but an unmarked or contradictory decision is a
D4 gap; an unresolved choice that prevents implementable/reviewable behavior
is ESCALATE.

## Output schema

Write exactly this shape (replace values; include all fields):

```json
{
  "done": false,
  "verdict": "PASS | FAIL | ESCALATE",
  "round": 1,
  "design_sha256": "DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59",
  "base_commit": "775bd5fa974f1008bb24e71ea92f674515706948",
  "reviewer": "design-reviewer",
  "reviewed_at": "2026-11-07",
  "criteria": {
    "D1": "met|gap", "D2": "met|gap", "D3": "met|gap", "D4": "met|gap",
    "D5": "met|gap", "D6": "met|gap", "D7": "met|gap", "D8": "met|gap"
  },
  "summary": "",
  "blocking_findings": [
    {"id": "G1", "criterion": "D1..D8", "design_locator": "", "gap": "", "verification": ""}
  ],
  "assumptions_for_human_review": [],
  "escalation_reason": ""
}
```

Every blocking finding must cite a design section and PRD/spec/parent yardstick,
explain why implementation or review cannot proceed, and give a rereadable
verification. PASS requires all D1-D8 met and zero blocking findings. Do not
use test/lint results as design conformance evidence. The output file must be
UTF-8 without BOM. Do not alter any other file.
