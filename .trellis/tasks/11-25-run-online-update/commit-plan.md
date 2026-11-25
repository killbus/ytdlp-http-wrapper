# Approved commit plan

`feat: support native yt-dlp online updates`

Task-owned files:

- src/executor.rs
- src/lib.rs
- src/main.rs
- src/online_update.rs
- src/routes.rs
- src/runtime_installer.rs
- src/supervisor.rs
- tests/run_contract.rs
- tests/query_deserialization_test.rs
- README.md
- .trellis/spec/wrapper/backend/index.md
- .trellis/spec/wrapper/backend/dependency-sources.md
- .trellis/spec/wrapper/backend/process-lifecycle.md
- .trellis/tasks/11-25-run-online-update/ (requirements, verification and task records)

Excluded: pre-existing untracked .claude/, all ignored target/ verification data.
No push or deployment is part of this commit. The user authorized this plan
with "commit" on 2026-11-25, after reviewing the external audit follow-up.
After the push exposed a stale query-test assertion, the user explicitly
authorized rewriting the feature commit to include its CI fix. Fold the fix into
the feature with fixup/autosquash and push using an explicit force-with-lease.
