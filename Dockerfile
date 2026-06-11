# syntax=docker/dockerfile:1
FROM rust:1.96-alpine AS builder
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/ytdlp-http-wrapper /app/ytdlp-http-wrapper

FROM alpine:3.24

RUN <<'EOF'
# system user
adduser -D -h /app -u 1001 app

# dependencies
apk update
apk upgrade -a
apk add --no-cache \
    ca-certificates \
    dumb-init \
    ffmpeg \
    nghttp2 \
    python3 \
    zstd
apk add --no-cache -X https://dl-cdn.alpinelinux.org/alpine/edge/community \
    deno
rm -rf /var/cache/apk/*

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
