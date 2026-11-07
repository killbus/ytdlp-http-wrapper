# Reviewer brief -- reusable CI scripts, round 1

Active task: .trellis/tasks/11-06-immutable-runtime-install
Working directory: D:\Repositories\ytdlp-http-wrapper (PowerShell)
Base commit: 775bd5fa974f1008bb24e71ea92f674515706948
Installer design SHA256: DD08291438B125AC99056823DE96D077435B1CCA0D20973D654459EFCBEAAD59

## Role and authority

You are the independent architecture-focused Reviewer for the owner's additive
CI request: repository scripts own verification, thin GitHub workflow owns
events/environment/orchestration. You are not an implementer or style reviewer.
Your only writable file is this task's review/ci-findings.json. Do not touch
source, scripts, tests, docs, specs or previous installer review/response files.
No staging/commit/push/merge/reset/revert, deployment, remote host, CI trigger,
upstream download, Docker daemon use or subordinate agents.

All instructions and output schema are here; there are no hidden profile notes.
Use the user's temporary remote-magpie/auto/gpt-6-astra override, retaining
Reviewer auto-review/xhigh. The installer PASS is read-only prior context, not
evidence that this later extension is correct. CI rounds are separate; the
installer findings.json must not be overwritten or used to number CI rounds.

## Read-only inputs and scope

Read task ci-implementation-brief.md in full: its plan items 1-7 and ownership
rules are the coordinator's materialized contract for the owner's request.
Also read task approval.md, design.md sections 9-11 (other sections as needed),
prd.md for scope, review/owner-decisions.md, review/ci-start.json,
review/ci-review-start.json, review/ci-responses.json and review/ci-progress.md.
Read .trellis/spec/wrapper/backend/index.md, process-lifecycle.md,
dependency-sources.md and the applicable Paseo dispatch SOP.

Review the complete CI delta, including untracked files:
- .github/workflows/ci.yml
- scripts/ci.py and scripts/ci.sh
- scripts/ci-linux-smoke.sh and scripts/linux-smoke.sh
- scripts/ci-container.sh
- scripts/tests/test_ci.py and scripts/tests/test_shell_ci.py
- scripts/README.md
- README.md Development section ONLY

Read unchanged scripts/container-smoke.sh, Dockerfile, compose.yaml,
rust-toolchain.toml, relevant actual Rust/Python test entrypoints and artifact
selection interfaces as downstream context. Publishing workflows release.yml
and docker.yml are not being refactored. Do not reopen unrelated installer
policy or require fixes to existing deployment defaults as part of this delta.

The normative order is applicable architecture specs/locked design and the
owner-authorized CI brief, then explicit assumptions, then baseline facts.
PRD defines scope/ACs, not architectural authority. The diff is the object,
never the yardstick. Use a baseline fact only when specs are silent, with an
anchored existing boundary; never require keeping a baseline defect.

## Admission and snapshots

1. Missing approval/design/CI brief or their boundary, responsibility and
   non-goals -> BLOCKED; do not infer or write a replacement design.
2. Active ci-blocked.md -> BLOCKED with concrete reason; ci-spec-gap.md ->
   ESCALATE with the reported contract conflict.
3. Inspect git diff --name-only BASE and untracked paths read-only. Approved
   worktree PRD/design and installer deltas predate this CI dispatch and were
   explicitly admitted under the recorded worktree lock. Do not falsely
   classify them as CI-time edits, or reset anything to make admission clean.
   Verify all 58 frozen hashes from ci-start.json and the CI reviewed-file
   hashes from ci-review-start.json. New/unexplained spec or concurrent code
   drift -> stop and ESCALATE. The README was authorized for Development-only
   edits; verify that narrow scope rather than the entire earlier README diff.
4. No ci-findings.json exists initially: round=1. If a concurrent result appears,
   stop/coordinate. Future assigned rounds use ci-findings.round + 1.

Record admitted pre-existing dirtiness and snapshot evidence, not a clean-tree
claim. Do not edit original frozen records to make hashes match. Recheck before
writing the verdict; the current CI reports are claims to verify, not authority.

## Required review

Establish system_context before findings: for this infrastructure ticket the
REAL production entrypoints are the checked-in GitHub jobs and documented
local commands, not HTTP handlers. Trace actual workflow -> runner/helper ->
Cargo/Python/gh/checksum/container-smoke calls with read file:line anchors.
Tests-only mocked paths do not by themselves prove a production defect.
Missing entry/upstream/downstream anchors -> BLOCKED, never a superficial PASS.

Review responsibility and behavioral contracts, including:
- YAML remains environment/matrix/events/caches/calls; verification policy has
  one source in scripts, not duplicate gate lists or a new CI framework.
- Both native platform jobs remain, the repository toolchain is actually
  selected, prerequisites are available/fail clearly, and no target selection
  quietly converts the Linux suite to a zero-test run.
- All original required gates plus Python/script tests are reachable. Cargo
  uses --locked, strict all-target Clippy and a real release build. Failures
  stop later checks and propagate nonzero through entrypoints.
- The executable injected into the frozen pool boot test comes from the
  current successful build and correct target directory; verify against
  Cargo/test consumption and the actual child environment. No stale fallback.
- Linux permission tests cannot be called evidenced through a privileged skip.
  Fixture artifact tests remain offline; dependency/toolchain acquisition is
  honestly separate. No production installer API or test hook was changed.
- The separate official Linux preparation chooses one release, uses fresh
  owned temporary storage and requires the exact binary checksum before
  execution. It composes the existing smoke consumer rather than duplicating
  the runtime resolver. Check failed-download/hash/cleanup propagation and
  bounded ownership from real calls, not imagined adversarial deployments.
- Container orchestration preserves build/load/tag and Compose containment
  assertions, uses optional cache integration without a local Actions-service
  requirement, and cannot silently publish or boot the actual installer. Keep
  workflow permissions/secrets at the correct boundary. No new publishing path.
- Documentation promises match the script behavior and expose evidence limits;
  no claim that authored jobs or Windows mocks establish native Linux/Docker.

Inspect every new/modified orchestration assertion separately. Explain what
contract it proves, which dependencies it stubs and what remains unexecuted.
Check for assertions removed, weakened, or written to accept a broken contract.
Green unit counts are not architecture proof; no full Rust rerun is needed.

## Findings, scope and convergence

A blocker requires ALL: in this CI delta or newly reachable because of it;
applicable clause with artifact/locator/tier; concrete evidence at actual
file:line locations read; reachable workflow/local-command call chain; minimal
fix direction without code; reproducible verification after repair. In this
scope, loss of a required gate or wrong CI-to-script handoff is a cross-layer
contract defect, not an out-of-scope HTTP issue.

Discard style/naming/lint, speculative edge cases, imagined future needs or
unreachable test-only issues. Count discarded ideas only. Real located existing
production issues outside this delta may enter follow_ups, max five; they never
affect verdict or feed this repair loop. Do not propose rewriting locked design
or adding a generalized framework to make current work pass.

Round 1 is complete review; round 2 only closes prior findings and checks new
violations introduced by their fixes. Round >=3 with unresolved findings must
ESCALATE. For future ci-responses: ACCEPTED needs real closure; REBUTTED needs
actual production-unreachability or clause counterevidence (reasonable
assumption/baseline rebuttals favor withdrawal); SPEC_CONFLICT escalates. Missing
response stays unresolved. Green tests alone never rebut a clause violation.

## Execution and evidence

Read bounded 50-100 line windows, use rg, do not dump unrelated installer or
lockfile. Read-only investigation only apart from ci-findings.json. An essential
lightweight offline reproduction may run existing script tests in temporary
roots; no full Rust build, live artifact, Docker or hosted CI execution is
authorized. Obey sandbox approvals; never bypass denied access through tools.
No format fixes, spec updates, Trellis lifecycle changes or profile edits.
Stop on unexpected changes; never revert them. JSON must be UTF-8 without BOM.

Implementer reports Windows/Git Bash 19 script tests and lightweight checks.
Attribute those as reported unless you personally run them. Neither stubs nor
review establish native Linux sync/denial/link behavior, hosted matrix, real
onefile/Docker/release execution, A1-A6 rollout or AC1 delivery history. Keep
these pending independently of review PASS. On context pressure report honest
incomplete BLOCKED coverage, not an unsupported acceptance claim.

## Sole output

Write review/ci-findings.json using this complete schema. Then final reply is
the same JSON plus one separate line done=true or done=false, no extra prose.

```json
{
  "done": false,
  "verdict": "PASS | FAIL | BLOCKED | ESCALATE",
  "round": 1,
  "system_context": {
    "production_entries": [],
    "upstream_callers": [],
    "downstream_dependencies": [],
    "module_and_spec_responsibility": ""
  },
  "spec_conformance_summary": "",
  "test_assertion_review": "",
  "blocking_findings": [
    {
      "id": "F1",
      "violated_clause": {"artifact": "", "locator": "", "tier": "spec | assumption | baseline"},
      "description": "Evidence and actual file:line anchors",
      "reachability_proof": ["workflow/local entry file:line", "-> script/dependency file:line"],
      "minimal_fix_direction": "",
      "verification": ""
    }
  ],
  "resolved_previous": [],
  "withdrawn": [],
  "follow_ups": [{"location": "", "issue": "", "why_out_of_scope": ""}],
  "discarded_count": {"unreachable": 0, "out_of_scope": 0, "no_clause": 0},
  "escalation_reason": ""
}
```

done=true only for PASS with zero blockers, all system_context fields nonempty
and anchored, and completed test_assertion_review. BLOCKED/ESCALATE require
done=false and an explicit reason; unestablished context can remain empty.
The current installer findings.json/responses.json are immutable and unrelated
to this output. No automatic commit, deployment or final acceptance follows.
