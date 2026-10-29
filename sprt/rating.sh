#!/usr/bin/env bash
# Rough absolute rating: a gauntlet of apefish against the anchors in
# sprt/anchors.tsv (by default Stockfish at fixed UCI_Elo levels), then a
# maximum-likelihood fit of apefish's Elo with the anchors' ratings fixed.
#
# The result is on Stockfish's UCI_Elo scale (anchored to CCRL 40/4), which is
# only a loose stand-in for FIDE. Expect it to be off by a couple of hundred.
#
# Games continue until the 95% interval is within ±--error Elo (and every
# anchor has played --min-games), or until --max-games per anchor.
#
# Usage: sprt/rating.sh [options]
#   --ref REF          build to rate: git ref or "worktree" (default)
#   --error N          stop once the 95% interval is within ±N Elo (default 100)
#   --min-games N      games per anchor before stopping early (default 10)
#   --max-games N      stop regardless after N games per anchor (default 500)
#   --tc TC            time control (default 60+0.6, which UCI_Elo is
#                      calibrated at; faster TCs skew the result)
#   --anchors FILE     anchor list (default sprt/anchors.tsv)
#   --concurrency N    games in parallel (default: cores - 2, or one per --cpus)
#   --cpus LIST        pin games to these CPUs, e.g. 2-11
#   --no-history       don't append the result to sprt/rating-history.tsv
#
# Each run writes its PGN, fastchess log and summary.txt to
# sprt/runs/<timestamp>_rating_*/.

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

REF="worktree"
ERROR=100
MIN_GAMES=10
MAX_GAMES=500
TC="60+0.6"
ANCHORS="${SPRT_DIR}/anchors.tsv"
CONCURRENCY=""
CPUS=""
HISTORY=1

while [ $# -gt 0 ]; do
  case "$1" in
    --ref) REF="$2"; shift 2 ;;
    --error) ERROR="$2"; shift 2 ;;
    --min-games) MIN_GAMES="$2"; shift 2 ;;
    --max-games) MAX_GAMES="$2"; shift 2 ;;
    --tc) TC="$2"; shift 2 ;;
    --anchors) ANCHORS="$2"; shift 2 ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --cpus) CPUS="$2"; shift 2 ;;
    --no-history) HISTORY=0; shift ;;
    -h|--help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

[ -f "${ANCHORS}" ] || die "anchor list not found: ${ANCHORS}"
ANCHORS="$(cd "$(dirname "${ANCHORS}")" && pwd)/$(basename "${ANCHORS}")"

AFFINITY=()
if [ -n "${CPUS}" ]; then
  AFFINITY=(-use-affinity "${CPUS}")
  : "${CONCURRENCY:=$(count_cpus "${CPUS}")}"
fi
: "${CONCURRENCY:=$(default_concurrency)}"

read -r BIN LABEL < <(build_engine "${REF}") || true
[ -n "${BIN:-}" ] || die "could not build ${REF}"

warn_if_bot_running
FASTCHESS_BIN="$(ensure_fastchess)"
BOOK="$(ensure_book)"

RUN_DIR="${RUNS_DIR}/$(date +%Y%m%d-%H%M%S)_rating_${LABEL}"
mkdir -p "${RUN_DIR}"
cp "${BIN}" "${RUN_DIR}/apefish"
cp "${ANCHORS}" "${RUN_DIR}/anchors.tsv"

# apefish goes first so it is the gauntlet's seed and plays every anchor.
ENGINES=(-engine cmd=./apefish args=--uci name=apefish)
NAMES=()
while IFS=$'\t' read -r name cmd elo options; do
  [ -z "${name}" ] || [[ "${name}" == \#* ]] && continue
  [ "${cmd}" = "stockfish" ] && cmd="$(ensure_stockfish)"
  ENGINE=(-engine "cmd=${cmd}" "name=${name}")
  if [ -n "${options}" ] && [ "${options}" != "-" ]; then
    for opt in ${options}; do ENGINE+=("option.${opt}"); done
  fi
  ENGINES+=("${ENGINE[@]}")
  NAMES+=("${name}(${elo})")
done < "${ANCHORS}"
[ "${#NAMES[@]}" -gt 0 ] || die "no anchors in ${ANCHORS}"

log "rating ${LABEL} vs ${NAMES[*]}"
log "until ±${ERROR} Elo (95%), ${MIN_GAMES}-${MAX_GAMES} games per anchor"
log "tc=${TC} concurrency=${CONCURRENCY}${CPUS:+ cpus=${CPUS}}"
log "output: ${RUN_DIR}"

# fastchess plays up to --max-games in the background while the rating is
# refitted from the PGN as games finish. A SIGINT stops it once the interval is
# narrow enough; it keeps every finished game.
cd "${RUN_DIR}"
"${FASTCHESS_BIN}" \
  "${ENGINES[@]}" \
  -tournament gauntlet -seeds 1 \
  -each proto=uci tc="${TC}" option.Hash=16 \
  -openings file="${BOOK}" format=pgn order=random \
  -repeat -rounds $(( (MAX_GAMES + 1) / 2 )) \
  -concurrency "${CONCURRENCY}" ${AFFINITY[@]+"${AFFINITY[@]}"} \
  -draw movenumber=40 movecount=8 score=10 \
  -resign movecount=3 score=1000 twosided=true \
  -ratinginterval 0 \
  -pgnout file=games.pgn \
  -recover > fastchess.log 2>&1 &
FASTCHESS_PID=$!
trap 'kill -INT "${FASTCHESS_PID}" 2>/dev/null' EXIT

fit() { python3 -I "${SPRT_DIR}/rating.py" games.pgn apefish anchors.tsv 2>/dev/null; }
LAST=""
STOPPED="max games reached"
while kill -0 "${FASTCHESS_PID}" 2>/dev/null; do
  sleep 10
  [ -s games.pgn ] || continue
  FIT="$(fit)" || continue
  # Last line is "rating <elo> <error>", or "rating >N" / "rating <N"; the
  # table rows above it have each anchor's game count in column 3.
  read -r _ RATING ERR <<<"$(tail -n 1 <<<"${FIT}")"
  PLAYED="$(awk 'NR > 1 && $1 != "rating"' <<<"${FIT}" | wc -l)"
  FEWEST="$(awk 'NR > 1 && $1 != "rating" { print $3 }' <<<"${FIT}" | sort -n | head -n 1)"
  STATUS="${RATING}${ERR:+ ± ${ERR}} after $(grep -c '^\[Result ' games.pgn || true) games"
  [ "${STATUS}" != "${LAST}" ] && log "${STATUS}"
  LAST="${STATUS}"
  if [ -n "${ERR}" ] && [ "${ERR}" -le "${ERROR}" ] && [ "${PLAYED}" -eq "${#NAMES[@]}" ] \
     && [ "${FEWEST:-0}" -ge "${MIN_GAMES}" ]; then
    STOPPED="interval within ±${ERROR}"
    log "${STOPPED}, stopping"
    kill -INT "${FASTCHESS_PID}" 2>/dev/null || true
    break
  fi
done
wait "${FASTCHESS_PID}" 2>/dev/null || true
trap - EXIT
cd - >/dev/null

[ -s "${RUN_DIR}/games.pgn" ] || die "no games played, see ${RUN_DIR}/fastchess.log"

FIT="$(python3 -I "${SPRT_DIR}/rating.py" "${RUN_DIR}/games.pgn" apefish "${RUN_DIR}/anchors.tsv")" \
  || die "rating fit failed"
RATING_LINE="$(tail -n 1 <<<"${FIT}")"
read -r _ RATING ERR <<<"${RATING_LINE}"
ABNORMAL="$(abnormal_terminations "${RUN_DIR}/games.pgn")"

{
  echo "engine: apefish ${LABEL} (${REF})"
  echo "tc:     ${TC}"
  echo "stop:   ${STOPPED}"
  echo
  sed '$d' <<<"${FIT}"
  echo
  echo "rating: ${RATING}${ERR:+ ± ${ERR}} (Stockfish UCI_Elo / CCRL 40/4 scale; a rough guide to FIDE at best)"
  if [ -n "${ABNORMAL}" ]; then
    echo "abnormal game endings (investigate before trusting this result):"
    echo "${ABNORMAL}"
  fi
} | tee "${RUN_DIR}/summary.txt"

if [ "${HISTORY}" -eq 1 ]; then
  HISTORY_FILE="${SPRT_DIR}/rating-history.tsv"
  [ -f "${HISTORY_FILE}" ] || printf 'date\tref\tsha\ttc\tgames\trating\terror\n' > "${HISTORY_FILE}"
  GAMES_PLAYED="$(grep -c '^\[Result ' "${RUN_DIR}/games.pgn" || true)"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -Iseconds)" "${REF}" "${LABEL}" "${TC}" \
    "${GAMES_PLAYED}" "${RATING}" "${ERR:-}" >> "${HISTORY_FILE}"
fi
