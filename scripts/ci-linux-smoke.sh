#!/usr/bin/env bash
set -euo pipefail
set +x
unset GH_DEBUG
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

if [[ $# -gt 1 ]]; then
    echo 'Usage: bash scripts/ci-linux-smoke.sh [official-release-tag]' >&2
    exit 2
fi
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    echo 'Official yt-dlp_linux smoke requires native x86_64 Linux.' >&2
    exit 1
fi
for tool in gh sha256sum awk mktemp chmod rm cargo rustc; do
    command -v "$tool" >/dev/null || { echo "Required tool missing: $tool" >&2; exit 1; }
done

release=${1:-$(gh release view --repo yt-dlp/yt-dlp --json tagName --jq .tagName)}
if [[ ! $release =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
    echo 'Expected a nonempty official release tag, not an option or path.' >&2
    exit 1
fi
echo "Testing yt-dlp release $release"
artifact_parent=$(cd "${TMPDIR:-/tmp}" && pwd -P)
artifact_dir=$(mktemp -d "$artifact_parent/ytdlp-ci-smoke.XXXXXXXX")
cleanup() {
    status=$?
    if [[ -L $artifact_dir || $(cd "$artifact_dir" && pwd -P) != "$artifact_dir" ||
          ${artifact_dir%/*} != "$artifact_parent" || ${artifact_dir##*/} != ytdlp-ci-smoke.* ]]; then
        echo 'Smoke cleanup target is no longer an owned directory under the temp parent.' >&2
        exit 1
    fi
    if ! rm -rf -- "$artifact_dir"; then
        echo "Cannot clean owned smoke directory: $artifact_dir" >&2
        exit 1
    fi
    exit "$status"
}
trap cleanup EXIT
gh release download "$release" --repo yt-dlp/yt-dlp \
    --pattern yt-dlp_linux --pattern SHA2-256SUMS --dir "$artifact_dir"

# Validate only the exact artifact, requiring one usable checksum record. Never
# let an unrelated verified file or --ignore-missing stand in for this binary.
if ! expected=$(awk '
    $2 == "yt-dlp_linux" || $2 == "*yt-dlp_linux" {
        if (NF != 2) exit 1
        count++; hash = $1
    }
    END { if (count != 1) exit 1; print hash }
' "$artifact_dir/SHA2-256SUMS") || [[ ! $expected =~ ^[a-fA-F0-9]{64}$ ]]; then
    echo 'Expected exactly one 64-hex SHA256 for yt-dlp_linux.' >&2
    exit 1
fi
(cd "$artifact_dir" && printf '%s  yt-dlp_linux\n' "$expected" | sha256sum --check --strict -)
echo "Verified yt-dlp release $release SHA256 $expected"
chmod +x "$artifact_dir/yt-dlp_linux"
export YTDLP_SMOKE_BINARY="$artifact_dir/yt-dlp_linux"
bash "$root/scripts/linux-smoke.sh"
