# Temporary storage and process lifecycle

Each admitted request gets a random, private `ytdlp-request-*` directory. The
wrapper sets only that child's `TMPDIR`, `TMP` and `TEMP`, so PyInstaller extracts
inside this directory. Downloads still use the normal working directory and
`--paths` settings; libraries still use `LIBS_DIR`. Programs or user flags that
explicitly choose other temporary locations are outside this cleanup boundary.

The supervisor owns the process, directory and concurrency permit independently
of the HTTP handler. A dropped handler requests cancellation; cleanup continues.
Not every network disconnect cancels an Axum handler immediately, so the execution
deadline remains the backstop. Queue cancellation never starts a process.

On timeout, cancellation or service shutdown, Unix sends the process group
SIGTERM, waits up to 3 seconds, then sends SIGKILL to any remaining members.
Windows terminates the JobObject immediately. The direct child and both output
pipes must finish; force-kill cleanup waits at most 2 seconds. Descendants are
terminated even if the leader exits normally. Only then is the owned directory
removed and the permit returned. Output retains at most 10 MiB of bytes per
stream; the rest is drained. UTF-8 replacement and JSON encoding can expand the
response in memory.

Process groups do not contain descendants that deliberately create a new session.
Container isolation is required for untrusted workloads. Unix cleanup also waits
for the group to disappear, including zombie members: run with a functioning init
reaper (the image uses dumb-init). A stuck/unreaped group fails closed instead of
removing files while process state is uncertain.

Infrastructure failures return the existing `{error, code}` JSON shape. A cleanup
failure logs `CLEANUP_FAILURE` or `TEMP_CLEANUP_FAILURE` and stops admission; inspect
the logged path before restarting. Process-cleanup failures preserve the directory.
Do not automatically restart in a tight loop after persistent cleanup failures.
Timeout responses remain HTTP 200 with `exit_code: -1`. A shut-down admission queue
returns HTTP 503. The execution budget is 1–300 seconds (default 30), excluding
queue time. Allow up to 5 seconds extra for Unix termination, plus filesystem
cleanup and response delivery; logs include cleanup time. Filesystem stalls are
not a hard bounded operation.

## Container settings and capacity

`compose.yaml` provides `/tmp:rw,exec,nosuid,nodev,size=512m,mode=1777`, four
concurrent processes, a 1 GiB memory limit and 15 seconds of stop grace. `exec`
allows executable mappings of unpacked libraries. `1777` allows the image's
non-root user to create private directories. Keep downloads on their volume.

512 MiB and 1 GiB are starting points, not universal guarantees. The incident's
approximately 23 MiB per extraction means four concurrent extractions need about
92 MiB before other temporary files. Budget separately for ffmpeg, downloads
using temporary storage, up to 20 MiB retained output per request, JSON copies,
and the service itself. tmpfs consumes the container's memory budget (and may
use swap); either filesystem exhaustion or memory pressure/OOM can occur.
Monitor temp usage, container memory, timeout rates and cleanup failures under
representative concurrency before increasing the limits.

tmpfs does not periodically reclaim files. The application cleans each completed
request; tmpfs is the fallback for abrupt wrapper death. Its contents disappear
when the container stops. A mere wrapper-process restart inside a running
container does not clear tmpfs. SIGKILL/OOM cannot run Rust destructors.

## Migration and operations

Stop admission and let current requests drain. Preserve the downloads volume,
then recreate the old container using the new image and tmpfs configuration.
Mounting tmpfs over an existing overlay `/tmp` only hides old files; recreating
the old container removes that old writable layer. Never remove the downloads
volume as part of this operation.

On bare metal, stop the wrapper and confirm its process groups have exited before
removing specifically reviewed, owned leftover directories. Do not sweep all
`_MEI*` files by age: long-running or unrelated processes can still use them.
No production cleanup or deployment is performed by this repository change.

`SIGTERM` (Unix) and Ctrl-C trigger explicit service cancellation. Shutdown closes
admission and awaits supervised cleanup. On Windows a pathological kernel wait
can leave a dependency's blocking job waiter alive; its handle is preserved on
timeout, admission closes, and runtime teardown has a two-second wait limit.

## Verification

`cargo test --locked` uses the test executable as native child-process fixtures
on Linux and Windows. The fixture tests themselves are ignored in the top-level
test run and are launched by the lifecycle integration tests. They cover normal
exit, spawn failure, repeated timeout, cancellation, queueing, shutdown, concurrent
isolation, descendants and output flooding. Unix adds SIGTERM cooperation,
ignored SIGTERM and cancellation during grace.

For a real Linux PyInstaller artifact, download `yt-dlp_linux` plus the matching
release's `SHA2-256SUMS`, verify its checksum, make it executable, then run:

```bash
YTDLP_SMOKE_BINARY=/absolute/path/yt-dlp_linux bash scripts/linux-smoke.sh
```

The smoke test uses a local stalled HTTP server, observes actual `_MEI` extraction,
and checks three timeout/reclamation cycles. CI resolves and logs a release tag
before downloading both files from that release. Pin the logged tag when
reproducing. Run the deployment example on Linux to verify tmpfs mount options
and memory sizing separately. Windows test success does not establish Linux
signal or PyInstaller behavior.

The Docker CI job also runs `scripts/container-smoke.sh` inside the built image
with the Compose settings. It checks the non-root user's writable/executable
tmpfs, 512 MiB capacity, mode 1777, and nosuid/nodev mount options. This probe
does not establish application shutdown behavior or workload memory sufficiency.
