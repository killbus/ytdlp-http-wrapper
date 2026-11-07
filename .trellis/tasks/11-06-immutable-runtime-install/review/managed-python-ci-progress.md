# Managed Python hosted-CI repair: C1-C3

Date: 2026-11-08. Round: 1. Status: implementation and lightweight checks
complete; awaiting coordinator diff review and hosted native gates. This is not
self-approval, a merge recommendation, or a claim that native CI now passes.

## Scope and starting state

- Read `managed-python-ci-fix-brief.md` first, then AGENTS/RTK, wrapper backend
  index/dependency-sources/process-lifecycle, M0-M6, and the approved pin update.
  The narrow CI brief and M0-M6 supersede historical PBS requirements.
- Verified HEAD `c08dcc1e1eccbc940b4ee8116486e1b9c21bafed` and branch
  `fix/pbs-cpython-selection`; both read-only Git commands exited 0.
- Initial `git status --short` showed only one tracked edit:
  `.trellis/tasks/11-03-resident-worker-runtime-upgrade/task.json`. Neither
  implementation file was dirty. Known untracked editor/spec/task dispatch
  records, docs/examples/todos, `.tmp-baseline-tests.log`, and `response.json`
  were already present and are untouched. Git warned that the user-level
  ignore file was inaccessible; repository status remained available.
- Changed only `src/runtime_installer.rs`, `tests/runtime_install.rs`, and the
  two new `review/managed-python-ci-{progress.md,responses.json}` records.
  Shared fixtures required no changes. No resolver/environment algorithm,
  version pin, dependency, worker, HTTP/lifecycle/pool, workflow, Docker,
  script, spec, or prior report was edited.
- No subagents, builds, Clippy, Rust suites, environment provisioning, Docker,
  Git staging/commit/push/merge, or publication was performed.

## C1: UNIX-only permission error

Added `#[cfg(unix)]` to the `PermissionFailure` enum variant and its `as_str`
match arm. Both production constructors already live inside the UNIX section
of `set_executable`, and the existing permission-error regression is UNIX-only.
The error code string, constructors, behavior, and regression are unchanged.
No dead-code allowance or Clippy relaxation was added.

A read-only source comparison reconstructed HEAD exactly after removing just
these two new attributes. This is source evidence, not Windows compilation.

## C2: same interpreter, different valid request text

The identity regression now inserts an explicit current-directory component
before the existing executable basename, using the native separator. Example:
`/host/python/bin/./python` instead of `/host/python/bin/python`. It does not
replace the executable with a directory or select a version from PATH.

The test asserts that the two request strings differ and that their canonical
filesystem paths match before the second installation. After installation it
also asserts distinct recorded request strings but identical complete probed
environment identity, including the canonical base executable and its hash.
Existing runtime-id inequality, fixed binary/Python/worker/search paths,
old-only sentinel removal, manifest/integrity, single-installation, and owned
staging cleanup assertions remain. The source plan stays offline.

Official pinned documentation and source read over HTTPS:

- <https://github.com/astral-sh/uv/blob/0.11.29/docs/concepts/python-versions.md#L44-L52>
  documents an explicit executable path separately from an installation root.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-python/src/discovery.rs#L1858-L1866>
  parses existing files as `PythonRequest::File` and directories separately.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-python/src/discovery.rs#L1066-L1074>
  queries the provided executable directly.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-python/src/discovery.rs#L1005-L1014>
  resolves directory requests using an environment's Python executable layout;
  a host executable's parent is not necessarily such a root.

No uv discovery, interpreter execution, or venv creation was run locally to
verify this new fixture. Native execution remains a hosted test obligation.

## C3: safe skipping, not mandatory rejection

Inspected official uv 0.11.29 source rather than inferring containment from
the hosted `Ok(RuntimePaths)` result:

- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-extract/src/stream.rs#L19-L36>
  `enclosed_name` starts at depth zero; a leading `ParentDir` fails
  `checked_sub(1)` and returns `None`. Absolute/prefixed paths are also rejected
  by this helper. For the fixture's `../outside`, this is not normalization.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-extract/src/sync.rs#L63-L82>
  the synchronous extractor logs a warning and returns `Ok(None)` for an unsafe
  name BEFORE joining the target or creating directories/files. Its final
  filter excludes skipped members from the extracted-file list (lines 153-157).
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-extract/src/stream.rs#L119-L131>
  the streaming extractor likewise skips the member and continues.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-distribution/src/distribution_database.rs#L1087-L1093>
  the local-wheel no-hash path calls `unzip_wheel`.
- <https://github.com/astral-sh/uv/blob/0.11.29/crates/uv-distribution/src/distribution_database.rs#L1185-L1218>
  `unzip_wheel` extracts into a temporary directory immediately under the uv
  cache root, calls the synchronous extractor, validates/heals RECORD, and
  persists the extracted tree. The hashed local-wheel streaming path also
  creates its extraction directory directly under that cache (lines 1117-1133).

In this wrapper the private cache is `libs/uv-cache`; therefore naive extraction
of `../outside` would leave `libs/uv-cache/outside`, not merely the previously
checked fixture-root `outside`.

Renamed the regression to `real_uv_skips_unsafe_wheel_paths_without_escape`.
The original malformed member and payload remain. It now requires successful
installation of the safe members and calls `assert_installed_plan`, which checks
manifest/artifact/worker identity, fixed publication, the single-installation
layout, and absence of owned staging. A directory-entry walk rejects any
`outside` entry throughout the isolated fixture, including the extraction
parent, all retained cache trees, runtime, and site-packages/parents. It does
not follow symlinks or read every file's contents. This also detects a payload
that was merely normalized into a cached/published tree instead of skipped.
The original fixture-root absence assertion remains.

Actual invalid-archive failure coverage is preserved unchanged:

- `initial_hash_and_extraction_failures_leave_no_runtime_then_recover` uses
  hash-pinned non-ZIP bytes, requires `RUNTIME_ENVIRONMENT_FAILURE`, no runtime,
  no owned staging, and successful recovery with the repaired archive.
- `failed_preparation_preserves_prior_runtime` tests a hash-pinned non-archive
  against both mismatched and corrupt prior installations, requiring failure,
  unchanged prior snapshots, and staging cleanup.
- `real_uv_incompatible_python_or_missing_dependency_preserves_prior_runtime`
  remains unchanged as additional real-uv failure coverage.

No production extraction code, custom ZIP parser, general defensive framework,
or dependency/tool version change was introduced. This is evidence for the
specific malformed-member fixture, not a general uv security audit or a claim
to sandbox arbitrary package code.

## Executed checks and evidence limits

- `uv --version`: exit 0, `uv 0.11.29 (901092ee1 2026-07-15 x86_64-pc-windows-msvc)`.
  `uv venv --help`: exit 0; confirms `--python`, `--offline`, and
  `--no-python-downloads`. These commands perform no provisioning.
- Read-only `Invoke-WebRequest` requests to raw GitHub sources: the first
  sandboxed request failed authentication (exit 1); the approved elevated
  read succeeded (HTTP 200, exit 0). A batched read encountered HTTP 404 for
  a guessed `docs/reference/python-discovery.md` path after the extraction and
  discovery source reads succeeded. The corrected pinned
  `docs/concepts/python-versions.md` and distribution call-site batch returned
  HTTP 200 and exit 0. No failed response is used as positive evidence.
- Initial `cargo fmt --check`: exit 1, one newly added manifest-read expression
  needed wrapping. `rustfmt --edition 2021 --config skip_children=true
  tests/runtime_install.rs`: exit 0. Subsequent `cargo fmt --check`: exit 0.
- `git diff --check -- src/runtime_installer.rs tests/runtime_install.rs`:
  exit 0 before and after formatting.
- Read-only PowerShell source comparison against `git show HEAD:<path>`:
  exit 0. Production matches HEAD after removing the two new cfg attributes;
  all 17 unrelated async tests match HEAD exactly; the total async-test count
  is unchanged (19); identity and containment assertions are present; tracked
  changes are limited to the two owned files plus original unrelated dirt.
- Final artifact checks: exit 0; both review files are valid UTF-8 without BOM,
  use LF only, and have no trailing whitespace. JSON parses and includes round
  1 responses for C1-C3. Final `cargo fmt --check` and scoped `git diff --check`
  both exited 0. Final status shows no new unexpected edits; HEAD and branch
  remain unchanged.
- Candidate SHA256: `src/runtime_installer.rs` =
  `934BE207ECD0814F1CAAA1A7412DDEF8E9F931AA8BCB0FA8E7FF47DE0884FC2C`;
  `tests/runtime_install.rs` =
  `C1BB3764AA80A82C168C2CC50BE1B6D78FE18FB2C890B23E3FC4D1FF350F06B3`.
- Simplification pass: kept the containment walk local to its one test, reused
  existing publication/cleanup assertions, and added no shared abstraction or
  duplicate invalid-archive test. No further refactor was needed.
- Spec-sync assessment: executable-directory semantics and safe-skip behavior
  are recorded here for coordinator consideration. Specs are read-only in this
  dispatch; existing tool-ownership and native-evidence rules remain valid.

Coordinator-supplied hosted evidence is from
<https://github.com/killbus/ytdlp-http-wrapper/actions/runs/42404267541>:
Windows job 128498470775 failed strict Clippy on the unused variant; Ubuntu job
128498470794 had 17 passing installer tests and the two fixture failures;
Docker job 128498470525 subsequently succeeded, including actual boot/resident
reuse. I did not independently rerun or reclassify these results.

Required next gates are coordinator diff review and native Windows/Linux
Clippy, tests, and release checks on the repaired candidate. Existing Docker
success is baseline evidence only, not execution evidence for this repair.
Merge and image publication remain withheld. Stop here for coordinator review.
