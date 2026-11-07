# ytdlp-http-wrapper

Lightweight HTTP wrapper that executes `yt-dlp` commands and returns JSON results.
Before serving, boot installs or validates one pinned runtime under `--libs-dir`.
The CLI backend remains the default; the resident backend is explicitly selected.

## Quick Start

```bash
docker run -d -p 127.0.0.1:8080:8080 \
  -e HOST=0.0.0.0 -e MAX_CONCURRENT_PROCESSES=4 \
  --tmpfs /tmp:rw,exec,nosuid,nodev,size=512m,mode=1777 \
  --memory 1g --stop-timeout 15 \
  ghcr.io/killbus/ytdlp-http-wrapper

# download audio from YouTube
curl -s -X POST http://localhost:8080/run \
  -H "Content-Type: application/json" \
  -d '{"args": ["-f", "bestaudio", "https://www.youtube.com/watch?v=dQw4w9WgXcQ"]}'
```

## API

### `POST /run` | `GET /run`

| Field | Type | Required | Default |
|---|---|---|---|
| `args` | `string[]` | yes | — |
| `timeout_seconds` | `int` | no | `30` |

Response `200 OK`:
```json
{ "exit_code": 0, "stdout": "...", "stderr": "" }
```

Timeouts return HTTP 200 with `exit_code: -1`. The 1–300 second execution
budget excludes queueing and includes output pipe completion. Termination can
add 3 seconds of Unix SIGTERM grace plus up to 2 seconds for forced process
cleanup; filesystem cleanup and response delivery take additional time. Windows
terminates the JobObject immediately. Each output stream retains its first
10 MiB and drains excess bytes without retaining them.

## Configuration

All options accept CLI flags (local dev) or environment variables (Docker). CLI flags take precedence.

| Flag | Short | Env | Default | Description |
|---|---|---|---|---|
| `--host` | — | `HOST` | `127.0.0.1` | Bind address |
| `--port` | `-p` | `PORT` | `8080` | Listen port |
| `--libs-dir` | `-l` | `LIBS_DIR` | `libs` | Private installation root containing `runtime/` and `staging/` |
| `--runtime-yt-dlp-version` | — | `RUNTIME_YTDLP_VERSION` | `2026.09.23` | Exact version for both the standalone binary and Python wheel; never `latest` |
| `--runtime-python-release` | — | `RUNTIME_PYTHON_RELEASE` | `20261026` | python-build-standalone release tag; Python version resolved from the selected archive |
| `--runtime-plan` | — | `RUNTIME_PLAN` | unset | JSON source plan with explicit artifact URLs and SHA256s; overrides live resolution |
| `--backend` | — | `BACKEND` | `cli` | `cli` or `resident`; no automatic fallback |
| `--worker-python` | — | `WORKER_PYTHON` | installed runtime interpreter | Explicit interpreter override for resident only |
| `--worker-path` | — | `WORKER_PATH` | installed runtime worker copy | Explicit script override for resident only |
| `--worker-python-path` | — | `WORKER_PYTHONPATH` | unset | Extra package paths, separated by `;` on Windows or `:` on Unix; appended after runtime packages |
| `--worker-yt-dlp-version` | — | `WORKER_YTDLP_VERSION` | runtime yt-dlp version | Must equal the runtime pin if set; mismatch fails boot with `RUNTIME_VERSION_MISMATCH` |
| `--worker-pool-size` | — | `WORKER_POOL_SIZE` | `1` | Resident pool capacity; must be at least 1, including when CLI is selected |
| `--max-concurrent` | — | `MAX_CONCURRENT_PROCESSES` | CPU×2 | Max concurrent yt-dlp processes |
| `--temp-dir` | — | `YTDLP_TEMP_DIR` | OS temp directory | Existing parent for isolated request temporary files |
| `--denied-args` | — | `DENIED_ARGS` | *(built-in list)* | JSON blocklist; `[]` to allow all |
| | | `RUST_LOG` | `info` | Tracing level (EnvFilter) |

```bash
# with CLI flags
cargo run -- --host 0.0.0.0 -p 3000 -l /tmp/libs

# with env vars (Docker style)
HOST=0.0.0.0 PORT=3000 cargo run
```

### One Installed Runtime

Both backends use the same completed installation, but consume distinct artifacts:

- CLI executes the official standalone yt-dlp binary from `runtime/bin/` per
  request. It does **not** launch `python -m yt_dlp`.
- Resident runs the installed Python interpreter and imports the pinned `yt_dlp`
  wheel from `runtime/python/site-packages/`. The worker script is embedded from
  `worker/ytdlp_worker.py` when the wrapper is built, then copied into the runtime;
  it is not downloaded from upstream.

```text
LIBS_DIR/                         # default: libs/
  runtime/                        # the only installed runtime
    manifest.json
    artifacts/                    # retained binary, Python tarball and wheel
    bin/yt-dlp.exe                # Windows; bin/yt-dlp on Unix
    python/python.exe             # Windows; python/bin/python3 on Unix
    python/site-packages/yt_dlp/
    worker/ytdlp_worker.py
  staging/<unique-attempt>/        # preparation only, never selectable
```

Installation completes before backend startup and listener binding, including
when explicit resident interpreter/script overrides are supplied. Overrides win
over the installed interpreter/script paths, but do not bypass runtime validation.
The runtime package path stays first; `--worker-python-path` adds entries after it
(unset or empty adds none). The worker handshake must report the runtime yt-dlp
pin: an equal explicit `--worker-yt-dlp-version` is accepted; a different one
rejects boot without retry, before backend startup, in either backend mode. Set
`--runtime-yt-dlp-version` too when retaining an older explicit worker pin.

Reuse requires the **full identity**, not just matching a version string or the
abbreviated `runtime_id`: platform, yt-dlp and Python versions/release, artifact
names/channels/resolved tags/SHA256s, and the embedded worker hash must match.
Boot verifies retained artifacts, the installed standalone binary and worker copy,
and the presence/containment of required Python/package paths. It does not rehash
the whole extracted Python tree or promise detection of every extracted-file edit.
Matching valid content is reused without artifact downloads or staging creation.
Runtime identity (also called `generation_id` in the worker handshake) is metadata,
not a version store, selector, or generation manager.

Missing, mismatched or corrupt content triggers preparation of a verified
replacement. There is no history, backup, rollback, or old-version fallback.
Changing the embedded worker in a new wrapper build also changes runtime identity.

### Sources And Prerequisites

The standalone binary comes from the exact yt-dlp GitHub release, Python from
astral-sh/python-build-standalone, and the wheel from PyPI. Without a plan, boot
resolves pinned release metadata even when checking an existing installation.
A source plan is recommended for production pinning and offline reuse: it takes
precedence over live endpoints, requires schema version 1, matching yt-dlp and
Python release versions, and one platform-correct binary, Python tarball and wheel,
each with a URL and 64-hex SHA256. URLs may use `https://` or local `file://`; URL
changes alone do not change identity if the pinned content is unchanged. See the
[source-plan schema](.trellis/tasks/11-06-immutable-runtime-install/design.md#6-runtime-source-plan-test-seam--production-pinning).

System `curl` handles HTTPS transfers using its inherited proxy environment and
CA configuration; the wrapper adds no custom CA store. System `tar` extracts the
Python archive; wheel extraction and SHA256 verification use the `zip` and `sha2`
crates. Missing tools or missing/unusable digests fail boot, never fall back to an
unverified artifact. Transfers are bounded by 512 MiB per artifact and 300 seconds
per attempt. Only pre-retirement download errors retry, at most three attempts
with 2s/4s backoff, and only after successful cleanup of this attempt's staging.
Cleanup uncertainty/failure rejects boot and prevents retry.

The default pins are configured values, **not a claim of current upstream
availability**. The default PBS tag was selected for CPython `3.13.18`; verify
the actual platform artifact/version and trusted hashes before rollout. Live
default availability, the API-digest trust choice when no plan is supplied,
transfer limits, system-tool availability and the wheel's dependency closure
remain rollout checks (A1-A6); offline fixture tests do not establish them.

### Deployment And Recovery (A8/A9)

Use one **private canonical `LIBS_DIR` for one instance's entire serving lifetime**.
Deployment must serialize **stop -> await all old consumers and children -> start**,
including backend/installer children and any external runtime consumers. No other
wrapper or filesystem writer may share the root. `runtime/` and `staging/` must be
ordinary directories on one local filesystem, not separate mounts, symlinks or
reparse points. `runtime/` is reserved for the installer, even with a missing or
corrupt manifest. This is an operator-enforced precondition, not automatic locking,
leases, or shared-root multi-instance support. Give overlapping instances separate
private roots instead of rolling replacements in the same root.

Preparation downloads, extracts and verifies the candidate, then syncs its manifest
(and candidate directory on Unix) before touching the old runtime.
**Preparation failure preserves old bytes but rejects boot.**
After verification, publication invalidates the old manifest, removes the old tree,
then renames the candidate into the now-absent `runtime/` path. On Unix the specified
directory syncs also run; Windows sharing/permission failures are surfaced, not
bypassed with copy-over. There is no saved old tree or restore transaction.

Deletion plus rename is **not atomic** and does not promise old-or-new crash or
power-loss durability. Failure or death after retirement starts may leave
`runtime/` absent or invalid; boot fails without starting a backend. A failed
post-rename sync/check may leave the candidate at `runtime/`, but returns no usable
paths. The next startup validates that location and reinstalls if necessary.
Correct the reported source, permission or filesystem issue and restart under the
same stop/await/start rule; do not expect service to resume using an old version.

Only this invocation's owned staging is cleaned, after its writers/children are
stopped and awaited; no complete manifest is required for partial-work cleanup.
Uncertain ownership/quiescence or failed cleanup is reported and work is preserved.
Another invocation's or a crashed boot's staging is never adopted or swept by age
or marker. Legacy flat `libs/yt-dlp(.exe)` files and old `versions/` stores are
neither used nor migrated/deleted automatically. While all consumers are stopped,
identify and manually move/remove only confirmed obsolete entries and inactive
staging, or choose a fresh private root. **Do not blanket-delete `libs/`.**

## Installation

### Docker (recommended)

```bash
docker run -d -p 127.0.0.1:8080:8080 \
  -e HOST=0.0.0.0 -e MAX_CONCURRENT_PROCESSES=4 \
  --tmpfs /tmp:rw,exec,nosuid,nodev,size=512m,mode=1777 \
  --memory 1g --stop-timeout 15 \
  ghcr.io/killbus/ytdlp-http-wrapper
```

For a local build with the same limits and a downloads volume:

```bash
docker compose up --build -d
```

See [process lifecycle operations](docs/process-lifecycle.md) for memory sizing,
shutdown, residue migration and Linux validation. These settings take effect
when the image/configuration is rebuilt and deployed.

### Binary (Linux only)

Download from [GitHub Releases](https://github.com/killbus/ytdlp-http-wrapper/releases):

```bash
curl -fsSL https://github.com/killbus/ytdlp-http-wrapper/releases/latest/download/ytdlp-http-wrapper-<tag>-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz
./ytdlp-http-wrapper
```

macOS/Windows users should use the Docker image.

## Development

```bash
scripts/dev.sh          # run locally
python scripts/ci.py    # shared Windows/Linux fixture gates (or bash scripts/ci.sh)
python scripts/ci.py release  # locked host release build
```

See [validation scripts](scripts/README.md) for prerequisites, lightweight script
tests, separate networked Linux/container checks, and evidence boundaries.

## Build

```bash
cargo build --release
docker build -t ytdlp-http-wrapper .
```

See [SPECS.md](SPECS.md) for full technical specification.
