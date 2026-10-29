#!/usr/bin/env bash
# SPRT test: play a dev build of apefish against a base build until fastchess
# can accept one of
#   H1: dev is at least <elo1> Elo stronger    -> PASS (exit 0)
#   H0: dev is at most  <elo0> Elo stronger    -> FAIL (exit 1)
# Anything else (stopped early, game cap hit, setup error) exits 2.
#
# Usage: sprt/sprt.sh [options]
#   --dev REF          build under test: a git ref, or "worktree" (default) for
#                      the current working tree including uncommitted changes
#   --base REF         reference build (default: main)
#   --mode MODE        gainer (default): bounds [0, 20]  - change should add Elo
#                      nonreg:           bounds [-20, 0] - change must not lose Elo
#   --elo0 N --elo1 N  override the mode's bounds (logistic Elo)
#   --tc TC            vstc (4+0.04), stc (8+0.08, default), ltc (40+0.4),
#                      or any fastchess tc string such as 10+0.1
#   --concurrency N    games in parallel (default: cores - 2)
#   --max-games N      stop undecided after N games (default 20000)
#   --force            run even when dev and base have the same bench signature
#   --no-history       don't append the result to sprt/history.tsv
#
# Each run writes its PGN and full fastchess log to sprt/runs/<timestamp>_*/.

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

DEV_REF="worktree"
BASE_REF="main"
MODE="gainer"
ELO0=""
ELO1=""
TC_NAME="stc"
CONCURRENCY="$(default_concurrency)"
MAX_GAMES=20000
FORCE=0
HISTORY=1

while [ $# -gt 0 ]; do
  case "$1" in
    --dev) DEV_REF="$2"; shift 2 ;;
    --base) BASE_REF="$2"; shift 2 ;;
    --mode) MODE="$2"; shift 2 ;;
    --elo0) ELO0="$2"; shift 2 ;;
    --elo1) ELO1="$2"; shift 2 ;;
    --tc) TC_NAME="$2"; shift 2 ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --max-games) MAX_GAMES="$2"; shift 2 ;;
    --force) FORCE=1; shift ;;
    --no-history) HISTORY=0; shift ;;
    -h|--help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

# Bounds are in logistic Elo, so they mean what a rating difference usually
# means. (fastchess's default nElo is ~2x larger at apefish's draw rates.)
case "${MODE}" in
  gainer) : "${ELO0:=0}" "${ELO1:=20}" ;;
  nonreg) : "${ELO0:=-20}" "${ELO1:=0}" ;;
  *) die "unknown mode: ${MODE} (gainer|nonreg)" ;;
esac

# Hash scales with time control; it must be set explicitly because the
# engine's default (512 MB) times 2 x concurrency would exhaust memory.
case "${TC_NAME}" in
  vstc) TC="4+0.04"; HASH=8 ;;
  stc) TC="8+0.08"; HASH=16 ;;
  ltc) TC="40+0.4"; HASH=64 ;;
  *) TC="${TC_NAME}"; HASH=16 ;;
esac

warn_if_bot_running
FASTCHESS_BIN="$(ensure_fastchess)"
BOOK="$(ensure_book)"

read -r DEV_BIN DEV_LABEL < <(build_engine "${DEV_REF}")
read -r BASE_BIN BASE_LABEL < <(build_engine "${BASE_REF}")

DEV_BENCH="$(bench_nodes "${DEV_BIN}")"
BASE_BENCH="$(bench_nodes "${BASE_BIN}")"
log "bench: dev ${DEV_LABEL} = ${DEV_BENCH}, base ${BASE_LABEL} = ${BASE_BENCH}"
if [ "${DEV_BENCH}" != "n/a" ] && [ "${DEV_BENCH}" = "${BASE_BENCH}" ] && [ "${FORCE}" -eq 0 ]; then
  log "identical bench signatures: the change cannot affect play, so there is nothing to test."
  log "(pass --force to run anyway)"
  exit 0
fi

# Binaries are copied into the run dir so a rebuild of bins/worktree
# mid-run can't swap an engine out from under fastchess.
RUN_DIR="${RUNS_DIR}/$(date +%Y%m%d-%H%M%S)_${DEV_LABEL}_vs_${BASE_LABEL}"
mkdir -p "${RUN_DIR}"
cp "${DEV_BIN}" "${RUN_DIR}/dev"
cp "${BASE_BIN}" "${RUN_DIR}/base"

log "SPRT ${MODE} [${ELO0}, ${ELO1}] tc=${TC} hash=${HASH}MB concurrency=${CONCURRENCY}"
log "dev  = ${DEV_LABEL} (${DEV_REF})"
log "base = ${BASE_LABEL} (${BASE_REF})"
log "output: ${RUN_DIR}"

# fastchess prints per-move warnings (e.g. PV runs past a repetition) and a
# line per game; the console only shows the periodic result blocks and any
# abnormal game ending. Everything goes to fastchess.log.
set +e
(cd "${RUN_DIR}" && "${FASTCHESS_BIN}" \
  -engine cmd=./dev args=--uci name=dev \
  -engine cmd=./base args=--uci name=base \
  -each proto=uci tc="${TC}" option.Hash="${HASH}" \
  -openings file="${BOOK}" format=pgn order=random \
  -repeat -rounds $(( (MAX_GAMES + 1) / 2 )) \
  -concurrency "${CONCURRENCY}" \
  -sprt elo0="${ELO0}" elo1="${ELO1}" alpha=0.05 beta=0.05 model=logistic \
  -draw movenumber=40 movecount=8 score=10 \
  -resign movecount=3 score=1000 twosided=true \
  -ratinginterval 100 \
  -pgnout file=games.pgn \
  -recover) 2>&1 \
  | tee "${RUN_DIR}/fastchess.log" \
  | sed -u 's/\x1b\[[0-9;]*m//g' \
  | grep --line-buffered -E '^(Results of|Elo:|LOS:|Games:|Ptnml|LLR:|SPRT|-{10,})|loses on time|stalls|disconnects|illegal move|H[01] was accepted'
set -e

CLEAN_LOG="$(sed 's/\x1b\[[0-9;]*m//g' "${RUN_DIR}/fastchess.log")"
last_line() { grep -E "$1" <<<"${CLEAN_LOG}" | tail -n 1 || true; }

GAMES_LINE="$(last_line '^Games:')"
ELO_LINE="$(last_line '^Elo:')"
LLR_LINE="$(last_line '^LLR:')"

if grep -q 'H1 was accepted' <<<"${CLEAN_LOG}"; then
  RESULT="H1"; STATUS=0
elif grep -q 'H0 was accepted' <<<"${CLEAN_LOG}"; then
  RESULT="H0"; STATUS=1
else
  RESULT="undecided"; STATUS=2
fi

# "Games: 20, Wins: 7, Losses: 8, Draws: 5, Points: ..."
num_after() { sed -nE "s/.*$1: (-?[0-9.]+).*/\\1/p" <<<"$2"; }
GAMES="$(num_after Games "${GAMES_LINE}")"
WINS="$(num_after Wins "${GAMES_LINE}")"
LOSSES="$(num_after Losses "${GAMES_LINE}")"
DRAWS="$(num_after Draws "${GAMES_LINE}")"
# "Elo: -17.39 +/- 76.79, nElo: ..."
ELO="$(sed -nE 's/^Elo: (-?[0-9.]+) \+\/- ([0-9.]+).*/\1 ± \2/p' <<<"${ELO_LINE}")"
LLR="$(sed -nE 's/^LLR: (-?[0-9.]+).*/\1/p' <<<"${LLR_LINE}")"

ABNORMAL="$(abnormal_terminations "${RUN_DIR}/games.pgn")"

{
  echo "dev:    ${DEV_LABEL} (${DEV_REF}) bench ${DEV_BENCH}"
  echo "base:   ${BASE_LABEL} (${BASE_REF}) bench ${BASE_BENCH}"
  echo "test:   ${MODE} [${ELO0}, ${ELO1}] tc=${TC} hash=${HASH}MB"
  echo "result: ${RESULT}"
  echo "games:  ${GAMES:-0} (W ${WINS:-0} / D ${DRAWS:-0} / L ${LOSSES:-0})"
  echo "elo:    ${ELO:-n/a}"
  echo "llr:    ${LLR:-n/a}"
  if [ -n "${ABNORMAL}" ]; then
    echo "abnormal game endings (investigate before trusting this result):"
    echo "${ABNORMAL}"
  fi
} | tee "${RUN_DIR}/summary.txt"

if [ "${HISTORY}" -eq 1 ]; then
  HISTORY_FILE="${SPRT_DIR}/history.tsv"
  if [ ! -f "${HISTORY_FILE}" ]; then
    printf 'date\tdev\tbase\tmode\telo0\telo1\ttc\tgames\twins\tdraws\tlosses\telo\tllr\tresult\tdev_bench\tbase_bench\n' \
      > "${HISTORY_FILE}"
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$(date -Iseconds)" "${DEV_LABEL}" "${BASE_LABEL}" "${MODE}" "${ELO0}" "${ELO1}" "${TC}" \
    "${GAMES:-0}" "${WINS:-0}" "${DRAWS:-0}" "${LOSSES:-0}" "${ELO:-n/a}" "${LLR:-n/a}" \
    "${RESULT}" "${DEV_BENCH}" "${BASE_BENCH}" >> "${HISTORY_FILE}"
fi

case "${RESULT}" in
  H1) log "PASS: H1 accepted (dev at +${ELO1} Elo favoured over +${ELO0})" ;;
  H0) log "FAIL: H0 accepted (dev at +${ELO0} Elo favoured over +${ELO1})" ;;
  *) log "UNDECIDED: test stopped before reaching a decision" ;;
esac
exit "${STATUS}"
