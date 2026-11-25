# 支持 /run 原生在线更新 yt-dlp

## Goal

Make `/run` execute standalone `--update` and `-U` as native service updates.
The user approved proceeding, then explicitly requested a simple task without
Paseo. Work directly in the main session; no delegation/design-review loop.

## Requirements

- Update the managed standalone binary and Python runtime together to the latest stable yt-dlp release, retaining the existing response shape.
- Wait for active requests, pause new executions, restart resident workers after installation; keep the Rust HTTP service running.
- Report actual success, already-current, failure and timeout. Do not silently ignore unsupported update argument combinations.
- Preserve the old installation until the new worker starts; restore on failed activation. Persist successful selection across restart with the same configured pin.
- Keep one installed runtime; temporary backup is only for an unacknowledged update. No retained version history, extra HTTP endpoints, scheduled updates, deployment or publishing.

## Acceptance Criteria

- [x] Both aliases reach service update handling for CLI and resident backends; GET/POST retain exit_code/stdout/stderr.
- [x] An update cannot replace files while requests use them; update failure/timeout restores the previous worker and version.
- [x] Successful update reports old/new versions; already-current reports that explicitly.
- [x] Same deployment configuration reuses the selected online version after restart; an explicitly changed pin wins.
- [x] Regression checks cover dispatch, persistence and transactional recovery; existing process containment and output bounds remain in force.

## Notes

- Baseline: 5f66bb4fc0c0642c5d571b19089b2386140b9025 (main).
- Custom source plans and external worker/interpreter overrides explicitly reject online updates; managed official-source deployments support them.
- Standard 1..300s request budget applies (default 30s); update requests should use 300s. Cleanup and rollback may extend response latency.
- Real CLI acceptance exposed the existing Windows PyInstaller failure with verbatim TEMP paths; the scoped child-environment fix and a regression test are included.
