# Implementation and delivery plan

1. Lock design after independent D1-D8 review; owner commits planning records and
   records the resulting BASE_COMMIT before implementation. Inputs remain frozen.
2. Implement installation requirement + identity and required EJS probe. Update
   affected shared offline fixtures and targeted migration/repair/reuse tests.
3. Extend reusable Docker smoke helper and its lightweight negative regressions;
   update README/scripts documentation without changing frozen spec files.
4. Run cargo fmt --check, scoped git diff --check and lightweight Python script
   tests locally. No cargo test/build/clippy, uv provisioning, Docker execution or
   YouTube/cookie requests locally. Record exact checks and pending hosted gates.
5. Independent code review; bounded fixes and closure review as needed. Owner
   updates applicable specs after review and checks consistency. Exact-path commit.
6. Push branch, create PR to main, wait for Ubuntu/Windows/Docker checks. Diagnose
   real failures without weakening tests. Squash merge only the tested PR head.
7. Verify main CI and Docker publication; read GHCR multi-arch index, platform
   manifests and image revision labels. Record durable delivery receipts; report
   immutable tag/digest and remaining PVE/video validation boundary.

Implementer appends review/implement-progress.md after steps 2, 3 and 4, and writes
review/responses.json. No subdelegation or Git delivery by role agents.
