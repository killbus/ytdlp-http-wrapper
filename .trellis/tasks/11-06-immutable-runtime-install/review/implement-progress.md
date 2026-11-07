# Implementation progress

## Stage 1

Stage 1 in progress: `src/runtime_installer.rs` ~1397 lines, incomplete at
line 1397 (ends mid-expression). `src/lib.rs` registers the module.

Remaining:

- Finish the minimal ZIP reader: complete central-directory entry parsing,
  local-header lookup, stored/deflate decoding, CRC verification, and
  extraction into `python/site-packages/`.
- Parse wheel `dist-info/RECORD` (CSV, URL-safe base64/no-pad SHA256 + size)
  and verify every extracted wheel entry.
- Finish staging preparation helpers: source artifact sizing, Python `tar.gz`
  extraction, executable permissions on Unix, and SHA256 verification helpers.
- Write `manifest.json` and `integrity.json`, fsync files/directories where
  supported, and atomically publish `staging/<uuid>` to
  `versions/<runtime-id>`; handle loser-reruns-reuse on rename collision.
- Complete `RuntimePaths` construction and integration-compatible public
  surface for Stage 2.
- Run the four gates, fix compile/clippy/test failures iteratively, then
  append Stage 1 completion output.

## Owner correction and rewritten implementation (2026-11-07)

The owner clarified that the dependency rule bans only a new third-party
yt-dlp library, not general-purpose crates. The hand-written SHA-256 /
DEFLATE / ZIP / RECORD implementation was discarded.

Implementation now:

- `src/runtime_installer.rs` uses `sha2` for all digests and `zip` for
  wheel extraction. No cryptographic, compression, or ZIP parser is
  implemented in-tree.
- `manifest.json` is the only metadata file; downloaded archives are
  retained under `versions/<id>/artifacts/` and are the reuse-time
  integrity source.
- `worker_script_sha256` participates in `runtime-id`, so changed worker
  scripts install a new runtime instead of hitting a reuse collision.
- `src/main.rs` installs the runtime before backend construction and
  passes the runtime-id into the resident backend as `generation_id`.
- `tests/runtime_install.rs` covers install, reuse-without-fetch, DEFLATE
  wheel extraction, checksum failure, missing source recovery, and
  content-bound runtime identity.

Gate results (2026-11-07, Windows):

- `cargo fmt --check`: clean after `cargo fmt`.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: all suites green (`runtime_install` 5/5).
- `python -m unittest tests.test_ytdlp_worker -v`: 14/14.

No commit was made; the owner commits.


## Final stage report (2026-11-07)

All three stages are done against the locked design (sha256
BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D).

Files touched: src/runtime_installer.rs (new), tests/runtime_install.rs
(new), src/lib.rs, src/main.rs, src/backend.rs, Cargo.toml, Cargo.lock.

Gates (Windows, 2026-11-07) -- all green:

- `cargo fmt --check`: clean (applied `cargo fmt` first).
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: all suites green -- runtime_install 6/6, resident_worker_pool
  9/9, process_lifecycle 10 passed / 6 ignored, backend_contract 3,
  resident_backend 6, resident_worker 14, resident_worker_manager 4,
  run_contract 3, worker_protocol_interop 4, query 1, lib unit 6.
- `python -m unittest tests.test_ytdlp_worker -v`: 14/14.

Final deviations observed (all recorded in review/responses.json):

- design.md section 11 row "cargo metadata has no yt-dlp or lofty" (prd
  AC1): the graph is read with `cargo tree -p ytdlp-http-wrapper --offline`
  instead of `cargo metadata --format-version 1`. A full metadata resolve
  unpacks every registry source (it needs the dev-dependency closure);
  `cargo tree` answers from the committed Cargo.lock, so it is
  offline-deterministic and covers the whole transitive graph instead of
  only this package's direct dependencies. The row's Cargo.toml text
  assertions (`[patch.crates-io]`, `yt-dlp`) are implemented verbatim, and
  a Cargo.lock package-name check was added.
- design.md section 4: the bin/ layout entry is the design's own flagged
  additive deviation; implemented as written.
- review/responses.json was written with round = 1 and an empty
  finding_responses array (no review/findings.json exists for this unit).

No git commit was made; the owner commits.
## Single-runtime implementation authorized (2026-11-07)

The earlier stage reports describe the superseded multi-version prototype,
not acceptance of the new design. Design review single-runtime round 1 is PASS
at DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.
The user accepted A8/A9; owner-decisions S5 and approval.md record the lock.
A7 is retired. A1-A6 are not newly approved deployment defaults.

Plan and self-contained brief now split work into three bounded dispatches:
1. Core lifecycle and basic installer regressions.
2. Full acceptance/failure fidelity.
3. Boot/docs alignment and final gates/review.
Stage 1 is the only initial assignment. Code work has not resumed yet and no
new test result is claimed. No commit/push. All role dispatches temporarily
use codex/remote-magpie/auto/gpt-6-astra per the user's subsequent instruction;
other profile settings are unchanged. Prior plan/brief/responses are archived.

## Stage 1 checkpoint -- single-runtime core (2026-11-07)

Scope: Stage 1 of implement.md only. Native .agents/.codex Trellis helpers are
absent, so implementation ran directly, without additional agents. The locked
design SHA256 was verified as
DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59;
approval.md and owner-decisions S5 authorize A8/A9. No frozen input was edited.

Code changed in this dispatch:
- src/runtime_installer.rs: fixed canonical libs/runtime location; full
  manifest identity and retained-artifact/binary/worker verification; required
  Python/package paths; valid matching reuse creates no staging. Missing,
  mismatched or corrupt content enters preparation, not fallback.
- Candidate preparation verifies required paths before writing/syncing the
  manifest. Publication rechecks the inspected runtime, invalidates its
  manifest, syncs on Unix, removes the old tree without traversing links,
  requires an absent destination, renames, syncs the root on Unix and validates
  the published candidate. Windows syncs the manifest file, not directories.
  There is no backup, version store, loser-reuse protocol or lock service.
- Invocation-local OwnedStaging records creation and directory identity under
  the canonical root. Explicit cleanup checks containment and ownership, needs
  no manifest, does not sweep other staging, and surfaces cleanup failure with
  the primary cause. Child collection is awaited; uncertain collection keeps
  staging. There is no automatic Drop deletion. Successful rename disarms
  staging cleanup before root sync/validation can fail.
- Only download-class errors can retry (three attempts, 2s/4s backoff), after
  owned cleanup succeeds. Publication failures never take that retry path.
- Removed the prototype's public FetchCounter/ensure_runtime_with_counter
  testing seam. The public installer is exercised with production plan files.
  Mature sha2/zip and source-resolution logic were retained.
- tests/runtime_install.rs: fixed-layout assertions, unavailable-source reuse,
  same-path digest replacement with old-only sentinel removal, worker repair,
  missing-manifest/Python repair, exact old-tree snapshots on failed candidate
  hashing (both mismatched and corrupt installs), no partial runtime on failed
  initial preparation, manifest-free cleanup and byte-identical foreign staging.
  Existing DEFLATE and dependency assertions remain.

Focused evidence on Windows (actual commands and exits):
1. rustfmt --edition 2021 src/runtime_installer.rs tests/runtime_install.rs:
   exit 0, twice; no other source was formatted.
2. cargo fmt --check: exit 0, twice (including after the last code edit).
3. cargo test --locked --test runtime_install: initial exit 0, 10 tests;
   final exit 0, 11 tests (7.02s reported). Fixtures are file:// sources, a real
   system-tar archive and a DEFLATE wheel. Reuse succeeds after renaming fixture
   sources away and preserves the full libs snapshot (including staging mtime).
4. cargo test --locked --lib runtime_installer::tests: exit 0 on both runs;
   final 3 tests, 6 unrelated lib tests filtered out (0.01s reported). These
   cover manifest-free cleanup, preservation on guard drop, and rejection of
   cleanup after staging-parent interference. The cfg(unix) unlink-without-
   traversal test is present but was NOT run on this Windows host.
5. Snapshot comparison against implementation-start.json: 35 frozen/preserved
   file hashes unchanged, including Cargo, boot/backend wiring and frozen
   suites. Design hash still matches the lock. Git status matches the recorded
   starting dirtiness plus this dispatch's permitted reports.
6. git diff --check: exit 2 from PRE-EXISTING whitespace in the frozen 11-03
   task.json:22/23 and review/design-findings.json:34. Their hashes still match
   the starting snapshot; no repairs were made. Git also warned that the global
   ignore file was unreadable. These warnings did not prevent status output.

Stage 1 test-to-clause coverage (not a full section-11 acceptance matrix):
- Sections 4/8/11: empty install, fixed paths, no-fetch/no-write reuse, changed
  artifact identity replaces one tree, worker corruption and missing required
  files cause reinstall rather than reuse.
- Sections 8/10/11: failed candidate hash preserves runtime path/type/bytes/
  permissions/mtimes after intentional mismatch or corruption; source failure
  leaves runtime absent and permits later recovery; partial staging is removed
  without a manifest and marked/unmarked/crashed foreign staging is untouched.
- No test in this dispatch starts a backend or uses a real installed libs root.
  Snapshot tests exclude access times and use ordinary fixture entries.

Explicitly deferred; Stage 2 and Stage 3 were NOT started:
- Stage 2: complete yt-dlp/Python/release/digest/embedded-worker identity matrix;
  independent manifest platform/identity mismatches; retained archive/binary
  corruption and all fetch/hash/extraction preservation combinations; native
  publication/cleanup denial and child-ordering evidence; private cfg(test)
  injection for remaining failure boundaries; complete restart-state matrix;
  source-resolution/error mapping (including missing tar/checksum cases).
  Correct the inherited AC1 cargo-tree commentary in that stage: cargo tree is
  allowed but not wider than full metadata, and offline use requires cache.
  The existing graph assertion ran; it is not delivery-commit evidence.
- Stage 3: boot/data-only wiring and overrides review, README operational
  contract, final locked Clippy/full Rust/Python gates and unchanged-suite
  verification, followed by coordinator-dispatched code review.
- Native Linux execution, Unix directory-sync/removal evidence, full final
  suite, release/container/CI evidence and the owner's single delivery commit
  remain pending. Windows focused tests do not establish those results.
- A1-A6 production defaults/trust assumptions remain unconfirmed; no live
  downloads, deployment, real-root cleanup, staging/commit/push or migrations
  were performed. The private-root/no-writer precondition is operator-enforced;
  metadata rechecks are not a shared-root locking/concurrency guarantee.

Final code hashes:
- src/runtime_installer.rs:
  70A57347824A1AE3135C91059B6CDDA3AE8F59CBA8933E6F558E9DEDE1ECAC44
- tests/runtime_install.rs:
  78A849CABA9FDB4E147B0A50C4E3DD025B25C88129267248E7B883A1946BD2E2

Stopped at the Stage 1 checkpoint. No implementation acceptance verdict is
issued here; review and subsequent stage dispatch belong to the coordinator.

## Stage 2 checkpoint -- identity and failure fidelity (2026-11-07)

Authorized scope: Stage 2 only, from implementer-stage2-brief.md. The native
.agents/.codex helpers remain absent; implementation ran directly with no
additional agents. Approval and owner-decisions S5 were read; the complete
design and applicable specs were read in bounded windows. The design hash and
both supplied Stage 1 code hashes matched before editing. All 35 recorded
frozen/preserved input hashes matched both before and after the code changes.

Only src/runtime_installer.rs, tests/runtime_install.rs and these permitted
progress/response reports changed. Stage 1 fixed-path lifecycle/cleanup is
preserved. Production changes are bounded to:
- Feed include_bytes! through the same private install/preparation functions
  used for module-local worker-identity fixtures. No public test API or
  production environment switch was added.
- Classify missing tar as RUNTIME_CURL_MISSING with tool=tar; preserve tar
  nonzero exits as RUNTIME_EXTRACT_FAILURE. Capture curl's final HTTP status
  with --write-out; only binary SUMS HTTP 404 becomes RUNTIME_RESOLVE_FAILURE.
  Other transfer failures remain download errors. Metadata also gets a
  post-transfer size check. Error context identifies tool/source basename or
  artifact without printing request/proxy URLs, credentials or query tokens.
- Reject missing/unusable source URLs, ambiguous selected release assets/PBS
  versions and inconsistent plan artifact names. Plan Python archive names
  must bind the stated Python version/release/target; binary/wheel names bind
  this platform and requested version. Digest fail-closed behavior is tested
  through the actual production resolver. Defaults/channels/dependencies are
  unchanged. Windows installer subprocesses use CREATE_NO_WINDOW.
- Remaining deterministic filesystem/command failures use module-local,
  thread-local cfg(test) injection only. All injection code disappears from
  production builds; no generic updater abstraction was introduced.

### Section 11 test mapping

Integration names below are in tests/runtime_install.rs; module-local names
are under runtime_installer::tests in src/runtime_installer.rs.

| Section 11 row / clause | Concrete coverage |
| --- | --- |
| install_happy_path_then_reuse_without_fetching; sections 4/6/8 | Same-named integration test; remove source availability and change endpoints, require identical paths and complete libs snapshot including staging mtimes. URLs do not determine identity. |
| deflated_wheel_entries_are_extracted | Same-named integration test uses real DEFLATE ZIP and system-tar fixtures. |
| identity_change_replaces_same_path | Integration independently changes yt-dlp, Python version, Python release and binary digest. Module embedded_worker_identity_replaces_same_path_and_disarms_guard changes only embedded-worker input to production's private logic. Assert fixed paths, full manifest identity, all retained digests, worker/binary bytes, removed old-only sentinel and no history/owned staging. |
| mismatched_manifest_reinstalls | mismatched_manifest_reinstalls_independent_of_runtime_id independently damages runtime_id, OS, architecture, Python version/release, Python-artifact digest and worker hash; all non-id cases keep the expected abbreviated id. |
| retained_archive_or_worker_corruption_reinstalls | retained_archive_binary_or_worker_corruption_reinstalls independently damages retained Python archive, binary copy and worker copy; unchanged plan/id is repaired by replacement, all requested artifact hashes/bytes restored, old-only sentinel removed. |
| failed_preparation_preserves_prior_runtime | Same-named integration test: both mismatched manifest and corrupt worker crossed with failed file source, hash, tar extraction and wheel extraction. Snapshots are taken after damage and before retry and compare paths/types/bytes/permissions/mtime exactly. Errors return no RuntimePaths. |
| missing_local_source_fails_before_admission_and_recovers | Same-named integration test checks final attempt=3, absent runtime, reclaimed staging and later recovery. initial_hash_and_extraction_failures_leave_no_runtime_then_recover covers initial hash/tar/wheel failures and recovery. |
| failed_staging_without_manifest_is_cleaned | failed_staging_without_manifest_is_cleaned_and_foreign_staging_is_untouched preserves marked/unmarked/crashed-complete foreign trees byte-for-byte. owned_partial_staging_cleanup_needs_no_manifest and failed_local_writer_is_awaited_before_manifest_free_cleanup exercise private cleanup/preparation. |
| publication_and_cleanup_failure_boundaries: candidate | publication_faults_preserve_or_retire_at_the_declared_boundary injects manifest sync (and Unix candidate directory sync); old snapshot stays exact. initial_manifest_sync_failure_never_exposes_partial_runtime covers no prior install plus recovery. |
| publication: invalidation/removal/rename | native_windows_locks_stop_manifest_invalidation_or_retired_removal uses real sharing locks; invalidation failure preserves exact old snapshot, removal failure leaves no manifest. publication_faults_preserve_or_retire_at_the_declared_boundary injects retired removal and rename failure; no restore/retry, later boot recovers. observed_runtime_interference_is_not_overwritten preserves a foreign destination. |
| publication: post-rename | publication_faults_preserve_or_retire_at_the_declared_boundary includes Unix root-sync failure with candidate validation; post_publication_validation_preserves_candidate_and_never_returns_paths retains invalid candidate for next boot. successful_rename_disarms_cleanup_even_when_called_explicitly proves explicit cleanup and Drop leave published bytes intact. |
| cleanup/quiescence precedence | cleanup_failure_retains_primary_and_prevents_download_retry preserves original download cause and staging; uncertain_child_quiescence_preserves_owned_work_without_retry returns staging error and preserves uncertain work. native_locked_staging_reports_cleanup_error_then_allows_owned_cleanup uses a real Windows sharing denial. Existing changed_staging_parent_is_not_cleanup_authority and uncertain_work_is_preserved_on_guard_drop remain. |
| abrupt-death states | restart_states_ignore_foreign_partial_staging covers old runtime, invalid manifest-free remainder, absent runtime and complete candidate; only matching complete candidate reuses, all foreign staging stays exact. No actual crash/power-loss transaction is claimed. |
| dependency_graph_has_no_yt_dlp_or_lofty | Same-named integration test now runs cargo tree --offline --locked -p ytdlp-http-wrapper and retains manifest/lockfile assertions. Corrected the inherited commentary: package-scoped tree is allowed, not wider than full metadata; offline use requires the local cache. Single-delivery-commit history remains pending. |

Additional sections 5-8/10 fidelity tests (module-local):
- release_digest_preference_fallback_and_exhaustion_use_real_resolver:
  usable API digest, missing/null/non-string/empty/malformed digest fallback,
  PBS fail-closed, malformed/unmatched SUMS, 404 exhaustion versus other
  HTTP/transport failures. Private command responses feed real resolution.
- release_and_python_metadata_fail_closed; wheel_metadata_requires_one_exact_wheel_and_usable_digest:
  missing/invalid metadata/URLs, absent or ambiguous release assets/PBS,
  zero/multiple/wrong wheels, and unusable selected-wheel digests.
- live_resolution_uses_pinned_channels_and_transfer_arguments:
  full live-resolver call chain with private responses, exact pinned endpoints,
  timeout/size/retry flags, no added proxy/CA flags; production plan precedence.
- plan_digest_and_configuration_errors_do_not_retry_or_create_staging:
  every artifact's missing/unusable digest, invalid metadata/config and names,
  worker-version mismatch; configuration failures remain attempt=1.
- missing_tools_and_nonzero_tar_use_distinct_error_codes:
  real native spawn-not-found after private command substitution, correct tool
  detail, missing tar versus nonzero tar, cleanup and no retry.
- download_status_limits_and_sensitive_urls_use_real_transfer_logic:
  HTTP/timeout/oversize/transport exits, metadata and artifact post-size checks,
  redacted error context. Only responses are injected, not resolver replicas.
- only_download_failures_retry_and_metadata_404_exhaustion_stops and
  partial_download_is_cleaned_before_each_bounded_retry:
  three bounded download attempts; each next attempt observes prior partial
  download/staging already removed; checksum exhaustion stops on attempt=1.
- unsafe_wheel_entry_is_an_extraction_error checks ZIP path rejection.

### Focused execution evidence

Host: native Windows. No live upstream transfer, backend admission, deployment
root or production install was used. All filesystem work uses isolated fixtures.

- rustfmt --edition 2021 src/runtime_installer.rs tests/runtime_install.rs:
  exit 0; only authorized code files formatted.
- cargo fmt --check: exit 0 after the final source edit.
- cargo test --locked --lib runtime_installer::tests: final exit 0,
  24 passed / 6 unrelated tests filtered, 7.20s. This count includes the
  writer_fixture test entrypoint (normally a no-op; also invoked as a child).
- cargo test --locked --test runtime_install: final exit 0, 15 passed, 14.45s.
- Earlier coherent checkpoints: module tests 11/19/23 passed and integration
  tests 15 passed, all exit 0. Initial compilation attempts exited 1 for test
  helper visibility and Value-versus-reference comparison; both were repaired
  before these successful runs. No failed assertion was suppressed.
- The local writer fixture executes this test binary as a child, writes a
  final marker, exits 7, is collected, then its owned manifest-free tree is
  removed. This establishes direct fixture-child completion/order only, NOT
  process-tree termination or arbitrary detached-descendant guarantees.
- Final 35 frozen/preserved SHA256 comparisons: unchanged. Design SHA256 still
  DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.
  No code findings.json, blocked.md or spec-gap.md appeared.
- git diff --check: exit 2 only for the previously recorded whitespace in
  frozen 11-03 task.json:22/23 and design-findings.json:34; unchanged hashes
  confirmed. Global git-ignore permission warnings recur; no cleanup attempted.

### Outstanding evidence and stop boundary

Stage 2 code/test work reaches a coherent Windows-validated checkpoint. No
unimplemented Windows matrix row is intentionally deferred, but independent
acceptance review has not run and native Linux Stage 2 validation is pending:
- Unix candidate/invalidation/root directory-sync fault cases were not executed.
- native_unix_denial_stops_invalidation_and_preserves_owned_cleanup_failure,
  unix_executable_permission_failure_is_not_retryable and the inherited
  staging_cleanup_unlinks_without_following_links are cfg(unix), unexecuted here.
  The native denial test reports unavailable evidence if a privileged runner
  bypasses directory chmod; such a run cannot establish real denial behavior.
- Windows fixture results are not Linux behavior, live-source/default validity,
  power-loss durability or process-tree containment evidence.

Stage 3 is NOT started: boot/override review and README operational guidance,
full locked Clippy/Rust/Python gates, native Linux/Windows CI/release/container
evidence, independent code review and the owner's single delivery commit remain.
A1-A6 rollout confirmations are still pending. No frozen documents, Cargo,
backend/protocol/pool/contract suites were edited; no staging/commit/push/deploy.

Final code SHA256:
- src/runtime_installer.rs: 8DB4CAFD6649FF81D45E7EFDB0B2B3974B5D8C901BE6892C530E1755C69139D2
- tests/runtime_install.rs: F9134DB10EDD62185A0AD5E66695EA1C35745D0B536BA000416121FEF255200B

Stopped before Stage 3. This report records implementation evidence, not an
acceptance verdict; coordinator review and subsequent dispatch remain separate.

## Stage 3 checkpoint -- boot wiring, operations and final gates (2026-11-07)

Authorized scope: implementer-stage3-brief.md, Stage 3 only. The Stage 2 installer
and integration-test hashes and all six writable starting hashes matched
review/stage3-start.json before edits. All 31 immutable hashes matched before
work and again after the final gates (individual comparisons follow). Approval
and S5 were read; the design hash remains
DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.
A8/A9 are accepted by S5; A7 remains retired, A1-A6 remain rollout checks.
Native .agents/.codex Trellis helpers are absent, so work ran directly without
nested agents, helper installation or lifecycle commands. No code findings.json,
blocked.md or spec-gap.md was present at start or at the final verification.

### Changes in this dispatch

- src/main.rs: clarify libs-dir and worker override/version help text; add five
  module-local tests (including a child-fixture entrypoint). Production boot/data
  wiring already meets the audited handoffs and is preserved, not refactored.
- src/runtime_installer.rs: implement RuntimeInstallError Debug using its existing
  Display. A real executable probe exposed that Result-returning main had printed
  the derived enum variant VersionMismatch instead of the stable
  RUNTIME_VERSION_MISMATCH code. The change preserves code/detail/attempt semantics
  while making the actual boot stderr grep-able. No installer lifecycle, identity,
  source, retry or failure-injection matrix was rewritten.
- tests/runtime_install.rs: add equal_explicit_worker_version_installs_and_reuses,
  asserting accepted equal pin, installed paths, reuse and single-layout invariants.
- README.md: document both artifacts, fixed layout, full-identity reuse and its
  extracted-tree integrity limit, exact runtime/backend/worker options, plan/source
  precedence, tool/transfer prerequisites and unverified live defaults. Document
  private-root stop/await/start (A8), verified preparation followed by manifest
  invalidation/removal/rename and the publication gap/reinstall behavior (A9), no
  backup/history/rollback/fallback, and manual legacy/crash-residue handling without
  blanket libs deletion. README.md:74, README.md:120 and README.md:148 are the main
  runtime/source/operations sections; configuration starts at README.md:43.
- Only these four code/doc files and the permitted progress/response reports were
  changed during Stage 3. src/backend.rs and src/lib.rs retain their starting
  boot integration bytes. Cargo.toml/Cargo.lock remain immutable in this dispatch;
  their pre-existing crate/lofty removal and direct sha2/zip dependencies remain.
  No frozen tests, worker/protocol/pool/request code, CI, Docker or scripts changed.

### Audited production handoffs

1. src/main.rs:176 constructs RuntimeConfig from the CLI/env configuration and
   awaits ensure_runtime. src/main.rs:245 awaits install_runtime before
   build_backend at src/main.rs:252, backend.start at src/main.rs:254, AppState
   construction at src/main.rs:258 and listener binding at src/main.rs:265.
   The same installer path precedes both backend choices; errors propagate with
   no old-runtime, alternate installer or backend fallback.
2. src/runtime_installer.rs:227 validates config before root/plan work;
   src/runtime_installer.rs:281 rejects differing explicit worker/runtime versions.
   src/runtime_installer.rs:198 retries only download-class errors; version
   mismatch stops on attempt 1. src/runtime_installer.rs:969 returns paths below
   fixed runtime/, with its identity only in metadata. Publication remains at
   src/runtime_installer.rs:1660. src/lib.rs:8 registers the installer module.
3. src/main.rs:131 chooses CLI by default; resident interpreter/script overrides
   win over RuntimePaths, installed package entries precede split user entries,
   and the expected version defaults to the runtime pin at src/main.rs:156.
   src/main.rs:160 calls the existing additive Backend::from_runtime_config API.
   src/backend.rs:101/121 turn those values into ResidentWorkerConfig without
   importing or calling the installer. Legacy from_config* facade signatures and
   their default identity remain unchanged (delegation at src/backend.rs:84).
4. src/resident_worker.rs:145/147 consume package/interpreter/script paths as data;
   src/resident_worker.rs:199/576 enforce handshake identity/version. Runtime id is
   passed as generation_id, not used as a directory selector or version manager.
   src/main.rs:258 passes runtime_paths.binary_path to AppState; the unchanged
   src/supervisor.rs:219 spawns that standalone executable, never python -m yt_dlp.
5. src/runtime_installer.rs:24 embeds the worker at build time. Runtime installer
   remains a leaf, with no crate-module dependency on backend/routes/supervisor.
   Production sources contain no LibraryInstaller/yt_dlp crate boot fallback.
   Static handoff inspection establishes direction/order; successful tests alone
   are not claimed as architecture acceptance.

### Added configuration evidence

- defaults_keep_cli_and_pinned_runtime (src/main.rs:324): default libs, backend,
  runtime version/release, unset plan/overrides and capacity 1; constructing CLI
  does not start a worker or mutate the fixture root.
- flags_override_environment plus configuration_environment_fixture
  (src/main.rs:345/382): a real child test process receives each documented runtime,
  backend, libs and worker env variable, checks env-only parsing, then checks flag
  precedence. Parent environment is not mutated. The fixture toggle exists only in
  cfg(test); there is no production switch or new public introspection/mock API.
- worker_version_mismatch_stops_both_backends_before_resolution
  (src/main.rs:436): each backend rejects mismatch on attempt 1, before an intentionally
  missing plan is read or libs root is created; boxed Debug retains the stable code.
- resident_paths_overrides_and_package_order_reach_handshake
  (src/main.rs:452): real host Python with a copied production worker and isolated
  fixture package. Default and empty-extra-path cases start; explicit interpreter
  and script overrides start while runtime counterparts are nonexistent. Two
  path-separated extra directories (including spaces) remain importable, while a
  conflicting yt_dlp in user entries cannot shadow the first runtime package path.
  Default/equal expected version and runtime generation metadata reach handshake;
  wrong imported version is rejected without a CLI fallback. Workers are explicitly
  shut down before their temporary roots are dropped. These are wiring/fixture
  tests, not execution of real upstream PBS or yt-dlp artifacts.
- equal_explicit_worker_version_installs_and_reuses
  (tests/runtime_install.rs:374): real offline plan/install/reuse with an equal pin.

### Actual validation and prerequisites

Host: native Windows. rustc 1.96.0, cargo 1.96.0, Python 3.14.0, system bsdtar
3.7.7 and an available local Cargo cache. Fixture sources were local; no live-source
artifact fetch, remote host, deployment or real libs installation was used.

Prerequisite: cargo build --locked --bin ytdlp-http-wrapper exited 0 (final build
14.12s) to provide the existing frozen pool-test harness with a fresh executable.
For both full-suite reruns, CARGO_BIN_EXE_ytdlp-http-wrapper was set in the command
process to D:\Repositories\ytdlp-http-wrapper\target\debug\ytdlp-http-wrapper.exe.
Final executable SHA256:
D5E6B27780415E02E55779FD140C195A73B0FCFAC63071FFDB30583232AB90E7.
No frozen test was modified to obtain that boot-probe evidence.

Final gate order, all after the final code edits:
1. cargo fmt --check: exit 0, no formatting differences.
2. cargo clippy --locked --all-targets -- -D warnings: exit 0, no warnings (6.59s).
3. cargo test --locked: exit 0; 105 passed, 0 failed, 6 ignored fixture entrypoints.
   Counts by target: lib 30 (24 installer tests including its child fixture), main
   5 (including its child fixture), backend_contract 3, process_lifecycle 10 + 6
   ignored, query_deserialization_test 1, resident_backend 6, resident_worker 14,
   resident_worker_manager 4, resident_worker_pool 9, run_contract 3, runtime_install
   16, worker_protocol_interop 4. Linux smoke target and doctests each ran 0 here.
   runtime_install took 18.81s; pool took 13.48s. The existing pool-size boot test
   had its executable supplied rather than relying on its skip-if-missing branch.
   The dependency test executed cargo tree --offline --locked -p ytdlp-http-wrapper
   and the manifest/lockfile assertions: final-tree graph evidence, not history.
4. python -m unittest tests.test_ytdlp_worker -v: exit 0, 14 tests (0.030s).

Earlier evidence and corrections are not hidden:
- Scoped rustfmt exited 0; initial cargo test --locked --bin ytdlp-http-wrapper
  exited 0 with 5 tests before the second extra-path/regression assertion additions.
- First full cargo test --locked exited 101: the new equal-pin test incorrectly
  passed None to a fixture helper that requires a binary SHA256. Corrected the
  fixture to supply its actual digest; no assertion or integrity rule was weakened.
  Focused cargo test --locked --test runtime_install
  equal_explicit_worker_version_installs_and_reuses exited 0, 1 passed/15 filtered.
- The next fmt/Clippy/full Rust run exited 0 (105 passed, 6 ignored), then executable
  probing identified the Debug rendering issue described above. That bounded fix
  was followed by a fresh build and all four final gates in the required order.
- Before the rendering fix, a real executable probe exited 1, left its proposed
  fixture root absent, but printed code: VersionMismatch. Afterward, both backend
  probes exited 1 and printed exactly the stable code plus attempt=1, with no root
  creation. Probe command shape (same explicit versions, intentionally missing plan):
  target/debug/ytdlp-http-wrapper.exe --backend <cli|resident> --libs-dir <isolated>
  --runtime-plan <missing-plan.json> --runtime-yt-dlp-version 2026.09.23
  --runtime-python-release 20261026 --worker-yt-dlp-version different.
  Observed stderr: Error: RUNTIME_VERSION_MISMATCH: attempt=1: worker yt-dlp version
  different does not match runtime version 2026.09.23.
- Scoped git diff --check for README/main/backend/lib exited 0. Whole-worktree
  git diff --check exited 2 only for the already recorded frozen-file whitespace;
  their hashes still match. Existing global Git-ignore access warnings remain.

Current logs are under the session temp directory recorded with the hash tables
below: fmt-delivery.log, clippy-delivery.log, rust-tests-delivery.log,
python-tests.log, build-final.log and boot-mismatch-cli/resident.log. Earlier logs
remain distinct; they are not substituted for final-tree evidence.

### Remaining evidence / stop boundary

Stage 3 code/documentation work and the four local gates are complete as described,
not independently accepted. Native Linux execution (including Unix sync, native
permission/link cases and official-binary smoke), CI matrix, release/container
validation, live-default validity/A1-A6 rollout confirmation, independent code
review and AC1's owner single-delivery-commit history procedure remain pending.
Fixture results do not prove upstream pins, shared-root safety or crash durability.
No remote connection, deployment, live-root cleanup, Git staging/commit/push or
task-status change occurred; HEAD remains the source baseline. Stop here for the
coordinator's independent reviewer, without dispatching one or issuing a verdict.


### Final immutable comparisons and writable hashes

All 31 expected SHA256s from review/stage3-start.json equal the observed values
below after the final gates. Both expected and actual are the listed hash.

| Immutable path | Expected = actual SHA256 | Comparison |
| --- | --- | --- |
| `.trellis/tasks/11-06-immutable-runtime-install/design.md` | `DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/prd.md` | `1A87CA82F303F0A19D7E0480F5D2619802DFD2DB74893FCD23CDA9A713B0C973` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/approval.md` | `AEE99393612762141BD61C6C9B551C821843DEE8780E5E7B72A707DDAF496B43` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/implement.md` | `C2316B9106186DD269974F2B3EFDEB4C8CD1CBA192EC917F65D0382C6576E62D` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/implementer-brief.md` | `C0AE7791D3B8B5A90390C3D167F05353ED352A2ED1CB1FAEED86E2093F1F9698` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/implement.jsonl` | `4A4E55E6FA351B6C55DF5172270CE71425B2568F4710D7BDA3755DADD62FBB52` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/check.jsonl` | `4A4E55E6FA351B6C55DF5172270CE71425B2568F4710D7BDA3755DADD62FBB52` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/review/owner-decisions.md` | `5EFF653373C66E9F66AE23C8D9836CC386AAB1E71C1AF8B5B08066020CECB6E5` | unchanged |
| `.trellis/tasks/11-06-immutable-runtime-install/review/design-findings.json` | `354E0717462563659737D0C6BAD3C488C128C26856D219571C888DA04C8983C5` | unchanged |
| `.trellis/spec/wrapper/backend/index.md` | `5F03BFFE1278A032BBD1A430F629E9B011EB2F52A8AE76FB7C51CABE287D3229` | unchanged |
| `.trellis/spec/wrapper/backend/process-lifecycle.md` | `DAC311068B7B544C4B004226E38583092EEA9B90B7419ECD9A469263FC587F6E` | unchanged |
| `.trellis/spec/wrapper/backend/dependency-sources.md` | `5FAEAE6D5FC1EDA4EDC9FA3E774CA847D189CBC014EE154396BF7BE56831B000` | unchanged |
| `.trellis/tasks/11-03-resident-worker-runtime-upgrade/design.md` | `AC528D8CADD614A951C9E8BF985F413730FBC826ADFECA4564D5ED124BE592A2` | unchanged |
| `Cargo.toml` | `80E8D2E75E2AC39EDF5EFD21133A8F689B153370187550E92A675ABCE6CD7F9C` | unchanged |
| `Cargo.lock` | `54552A42182C91E0104C73F07616F2BB8D2BB4ED772D4610168D3DD64A9905D1` | unchanged |
| `src/resident_worker.rs` | `20968034D14454812ABCFD9B4474DD314F5A5DC99FCD211583CD8FD870A44BD0` | unchanged |
| `src/resident_worker_pool.rs` | `D7F2A7453D769F7C938190823834F7394B854DC0D02830DB9BEA5E69D5994A01` | unchanged |
| `src/resident_worker_manager.rs` | `3AEF5A816B7391D2EBE2BA6CBEF206F4C7FAE8D66941BCA959624CF210A7B392` | unchanged |
| `src/executor.rs` | `AE65484DF047401949B17081214E7D7FAD88E7C69F48769FE779BA92623A2F90` | unchanged |
| `src/supervisor.rs` | `364C506F6A5805FAC572B76EB1A8E1A82066CBB92DDE1C95320D496F87F39CA8` | unchanged |
| `src/routes.rs` | `DE42DED79B2283216FFD16BE2E4188FB358EABD901678A15B9D0876D97C99847` | unchanged |
| `src/worker_protocol.rs` | `3A8779668CFA0976E9C2DA1B6ADD9D307C6271215C55039CBADB7AAB2121E385` | unchanged |
| `worker/ytdlp_worker.py` | `21C77F10DA0E87BCD62E29EF39702F259F4BE5A564D289CA4458384AA0CB5FF3` | unchanged |
| `tests/run_contract.rs` | `1E4FA6892436C8893A7D5BE389D92952CFEE87268BCA9B3953C393C941CD831E` | unchanged |
| `tests/backend_contract.rs` | `F3F802ED5562B5B9D7D4A0E8388C437CC6EB484252E792CEBE7795D8702A0602` | unchanged |
| `tests/resident_backend.rs` | `AAB3B10321EB881A4089508B2B2CB491B6A1BC552AE59E18081E4A6B443D5489` | unchanged |
| `tests/resident_worker.rs` | `0DBDD02B77B45CC324EA426E7D339F387D2CC41F8893FE7E9AD0AC6FFB30B556` | unchanged |
| `tests/resident_worker_manager.rs` | `5917D5C583B331BA8AB7A99274A962838C83AAA95FDAFA586AE85627BEBD311C` | unchanged |
| `tests/resident_worker_pool.rs` | `31DF1E81185F21FCCA2B471A4B1771CA5C4AFA902DF704626AC461836BF78D8B` | unchanged |
| `tests/worker_protocol_interop.rs` | `CAFAD5D541CCAC0DF2544827EFF9A539A4D121E699685D3CC39E713C338C2F48` | unchanged |
| `.trellis/tasks/11-03-resident-worker-runtime-upgrade/task.json` | `5D3D030A413E745679B953EEEFDF4773E2F2BFB87754318984D808F455C5DE00` | unchanged |

| Authorized writable path | Final SHA256 | Compared with Stage 3 start |
| --- | --- | --- |
| `src/main.rs` | `535BE7C7AFB46715D90A17C10C39F56CFD9000D8FCDD105E7C1AD49BA4383E1A` | Stage 3 changed |
| `src/backend.rs` | `E002DCB553BABA7B7EC8E0B82DBEC18281C8C9B7286CCE8533710E340C2E76D8` | unchanged |
| `src/lib.rs` | `E4457D7D9CCC64158FBA92B9BA7A2EE75640B652A5CA965341E8B2DB5FFB8BF1` | unchanged |
| `README.md` | `1F5A275D1197BC0E78BF9EE0E4365E6F048B317DFE4379CCDC62205006853DAF` | Stage 3 changed |
| `src/runtime_installer.rs` | `DC02D29706EF50CC422FC67169E7D7F7E0954FA6F14376D006B9CAD8285F79E4` | Stage 3 changed |
| `tests/runtime_install.rs` | `D8F61C143DFB052393F25596E3B683EF69D1663B98EDDE338CE5A19651A905C5` | Stage 3 changed |

Session log directory: C:\Users\ying\AppData\Local\Temp\ytdlp-stage3-addc2805a3234c43ad6235506f854362
