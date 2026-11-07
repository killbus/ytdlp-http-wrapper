# Implementation checkpoint

## Stage 1 - runtime recipe, capability probe and offline regressions

- Confirmed planning-lock HEAD `c2ce8adcbbe88ccd2b1ca1d4ad0113ee64954c84`
  and frozen design SHA256
  `76426AA7A6026E8C7EC860AC40EDD06B14B53149D8C727F9C1F283C98380426C`.
- Installation and derived identity now share `yt-dlp[default]==<explicit pin>`;
  schema 2, staging/publication, binary acquisition and request/worker boundaries
  are unchanged.
- The read-only Python probe requires EJS metadata, in-venv package/solver imports
  and nonempty package-provided lib/core resources, without Deno or networking.
- Shared offline fixtures now supply a separate synthetic EJS wheel selected by
  the yt-dlp default extra. Both shared-fixture callers supply a wheelhouse.
  No `src/main.rs` fixture caller requires changes.
- Added old-formula identity migration with EJS present and with a simulated bare
  environment, same-pin repair, failed preparation preserving the previous tree,
  missing-uv/source reuse, missing default wheel, and missing/empty EJS component
  rejection/repair regressions. Existing mandatory dependency and unrelated-extra
  assertions remain, with the expected inventory extended by EJS.
- Native Rust/uv assertions have not run locally; Ubuntu/Windows hosted CI is
  required. Formatting and lightweight Python gates remain for stage 3.
- Unfinished at this checkpoint: real-image offline Deno load helper, orchestration
  regressions, docs, local permitted checks and round-1 responses.

## Stage 2 - real-image offline bundle-load gate

- Added the CI-only `scripts/runtime-ejs-smoke.py` helper. It loads package API
  resources in upstream order, runs restricted stdin Deno without remote/npm
  imports/config/lockfiles/permission prompts, and requires both process success
  and an explicit function-entrypoint marker.
- Existing boot smoke retains the real entrypoint, CLI boot, clean first shutdown,
  offline resident restart with missing uv, exact manifest/log reuse checks and
  owned cleanup. It invokes the helper via isolated installed Python in that
  offline resident container before the second shutdown.
- Lightweight tests cover helper invocation, failure propagation/cleanup, missing
  package/metadata/resources, Deno errors/timeouts and false zero-exit evidence.
- README and scripts docs explain default extras, same-pin boot migration, full
  offline wheel closure and the distinction between bundle load and live challenge
  solving. Pins, workflows, Dockerfile and request handling remain unchanged.
- Unfinished at this checkpoint: permitted local formatting/Python tests, focused
  readability/diff review and round-1 responses. Native/uv/Docker execution remains
  exclusively a hosted gate; no deployment or live-video evidence is claimed.

## Stage 3 - local evidence and review handoff

- `cargo fmt --check`: first run exited 1 with formatting-only differences in
  newly added Rust assertions/tests. Ran `cargo fmt` (exit 0), then reran
  `cargo fmt --check` (exit 0). Only the assigned Rust files changed.
- `python -B -m unittest discover -s scripts/tests -p 'test_runtime_*smoke.py' -v`:
  exit 0, 13 tests. Deno and Docker invocations are mocked in these tests.
- `python -B -m unittest discover -s scripts/tests`: exit 0, 37 tests in 25.356s.
  Shell orchestration uses offline command stubs; printed intentional failure
  diagnostics are negative-test evidence, not real Cargo/Docker execution.
- `git diff --check -- src/runtime_installer.rs worker/probe_environment.py tests/fixtures/managed_runtime.rs tests/runtime_install.rs scripts/runtime-boot-smoke.py scripts/runtime-ejs-smoke.py scripts/tests/test_runtime_boot_smoke.py scripts/tests/test_runtime_ejs_smoke.py README.md scripts/README.md .trellis/tasks/11-08-resident-ejs-dependencies/review/implement-progress.md .trellis/tasks/11-08-resident-ejs-dependencies/review/responses.json`:
  exit 0. New files also receive a lightweight source/whitespace/JSON inspection.
- Final `cargo fmt --check` and the same scoped `git diff --check`: exit 0.
  `python -B -` with a read-only stdin inspection script: exit 0; checked new-file
  whitespace/final newlines, new Python AST syntax, exact response object keys and
  round-1 empty findings, and the frozen design SHA256. `git status --short`
  confirmed only assigned changes plus the two pre-existing untracked locations.
- Focused readability review kept the shared requirement constructor, one fixture
  wheel writer and a separate CI-only helper; no worker/protocol/lifecycle
  refactoring, resolver logic or production test bypass was introduced.
- Native Rust tests, Clippy, builds, uv provisioning and real-image Docker/Deno
  smoke have NOT run locally. Required pending gates are independent review,
  hosted Linux/Windows native checks and hosted real-image boot/offline EJS load.
  Git delivery, publication and registry revision verification remain owner work.
- Existing untracked `review/delivery-status.json` and `.trellis/workspace/codex/`
  were present at entry and left untouched. Frozen design/specs are unchanged.
- Round-1 responses record implementation scope without a self-review verdict.
  Stop here for independent review; no live YouTube/cookie/format result is claimed.
