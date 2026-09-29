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
- [x] Diagnose PR #1 dependency-fetch failure and verify the identical pinned commit is downloadable from public upstream into an empty Git repository.
- [x] Switch the lofty patch and its three lockfile source entries to public upstream without changing versions or revision.
- [x] Verify locked Cargo fetching with an empty Git cache; fmt, Clippy, Windows tests and release build passed before moving further heavy validation to CI.
- [ ] Validate the updated PR on Windows/Linux and Docker CI.
- [x] Obtain the workflow-required commit-plan approval for the listed task files (user: "提交", 2026-09-29).

Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`, Trellis context validation. Linux-only tests must not be reported as executed on Windows.

User preference (2026-09-29): use CI for heavyweight builds and tests. Do not
start further local builds; review and lightweight checks remain local.
