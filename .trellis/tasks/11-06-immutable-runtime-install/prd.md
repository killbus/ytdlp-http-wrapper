# PRD — Project-owned immutable yt-dlp runtime installation

Parent: 11-03-resident-worker-runtime-upgrade (design.md section 6, section 8,
rollout step 3: "Add project-owned immutable runtime installation and remove
the Rust crate and lofty patch in the same reviewed implementation").

## Background
Today boot calls the Rust `yt-dlp` crate's `LibraryInstaller` (src/main.rs:237)
to install the yt-dlp binary into `--libs-dir`; the crate exists only for that
and drags `lofty` (patched to a fork rev for a pastey fix). The CLI backend
executes the installed binary path; the resident backend needs a Python runtime
+ worker script + the yt-dlp binary. Rollout step 3 replaces the crate with a
project-owned installer producing immutable, integrity-verified runtime
installations, and removes the crate + patch in the same reviewed unit.

## Requirements
1. Project-owned installer, no `yt-dlp` crate. The crate dependency, all
   `LibraryInstaller` usage, the `lofty` patch, and transitive-only lockfile
   entries are removed in the SAME commit that makes the new installer the sole
   boot path (parent design section 8 "Remove").
2. Immutable runtime layout (parent design section 6), rooted at the existing
   `--libs-dir`:
   - `versions/<runtime-id>/{manifest.json, python/, worker/, integrity.json}`
   - `staging/` for in-flight preparation
   - `runtime-id` is derived from the normalized yt-dlp version + Python
     version + integrity metadata — never a mutable label.
3. Candidate preparation (section 6 steps 1-7): resolve an explicit yt-dlp
   version + Python runtime; download into a unique staging dir; verify
   checksum/integrity; install without mutating any existing version; write +
   fsync manifest and integrity metadata; atomically publish staging to
   `versions/<runtime-id>`; refuse to reuse a path whose manifest differs.
   Partial staging dirs are never selectable; cleanup only touches owned,
   manifest-validated paths.
4. Boot behavior: if a verified installation matching the requested versions
   exists, reuse it without re-downloading (section 8 "startup reuse");
   otherwise prepare one. Boot failure is an actionable structured error
   before admission; there is no silent fallback to any other installer.
5. Both backends source their executables from the installed runtime: the CLI
   backend keeps receiving a binary_path from boot (its existing contract is
   unchanged in shape); the resident backend's Python path + worker path come
   from the runtime directory. The public HTTP API (GET/POST /run, args,
   timeout_seconds, success/failure shapes) is unchanged.
6. Installer capabilities (section 8 list): platform/arch selection, explicit
   version resolution, bounded-retry with backoff, download size + timeout
   limits, checksum/integrity verification, executable permissions,
   proxy/certificate behavior (honor standard environment proxy variables; no
   custom CA store), atomic staging/publication, actionable structured errors.
7. Zero new Cargo dependencies. Downloads may shell out to the system `curl`
   (present by default on Windows 10+ and standard Linux images); a missing or
   failing curl is a structured boot error, not a silent code-path fallback.
8. Tests are deterministic and offline: the download step is exercised against
   a local fixture (file:// or a stub executable), never against the network;
   verification, publication, reuse, and failure paths are each covered.

## Acceptance criteria (mechanically falsifiable)
- AC1: `cargo tree -p yt-dlp-http-wrapper` (or `cargo metadata`) shows no
  `yt-dlp` or `lofty` package after the change; Cargo.toml has no `yt-dlp`
  dependency and no `[patch.crates-io]` section; the change and the installer
  ship in one commit.
- AC2: boot against an empty libs-dir with a local fixture source installs a
  `versions/<runtime-id>/` containing manifest.json, integrity.json, python/,
  worker/; a second boot with the same versions performs no re-download (test
  observes no staging dir created / a reuse log or counter).
- AC3: boot against a corrupt/manifest-mismatched existing `versions/<id>`
  refuses reuse (structured error or fresh install under a different
  runtime-id) and never mutates the existing directory in place.
- AC4: a failed download (fixture source that 404s / times out) fails boot
  before admission with a structured error carrying a code; no partial
  `versions/` entry is created; a subsequent successful boot recovers.
- AC5: CLI and resident contract tests remain green unchanged
  (tests/run_contract.rs, tests/backend_contract.rs, resident suite) — the
  executables they consume now come from the installed runtime or the same
  fixture injection point they use today.
- AC6: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `python -m unittest tests.test_ytdlp_worker -v` all green.

## Non-goals (explicit)
- Generation management (section 7): active/previous pointers, promotion,
  drain, rollback, retention, canary — separate unit after this one.
- Changing the public HTTP API or the CLI-backend subprocess contract.
- Changing the resident worker protocol or pool behavior.
- Vendoring upstream crate code or making a clone a build/runtime input
  (section 8: read-only research only).
- Removing the CLI backend (rollout step 7 is a later explicit decision).

## Owner constraints
- CLI backend stays default; resident stays explicit-config only; no silent
  fallback between backends (inherited hard rule).
- tests/run_contract.rs stays unchanged and green.
