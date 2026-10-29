# shellcheck shell=bash
# Shared helpers for sprt/sprt.sh, sprt/smoke.sh and sprt/rating.sh. Source, don't execute.
#
# Everything is non-interactive and configured through flags/env vars so the
# same scripts can run unattended in CI.

SPRT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SPRT_DIR}/.." && pwd)"

# SPRT_CACHE_DIR moves the tool/book/binary caches (and the shared cargo target
# dir) out of the checkout, e.g. to a persistent directory on a CI runner.
# SPRT_RUNS_DIR does the same for per-run output.
CACHE_DIR="${SPRT_CACHE_DIR:-${SPRT_DIR}}"
TOOLS_DIR="${CACHE_DIR}/.tools"
BINS_DIR="${CACHE_DIR}/bins"
BOOKS_DIR="${CACHE_DIR}/books"
TARGET_DIR="${CACHE_DIR}/.target"
# shellcheck disable=SC2034 # used by the scripts that source this file
RUNS_DIR="${SPRT_RUNS_DIR:-${SPRT_DIR}/runs}"

# fastchess release to build from source when it isn't on PATH and nix isn't
# available. Matches the version nixpkgs currently ships.
FASTCHESS_TAG="${FASTCHESS_TAG:-v1.8.0-alpha}"

BOOK_NAME="8moves_v3.pgn"
BOOK_URL="https://github.com/official-stockfish/books/raw/master/${BOOK_NAME}.zip"

log() { echo ">> $*" >&2; }
die() { echo "error: $*" >&2; exit 2; }

# Print the path of a usable fastchess binary, fetching one if needed.
# Order: $FASTCHESS, PATH, nixpkgs, build ${FASTCHESS_TAG} from source.
ensure_fastchess() {
  if [ -n "${FASTCHESS:-}" ]; then
    [ -x "${FASTCHESS}" ] || die "FASTCHESS=${FASTCHESS} is not executable"
    echo "${FASTCHESS}"; return
  fi
  if command -v fastchess >/dev/null 2>&1; then
    command -v fastchess; return
  fi

  local nix_link="${TOOLS_DIR}/fastchess-nix"
  local src_dir="${TOOLS_DIR}/fastchess-src"
  if [ -x "${nix_link}/bin/fastchess" ]; then
    echo "${nix_link}/bin/fastchess"; return
  fi
  if [ -x "${src_dir}/fastchess" ]; then
    echo "${src_dir}/fastchess"; return
  fi

  mkdir -p "${TOOLS_DIR}"
  if command -v nix >/dev/null 2>&1; then
    log "fetching fastchess from nixpkgs"
    nix build nixpkgs#fastchess --out-link "${nix_link}" >&2 \
      || die "nix build nixpkgs#fastchess failed"
    echo "${nix_link}/bin/fastchess"; return
  fi

  command -v make >/dev/null 2>&1 && command -v g++ >/dev/null 2>&1 \
    || die "fastchess not found; install it, set FASTCHESS, or provide make + g++ to build it"
  log "building fastchess ${FASTCHESS_TAG} from source"
  rm -rf "${src_dir}"
  git clone --quiet --depth 1 --branch "${FASTCHESS_TAG}" \
    https://github.com/Disservin/fastchess.git "${src_dir}" >&2
  make -C "${src_dir}" -j"$(nproc)" >&2 || die "fastchess build failed"
  echo "${src_dir}/fastchess"
}

# Print the path of a Stockfish binary for rating anchors.
# Order: $STOCKFISH, PATH, nixpkgs.
ensure_stockfish() {
  if [ -n "${STOCKFISH:-}" ]; then
    [ -x "${STOCKFISH}" ] || die "STOCKFISH=${STOCKFISH} is not executable"
    echo "${STOCKFISH}"; return
  fi
  if command -v stockfish >/dev/null 2>&1; then
    command -v stockfish; return
  fi

  local nix_link="${TOOLS_DIR}/stockfish-nix"
  if [ ! -x "${nix_link}/bin/stockfish" ]; then
    command -v nix >/dev/null 2>&1 \
      || die "stockfish not found; install it or set STOCKFISH"
    mkdir -p "${TOOLS_DIR}"
    log "fetching stockfish from nixpkgs"
    nix build nixpkgs#stockfish --out-link "${nix_link}" >&2 \
      || die "nix build nixpkgs#stockfish failed"
  fi
  echo "${nix_link}/bin/stockfish"
}

# Print the path of the opening book, downloading it on first use.
ensure_book() {
  local book="${BOOKS_DIR}/${BOOK_NAME}"
  if [ ! -f "${book}" ]; then
    mkdir -p "${BOOKS_DIR}"
    log "downloading opening book ${BOOK_NAME}"
    curl -fsSL -o "${book}.zip" "${BOOK_URL}" || die "book download failed"
    if command -v unzip >/dev/null 2>&1; then
      unzip -oq "${book}.zip" -d "${BOOKS_DIR}"
    else
      python3 -I -m zipfile -e "${book}.zip" "${BOOKS_DIR}"
    fi
    rm -f "${book}.zip"
  fi
  echo "${book}"
}

# True when the working tree has uncommitted changes to tracked files.
worktree_dirty() {
  ! git -C "${REPO_ROOT}" diff --quiet HEAD --
}

# Build the engine for <ref> and print "<binary path> <label>".
#
# <ref> is any git revision, or "worktree" for the current working tree
# including uncommitted changes. Commit builds are cached in bins/ by SHA;
# worktree builds are rebuilt every time (cargo makes that incremental).
build_engine() {
  local ref="$1"
  mkdir -p "${BINS_DIR}"

  if [ "${ref}" = "worktree" ]; then
    local sha label
    sha="$(git -C "${REPO_ROOT}" rev-parse --short=10 HEAD)"
    label="${sha}"
    worktree_dirty && label="${sha}+dirty"
    log "building working tree (${label})"
    (cd "${REPO_ROOT}" && cargo build --release --quiet -p apefish-cli) >&2 \
      || die "cargo build failed for working tree"
    cp "${REPO_ROOT}/target/release/apefish" "${BINS_DIR}/worktree"
    echo "${BINS_DIR}/worktree ${label}"
    return
  fi

  local sha
  sha="$(git -C "${REPO_ROOT}" rev-parse --verify --short=10 "${ref}^{commit}" 2>/dev/null)" \
    || die "unknown git ref: ${ref}"
  local bin="${BINS_DIR}/${sha}"
  if [ ! -x "${bin}" ]; then
    log "building ${ref} (${sha})"
    local tree
    tree="$(mktemp -d "${TMPDIR:-/tmp}/apefish-sprt-XXXXXX")"
    git -C "${REPO_ROOT}" worktree add --quiet --detach "${tree}" "${sha}" >&2
    # Shared target dir so dependency builds are reused across commits.
    local status=0
    (cd "${tree}" && CARGO_TARGET_DIR="${TARGET_DIR}" \
      cargo build --release --quiet -p apefish-cli) >&2 || status=$?
    git -C "${REPO_ROOT}" worktree remove --force "${tree}"
    [ "${status}" -eq 0 ] || die "cargo build failed for ${ref} (${sha})"
    cp "${TARGET_DIR}/release/apefish" "${bin}"
  fi
  echo "${bin} ${sha}"
}

# Print the bench node count for <binary>, or "n/a" if the build predates
# `apefish bench`.
bench_nodes() {
  local out
  out="$("$1" bench 2>/dev/null | tail -n 1)"
  if [[ "${out}" =~ ^([0-9]+)\ nodes ]]; then
    echo "${BASH_REMATCH[1]}"
  else
    echo "n/a"
  fi
}

# Warn when the Lichess bot is running on this machine: it competes for CPU
# and skews both engines' clocks.
warn_if_bot_running() {
  if command -v systemctl >/dev/null 2>&1 \
     && systemctl --user is-active --quiet apefish-bot 2>/dev/null; then
    log "WARNING: apefish-bot.service is running and will compete for CPU."
    log "         Stop it for clean results: systemctl --user stop apefish-bot"
  fi
}

# Number of CPUs in a list like "2-11" or "0,2,4-7".
count_cpus() {
  local total=0 part
  IFS=',' read -ra parts <<<"$1"
  for part in "${parts[@]}"; do
    if [[ "${part}" =~ ^([0-9]+)-([0-9]+)$ ]]; then
      total=$((total + BASH_REMATCH[2] - BASH_REMATCH[1] + 1))
    elif [[ "${part}" =~ ^[0-9]+$ ]]; then
      total=$((total + 1))
    else
      die "invalid CPU list: $1"
    fi
  done
  echo "${total}"
}

# Append key=value lines to $GITHUB_OUTPUT when running under GitHub Actions.
gh_output() {
  [ -n "${GITHUB_OUTPUT:-}" ] || return 0
  local kv
  for kv in "$@"; do echo "${kv}" >> "${GITHUB_OUTPUT}"; done
}

default_concurrency() {
  local n
  n="$(nproc)"
  if [ "${n}" -gt 3 ]; then echo $((n - 2)); else echo 1; fi
}

# Print one "<count> <termination>" line per abnormal game ending in <pgn>
# (time forfeits, illegal moves, crashes/stalls). Empty when every game ended
# normally or by adjudication.
abnormal_terminations() {
  grep -h '^\[Termination ' "$1" 2>/dev/null \
    | sed -E 's/^\[Termination "(.*)"\]$/\1/' \
    | grep -viE '^(normal|adjudication)$' \
    | sort | uniq -c || true
}
