# Delivery notes

Source baseline: 643a5f0fcd9b07fbf378ae1449e4cded56d10a6f.
Planning/review baseline: c2ce8adcbbe88ccd2b1ca1d4ad0113ee64954c84.
Design review: PASS round 1, agent 5f30520e-d1bd-494e-a8ec-8464b4d8e907.
Code review: PASS round 1, agent f8793478-61d5-47b2-bea4-cd5225e0039e.
All ten reviewed candidate file hashes match review/candidate-hashes.json.
No implementation changes follow code review. The coordinator's spec update
codifies design sections 3-6; it does not alter the implementation yardstick.

Local evidence: formatting and diff checks exited zero; implementer ran 37
lightweight script tests, coordinator independently reran 13 focused mocked smoke
tests. No native Rust/uv, real Docker, Deno or live YouTube test ran locally.
Hosted PR native/Docker checks are required before squash merge. Main CI, Docker
publication and registry/platform/revision verification follow merge. Until those
receipts exist, this document does not assert release acceptance.

Operations: preserve existing volume, cookies mount, yt-dlp pin and BACKEND=resident;
stop all old consumers before recreation. The first repaired boot may require
network or a full offline default-extra wheelhouse and replaces the old bare
recipe at the same fixed path. Subsequent valid reuse remains offline without uv.
There is no Deno service, hot-update endpoint, automatic latest, request-time
component download or cookie/format workaround. Smoke proves real EJS bundle load,
not this user's live video's challenge solution or format inventory.
