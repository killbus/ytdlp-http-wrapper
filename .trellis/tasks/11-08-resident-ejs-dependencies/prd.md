# Resident EJS dependency completeness

## Authorization and incident
The owner requested investigation, a converged repair design, implementation/review,
and final Docker publication using the previous GitHub delivery flow. This includes
creating this bounded task and executing it after the design review, without a new
product-design round. No PVE deployment or cookie access is authorized or needed.

The reported resident /run request fails format selection. Its diagnostic request
reports yt-dlp 2026.10.26 (pip) API, deno-2.9.8, skipped remote solver components,
n-challenge failure, and only storyboard formats. Deno is present, but no usable
local EJS solver is available. A list-formats exit code of zero is not extraction
success. The original filtering requirements must not be weakened as a workaround.

## Requirements
R1. Resident installation must include the explicit yt-dlp version's upstream
recommended default dependencies, including EJS; uv selects compatible versions.
R2. Existing installations from the bare-package recipe must be upgraded at boot
without changing the yt-dlp pin or manually deleting a persistent volume.
R3. Valid repaired installations remain reusable without network or uv. Preserve
fixed runtime/, staging ownership, stop/await/start, and pre-publication validation.
R4. Detect missing EJS modules/resources before request admission and verify the
actual shipped EJS resources run with the image's Deno in offline Docker CI.
R5. Preserve binary acquisition/integrity, request contracts, CLI/resident routing,
worker lifetime/cancellation/isolation, deployment pins and thin CI workflows.
R6. Publish only after independent review and hosted native Windows/Linux and
Docker checks; verify the multi-architecture registry image and source revision.

## Acceptance
AC1. The uv command installs yt-dlp[default]==<explicit pin>; offline fixtures prove
that optional dependencies are selected from wheel metadata, not a hand-coded pin.
AC2. A baseline bare-installation identity cannot reuse; same-version repair installs
EJS and publishes one runtime. Failed preparation preserves the previous runtime.
AC3. After repair, an unchanged runtime reuses with uv/sources unavailable; missing
EJS package or bundled script cannot be admitted as a valid environment.
AC4. Hosted Docker uses the real entrypoint and retains CLI boot/offline resident
reuse checks; it additionally loads both package solver bundles and executes them
with Deno under no-network conditions, failing on missing/broken components.
AC5. Existing native/contract tests remain, and new fixtures/tests expose the
pre-fix failure rather than relaxing unrelated assertions.
AC6. PR CI is green before squash merge; main CI and Docker publication succeed;
GHCR tag/index/platform config revision is independently read back. Report the
remaining limitation: CI does not prove this cookie-dependent YouTube request,
PO Token availability, SABR behavior, or the video's AVC/HEVC format availability.

## Non-goals
No Deno service/pool, hot-update API, runtime installer in /run, remote-components
injection, Python/EJS version tables, package solver, broad refactor, changes to
user cookies/format selection, or automatic update to latest.
