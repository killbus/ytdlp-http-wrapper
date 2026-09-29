#!/bin/sh
# Run inside the image with compose.yaml's settings, as the image's app user.
set -eu
test "$(id -u)" != 0
test "$YTDLP_TEMP_DIR" = /tmp
test "$MAX_CONCURRENT_PROCESSES" = 4
test "$(stat -f -c %T /tmp)" = tmpfs
test "$(stat -c %a /tmp)" = 1777
test "$(df --block-size=1 --output=size /tmp | tail -n 1 | tr -d ' ')" = 536870912
awk '$2 == "/tmp" && $3 == "tmpfs" {
    n = split($4, options, ",")
    for (i = 1; i <= n; i++) flags[options[i]] = 1
    found = flags["rw"] && flags["nosuid"] && flags["nodev"] && !flags["noexec"]
} END { exit !found }' /proc/mounts
request_dir=$(mktemp -d /tmp/ytdlp-deployment-check.XXXXXX)
trap 'rm -rf -- "$request_dir"' EXIT
printf '#!/bin/sh\nexit 0\n' > "$request_dir/probe"
chmod +x "$request_dir/probe"
"$request_dir/probe"
echo 'Verified non-root writable/executable tmpfs with 512 MiB capacity and restrictive mount options'
