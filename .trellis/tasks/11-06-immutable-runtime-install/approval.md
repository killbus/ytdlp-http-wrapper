# Approval -- 11-06-immutable-runtime-install

Design locked for implementation.

- artifact: .trellis/tasks/11-06-immutable-runtime-install/design.md
- design_sha256: 91FED83776BE05889E6AF3F47B1539D0EF8E28F2D7A5C7D908E8D417DF778107
- review: review/design-findings.json round 2, verdict PASS (D1-D8 all met)
- round 1: G1 (absent-checksum fail-closed for the no-plan PBS path;
  plan sha256 required) and G2 (A6 inline marker) -- both closed and
  re-verified by re-reading the revised sections.
- round 2 observation N1 (error-table row for missing SHA2-256SUMS)
  resolved by owner edit before locking (section 10 row now names the
  API-no-sha256 case; sums-file-missing stays RUNTIME_INTEGRITY_FAILURE
  per section 5 source 1).
- assumptions pending owner confirmation (do not block; confirm before
  promoting this unit's runtime defaults): A1 (PBS tag 20261026 /
  CPython 3.13.18), A2 (API sha256 trust anchor when no plan file),
  A3 (512 MiB cap / 300 s / 3 attempts), A4 (system tar present),
  A5 (default --runtime-yt-dlp-version 2026.09.23), A6 (wheel + RECORD
  suffices; no hard runtime deps).
- layout note: bin/ added to parent design section 6 layout (additive
  deviation, flagged in design.md section 4).

Locked at: 2026-11-06 (Asia/Shanghai).