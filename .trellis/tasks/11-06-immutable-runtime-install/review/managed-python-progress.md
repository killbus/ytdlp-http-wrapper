# Managed Python implementation checkpoints

## 2026-11-08 - checkpoint 1: inputs and uv capability evidence

- Baseline/branch and recorded production hashes match the dispatch snapshot.
  Existing unrelated dirt is unchanged; dispatch/start records are expected additions.
- Read M0-M6, workflow/backend contracts and installer/boot/worker dispatch paths.
  No backend, protocol, pool or supervisor changes are needed.
- Local read-only `uv --version` reports 0.11.29. `uv help venv`, `uv help
  python`, `uv python find --help`, and `uv help pip install` establish supported
  `--no-project`, `--managed-python`, `--python`, `--relocatable`, `--no-config`,
  `--link-mode copy`, `--no-build`, explicit interpreter installation and resolution.
  One PowerShell Select-String invocation was malformed; rerun succeeded.
- Bounded arrangement: uv creates a relocatable venv in owned staging; managed
  base interpreters live at the stable private `<libs>/uv-python` path, outside
  replaced runtime/staging. Never relocate/delete that base or a global uv cache.
  Probe the environment both before and after publication. Reuse the recorded
  base on updates unless the deployment supplies a changed Python request.
- uv venv chooses an interpreter, then uv pip resolves the explicit yt-dlp
  dependency against it. This is NOT automatic search across Python versions
  satisfying transitive dependencies. A mismatch fails with migration guidance;
  no Rust fallback/series table is introduced.
- Legacy PBS source plans and release tags will be rejected explicitly rather
  than retaining manual Python extraction/package installation. Schema-2 plans
  retain pinned binary acquisition and optional pinned wheel/offline input.
  Existing schema-1 installed manifests are rebuilt before admission.
- No Rust build/test, Docker execution or real runtime provisioning has run.
  Default yt-dlp 2026.09.23 remains unchanged and its live availability unverified.

## 2026-11-08 - checkpoint 2: implementation and deployment wiring

- Replaced live PBS/PyPI scraping and manual archive installation with uv venv,
  explicit `yt-dlp==<pin>` pip installation, no source builds/no extras, copy link
  mode, and pip check. Retained GitHub binary acquisition/hash checks and fixed
  publication/owned-staging machinery. Legacy release/plan rejection is explicit.
- Added read-only embedded environment probe: CPython, venv/base/import paths,
  exact yt-dlp module version, distribution identity, base executable hash,
  worker import and package paths. Manifest schema 2 includes observed identity;
  package lists are not presented as a portable lock. Published venv is probed.
- Reuse precedes live metadata and does not require uv. Updates prefer a validated
  recorded base; no dependency-driven interpreter retry/search. A new Python
  request is the explicit override. uv directories are private and not swept.
- Boot flags and native docs updated; official uv 0.11.29 copied into Docker and
  installed in native CI. zip moved to dev-dependencies (fixture writer only);
  no dependency/version changes or Cargo.lock edits. Worker protocol unchanged.
- Added a separate repository-owned real-entrypoint Docker smoke: CLI version,
  clean stop, offline resident restart/reuse with uv unavailable, same manifest.
  Existing Compose containment and Linux lifecycle smoke remain unchanged.
- Adapted existing publication/integrity/cleanup/native-lock fixtures to real uv
  offline environments with synthetic wheels and the existing host Python.
  Added incompatibility, missing dependency, extras, legacy migration and synthetic
  command-contract coverage. Rust tests remain unrun locally.
- Official metadata/docs access initially hit sandbox TLS errors; approved
  read-only rerun succeeded. GitHub confirms uv tag 0.11.29. Sources:
  https://github.com/astral-sh/uv/releases/tag/0.11.29
  https://docs.astral.sh/uv/guides/integration/docker/
  https://raw.githubusercontent.com/astral-sh/uv/0.11.29/crates/uv-virtualenv/src/virtualenv.rs
  Pinned source confirms Unix base symlink uses executable_target, Windows uses
  base trampolines, and relocatable handles environment scripts, not base copies.
- Local script suite: `python -m unittest discover -s scripts/tests -v` exit 0,
  25 small tests in 40.482s (mock/stub orchestration only, no build/provisioning).
  Initial cargo fmt --check reported formatting differences; scoped rustfmt ran.
  A README editing script first failed on Windows' default GBK decoding before
  writing; explicit UTF-8 rerun succeeded. Final scoped checks still pending.

## 2026-11-08 - checkpoint 3: lightweight checks and review handoff

- Final `cargo fmt --check`, scoped `git diff --check`, and Python AST parsing of
  four changed/new Python files exited 0. Composed production uv flags accepted
  by local venv/pip `--help` (no environment execution). Script suite remains
  25/25 successful mock/stub checks; Rust/native/live gates are still pending.
- Added positive offline dependency closure and extras tests, unsafe wheel-path
  regression, new CLI environment/flag precedence assertions, and synthetic uv
  command assertions. Native fixtures use the existing host Python, not downloads.
- Simplified within edited scope: removed obsolete selectors/extractors, retained
  existing publication/containment checks, moved ZIP solely to fixture dependencies,
  and preserved installed package path precedence for explicit worker overrides.
- Compared protected starting hashes: backend, supervisor, worker protocol,
  lifecycle/pool components, Cargo.lock, historical task records and unrelated
  task.json unchanged. No new unexplained dirty paths were observed.
- Additional pinned uv source inspection confirms executable_target uses the base
  path unless upgradeable mode is requested (virtualenv.rs lines 225-255). No
  upgrade command/mode is requested here. Actual Linux/Windows validation awaits CI.
- Wrote `review/managed-python-responses.json` and `review/managed-python-follow-ups.md`
  per the brief's ownership/response contract. Stopping for independent
  review, with no self-verdict, delivery action or claim of complete validation.

## 2026-11-08 - round 2: owner-accepted F1-F3 repairs

- Confirmed branch `fix/pbs-cpython-selection`, HEAD
  `658075b17d1d02475bc2506ba86d93caac03714c`, and all 19 round-2 starting
  snapshot hashes before edits. Read the round-2 brief, M0-M6, F1-F3 and the
  backend guidelines. No delegation or design reopening.
- F1: restored `std::process::Command` in `tests/runtime_install.rs`. The
  dependency-graph test text remains identical to the specified baseline.
- F2: `src/runtime_installer.rs` explicitly forwards the canonical installation
  root to recorded-runtime, reuse and candidate probes/validation; publication
  forwards the same root already held by `OwnedStaging`. In
  `worker/probe_environment.py`, `resolved(sys.argv[3])` replaces basename
  inference. Both base-executable and base-prefix containment assertions remain.
- Added `scripts/tests/test_probe_environment.py`: executes the actual probe's
  argument/containment AST section with pure POSIX/Windows paths. Covers ordinary
  and staging-named roots, candidate/published locations, stable private/external
  bases, and forbidden base/prefix directories and descendants. This is path
  semantics evidence, not native environment relocation evidence.
- Extended the existing offline worker-identity replacement/reuse unit test in
  `src/runtime_installer.rs` to use a root named staging. Its existing test-only
  command recorder now supports passthrough replies, so the test asserts the
  canonical root on all four update probes (old, candidate twice, published)
  without replacing their output. Existing fixed-path, old-only sentinel,
  validation, guard-disarming and byte-identical no-fetch reuse assertions stay.
  This native real-uv fixture remains unexecuted locally and never downloads a
  managed runtime; it uses the existing host interpreter and synthetic wheel.
- F3: restored exact argv equality in
  `live_resolution_only_fetches_the_explicit_binary_release`: one GitHub request
  to the explicit 2026.09.23 tag, `-fsSL --retry 0 --max-time 300 --max-filesize
  67108864`, and the existing HTTP-status suffix. Extra proxy/cacert/latest
  arguments cannot satisfy equality. Production curl policy is untouched; no
  obsolete PBS/PyPI selector is restored.
- Updated `scripts/runtime-boot-smoke.py` to use `/downloads/staging` for both
  cold install and offline reuse, read that root's manifest, and require the
  observed base executable/prefix under its stable `uv-python` directory.
  Extended `scripts/tests/test_runtime_boot_smoke.py` to check both arguments,
  manifest paths and stable/invalid base observations with mocked Docker calls.
  No second upstream release, changed pin, workflow edit or CI-policy expansion.
- Actual local checks and exit codes:
  - Before the fix, `python -B -m unittest discover -s scripts/tests -p
    test_probe_environment.py -v` exited 1, with 12 failing subcases for the
    published staging-named root on both path flavors. After repair it exited 0:
    3 tests, 80 path subcases.
  - `python -B -m unittest discover -s scripts/tests -p
    test_runtime_boot_smoke.py -v` exited 0: 5 mocked tests. The logged Docker
    timeout is the existing failure-propagation mock, not real Docker execution.
  - Initial `cargo fmt --check` exited 1 for two formatting-only hunks.
    `rustfmt --edition 2021 src/runtime_installer.rs tests/runtime_install.rs`
    exited 0; subsequent `cargo fmt --check` and
    `git diff --check -- src/runtime_installer.rs tests/runtime_install.rs`
    exited 0.
  - Inline `python -B -` source checks exited 0: parsed the four Python files;
    verified Command binding and unchanged dependency-graph function text
    against baseline; checked explicit root propagation and unchanged default.
  - Final hash checks matched all 13 unchanged round-2 entries and all 13
    protected original entries. Initial dirty-list comparison exited 1 because
    the snapshot's known_dirty omitted its own fix-start/fix-dispatch records;
    both were already present in this turn's first status and are explicitly
    expected coordinator records. Rechecking with those records included exited
    0; the only new dirty path is the focused probe test. Branch/HEAD unchanged.
- Exact round-2 write paths: `src/runtime_installer.rs`,
  `worker/probe_environment.py`, `tests/runtime_install.rs`,
  `scripts/runtime-boot-smoke.py`, `scripts/tests/test_runtime_boot_smoke.py`,
  `scripts/tests/test_probe_environment.py`,
  `.trellis/tasks/11-06-immutable-runtime-install/review/managed-python-responses.json`,
  and this append-only log
  `.trellis/tasks/11-06-immutable-runtime-install/review/managed-python-progress.md`.
- Wrote UTF-8/no-BOM round-2 responses for F1, F2 and F3. No frozen record,
  dependency, fixture helper, main/default pin, backend/pool/supervisor or worker
  protocol edits. No Git staging/commit/push or runtime provisioning/removal.
- Remaining evidence belongs in CI: all-target Clippy/native tests on Windows
  and Linux, exact curl argv and offline update/reuse assertions, release and
  lifecycle/containment gates, actual uv-managed base relocation/publication and
  real-entrypoint cold install/offline reuse. The unchanged default 2026.09.23
  remains availability-unverified. Stopping for independent round-2 review;
  these implementation notes are not a self-approval or live-success claim.
- Final artifact validation via inline `python -B -` exited 0: round-2 response
  schema, all F1-F3 entries, UTF-8/no-BOM and whitespace for all eight write paths.
  Repeated `cargo fmt --check` and scoped `git diff --check` also exited 0.
