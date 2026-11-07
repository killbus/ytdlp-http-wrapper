# Implement notes -- 11-06-immutable-runtime-install

IMPLEMENTATION HOLD (2026-11-07): the owner approved a single installed
runtime, superseding the multi-version plan below. Read prd.md and
review/owner-decisions.md. A revised design, design review, updated stage
plan, and explicit new lock are required before this plan can be used.

Historical plan (superseded): design was locked with sha256
BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D.
Do not execute these historical stages against the new requirements.

## Read first (in this order, windows of ~100 lines, rg -n to locate)
1. design.md (all 13 sections -- it is the spec)
2. prd.md (AC1-AC6 are the exit criteria)
3. src/main.rs (flags ~20-172, boot ~174-280), src/backend.rs:53-120,
   src/resident_worker.rs:33-41
4. .trellis/spec/wrapper/backend/ (process-lifecycle.md, dependency-sources.md)

## Work in stages; after EACH stage run the gates and only then continue
Gates (all must be green before the stage counts as done):
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  python -m unittest tests.test_ytdlp_worker -v
Truncate gate output to the last ~40 lines in your notes.

- Stage 1: src/runtime_installer.rs core -- error type (design section 10
  codes verbatim), plan parse (section 6), runtime-id derivation
  (section 4, URL excluded from identity), staging create/sweep (marker
  rule), curl download helper (section 5 mechanics, file:// copy path),
  tar extraction via system tar, checksum verification (sha2), ZIP
  extraction (zip crate) for the wheel, worker script copy
  (include_bytes!), manifest.json write + fsync, atomic publish (section 8
  steps 9-10 incl. the loser-retries-reuse branch), reuse check
  (section 8 step 4, both AC3 branches).
- Stage 2: boot integration -- main.rs new flags (section 7),
  ensure_runtime before backend construction, backend.rs RuntimePaths
  signature change, resident config wiring (python_path/worker_path/
  python_path_entries/generation_id/expected version, section 4 wiring
  list), DELETE the yt_dlp crate usage (main.rs:9, 81-132, 237-239),
  Cargo.toml yt-dlp dep + [patch.crates-io] removal, sha2/zip direct
  deps, cargo lock regeneration.
- Stage 3: tests/runtime_install.rs per design section 11 table (all 9
  named tests, offline fixtures, plan file with file:// URLs, in-test ZIP
  writer for the wheel fixture). Verify AC5 suites are untouched and
  green.

## Hard rules
- Dependency rule: no third-party yt-dlp library. General-purpose crates
  already in the lockfile (sha2, zip/flate2) are allowed and required;
  do not hand-write SHA-256, DEFLATE, or ZIP parsing, and do not add a
  new HTTP client.
- No unsafe (crate lint forbids it).
- Do NOT git commit/push/merge -- the owner commits.
- Do NOT touch prd.md, design.md, approval.md, .trellis/spec/, or any
  frozen file. Never stage: todos/, response.json, .opencode/,
  .claude/, .trellis/spec/mpd/, .trellis/spec/myMPD/, docs/superpowers/,
  examples/, .tmp-*.log, target-gate/, __pycache__/.
- Windows/PowerShell: write JSON with
  [System.IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding $false))
  (Set-Content -Encoding UTF8 adds a BOM that breaks json parsing).
- Read files in 50-100 line windows; do not print whole large files.

## Report per stage (final message)
Stage done, gates green (paste the tail), files touched, any
deviation observed (with design.md section cited) or spec-gap.md written.
No verdict language (PASS/FAIL) -- the reviewer owns that.
