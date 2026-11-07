# Validation entrypoints

Run from the repository root, or supply an absolute script path from another
directory. Scripts select the repository root themselves. No script publishes
images/releases or touches deployed roots. The explicit real-boot smoke provisions
a runtime only in its own disposable container volume.

## Shared Windows/Linux gates

```sh
python scripts/ci.py check
python scripts/ci.py release
python scripts/ci.py script-tests
# Bash compatibility entrypoint; no argument means check:
bash scripts/ci.sh check
```

- `check`: prerequisite checks, fmt, locked all-target strict Clippy, locked host
  binary build, locked Rust suite, Python worker suite, then script unit tests.
- `release`: locked host release build, separate from tests and from publishing.
- `script-tests`: lightweight stdlib unittest discovery in `scripts/tests/` only;
  no Cargo build, upstream download, Docker daemon or runtime installation.

Prerequisites: rustup/Cargo/rustc with the toolchain and components selected by
`rust-toolchain.toml`; Python 3.10+ available as `python` or `python3`; system tar
supporting gzip archives, curl for fixture transfers, and uv 0.11.29; Bash and its standard
Unix tools for script tests (Git for Windows supplies these). CI explicitly
supplies Python 3.13 on both matrix
hosts. CI uses rustup's repository toolchain selection, not a separate stable pin.
Missing prerequisites fail rather than silently skipping fixture tests. An
ordinary non-root Linux user and a local temp filesystem honoring chmod are
required: the runner rejects root and tests that mode 0555 denies writes before
allowing the Rust permission suite. Do not use sudo to bypass this precondition.

The runner explicitly selects rustc's host triple, overriding cross-target Cargo
defaults so tests and the boot probe are runnable locally. Cargo still controls
its target directory (including `CARGO_TARGET_DIR` and repository Cargo config).
After a successful build, the runner reads that invocation's compiler-artifact
JSON and requires the reported executable to exist. It supplies the absolute
path as `CARGO_BIN_EXE_ytdlp-http-wrapper` only in the Rust test child environment.
It never guesses `target/debug`, accepts a stale environment value, or continues
after a missing artifact/build failure. Cargo may reuse up-to-date build outputs;
"fresh" means validated by the current successful Cargo build, not a forced clean.

Fixtures use local plans/artifacts and loopback/stub processes, not public artifact
downloads. Cargo dependency/toolchain acquisition is separate: a fresh runner may
need network access, so these commands intentionally do not default to `--offline`.
For already provisioned machines, `CARGO_NET_OFFLINE=true` prevents Cargo network
access; a missing cache is a prerequisite failure, not a runtime assertion.
The dependency graph fixture checks the resulting graph, not delivery history.

## Official Linux onefile smoke (networked preparation)

```sh
bash scripts/ci-linux-smoke.sh
bash scripts/ci-linux-smoke.sh 2026.10.26  # explicit reproduction tag, not a new default
```

Requires native x86_64 Linux, Bash, Cargo/toolchain, GitHub CLI (`gh`), sha256sum,
awk and normal filesystem tools. Authenticate gh through its normal configuration
or `GH_TOKEN`; CI supplies its read-only GitHub token to this step only. Never
enable credential-bearing debug output. By default the script preserves the old
`gh release view --repo yt-dlp/yt-dlp` release selection, then pins that returned
tag for one download of `yt-dlp_linux` and `SHA2-256SUMS` into a new private temp
directory. An explicit tag bypasses selection, not integrity verification.

It requires exactly one well-formed SHA256 entry for the exact binary and checks
the bytes before chmod or execution; unrelated checksums and missing/ambiguous
records cannot satisfy this gate. It prints the selected tag/hash, calls the
existing smoke consumer, and removes only its own temp directory on exit after
the foreground commands stop. No stale download directory is reused.

For an already verified executable, the no-download consumer remains available:

```sh
YTDLP_SMOKE_BINARY=/absolute/path/to/verified/yt-dlp_linux bash scripts/linux-smoke.sh
```

The consumer trusts the caller's verification, prints the binary version, and runs
the ignored `linux_ytdlp_smoke` test with output visible. The test uses a loopback
HTTP fixture to observe real onefile extraction and three timeout/cleanup cycles;
it does not fetch a public video. Cargo can still acquire missing dependencies.
The consumer explicitly selects the native Linux Rust host target too, so a
cross-target Cargo default cannot turn the Linux-only test into a zero-test run.
This is a separate standalone-binary test, not a copy of the production resolver,
managed Python validation, or evidence that runtime defaults/assumptions A1-A6 are valid.

## Container build and Compose smoke (networked build)

```sh
bash scripts/ci-container.sh
# CI-only cache integration, optional locally:
bash scripts/ci-container.sh --cache-from=type=gha --cache-to=type=gha,mode=max
```

Requires Linux, an accessible Linux Docker daemon, buildx, Compose v2 and network
access for the existing Dockerfile's build inputs. Uses the Dockerfile with its pinned official uv tool
and explicit repository compose.yaml; validates Compose, builds/loads the existing
`ytdlp-http-wrapper:local` tag, then runs `scripts/container-smoke.sh` as the image's
non-root user instead of starting the wrapper. Only optional build cache arguments
are accepted; there is no implicit Actions cache dependency or publish flag.

The unchanged Compose limits are 4 concurrent requests, 1 GiB container memory,
15s stop grace, and a 512 MiB mode-1777 executable tmpfs. The in-container smoke
asserts non-root, environment values, tmpfs type/size/mode and rw/nosuid/nodev/exec
mount flags, then executes a disposable probe in that tmpfs. It does not start a
real runtime install, exercise memory pressure or measure the shutdown deadline.
Compose may create its configured downloads volume; it is not deleted. The local
image tag is updated, not pruned. No user volumes/images are removed.

## Evidence and limits

The `rust` job retains Ubuntu/Windows matrix execution and the Linux-only official
smoke step; `docker` remains separate. Workflow YAML owns events, permissions,
concurrency, checkout/tool/cache setup and short calls, not validation policy.

Keep command exit codes, platform/tool versions, current host executable path,
Rust/Python test results, official tag/hash/version and smoke output, and container
build/Compose assertions with review evidence. A first failing command stops its
script and returns a nonzero exit. Unit tests use mocks and local shell stubs for
ordering, exit propagation, checksum rejection and path handling; these are not
native Linux permission, upstream artifact or Docker execution evidence.

Adding an invocation does not complete a pending gate. Hosted matrix execution,
native Linux permission/sync/link cases, real official-binary smoke, container
validation and actual release builds need their own run records. A release build
is not release publication; mount smoke is not production boot or deployment
acceptance. A1-A6 rollout confirmation and AC1's owner delivery-commit/history
check remain separate from these scripts.

## Real installer boot (CI-only networked gate)

```sh
python scripts/runtime-boot-smoke.py --image ytdlp-http-wrapper:local
# Only with an explicitly chosen deployment pin:
python scripts/runtime-boot-smoke.py --image ytdlp-http-wrapper:local --yt-dlp-version <pin>
```

The Docker workflow calls this in addition to the unchanged containment smoke.
It uses the image's real dumb-init/application entrypoint and non-root user, a
unique disposable downloads volume, 1 GiB/512 MiB tmpfs budgets and a 900-second
per-boot deadline. It checks CLI `/run --version`, clean shutdown, then resident
startup using the same runtime with network disabled and uv unavailable. The
second request must return the same explicit yt-dlp version and retain a
byte-identical manifest/runtime ID; logs must report `runtime_reused`. Cleanup
stops/awaits owned containers before removing their private volume; uncertain
stop retains the data. No deployed roots or existing volumes are used.

The default version is the owner-approved application pin 2026.10.26, not latest.
GitHub release and PyPI wheel metadata were verified available on 2026-11-08.
This gate inherits the application's default; it has no separate pin or fallback.
Metadata availability does not establish successful boot, and no real boot has
run locally for this change.

Offline Rust installer fixtures now require uv 0.11.29 and host Python. CI creates
venvs using that existing interpreter and synthetic local wheels with METADATA;
`--offline --no-index` prevents runtime/package downloads. These exercise actual
uv resolution/publication, not upstream availability. Synthetic command-injection
tests are separately identified in their test names.
