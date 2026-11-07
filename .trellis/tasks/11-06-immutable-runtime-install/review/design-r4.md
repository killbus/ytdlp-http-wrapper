# Immutable runtime installation design

Unit of parent task 11-03-resident-worker-runtime-upgrade (rollout step 3,
parent design.md sections 6 and 8). Parent baseline: commit 7611b95 on
feature/resident-worker-runtime-upgrade. Scope: prd.md of this unit.

## 0. Owner amendment (2026-11-07)

The owner clarified that "zero new Cargo dependencies" (prd req 7) meant
"no new third-party yt-dlp library or downloader", not a ban on
general-purpose crates. This amendment supersedes any conflicting clause
below:

- `sha2` and `zip` (already present in Cargo.lock through the old
  `yt-dlp` dependency) are direct dependencies of this crate. The
  installer must not hand-write SHA-256, DEFLATE, ZIP container parsing,
  CRC32, base64, or wheel RECORD parsing.
- `manifest.json` is the single metadata file. `integrity.json` is
  removed: a second mutable copy of the same hash list under the same
  permissions is not an independent trust anchor.
- Downloaded archives are retained under `versions/<runtime-id>/artifacts/`
  and are the reuse-time integrity source. Extracted trees are produced
  from those verified archives during installation and are not re-hashed
  on every boot.
- `worker_script_sha256` participates in `runtime-id` derivation, so a
  wrapper build with a changed worker script installs a new runtime
  instead of colliding with an old directory.
- Offline tests use plan files with `file://` URLs. The wheel fixture is
  built with DEFLATE entries so the production path is exercised.

## 1. Context and the two-artifact split

Boot today calls the Rust yt-dlp crate's LibraryInstaller
(src/main.rs:237-239) to place a standalone yt-dlp executable into
--libs-dir; the CLI backend spawns that path per request
(src/supervisor.rs:219). The resident backend instead spawns a Python
interpreter that imports the yt_dlp package in-process
(worker/ytdlp_worker.py:135), configured via python_path / worker_path /
python_path_entries (src/resident_worker.rs:33-41). Nothing today installs
that Python closure: resident deployments rely on a pre-existing local
Python environment, which the parent design's immutable runtime must
replace.

The standalone executable and the Python package closure are two distinct
artifacts of the same upstream source, built by different channels:

| Artifact | Serves | Today | Upstream channel |
|---|---|---|---|
| yt-dlp.exe (PE, ~18 MB, libs/yt-dlp.exe header MZ) | CLI backend (spawn) | crate LibraryInstaller, unversioned "latest" release, optional checksum | GitHub release assets (yt-dlp/yt-dlp, e.g. yt-dlp.exe, yt-dlp_linux) |
| Python interpreter + yt_dlp package + deps | resident backend (import) | nothing (ambient local Python) | astral-sh/python-build-standalone release tarball + PyPI yt_dlp wheel |

This unit makes boot produce ONE immutable runtime directory that contains
BOTH artifacts, pinned by version, so a single runtime-id binds them and
the two channels can no longer drift. The CLI keeps its frozen spawn
contract (section 9).

Key verified facts:
- yt-dlp 2.7.2 crate's YoutubeFetcher resolves the latest GitHub release
  (api.github.com/repos/yt-dlp/yt-dlp/releases/latest) and selects the
  asset by platform name (yt-dlp.exe / yt-dlp_linux / ...); the asset
  digest is verified only when the API provides one. No version pinning,
  no fsync, no immutable layout.
- Removing the crate removes sha2, flate2, and zip from the transitive
  graph. The owner clarified that the dependency rule only bans a new
  third-party yt-dlp library, not general-purpose crates, so this unit
  promotes sha2 and zip to direct dependencies instead of reimplementing
  SHA-256 or DEFLATE (section 0 amendment).
- yt-dlp on PyPI declares no unconditional runtime dependencies
  (requires_dist is entirely extras-gated; verified via PyPI JSON API,
  version 2026.10.26). Its single-platform wheels are
  py3-none-any:yt_dlp-<version>-py3-none-any.whl, whose dist-info RECORD
  self-describes every installed file with its SHA256.
- python-build-standalone publishes self-contained CPython builds as
  cpython-<ver>+<tag>-<target>-install_only.tar.gz under a release
  named <tag> (verified: tag 20261026, e.g.
  cpython-3.13.18+20261026-x86_64-pc-windows-msvc-install_only.tar.gz).
  These bundles run with no host Python and no admin privileges; the
  archive root contains python.exe (Windows) / bin/python3 (Linux).

## 2. Module boundaries and responsibilities

| Module | Responsibility | Change |
|---|---|---|
| src/runtime_installer.rs (new) | Runtime plan + versions, artifact resolution (sources), staging, sha2 verification, zip extraction, publication (atomic rename), reuse check, staging cleanup, structured errors, runtime-id derivation | new |
| src/main.rs | New CLI/env flags (section 7); boot calls ensure_runtime BEFORE backend construction; passes resolved paths to AppState and Backend config; deletes the yt_dlp import (line 9), install_with_retry (lines 81-132), and the LibraryInstaller boot block (lines 237-239) | rewrite of boot section |
| src/backend.rs | Adds from_runtime_config(...) with the runtime-id as generation_id; the existing from_config* signatures remain for in-process tests and delegate with "resident"; resident validation keeps the no-fallback rules | additive, same rules |
| src/resident_worker.rs | Config fields unchanged; generation_id value becomes the runtime-id (set by caller) instead of the literal "resident" (src/backend.rs:98) | value only |
| src/supervisor.rs, src/executor.rs, src/routes.rs, src/worker_protocol.rs, worker/ytdlp_worker.py | Unchanged. CLI still spawns state.binary_path; worker protocol, handshake, pool semantics untouched | none |
| Cargo.toml | Delete yt-dlp = "2.7.2" and the entire [patch.crates-io] section; promote sha2 and zip to direct dependencies | replacement |
| Cargo.lock | Regenerated by cargo; yt-dlp, lofty, and their orphaned transitives drop out; sha2/zip remain as direct deps | regenerated |
| tests/runtime_install.rs (new) | Offline installer tests per AC2-AC4 (section 11) | new |

No changes to the public HTTP API, the CLI subprocess contract, the
resident worker protocol, or the pool.

## 3. Dependency direction

runtime_installer is a leaf module used only by main.rs:

  main.rs -> runtime_installer (planning + publication, boot-time only)
  main.rs -> backend -> resident_worker (runtime paths as data)
  main.rs -> routes -> supervisor (binary_path as data)

runtime_installer imports no other crate module (it owns its error type);
it never touches supervisor, executor, backend, or routes. It runs to
completion before any request is admitted and is never called from the
request path. Forbidden: runtime_installer importing crate::routes or
crate::backend; backend constructing its own installer.

## 4. Runtime layout and data model

Rooted at the existing --libs-dir (default libs/, .gitignored). This
EXTENDS parent design section 6 with a bin/ entry (the parent layout lists
python/ and worker/ but not the standalone binary; prd req 5 requires the
CLI binary to come from the installed runtime, so bin/ is added -- an
additive deviation, flagged for the reviewer):

```text
LIBS_DIR/
  versions/
    <runtime-id>/
      manifest.json
      artifacts/      # downloaded archives retained for reuse verification
      bin/            # yt-dlp.exe (windows) | yt-dlp (unix)
      python/         # PBS install_only contents (exe + lib/ + site-packages/)
      worker/         # ytdlp_worker.py (copy of the repo file)
  staging/
    <staging-uuid>/   # in-flight preparation; never selectable
      RUNTIME_STAGING_OWNER marker file  # not deletion authority by itself
      (partial artifacts)
```

No generations/, state/active.json, previous.json: generation management
is the next unit (parent section 7) and prd non-goal 1. With one runtime
version installed there is nothing to point at yet; when the generation
unit lands, the pointer layer sits above this layout unchanged.

### manifest.json (written after verification, fsync'd, part of identity)

```json
{
  "schema_version": 1,
  "runtime_id": "yt-dlp-2026.09.23-py3.13.18-pbs20261026-a1b2c3d4",
  "platform": { "os": "windows", "arch": "x86_64" },
  "versions": {
    "yt_dlp": "2026.09.23",
    "python": "3.13.18",
    "python_release_tag": "20261026"
  },
  "worker_script_sha256": "<hex of worker/ytdlp_worker.py as copied>",
  "artifacts": [
    {
      "name": "yt-dlp.exe",
      "kind": "binary",
      "source": "github-release",
      "repo": "yt-dlp/yt-dlp",
      "resolved_tag": "2026.09.23",
      "url": "https://github.com/yt-dlp/yt-dlp/releases/download/2026.09.23/yt-dlp.exe",
      "sha256": "<hex>", "size_bytes": 0
    },
    {
      "name": "cpython-3.13.18+20261026-x86_64-pc-windows-msvc-install_only.tar.gz",
      "kind": "python",
      "source": "python-build-standalone",
      "resolved_tag": "20261026",
      "url": "https://github.com/astral-sh/python-build-standalone/releases/download/20261026/cpython-3.13.18+20261026-x86_64-pc-windows-msvc-install_only.tar.gz",
      "sha256": "<hex>", "size_bytes": 0
    },
    {
      "name": "yt_dlp-2026.09.23-py3-none-any.whl",
      "kind": "wheel",
      "source": "pypi",
      "resolved_version": "2026.09.23",
      "url": "https://files.pythonhosted.org/.../yt_dlp-2026.09.23-py3-none-any.whl",
      "sha256": "<hex>", "size_bytes": 0
    }
  ]
}
```

The manifest is the single metadata file. There is deliberately no
second `integrity.json`: both files would live in the same writable
directory with the same permissions, so a second copy of the same hash
list is not an independent trust anchor.

### runtime-id derivation (deterministic, computed pre-download)

```text
runtime-id = "yt-dlp-<norm-ytdlp-ver>-py<pbs-python-ver>-pbs<pbs-tag>-<idhash>"
idhash     = first 8 hex of SHA256(canonical-JSON(artifacts[], worker_script_sha256))
```

where artifacts[] is the resolved list, each entry reduced to
{name, kind, source, resolved_tag, sha256} in fixed order (binary,
python, wheel), plus the worker script hash. The URL is deliberately EXCLUDED from identity: it is
provenance metadata (which endpoint served the bytes), not content.
Plan-file URLs are deployment-local (file:///tmp/... in tests), so
hashing them would make the same pinned versions produce different ids
on different machines; the sha256 is the content anchor, and an upstream
re-release of a tag changes the SHA256 and therefore the id (a fresh
install, never a silent swap of an existing directory). Normalization:
yt-dlp version lowercased, whitespace-stripped; PBS python version taken
from the archive name (MAJOR.MINOR.MICRO), not from a mutable label.

### ResidentWorkerConfig wiring (value change only)

- python_path      = versions/<id>/python/python.exe (windows) | versions/<id>/python/bin/python3 (unix)
- worker_path      = versions/<id>/worker/ytdlp_worker.py
- python_path_entries = [versions/<id>/python/site-packages] + any user-supplied
  --worker-python-path entries (additive; the runtime closure first)
- generation_id    = runtime-id (parent design section 10 wants a
  generation id on request logs; until the generation unit lands, the
  runtime id is the identity)
- expected_yt_dlp_version = --runtime-yt-dlp-version (the same value the
  binary was resolved with; the worker handshake then cross-checks the
  actually-imported package, src/resident_worker.rs:573-577)

## 5. Artifact resolution and download (capability per prd req 6)

### Sources

A usable SHA256 is 64 hexadecimal digits (after the release API's
`sha256:` prefix, where applicable). Missing, empty, non-string, or
malformed values are unusable; the source-specific outcomes below apply.

1. binary: GitHub release of yt-dlp/yt-dlp at the explicit
   --runtime-yt-dlp-version tag. Asset selected exactly like the old
   crate (yt-dlp.exe, yt-dlp_x86.exe, yt-dlp_linux, yt-dlp_linux_armv7l,
   yt-dlp_linux_aarch64, yt-dlp_macos) -- this table is the only place
   that knowledge lives. Checksum: prefer the release API's per-asset
   `digest`; if the API provides no usable one, download and parse the
   release's SHA2-256SUMS file. If neither source yields a usable
   sha256, boot fails RUNTIME_RESOLVE_FAILURE without retry. A missing
   SUMS file (HTTP 404) counts as checksum exhaustion, not a retryable
   download failure; other transport failures use the download rules
   below. The old crate's optional checksum behavior is deliberately
   rejected.
2. python: python-build-standalone release <tag>
   (default 20261026, [ASSUMPTION A1]) at the asset
   cpython-<ver>+<tag>-<target>-install_only.tar.gz. PBS publishes no
   third-party checksum file for release assets, so the expected SHA256
   comes from a runtime source plan (section 6) when the network source
   is used in tests, and in production from the operator: the plan file
   (--runtime-plan, section 6) is where the SHA256s live; a plan file
   takes precedence over any live source in BOTH tests and production.
   When no plan file is given, boot fetches the GitHub release API JSON
   for the tag (each asset carries the sha256 in the API response) and
   records that value in the manifest -- the API value is the
   upstream-attested digest. If the API response has no usable sha256
   for the PBS asset and no plan file is supplied, boot fails
   RUNTIME_RESOLVE_FAILURE -- an artifact is never installed without a
   verified sha256.
   [ASSUMPTION A2: API-attested sha256 is the production trust anchor
   when no plan file is supplied; no signature verification is in scope
   for this unit.]
3. wheel: PyPI JSON API (https://pypi.org/pypi/yt-dlp/<version>/json)
   provides the exact wheel URL and its digests.sha256. The installer
   requires exactly one py3-none-any wheel in the response; zero or
   several is RUNTIME_RESOLVE_FAILURE (fail closed on ambiguity).
   Exactly one wheel with an absent or unusable digests.sha256 also
   fails RUNTIME_RESOLVE_FAILURE without retry; it is never downloaded
   or installed without a usable expected digest.

### Download mechanics (owner amendment, prd req 7)

- Every download shells out to the system curl (curl.exe on Windows 10+,
  /usr/bin/curl on Linux; the Dockerfile already installs curl).
  Arguments: -fsSL --retry 0 --max-time <T> --max-filesize <N> -o <dst>
  <url>. Standard proxy environment (HTTP(S)_PROXY, NO_PROXY) and system
  CA store are honored via the environment; the installer adds no proxy
  flags and no custom CA. Missing curl is RUNTIME_CURL_MISSING with
  tool=curl in the detail, without retry. Non-zero exit (except source
  1's missing SUMS), timeout, or oversize is RUNTIME_DOWNLOAD_FAILURE;
  section 8 owns the bounded retry (3 attempts, backoff 2s/4s, mirroring
  the old install_with_retry), with the curl exit status when available
  in the detail. These transfer rules also apply to metadata requests.
- Size limit: --max-filesize and a post-download length check, default
  512 MiB per artifact [ASSUMPTION A3]. Timeout default 300s per attempt.
- tar.gz extraction shells out to the system tar (Windows 10+ ships
  bsdtar as tar.exe; Linux images ship GNU tar):
  tar -xzf <archive> -C <staging-subdir> --no-same-owner (unix only).
  Missing tar is RUNTIME_CURL_MISSING with tool=tar in the detail (the
  existing code covers either missing system tool), without retry.
  A non-zero tar exit is RUNTIME_EXTRACT_FAILURE, also without retry.
  [ASSUMPTION A4: system tar is available on both deployment targets.]
- The wheel is extracted with the `zip` crate (DEFLATE through
  `flate2`/`zlib-rs`). The installer verifies the wheel archive SHA256
  before extraction and relies on the ZIP implementation for container
  and DEFLATE correctness. There is no in-tree ZIP parser, SHA-256, or
  DEFLATE implementation.
- RECORD self-attestation is not duplicated in this unit: the wheel
  archive is already hash-pinned, and the ZIP implementation validates
  the archive during extraction. Supply-chain integrity comes from the
  pinned wheel SHA256, not from a file written by the same upstream.
  [ASSUMPTION A6] The pinned wheel plus ZIP extraction supplies the
  Python package closure; a future pin with hard runtime dependencies
  needs additional pinned wheel entries, not hand-written RECORD parsing.

### Executable permissions

Unix: after extraction, chmod 0o755 on the standalone binary and on
python/bin/* executables; failure is RUNTIME_PERMISSION_FAILURE. Windows:
no-op.

## 6. Runtime source plan (test seam + production pinning)

A JSON plan file (--runtime-plan / RUNTIME_PLAN, optional) overrides
resolution endpoints and pins expected hashes; it is the ONLY way tests
run offline (prd req 8) and the recommended production mechanism:

```json
{
  "schema_version": 1,
  "yt_dlp": "2026.09.23",
  "python": { "release_tag": "20261026", "python_version": "3.13.18" },
  "artifacts": [
    { "name": "yt-dlp.exe",
      "url": "file:///tmp/fixtures/runtime/yt-dlp.exe",
      "sha256": "<hex>" },
    { "name": "cpython-...-install_only.tar.gz",
      "url": "file:///tmp/fixtures/runtime/python.tar.gz",
      "sha256": "<hex>" },
    { "name": "yt_dlp-2026.09.23-py3-none-any.whl",
      "url": "file:///tmp/fixtures/runtime/yt_dlp.whl",
      "sha256": "<hex>" }
  ]
}
```

url prefixes: https:// (curl download) or file:// (local copy; tests).
The plan requires a usable sha256 (section 5) for every artifact (absent
or unusable -> RUNTIME_CONFIG_INVALID, without retry); a plan URL pointing
at a local file whose hash differs is the AC4 failure injection. Without
a plan file, resolution follows section 5. The plan influences identity
only through the sha256 values it pins: a plan pointing at different
content (different hash) produces a new runtime-id, while a different
endpoint serving byte-identical artifacts keeps the same id (identity
is content-bound, section 4).

Worker script source: the repo file worker/ytdlp_worker.py at build time
(include_bytes!); its SHA256 is recorded in the manifest, participates in
runtime-id derivation, and is re-verified against the copied file at the
end of staging and on reuse. Including it in the id means a wrapper build
with a changed worker script installs a new runtime instead of colliding
with an existing directory. The copy makes the runtime self-contained (a
later repo edit cannot drift the installed runtime).

## 7. Configuration surface (additive flags)

| Flag / env | Default | Meaning |
|---|---|---|
| --runtime-yt-dlp-version / RUNTIME_YTDLP_VERSION | [ASSUMPTION A5] 2026.09.23 | Explicit yt-dlp version, binary AND wheel. Required semantics: the default keeps deployment continuity; it is a pin, never "latest" |
| --runtime-python-release / RUNTIME_PYTHON_RELEASE | 20261026 (A1) | PBS release tag (and thus CPython version) |
| --runtime-plan / RUNTIME_PLAN | none | Source plan file (section 6) |
| --libs-dir | libs | UNCHANGED; now the runtime root |
| --worker-python / WORKER_PYTHON | runtime python | UNCHANGED flag, now an explicit override of the runtime interpreter (resident only) |
| --worker-path / WORKER_PATH | runtime worker copy | UNCHANGED flag, now an override (resident only) |
| --worker-yt-dlp-version / WORKER_YTDLP_VERSION | = runtime version | Resident validation pin. If set and different from --runtime-yt-dlp-version: boot error RUNTIME_VERSION_MISMATCH, without retry. If set equal: accepted (back-compat). If unset: uses the runtime version |
| --worker-pool-size, --worker-python-path, backend flags | unchanged | unchanged |

Existing resident deployments keep working: their
--worker-yt-dlp-version now also determines which runtime is installed
(same value as the explicit runtime version by the mismatch rule), and
their --worker-python/--worker-path overrides still win.

## 8. Install sequence and reuse

ensure_runtime(root, plan, versions) -> RuntimePaths:

```text
1. Validate flags (version non-empty, numeric-looking python release tag,
   worker-yt-dlp-version equality)                       -> structured err
2. Resolve artifacts (plan file, or section 5 sources)   -> resolved list
3. runtime-id = derive(resolved) (section 4)
4. REUSE CHECK: if versions/<id>/ exists:
     a. parse manifest.json, require runtime_id, platform, versions,
        artifact identity, and worker hash to match the current build
     b. re-verify every retained archive under artifacts/ against its
        SHA256, plus the worker script hash and binary copy
     c. pass -> return paths (log runtime_reused, no download, no staging)
        fail -> RUNTIME_INTEGRITY_FAILURE (directory left untouched,
                never repaired in place)
   If versions/<id>/ does not exist: continue.
   NOTE: a different requested version simply yields a different id and a
   second coexisting directory; nothing deletes old versions (retention
   is the generation unit).
5. STAGE: unique staging/<uuid>/ with RUNTIME_STAGING_OWNER marker;
   no cross-boot sweep. A marker alone proves neither manifest-validated
   ownership nor that the creating boot has stopped (cleanup rule below).
6. For each artifact (binary, python, wheel): download to staging,
   verify SHA256 + size; retain the archive under artifacts/; extract
   (system tar for Python / zip crate for the wheel / direct copy for
   the binary); chmod on unix. Any failure: abort, apply the cleanup
   rule below, return structured error, no versions/ entry created.
7. Copy worker script into staging/worker/, hash-verify.
8. Write manifest.json into staging, fsync file and directory (unix)
   before publication.
9. PUBLISH: rename staging/<uuid> -> versions/<id>. If the target path
   appears between steps 4 and 9 (two boots): the loser applies the
   cleanup rule to its own staging dir, then re-runs step 4 (reuse path).
   std::fs::rename is atomic within one volume; both are under LIBS_DIR.
10. Return RuntimePaths { binary, python, worker, python_path_entries,
    runtime_id }.
```

[ASSUMPTION A7] Staging cleanup is conservative: remove only a directory
created by this ensure_runtime invocation, still a direct child of the
canonical staging root (not a symlink/reparse point), after the attempt
is abandoned and all its download/extraction/write tasks and child
processes have stopped and been awaited. This is the only positive proof
of an inactive owner; another invocation's directory (including a prior
boot's) is treated as unknown/live, never inferred stale from a marker,
PID, or age, and is not removed. Before any removal, require both the
owner marker and a readable schema-v1 manifest whose runtime_id, platform,
versions, artifact identity, and worker hash match this invocation's
resolved plan/build (step 4a).
Missing/invalid manifests, partial staging, or uncertain quiescence mean
retain the directory and log the path/reason, not delete it. A removal IO
failure is RUNTIME_STAGING_FAILURE, logged with the original failure when
applicable. Step 8 still writes the manifest only after verification;
failed earlier attempts therefore leave unselectable staging behind.
This narrows automatic reclamation without deviating from prd req 3 or
parent section 6's owned, manifest-validated cleanup boundary; it adds no
cross-boot liveness protocol or marker-only exception.

Boot ordering in main.rs: parse flags -> ensure_runtime (with bounded
retry wrapper, 3 attempts with 2s/4s backoff on RUNTIME_DOWNLOAD_FAILURE
only; every other section 10 code fails immediately, including missing
tools and version/digest validation errors) ->
build_backend(cli, temp_dir, runtime_paths)
-> backend.start().await? -> AppState::new(runtime_paths.binary, ...) ->
listener bind. The HTTP server binds AFTER the runtime exists: there is
no window where admission is open without a verified runtime, and no
fallback to any other installer (prd req 4).

## 9. Compatibility (frozen contracts)

- CLI backend: state.binary_path is set from the runtime's
  versions/<id>/bin/yt-dlp(.exe); supervisor spawn, temp-dir, output cap,
  timeout and error matrix (process-lifecycle.md) are byte-for-byte
  unchanged. The standalone binary stays the CLI's contract: the CLI
  backend is a short-lived-per-request model where spawning a self-
  contained executable is simpler than booting a Python import per
  request, and unifying the CLI onto the package closure is rollout
  step 7's explicit decision, not this unit's.
- Resident backend: handshake (protocol_version, yt_dlp_version,
  python_version, generation_id, worker_id) unchanged; the worker now
  imports the runtime's yt_dlp, and its reported yt_dlp_version must
  equal the pinned version (existing check, src/resident_worker.rs:573-
  577). generation_id value changes from the literal "resident" to the
  runtime-id: no test or spec asserts the literal (verified: the string
  appears only in src/backend.rs:98 and log output), and parent design
  section 10 requires a generation/identity field on request logs.
- HTTP API, RunResponse shapes, tests/run_contract.rs,
  tests/backend_contract.rs: unchanged (AC5). run_contract.rs already
  uses a non-existent binary path directly through AppState::new and
  never touches the installer; backend_contract and resident suites use
  their existing fixture injection points (contract_echo.py,
  resident_worker_runtime.py) and are constructed in-process without
  boot, so they are unaffected by the installer's presence.
- Layout breaking change (ops note, not an API change): libs/yt-dlp.exe
  (flat, crate-installed) is no longer produced or read. Existing
  deployments must delete libs/ on first upgrade (the installer will not
  touch or clean the legacy flat file; it lives outside versions/ and is
  never owned by this unit). Documented in the commit message and
  README.

## 10. Error model (structured, actionable, pre-admission)

Boot errors are returned by main as today (Box<dyn Error>, exit code 1)
but carry stable codes for grep-ability and for the AC assertions:
only RUNTIME_DOWNLOAD_FAILURE is retryable (section 8); all other codes
are non-retryable.

| Code | When |
|---|---|
| RUNTIME_CONFIG_INVALID | flag validation (empty version, bad release tag), plan parse failure or absent/unusable plan sha256 (section 6); worker-version mismatch is excluded |
| RUNTIME_VERSION_MISMATCH | --worker-yt-dlp-version set and != --runtime-yt-dlp-version |
| RUNTIME_CURL_MISSING | curl or tar binary not found; detail identifies tool=curl or tool=tar; not a download/extraction error |
| RUNTIME_DOWNLOAD_FAILURE | curl non-zero / timeout / oversize (metadata or artifact transfer), file:// copy failure; excludes source 1's missing SUMS (attempt, url basename, curl exit status when available in detail; never proxy URLs or tokens in logs) |
| RUNTIME_RESOLVE_FAILURE | unusable/ambiguous release API or PyPI metadata (including no/many wheels); binary has no usable sha256 after API and SUMS exhaustion (including SUMS HTTP 404), PBS API has no usable sha256 without a plan, or selected wheel digests.sha256 is absent/unusable (section 5); plan validation uses RUNTIME_CONFIG_INVALID, other transfer failures use RUNTIME_DOWNLOAD_FAILURE |
| RUNTIME_INTEGRITY_FAILURE | usable expected checksum differs from bytes, manifest/runtime-id mismatch, retained archive mismatch, worker hash mismatch |
| RUNTIME_EXTRACT_FAILURE | tar non-zero, zip crate read/extract failure, unsafe archive entry path |
| RUNTIME_PERMISSION_FAILURE | chmod failed (unix) |
| RUNTIME_PUBLISH_FAILURE | rename failed, target appeared (after one retry loop), fsync failed |
| RUNTIME_STAGING_FAILURE | staging dir creation/removal IO failure |

Logging: tracing with runtime_id, artifact name (basename), attempt,
duration; credentials, proxy URLs, tokens are never logged
(parent section 10).

## 11. Validation plan (deterministic, offline -- prd req 8)

New tests/runtime_install.rs builds fixture source trees under
tempfile::TempDir. The installer never executes any installed artifact,
so fixtures need only be hashable files with correct names: the binary
fixture is a small text file named yt-dlp(.exe); the Python fixture is a
real tar.gz (built with the system tar in-test) containing
python/python.exe (a marker file) plus python/bin/python3; the wheel
fixture is a real DEFLATE wheel written in-test with the `zip` crate
containing yt_dlp/__init__.py and yt_dlp/version.py. A plan file
(section 6) points file:// URLs at the fixtures.

| Test | AC | Asserts |
|---|---|---|
| install_happy_path_then_reuse_without_fetching | AC2 | versions/<id>/ has manifest.json, artifacts/, bin/, python/, worker/; second run returns identical paths and the fetch counter stays at 3 |
| deflated_wheel_entries_are_extracted | AC2 (integrity) | a DEFLATE wheel entry is readable as site-packages/yt_dlp/version.py with the expected bytes |
| checksum_mismatch_fails_closed_without_publishing | AC3/4 | a wrong plan SHA256 fails RUNTIME_INTEGRITY_FAILURE and creates no versions/ entry |
| missing_local_source_fails_before_admission_and_recovers | AC4 | a missing file:// source fails RUNTIME_DOWNLOAD_FAILURE with no versions/ entry; the corrected plan then installs |
| worker_and_artifacts_are_bound_into_runtime_identity | AC3 | different artifact content produces a different runtime-id and both version directories remain isolated |
| retained_archive_corruption_rejects_same_id_without_mutation | AC3 | after a valid install, corrupt retained artifacts/<python-archive>; original-plan retry reaches step 4b, returns RUNTIME_INTEGRITY_FAILURE, and leaves the corrupt directory unchanged (procedure below) |
| manifest_mismatch_rejects_same_id_without_mutation | AC3 | after a separate valid install, replace manifest.runtime_id with a different string; original-plan retry reaches step 4a, returns RUNTIME_INTEGRITY_FAILURE, and leaves the mismatched directory unchanged (procedure below) |
| cargo metadata has no yt-dlp or lofty | AC1 | cargo metadata --offline --locked --format-version 1 parse; assert Cargo.toml text lacks [patch.crates-io] and yt-dlp; owner/reviewer also runs the single-commit procedure below |

For each new AC3 reuse-rejection case, start from a fresh fixture root,
install successfully, and record the returned id and paths. Corrupt only
the installed directory as specified in the row; keep the source fixture
bytes, original plan (including hashes), flags, and embedded worker build
unchanged. Snapshot the directory AFTER corruption and BEFORE retry:
relative paths, entry types, file bytes, permissions, and modification
times (not access times). Retry with that original plan; assert the
derived/diagnostic id equals the first returned id, no paths are returned,
and the structured code is RUNTIME_INTEGRITY_FAILURE. The fetch counter
stays at 3 and the staging-creation counter does not increase, proving
the retry enters section 8 step 4 rather than a different-id installation.
Compare the same snapshot after rejection for exact equality, including
the deliberately corrupt bytes/manifest; no repair, deletion, or new
versions/ entry is allowed. Snapshot comparison is test-only and adds no
reuse-time hashing of the extracted Python tree.

AC1 single-commit procedure (local Git history, no network): after the
implementation review and gates, the owner creates and records delivery
commit C. The reviewer runs this check immediately after that commit and
before final acceptance/push; the design reviser does not commit. Require
`git merge-base --is-ancestor c08f398 C` to succeed. Enumerate all relevant
commits with `git rev-list --full-history --reverse c08f398..C --
src/runtime_installer.rs src/lib.rs src/main.rs src/backend.rs Cargo.toml
Cargo.lock`, and inspect each patch with `git show -m <sha> -- <same paths>`
(including each parent of a merge).
Use `git diff C^ C -- <same paths>` and `git show C^:<path>` /
`git show C:<path>` to assert all of the following in that ONE transition:

- The installer module is introduced and wired into the crate and boot;
  ensure_runtime replaces the old boot installer as the sole path for
  both backends (section 2).
- All LibraryInstaller usage, its import, and the old install_with_retry
  helper disappear from src/main.rs; no fallback to them remains.
- Cargo.toml loses the yt-dlp dependency and the entire [patch.crates-io]
  section, including lofty, while adding the direct sha2/zip dependencies.
- The same commit includes the matching Cargo.lock delta: yt-dlp, lofty,
  and orphaned transitives disappear, and the root dependency list now
  binds sha2/zip directly (section 2; dependency-sources.md).

The parent tree must still have the old dependency/patch and boot path;
the resulting tree must have the complete replacement. Earlier commits
in the enumerated history must not already deliver any of these removal
or installer-introduction portions. A split removal/installer/wiring or
lockfile delivery therefore does not satisfy AC1 even if C's final tree
passes the metadata/text checks. Record C and the inspected deltas in
the review evidence; final-tree metadata alone cannot close AC1.

Existing gates (AC5, AC6): cargo fmt --check; cargo clippy --all-targets
-- -D warnings; cargo test (all suites); python -m unittest
tests.test_ytdlp_worker -v. CI unchanged shape: the rust matrix runs
purely offline (fixtures); the Linux smoke job keeps using its own
gh-downloaded official binary via YTDLP_SMOKE_BINARY (independent of the
installer).

## 12. Assumptions (owner-confirmed at review)

- A1: default PBS release tag 20261026 (CPython 3.13.18).
- A2: production trust anchor for the PBS tarball = the GitHub API's
  per-asset sha256 (or the operator's plan file); signature verification
  is out of scope.
- A3: 512 MiB per-artifact size cap, 300 s download timeout, 3 attempts
  with 2s/4s backoff for download-class errors only.
- A4: system tar exists on all deployment targets (Windows 10+ bsdtar,
  standard Linux).
- A5: default --runtime-yt-dlp-version 2026.09.23; a default is a pin
  chosen at review time, overridable, never "latest".
- A6: the pinned wheel plus ZIP extraction is sufficient for the Python
  closure (yt-dlp has no hard runtime deps, verified; if a future
  pinned version gains hard deps, the plan/manifest model extends with
  more wheel entries without layout change).
- A7: automatic staging removal is limited to this invocation's owned,
  manifest-validated directory after all writers/children stop and the
  attempt is abandoned. No cross-invocation/boot stale sweep; unknown
  liveness, partial staging, or invalid/missing manifests are retained.
  This trades crash-residue reclamation for the inherited cleanup safety
  boundary without adding locks, leases, or age-based deletion. This new
  assumption awaits owner confirmation at review.

## 13. Rejected alternatives

- Keep the crate and add a second installer: two sources of truth for
  the same upstream; the crate's unversioned latest-release behavior is
  exactly the drift this unit removes. Parent section 8 mandates removal.
- CLI backend switches to the Python package closure (python -m style):
  per-request cold start would pay the interpreter + import cost on
  every request; the CLI subprocess contract (supervisor, run_contract)
  is frozen for this unit; unification is rollout step 7's explicit
  decision.
- Vendor a Python interpreter into the repo / commit libs/: breaks
  immutability tooling, bloats the tree, and pre-fills versions/ with
  content the installer must own end-to-end.
- Download with a Rust HTTP client (reqwest/etc.): a large new transport
  dependency is not justified for three one-shot boot downloads; curl is
  present on every target and is what the crate removal frees us from.
- pip install into a venv during boot: pulls pip's resolver + index
  metadata at boot time, needs pip inside the PBS bundle, and cannot be
  pinned by artifact hash; the wheel-direct layout is fully hash-pinned.
- Hand-written SHA-256/DEFLATE/ZIP code: the generic `sha2` and `zip`
  crates are already present in the lockfile and are used directly; this
  unit does not reimplement cryptographic or compression algorithms.
