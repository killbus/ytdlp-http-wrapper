# Implementer brief -- single runtime, implementation round 1

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Branch: feature/resident-worker-runtime-upgrade
Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948
Design SHA256: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

This is self-contained: role, ownership, inputs and response schema are below.
There is no hidden profile document. Follow the stage named in your dispatch.

## Role and admission

Implement inside the approved design; you are not its designer or acceptance
reviewer. No PASS/FAIL verdict about your own work. Design review approved this
hash; the user accepted A8/A9 in review/owner-decisions.md S5. That external
record supersedes ONLY A8/A9 pending-confirmation wording without editing the
reviewed document. A1-A6 are not newly approved production defaults.

Read approval.md, design.md sections 0-13 in windows, prd.md, implement.md,
owner-decisions S1-S5, wrapper backend specs and applicable entry-point sources.
Verify actual design SHA256. Missing approval/design -> review/blocked.md;
wrong hash or existing active blocked/spec-gap -> stop and report. Do not use
the old prototype or historical design verdicts as the new yardstick.

## Ownership rules (binding for you and any implementation child)

- prd.md, design.md, implement.md, approval.md, all briefs, owner-decisions.md,
  snapshots, parent documents and all .trellis/spec/ files are read-only: no
  edits/additions/deletions, including small clarifications.
- If implementation conflicts with design, the design is incomplete or not
  feasible, stop that part and write review/spec-gap.md with clause, problem
  and blocked work. Do not silently choose policy or change the yardstick.
- Module boundaries beat green tests. No public testing hooks, exposed internal
  state, production test-only branches or cross-layer bypasses. Section 11
  permits PRIVATE module-local cfg(test) error injection, not runtime toggles
  or new public/pluggable updater abstractions.
- Lint/typecheck/test success is mechanical/behavioral evidence, not proof of
  architecture conformance or a rebuttal to a clause-based finding.
- Out-of-scope existing issues: do not fix/refactor; append a located observation
  to review/follow-ups.md. No imagined edge cases or future infrastructure.
- No git commit/push/merge/staging/reset/revert; preserve existing dirty work.

## Initial assignment and starting state

Stage 1 ONLY of implement.md. Allowed code edits: src/runtime_installer.rs and
 tests/runtime_install.rs (module-local tests may be in the installer). Do not
edit other source/tests/Cargo/docs in this stage. Later stages are separate.

The worktree is intentionally dirty. The installer and its integration tests
are untracked prototype work; Cargo.toml/lock, src/lib.rs/main.rs/backend.rs
already contain dependency removal and wiring changes. Preserve them. Task
planning records and unrelated 11-03 task.json were dirty before this dispatch.
review/implementation-start.json records starting hashes and dirty paths.
Do not stop merely on recorded dirtiness; stop and ask the coordinator about
genuinely unexpected concurrent changes. Old responses are archived as
review/responses-before-single-runtime.json; they do not prove new completion.

Stage 1 removes versions/<id> selection, cross-boot sweeping, marker-only
cleanup authority, corruption reject-only behavior and loser-retry publication.
Replace with the fixed runtime/full-identity flow and accepted A8/A9. Do not
reintroduce backups/history, lock/lease systems or old-runtime fallback.

Read before patching. Preserve sha2/zip and unrelated resolver code; do not
delete the installer file in order to rewrite it later. Work in coherent small
edits. Use isolated fixture roots only, never real installed runtime cleanup.

## Execution discipline

- The role normally uses a trellis-implement child. If that native helper is
  actually available, use one bounded child with first prompt line
  `Active task: .trellis/tasks/11-06-immutable-runtime-install` and the ownership
  rules copied verbatim. No parallel writers or recursive role loops.
  This checkout currently has no .agents/ or .codex/ helpers: if unavailable,
  perform the stage yourself, state the fallback, and do not spawn extra Paseo
  agents or install infrastructure to simulate a helper.
- No Trellis planning/brainstorm/update-spec/finish-work. If using trellis-check,
  limit it to lint/type checks and those repairs; no style/bug review or edits
  to read-only inputs. Only the coordinator dispatches the acceptance reviewer.
- Use rg, read 50-100 line windows, concise output. Save coherent checkpoints
  on context pressure, report outstanding work; never imply half-written work
  is complete. Do not wait on other agents by sleeping/polling.
- Record commands and exits. Stage 1 uses fmt plus focused installer integration
  and module-local tests. Old full-suite results do not validate new code.
- Use PowerShell literal paths for file operations, check resolved deletion
  containment, do not compose deletion across shells. Follow sandbox policies;
  seek escalation for denied cache/network writes, never bypass via other tools.
- Write JSON as UTF-8 without BOM. Do not stage or clean generated caches.
  Follow RTK.md: rtk proxy only when normal output is unusable. Do not equate a
  clipped log with proof of its omitted contents.

## Review responses and report contract

No review/findings.json (CODE review) exists at dispatch. design-findings.json
is a different cycle: do not derive implementation round from it. For later
code findings respond to EACH with ACCEPTED (minimal fix), REBUTTED (read
production-unreachability call-chain evidence or applicable clause evidence),
or SPEC_CONFLICT (spec-gap.md and stop that part). Tests passing is no rebuttal.

Only writable task reports: append review/implement-progress.md; overwrite
review/responses.json; write review/blocked.md or spec-gap.md as needed; append
review/follow-ups.md for real out-of-scope observations. Do not modify findings,
owner/revision records, input snapshots or lock. No self-verdict language.

Use exactly this responses.json shape for this implementation round:
```json
{
  "round": 1,
  "scope_summary": "Stage completed, non-goals excluded, remaining stages explicit",
  "changes": [{"file": "", "intent": "", "spec_clause": "design.md section x"}],
  "test_notes": "Actual behaviors, commands/exits, OS limits and missing coverage",
  "finding_responses": []
}
```
Later rounds use code findings.round + 1 and finding_responses objects with
id, response, action_or_evidence. Current round=1, empty finding_responses.
Append Stage 1 report to implement-progress.md and reply briefly with changed
files, focused gates and remaining Stage 2/3 work. Stop after Stage 1.
