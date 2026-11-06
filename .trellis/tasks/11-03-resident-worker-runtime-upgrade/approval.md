# Approval record

- approved: design.md locked as the review yardstick for the resident-worker task
- approved_by: owner (session 2026-11-04, before TEAM B implementation)
- design_sha256: AC528D8CADD614A951C9E8BF985F413730FBC826ADFECA4564D5ED124BE592A2
- base_commit_for_review: 0fe685f (TEAM A handoff commit)

Notes:
- The design predates the loop naming: its scope-limiting commitments live in
  section 13 "Rejected alternatives" plus explicit prohibitions in sections 2/3
  ("Silent per-request fallback ... is forbidden", "does not install or upgrade
  itself", "does not decide which generation is active"). design.md is frozen;
  any edit now would appear in the baseline diff and must be treated per the
  pre-check rules.
- TEAM B scope for the reviewed unit: implement.md Phase 3 remainder (backend
  abstraction + explicit non-default resident wiring + per-request temp-dir
  ownership). Phase 4/6 deliberately out of scope (bounded worker pool,
  runtime generations).
