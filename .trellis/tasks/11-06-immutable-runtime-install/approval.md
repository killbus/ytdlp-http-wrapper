# Approval -- 11-06-immutable-runtime-install

Single-runtime design locked for implementation on 2026-11-07.

## Current status

- status: approved-for-implementation
- artifact: .trellis/tasks/11-06-immutable-runtime-install/design.md
- design_sha256: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59
- source_baseline: 775bd5fa974f1008bb24e71ea92f674515706948
- review: review/design-findings.json, single-runtime cycle round 1, PASS;
  D1-D8 met, no blocking findings, and actual candidate hash verified.
- Requirement approval: one fixed `runtime/` installation; reuse when valid
  and matching, otherwise prepare a verified replacement; no version history.
- Cleanup: this invocation's stopped, owned staging can be reclaimed without
  a completed manifest. The previous A7 manifest prerequisite is superseded.
- Authority: amended prd.md and review/owner-decisions.md (S1-S5).
- Owner explicitly accepted A8/A9 after their deployment/availability costs
  were explained. S5 supersedes the pending-confirmation wording for those
  assumptions without changing the reviewed content hash. A7 is retired.
  A1-A6 retain their prior production-rollout confirmation requirement.
- Resume using the updated implement.md and implementer-brief.md. The current
  dirty implementation is an earlier prototype, not acceptance evidence.
- Round 4 PASS applies only to
  3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC;
  it is not review or approval of the new single-installation design.
- This is a worktree-content lock, not a claim that docs/code are committed.
  No role agent may commit/push. Preserve the frozen input hashes during
  implementation; the owner handles delivery after code review. Staging must
  remain scoped to task-owned paths at that later delivery step.
- No deployment, live-root cleanup or live-source validation is authorized.
  Native Linux/Windows evidence and AC1's owner delivery-commit history check
  remain required before final acceptance.

## Historical approval (superseded)

The record below is retained for traceability, not as an active lock.

Owner amendment 2026-11-07: "zero new Cargo dependencies" was clarified
to mean "no new third-party yt-dlp dependency", not a ban on
general-purpose crates. design.md section 0 records the amendment:
`sha2`/`zip` are direct dependencies, hand-written SHA-256/DEFLATE/ZIP
code is removed, `integrity.json` is removed, retained archives under
`artifacts/` are the reuse integrity source, and the worker script hash
participates in `runtime-id`. The pre-amendment review verdicts below
apply to the prior revision; the amended design is covered by the
implementation tests and is pending a fresh design-review pass.

- artifact: .trellis/tasks/11-06-immutable-runtime-install/design.md
- historical_design_sha256: BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D
- review: review/design-findings.json round 2, verdict PASS (D1-D8 all met)
- round 1: G1 (absent-checksum fail-closed for the no-plan PBS path;
  plan sha256 required) and G2 (A6 inline marker) -- both closed and
  re-verified by re-reading the revised sections.
- round 2 observation N1 (error-table row for missing SHA2-256SUMS)
  resolved by owner edit before locking (section 10 row now names the
  API-no-sha256 case; sums-file-missing stays RUNTIME_INTEGRITY_FAILURE
  per section 5 source 1).
- assumptions pending owner confirmation (do not block; confirm before
  promoting this unit's runtime defaults): A1 (PBS tag 20261026 /
  CPython 3.13.18), A2 (API sha256 trust anchor when no plan file),
  A3 (512 MiB cap / 300 s / 3 attempts), A4 (system tar present),
  A5 (default --runtime-yt-dlp-version 2026.09.23), A6 (pinned wheel +
  ZIP extraction suffices; no hard runtime deps).
- layout note: bin/ added to parent design section 6 layout (additive
  deviation, flagged in design.md section 4).

Locked at: 2026-11-06 (Asia/Shanghai).
