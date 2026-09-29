# Dependency Sources

- Keep Git patches pinned to a full commit hash and commit the matching lockfile.
- A successful locked/offline build only proves that the local cache is usable.
  When changing Git sources or investigating CI fetch failures, fetch the exact
  revision with a fresh Cargo Git cache before claiming the dependency is available.
- Prefer a reachable public upstream source when it serves the identical commit.
  Preserve the revision and package versions when only repairing a source URL;
  check all lockfile packages originating from that Git repository.
- The lofty patch uses public `Serial-ATA/lofty-rs` at
  `d2e41640481a48a95303d95939ba831767afcec8`. It preserves yt-dlp 2.7.2
  compatibility and the replacement of abandoned `paste` with `pastey`
  (RUSTSEC-2024-0436). The old `boul2gom/lofty-rs` endpoint returned 404.
  Do not remove the patch without verifying compatibility and its original purpose.
- Use CI for locked Clippy, tests and release builds; do not repeat heavyweight
  work locally. Local formatting and lockfile/source verification are sufficient
  before pushing a reviewed repair to the PR branch.
  Windows, Linux and Docker CI must resolve the corrected source; a failure before
  compilation is not evidence of a failing runtime assertion.
