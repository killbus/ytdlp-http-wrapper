# Online update implementation — 2026-11-25

User explicitly requested a simple task without Paseo. No agents were created.
Main session implemented directly after the user's instruction to proceed.

## Changes

- `/run` routes standalone `--update` / `-U` to service update coordination.
- Shared request leases span supervised cleanup; exclusive updates drain requests.
- Existing process supervisor runs a hidden installer entrypoint, containing curl,
  uv and Python children with the usual process group/JobObject and timeout.
- Existing installer prepares both artifacts, then retains a temporary old tree
  until backend activation. Failure restores it; interrupted transactions recover
  before boot selection. No version history or new management endpoints.
- Manifest online_update_from preserves user-requested online selection across
  same-pin restarts and recipe repairs; explicitly changed startup pins win.
- Official PyPI stable metadata selects the date version; official GitHub binary
  and SHA2-256SUMS plus exact uv package requirements validate the chosen release.
- Unsupported mixed update args, source plans and worker overrides fail explicitly.
- Real acceptance exposed Windows PyInstaller's inability to use canonical
  verbatim TEMP paths; only child environment paths convert back to Win32 syntax.

## Verification

- Windows local: cargo fmt --check, cargo check --offline --all-targets,
  cargo clippy --offline --all-targets -- -D warnings, cargo build --offline.
- 5 update unit tests: rollback before commit, interrupted publication recovery,
  persisted selection/recipe repair/changed-pin behavior, stable date parsing,
  update timeout while a request holds its runtime lease.
- run_contract: 4 passed; resident_backend: 6 passed.
- process_lifecycle: 10 passed; 6 ignored fixture entrypoints are launched by tests.
- Windows child-temp-path regression: 1 passed.
- Python worker: 14 passed.
- Real HTTP source verification in target/online-update-_olbw4qc:
  injected missing uv causes exit_code 1, then old resident --version remains
  2026.10.26; successful --update installs 2026.11.23; --version observes it;
  -U explicitly reports already-current; resident restart with uv unavailable
  reuses 2026.11.23. After the TEMP path fix, CLI restart also returned
  2026.11.23 and -U returned already-current. Final script exited 0.
- Scripts and logs in the task/target directories are local verification artifacts.
  The original Windows HTTPS sandbox restriction required escalated local tests.

## Scope and remaining delivery

No production request/update, container rebuild/deployment or push.
Linux/Windows CI matrix and release build were not run in this session; local
Windows checks and actual official-source HTTP behavior are the evidence above.
The user authorized the planned commit after the audit follow-up. Existing
untracked .claude/ is user-owned and excluded from the commit.
An externally killed installer may leave owned staging for manual inspection;
no cross-boot staging sweep or power-loss atomicity is claimed.

## External audit follow-up

- Added [audit-response.md](audit-response.md) with numbered implementation
  rationale, version-pin semantics, commit/response boundary, module ownership,
  and the exact scope/reasons of both spec changes.
- The requested lightweight/no-Paseo workflow remains in force. No retrospective
  approval, design-lock hash or per-clause owner authorization is fabricated.
  Trellis permits lightweight PRD-only tasks and requires related spec updates.
- Clarified the configuration/selected-version distinction with a README example
  and aligned CLI help text. Update behavior is unchanged.
- This follow-up checks source/document consistency; it is not an independent
  rerun of the earlier Windows tests or real-source HTTP acceptance.
- Follow-up validation: cargo fmt --check and git diff --check passed.

## CI follow-up after push

- Feature commit 3c67199 was pushed at the user's request. CI run 45588511533
  passed container validation, but both Linux and Windows stopped at
  `query_deserialization_test`: the old `--update` fixture expected spawn-failure
  HTTP 500; the new update route correctly returns HTTP 503 UPDATE_UNAVAILABLE
  when its test AppState has no updater. The failure was reproduced locally.
- Reworked that test to assert the actual extracted args and timeout, retaining
  the original valid/missing-input cases and checking an invalid timeout. Existing
  run_contract tests still cover update routing and failure codes.
- User requested rewriting the existing feature commit rather than adding a
  separate repair commit. Use fixup/autosquash and an explicit lease on the old
  remote main commit; no release tag or production deployment is included.
- Local fix verification passed: query_deserialization_test (2), run_contract
  (4), targeted Clippy with warnings denied, cargo fmt --check, git diff --check.
  The complete Linux/Windows and container gates will rerun on the rewritten
  feature commit; their results are tracked by GitHub Actions.
