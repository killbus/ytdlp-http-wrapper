# Implementer brief
Working directory: D:/Temp/ytdlp-resident-ejs-delivery
Active task: .trellis/tasks/11-08-resident-ejs-dependencies
BASE_COMMIT: read approval.md (the planning-lock commit; git HEAD at dispatch).
Role: implement the locked design, not judge it or redesign it. Do not declare
PASS/FAIL for your own implementation. The complete role is in THIS brief, not
in invisible profile notes. Model override: codex/remote-magpie/auto/gpt-6-astra.

Read implement.jsonl -> prd.md -> design.md -> implement.md -> approval.md. Missing
design/approval: write review/blocked.md and stop. Confirm design hash. Then read
focused relevant code before edits. You are the implementation subagent: perform
the work directly, no nested trellis agents or other delegation. No planning,
brainstorm, update-spec, finish-work, staging, commit, push, merge or deployment.

Ownership rules (binding):
- prd.md, design.md, implement.md, approval.md and all .trellis/spec files are
  read-only. Never edit/add/delete them, even for clarification.
- A design conflict/omission/impossibility must be written to review/spec-gap.md
  with exact clause, problem and blocked work; stop that part, do not route around.
- Module boundaries beat tests: no production test-only branches, reverse
  dependencies, exposed internals or alternate installer. Lint/tests aren't
  architectural proof. No invented hardening or future extension framework.
- Out-of-scope existing issues go only to review/follow-ups.md, not code.
- Only implementation-owned files: src/runtime_installer.rs,
  worker/probe_environment.py, tests/fixtures/managed_runtime.rs,
  tests/runtime_install.rs, src/main.rs TEST MODULE ONLY if a shared fixture caller
  needs updating, scripts/runtime-boot-smoke.py, focused new scripts/runtime-ejs-smoke.py,
  scripts/tests/test_runtime_boot_smoke.py, focused new scripts/tests/test_runtime_ejs_smoke.py,
  README.md, scripts/README.md, and your review/responses.json,
  review/implement-progress.md or permitted blocked/spec-gap/follow-ups reports.
  If another file is necessary, report before modifying it.
- Keep binary curl argv/integrity tests, process lifecycle, request contract,
  worker/protocol/pool/supervisor, pins, Cargo dependencies and CI/release workflow
  unchanged. Keep old assertion meaning; do not relax failures to pass CI.

Stages:
1. Add shared exact default-extra requirement, recipe identity migration and
   required-resource Python probe; update separate EJS-wheel offline fixtures and
   affected callers; add migration/repair/failure/reuse regressions from design 6.
   Append progress and any unfinished scope before proceeding.
2. Add the real-image offline EJS/Deno script-load gate and focused negative unit
   tests, integrate with existing boot smoke and docs. Do not use live YouTube or
   download at request time. Clearly distinguish JS-load from challenge-solving.
3. Local cargo fmt --check (format only if needed), scoped git diff --check,
   lightweight Python script tests. NO cargo test/build/clippy, uv provisioning,
   Docker, network-dependent tests or raw cookie access locally; native/runtime
   evidence is exclusively hosted CI. Append exact commands/results and pending
   gates. Refine only your code for readability without broad refactors.
Read source with rg and 50-100 line windows; keep gate output to useful tails.

If review/findings.json exists, respond to every blocker: ACCEPTED minimal fix;
REBUTTED only with concrete reachability or clause counterevidence; SPEC_CONFLICT
requires spec-gap.md and no silent decision. Test success is not a rebuttal.
Each round overwrite review/responses.json with this exact structure:
{
  "round": 1,
  "scope_summary": "implemented scope and explicit exclusions",
  "changes": [{"file":"", "intent":"", "spec_clause":"design.md section N"}],
  "test_notes": "behavior checked, exact local evidence, hosted gaps",
  "finding_responses": [{"id":"F1", "response":"ACCEPTED | REBUTTED | SPEC_CONFLICT", "action_or_evidence":""}]
}
First round finding_responses is []; otherwise round = findings.round + 1.
Final response: only two or three sentences of factual progress, no self-verdict.
Stop after checkpoint for independent review.
