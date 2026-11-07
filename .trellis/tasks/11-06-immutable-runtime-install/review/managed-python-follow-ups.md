# Review and remaining evidence

- Independent code review is next. This is an implementation handoff, not an
  approval/verdict or completion of the overarching Trellis task.
- GitHub CI must run strict locked Clippy, Rust tests/release, native Windows and
  Linux offline real-uv fixtures, existing lifecycle/onefile checks, Docker build,
  existing containment smoke and the new real-entrypoint boot/reuse smoke.
- The default yt-dlp pin 2026.09.23 was deliberately NOT changed. Owner metadata
  evidence reports PyPI 404. Resolve that deployment-pin decision explicitly;
  do not weaken the new boot gate or silently choose latest to obtain green CI.
- Review stable-base paths on both OSes, package dependency/metadata handling,
  native lock/failure cleanup and post-rename probe behavior with actual CI output.
  Read-only uv help/source evidence is not claimed as executed environment proof.
- Schema-1 source plans/PBS release inputs are explicitly rejected. Migration is
  schema-2 binary/optional wheel inputs plus a uv Python request or stable external
  interpreter. uv's private base/cache directories are not wrapper runtime history
  and have no automatic cross-boot scavenger. Operator cleanup requires stopped
  consumers; deleting them can invalidate an otherwise intact runtime.
- No spec files were edited under this role's ownership. After review, the owner
  should reconcile historical dependency-source/install guidance with M1-M6.
- No git staging/commit/push, release, deployment or task archival is authorized
  or performed by this handoff.
