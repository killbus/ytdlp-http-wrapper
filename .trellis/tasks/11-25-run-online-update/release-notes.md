# v1.1.0

`/run` now executes standalone `--update` and `-U` as service-level yt-dlp updates.
Both the CLI binary and resident Python package update together, and the response
reports the installed version or explicitly says it is already current.

```bash
curl -s http://localhost:8080/run \
  -H 'Content-Type: application/json' \
  -d '{"args":["--update"],"timeout_seconds":300}'
```

Updates drain active requests, stop resident workers, validate the replacement
runtime and restart the selected backend before returning success. Failed
installation or activation restores the previous runtime; interrupted transactions
recover at the next boot. New executions wait while an update is running.

The selected online version persists in the libs volume across restarts with the
same `RUNTIME_YTDLP_VERSION`. Changing that configured pin selects the new explicit
version. The pin is an initial version and selection anchor; after an update it
can differ from the running version. There are no background or scheduled updates.
Custom source plans and external worker overrides explicitly reject online updates.

Since v1.0.0, this release also includes:

- Resident Python workers and validated runtime installation, with uv managing
  Python and package setup.
- Official fixed-version binary downloads and checksum verification without
  depending on the anonymous GitHub release API.
- Restored EJS dependencies and runtime reuse checks for resident execution.
- Windows temporary-path handling compatible with PyInstaller extraction.
- Platform-specific release archive names for Linux x86_64/ARM64,
  macOS x86_64/ARM64 and Windows x86_64.

Docker images are published for Linux AMD64 and ARM64. Use
`ghcr.io/killbus/ytdlp-http-wrapper:1.1.0` to pin this release, or `:latest` to
follow published releases. Pull the image and recreate your container to install
the wrapper update; preserve its libs volume to retain runtime selections.
The `/run --update` command updates yt-dlp, not the wrapper executable.
