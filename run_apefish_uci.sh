#!/bin/bash

BASE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# UCI trace log for debugging GUI stalls. One file per engine launch. Remove
# this line (or unset APEFISH_UCI_LOG) to disable.
export APEFISH_UCI_LOG="${BASE_DIR}/apefish_uci_$(date +%Y%m%d_%H%M%S_%N).log"

# Forward any extra args (e.g. --hash 2048) through to the engine.
exec "${BASE_DIR}/target/release/apefish" --uci "$@"
