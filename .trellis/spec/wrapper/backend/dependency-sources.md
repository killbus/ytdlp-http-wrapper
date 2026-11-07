# Dependency Sources

- Keep Git patches pinned to a full commit hash and commit the matching lockfile.
- A successful locked/offline build only proves that the local cache is usable.
  When changing Git sources or investigating CI fetch failures, fetch the exact
  revision with a fresh Cargo Git cache before claiming the dependency is available.
- Prefer a reachable public upstream source when it serves the identical commit.
  Preserve the revision and package versions when only repairing a source URL;
  check all lockfile packages originating from that Git repository.
- The yt-dlp Cargo dependency and its lofty patch were removed together with
  their replacement installer. Do not reintroduce either as a runtime manager.
- Use CI for locked Clippy, tests and release builds; do not repeat heavyweight
  work locally. Local formatting and lockfile/source verification are sufficient
  before pushing a reviewed repair to the PR branch.
  Windows, Linux and Docker CI must resolve the corrected source; a failure before
  compilation is not evidence of a failing runtime assertion.

## Python Runtime Ownership

- Boot prepares the runtime before admission. CLI executes the standalone
  yt-dlp binary; resident workers use isolated CPython subprocesses. Runtime
  preparation must not move into a request handler or require rebuilding Rust.
- Deployment supplies a pinned uv executable. uv owns interpreter discovery,
  Python downloads and package resolution; Rust owns binary integrity and
  environment publication. Do not maintain PBS version tables, download metadata
  or Python dependency solvers in Rust. Requires-Python constrains compatibility,
  not a uniquely selected interpreter. Do not claim uv venv automatically searches
  interpreters satisfying all package dependencies.
- Install the explicit yt-dlp version with its upstream `default` extra, including
  EJS; uv and package metadata select compatible dependencies. Deno availability
  alone does not supply challenge solver scripts. Keep the exact installation
  requirement in runtime identity so a recipe change repairs existing volumes.
  Probe required EJS package/resources before admission; do not implement another
  dependency resolver, inject remote downloads into requests, or make Deno a pool.
  Offline inputs must supply the selected extra's dependency closure. Preserve
  valid no-uv/no-network reuse and prepare-before-retirement on same-pin upgrades.
- Real Docker boot verification must exercise the installed EJS bundles under
  offline Deno, not only `--version`. Distinguish bundle/entrypoint loading from
  live YouTube challenge solving, cookies, PO Tokens and format availability.
- Use the explicit yt-dlp request for both binary and package identity. Verify
  proposed release pins against both official GitHub and PyPI metadata. Never
  silently substitute latest or another release. Metadata availability and mock
  tests do not establish successful installation: retain real-entrypoint CI boot
  checks in addition to container containment checks.
- Runtime binary acquisition uses official version-pinned release download URLs
  and SHA2-256SUMS, not the anonymous GitHub release API. The configured version
  and platform asset name are already known; API discovery only adds an avoidable
  shared-IP rate-limit dependency. Require one valid checksum for the exact asset
  and verify downloaded bytes before publication; never fall back to unchecked
  installation. Keep explicit source plans and validated offline reuse unchanged.
- Reuse a validated recorded environment without network resolution; prefer its
  compatible stable base for updates. Keep base interpreters outside replaceable
  runtime/staging trees, and pass the canonical installation root explicitly to
  validation. Directory basenames are not proof of ownership or publication state.
- Fixed-root replacement still requires stop/await/start. It is not hot reload.
  Prepare and verify before retiring the old runtime; clean only owned quiescent
  staging. Preserve binary transfer/integrity assertions when removing obsolete
  Python resolver tests. Reject unsupported legacy inputs with migration guidance.
