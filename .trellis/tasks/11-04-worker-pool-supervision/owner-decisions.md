# Owner decisions - worker-pool unit (session 2026-11-06)

Recorded by the owner after Reviewer round 1 (review/findings.json, verdict
ESCALATE, base c08f398, reviewed f3d788f). This file is a task-level decision
record. It deliberately does NOT modify design.md, prd.md, approval.md, or
anything under .trellis/spec/ (those stay frozen).

## D1 - Dependency topology (resolves the round-1 ESCALATE)
The reviewer correctly noted that the frozen design.md is internally
inconsistent on the backend/manager/pool topology:
- section 2, line 19: "resident_worker_manager becomes a thin compatibility
  facade; ResidentWorkerManager is a pool of capacity 1" (manager wraps pool).
- section 2, line 21: "Backend::Resident payload becomes Arc<ResidentWorkerPool>."
- section 3, lines 31-32: dependency chain "backend -> resident_worker_pool ->
  resident_worker_manager (compat facade) -> resident_worker" (pool depends on
  manager).
- section 10, line 222 (AC6): "full existing suite green with default config,
  zero existing-test edits." The unchanged tests/resident_backend.rs
  constructs Backend::Resident with Option<Arc<ResidentWorkerManager>>.

Lines 19 and 31 assert OPPOSITE pool/manager ownership and cannot both hold;
line 21 is only satisfiable by editing the existing tests, which AC6 (line 222)
forbids.

RULING: the implementation topology is accepted as the correct reading:
    backend -> resident_worker_manager (thin facade, holds the pool)
             -> resident_worker_pool -> resident_worker
This is the ONLY reading that simultaneously satisfies AC6 (zero existing-test
edits - a hard, mechanically checkable acceptance criterion) and section 2
line 19 (manager is the compat facade wrapping the pool). The backend holds
Arc<ResidentWorkerManager> for the compat/test surface and reaches the pool
through it (manager.pool()) for the bounded N-slot production dispatch.

Consequences:
- section 2 line 21 ("payload becomes Arc<ResidentWorkerPool>") is read as
  naming the effective dispatch engine (the pool is what executes), not the
  literal Backend::Resident payload type.
- section 3 lines 31-32 pool/manager direction is treated as a documentation
  looseness (chain written pool->manager; the compat facade owns the pool).

This is an owner ADJUDICATION of an internal design contradiction in favor of
the hard, checkable AC - NOT a change to any normative clause. design.md is
left byte-for-byte untouched. This ruling is FINAL for round 2: the topology
is owner-blessed and must not be re-raised as a blocker.

## D2 - F1 (deadline handoff across lazy replacement)
F1 (review/findings.json) is a real, in-scope, actionable defect: the bounded
production dispatch loses its absolute deadline across the lazy-replacement
await (src/resident_worker_pool.rs: it computes the remaining duration, then
may await ResidentWorker::start for a replacement, then hands that stale
duration to execute_with_timeout). It must be fixed in code. No spec or
ResidentWorker API change is required (per the reviewer's own analysis).
Scope: the bounded `dispatch` path only; the compatibility
`dispatch_unbounded` (relative exec_timeout) stays explicitly separate. Fix and
verification spec are in review/findings.json F1.
