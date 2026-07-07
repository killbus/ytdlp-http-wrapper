# PRD: Performance & Concurrency Refactor

## Requirements

1. **Cascade subprocess termination**: When `yt-dlp` times out, is cancelled, or exits abnormally, all its spawned child processes (`ffmpeg`/`aria2c` etc.) must be forcefully terminated together — no orphans left behind
2. **Concurrency throttling**: Prevent unbounded physical process growth under high concurrency that leads to OOM / FD exhaustion / CPU thrashing. Requests exceeding the concurrency limit should queue or be rejected
3. **Safety constraint compliance**: All Rust source must keep `unsafe_code = "forbid"` lint rule — no custom FFI
4. **Cross-platform compatibility**: The solution must work on Linux (Docker production) and Windows (local dev)
5. **Existing functionality unchanged**: `/run` endpoint request/response format, timeout logic, output truncation (10MB) and other existing behavior must not change

## Acceptance Criteria

- [ ] `cargo build --release` passes with no warnings and no `unsafe` code
- [ ] `cargo clippy` passes
- [ ] Linux: Run a long-lived `yt-dlp` request, cancel the client connection midway, `ps aux` confirms no `ffmpeg` / `yt-dlp` residual processes
- [ ] Under 50-concurrent-request test, actual physical processes do not exceed `MAX_CONCURRENT_PROCESSES` value
- [ ] Semaphore permit auto-releases on Future cancellation (no leak)
- [ ] Windows: JobObject binding failure degrades gracefully (does not block the service), logs a `warn`
