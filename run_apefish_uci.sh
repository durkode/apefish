#!/bin/bash

BASE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# UCI trace log for debugging GUI stalls. One file per engine launch, under
# logs/ (git-ignored). Remove these two lines (or unset APEFISH_UCI_LOG) to
# disable. The engine won't create the directory itself, so mkdir it here.
mkdir -p "${BASE_DIR}/logs"
export APEFISH_UCI_LOG="${BASE_DIR}/logs/apefish_uci_$(date +%Y%m%d_%H%M%S_%N).log"

# Forward any extra args (e.g. --hash 2048) through to the engine.
exec "${BASE_DIR}/target/release/apefish" --uci "$@"
