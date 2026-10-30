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
#   --stall-timeout S  fail if no game finishes for S seconds (default: twice
#                      the longest a single game can last at --tc, min 60)
#   --timeout S        fail if the whole match takes over S seconds (default:
#                      twice the worst case for --games at --tc; 0 disables)
#
# Progress is printed every 10 seconds. On a stall or timeout the engine
# processes are listed (CPU-busy means a runaway search, idle means a
# deadlock or lost message), the unfinished games are named, and fastchess
# is stopped.

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

REF="worktree"
GAMES=200
TC="2+0.02"
CONCURRENCY="$(default_concurrency)"
STALL_TIMEOUT=""
TIMEOUT=""
POLL_SECONDS=10

while [ $# -gt 0 ]; do
  case "$1" in
    --ref) REF="$2"; shift 2 ;;
    --games) GAMES="$2"; shift 2 ;;
    --tc) TC="$2"; shift 2 ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --stall-timeout) STALL_TIMEOUT="$2"; shift 2 ;;
    --timeout) TIMEOUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

# Longest one game can legitimately take: both clocks plus 200 moves of
# increment each. "moves/base+inc" and "base" alone are accepted too.
MAX_GAME_SECONDS="$(awk -v tc="${TC##*/}" 'BEGIN {
  n = split(tc, p, "+"); base = p[1]; inc = (n > 1) ? p[2] : 0
  printf "%d", 2 * (base + 200 * inc) + 1 }')"
if [ -z "${STALL_TIMEOUT}" ]; then
  STALL_TIMEOUT=$(( 2 * MAX_GAME_SECONDS ))
  [ "${STALL_TIMEOUT}" -lt 60 ] && STALL_TIMEOUT=60
fi
if [ -z "${TIMEOUT}" ]; then
  ROUNDS_OF_GAMES=$(( (GAMES + CONCURRENCY - 1) / CONCURRENCY ))
  TIMEOUT=$(( 2 * ROUNDS_OF_GAMES * MAX_GAME_SECONDS + 120 ))
fi

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
log "limits: fail if no game finishes for ${STALL_TIMEOUT}s or the match exceeds ${TIMEOUT}s"
log "output: ${RUN_DIR}"
(cd "${RUN_DIR}" && exec "${FASTCHESS_BIN}" \
  -engine cmd=./engine args=--uci name=a \
  -engine cmd=./engine args=--uci name=b \
  -each proto=uci tc="${TC}" option.Hash=8 \
  -openings file="${BOOK}" format=pgn order=random \
  -repeat -rounds $(( (GAMES + 1) / 2 )) \
  -concurrency "${CONCURRENCY}" \
  -ratinginterval 0 \
  -pgnout file=games.pgn \
  -recover) > "${RUN_DIR}/fastchess.log" 2>&1 &
FASTCHESS_PID=$!
trap 'kill -INT "${FASTCHESS_PID}" 2>/dev/null || true' EXIT

game_numbers() { grep -oE "^$1 game [0-9]+" "${RUN_DIR}/fastchess.log" | awk '{ print $3 }' | sort -n; }
finished_count() { grep -c '^Finished game' "${RUN_DIR}/fastchess.log" 2>/dev/null || true; }

START=${SECONDS}
LAST_PROGRESS=${SECONDS}
LAST_FINISHED=0
FAILURE=""
while kill -0 "${FASTCHESS_PID}" 2>/dev/null; do
  sleep "${POLL_SECONDS}"
  kill -0 "${FASTCHESS_PID}" 2>/dev/null || break
  FINISHED="$(finished_count)"
  if [ "${FINISHED}" -ne "${LAST_FINISHED}" ]; then
    LAST_FINISHED="${FINISHED}"
    LAST_PROGRESS=${SECONDS}
  fi
  log "${FINISHED}/${GAMES} games finished ($(( SECONDS - START ))s elapsed)"
  if [ $(( SECONDS - LAST_PROGRESS )) -ge "${STALL_TIMEOUT}" ]; then
    FAILURE="no game finished in the last ${STALL_TIMEOUT}s"
    break
  fi
  if [ "${TIMEOUT}" -gt 0 ] && [ $(( SECONDS - START )) -ge "${TIMEOUT}" ]; then
    FAILURE="match exceeded ${TIMEOUT}s"
    break
  fi
done

if [ -n "${FAILURE}" ]; then
  log "FAIL: ${FAILURE}"
  log "engine processes (%CPU is a lifetime average; near 0 = idle, not searching):"
  ps -o pid,stat,pcpu,etime,args --ppid "${FASTCHESS_PID}" >&2 || true
  UNFINISHED="$(comm -23 <(game_numbers Started) <(game_numbers Finished) | tr '\n' ' ')"
  log "unfinished games: ${UNFINISHED:-none}"
  # Collect the engines first: once fastchess exits they're reparented and
  # can no longer be found by parent PID.
  ENGINE_PIDS="$(pgrep -P "${FASTCHESS_PID}" || true)"
  kill -INT "${FASTCHESS_PID}" 2>/dev/null || true
  for _ in 1 2 3 4 5; do kill -0 "${FASTCHESS_PID}" 2>/dev/null || break; sleep 1; done
  # shellcheck disable=SC2086 # word-splitting the PID list is intended
  [ -n "${ENGINE_PIDS}" ] && kill -KILL ${ENGINE_PIDS} 2>/dev/null || true
  kill -KILL "${FASTCHESS_PID}" 2>/dev/null || true
  log "details: ${RUN_DIR}/fastchess.log"
  exit 1
fi

FASTCHESS_STATUS=0
wait "${FASTCHESS_PID}" || FASTCHESS_STATUS=$?
trap - EXIT
if [ "${FASTCHESS_STATUS}" -ne 0 ]; then
  log "FAIL: fastchess exited with status ${FASTCHESS_STATUS}, see ${RUN_DIR}/fastchess.log"
  exit 1
fi

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
