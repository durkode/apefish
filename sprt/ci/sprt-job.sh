#!/usr/bin/env bash
# One SPRT job in .github/workflows/sprt.yml: run sprt.sh at <tc> and report the
# outcome as a commit status (sprt/<tc>), a PR comment and a row in history.tsv
# on the sprt-results branch. Results are advisory: the job only fails when the
# test itself could not run.
#
# Usage: sprt/ci/sprt-job.sh <stc|ltc|vstc>
# Env:   DEV, BASE   commits to test (dev against base)
#        MODE        gainer | nonreg
#        PR          pull request number for the comment (empty: no comment)
#        SPRT_CPUS   CPUs to pin games to, e.g. 2-11 (empty: no pinning)
#        GITHUB_*, GH_TOKEN as provided by GitHub Actions

set -euo pipefail
CI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${CI_DIR}/../lib.sh"

TC_NAME="${1:?usage: sprt-job.sh <tc>}"
CONTEXT="sprt/${TC_NAME}"
RUN_URL="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
report() { "${CI_DIR}/report.sh" "$@"; }

# Keep fastchess, the book and built binaries between jobs. The runner's own
# environment wins (set SPRT_CACHE_DIR there), else the runner tool cache.
if [ -z "${SPRT_CACHE_DIR:-}" ] && [ -n "${RUNNER_TOOL_CACHE:-}" ]; then
  export SPRT_CACHE_DIR="${RUNNER_TOOL_CACHE}/apefish-sprt"
fi

report status "${DEV}" "${CONTEXT}" pending "Running ${MODE} SPRT at ${TC_NAME}" "${RUN_URL}"

# sprt.sh writes its results as key=value lines; capture them, then forward
# them to this step's real outputs.
OUT="$(mktemp)"
STATUS=0
GITHUB_OUTPUT="${OUT}" "${SPRT_DIR}/sprt.sh" \
  --dev "${DEV}" --base "${BASE}" --mode "${MODE}" --tc "${TC_NAME}" \
  ${SPRT_CPUS:+--cpus "${SPRT_CPUS}"} --no-history || STATUS=$?
cat "${OUT}" >> "${GITHUB_OUTPUT}"
get() { sed -n "s/^$1=//p" "${OUT}" | tail -n 1; }

RESULT="$(get result)"
if [ -z "${RESULT}" ]; then
  report status "${DEV}" "${CONTEXT}" error "SPRT could not run (exit ${STATUS}), see logs" "${RUN_URL}"
  exit 1
fi

ELO="$(get elo)"; LLR="$(get llr)"; GAMES="$(get games)"
case "${RESULT}" in
  H1) STATE=success; DESC="H1 accepted: ${ELO} Elo, ${GAMES} games, LLR ${LLR}" ;;
  H0) STATE=failure; DESC="H0 accepted: ${ELO} Elo, ${GAMES} games, LLR ${LLR}" ;;
  skipped) STATE=success; DESC="Identical bench: cannot affect play, nothing to test" ;;
  *) STATE=error; DESC="Undecided after ${GAMES} games: ${ELO} Elo, LLR ${LLR}" ;;
esac
report status "${DEV}" "${CONTEXT}" "${STATE}" "${DESC}" "${RUN_URL}"

if [ -n "${PR:-}" ]; then
  BODY="$(mktemp)"
  {
    echo "### SPRT ${TC_NAME}: ${RESULT}"
    echo
    if [ "${RESULT}" = skipped ]; then
      echo "Bench signature is identical to base ($(get dev_bench)), so this change cannot affect play."
    else
      echo "| | |"
      echo "|---|---|"
      echo "| Test | ${MODE} at ${TC_NAME} |"
      echo "| Elo | ${ELO} |"
      echo "| LLR | ${LLR} (bounds ±2.94) |"
      echo "| Games | ${GAMES} (W $(get wins) / D $(get draws) / L $(get losses)) |"
      echo "| Dev | \`$(get dev_sha)\` bench $(get dev_bench) |"
      echo "| Base | \`$(get base_sha)\` bench $(get base_bench) |"
      if [ "$(get abnormal)" = true ]; then
        echo
        echo "**Warning:** some games ended abnormally (time loss, illegal move or crash). Check the PGN before trusting this result."
      fi
      if [ "${TC_NAME}" = stc ] && [ "${RESULT}" = H1 ]; then
        echo
        echo "Passed STC, so LTC runs next."
      fi
    fi
    echo
    echo "[Logs and games](${RUN_URL})"
  } > "${BODY}"
  report comment "${PR}" "sprt-${TC_NAME}" "${BODY}"
fi

if [ "${RESULT}" != skipped ]; then
  report record history.tsv "$(get run_dir)/history-row.tsv"
fi
