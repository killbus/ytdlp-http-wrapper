#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if command -v python >/dev/null 2>&1; then
    exec python "$root/scripts/ci.py" "$@"
fi
exec python3 "$root/scripts/ci.py" "$@"
