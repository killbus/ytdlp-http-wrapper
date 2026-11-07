# Owner-authorized delivery and hosted validation

The owner authorized one task-scoped delivery commit, a push to the PR branch,
and following GitHub CI. This supersedes the earlier no-commit/no-push limits
for the coordinator only. It does not authorize merging, publishing releases,
deploying, deleting a live runtime, or including unrelated worktree changes.

## State before delivery

- Source baseline: 775bd5fa974f1008bb24e71ea92f674515706948.
- Locked design SHA256:
  DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.
- Installer architecture/test-assertion review: round 1 PASS, no blockers.
- CI extension review: round 2 PASS, F1 closed by removing the unsupported
  Compose run option. This is source/contract closure, not hosted execution.
- Coordinator rechecked 76 round-2 snapshot entries unchanged immediately
  before preparing delivery; scoped diff checks and cargo fmt --check exit 0.
- Existing final installer Windows evidence is 105 Rust tests and 14 Python
  tests passing. CI repair focused shell evidence is 11 tests passing, while
  two complete 21-test script attempts failed on unchanged 30-second smoke
  timeouts. Root cause remains undiagnosed; no full post-fix green is claimed.
- The owner stopped further local timeout diagnosis. It produced no code edit
  or diagnostic report. Native Windows/Linux, container and release-build
  evidence should now come from the repository scripts invoked by GitHub CI.

## Commit boundary

Deliver dependency/lockfile removal, installer introduction, boot wiring,
installer/config regressions, operational README, reusable validation scripts,
thin CI workflow and this task's applicable design/review/response records in
one commit. The required AC1 check compares that commit with its parent and
the baseline; an already-correct final tree alone is insufficient evidence.
The commit message records the fixed-path layout and A8/A9 operational limits.

Do not stage other task changes, todos, response.json, editor/agent directories,
other package specs, examples, unrelated docs, temporary logs or generated
caches. The abandoned timeout-diagnostic brief/snapshot are not delivery inputs.
No independent dependency-removal or installer-introduction commit precedes
this transition after the baseline.

Spec-sync considered: this work applies the locked installer contract and the
owner's explicit scripts/thin-workflow extension. Durable command/evidence
guidance is in scripts/README.md; no frozen normative spec is silently changed
as part of delivery. Historical review snapshots keep their original hashes.

## Hosted validation and remaining limits

Target GitHub repository: killbus/ytdlp-http-wrapper.
PR head: feature/resident-worker-runtime-upgrade; base: main.
The workflow runs for PRs to main; pushing only this feature branch does not
by itself trigger it. Do not merge or change triggers to bypass this boundary.

At preflight, the Git transport reached origin successfully, but GitHub CLI
reported no authenticated account both inside and outside the sandbox. Creating
or inspecting a PR through gh requires normal GitHub CLI authentication; do not
extract other stored credentials. If that remains unavailable after push,
report the authentication blocker and provide the branch compare URL rather
than claiming CI was triggered.

No hosted CI, actual official binary smoke, Docker execution or release build
is established by this record. A1-A6 rollout/default confirmation remains
separate. This is authorization to submit for CI, not final task acceptance.
