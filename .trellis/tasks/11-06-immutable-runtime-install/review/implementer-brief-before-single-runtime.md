# Implementer brief -- 11-06-immutable-runtime-install (round 1)

SUPERSEDED / DO NOT DISPATCH (2026-11-07): the owner approved a single-runtime
scope in prd.md and review/owner-decisions.md. approval.md has withdrawn the
old implementation lock. This historical brief cannot authorize further code
changes; await a reviewed replacement design and an updated brief.

THIS DOCUMENT IS SELF-CONTAINED. There is no separate "profile" or hidden
role file: everything you need (role, rules, schema) is here and in
design.md/prd.md/implement.md. Do not look for or wait on any other
instructions.

Task dir: .trellis/tasks/11-06-immutable-runtime-install/
Working dir: D:\Repositories\ytdlp-http-wrapper (branch feature/resident-worker-runtime-upgrade)

## Your job
Implement design.md (LOCKED, approval.md owner amendment 2026-11-07,
sha256 BC19B71C241D00F3D260B20E6C287BB4335F079EDAD410F0500C1B2DE0364E4D).
design.md is the spec. prd.md AC1-AC6 are the exit criteria. implement.md
has the stage plan and gate list -- follow it.

## Ownership rules (verbatim)
- prd.md, design.md, approval.md, implement.md and everything under
  .trellis/spec/ are READ-ONLY. No editing, adding, or deleting,
  including "small clarifications".
- If the implementation conflicts with design.md, design.md has a gap,
  or something is not feasible in reality: STOP that part and write
  review/spec-gap.md with the clause location (design.md section), the
  problem, and the blocked work. Do not work around it.
- Module boundaries beat green tests: no test-only openings, no exposed
  internal state, no test-only branches in production code, no cross-layer
  calls or abstraction bypasses. If you need one of those, write
  spec-gap.md instead.
- Passing lint/typecheck/tests is NOT evidence of design conformance.
- Dependency boundary: no third-party yt-dlp library or replacement
  downloader. General-purpose crates already in Cargo.lock (sha2,
  zip/flate2) are allowed; do not hand-write SHA-256, DEFLATE, ZIP
  parsing, or add a new HTTP client.
- Pre-existing issues found out of scope: do not fix, do not refactor;
  append them to review/follow-ups.md.
- NO git commit / push / merge. The owner commits.
- Never stage or touch: todos/, response.json, .opencode/, .claude/,
  .trellis/spec/mpd/, .trellis/spec/myMPD/, 00-bootstrap-guidelines,
  docs/superpowers/, examples/, .tmp-*.log, target-gate/, __pycache__/,
  the pre-existing untracked .trellis/tasks/11-03 review files.

## Execution discipline (gateway context limits are real)
- Work in the 3 stages from implement.md; after each stage run all four
  gates (fmt, clippy -D warnings, cargo test, python unittest) and only
  continue when green. Keep your own notes on disk in
  review/implement-progress.md (append per stage) so a retry can resume.
- Read code in 50-100 line windows (rg -n first). Never print whole files.
- Write any JSON file with
  [System.IO.File]::WriteAllText($p, $t, (New-Object System.Text.UTF8Encoding $false))
  (Set-Content -Encoding UTF8 writes a BOM and breaks json parsing).
- If a command fails on escaping, write a small .ps1 in $env:TEMP and run
  it; do not fight inline quotes.

## Exit
When all 3 stages are done and all gates are green: write
review/responses.json with EXACTLY this shape (round = findings.json
round + 1; no findings.json exists, so round = 1 and finding_responses
is an empty array):

`json
{
  "round": 1,
  "scope_summary": "what was implemented, what was explicitly NOT implemented (against non-goals)",
  "changes": [{"file": "", "intent": "", "spec_clause": "design.md section x"}],
  "test_notes": "what behavior the tests verify; do NOT claim they prove design conformance",
  "finding_responses": []
}
`

Append the final stage report to review/implement-progress.md, and reply
with 2-3 sentences: what you built, gates green, deviations (if any)
with design.md section cites.

## Role reminders (binding)
- You are the Implementer in a design -> implement -> review loop. Your
  ONLY job: land this ticket as code inside the locked design. You are
  not the designer and not the arbiter; your output must contain no
  verdict language (PASS/FAIL/verdict) -- the reviewer owns that.
- No defensive logic for imagined edge cases, no "might be needed
  later" abstraction or configuration (YAGNI). Handle only what is
  reachable in real production call chains and specified by design.md.
- Every design.md clause you implement must be traceable: record the
  section cite in responses.json changes[].
