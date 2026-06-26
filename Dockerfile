# syntax=docker/dockerfile:1
FROM rust:1.96-alpine AS chef
RUN cargo install cargo-chef --version 0.1.77 --locked

FROM chef AS planner
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/ytdlp-http-wrapper /app/ytdlp-http-wrapper

FROM debian:bookworm-slim

ARG TARGETARCH

RUN <<'EOF'
set -eux

# system user
groupadd -r app && useradd -r -g app -d /app -s /sbin/nologin app

# runtime deps
apt-get update
apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    dumb-init \
    libnghttp2-14 \
    unzip \
    xz-utils
apt-get clean

# static ffmpeg (musl-linked, zero shared library deps — no /usr/lib/* bloat)
case "$TARGETARCH" in
    amd64) FFMPEG_ARCH="amd64" ;;
    arm64) FFMPEG_ARCH="arm64" ;;
    *) echo "unsupported arch: $TARGETARCH" && exit 1 ;;
esac
curl -fsSL "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-${FFMPEG_ARCH}-static.tar.xz" \
    | tar -xJ --wildcards --strip-components=1 -C /usr/local/bin/ '*/ffmpeg' '*/ffprobe'

# deno (download, not a Debian package)
case "$TARGETARCH" in
    amd64) DENO_ARCH="x86_64" ;;
    arm64) DENO_ARCH="aarch64" ;;
    *) echo "unsupported arch: $TARGETARCH" && exit 1 ;;
esac
curl -fsSL "https://github.com/denoland/deno/releases/latest/download/deno-${DENO_ARCH}-unknown-linux-gnu.zip" \
    -o /tmp/deno.zip
unzip -j /tmp/deno.zip deno -d /usr/local/bin/
rm /tmp/deno.zip

# cleanup build-only packages
apt-get purge -y --auto-remove -o APT::AutoRemove::RecommendsImportant=false xz-utils unzip
rm -rf /var/lib/apt/lists/*

# directories
mkdir -p /app /downloads /app/.cache
chown -R app:app /app /downloads /app/.cache
EOF

COPY --link --from=builder /app/ytdlp-http-wrapper /app/ytdlp-http-wrapper

USER app:app
WORKDIR /downloads
VOLUME ["/downloads"]
EXPOSE 8080
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    RUST_LOG=info

ENTRYPOINT ["dumb-init", "/app/ytdlp-http-wrapper"]
