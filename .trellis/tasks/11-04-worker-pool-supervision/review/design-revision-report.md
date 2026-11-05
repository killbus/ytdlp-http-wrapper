# Design revision report — round 1 -> 2

Artifact: `design.md`
Base commit: `c08f398`
Input: `review/design-findings.json` (verdict `needs_revision`, 7 findings)
Edits limited to `design.md`; this file added under `review/`. No source,
`prd.md`, spec, or git state touched.

## Finding-by-finding disposition

### F1 (major) — boot-failure semantics
- Change: §6 Boot paragraph rewritten to a single, explicit **strict fail-fast**
  rule: any slot start failure returns the error and fails the whole boot
  before admission. Deleted the contradictory "leaves that slot absent /
  requests may still be served by the remaining slots / reported at boot"
  wording. Added that partial-degraded boot is rejected because it would
  relax the preserved rule.
- Source: baseline `src/main.rs:215` (`backend.start().await?`),
  `src/backend.rs:91-97` (`start` propagates `ensure_started` error),
  `prd.md:28-30` ("All Phase 3 hard constraints stay unchanged" / explicit
  config fails at boot). No owner escalation; resolved to option (a) per
  instructions.

### F2 (major) — per-slot Retired state
- Change: §4 `PoolSlot` gains `retired_reason: Option<&'static str>`;
  added a derivation rule (live worker -> Idle/Busy; reason set -> Retired
  with reason; neither -> Absent), cleared on successful replacement. §6
  retirement/replacement bullets now set/clear the per-slot reason. §7
  `SlotHealth` gains `retired_reason` and the pool-wide
  `retiring_reason_last` is explicitly **dropped** (per-slot granularity
  satisfies the prd; no duplication). §10 AC5 landing now asserts per-slot
  `state` + `retired_reason` + counters.
- Source: `prd.md:23-25` (per-slot state idle/busy/retired/absent +
  "retirement reason"); `prd.md:40` (AC5); finding F2 ask.

### F3 (minor) — selection policy
- Change: §2 module-table wording changed from "least-recently-occupied" to
  "first-available in slot index order (section 4)". §4 Selection restated
  as the single policy, with the low-index bias note kept (tests may rely).
- Source: finding F3; §4's pre-existing deterministic index-order rule.

### F4 (minor) — capacity-1 equivalence
- Change: §8 claim scoped to "identical for the existing test surface and
  uncontended requests"; contended requests now wait bounded by the request
  deadline (prd-mandated, not a regression). AC6 framing unchanged.
- Source: baseline `src/resident_worker_manager.rs:122` (unbounded
  `execution_gate` wait), `prd.md:17-19` (wait within own deadline, no
  unbounded queue).

### F5 (minor) — deadline anchor + wait loop
- Change: §5 `dispatch(..., deadline: Instant)` — absolute deadline
  (request-start Instant + timeout), with baseline anchors cited
  (`src/backend.rs:125`, `src/executor.rs:79`). Named the wait-loop shape
  (arm `Notify` before scanning, scan, await, **re-arm before retry**).
  Added explicit fairness model: immediate-lease preference for new
  arrivals, no FIFO — marked `[ASSUMPTION]`. B1 relabelled so only the
  absence of a queue-depth knob is `[ASSUMPTION]` (wait semantics noted as
  prd-mandated).
- Source: prd requirement 4 / AC3; finding F5; fairness choice
  `[ASSUMPTION]` (not sourceable).

### F6 (nit) — AC2 landing channel
- Change: §10 AC2 now specifies `worker_id` travels inside the result-frame
  payload (e.g. result `stdout` field) or on stderr, never plain-text stdout;
  notes stdout is the length-prefixed frame stream.
- Source: `tests/fixtures/resident_worker_runtime.py` (frames written to
  `sys.stdout.buffer`; existing `stderr-diagnostic` pattern).

### F7 (nit) — Phase 7 citation
- Change: §7 now cites parent `implement.md` Phase 7 "Operational hardening"
  and parent design.md §10 for the metrics inventory.
- Source: parent `implement.md:147`.

## Assumption review nit
- B1 relabelled as described under F5 (wait semantics prd-mandated; only
  no-queue-depth-knob carries `[ASSUMPTION]`).

## Unresolved
- None. No `open-questions.md` written; no owner input required.

## Section numbering
- §1–§10 headers unchanged; downstream "design.md §x" citations remain valid.
