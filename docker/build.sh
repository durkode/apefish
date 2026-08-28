#!/usr/bin/env bash
# Build the apefish-bot OCI image:
#   1. compile apefish as a static x86_64 musl binary on the host
#   2. stage it next to the Containerfile
#   3. build the image with podman (or docker)
#
# Usage: docker/build.sh
# Env:   IMAGE=apefish-bot:latest   CONTAINER_ENGINE=podman

set -euo pipefail

TARGET="x86_64-unknown-linux-musl"
IMAGE="${IMAGE:-apefish-bot:latest}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${HERE}/.." && pwd)"

ENGINE="${CONTAINER_ENGINE:-}"
if [ -z "${ENGINE}" ]; then
  ENGINE="$(command -v podman || command -v docker || true)"
fi
if [ -z "${ENGINE}" ]; then
  echo "error: need podman or docker on PATH (or set CONTAINER_ENGINE)" >&2
  exit 1
fi

cd "${REPO_ROOT}"

if ! rustup target list --installed 2>/dev/null | grep -qx "${TARGET}"; then
  echo ">> adding Rust target ${TARGET}" >&2
  rustup target add "${TARGET}"
fi

echo ">> building apefish (${TARGET}, release)" >&2
cargo build --release --target "${TARGET}" -p apefish-cli

cp "target/${TARGET}/release/apefish" "${HERE}/apefish.bin"

echo ">> building image ${IMAGE} with $(basename "${ENGINE}")" >&2
"${ENGINE}" build -t "${IMAGE}" "${HERE}"

echo ">> done: ${IMAGE}" >&2
