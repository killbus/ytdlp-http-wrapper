# CI extension progress -- 2026-11-07

Implementation handoff; independent CI-delta review has not occurred.
Authority: ci-implementation-brief.md, bounded items 1-7. Existing installer
review and acceptance records are not edited or extended by this report.

## Current handoff

Implemented the shared Python runner, thin ci.sh, official Linux preparation,
container orchestration, thin workflow, scripts documentation and offline script
unit tests. README changes in this dispatch are Development-only. No production
source, Cargo inputs, frozen test suites, Dockerfile or compose.yaml edits.
Local orchestration checks are complete. Next action belongs to the coordinator:
inspect ci-responses.json and obtain independent review of this CI delta before
owner delivery/CI execution. No task-status or previous review record is changed.

## Changed files and validation policy

| Path | Change | Brief item / design |
| --- | --- | --- |
| scripts/ci.py | Shared fmt, locked strict all-target Clippy, current host binary build, locked Rust tests with absolute child-only CARGO_BIN_EXE, Python suite and script tests; separate locked release gate | 1-3, 7 / section 11 |
| scripts/ci.sh | Argument/exit forwarding to Python runner only | 1 / section 11 |
| scripts/ci-linux-smoke.sh | Existing gh release selection or explicit tag; private fresh download directory, exact required checksum before execution, selected tag/hash, contained owned cleanup | 4 / section 11 |
| scripts/linux-smoke.sh | Keep no-download consumer; require native Linux x86_64, absolute executable and explicit Linux Rust host target; retain ignored smoke test | 4 / section 11 |
| scripts/ci-container.sh | Existing local-tag buildx load plus Compose config/run; optional cache-only arguments; reuse unchanged in-image assertions | 5 / section 11 |
| .github/workflows/ci.yml | Keep rust/docker identities, Ubuntu/Windows matrix, push/PR main and concurrency; Python setup, repo rustup selection/components, caches and short script calls; explicit contents:read | 3, 6 / section 11 |
| scripts/tests/test_ci.py | 10 stdlib orchestration tests using subprocess mocks | 7 / section 11 |
| scripts/tests/test_shell_ci.py | 9 offline shell tests using private gh/cargo/rustc/docker stubs | 7 / section 11 |
| scripts/README.md | Entrypoints, prerequisites, network boundaries, expected evidence and exclusions | 7 / section 11 |
| README.md | Development commands and validation-guide link only | 7 / section 11 |

Reports written only to review/ci-progress.md and review/ci-responses.json.
scripts/container-smoke.sh remains byte-identical; its non-root, tmpfs size/mode,
mount flags and executable-probe assertions are reused. Dockerfile and compose.yaml
are unchanged. Local image tagging and configured downloads-volume creation remain
possible when that script is eventually run; there is no image publication or
user-volume/image removal. Container entrypoint replacement avoids real boot.

The runner forces rustc's host target, honors Cargo target-directory configuration,
and reads the executable from the current successful build's JSON, not an assumed
path. Cargo's up-to-date output reuse is allowed; no clean/full rebuild is forced.
Root or chmod-bypassing Linux environments stop before Rust tests. Tar, curl and
discoverable Python are required; script tests require Bash. Dependency acquisition
may use network on fresh runners; fixture data itself stays local. The scripts do
not clone/reimplement production artifact resolution or validate A1-A6 defaults.

## Personally executed lightweight checks

Native host: Windows; Python 3.14.0; Git Bash 5.2.37 (x86_64-pc-msys);
Ruff 0.17.0; yq 4.47.2. No new tooling was installed.

- Initial snapshot check: all 58 frozen files and 5 authorized starting files
  match review/ci-start.json (63 comparisons, zero mismatches).
- Final `python scripts/ci.py script-tests` from repository root: exit 0;
  19 tests, 23.168s. Also invoked by absolute script path with cwd=scripts:
  exit 0, 19 tests, 42.830s, verifying repository-cwd selection in a real process.
  Earlier incremental runs: 17 tests, exits 0, 58.472s and 19.617s.
  Shell tests use Git Bash on Windows and offline gh/cargo/docker stubs; no
  Linux runtime, upstream downloads or Docker daemon operations occurred.
- Python ast parsing: exit 0, all 3 new Python files. Scoped LF/trailing-whitespace
  checks: exit 0 (includes new files not covered by ordinary git diff).
- `bash -n` individually for ci.sh, ci-linux-smoke.sh, linux-smoke.sh,
  ci-container.sh and container-smoke.sh: all five exits 0.
- Scoped `git diff --check -- .github/workflows/ci.yml scripts README.md`: exit 0.
- Initial Ruff check: exit 1, import ordering/test-style suggestions only;
  initial format check: exit 1, three files required formatting. Manual test-style
  corrections, import fix and formatter applied only to the three new Python
  files; final `ruff check ... --no-cache` and `ruff format --check ... --no-cache`
  both exit 0. A simplification pass retains explicit functions/commands and no
  scheduler, plugin engine, workflow generator or extra validation framework.
- `yq -o=json . .github/workflows/ci.yml`: exit 0. Parsed object assertions retain
  rust/docker, Ubuntu/Windows, push/PR on main, and contents:read. No actionlint
  or hosted workflow execution is claimed; actionlint was not available on PATH.
- `python scripts/ci.py --help`: exit 0. No real check/release entrypoint run.
- README verification: reverse only this dispatch's Development replacement in
  memory, SHA256 equals the recorded starting README hash
  1F5A275D1197BC0E78BF9EE0E4365E6F048B317DFE4379CCDC62205006853DAF.
  The larger git README diff is pre-existing accepted operational documentation.
- Final frozen-file SHA256 comparison: all 58 entries in ci-start.json match,
  zero mismatches. Design remains
  DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.
  container-smoke.sh also matches its authorized starting hash. HEAD remains
  775bd5fa974f1008bb24e71ea92f674515706948. Global Git-ignore access warnings were
  already present and were not repaired or treated as a reason to alter Git state.

## Test-to-contract mapping

| Script test area | Evidence established locally | Still not established |
| --- | --- | --- |
| Shared sequence, six injected command failures, release gate | Exact locked gate lists, strict Clippy, first nonzero exit propagated; no later gates | Real Rust execution through new runner |
| Current artifact / missing, duplicate, malformed or relative artifact | No stale env fallback, custom target path with spaces used, absolute child-only CARGO_BIN_EXE; explicit host target | Actual Cargo artifact output on hosted OSes |
| Missing tools/Python, root, permission bypass/denial | Fail-closed precondition branches with mocks | Native Linux filesystem denial |
| run cwd and arguments, absolute-path invocation, shell wrapper | Argument-list subprocesses, stable repo cwd, Bash forwarding and nonzero exit | Hosted shell integration |
| Official artifact normal/tag override and checksum error matrix | One release selected; missing/wrong/duplicate/malformed/unrelated checksums or absent binary/download error stop before chmod/execution; owned directory removed | Real upstream availability/trust or onefile extraction |
| Consumer platform/path and Rust-host rejection | Non-Linux/relative artifact/cross-host cannot silently run a zero-test target | Native Linux smoke |
| Container sequence/cache/build failure | Same local tag, config validation, no publish option, unchanged in-image script piped to shell; stop on build error | Actual image build/Compose/tmpfs assertions |

References were scanned in scripts, README, docs and workflows. Existing
docs/process-lifecycle.md consumer usage remains compatible and was left alone;
release/docker publishing workflows and frozen records are unchanged. Durable
usage is in scripts/README.md; this report is the scoped handoff, not a new project
management framework or a rewrite of installer records.

## Pending evidence

Native Linux tests including Unix denial/sync/link behavior, hosted CI matrix,
actual official onefile download/smoke, Docker/Compose execution, locked release
builds, A1-A6 rollout/default validation and AC1 owner delivery-commit history
remain pending. Existing Windows Rust evidence belongs to the frozen installer
report, not a rerun or new CI result here. No staging/commit/push, CI trigger,
remote host, live-runtime install/cleanup or deployment has been performed.

## Round 2 -- F1 only (2026-11-07)

Role: bounded Implementer, following ci-fix-brief.md and
ci-implementation-brief.md plan items 5-6. Response to F1 is ACCEPTED (minimal
repair), not an independent closure verdict. Stop for the same Reviewer's
round-2 check; neither prior installer acceptance nor CI findings is rewritten.

### Repair and preserved handoff

- `scripts/ci-container.sh:22`: remove only `--no-build` from Compose run.
  `.github/workflows/ci.yml:55` still invokes this script; buildx loads the same
  `ytdlp-http-wrapper:local` image used by `compose.yaml:3`.
- Retain `--rm --no-deps --pull never -T --entrypoint /bin/sh ytdlp -eu` and
  exact unchanged `scripts/container-smoke.sh` stdin. Workflow, Compose,
  Dockerfile and the smoke's non-root/tmpfs size/mode/mount/executable assertions
  are byte-identical; no installer boot, image fallback, publish or skipped gate.
- `scripts/tests/test_shell_ci.py:66`: minimal exact-argv test guard, not a
  Compose emulator. It rejects missing, changed or extra handoff arguments and
  consumes stdin only after checking the expected sequence. Correct the old
  assertion and require exactly five orchestration calls.
- `scripts/tests/test_shell_ci.py:207`: in a private per-test temporary tree,
  copy Compose/smoke and reinsert `--no-build` into a script copy; require exit
  64, the invalid-option diagnostic, five calls and no consumed smoke input.
  The workspace script is never restored or reverted. Reinserting that token
  reproduces the exact starting script SHA256 recorded in ci-fix-start.json:
  `6D46187D47CEE8CB0000F3B7EB75DF9A13D0557442417B28DB51A6E766C16509`.
- `scripts/tests/test_shell_ci.py:228` retains byte-for-byte positive stdin
  checking. At line 249, simulated Compose/smoke exits 1 and 37 propagate after
  unchanged stdin with no subsequent commands. These do not execute real
  container assertions. Existing cache/build-error, checksum, cleanup and
  platform assertions and the 30-second subprocess timeout are not loosened.

### Personally executed checks

PowerShell at the repository root; Python commands set
`$env:PYTHONDONTWRITEBYTECODE='1'`. No tooling installation or source download.

| Command | Exit / observation |
| --- | --- |
| `python -m unittest scripts.tests.test_shell_ci -v` | 0; 11 tests, 102.980s, including isolated pre-fix rejection and exits 1/37 |
| `python scripts/ci.py script-tests` (first run) | 1; 21 tests, 117.302s; TimeoutExpired after 30s in unchanged `test_wrong_rust_host_does_not_execute_a_zero_test_smoke`; four container methods completed |
| `python scripts/ci.py script-tests` (retry) | 1; 21 tests, 202.628s; TimeoutExpired after 30s in unchanged `test_missing_malformed_duplicate_or_wrong_checksum_never_executes`, mode `wrong`; four container methods completed and wrong-host test completed |
| `ruff check scripts/tests/test_shell_ci.py --no-cache` | 0, initial and final |
| `ruff format --check scripts/tests/test_shell_ci.py --no-cache` | Initial 1 for one new long copyfile line; wrapped only that line; final 0 |
| `D:\Applications\Scoop\apps\git\2.53.0\bin\bash.exe -n scripts/ci-container.sh` (Python subprocess argv) | 0 |
| `git diff --check -- scripts/ci-container.sh scripts/tests/test_shell_ci.py` | 0; supplemented with explicit LF/trailing-whitespace checks because both files are untracked in the admitted worktree |

Python AST parsing and explicit LF/trailing-whitespace checks on the two code
files exited 0. The byte-replacement/SHA256 check described above also exited
0. `shutil.which("docker")` returned `None`; no installed Compose version/help
was available. The unchanged full-suite timeout cases are disclosed, not
silently counted as successful checks. Their cause was not diagnosed within
F1 scope; no unrelated tests, timeout limits or smoke scripts were changed.
A clean full `python scripts/ci.py script-tests` run is still not established
in this repair turn. No additional repeated full-suite attempts were made.

### Interface evidence versus execution

The interface basis is the existing round-1 review's official references:
- https://docs.docker.com/reference/cli/docker/compose/run/
- https://github.com/docker/compose/blob/v2.40.3/cmd/compose/run.go#L225
- https://github.com/docker/compose/blob/v2.40.3/cmd/compose/create.go#L79

That recorded source/documentation evidence places `--no-build` on create,
not run, and supports the retained run options. The test comments cite the
run reference and pinned source. This turn did not independently fetch those
pages or run an actual Docker CLI; a stub accepting our expected argv is not
real CLI compatibility/execution evidence. Native Docker/Compose smoke, the
image's actual containment assertions, native Linux/hosted CI, official upstream
smoke, release execution, A1-A6 rollout and AC1 owner delivery history remain
pending independently of this repair and of any later review verdict.

### Scope and locks

At admission all 72 immutable and four writable starting hashes matched
ci-fix-start.json; no active CI blocked/spec-gap marker was present. Before
reporting all 72 frozen hashes and both original report hashes still matched.
HEAD remains `775bd5fa974f1008bb24e71ea92f674515706948`; admitted pre-existing
worktree dirtiness is preserved, not claimed clean. Final frozen-hash and
append-only/JSON checks are required immediately after this report write.

Only the two allowed code files, this appended round-2 section and the current
ci-responses.json are written. No source/spec/other workflow edit, archived
review edit, nested agent, live artifact download, Docker daemon, remote host,
full Rust rerun, Git mutation, deployment or live runtime cleanup occurred.

Post-write verification: all 72 frozen hashes still match; the original progress
prefix matches its starting SHA256; exactly one round-2 F1 response validates
against the required schema and UTF-8/no-BOM checks. No CI marker appeared.
