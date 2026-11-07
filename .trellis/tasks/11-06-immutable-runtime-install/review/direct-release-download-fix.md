# Direct release download repair

## Authority and scope

The deployment reported GitHub's "API rate limit exceeded" response during
runtime resolution. The user approved replacing API discovery with official
version-pinned release URLs, then explicitly requested direct inline repair
without another agent dispatch or design cycle.

Working baseline: 0f88e7c3853eb8ad0143f88d5047ac8a5593acea. Its file tree was
verified identical to merged main commit 50a342da13bd8c1494b0e41420df38f57ad2b102.
This bounded addendum supersedes API-first binary discovery in historical
design records; all unrelated contracts remain unchanged.

## Implementation

- Resolve the known platform binary from the configured release tag's official
  downloads and SHA2-256SUMS. No release API request, token configuration, latest
  fallback, new dependency or general download framework.
- Require one well-formed SHA-256 entry for the exact asset. Missing, malformed,
  ambiguous or non-UTF-8 checksum data fails before staging or binary download.
- Preserve checksum 404 as RUNTIME_RESOLVE_FAILURE; other HTTP/transport failures
  remain download errors under the existing bounded retry policy.
- Preserve binary byte verification, source-plan handling, manifest identity,
  recorded offline reuse, uv ownership, publication and worker lifecycle.
- Replace obsolete API-response tests with exact direct-download request tests,
  checksum fail-closed cases and existing HTTP/retry classification checks.

## Validation

- `cargo fmt --check` and scoped `git diff --check` exited 0.
- Source comparison against HEAD verified unchanged configuration/source-plan
  handling, identity/reuse, uv, transfer functions, byte verification and runtime
  publication. Obsolete API symbols and their fixtures are absent. Scope and
  UTF-8/LF checks completed successfully.
- A real curl GET to the official 2026.10.26 SHA2-256SUMS URL exited 0 without
  API use or credentials. Exactly one valid entry was observed for yt-dlp.exe,
  yt-dlp_linux and yt-dlp_linux_aarch64. This validates source availability and
  checksum format, not runtime installation or the deployment's network path.
- Native Rust/Clippy suites, real runtime boot and Docker execution remain CI
  gates; no local native build/test or runtime provisioning was performed.
- At the implementation checkpoint, no staging, commit, push, new image
  publication or deployment had been performed. The user subsequently authorized
  committing only this repair; push/publication remain pending. Pre-existing
  unrelated work remains untouched.
