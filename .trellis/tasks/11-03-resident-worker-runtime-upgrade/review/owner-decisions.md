# Owner decisions for review round 1 findings

Recorded by the owner session on 2026-11-04 after Reviewer round 1
(`review/findings.json`, base `0fe685f`, reviewed commit `c08f398`).
This file is a task-level decision record. It deliberately does NOT modify
`design.md`, `prd.md`, or anything under `.trellis/spec/` -- those stay frozen
so the round 2 review keeps a stable yardstick.

## Finding 1 (major): client cancellation not observed on the resident path

DECISION: option (a) -- wire cancellation.

Rationale (owner-reviewed):
- Not a new requirement: the CLI backend already implements cancellation
  (`src/supervisor.rs` `reply.closed()` branch), implement.md Phase 3 already
  ticks "timeout/cancel retirement", and design section 5 already specifies the
  exact sequence (cooperative cancel frame only when it can be proven to apply
  to the active request; otherwise terminate and replace the whole worker
  within a bounded grace period). Option (a) therefore needs no design change
  and only connects an existing trigger to machinery the timeout path already
  exercises.
- yt-dlp requests have real side effects (downloads written to disk). Letting a
  cancelled request run to its full deadline keeps producing unclaimed output
  while holding the only warm worker and blocking the retrying client.
- Known cost, accepted: the Python worker does not read frames while executing,
  so cancellation in practice retires a healthy warm worker. design section 5
  already covers this fallback. The cost is expected to shrink once the bounded
  worker pool lands (separate task 11-04-worker-pool-supervision).

Scope for round 2: cancellation trigger on the resident path + bounded grace +
retire/replace + deterministic test (slow fixture request with a dropped
client). Executor mapping stays consistent with the CLI 503-on-cancelled
behavior. Do NOT edit design.md.

## Finding 2 (minor): timeout/business-error contract divergence

DECISION: option (2) -- accept the divergence as an owner-approved API change,
and add the resident-vs-CLI deterministic fixture comparison tests that design
section 9 requires before the resident backend can become default.

Rationale (owner-reviewed):
- The resident backend is not yet the default, so this is the window to fix its
  error semantics deliberately rather than inherit the CLI's lossy `exit_code:
  -1` magic number.
- `500` plus an explicit code (`WORKER_TIMEOUT`, or the worker's structured
  business code) lets a caller distinguish infrastructure failure from yt-dlp
  failure. `200` plus `-1` conflates timeout with kill.
- This is an explicit, recorded approval of the divergence from the wrapper
  backend spec error matrix rows for the resident backend only. The CLI
  backend's `200`/`exit_code: -1` contract is unchanged.

Scope for round 2: add deterministic resident-vs-CLI comparison tests over the
same local fixture arguments, asserting the approved shapes on both backends
(success: identical `RunResponse`; timeout: CLI `200`/`-1` vs resident
`500`/`WORKER_TIMEOUT`; business error: resident `500` plus verbatim worker
code). Reference this decision file from the test or code comment so the
divergence is traceable.

Deferred (NOT part of the reviewed diff): updating the wrapper backend spec
error matrix to record the approved resident-backend divergence. Per the review
loop rules, spec changes must not land inside the implementation diff, so this
is a post-review capture step for the owner to schedule separately.
