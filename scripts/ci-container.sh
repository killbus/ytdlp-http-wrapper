#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

if [[ $(uname -s) != Linux ]]; then
    echo 'Container validation requires Linux and a Linux Docker daemon.' >&2
    exit 1
fi
cache_args=()
for arg in "$@"; do
    case "$arg" in
        --cache-from=?*|--cache-to=?*) cache_args+=("$arg") ;;
        *) echo 'Only --cache-from=VALUE and --cache-to=VALUE are supported.' >&2; exit 2 ;;
    esac
done
command -v docker >/dev/null || { echo 'Required tool missing: docker' >&2; exit 1; }
docker buildx version
docker compose version
docker compose -f "$root/compose.yaml" config --quiet
docker buildx build "${cache_args[@]}" --load --tag ytdlp-http-wrapper:local .
docker compose -f "$root/compose.yaml" run --rm --no-deps --pull never -T \
    --entrypoint /bin/sh ytdlp -eu < "$root/scripts/container-smoke.sh"
