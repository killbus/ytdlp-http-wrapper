# Resident yt-dlp worker and runtime generation design

## 1. Context

The current Rust service prepares a PyInstaller yt-dlp executable through the
Rust `yt-dlp` crate and starts a new executable for every `/run` request. Each
request therefore pays for process creation, PyInstaller extraction, Python
initialization, imports, extractor loading, and temporary-directory cleanup.

The target architecture keeps Rust as the control plane and moves extraction
into supervised resident Python workers. It also removes the Rust `yt-dlp`
crate completely. Runtime acquisition, verification, activation, retention,
and rollback become project-owned responsibilities.

## 2. Architecture

```text
HTTP GET/POST /run
        |
        v
Rust request validation and admission
        |
        v
Backend router
  |                     |
  v                     v
Resident backend     Legacy CLI backend
  |                     |
  v                     v
WorkerManager        Existing supervisor
  |
  v
Active generation worker pool
  |
  v
Python worker -> fresh YoutubeDL instance per request

VersionManager
  |
  +-- discover and download candidate
  +-- verify immutable installation
  +-- create candidate generation
  +-- health/fixture/smoke validation
  +-- owner-gated promotion
  +-- drain previous generation
  +-- rollback and retention
```

### Rust responsibilities

- Preserve the public `/run` request and response contracts.
- Enforce denied arguments before admission.
- Own queueing, concurrency, deadlines, cancellation, shutdown, and tracing.
- Select the active backend and generation for each new request.
- Supervise worker processes and replace unhealthy workers.
- Own manifests, immutable installations, generation state, promotion,
  draining, rollback, and retention.
- Enforce the existing 10 MiB retained-output limit per stream.

### Python worker responsibilities

- Start once per worker and import yt-dlp once.
- Complete a versioned startup handshake.
- Process at most one request at a time.
- Create a new `YoutubeDL` object for every request.
- Keep request options, cookies, headers, proxy settings, paths, and output
  isolated.
- Return structured completion or failure frames.
- Exit when the protocol is corrupted or Rust requests retirement.

The Python process does not install or upgrade itself and does not decide which
generation is active.

The worker adapts the upstream Python API through `parse_options(argv)`. Source
verification at upstream revision
`b77c9304e1c4220cca450c583491904daf8455af` confirms that the returned
`ParsedOptions` exposes `urls` and `ydl_opts`. The adapter copies `ydl_opts`
and `urls` before constructing a fresh `YoutubeDL` instance. This source check
does not replace import and smoke validation of the exact candidate runtime.

## 3. Backend boundary

Introduce a backend abstraction whose resident and CLI implementations return
the existing `RunResponse` contract:

```text
run(args, deadline, cancellation, request_context)
    -> exit_code, stdout, stderr
```

The resident backend is the target default after compatibility gates pass. The
legacy CLI backend remains a controlled migration fallback, but it must use the
project-owned runtime installation rather than the removed Rust crate.

Backend selection must be explicit and observable. Silent fallback after a
resident execution has started is forbidden because it can duplicate side
effects. Fallback is allowed only before dispatch or through an explicit
operator-controlled mode.

## 4. Worker protocol

Use a versioned length-prefixed binary frame envelope carrying UTF-8 JSON
payloads. stdout is reserved for protocol frames; diagnostic logs go to stderr.

Each frame contains:

- `protocol_version`
- `message_type`
- `request_id`
- payload

The four-byte big-endian frame prefix is the sole byte-length authority for the
complete JSON envelope. The envelope deliberately does not repeat a serialized
payload length: Rust and Python may use different but equivalent JSON escaping
and whitespace, so recomputing a nested JSON byte length after decoding would
create false protocol failures.

Required message types:

- `hello`
- `ready`
- `run`
- `result`
- `error`
- `cancel`
- `shutdown`
- `ping`
- `pong`

The startup `hello` reports:

- `protocol_version`
- `yt_dlp_version`
- `python_version`
- `generation_id`
- `worker_id`

Rust rejects an inconsistent handshake before admitting the worker to the pool.
Frames exceeding configured limits, malformed JSON, unknown required message
types, request-ID mismatches, unexpected frame ordering, or protocol bytes on
stderr retire the worker.

## 5. Worker lifecycle and isolation

Worker states:

```text
STARTING -> HANDSHAKING -> IDLE -> BUSY -> IDLE
                               \-> RETIRING -> EXITED
                               \-> UNHEALTHY -> TERMINATED
```

- A worker handles one active request.
- A worker is leased exclusively until result delivery and request cleanup
  finish.
- Each request gets an owned temporary directory.
- No mutable options object is reused across requests.
- Cookies, headers, proxy settings, output templates, and paths are rebuilt
  from the request.
- A timeout or cancellation sends a cooperative cancel frame only when the
  worker protocol can prove it applies to the active request.
- If completion cannot be proven within the cancellation grace period, Rust
  terminates and replaces the whole worker.
- Worker crashes, EOF, framing errors, or excessive output fail only the active
  request and trigger replacement unless a wider invariant is uncertain.

The existing process-group/JobObject containment remains applicable to worker
processes and their descendants. Temporary files are removed only after worker
termination or verified request cleanup.

## 6. Runtime layout

Runtime installations are immutable:

```text
LIBS_DIR/
  versions/
    <runtime-id>/
      manifest.json
      python/
      worker/
      integrity.json
  generations/
    <generation-id>.json
  state/
    active.json
    previous.json
  staging/
```

`runtime-id` is derived from normalized version and integrity metadata, not
from a mutable label such as `latest`.

Candidate preparation:

1. Resolve an explicit yt-dlp version and platform-compatible Python runtime.
2. Download into a unique staging directory.
3. Verify checksums/signatures where the upstream artifact channel supports
   them.
4. Install without mutating any existing version.
5. Write and fsync the manifest and integrity metadata.
6. Atomically rename staging into `versions/<runtime-id>`.
7. Refuse to reuse an existing path whose manifest differs.

Partial staging directories are never selectable. Cleanup operates only on
owned, manifest-validated paths.

## 7. Generation management

A generation manifest binds:

- generation ID
- runtime ID
- yt-dlp version
- Python version
- worker protocol version
- worker artifact version
- pool configuration
- creation time
- integrity metadata

Generation states:

```text
DISCOVERED
-> DOWNLOADING
-> VERIFIED
-> INSTALLED
-> STARTING
-> HEALTHY
-> CANARY
-> ACTIVE
-> DRAINING
-> RETIRED
```

Failure before promotion marks the candidate failed and leaves active routing
unchanged.

Promotion atomically replaces the active-generation pointer for new requests.
Existing leases retain a strong reference to their original generation.
The previous generation stops receiving new work, drains in-flight requests,
then retires. At least active and previous stable installations are retained.

Rollback changes the new-request pointer back to the retained previous stable
generation without reinstalling it. A generation implicated in protocol or
integrity failure cannot be promoted automatically.

Candidate discovery, download, preparation, and validation may be automated.
Full production promotion requires owner approval by default.

## 8. Runtime installer ownership

Remove:

- the Rust `yt-dlp` dependency
- `LibraryInstaller` usage
- the `lofty` patch and transitive-only lockfile entries

The project-owned installer must cover:

- supported platform and architecture selection
- explicit version resolution
- retry with bounded backoff
- download size and timeout limits
- checksum/integrity verification
- executable permissions
- proxy and certificate behavior
- atomic staging and publication
- actionable structured errors
- startup reuse of a verified installed runtime

A depth-1 clone of the upstream Rust crate may be created outside committed
source paths for read-only research. Useful techniques may be reimplemented
under this project's conventions, but upstream code is not vendored and the
clone is not a build or runtime input.

## 9. Compatibility

The public API remains:

- GET and POST `/run`
- `args`
- optional `timeout_seconds`
- success shape: `exit_code`, `stdout`, `stderr`
- infrastructure failure shape: `error`, `code`

Timeout compatibility remains HTTP 200 with `exit_code: -1` unless a separately
reviewed API change is approved.

Before the resident backend becomes default, contract tests compare resident and
CLI results for local deterministic fixtures. Comparisons normalize only fields
explicitly known to be nondeterministic.

## 10. Observability

Every request log carries:

- request ID
- backend
- generation ID
- worker ID
- yt-dlp version
- queue duration
- worker-acquire duration
- execution duration
- first-response duration where applicable
- cleanup duration
- result and retirement reason

Metrics distinguish queue saturation, worker startup, handshake failure,
replacement rate, protocol failure, timeout, cancellation, candidate
validation, promotion, drain, rollback, runtime disk use, and worker RSS.
Credentials, cookies, headers, proxy URLs, and tokens must not be logged.

## 11. Validation strategy

Local validation stays lightweight according to the wrapper backend spec.
CI provides locked formatting, Clippy, tests, release builds, Linux runtime
smoke, and container checks.

Add deterministic tests for:

- frame encode/decode, partial reads, truncation, and oversized frames
- handshake compatibility and manifest mismatch
- single-request worker leasing
- request-state isolation
- cancellation and timeout worker retirement
- crash, EOF, protocol corruption, and pool replenishment
- output caps
- immutable installation and interrupted staging
- candidate validation failure
- atomic promotion and new-request routing
- in-flight draining
- rollback without reinstall
- retention safety
- resident/CLI contract equivalence
- repeated requests and bounded RSS growth
- Linux process groups and Windows JobObjects

External extractor smoke tests supplement deterministic fixtures but do not
replace them.

## 12. Rollout

1. Establish protocol and a single resident worker behind a non-default backend.
2. Add worker pool supervision and deterministic compatibility tests.
3. Add project-owned immutable runtime installation and remove the Rust crate
   and `lofty` patch in the same reviewed implementation.
4. Add candidate generation validation, promotion, drain, rollback, and
   retention.
5. Run shadow or explicit canary validation.
6. Promote the resident backend only after owner approval and CI evidence.
7. Retain the CLI backend until the acceptance gates pass; removing that backend
   requires a later explicit decision.

## 13. Rejected alternatives

### Bump the Rust `yt-dlp` crate

Rejected because it only refreshes the installer dependency and does not remove
per-request Python/PyInstaller startup or provide resident workers and
generation management.

### Upgrade packages in the active environment

Rejected because partial upgrades and already-imported modules make rollback and
runtime identity unreliable.

### `importlib.reload` inside live workers

Rejected because yt-dlp and its dependencies do not provide a transactional
whole-process reload boundary.

### Concurrent requests inside one Python worker

Rejected for the initial architecture because mutable library and process-global
state would complicate isolation, cancellation, and fault containment.

### Silent per-request fallback

Rejected because retrying a partially executed request on another backend can
duplicate downloads or other side effects.
