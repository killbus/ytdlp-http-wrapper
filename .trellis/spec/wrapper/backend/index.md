# Wrapper Backend Guidelines

This package is the root Rust HTTP wrapper, not `third_party/yt-dlp`.

## Pre-Development Checklist

- Read `Cargo.toml`, `src/executor.rs`, `src/routes.rs`, and `src/main.rs`.
- Read [Process lifecycle](process-lifecycle.md) for subprocess changes.
- Preserve `/run` request and response contracts and the 10 MiB output caps.
- Keep project code free of unsafe blocks; use safe library APIs.
- Use tracing for operational failures; never silently swallow cleanup failures.

## Quality Check

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`
- Run lifecycle tests on both Linux and Windows; compilation is not runtime evidence.
- Validate container mount options, concurrency/memory budgeting, and shutdown deadline when changing deployment defaults.
