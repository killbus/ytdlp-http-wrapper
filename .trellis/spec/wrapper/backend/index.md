# Wrapper Backend Guidelines

This package is the root Rust HTTP wrapper, not `third_party/yt-dlp`.

## Pre-Development Checklist

- Read `Cargo.toml`, `src/executor.rs`, `src/routes.rs`, and `src/main.rs`.
- Read [Process lifecycle](process-lifecycle.md) for subprocess changes.
- Read [Dependency sources](dependency-sources.md) for Cargo patch or build-source changes.
- Preserve `/run` request and response contracts and the 10 MiB output caps.
- Query deserialization tests assert extracted args and timeout values directly;
  verify execution/update routing in `tests/run_contract.rs`. Do not infer parsing
  success from a missing executable's HTTP 500 response.
- Keep project code free of unsafe blocks; use safe library APIs.
- Use tracing for operational failures; never silently swallow cleanup failures.

## Quality Check

Run heavyweight builds and test suites in CI. Keep local validation lightweight
(formatting, diff review and focused source/lockfile checks). The following checks
are required gates; they do not need to be duplicated locally.

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`
- Run lifecycle tests on both Linux and Windows; compilation is not runtime evidence.
- Validate container mount options, concurrency/memory budgeting, and shutdown deadline when changing deployment defaults.
- Release archives must include the target triple in their names so matrix jobs
  cannot overwrite another platform's binary. Verify all five platform assets
  and the multi-architecture `latest` manifest after a tag release.
