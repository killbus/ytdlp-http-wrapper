# Managed Python: direct implementation authorization

## M0. Authority and scope

Date: 2026-11-08. Baseline: `658075b17d1d02475bc2506ba86d93caac03714c`.
Workspace: `D:/Repositories/ytdlp-http-wrapper`, branch `fix/pbs-cpython-selection`.

The user accepted this responsibility split: dependency declarations express
intent; package metadata constrains compatibility; a mature environment tool
resolves/prepares the Python environment; Rust supervises execution. The user
then explicitly requested direct implementation as a simple task, without a
new design-review cycle. This addendum is the operative authorization for this
bounded change. Do not edit or re-approve historical design/PRD/approval records.
The former `review/pbs-owner-decisions.md` P1 hardcoded-3.13 repair is superseded,
not an additional requirement. Its uncommitted implementation may be replaced.

This authorization overrides historical requirements for Rust-owned live PBS
selection, the fixed PBS default, and manual Python-package closure preparation
where they conflict with M1-M6. Unaffected contracts remain in force. Record
concrete legacy flag/plan compatibility decisions rather than silently ignoring
inputs. No new design agent, task hierarchy, generic framework, or delivery
approval is implied.

## M1. Python ownership

Use `uv` as the concrete mature tool for the managed Python/package path, after
checking its real documented commands and behavior. It is already available on
this Windows host (`uv 0.11.29`). Rust must not enumerate PBS releases, maintain
Python version tables, select a hardcoded minor series, implement PEP 440 / 508
resolution, or manufacture Python download URLs/digests. Tool packaging belongs
in deployment/CI configuration, not a Rust self-updater for uv.

The selected yt-dlp version remains an explicit dependency request. Its
Requires-Python / Requires-Dist, applicable extras, and the worker's genuine
compatibility requirements are consumed by the mature tool. Do not invent
constraints or enable optional extras without justification. Requires-Python is
a constraint, not an exact interpreter lock. Do not claim uv automatically
chooses a satisfying interpreter from every dependency unless its documented
entrypoint actually does that. Incompatibility must be actionable and fail
before admission, not trigger a custom search/fallback algorithm.

Prefer a recorded, validated compatible interpreter when preparing an update;
do not upgrade Python merely because yt-dlp changes. First-install discovery
and explicit overrides must follow uv's documented policy. An explicit Python
request, if necessary, is an external deployment input rather than a compiled
default version. Record the actual interpreter and installed distribution
identity. A package lock alone must not be presented as a full interpreter lock.

## M2. Preserved execution and update boundaries

- CLI still executes the installed standalone yt-dlp binary. Never silently
  replace it with `python -m yt_dlp` or an uv tool entrypoint.
- Resident workers remain isolated CPython subprocesses using the existing
  script/protocol, pool, cancellation and process containment. No PyO3,
  RustPython, in-process reload, pool rewrite, or HTTP contract change.
- Runtime preparation remains available at boot, before backend/admission;
  updates must not require recompiling Rust or rebuilding the image.
- Once a requested environment is prepared and validated, startup reuses it;
  no implicit `latest`, periodic self-update, or package resolution per request.
- Keep the private-root stop/await/start deployment rule. This change does not
  promise hot reload or zero-downtime activation. No version-history store,
  rollback service, leases or cross-boot scavenger is authorized.

## M3. Installation and compatibility

Keep the existing binary acquisition/integrity path and fixed runtime publication
behavior where possible. Prepare and verify before retiring the old installation;
preserve the old installation on preparation failure and clean only owned,
quiescent staging. Never delete a live runtime or a user's global uv cache.

For managed Python, let uv own distribution/download/package mechanics instead
of translating uv back into the old PBS resolver. Account for interpreter/venv
paths across staging publication: a venv, pyvenv.cfg, symlink, shebang or base
interpreter pointing into deleted staging is not a valid installation. Do not
assume `--relocatable` makes every base interpreter portable. Use one documented,
simple ownership arrangement and validate the published environment.

Preserve explicit legacy source plans when feasible as a pinned/offline input,
not as a second live version-selection strategy. If an input cannot be honored,
reject it explicitly with migration guidance; do not silently ignore it. Avoid
retaining dead PBS discovery code merely to keep obsolete tests unchanged.
Document any superseded manifest/source fields and migration behavior. No
production test-only hooks or parallel public installer APIs for fixtures.

## M4. Deployment

Supply uv in the Docker image using its official distribution and a deliberate
deployment pin; document native prerequisites and any supported executable
override. Do not bake the Python/yt-dlp environment into the image as the only
installation path. Preserve non-root execution, existing containment/mounts,
ffmpeg/deno behavior and process cleanup. A missing tool must fail clearly.

## M5. Verification

Local work is lightweight: formatting, diff/source inspection and lightweight
script/metadata checks. Rust builds, Clippy, complete tests, release and actual
Docker/runtime execution belong in GitHub CI. Do not install/download a real
Python/yt-dlp runtime on the user's machine or touch their deployed roots.

Cover delegated command inputs, explicit dependency selection, reuse, update
identity, incompatible requirements, missing/failing uv, preparation failure,
staging/publication path correctness and unchanged CLI dispatch. Distinguish
synthetic command fixtures from real-tool behavior. Add a bounded real boot
check to repository-owned CI scripts, referenced by thin workflow YAML, because
the current container containment smoke overrides the application entrypoint
and never proves installer startup. Preserve that containment smoke as well.
Tests must not remove/relax unrelated lifecycle or integrity assertions.

Public metadata read in the coordinator's latest turn returned HTTP 404 for
`https://pypi.org/pypi/yt-dlp/2026.09.23/json`. Treat the old default's availability
as unverified; do not fabricate live metadata, silently substitute a release,
or claim a successful boot. An unrelated release-pin change requires a clearly
reported, evidence-backed recommendation rather than an incidental edit.

## M6. Ownership and delivery

Only the implementer writes the authorized production/test/deployment paths.
The coordinator owns this addendum, the dispatch brief and baseline snapshot.
Historical records and unrelated dirty files are preserved. No staging, commit,
push, merge, tag, release or deployment is authorized. Independent code review
follows implementation; heavy validation and delivery remain separate steps.
