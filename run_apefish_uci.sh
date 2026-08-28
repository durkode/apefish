#!/bin/bash
set -e

BASE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# Build first; send cargo's output to stderr so it can't corrupt the UCI
# stdout stream the GUI reads.
cargo build --release --manifest-path "${BASE_DIR}/Cargo.toml" --bin apefish --quiet >&2

exec "${BASE_DIR}/target/release/apefish" --uci
