#!/usr/bin/env bash
# Robustness smoke test: a short, fast self-play match that fails (exit 1) if
# any game ends abnormally - time forfeit, illegal move, crash or stall - or the
# engine fails fastchess's UCI compliance check. Run this before trusting SPRT
# results, after touching time management or the UCI layer, and in CI.
#
# Usage: sprt/smoke.sh [options]
#   --ref REF          build to test: git ref or "worktree" (default)
#   --games N          games to play (default 200)
#   --tc TC            fastchess time control (default 2+0.02: deliberately
#                      faster than SPRT, to shake out clock handling)
#   --concurrency N    games in parallel (default: cores - 2)

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

REF="worktree"
GAMES=200
TC="2+0.02"
CONCURRENCY="$(default_concurrency)"

while [ $# -gt 0 ]; do
  case "$1" in
    --ref) REF="$2"; shift 2 ;;
    --games) GAMES="$2"; shift 2 ;;
    --tc) TC="$2"; shift 2 ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

warn_if_bot_running
FASTCHESS_BIN="$(ensure_fastchess)"
BOOK="$(ensure_book)"
read -r BIN LABEL < <(build_engine "${REF}")

RUN_DIR="${RUNS_DIR}/$(date +%Y%m%d-%H%M%S)_smoke_${LABEL}"
mkdir -p "${RUN_DIR}"
cp "${BIN}" "${RUN_DIR}/engine"

log "UCI compliance check"
if ! "${FASTCHESS_BIN}" --compliance "${RUN_DIR}/engine" --uci > "${RUN_DIR}/compliance.log" 2>&1; then
  sed 's/\x1b\[[0-9;]*m//g' "${RUN_DIR}/compliance.log" | grep -iE 'fail|error' >&2 || true
  log "FAIL: UCI compliance check failed, see ${RUN_DIR}/compliance.log"
  exit 1
fi

log "smoke: ${GAMES} self-play games of ${LABEL} at tc=${TC}, concurrency=${CONCURRENCY}"
(cd "${RUN_DIR}" && "${FASTCHESS_BIN}" \
  -engine cmd=./engine args=--uci name=a \
  -engine cmd=./engine args=--uci name=b \
  -each proto=uci tc="${TC}" option.Hash=8 \
  -openings file="${BOOK}" format=pgn order=random \
  -repeat -rounds $(( (GAMES + 1) / 2 )) \
  -concurrency "${CONCURRENCY}" \
  -ratinginterval 0 \
  -pgnout file=games.pgn \
  -recover) > "${RUN_DIR}/fastchess.log" 2>&1 \
  || { log "FAIL: fastchess exited with an error, see ${RUN_DIR}/fastchess.log"; exit 1; }

PLAYED="$(grep -c '^\[Result ' "${RUN_DIR}/games.pgn" || true)"
ABNORMAL="$(abnormal_terminations "${RUN_DIR}/games.pgn")"

if [ "${PLAYED}" -lt "${GAMES}" ]; then
  log "FAIL: only ${PLAYED}/${GAMES} games completed, see ${RUN_DIR}/fastchess.log"
  exit 1
fi
if [ -n "${ABNORMAL}" ]; then
  log "FAIL: abnormal game endings in ${PLAYED} games:"
  echo "${ABNORMAL}" >&2
  grep -hE 'loses on time|stalls|disconnects|illegal move' "${RUN_DIR}/fastchess.log" | head -n 10 >&2 || true
  log "details: ${RUN_DIR}"
  exit 1
fi
log "PASS: ${PLAYED} games, no time losses, illegal moves or crashes"
