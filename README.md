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
| `--runtime-yt-dlp-version` | — | `RUNTIME_YTDLP_VERSION` | `2026.10.26` | Exact version for both the standalone binary and Python wheel; never `latest` |
| `--runtime-python` | - | `RUNTIME_PYTHON` | unset | uv request; reuse validated interpreter on updates |
| `--runtime-uv` | - | `RUNTIME_UV` | `uv` | Deployment-provided executable |
| `--runtime-python-release` | - | `RUNTIME_PYTHON_RELEASE` | unset | Removed PBS input; boot rejects with migration guidance |
| `--runtime-plan` | - | `RUNTIME_PLAN` | unset | Schema-2 pinned binary/optional wheel source plan |
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
- Resident runs an isolated CPython subprocess from a uv-created virtual
  environment. The worker protocol, process containment and pool are unchanged.
  The embedded worker script is copied into each prepared runtime.

```text
LIBS_DIR/                         # private to one instance, location-bound
  runtime/                        # the only installed runtime
    manifest.json                 # schema 2: request plus observed identities
    artifacts/                    # verified binary, optional pinned plan wheel
    bin/yt-dlp.exe                # Windows; bin/yt-dlp on Unix
    python/Scripts/python.exe     # Windows; python/bin/python on Unix
    python/                       # relocatable venv and uv-installed packages
    worker/ytdlp_worker.py
  uv-python/                      # stable managed base interpreters; never moved
  uv-cache/                       # private uv cache, not the user's global cache
  staging/<unique-attempt>/       # preparation only, never selectable
```

Installation completes before backend startup/listener binding. Explicit resident
interpreter/script overrides still win but never bypass installation validation.
Installed venv package paths come first; `--worker-python-path` appends extra paths.
Worker version must equal the explicit runtime pin. No per-request uv calls,
automatic latest, or periodic self-update are introduced.

### Python Ownership And Reuse

Deployment supplies uv (Docker pins official `ghcr.io/astral-sh/uv:0.11.29`);
Rust does not update it or enumerate Python releases. First install runs
`uv venv --no-project --relocatable --managed-python --python cpython`, letting
uv discover/download managed CPython. `--runtime-python` passes an explicit uv
request instead (version/specifier or external interpreter path). External
interpreters must remain available at their original location for the runtime's
lifetime. There is no compiled default Python minor series.

Next, `uv pip install --python <venv-python> --no-build --link-mode copy
"yt-dlp==<explicit-version>"` resolves the package and mandatory dependencies,
followed by `uv pip check`. No optional extras or source-build backends are enabled.
uv enforces Requires-Python, Requires-Dist and wheel compatibility against the
**already chosen interpreter**; it does not search all Python versions to satisfy
every dependency. Incompatibility rejects boot with uv's diagnostic: correct the
pin or `--runtime-python`, then restart. There is no automatic version fallback.
A read-only Python probe imports yt-dlp and loads the worker to check compatibility.

Updates prefer the previous validated base interpreter when the Python request
has not changed, rather than upgrading Python because yt-dlp changed. Base paths
are outside replaceable staging/runtime. `--relocatable` alone is **not** a portable
base-interpreter promise. Boot probes both staging and the published environment,
checks the venv prefix/import location and rejects bases in replaceable storage.
Do not move the whole private root while retaining its venv.

Identity records the dependency request, actual Python version/base path/executable
hash, distribution names/versions, binary pins and embedded worker hash. It is not
a universal package/interpreter lock. Valid reuse checks local artifacts and the
recorded environment without network metadata, uv resolution, or staging creation;
uv can even be absent. This does not rehash every Python library/package file or
lock transitive versions for a fresh rebuild. Missing/corrupt environments are
prepared again before use. There is no history store or old-version fallback.

### Sources And Prerequisites

Native installations need uv 0.11.29 (the deployment pin; other versions require
validation) and system curl. `--runtime-uv` / `RUNTIME_UV` overrides the executable;
use an absolute path when not on PATH. Binary acquisition uses the official
`releases/download/<version>/` URLs for the platform binary and `SHA2-256SUMS`,
without calling the GitHub release API or requiring a GitHub token. Exactly one
valid checksum entry for the platform binary is required; missing/invalid/ambiguous
entries or mismatched downloaded bytes reject installation. Verified bytes are retained.
uv owns Python/package downloads and installation, not Rust PBS URL/digest tables.
Boot ignores ambient `UV_*` configuration and project/uv.toml files, uses private
uv directories, and leaves standard proxy/TLS handling to the tools. curl artifact
limits remain 512 MiB and 300 seconds; these are not claimed as uv limits. Only Rust
artifact download failures retry (three attempts, cleanup first). uv failures reject
boot directly. Neither the base-interpreter store nor global caches are deleted.

Schema-1 plans and `--runtime-python-release` / `RUNTIME_PYTHON_RELEASE` are
**rejected**, not ignored. Remove the PBS input; use a uv request or a provisioned
stable interpreter. Schema-1 installed manifests trigger preparation before
retirement. Historical Trellis design documents describe the superseded schema.

Schema-2 plans retain a hash-pinned platform binary and optional hash-pinned
pure-Python yt-dlp wheel. Replace example paths/hashes with actual trusted inputs:

```json
{
  "schema_version": 2,
  "yt_dlp": "2026.10.26",
  "offline": true,
  "wheelhouse": "/absolute/offline-wheelhouse",
  "artifacts": [
    {"name": "yt-dlp_linux", "url": "file:///absolute/yt-dlp_linux", "sha256": "<64 hex digits>"},
    {"name": "yt_dlp-2026.10.26-py3-none-any.whl", "url": "file:///absolute/yt_dlp-2026.10.26-py3-none-any.whl", "sha256": "<64 hex digits>"}
  ]
}
```

`offline` defaults false; `wheelhouse` is optional. Offline sets uv's
`--offline --no-index`: provision a stable interpreter with `--runtime-python`
and supply all required wheels. uv checks the explicit version against wheel
metadata and resolves dependencies. Rust does not parse PEP 440/508. URLs may
use https:// or file://; endpoint changes alone do not change artifact identity.
The wheelhouse is an input source, not a transitive hash lock.

The default yt-dlp pin is `2026.10.26`. Official GitHub release metadata and PyPI
wheel metadata were verified available on 2026-11-08; the former `2026.09.23` pin
returned 404 on both sources. Metadata availability is not successful live-boot
evidence: CI must still exercise the real entrypoint. The installer never
silently substitutes a release, and offline fixtures are not live evidence.

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

Preparation acquires the binary and lets uv prepare/validate the candidate, then syncs its manifest
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
