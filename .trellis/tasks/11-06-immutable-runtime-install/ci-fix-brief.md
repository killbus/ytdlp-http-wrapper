# CI repair brief -- round 2, F1 only

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Base commit: 775bd5fa974f1008bb24e71ea92f674515706948
Locked installer design: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

## Role and inputs

You are the bounded Implementer, not a designer or acceptance reviewer. Repair
only the existing CI finding F1. No own PASS/FAIL verdict. All rules and schema
are explicit here; no hidden profile notes are required. Use the owner's
temporary gpt-6-astra override with auto-review/xhigh. Implement directly;
native Trellis helpers are absent, no nested agents or helper installation.

Read task ci-implementation-brief.md, ci-reviewer-brief.md, approval.md,
design.md section 11 and applicable .trellis/spec/wrapper/backend guidelines.
Read review/ci-findings.json, ci-findings-r1.json, ci-responses-r1.json,
ci-progress.md, ci-start.json, ci-review-start.json and ci-fix-start.json.
Read the actual script/test plus unchanged workflow/Compose/Dockerfile/smoke
handoffs, not just the reviewer summary. Existing installer findings/responses
are a separate completed review and remain immutable.

Missing approval/design -> write review/ci-blocked.md and stop. Hash mismatch,
active CI blocked/spec-gap marker or unexpected concurrent change -> stop/report.
Recorded worktree dirtiness is expected; never reset/revert it. Snapshot hashes
are the explicit lock, not a claim the worktree was clean at BASE.

## F1 and repair boundary

F1: scripts/ci-container.sh uses unsupported `docker compose run --no-build`,
preventing the required container-smoke gate. The permissive Docker logging
stub and its expected argv wrongly accept and require this flag.
Applicable clauses: ci-implementation-brief.md plan items 5 and 6 (preserved
Compose assertions and existing checks). Verify the reachable path from
.github/workflows/ci.yml docker job -> script -> Compose run -> smoke stdin.

Reviewer cited actual interfaces:
- https://docs.docker.com/reference/cli/docker/compose/run/
- https://github.com/docker/compose/blob/v2.40.3/cmd/compose/run.go#L225
- https://github.com/docker/compose/blob/v2.40.3/cmd/compose/create.go#L79
`--no-build` belongs to create, not run. Source/documentation evidence is not
a claim of a local Docker run. Docker is currently absent from this host PATH.

Make the minimal correction to supported run options. Preserve the already
loaded local image, --pull never, --no-deps, --rm, -T, /bin/sh entrypoint,
service arguments and exact unchanged scripts/container-smoke.sh stdin. Do
not skip the container gate, change Compose defaults, add an image fallback,
publish anything or start the production installer.

Correct the regression expectation and tighten the test's contract for the
actual Compose handoff so it rejects the unsupported option instead of blessing
any argv. Add focused regression coverage showing this invalid invocation is
rejected, the corrected invocation reaches unchanged smoke input, and a run/
assertion failure propagates nonzero. Use a minimal test-only contract guard,
not a Compose emulator or production abstraction. Tie supported argv to the
real CLI reference/source; a fake accepting your expected list is not proof of
real Docker execution. If the real installed CLI happens to be available,
version/run --help is allowed without daemon use; never install/download an
executable merely to obtain that check. Report unavailable native evidence.

Only code files writable:
- scripts/ci-container.sh
- scripts/tests/test_shell_ci.py

Everything else is read-only, including workflow, shared runner, other scripts,
README, production sources, Cargo files, all existing Rust/Python tests,
Dockerfile, compose.yaml, frozen specs/design/PRD/approval/owner records, briefs
and snapshots. No style refactor, unrelated fixes or new framework. If scope
cannot repair F1, explain the precise conflict in review/ci-spec-gap.md and
stop instead of silently broadening the edit set or changing the yardstick.

## Validation and reporting

Run lightweight focused shell-orchestration tests then
`python scripts/ci.py script-tests`; inspect syntax and scoped diff checks.
Record exact commands/exits/test counts. Where practical prove the revised
regression rejects the pre-fix invocation using an isolated temporary copy,
not by restoring/reverting the workspace script. Do not loosen assertions.

No real Docker daemon/build, live downloads, hosted CI, remote host, full Rust
suite/build, deployment, runtime cleanup or Git stage/commit/push/merge/reset/
revert. Obey sandbox approvals; no permission bypass. Isolated temporary test
roots only; on Windows verify absolute containment, stop writers, use literal
paths and one shell for delete/move. No visible background helper windows.
Use bounded reads, coherent edits, JSON UTF-8 without BOM and concise output.

Check the ci-fix-start.json immutable hashes at the end. Append only a round-2
section to review/ci-progress.md, and overwrite review/ci-responses.json.
Out-of-scope real issues may be appended to review/ci-follow-ups.md; do not fix.
Do not modify ci-findings.json or any archives. CI round = finding round + 1 = 2;
do not use the installer review round. Respond to F1 with exactly one of:
ACCEPTED (minimal fix), REBUTTED (verified production-unreachability or clause
evidence), SPEC_CONFLICT (gap report and stop). Tests-green is not a rebuttal.
Coordinator expects ACCEPTED given the concrete CLI-contract evidence, but
report any genuine counterevidence rather than pretending agreement.

Full response schema:
```json
{
  "round": 2,
  "scope_summary": "F1 repair and precisely excluded work",
  "changes": [{"file": "", "intent": "", "spec_clause": "ci-implementation-brief.md plan items 5-6"}],
  "test_notes": "Personally executed commands/exits and limits of stub versus real Docker evidence",
  "finding_responses": [
    {"id": "F1", "response": "ACCEPTED | REBUTTED | SPEC_CONFLICT", "action_or_evidence": ""}
  ]
}
```

Stop for the same Reviewer's round-2 closure check; no self-acceptance. Future
unresolved third-round disagreements escalate instead of an endless repair
loop. Native Linux/hosted CI/official smoke/Docker/release/rollout/history
evidence remains pending. Final reply only two or three factual progress
sentences, no verdict or claim of real Docker validation.
