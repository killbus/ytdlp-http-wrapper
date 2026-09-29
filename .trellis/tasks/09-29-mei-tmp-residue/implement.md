# Implementation Plan

- [x] Review current source, dependency APIs, task findings, and wrapper scope.
- [x] Record design and user authorization to implement.
- [x] Add tracked process supervision, owned temporary directories, bounded drains, and explicit termination.
- [x] Wire service shutdown and temporary parent configuration.
- [x] Add lifecycle regressions and Linux/Windows CI configuration.
- [x] Add Compose containment and deployment/operations documentation.
- [x] Run final fmt, clippy, tests, release build, and inline code review per Codex dispatch mode. Retry transient audit service timeouts if encountered.
- [x] Record evidence and remaining runtime limitations in progress.md; update spec.
- [ ] Execute Linux lifecycle, real-artifact and container runtime checks on a Linux runner.
- [x] Obtain the workflow-required commit-plan approval for the listed task files (user: "提交", 2026-09-29).

Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`, Trellis context validation. Linux-only tests must not be reported as executed on Windows.
