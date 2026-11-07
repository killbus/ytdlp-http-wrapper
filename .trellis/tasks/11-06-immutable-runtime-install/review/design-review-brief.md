> SUPERSEDED (2026-11-07). This is the round-1 brief for the pre-amendment
> design. The owner amended design.md (section 0, sha256 BC19B71C...), so the
> current brief for this unit is
> `.trellis/tasks/11-06-immutable-runtime-install/design-reviewer-brief.md`
> (round 3, full D1-D8 pass). This file is kept only as the round-1 record.
> Note it contains the "your profile notes" anti-pattern (SOP R1) that the
> new brief replaces with inlined profile notes.

# Design Review Brief -- 11-06-immutable-runtime-install (round 1)

## Your task
You are the Design Reviewer for this unit (round 1). Your role, rubric
(D1-D8), and OUTPUT SCHEMA are defined in your profile notes -- follow
them exactly: write ONLY
  .trellis/tasks/11-06-immutable-runtime-install/review/design-findings.json
using that JSON shape (verdict PASS | FAIL | ESCALATE, criteria D1-D8
met|gap, blocking_findings G1.. with criterion/design_locator/gap/
verification, assumptions_for_human_review[]), then reply with the same
JSON plus a done=true/false line. Do NOT modify design.md, prd.md,
source, specs, or any git state. Do NOT run cargo/test suites (design
review is document-level; you may READ source to verify the design's
citations, but do not execute builds). Each finding must cite the exact
design.md section and the yardstick (prd req/AC, parent design section,
or spec file) it fails.

## Artifact under review
.trellis/tasks/11-06-immutable-runtime-install/design.md (13 sections,
~485 lines)

## Frozen yardsticks (read in full before judging)
1. .trellis/tasks/11-06-immutable-runtime-install/prd.md (8 requirements,
   AC1-AC6, non-goals, owner constraints)
2. Parent design: .trellis/tasks/11-03-resident-worker-runtime-upgrade/
   design.md -- especially section 6 (runtime layout), section 8
   (installer ownership: Remove list + capability list), section 9
   (compatibility), section 10 (observability), section 12 (rollout
   step 3), section 13 (rejected alternatives)
3. .trellis/spec/wrapper/backend/index.md, process-lifecycle.md,
   dependency-sources.md
4. Baseline sources to verify the design's code citations against:
   src/main.rs (flags lines 20-172, boot lines 174-280),
   src/backend.rs:53-120, src/resident_worker.rs:33-41 + 573-577,
   src/supervisor.rs:219, worker/ytdlp_worker.py:135 + 197-199,
   Cargo.toml (yt-dlp dep + [patch.crates-io]), tests/run_contract.rs,
   .github/workflows/ci.yml

## Hard rules the design must respect (violation = major finding)
- CLI backend stays default; resident explicit-config only; no silent
  fallback between backends or installers.
- Public HTTP API and CLI subprocess contract unchanged;
  tests/run_contract.rs unchanged and green (AC5).
- Zero new Cargo dependencies (prd req 7). NOTE the verified fact: sha2
  and flate2 leave Cargo.lock with the yt-dlp crate; the design's
  in-process SHA256 + minimal ZIP reader + system curl/tar strategy is a
  direct consequence -- check it is coherent, bounded, and has no hidden
  dependency assumption.
- Crate removal + lofty patch + transitive lock entries in the SAME
  commit as the new installer becoming the sole boot path (prd req 1).
- Immutable, integrity-verified installations; atomic staging/publication;
  partial staging never selectable; cleanup only owned paths (prd req 3).
- Deterministic offline tests only (prd req 8).
- Frozen docs (parent design.md, prd.md, .trellis/spec/) are untouchable
  by implementers -- the design may only EXTEND the parent layout
  (bin/) with the deviation explicitly flagged.

## Specific items the owner wants judged
1. TWO-ARTIFRAC split (owner's original question): the design must make
   clear the standalone yt-dlp executable (CLI spawn contract) and the
   Python package closure + interpreter (resident import target) are two
   distinct artifacts bound by ONE runtime-id. Verify section 1 + 4
   actually hold this, and that prd req 5 wording ("both backends source
   their executables from the installed runtime") is satisfied or
   flagged as needing a prd wording fix.
2. bin/ layout addition vs parent design section 6 (flagged additive
   deviation in design section 4) -- is the justification sound and the
   extension minimal?
3. runtime-id derivation (section 4): content-bound identity, url
   excluded. Is it deterministic across machines and deployments? Any
   collision or ambiguity path?
4. Manifest/integrity.json pairing (section 4): is the anti-tamper
   argument sound (tampered manifest vs tampered integrity file vs
   tampered payload -- each maps to a distinct failure path)?
5. Reuse check (section 8 step 4): manifest-mismatch handling must never
   mutate an existing versions/<id> in place (AC3). Check the two AC3
   branches (corrupt manifest vs wrong dir name) are both specified.
6. Source of trust (section 5): GitHub SHA2-256SUMS for the binary,
   GitHub API sha256 for PBS, PyPI digests.sha256 for the wheel, plan
   file override. Any case where an artifact is installed WITHOUT a
   required checksum (the old crate's optional-checksum hole)?
7. Boot ordering (section 8): is there any window where the HTTP server
   admits requests before a verified runtime exists?
8. AC coverage (section 11): every prd AC1-AC6 must map to at least one
   named test; flag any AC with no landing point or any test asserting
   something the design does not specify.
9. Assumptions A1-A6 (section 12): each must be explicitly marked in the
   body where it bites, and each must have a documented alternative or
   owner-confirmation requirement. Flag any undeclared assumption.
10. Non-goals (prd): design must not sneak in generation management,
    API changes, worker protocol changes, pool changes, or vendor code.

## Context notes (verified facts you may rely on, re-verify if cheap)
- libs/yt-dlp.exe is a PE (MZ header, 18,226,085 bytes) -- the standalone
  executable artifact, installed today by the crate.
- worker/ytdlp_worker.py:135 imports yt_dlp in-process; the worker
  handshake cross-checks yt_dlp_version (src/resident_worker.rs:573-577).
- The yt-dlp 2.7.2 crate resolves the LATEST GitHub release (no pin),
  verifies the checksum only when the API provides one.
- yt-dlp PyPI requires_dist is entirely extras-gated (no hard runtime
  deps, verified via PyPI JSON, latest 2026.10.26); wheel is
  py3-none-any.
- python-build-standalone release tag 20261026 exists; asset naming
  cpython-<ver>+<tag>-<target>-install_only.tar.gz verified for
  x86_64-pc-windows-msvc and x86_64-unknown-linux-gnu.
- cargo tree -i sha2 / flate2: both are yt-dlp-only transitives today.
- .gitignore already ignores /libs/; CI's Linux smoke job downloads its
  own official binary (independent of the installer).

## Verdict guidance (owner)
Round = 1. Record design_sha256 (Get-FileHash -Algorithm SHA256 of
design.md) in your reply summary. FAIL when any D1-D8 gap blocks
downstream implementation or review; ESCALATE only when a finding
conflicts with a FROZEN yardstick in a way the owner must rule on (name
the conflict and both readings); otherwise PASS, listing remaining
observations in assumptions_for_human_review for the reviser/owner.