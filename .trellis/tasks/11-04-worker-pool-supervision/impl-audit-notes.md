# Implementation audit of the in-progress src/resident_worker_pool.rs (by implementer be697a8c, 2026-11-05)

Context: a prior implementer left src/resident_worker_pool.rs on disk (Notify + per-slot-gate model,
~17KB). A subsequent implementer audited a SEPARATE external proposal (FIFO Semaphore based, not on disk)
and produced the findings below. Only the findings relevant to OUR in-progress file are actionable here.

## Findings applicable to the in-progress file (src/resident_worker_pool.rs)

1. MAJOR - line 190: `.expect("pool capacity is always >= 1")` will fail the clippy -D warnings gate
   (clippy::expect_used = "warn" in Cargo.toml, promoted to error by -D warnings). Existing production
   code has zero expect/unwrap. Fix: use a non-panicking structure or #[allow] with justification comment.
   Recommendation: since capacity is always >=1 by construction (validated at boot), restructure to avoid expect.

2. MAJOR - the file currently uses `Notify` + per-slot gate (correct per design section 5). DO NOT switch
   to a Semaphore: the frozen design explicitly rejects FIFO ("no separate queue structure is introduced",
   "immediate-lease preference for new arrivals"). The Semaphore-based external proposal is NOT on disk and
   should NOT be merged.

3. DESIGN CLARITY (informational, for reviewer awareness): design section 5 step 2 says "immediate lease +
   live worker" but section 6 says a dispatch replenishes a retired slot in place. The authoritative reading
   (used by both the in-progress file and the external proposal): lease the first gate-available slot
   (regardless of live worker), then replenish if the slot has no worker. The step-2 "+ live worker" phrasing
   is loose; O2 already flagged similar looseness. Not a blocking contradiction, but the reviewer should be
   aware the implementer is resolving the tension in favor of section 6.

## Adoptable patterns (from the audited external proposal, NOT on disk)
- RAII SlotLease with Drop-based settlement (more robust to cancellation/panic than settling at function end)
- Single Slot enum holding worker by value (four states can't drift)
- ExecBound enum (Deadline/Timeout/Unbounded) for the three dispatch entry points
These are optional refinements; the in-progress file already works with a simpler structure.

## Action for next implementer
- Fix finding 1 (remove .expect at line 190)
- Keep the Notify model (do NOT adopt Semaphore)
- Continue implementing the remaining pieces per handoff-notes.md
