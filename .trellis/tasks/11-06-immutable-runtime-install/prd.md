# PRD — Project-owned single yt-dlp runtime installation

Parent: 11-03-resident-worker-runtime-upgrade (design.md section 6, section 8,
rollout step 3: "Add project-owned immutable runtime installation and remove
the Rust crate and lofty patch in the same reviewed implementation").

## Owner amendment (2026-11-07, single installation)

The owner confirmed one installed runtime, reused when it matches the
configured requirements and replaced when it does not. There is no use case
for a multi-version store, historical retention, or runtime generation
switching. See `review/owner-decisions.md` (S1-S4).

This amendment supersedes the inherited parent section 6 version-directory
layout and manifest prerequisite for cleaning this invocation's temporary
staging. Parent section 7's generation/retention plan is not an installer
requirement or a reason to reserve infrastructure. Unrelated parent contracts
(backend selection, HTTP API, worker protocol, crate removal) remain intact.
The round-4 design PASS covers the earlier multi-version candidate, not this
amendment. A revised design review and lock are required before implementation.

## Background
Today boot calls the Rust `yt-dlp` crate's `LibraryInstaller` (src/main.rs:237)
to install the yt-dlp binary into `--libs-dir`; the crate exists only for that
and drags `lofty` (patched to a fork rev for a pastey fix). The CLI backend
executes the installed binary path; the resident backend needs a Python runtime
+ worker script + the yt-dlp binary. Rollout step 3 replaces the crate with a
project-owned installer producing one integrity-verified runtime installation,
and removes the crate + patch in the same reviewed unit. Preparation remains
separate from the installed runtime; the installed files are not patched while
a backend is using them.

## Requirements
1. Project-owned installer, no `yt-dlp` crate. The crate dependency, all
   `LibraryInstaller` usage, the `lofty` patch, and transitive-only lockfile
   entries are removed in the SAME commit that makes the new installer the sole
   boot path (parent design section 8 "Remove").
2. Single installed runtime, rooted at the existing
   `--libs-dir`:
   - `runtime/{manifest.json, artifacts/, bin/, python/, worker/}` is the only
     installed runtime, not a version-addressed directory collection.
   - `staging/` is for in-flight preparation only, not retained versions.
   - Matching includes the configured yt-dlp/Python versions, platform,
     artifact identity, and this wrapper build's worker script identity.
     A runtime identity may remain manifest/log/handshake metadata; it must
     not imply multiple installed versions or generation management.
3. Preparation and replacement: resolve explicit versions; download into a
   unique staging directory; verify integrity and finish installation there;
   write and fsync the manifest before publishing to the fixed runtime path.
   Do not alter an existing installation while preparing its replacement.
   A missing, mismatched, or corrupt installation must not be reused; prepare
   a verified replacement, or fail boot explicitly if that cannot complete.
   Replacement happens before backend startup, not during request serving.
   Successful replacement leaves one installed runtime and no historical
   version retained for rollback. The design must specify feasible replacement
   and failure semantics on Windows and Unix rather than assume an atomic
   overwrite of an existing nonempty directory.
   Partial staging is never selectable. On failure, stop and await this
   attempt's writers/children, then clean its owned staging without requiring
   a completed manifest. Ownership and containment must come from the current
   invocation's creation record, not just a marker or directory age. If
   quiescence or cleanup cannot be established, report that explicitly and
   preserve uncertain paths. Do not blindly sweep another invocation's or a
   prior crashed boot's staging directory.
4. Boot behavior: if a verified installation matching the requested versions
   exists, reuse it without re-downloading (section 8 "startup reuse");
   otherwise prepare and replace the single installation. A preparation
   failure leaves the prior installation unchanged but does not authorize
   starting an old mismatched or corrupt runtime. Boot failure is an
   actionable structured error before admission; there is no silent fallback
   to an old version, another backend, or another installer.
5. Both backends source their executables from the installed runtime: the CLI
   backend keeps receiving a binary_path from boot (its existing contract is
   unchanged in shape); the resident backend's Python path + worker path come
   from the runtime directory. The public HTTP API (GET/POST /run, args,
   timeout_seconds, success/failure shapes) is unchanged.
6. Installer capabilities (section 8 list): platform/arch selection, explicit
   version resolution, bounded-retry with backoff, download size + timeout
   limits, checksum/integrity verification, executable permissions,
   proxy/certificate behavior (honor standard environment proxy variables; no
   custom CA store), staged publication/replacement without exposing partial
   installs to a backend, actionable structured errors.
7. Dependency boundary: no third-party `yt-dlp` crate or replacement downloader
   may be introduced. General-purpose crates already present in the lockfile
   (for example `sha2` and `zip`/`flate2`) may be promoted to direct
   dependencies; cryptographic, compression, and archive algorithms must not be
   reimplemented by hand. Downloads may shell out to the system `curl` (present
   by default on Windows 10+ and standard Linux images); a missing or failing
   curl is a structured boot error, not a silent code-path fallback.
8. Tests are deterministic and offline: the download step is exercised against
   a local fixture (file:// or a stub executable), never against the network;
   verification, publication, reuse, and failure paths are each covered.

## Acceptance criteria (mechanically falsifiable)
- AC1: `cargo tree -p yt-dlp-http-wrapper` (or `cargo metadata`) shows no
  `yt-dlp` or `lofty` package after the change; Cargo.toml has no `yt-dlp`
  dependency and no `[patch.crates-io]` section; the change and the installer
  ship in one commit.
- AC2: boot against an empty libs-dir with a local fixture source installs a
  `runtime/` containing manifest.json, artifacts/, bin/, python/, worker/;
  a second boot with matching configuration and a valid installation performs
  no re-download (test observes no new staging / a reuse log or counter).
- AC3: changed requested versions, corrupt installed content, or a mismatched
  manifest never cause reuse of the old installation. Successful preparation
  replaces the fixed `runtime/` with the verified requested installation;
  the returned paths use the same location and no historical version store
  remains. If preparation fails, boot returns a structured error and the
  prior installation remains unchanged, with no old-version fallback. Tests
  cover both mismatch and corruption, including failed replacement.
- AC4: a failed download (fixture source that 404s / times out) fails boot
  before admission with a structured error carrying a code; no partial
  `runtime/` is exposed; this attempt's staging is reclaimed after writers
  stop, even if no manifest was written. Another invocation's staging is
  untouched. A subsequent successful boot recovers. Publication/cleanup
  failures follow the design's explicit error rules and never return usable
  paths for a partial installation.
- AC5: CLI and resident contract tests remain green unchanged
  (tests/run_contract.rs, tests/backend_contract.rs, resident suite) — the
  executables they consume now come from the installed runtime or the same
  fixture injection point they use today.
- AC6: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `python -m unittest tests.test_ytdlp_worker -v` all green.

## Non-goals (explicit)
- A multi-version store, historical retention/GC, active/previous pointers,
  promotion, runtime switching, rollback-to-history, or canary deployment.
  These are not assumed future requirements to reserve infrastructure for.
- Changing the public HTTP API or the CLI-backend subprocess contract.
- Changing the resident worker protocol or pool behavior.
- Vendoring upstream crate code or making a clone a build/runtime input
  (section 8: read-only research only).
- Removing the CLI backend (rollout step 7 is a later explicit decision).

## Owner constraints
- CLI backend stays default; resident stays explicit-config only; no silent
  fallback between backends (inherited hard rule).
- tests/run_contract.rs stays unchanged and green.
