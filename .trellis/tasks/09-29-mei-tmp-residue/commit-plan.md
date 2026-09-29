# Approved local commit scope

One local commit:

`fix(wrapper): supervise yt-dlp cleanup and isolate temporary files`

This contains the coherent implementation, regression coverage, deployment examples,
spec synchronization and task evidence. It does not publish an image, push a branch,
archive the task or assert Linux runtime acceptance.

User approved this scope with "提交" on 2026-09-29.

## Exact approved file set

- `Cargo.toml`
- `Cargo.lock`
- `src/executor.rs`
- `src/lib.rs`
- `src/main.rs`
- `src/routes.rs`
- `src/supervisor.rs`
- `tests/query_deserialization_test.rs`
- `tests/process_lifecycle.rs`
- `tests/linux_ytdlp_smoke.rs`
- `scripts/linux-smoke.sh`
- `scripts/container-smoke.sh`
- `compose.yaml`
- `.github/workflows/ci.yml`
- `README.md`
- `SPECS.md`
- `docs/process-lifecycle.md`
- `.trellis/config.yaml`
- `.trellis/spec/wrapper/backend/index.md`
- `.trellis/spec/wrapper/backend/process-lifecycle.md`
- `.trellis/spec/yt-dlp/backend/service-architecture.md`
- `.trellis/tasks/09-29-mei-tmp-residue/task.json`
- `.trellis/tasks/09-29-mei-tmp-residue/prd.md`
- `.trellis/tasks/09-29-mei-tmp-residue/design.md`
- `.trellis/tasks/09-29-mei-tmp-residue/implement.md`
- `.trellis/tasks/09-29-mei-tmp-residue/progress.md`
- `.trellis/tasks/09-29-mei-tmp-residue/implement.jsonl`
- `.trellis/tasks/09-29-mei-tmp-residue/check.jsonl`
- `.trellis/tasks/09-29-mei-tmp-residue/research/mei-tmp-residue-root-cause.md`
- `.trellis/tasks/09-29-mei-tmp-residue/research/planning-review-2026-09-29.md`
- `.trellis/tasks/09-29-mei-tmp-residue/commit-plan.md`

## Pre-existing unrelated files excluded

- `.claude/`
- `.opencode/`
- `.trellis/spec/mpd/`
- `.trellis/spec/myMPD/`
- `.trellis/tasks/00-bootstrap-guidelines/`
- `docs/superpowers/`
- `examples/`
- `response.json`
- `todos/001-release-automation.md`
- `todos/002-performance-incidents/`

Approval source: .trellis/workflow.md Phase 3.4 step 5, "Present the plan once,
ask for one-shot confirmation." User implementation authorization has already
been applied to all local preparation. Local commit approval was subsequently
granted; pushing or remote execution would require their own authorization.
