# Approved default yt-dlp pin update

## Authorization and scope

On 2026-11-08 the user approved the coordinator's proposal to change the default
yt-dlp pin from `2026.09.23` to `2026.10.26`. This is the release-pin decision
left pending in M5 and `managed-python-release-pin-check.json`; those historical
records and the round-2 review are retained unchanged.

The official GitHub release and PyPI version endpoints returned 404 for the old
pin and 200 for the new pin. Required binary assets/checksum metadata and an
unyanked Python wheel exist in the new release metadata. See the exact URLs and
evidence limits in `managed-python-release-pin-check.json`.

Changes are limited to the Clap default and its default-value regression in
`src/main.rs`, current README configuration/plan examples, and script README
usage/availability guidance. Existing explicit flag/env override tests remain.
The real boot smoke inherits the application default; no independent CI version
selection, `latest`, fallback, resolver change or Python constraint is added.

Keep the former date in synthetic installer/transport fixtures and historical
records: those exercise explicit input handling and replacement with distinct
versions, not upstream availability or the current application default.

## Validation and delivery limits

The uv implementation has an independent round-2 code-review PASS closing F1-F3.
That review predates this narrowly authorized default/documentation delta and
must not be represented as an exact-hash review of the updated main/README files.
Native Rust/uv, release, lifecycle, Docker and actual boot gates remain pending
GitHub CI. Metadata availability alone is not live success. No staging, commit,
push, release or deployment is authorized by the pin approval.

## Executed lightweight checks

- `cargo fmt --check`: exit 0.
- Scoped `git diff --check -- src/main.rs README.md scripts/README.md`: exit 0.
- Read-only Python source/hash check: restoring the two old version literals in
  memory reproduces the reviewed main.rs SHA256 exactly. All other reviewed
  candidate hashes except the two intentionally updated README files still match.
- Parsed the README schema-2 plan and verified its version and wheel name/URL.
  Confirmed the smoke script has no duplicate default and the old synthetic
  fixture version remains. The first documentation check selected the unrelated
  first JSON example and raised KeyError; selecting the schema-2 block corrected
  the checker, and the rerun exited 0 without further product edits.
- Simplification pass: no abstraction or refactor needed for two literal changes.
- Spec-sync assessment for this pin-only follow-up: no new architecture rule;
  retain the existing explicit-input policy and record source evidence here.
  Broader managed-Python spec reconciliation remains separate delivery work.

No native compilation/tests, dependency provisioning or actual boot was run.
