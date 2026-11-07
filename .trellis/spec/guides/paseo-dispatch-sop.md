# Paseo Multi-Agent Dispatch SOP

> Status: normative for owner-managed Paseo role loops in this repo.
> Scope: the design -> review -> implement loop run by DISPATCHING role
> agents through Paseo MCP. See section 0 for the availability gate --
> this SOP does NOT apply when Paseo MCP is not available.
> Learned from live loops (11-03 / 11-04 / 11-06, 2026-11).

## 0. Precondition -- Paseo MCP must be available

This SOP is for one specific environment: dispatching role agents via
Paseo MCP. It is NOT a general rule for all sub-agents.

Enter this flow ONLY when Paseo MCP is available in the session.
Detect by ANY one of:
- `tool_search "paseo create agent send prompt"` returns
  `mcp__paseo__create_agent` / `mcp__paseo__send_agent_prompt` /
  `mcp__paseo__list_profiles`.
- `mcp__paseo__list_profiles` succeeds and returns the role profiles
  (Implementer / Reviewer / Design Reviewer / Design Reviser).
- A Paseo MCP server (`srv_...`) is connected this session.

If Paseo MCP is NOT available (not configured, server not connected,
`list_profiles` empty or failing):
- Do NOT use this SOP. The profile registry, dispatch rules, and
  recovery playbook below are Paseo-specific and do not apply.
- Fall back to the inline Trellis workflow in `.trellis/workflow.md`:
  main session implements directly, or dispatch Task/Agent sub-agents
  with `implement.jsonl` / `check.jsonl` context.

Paseo tools can vanish mid-session (`unsupported call`). When that
happens, `tool_search` re-registers them (see section 4). If they cannot
be re-registered, treat Paseo as unavailable and fall back per above.

## 1. The loop

```
prd.md (owner)
  -> design.md (owner or agent)
  -> Design Reviewer          D1-D8, writes review/design-findings.json
       FAIL -> Design Reviser (minimal fixes only) -> re-review
       (round >= 3 still failing -> ESCALATE to owner)
  -> approval.md (owner: locks design_sha256; commit the design record)
  -> Implementer              stages + gates, writes review/responses.json
  -> Reviewer                 reviews git diff vs BASE_COMMIT, findings.json
       FAIL -> Implementer (per-finding ACCEPTED/REBUTTED/SPEC_CONFLICT)
       (round >= 3 -> ESCALATE)
  -> owner commits (single reviewed commit per unit where the prd
     requires it, e.g. "crate removal + replacement in the SAME commit")
```

Frozen yardsticks: prd.md, design.md (after approval), .trellis/spec/.
No role agent may edit them; a diff that touches them is an ESCALATE,
not a finding.

## 2. Profile registry (this repo)

Agent profiles are caller-side configuration: `create_agent` takes only
provider/model/settings -- there is NO profile parameter, and the
profile `notes` are NOT injected into the spawned agent (section 3, R1).
The owner materializes the profile into every dispatch.

| Role            | model (provider)                        | modeId       | thinking |
|-----------------|-----------------------------------------|--------------|----------|
| Implementer     | codex/remote-magpie/auto/glm-5.3-flash  | auto-review  | xhigh    |
| Reviewer        | codex/remote-magpie/auto/deepseek-v4-flash | auto-review | xhigh    |
| Design Reviewer | codex/remote-magpie/auto/qwen3.8-27b    | auto-review  | xhigh    |
| Design Reviser  | codex/remote-magpie/auto/glm-5.3-flash  | auto-review  | high     |

Registry verified 2026-11-06. Models rotate and gateways go down:
re-verify with `list_profiles` / a cheap probe before a long loop, and
switch models on repeated upstream errors (502/404/524) rather than
retrying a dead endpoint forever. The harness auto-permission reviewer
(model per ~/.codex config, NOT a profile) is not configurable; when it
times out, retry the same call (no sleep).

## 3. Dispatch rules (binding, when Paseo is available)

R1. **Profile notes are invisible to the spawned agent.** A dispatch
    prompt must NEVER say "your role is in your profile notes" or rely
    on any caller-side context. The prompt must be self-sufficient or
    point at a file the agent can read. (This exact mistake shipped in
    the 11-06 implementer dispatch; fixed in-loop.)
R2. **Brief-on-disk for every non-trivial dispatch.** Write
    `<task>/<role>-brief.md` (self-contained: role definition, input
    file list, output contract with the FULL JSON schema inline,
    ownership rules, convergence rule, working dir + baseline commit)
    and make the dispatch prompt one short paragraph pointing at it:
    `Active task: <path>` + "read <brief> first" + 2-3 task-specific
    sentences (round number, base commit, what changed since last round).
    The brief is the durable prompt: it survives agent death and is
    what a fresh replacement agent reads.
R3. **Staging discipline (implementer).** Long work is split into
    numbered stages in implement.md; the agent appends to
    `review/implement-progress.md` after each stage. Recovery from a
    mid-turn death = send "continue" with the disk-state summary, or
    spawn a fresh agent whose prompt points at the brief +
    implement-progress.md. Never assume a dead agent's partial work is
    coherent -- the gates decide.
R4. **Context discipline.** All roles: rg -n to locate, read in
    50-100 line windows, never print whole large files, truncate gate
    output to the last ~40 lines. The gateway models die around
    100-200k real context; a single giant turn is a failed turn.
R5. **Owner owns the commit.** Role agents: no git commit/push/merge.
    Owner commits, staging ONLY task-owned files. Never stage: todos/,
    response.json, .opencode/, .claude/, .trellis/spec/mpd/,
    .trellis/spec/myMPD/, docs/superpowers/, examples/, .tmp-*.log,
    target-gate/, __pycache__/. `.git` writes need escalated sandbox
    permission (auto-approve with a clear justification).
R6. **Re-lock on any post-approval design change.** If design.md is
    edited after approval.md exists (owner fix of a non-blocking
    observation included), recompute design_sha256, update approval.md,
    and commit the updated design record before dispatching
    implementation.
R7. **Gate order before any commit** (this repo):
    `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
    `cargo test`; `python -m unittest tests.test_ytdlp_worker -v`.
    (Extend per prd; gates live in the task's implement.md, and this
    line is the default.)
R8. **Windows specifics.** Write JSON with
    `[System.IO.File]::WriteAllText($p, $t, (New-Object System.Text.UTF8Encoding $false))`
    -- `Set-Content -Encoding UTF8` adds a BOM and breaks json parsing.
    Avoid apply_patch for long files on Windows (escaping fights);
    write files directly. When a shell command fails on quoting, write a
    small .ps1 to $env:TEMP and run it. For edits to existing
    line-terminated files, prefer ReadAllLines + replace-the-whole-line
    over string-matching (heredocs introduce invisible chars), and
    preserve LF (do not let WriteAllLines flip a file to CRLF); verify
    with `git diff --stat` before committing.
R9. **Owner decisions are recorded, not verbal.** Adjudications
    (e.g. a frozen-design ambiguity resolved by the owner) go to
    `<task>/review/owner-decisions.md` and are committed with the work,
    so a round-2 reviewer can verify the fix against the ruling.

## 4. Failure recovery playbook (Paseo)

| Symptom | Action |
|---|---|
| Agent errored mid-turn (502/404/524, context window, app-server crash) | Check disk state (git status + progress files). Send "continue" with a short disk-state summary; if the thread is poisoned (context window), spawn fresh with the R2 brief. |
| Permission prompt times out (auto-reviewer 90s) | Retry the identical respond_to_permission call; no sleep, just keep retrying. |
| Paseo MCP tools missing ("unsupported call") | Re-register with tool_search (query "paseo create agent send prompt"). If that fails, Paseo is unavailable -> fall back per section 0. |
| Model 404 ("may not exist") | Owner re-picks model from the registry; update the registry date. |
| Reviewer and implementer loop (round >= 3) | Stop the loop. ESCALATE to owner with both sides' last artifacts. The owner either rules (owner-decisions.md) or splits the ticket. |
| Agent "helpfully" rewrites a frozen file | `git diff --name-only BASE` shows it -> the round is void; restore from git, dispatch a fresh agent with the brief (the brief is the contract). |
| Shell wrapper (rtk etc.) corrupts/truncates git stdout (`git show`/`git log`| `git diff` text returns mangled or empty) | Do NOT trust git stdout. Restore via `git checkout <commit> -- <path>` (writes the blob to disk, no stdout) and verify by READING THE FILE directly, never via piped git output. `.git` writes need escalated sandbox permission. |

## 5. Artifact map (per task dir)

```
<task>/
  prd.md                      frozen after loop start (owner edits only)
  design.md                   frozen after approval.md
  approval.md                 owner-written: design_sha256 + review record
  implement.md                stage plan + gates + hard rules
  <role>-brief.md             self-contained dispatch brief (R2)
  review/
    design-findings.json      Design Reviewer output (D1-D8 schema)
    findings.json             Reviewer output (verdict/round schema)
    responses.json            Implementer per-round responses
    owner-decisions.md        owner rulings (R9)
    spec-gap.md               implementer: blocked on design gap
    blocked.md                implementer: missing precondition
    follow-ups.md             out-of-scope observations
    implement-progress.md     implementer stage log (R3)
```

## 6. Anti-patterns (each caused a real incident)

- "Role is in your profile notes" in a dispatch prompt -> agent has no
  role (R1).
- Applying this SOP when Paseo MCP is not available -> dispatching to
  models/harness that don't exist this session (section 0).
- One unstageable mega-turn implementation -> context death mid-write,
  orphaned half-state (R3/R4).
- Trusting the model string from a previous session -> 404 after
  upstream rotation (section 2).
- Owner editing design.md after approval without re-lock -> reviewer
  hashes mismatch, round void (R6).
- Staging "just one more" unrelated dirty file into the unit commit ->
  AC1-style "same commit" acceptance criteria fail (R5).
- Sleeping on permission retries instead of retrying -> dead loop
  (section 4).
