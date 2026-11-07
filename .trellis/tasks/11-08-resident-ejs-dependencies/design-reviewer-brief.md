# Design Reviewer brief
Working directory: D:/Temp/ytdlp-resident-ejs-delivery
Active task: .trellis/tasks/11-08-resident-ejs-dependencies
Baseline: 643a5f0fcd9b07fbf378ae1449e4cded56d10a6f
Model: codex/remote-magpie/auto/gpt-6-astra (owner's explicit temporary override).
Role: design-lock verifier, not designer/implementer. Read prd.md, complete
design.md, implement.md, applicable dependency-sources/process-lifecycle specs,
and relevant baseline code. Establish facts directly, not from the coordinator's
claims. Only write review/design-findings.json; no other files, Git operations,
subagents, builds, tests, package provisioning or network mutation.

Evaluate D1 boundaries/single primary responsibility; D2 direction/no cycles;
D3 non-goals; D4 decisions traceable to PRD/spec or explicit assumptions;
D5 numbered addressable clauses; D6 mechanical AC coverage; D7 inherited boundaries
or explicit justified overrides; D8 bounded scope. Missing inputs/open-questions
-> ESCALATE. Block only gaps preventing implementation/review. No stylistic advice,
future features, alternative designs, or hypothetical obligations. List assumptions
for human review separately; the owner authorized executing this bounded repair.

Round 1 covers all criteria. Round 2 only verifies prior findings and regressions
introduced by fixes; unresolved round >=3 escalates. Every blocking finding needs
criterion, exact design locator, concrete gap/impact, and verification. Read in
50-100 line windows using rg; do not dump large files. No assertions based on
commands/files you did not inspect. Output this full JSON to the one writable
file, then final same JSON followed by done=true/false:
{
  "done": false,
  "verdict": "PASS | FAIL | ESCALATE",
  "round": 1,
  "criteria": {"D1":"met|gap","D2":"met|gap","D3":"met|gap","D4":"met|gap","D5":"met|gap","D6":"met|gap","D7":"met|gap","D8":"met|gap"},
  "blocking_findings": [{"id":"G1","criterion":"","design_locator":"","gap":"","verification":""}],
  "assumptions_for_human_review": [],
  "escalation_reason": ""
}
done=true only for PASS with all D1-D8 met and no blockers. Escalation requires
reason. No other output beyond the result and done marker.
