#!/usr/bin/env bash
# Rebuild apefish and the image, then bounce the systemd --user service onto
# the fresh image.
#
#   1. cargo build (static musl) + podman build   -> via build.sh
#   2. stop the apefish-bot user service
#   3. daemon-reload (pick up any unit edit) + clear a tripped start limit
#   4. start it again on the new image
#
# Usage: docker/redeploy.sh
# Env:   IMAGE=apefish-bot:latest   CONTAINER_ENGINE=podman   UNIT=apefish-bot

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIT="${UNIT:-apefish-bot}"

if ! systemctl --user cat "${UNIT}" >/dev/null 2>&1; then
  echo "error: ${UNIT}.service is not installed - see 'Run overnight (Quadlet)'" \
       "in docker/README.md" >&2
  exit 1
fi

# 1: engine binary + image (honours IMAGE / CONTAINER_ENGINE)
"${HERE}/build.sh"

# 2: stop the running service, if it is running
if systemctl --user is-active --quiet "${UNIT}"; then
  echo ">> stopping ${UNIT}" >&2
  systemctl --user stop "${UNIT}"
fi

# 3: pick up any change to the unit file, clear a tripped start limit
systemctl --user daemon-reload
systemctl --user reset-failed "${UNIT}" 2>/dev/null || true

# 4: back up on the new image (the unit runs `podman run --replace` against
#    localhost/apefish-bot:latest, so a fresh start uses the rebuilt image)
echo ">> starting ${UNIT}" >&2
systemctl --user start "${UNIT}"

sleep 5
if systemctl --user is-active --quiet "${UNIT}"; then
  echo ">> ${UNIT} is active" >&2
else
  echo ">> WARNING: ${UNIT} is not active - see logs below" >&2
fi
journalctl --user -u "${UNIT}" -n 15 --no-pager || true
