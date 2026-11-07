# CI validation follow-through -- thin workflow and reusable scripts

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Base commit: 775bd5fa974f1008bb24e71ea92f674515706948
Installer design hash: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

## Authority, role and inputs

The owner now requests repository-owned validation logic in scripts/ or ci/
and thin GitHub workflows invoking it, instead of choosing an ad-hoc remote
Linux host. This is an additive validation-delivery assignment in the existing
task, not an installer redesign or permission to change its frozen inputs.
The previous no-CI-edit boundary is superseded ONLY for the paths listed below.
The existing installer architecture review is PASS for its recorded files;
do not overwrite that review or pretend it already covers this CI extension.

You are the bounded Implementer. Implement directly (native Trellis helpers
are absent); do not spawn another agent, run planning/update-spec/finish-work,
or give your own implementation an acceptance verdict. No PASS/FAIL prose.
Temporary model override: remote-magpie/auto/gpt-6-astra, auto-review/xhigh.

Read this brief, task design sections 9-11, PRD AC1/AC5/AC6, approval.md,
review/owner-decisions.md, implement.md, review/acceptance-status.json,
review/implement-progress.md Stage 3 and review/ci-start.json. Read applicable
.trellis/spec/wrapper/backend/index.md, process-lifecycle.md and
dependency-sources.md, plus current scripts, .github/workflows/ci.yml,
rust-toolchain.toml, Cargo.toml, README, Dockerfile and compose.yaml.
Release/docker publishing workflows are read-only context, not this scope.

## Bounded implementation plan and contract

1. Extract verification policy from ci.yml into small repository scripts.
   Reuse scripts/ rather than introducing a parallel framework. Prefer one
   Python-standard-library runner for shared Windows/Linux Rust/Python gates
   (Python is already a test prerequisite), with simple Bash wrappers for
   Linux-only smoke/container orchestration. Existing scripts/ci.sh becomes
   a thin compatible wrapper to the shared runner, not a second command list.
   Use a small number of explicit commands; no plugin engine, generic task
   scheduler, workflow generator, custom actions or dependency-management layer.
2. Shared checks run fmt, locked all-target strict Clippy, locked Rust tests,
   and the Python worker suite, with a locked release-build entrypoint retained
   in CI. Before tests require a freshly built host executable and pass its
   absolute path as CARGO_BIN_EXE_ytdlp-http-wrapper through the child process
   environment: tests/resident_worker_pool.rs otherwise silently skips its
   boot test. Respect Cargo's target directory; do not use a stale guessed
   executable. Test fixtures stay offline; dependency acquisition for builds
   is separate and does not make --offline a fresh-runner default. Exit nonzero
   on the first failed command and on missing prerequisites. Use argument
   lists rather than shell-string construction in the cross-platform runner.
3. Preserve the existing Ubuntu/Windows matrix. Python must be explicitly
   available on both; tar and other required test tools must not silently skip.
   Prefer the repository rust-toolchain.toml as the toolchain source instead
   of drifting between an unrelated action stable pin and the local pin.
   Linux permission tests must run as an ordinary non-root user so denial
   behavior is meaningful; fail with an actionable precondition if necessary,
   rather than treating a privileged skip as real permission evidence.
4. Move the official Linux onefile preparation currently embedded in ci.yml
   into a dedicated script, retaining scripts/linux-smoke.sh as the no-download
   consumer of YTDLP_SMOKE_BINARY. Fetch one selected official release, verify
   the exact binary's required SHA256 before chmod/execution, then run the
   existing ignored smoke test. Keep this explicitly networked smoke separate
   from fixture gates and from the production runtime installer. No implicit
   reuse of stale files or unverified artifacts; no credentials in logs. Preserve
   current release-selection behavior unless an explicit override is needed
   for reproducibility; print the selected tag/hash. Never copy the production
   resolver or claim that this smoke verifies the A1-A6 runtime defaults.
5. Move CI container build/Compose verification orchestration into a small
   script reusing scripts/container-smoke.sh. Preserve existing buildx local
   image tag, Compose limits and non-root/tmpfs assertions. GitHub-specific
   cache options may be passed by the workflow; local execution must not
   require the Actions cache service. Do not publish images, change deployment
   defaults, start a real runtime install or remove user volumes/images.
6. Reduce .github/workflows/ci.yml to existing push/PR triggers, permissions,
   concurrency/matrix, checkout/tool/cache setup and short script calls. Keep
   both job identities and the existing checks. Do not move GitHub secrets into
   scripts or use pull_request_target. Do not broaden publish/release triggers.
   Small provider-specific configuration in YAML is fine; downloads, integrity
   decisions, gate lists and container checks belong to scripts.
7. Document exact local/CI entrypoints, prerequisites, offline-vs-networked
   boundaries, what checks do NOT prove, and expected evidence in scripts/README.md.
   README's Development commands may link there; preserve all accepted runtime
   operational text. Add small stdlib-only tests for new script orchestration
   where valuable (command sequence, exit propagation, fresh-binary env, cwd
   handling and critical validation). Place them under scripts/tests/ so no
   existing frozen tests need edits. CI should run these checks too.

The task design section 11's CI shape is retained: ordinary platform fixture
tests plus a separate official Linux binary smoke. The release-build gate is
not release-publishing evidence. Container mount smoke is not production boot,
network artifact validation, or deployment acceptance. Do not close pending
Linux/CI checks merely by adding their invocation.

## Write scope and safety

Allowed: .github/workflows/ci.yml; existing scripts/ci.sh, linux-smoke.sh and
container-smoke.sh only as necessary; small new scripts/ci*.py or scripts/ci*.sh,
scripts/tests/ and scripts/README.md; README.md Development section only.
scripts/dev.sh, other workflows, Dockerfile, compose.yaml, rust-toolchain.toml,
Cargo files, src/**, tests/**, worker/** and all .trellis/spec/** stay read-only.
No changing installer tests to accommodate CI or weakening gates. If an
out-of-scope production fix is required, report the exact failure and stop
that part for coordinator triage.

Every existing task design/PRD/approval/plan/brief/snapshot/review record stays
read-only, including review/findings.json and responses.json. Only CI reports
below are writable. Recheck hashes in review/ci-start.json at the end. Known
dirty work and frozen whitespace are pre-existing; do not revert/repair them.
Stop immediately on genuinely unexpected concurrent changes.

No Git staging/commit/push/merge/reset/revert, no workflow dispatch, remote host,
live upstream artifact download, deployment or real libs cleanup in this turn.
The owner controls the eventual single delivery commit and CI trigger. Do not
install global tooling to make a check appear available. Permission-denied
cache/network work needs escalation, never bypass. Use isolated test roots;
Windows cleanup/moves use one shell, literal paths and resolved containment,
after writers stop. Do not open visible background helper windows.

Keep local validation light, consistent with backend specs: Python script
unit checks, syntax/format/diff/YAML validation with available tooling and safe
offline stubs. Do not rerun the full Rust build/test suite just to test the
orchestrator; it was already run on the unchanged source. Bash syntax checks
can use available Git Bash, but do not label that Linux runtime evidence.
Report unavailable tools/checks honestly; native Linux, hosted CI, Docker and
upstream network smoke remain pending until really executed.

Use rg and bounded reads. No whole-file dumps of the installer/lockfile.
Write coherent patches; checkpoint partial progress before context pressure.
No polling or nested agents. JSON is UTF-8 without BOM; ASCII for new docs.

## Reports and response schema

Write review/ci-progress.md with changed files, command exits, validation/test
mapping, hash verification and pending evidence. Write review/ci-responses.json
using the schema below; no code-review findings exist for this extension, so
round=1. The installer findings are a separate completed review, not a CI round.
If blocked write review/ci-blocked.md; a real design conflict goes in
review/ci-spec-gap.md with clause, problem and blocked work. Do not change the
normative docs to resolve a conflict. Out-of-scope observations may be appended
to review/ci-follow-ups.md, with locations and scope rationale, not extra fixes.

```json
{
  "round": 1,
  "scope_summary": "CI/script changes completed; excluded work and evidence explicit",
  "changes": [{"file": "", "intent": "", "spec_clause": "ci-implementation-brief.md plan item N; design.md section 11"}],
  "test_notes": "Personally executed commands/exits and missing native/hosted evidence",
  "finding_responses": []
}
```

For later assigned repair rounds respond to every CI finding with ACCEPTED
(minimal fix), REBUTTED (verified reachability/clause evidence) or SPEC_CONFLICT
(gap report and stop), never tests-green as an architecture rebuttal. Unresolved
third-round disagreements return to the owner instead of an endless loop.
Final reply: two or three factual sentences, no acceptance verdict. Stop for
coordinator inspection and independent review of the bounded CI delta.
