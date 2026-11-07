# Owner decisions -- single installed runtime

Recorded by the coordinating assistant on 2026-11-07 from the user's direct
conversation instructions. These are requirement changes, not a subordinate
agent's assumptions or a claim that the resulting design has passed review.

The user rejected retaining multiple versions, asked for reuse unless the
specified version mismatches, and then confirmed the proposed single-runtime
model with "yes". That confirmed model includes matching installation identity
(yt-dlp, Python, artifact and embedded worker identities), staged preparation,
replacement before backend startup, and no historical retention.

## S1 -- One installed runtime

Use one fixed LIBS_DIR/runtime installation. Reuse a valid matching install.
When absent, mismatched, or corrupt, prepare a verified replacement in staging
and replace the installation. Successful installation/replacement leaves one
installed runtime, not versions/<id> directories or retained history.
Keep staging as an installation work area, not a second selectable version.

Runtime identity in a manifest, logs, or the existing worker handshake does
not authorize a multi-generation architecture. No hot promotion, canary,
rollback-to-history, version-retention policy, or version GC is required.

## S2 -- Failure and cleanup

Do not modify the old install during download, extraction, or verification of
the candidate. If preparation fails, boot fails explicitly; never silently
serve an old mismatched runtime. Clean this attempt's created staging after
its writers/children stop. A complete runtime manifest proves installation
validity, not the only possible authority to remove owned temporary work.
It is therefore NOT required for current-invocation failure cleanup.

Do not infer ownership or inactivity of other attempts from a marker or age.
Uncertain liveness or failed cleanup must be diagnosed, not silently declared
clean. No unsafe cross-boot sweep is authorized. The previous A7 proposal,
which retained even this attempt's partial staging solely for lack of a
manifest, is superseded rather than confirmed.

## S3 -- Scope and inherited-document precedence

For this task, these decisions and the amended PRD supersede the parent
design section 6's versions/<id> layout and universal manifest-before-cleanup
rule. Parent section 7's future generation/retention assumptions must not be
used to restore complexity rejected here. Existing parent documents remain
historical references; this is an explicit scoped override, not a silent
claim that they already describe the new model.

The CLI still executes the installed yt-dlp binary; resident still uses the
installed Python/package/worker closure. CLI remains the default. Public HTTP
contracts, worker protocol, pool behavior, generic-crate allowance, integrity
verification and the same-commit crate-removal requirement remain unchanged.

## S4 -- Review and implementation gate

Round-4 PASS covered design hash
3B524845DAC0A49CA0030C325281BB26C06327B95BDC690E3F2541C890E848BC.
It does not approve the single-runtime design. Withdraw the old implementation
lock, revise the design against the amended PRD, obtain a fresh full review,
then update the implementation plan and lock before resuming code work.

Windows/Unix replacement mechanics, interrupted-publication handling, and any
shared-installation concurrency precondition must be stated honestly in the
revised design. Do not invent an atomic rename-over-nonempty-directory
guarantee, silently destroy another running instance's runtime, or build a
generation manager to solve this. If a deployment choice is necessary and
cannot be derived from these requirements, label the assumption or raise a
specific open question. Requirement approval is not advance approval of an
invented filesystem transaction or locking mechanism.

## S5 -- A8 and A9 confirmed; implementation may resume

After the full single-runtime design review returned PASS, the coordinator
asked the owner to confirm A8 (private install root, stop old consumers before
startup) and A9 (no saved old install; publication failure after retirement may
require reinstall). The owner replied "agree" on 2026-11-07. This confirmation
applies to the reviewed design SHA256:
DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59.

- A8 is accepted: one canonical libs-dir per wrapper instance for its serving
  lifetime; deployment serializes stop/await/start. Runtime/staging are ordinary
  directories on one local filesystem. No automatic lock/lease or shared-root
  rolling-update system is requested.
- A9 is accepted: fully prepare/verify first; invalidate the old manifest,
  remove the old tree, and rename to the absent runtime path. No backup/restore.
  Preparation failures preserve old bytes; after retirement starts, failure
  or death may leave an invalid/absent installation. Boot fails explicitly;
  later startup reinstalls. No old-version fallback or crash-atomicity promise.
- Resume staged implementation after updating the plan and lock. This external
  decision supersedes only A8/A9's pending-confirmation wording in design.md;
  do not change the reviewed document merely to record the approval.
- A7 stays retired. A1-A6 retain their prior production-rollout confirmation
  requirement; this reply does not confirm their defaults/trust prerequisites.
  Implementation and offline fixture validation can proceed under the recorded
  assumptions, but do not claim live-source/deployment verification.
- No deployment, real installed-runtime deletion, or git commit/push is
  authorized. Tests use isolated fixture roots; the owner controls delivery.

## Dispatch override (temporary, 2026-11-07)

The owner subsequently instructed all role dispatches to temporarily use
remote-magpie/auto/gpt-6-astra. Keep each role's other profile settings and
materialize its instructions in the brief. This is a per-dispatch model
override, not permission to change global Paseo profiles.
