#!/bin/bash

BASE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
"${BASE_DIR}/target/release/apefish" --uci
