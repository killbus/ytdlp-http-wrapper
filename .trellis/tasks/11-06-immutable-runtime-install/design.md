# Single runtime installation design

Unit of parent task 11-03-resident-worker-runtime-upgrade (rollout step 3,
parent design.md sections 6 and 8). Source baseline:
775bd5fa974f1008bb24e71ea92f674515706948 on
feature/resident-worker-runtime-upgrade. Scope: the amended prd.md and
review/owner-decisions.md S1-S4 of this unit, not the dirty implementation.

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
- Downloaded archives are retained under `runtime/artifacts/`
  and are the reuse-time integrity source. Extracted trees are produced
  from those verified archives during installation and are not re-hashed
  on every boot.
- `worker_script_sha256` participates in `runtime-id` derivation, so a
  wrapper build with a changed worker script requires a verified
  replacement at the same fixed runtime path, not reuse of the old worker.
- Offline tests use plan files with `file://` URLs. The wheel fixture is
  built with DEFLATE entries so the production path is exercised.

The subsequent single-installation amendment (S1-S4, prd reqs 2-4)
requires exactly one LIBS_DIR/runtime. Reuse it only when identity and
integrity match; otherwise prepare and verify a replacement in owned
staging before changing that fixed installation. Failed preparation
preserves the old installation but fails boot, with no old-runtime
fallback. Clean this invocation's stopped, owned staging even without a
manifest; never sweep another invocation's or a crashed boot's staging.

This explicitly overrides parent section 6's versions/<id> layout and
universal manifest prerequisite for temporary cleanup. Parent section 7's
generation, retained-history, and pointer infrastructure is excluded, not
reserved for later. Parent section 8's publication requirement is realized
as staged fixed-path replacement with the limits in section 8 below, not
an atomic overwrite of a nonempty directory. All unrelated contracts stand.
The former A7 is superseded, not approved. The round-4 review and old
implementation lock are historical; this revision needs a fresh review
and owner lock, and does not authorize implementation (S4).

## 1. Context and the two-artifact split

Boot today calls the Rust yt-dlp crate's LibraryInstaller
(src/main.rs:237-239) to place a standalone yt-dlp executable into
--libs-dir; the CLI backend spawns that path per request
(src/supervisor.rs:219). The resident backend instead spawns a Python
interpreter that imports the yt_dlp package in-process
(worker/ytdlp_worker.py:135), configured via python_path / worker_path /
python_path_entries (src/resident_worker.rs:33-41). Nothing today installs
that Python closure: resident deployments rely on a pre-existing local
Python environment, which this unit's installed runtime must replace.

The standalone executable and the Python package closure are two distinct
artifacts of the same upstream source, built by different channels:

| Artifact | Serves | Today | Upstream channel |
|---|---|---|---|
| yt-dlp.exe (PE, ~18 MB, libs/yt-dlp.exe header MZ) | CLI backend (spawn) | crate LibraryInstaller, unversioned "latest" release, optional checksum | GitHub release assets (yt-dlp/yt-dlp, e.g. yt-dlp.exe, yt-dlp_linux) |
| Python interpreter + yt_dlp package + deps | resident backend (import) | nothing (ambient local Python) | astral-sh/python-build-standalone release tarball + PyPI yt_dlp wheel |

This unit makes boot produce ONE fixed runtime/ directory containing BOTH
artifacts, pinned by version, so a single runtime-id binds them. Installed
files are not patched while serving; replacement is a pre-backend startup
operation. The CLI keeps its frozen spawn contract (section 9).

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
| src/runtime_installer.rs (new) | Requested runtime identity, artifact resolution (sources), owned staging, sha2 verification, zip extraction, fixed-path reuse/replacement (section 8), cleanup, structured errors, runtime-id metadata | new |
| src/main.rs | New CLI/env flags (section 7); boot calls ensure_runtime BEFORE backend construction; passes fixed resolved paths to AppState and Backend config; deletes the yt_dlp import (line 9), install_with_retry (starting line 108), and the LibraryInstaller boot block (lines 237-239) | rewrite of boot section |
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

Rooted at the existing --libs-dir (default libs/, .gitignored). Per S1/S3
and prd req 2 this replaces, rather than extends, parent section 6's
version-directory layout. bin/ supplies the standalone CLI artifact:

```text
LIBS_DIR/
  runtime/            # the only installed location
    manifest.json
    artifacts/        # downloaded archives retained for reuse verification
    bin/              # yt-dlp.exe (windows) | yt-dlp (unix)
    python/           # PBS install_only contents (exe + lib/ + site-packages/)
    worker/           # ytdlp_worker.py (copy of the repo file)
  staging/
    <staging-uuid>/   # in-flight preparation; never selectable
      (candidate runtime tree; may lack manifest until verified)
```

There is no versions/ store, generations/, active/previous pointer,
historical retention, or reserved future generation layer. runtime-id is
metadata, not a directory selector. Successful replacement leaves only
runtime/ and no work directory from this invocation. Prior crash residue
under staging/ is unselectable and is never automatically swept (S2).

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
re-release of a tag changes the SHA256 and therefore the id, requiring
verified replacement at runtime/, never unchecked reuse. Normalization:
yt-dlp version lowercased, whitespace-stripped; PBS python version taken
from the archive name (MAJOR.MINOR.MICRO), not from a mutable label.
Match the full manifest identity, including platform and every expected
digest, not just the abbreviated id hash. The id neither authorizes
reuse on its own nor identifies a retained filesystem version.

### ResidentWorkerConfig wiring (value change only)

- python_path      = runtime/python/python.exe (windows) | runtime/python/bin/python3 (unix)
- worker_path      = runtime/worker/ytdlp_worker.py
- python_path_entries = [runtime/python/site-packages] + any user-supplied
  --worker-python-path entries (additive; the runtime closure first)
- generation_id    = runtime-id (the frozen handshake/log field name;
  metadata only, with no generation manager or runtime selector)
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
with a changed worker script replaces runtime/ rather than reusing the old
worker. The copy makes the runtime self-contained (a later repo edit
cannot drift the installed runtime).

## 7. Configuration surface (additive flags)

| Flag / env | Default | Meaning |
|---|---|---|
| --runtime-yt-dlp-version / RUNTIME_YTDLP_VERSION | [ASSUMPTION A5] 2026.09.23 | Explicit yt-dlp version, binary AND wheel. Required semantics: the default keeps deployment continuity; it is a pin, never "latest" |
| --runtime-python-release / RUNTIME_PYTHON_RELEASE | 20261026 (A1) | PBS release tag (and thus CPython version) |
| --runtime-plan / RUNTIME_PLAN | none | Source plan file (section 6) |
| --libs-dir | libs | UNCHANGED; contains the single runtime/ and staging/; exclusive deployment precondition in section 8 |
| --worker-python / WORKER_PYTHON | runtime python | UNCHANGED flag, now an explicit override of the runtime interpreter (resident only) |
| --worker-path / WORKER_PATH | runtime worker copy | UNCHANGED flag, now an override (resident only) |
| --worker-yt-dlp-version / WORKER_YTDLP_VERSION | = runtime version | Resident validation pin. If set and different from --runtime-yt-dlp-version: boot error RUNTIME_VERSION_MISMATCH, without retry. If set equal: accepted (back-compat). If unset: uses the runtime version |
| --worker-pool-size, --worker-python-path, backend flags | unchanged | unchanged |

Existing resident deployments keep working: their
--worker-yt-dlp-version now also determines which runtime is installed
(same value as the explicit runtime version by the mismatch rule), and
their --worker-python/--worker-path overrides still win.

## 8. Install sequence, reuse, and fixed-path replacement

### Deployment precondition

[ASSUMPTION A8] One canonical LIBS_DIR is private to one wrapper service
instance, including its full serving lifetime, and lives on one local
filesystem with same-volume directory rename support. Deployment must
serialize stop -> await all backend/installer children -> start; no other
wrapper, external runtime consumer, or filesystem writer may use/change
that root during replacement. runtime/ and staging/ must not be separate
mounts, symlinks, or reparse points. The exact runtime/ path is reserved
for this installer's tree, even if its manifest is missing or corrupt.
This is an operator-enforced precondition, not a claimed automatic lock;
shared/rolling-start deployments need separate libs-dir roots. No lease,
PID/age inference, or concurrent-publication winner/loser protocol is
added. If interference or unsafe path type is observed, fail before any
further removal with RUNTIME_PUBLISH_FAILURE; do not overwrite the racer.

### Preparation and matching

ensure_runtime(root, plan, requested_versions) -> RuntimePaths:

```text
1. Validate flags and safe root layout (A8)              -> structured err
2. Resolve artifacts (plan file, or section 5 sources)   -> resolved list
3. runtime-id = derive(resolved) (section 4); installed path = root/runtime
4. REUSE CHECK: if runtime/ exists as the expected ordinary directory:
     a. parse schema-v1 manifest; match runtime_id, platform (OS/arch),
        yt-dlp/Python/release versions, full artifact identity/digests,
        and embedded-worker SHA256 against the requested plan/build
     b. hash every retained artifact against the resolved expected hash,
        plus the installed binary and worker copy; require expected
        Python/package/worker paths to exist (no extracted-tree rehash)
     c. match + valid -> return fixed paths; log runtime_reused;
        no artifact fetch and no staging creation
        missing/mismatched/corrupt -> log runtime_reinstall_required
        with reason; do not return paths; continue to preparation
   An unreadable manifest or missing retained file is not permission to
   reuse. Unsafe layout/interference instead follows A8's failure rule.
5. Create a unique staging/<uuid>/ and keep its creation record in this
   invocation (exact path and owning directory handle/guard).
6. For each artifact: download there, verify SHA256 + size, retain under
   artifacts/, extract with system tar / zip crate / binary copy, chmod.
7. Copy the embedded worker, verify its hash and required candidate paths.
8. Write manifest.json after verification; sync_all the file, sync the
   candidate directory on Unix, close files and await all writers/children.
   Any failure in steps 1-8: preserve runtime/ unchanged; clean only this
   attempt's quiescent staging; return the structured error, not paths.
9. Replace/publish to runtime/ using the transition below, never by copying
   individual candidate files over the installed tree.
10. After publication checks and owned cleanup succeed, return RuntimePaths
    { binary, python, worker, python_path_entries, runtime_id } rooted at
    runtime/, not at staging/ or at an identity-derived directory.
```

Plan/build identity is resolved before inspecting the installed manifest;
corrupt installed metadata cannot redefine the expected identity. The
retained-archive policy is unchanged: it does not detect arbitrary edits
inside an otherwise present extracted Python tree, which is not re-hashed
on reuse (section 0). Matching overrides in section 7 do not bypass this
installation check.

### Publication without a saved old runtime

[ASSUMPTION A9] The minimal replacement accepts a pre-admission gap:
after the entire candidate is verified, retire/remove runtime/ in place
and rename the candidate to that now-absent path. No old-directory backup,
restore transaction, or selectable history is created. Preparation failure
preserves the old bytes; once retirement starts, publication failure or
abrupt death may leave no installation or an unusable remainder. Recovery
is a fresh staged install on a subsequent boot, never an old-runtime
fallback. This loss of post-preparation availability needs owner review.

On both Windows and Unix, use these ordered transitions:

1. Recheck that runtime/ is the originally inspected path (or still absent)
   under the exclusive-root precondition. Recheck containment and path
   type before removal. All candidate handles and child processes are
   closed/stopped; no backend has started.
2. If runtime/ exists, first remove its manifest.json (absence is already
   invalid); on Unix sync that directory before deleting any other file.
   If manifest invalidation or its sync fails, do not delete further
   files. Removing the manifest before tree deletion prevents an
   interrupted removal from later looking like a complete installation.
3. Remove the retired runtime/ tree without following symlink/reparse
   targets; these entries may be unlinked, not traversed. Stop on error.
   Recheck the target is absent. If it is not, do not call rename or
   attempt to merge trees. No existing nonempty target is overwritten.
4. Rename the verified staging directory to the absent runtime/ path on
   the same filesystem; sync LIBS_DIR on Unix. Windows sharing/permission
   errors and Unix removal/rename/sync errors are surfaced, not worked
   around with copy-over or directory-exchange tricks. A successful
   same-volume rename publishes the complete candidate namespace; the
   preceding delete + rename sequence is NOT one atomic transaction.
5. Recheck installed identity/retained artifacts/binary/worker as in step 4
   of preparation. Only then return the fixed paths. On a failed check,
   fail boot; no backend may consume the published but invalid tree.

| Failure boundary | Filesystem result and boot outcome |
|---|---|
| Candidate download/extract/hash/manifest-sync failure | Old runtime bytes unchanged (or runtime absent); clean owned staging after quiescence; return the original error unless cleanup fails |
| Old-manifest invalidation/sync, retired-tree removal, or candidate rename fails | RUNTIME_PUBLISH_FAILURE; no paths/backend; runtime may be unchanged, invalid/partial, or absent depending on the phase; clean this attempt's quiescent candidate; no restore/fallback |
| Rename succeeds but root sync fails | RUNTIME_PUBLISH_FAILURE; complete candidate may be at runtime/; do not delete it as staging or return paths |
| Post-publication integrity check fails | RUNTIME_INTEGRITY_FAILURE; retain runtime/ for the next boot's validation/reinstall; no paths/backend |
| Owned staging cleanup fails or quiescence/ownership is uncertain | RUNTIME_STAGING_FAILURE with primary cause and path/phase; retain uncertain work, no automatic retry or successful boot |

Errors include the publication phase and the observed runtime state
(old/invalid/absent/candidate/unknown); unknown is not success. No error
after retirement is automatically retried inside this boot. A new boot
uses the same step 4 validation and prepares anew if runtime is absent,
invalid, or mismatched; it never accepts a partial tree merely because
the path or an abbreviated runtime-id exists.

### Owned staging cleanup and abrupt death

S2/prd req 3 supersedes A7: before removing a failed attempt's staging,
stop and await all its download/extraction/write tasks and children.
Use the current invocation's creation record and rechecked containment
under the canonical staging root; a marker, manifest, PID, or age is not
ownership authority. Unlink links without following them. A completed
manifest is NOT required, so a partial failed download is reclaimed.
Never sweep other invocation/crashed-boot staging. If ownership or
quiescence is uncertain, preserve the path and report
RUNTIME_STAGING_FAILURE. Explicit cleanup errors are logged, not hidden
in Drop; disarm automatic deletion when work must be preserved because
ownership or quiescence is uncertain. After a successful rename the
candidate is no longer staging; disarm its temporary cleanup guard so
runtime/ cannot be deleted by it.

Abrupt process death runs no reliable cleanup. Before retirement it may
leave old runtime plus unselectable staging; after manifest invalidation
it may leave an invalid old remainder; between deletion and rename it
leaves runtime absent; after rename it may leave the complete candidate.
Next boot validates only runtime/ against its own requested identity and
does not adopt, rename, or remove prior staging. Staging residue requires
explicit operator cleanup after proving inactivity; no GC is designed.
Manifest/file sync and Unix directory sync do not establish a portable
power-loss transaction. After host/power failure, trust only a fresh
successful validation under the retained-archive policy, not the last
process's progress. This design does not promise old-or-new crash atomicity
or full extracted-tree bit-rot detection.

Boot ordering: parse flags -> ensure_runtime -> build_backend ->
backend.start().await? -> AppState::new(runtime_paths.binary, ...) ->
listener bind. Retry at most 3 attempts with 2s/4s backoff only for
RUNTIME_DOWNLOAD_FAILURE before retirement, and only after owned cleanup
succeeds; a cleanup error supersedes retry. All other section 10 codes
fail immediately. No old mismatched runtime, other backend, or alternate
installer may be used to make a failed boot appear successful.

## 9. Compatibility (frozen contracts)

- CLI backend: state.binary_path is set from the runtime's
  runtime/bin/yt-dlp(.exe); supervisor spawn, temp-dir, output cap,
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
  section 10 requires a generation/identity field on request logs. The
  field is metadata only; paths always refer to the single installation.
- HTTP API, RunResponse shapes, tests/run_contract.rs,
  tests/backend_contract.rs: unchanged (AC5). run_contract.rs already
  uses a non-existent binary path directly through AppState::new and
  never touches the installer; backend_contract and resident suites use
  their existing fixture injection points (contract_echo.py,
  resident_worker_runtime.py) and are constructed in-process without
  boot, so they are unaffected by the installer's presence.
- Layout breaking change (ops note, not an API change): libs/yt-dlp.exe
  (flat, crate-installed) is no longer produced or read. Neither that
  legacy file nor an earlier prototype versions/ store is adopted or
  automatically removed. Use a dedicated root satisfying A8; if legacy
  history exists, the operator removes/moves it while all consumers are
  stopped, or chooses a fresh root, before deployment. Do not blanket
  delete libs/ during boot. Document the fixed layout, A8/A9 maintenance
  and failure limits in the delivery commit message and README.

## 10. Error model (structured, actionable, pre-admission)

Boot errors are returned by main as today (Box<dyn Error>, exit code 1)
but carry stable codes for grep-ability and for the AC assertions:
only pre-retirement RUNTIME_DOWNLOAD_FAILURE with successful staging
cleanup is retryable (section 8); all other codes are non-retryable.

| Code | When |
|---|---|
| RUNTIME_CONFIG_INVALID | flag validation (empty version, bad release tag), plan parse failure or absent/unusable plan sha256 (section 6); worker-version mismatch is excluded |
| RUNTIME_VERSION_MISMATCH | --worker-yt-dlp-version set and != --runtime-yt-dlp-version |
| RUNTIME_CURL_MISSING | curl or tar binary not found; detail identifies tool=curl or tool=tar; not a download/extraction error |
| RUNTIME_DOWNLOAD_FAILURE | curl non-zero / timeout / oversize (metadata or artifact transfer), file:// copy failure; excludes source 1's missing SUMS (attempt, url basename, curl exit status when available in detail; never proxy URLs or tokens in logs) |
| RUNTIME_RESOLVE_FAILURE | unusable/ambiguous release API or PyPI metadata (including no/many wheels); binary has no usable sha256 after API and SUMS exhaustion (including SUMS HTTP 404), PBS API has no usable sha256 without a plan, or selected wheel digests.sha256 is absent/unusable (section 5); plan validation uses RUNTIME_CONFIG_INVALID, other transfer failures use RUNTIME_DOWNLOAD_FAILURE |
| RUNTIME_INTEGRITY_FAILURE | candidate or post-publication checksum/manifest/worker validation fails; an existing runtime's mismatch/corruption instead logs runtime_reinstall_required and enters preparation, with the eventual preparation/publication result returned |
| RUNTIME_EXTRACT_FAILURE | tar non-zero, zip crate read/extract failure, unsafe archive entry path |
| RUNTIME_PERMISSION_FAILURE | chmod failed (unix) |
| RUNTIME_PUBLISH_FAILURE | unsafe installed layout or observed interference; candidate manifest write/file-or-directory sync, old-manifest invalidation/sync, retired-runtime removal, rename-to-absent target, or post-rename root sync fails; include phase/state, no race retry or old-runtime fallback |
| RUNTIME_STAGING_FAILURE | staging creation/removal IO failure, uncertain ownership/containment or writer/child quiescence; cleanup failure takes precedence with the original cause retained in detail; preserve uncertain paths |

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
| install_happy_path_then_reuse_without_fetching | AC2 | runtime/ has manifest.json, artifacts/, bin/, python/, worker/; second run returns identical fixed paths with no fetch or staging creation; no history is created |
| deflated_wheel_entries_are_extracted | AC2 (integrity) | a DEFLATE wheel entry is readable as site-packages/yt_dlp/version.py with the expected bytes |
| identity_change_replaces_same_path | AC3 | install A, then request B; separate cases change yt-dlp, Python version/release, artifact digest, and worker identity; B replaces runtime/, returned path strings stay fixed, requested full identity is installed, and no old usable tree remains |
| mismatched_manifest_reinstalls | AC3 | after a valid install, independently change manifest runtime_id, platform, Python identity, artifact digest, or worker hash; unchanged original plan causes preparation and replacement with the correct manifest, not reuse |
| retained_archive_or_worker_corruption_reinstalls | AC3 | separately corrupt runtime/artifacts/<python-archive>, binary copy, or worker copy while leaving fixture sources/plan unchanged; refetch and verification repair through whole-directory replacement at runtime/ |
| failed_preparation_preserves_prior_runtime | AC3/4 | for both mismatched and corrupt installs, failed source fetch or candidate checksum/extraction preserves a pre-attempt snapshot of runtime/, returns the relevant error, and never starts a backend or returns old paths |
| missing_local_source_fails_before_admission_and_recovers | AC4 | without an old runtime, missing file:// source fails RUNTIME_DOWNLOAD_FAILURE after bounded retry; runtime/ stays absent; owned staging is gone; corrected source permits a later boot |
| failed_staging_without_manifest_is_cleaned | AC4 | candidate failure before manifest creation stops/awaits its writers/children then removes only this attempt's directory; marked/unmarked foreign and crash-residue staging remains byte-identical |
| publication_and_cleanup_failure_boundaries | AC4 | exercise section 8's failure table and interrupted states as detailed below; no error returns usable paths or admits a backend |
| dependency_graph_has_no_yt_dlp_or_lofty | AC1 | cargo tree --offline --locked -p ytdlp-http-wrapper (or cargo metadata --offline --locked --format-version 1) has neither package; Cargo.toml has no yt-dlp dependency/[patch.crates-io]; single-commit procedure below also applies |

For each successful replacement case, save the initial fixed paths and
an old-only sentinel under runtime/; after replacement assert the paths
are identical, expected manifest/archive/worker bytes match the requested
fixture, the sentinel is gone, and no versions/, backup, pointer file,
or owned staging remains. Change embedded-worker identity through
module-local fixture inputs to the same private identity/ensure logic
that production feeds with include_bytes!, never an environment override
or public production test hook. Manifest-mismatch cases also cover
platform and Python matching independently of an id-string comparison.

For retained-archive corruption, keep the original plan/hashes, fixtures,
flags, and worker build unchanged; the expected id remains unchanged.
The damaged install must be rejected for reuse, then reinstalled from the
verified fixtures (three artifact transfers, not an early reuse return).
For failed preparation, take a snapshot AFTER intentional corruption or
mismatch and BEFORE retry: relative paths, types, bytes, permissions,
modification times (not access times). After the error require exact
equality, including the corrupt bytes, and no runtime paths/backend.
Snapshot comparison is test-only; it adds no extracted-tree hashing to
reuse. With no old installation the same failures leave runtime absent.

Observe no-fetch reuse by removing/renaming source fixtures after the
first install and requiring reuse to succeed with unchanged staging and
installed-directory snapshots. For cleanup ordering use local stub
processes/ordinary internal preparation functions: a failed writer must
be stopped and awaited before its owned path is removed; foreign staging
with identical-looking markers is untouched. Exercise real filesystem
denials/locks in native Windows and Unix tests where deterministic. Any
remaining error branch injection is module-local under cfg(test), not a
production hook or a pluggable updater abstraction.

Publication/error assertions on both OSes (section 8 phase table):
- Candidate manifest-sync failure occurs before retirement, so the old
  snapshot is unchanged and candidate cleanup runs; the code is
  RUNTIME_PUBLISH_FAILURE unless cleanup itself fails.
- Old-manifest invalidation failure stops before any further removal;
  removal interrupted after invalidation leaves no completed manifest.
  Retired-tree deletion or rename-to-absent failure returns
  RUNTIME_PUBLISH_FAILURE, without restore or old-runtime fallback.
- A failed post-rename root sync (Unix) leaves the complete candidate
  but returns no paths; Windows does not claim Unix directory fsync.
  A post-publication validation failure is RUNTIME_INTEGRITY_FAILURE.
- Cleanup failure/uncertain quiescence yields RUNTIME_STAGING_FAILURE
  with original cause, prevents retry/admission, and preserves uncertain
  paths. Successful rename disarms the staging guard; dropping it cannot
  remove runtime/.
- Model abrupt-death states with filesystem fixtures: old runtime plus
  foreign partial staging, runtime without a manifest, absent runtime,
  and fully published candidate. The next invocation validates/reuses
  only a matching complete runtime, otherwise prepares anew; foreign
  staging remains untouched. These are restart-state assertions, not a
  claim to simulate power-loss durability or an atomic delete + rename.

AC1 graph commands are offline only with the needed local Cargo cache;
missing cache is a validation prerequisite, not evidence about the graph.
The package-scoped cargo tree checks this wrapper's dependency graph;
full cargo metadata is an allowed alternative, not a narrower graph claim.

AC1 single-commit procedure (local Git history, no network): after the
implementation review and gates, the owner creates and records delivery
commit C. The reviewer runs this check immediately after that commit and
before final acceptance/push; the design reviser does not commit. Require
`git merge-base --is-ancestor 775bd5fa974f1008bb24e71ea92f674515706948 C`
to succeed. Enumerate all relevant commits with
`git rev-list --full-history --reverse 775bd5fa974f1008bb24e71ea92f674515706948..C --
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

## 12. Assumptions (require owner confirmation at review)

A1-A6 are carried forward, not newly approved by S1-S4. A8-A9 are new
proposals, not consequences the owner has already authorized. No new
implementation lock is implied by this document.

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
- A7 (retired): the former completed-manifest prerequisite for cleaning
  this invocation's partial staging is superseded by S2/prd req 3, not
  approved. Current-invocation ownership/containment and quiescence,
  rather than a complete manifest, authorize temporary cleanup.
- A8: one private, operator-serialized libs-dir per wrapper instance for
  its entire lifetime; all prior consumers/children stop before startup;
  runtime/ is reserved installer-owned content and runtime/staging are
  ordinary directories on one local filesystem. No shared-root writers,
  rolling-start replacement, or automatic locking/lease service.
- A9: after candidate verification, invalidate the old manifest, remove
  the old fixed tree, then rename the candidate to the absent runtime/;
  no old-tree backup or restore. A gap or loss of the old installation
  after retirement starts is accepted only if the owner confirms it;
  failures reject boot and later recovery uses staged reinstallation.

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
  installation ownership, bloats the tree, and pre-fills runtime/ with
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
- Version-addressed installs, retained old directories, active/previous
  pointers, generation switching, retention/GC, or reserved extension
  points: contrary to S1/S3 and prd non-goals, not future infrastructure.
- Rename over a nonempty runtime/ or copy candidate files over it: the
  first is not portable Windows/Unix replacement, the second exposes
  mixed/partial trees and violates the staged-install boundary.
- Back up the old directory and build a crash-recovery transaction:
  unnecessary for the proposed A9 fail-boot/reinstall semantics; no
  retained selectable fallback or updater framework is introduced.
- Require a complete manifest to clean this attempt's failed staging,
  or sweep other staging by marker/age: both contradict S2's explicit
  current-invocation ownership and quiescence boundary.
