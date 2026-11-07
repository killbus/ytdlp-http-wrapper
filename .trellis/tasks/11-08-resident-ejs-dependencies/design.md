# Resident EJS repair design

## 1. Evidence and authority
Baseline: 643a5f0fcd9b07fbf378ae1449e4cded56d10a6f.
PRD R1-R6 and AC1-AC6 are the authority for this bounded follow-up.
The earlier 11-06 choice to install no optional extras is superseded specifically
by PRD R1: a bare Python distribution lacks a capability bundled in the standalone
binary. This is not a change to execution backend ownership.

Observed sources (2026-11-08):
- https://pypi.org/pypi/yt-dlp/2026.10.26/json declares
  yt-dlp-ejs==0.8.0 under extra == default (also under pin).
- https://github.com/yt-dlp/yt-dlp/wiki/EJS recommends yt-dlp[default] for PyPI
  installations and explains that official executable distributions bundle EJS.
- https://raw.githubusercontent.com/yt-dlp/yt-dlp/2026.10.26/yt_dlp/extractor/youtube/jsc/_builtin/ejs.py
  loads the Python package first, via yt_dlp_ejs.yt.solver.lib()/core(), before
  cache/builtin/remote sources; upstream owns solver version/hash compatibility.
- https://raw.githubusercontent.com/yt-dlp/ejs/0.8.0/yt_dlp_ejs/yt/solver/__init__.py
  implements these functions by reading packaged lib.min.js/core.min.js.
The incident log confirms missing usable solver and challenge failure; it does
not prove that all subsequent YouTube restrictions disappear after this repair.

## 2. Module boundaries, responsibilities and dependency direction
- src/runtime_installer.rs: boot-time preparation, identity, validation and
  publication orchestration. It calls uv and the embedded read-only Python probe.
- worker/probe_environment.py: observes the prepared Python environment and
  verifies required installed capability using package APIs; never installs.
- uv + upstream package metadata: sole authority for Python package resolution,
  extras and compatible EJS versions. No resolver implementation in Rust/Python.
- yt-dlp: controls extraction and JS invocation; Deno remains its executable
  dependency, not a wrapper-managed worker/service.
- scripts/runtime-boot-smoke.py and focused helper/tests: CI evidence only.
  Workflow YAML remains orchestration; smoke code does not enter production.
Direction: boot -> installer -> uv/probe; request -> backend -> yt-dlp -> Deno.
Forbidden: request -> installer; production -> tests/CI; probe -> package installer;
Rust selecting EJS versions; worker or supervisor owning dependency downloads.
Existing process-lifecycle and dependency-sources specifications remain binding.

## 3. Package installation contract (R1, R5; AC1)
Replace the bare requirement with yt-dlp[default]==<configured exact version>.
Use one requirement construction for installation and identity (section 4), so
these cannot silently diverge. Keep --no-build, isolation, explicit wheel inputs,
--offline/--no-index/--find-links and subsequent uv pip check as before.
An offline plan/wheelhouse must now provide the selected default dependency
closure. Missing wheels fail via the existing environment error, before retiring
the old runtime. Do not fall back to bare yt-dlp, latest, or request-time fetching.
Do not pin 0.8.0 in application code: that value is evidence for this release only.

## 4. Recipe identity and existing-volume upgrade (R2, R3; AC2, AC3)
Include the explicit Python installation requirement from section 3 as an extra
field in the identity hashed by derive_runtime_id (not a new manifest schema).
Keep existing schema-2 source plans/manifests readable. Old derived runtime IDs
lack this field and therefore fail the existing full manifest validation, even
when yt-dlp and the worker script are unchanged. This deliberately causes one
same-pin replacement through the existing staging/publication path.
Do not mutate the old runtime in place, erase it early, change the configured pin,
or require the operator to delete a volume. Preserve old runtime on preparation
failure. Repaired identity plus observed package inventory then supports the
existing no-network/no-uv reuse path. A first upgrade may need network or a complete
offline wheelhouse; subsequent valid reuse must not.

## 5. Required EJS capability probe (R3, R4; AC3)
Extend the existing isolated read-only probe, shared by candidate, reuse and
post-publication validation, to import yt_dlp_ejs and yt_dlp_ejs.yt.solver, require
EJS distribution metadata, check the imported package resolves inside the runtime
venv, and load nonempty lib()/core() resources via the package's own API.
The existing installed-distribution inventory records the EJS version. Do not
maintain version comparisons, metadata/extras parsers or script checksum tables;
uv resolution and upstream yt-dlp own those policies. Do not spawn Deno or access
the network from the production probe. Loading required resource files is a
capability check, not whole-tree cryptographic attestation.
Probe failure uses existing environment failure for candidates; reuse validation
maps it to integrity failure and rebuilds through the existing path. Do not add
an admitted-runtime bypass for tests, broken packages or missing resources.

## 6. Test and real-image evidence (R4-R6; AC1-AC6)
Offline Rust fixtures: model a separate fixture EJS wheel required through the
fixture yt-dlp default extra. Its solver API supplies small deterministic resource
fixtures, not real upstream code. All fixtures use host Python/uv without downloads.
Update affected fixture callers, preserving unrelated assertions. Test default
extra resolution, migration from a baseline (pre-new-field) identity, same-pin
repair, failure preserving old runtime, missing package/resources rejection or
repair, and subsequent unavailable-uv/source reuse. Test old-ID migration with
otherwise valid EJS present too, so a missing-module failure cannot mask ID logic.

Real hosted Docker: retain real entrypoint CLI provisioning, stopped first boot,
offline resident restart with missing uv, unchanged-manifest reuse and cleanup.
Add a small repository-owned CI helper executed with the installed venv interpreter
inside the offline resident container. Load both real EJS bundles using lib()/core(),
assemble them in upstream order (lib, Object.assign(globalThis, lib), core), and
execute with Deno via restricted stdin, no remote imports, no config and cached-only.
Require an explicit successful JS entrypoint-load result, not just deno --version
or package metadata. A deterministic marker checking typeof jsc === function is
sufficient: this gate proves the bundled code loads under Deno, NOT a real YouTube
challenge solution. It must fail on a missing/empty bundle or failed Deno process.
Keep this test under scripts/ with lightweight orchestration regressions; do not
embed scripts in YAML or add package/script fetching to request execution.
Negative coverage must show that bare-package environments and missing-script or
Deno-failure cases cannot satisfy the added gate. No user cookies or live YouTube
endpoint are needed for CI. Do not mislabel fixture evidence as deployment repair.

## 7. Delivery and rollout (R6; AC6)
Heavy Rust/uv/Docker work runs only in GitHub CI. Local work is formatting,
lightweight script tests/source review. Independent reviewer examines production
reachability, boundaries and regression assertions. Owner handles exact-path
commits, PR, gated squash merge, main CI, existing Docker publication and GHCR
index/platform/revision read-back. No force push, version tag, latest retag or
PVE deployment. Preserve the user's original worktree and unrelated dirty files.
Rollout requires stopping all users of the libs directory before recreation.
Preserve the volume, cookies mount, explicit yt-dlp pin and BACKEND=resident.
The first upgraded boot replaces the old recipe; there is no hot swap.

## 8. Non-goals and scope limit
No Deno daemon/pool or Python embedding, updater API, dependency solver, automatic
latest, remote-component default, new config surface, protocol/pool/supervisor
changes, image Deno pin changes, arbitrary cleanup, unrelated test weakening or
format/cookie changes. If a necessary repair violates this boundary, stop and
record the precise spec gap instead of broadening implementation.
