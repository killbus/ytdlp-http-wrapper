# Managed Python delivery

## Authorization and scope

The user approved the explicit default pin `2026.10.26`, then instructed the
coordinator to proceed with the proposed single combined uv/default-pin change
and PR-based GitHub CI. Only the named task-owned files are staged. The repair
branch is `fix/pbs-cpython-selection`, based on main commit
`658075b17d1d02475bc2506ba86d93caac03714c`.

Commit subject:
`fix(runtime): delegate Python setup to uv and update yt-dlp pin`

No main merge, tag, release, image publication, production deployment, amend or
force push is authorized. The existing main/tag publishing workflows are not
changed. PR validation is not a release.

## Included files

- `.github/workflows/ci.yml`
- `.trellis/spec/wrapper/backend/dependency-sources.md`
- `Cargo.toml`
- `Dockerfile`
- `README.md`
- `scripts/README.md`
- `scripts/ci.py`
- `scripts/runtime-boot-smoke.py`
- `scripts/tests/test_probe_environment.py`
- `scripts/tests/test_runtime_boot_smoke.py`
- `src/main.rs`
- `src/runtime_installer.rs`
- `tests/fixtures/managed_runtime.rs`
- `tests/runtime_install.rs`
- `worker/probe_environment.py`

Task-owned records included under this review directory:

- `managed-python-owner-decisions.md`
- `managed-python-findings-r1.json`
- `managed-python-findings.json`
- `managed-python-responses-r1.json`
- `managed-python-responses.json`
- `managed-python-progress.md`
- `managed-python-follow-ups.md`
- `managed-python-release-pin-check.json`
- `managed-python-pin-update.md`
- `managed-python-delivery.md` (this file)

Dispatch briefs, machine-local candidate/dirty-state snapshots and agent receipts
remain local working records rather than shipped runtime requirements. References
to them in historical review evidence describe local checks, not delivered APIs.
The earlier abandoned PBS repair and unrelated historical CI/task records are
not included. Cargo.lock, backend/worker protocol/pool/supervisor files, historical
PRD/design/approval documents and all unrelated user work are preserved.

## Verification and operational boundary

Independent code review round 2 closed F1-F3 and returned PASS before the final
owner-approved pin change. The pin-only main.rs delta is exactly two literals;
its source/default assertion and documentation were checked separately. The
owner's final dependency-sources update records already approved M1-M5 boundaries,
removes obsolete lofty-patch guidance and captures F2/F3 lessons; it does not
retroactively change the review standard or add new production behavior.

Lightweight formatting, scoped diff/source/hash checks and focused pure-path/mock
script tests have completed; see the responses, progress and pin-update records
for actual commands and evidence limits. Heavy builds/tests intentionally run in
GitHub CI. Native Windows/Linux Clippy/tests, release, Docker build/containment and
real runtime cold-boot/offline-reuse checks are pending, not claimed successful.

Before staging, the coordinator reran formatting, scoped whitespace checks,
the 3 pure-path probe tests and 5 mocked boot-smoke tests, Python syntax/JSON
validation and candidate hash comparisons: all completed with exit 0. The mock
failure-propagation test's timeout diagnostic is not a real Docker timeout.
Remote main still matched the baseline and no open PR existed for the repair
branch at the preflight check.

The first staged whitespace check found CRLF endings in the new round-2 response
JSON. Only that task-owned record was normalized to LF; parsed JSON equality
confirmed no content change. No product code changed as part of normalization.

Runtime preparation remains at boot. uv is a deployment prerequisite; schema-1
plans and PBS-release input require migration. A private libs-dir still requires
stop/await/start; no hot reload or concurrent shared-root update is introduced.
The stable uv base/cache is not moved with the runtime. The real smoke uses its
own disposable container volume, not an operator's deployed runtime.

## Excluded existing dirt

- `.trellis/tasks/11-03-resident-worker-runtime-upgrade/task.json`
- `.claude/`, `.opencode/`, `.tmp-baseline-tests.log`
- `.trellis/spec/mpd/`, `.trellis/spec/myMPD/`
- `.trellis/tasks/00-bootstrap-guidelines/`, `.trellis/tasks/11-03-bump-yt-dlp-2-10/`
- Other untracked 11-06 task/dispatch/history/abandoned repair records
- `docs/superpowers/`, `examples/`, `response.json`, `todos/`

These paths are not staged, reverted, removed or otherwise repurposed.
