# Coordinator review of hosted-CI repairs

Baseline: `c08dcc1e1eccbc940b4ee8116486e1b9c21bafed`, PR #3.
Scope: C1-C3 from `managed-python-ci-responses.json`, not a new design cycle.

- C1 accepted: the only production changes are cfg(unix) on the permission-error
  variant and its string arm. Constructors and the UNIX permission regression
  were already cfg(unix), and their contents remain unchanged. No lint allowance
  or platform-independent error behavior was introduced.
- C2 accepted: an explicit executable path with a dot component replaces the
  invalid installation-root request. Canonical equality proves it names the same
  file; distinct request text and identical recorded environment are asserted.
  Existing runtime-id, publication-path and cleanup checks remain.
- C3 accepted: pinned uv source evidence is recorded in ci-progress. The revised
  test requires successful safe-member installation and checks the isolated
  filesystem for any `outside` entry, including the cache-relative escape target.
  The traversal does not follow symlinks. assert_installed_plan checks identities,
  no old sentinel, fixed layout and owned staging cleanup. This is not accepting
  success without safety assertions. Invalid-archive failure tests are retained.

The coordinator read the full source diff, helper assertions and response/source
evidence; checked the two candidate SHA256 values against the implementer report;
and mechanically verified that removing the two new cfg attributes reproduces the
baseline production file exactly. `cargo fmt --check`, scoped `git diff --check`,
and source/hash/JSON/UTF-8/LF checks exited 0. No local native build/test or runtime
provisioning was performed. The prior Docker success belongs to the baseline;
CI must execute on the repaired commit before merge/publication.

No unresolved source-review finding remains for this bounded repair. This is
coordinator review, not a claim that hosted native tests have passed. The existing
tool-ownership/native-validation spec remains applicable; no new architecture
policy is needed. Concrete fixture semantics and source references remain in
the CI progress record rather than adding a general archive/runtime framework.

The user's merge-and-image instruction authorizes continuing delivery after green
CI. Commit only src/runtime_installer.rs, tests/runtime_install.rs and these three
new CI repair/review records. Preserve unrelated dirty files. Do not amend or
force-push; the PR will be squash-merged to keep main history coherent. Do not
create a version tag or change image labeling policy.
