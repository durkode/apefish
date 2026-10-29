#!/usr/bin/env bash
# GitHub reporting helpers for the CI workflows. Needs `gh` with GH_TOKEN set
# and GITHUB_REPOSITORY (both provided by GitHub Actions).
#
#   report.sh status  <sha> <context> <state> <description> [target_url]
#       Set a commit status. state: pending | success | failure | error.
#   report.sh comment <pr> <key> <body-file>
#       Create, or update in place, this workflow's PR comment identified by <key>.
#   report.sh record  <file> <row-file>
#       Append the last line of <row-file> to <file> on the sprt-results branch,
#       creating the branch / file (with <row-file>'s first line as header) if
#       needed. Must run inside a checkout whose `origin` remote can be pushed to.

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"

RESULTS_BRANCH="sprt-results"

cmd_status() {
  local sha="$1" context="$2" state="$3" description="$4" url="${5:-}"
  gh api --silent "repos/${GITHUB_REPOSITORY}/statuses/${sha}" \
    -f state="${state}" -f context="${context}" \
    -f description="${description:0:140}" \
    ${url:+-f target_url="${url}"}
}

cmd_comment() {
  local pr="$1" key="$2" body_file="$3"
  local marker="<!-- apefish-ci:${key} -->"
  local body
  body="$(printf '%s\n%s' "${marker}" "$(cat "${body_file}")")"
  local id
  id="$(gh api "repos/${GITHUB_REPOSITORY}/issues/${pr}/comments" --paginate \
        --jq ".[] | select(.body | startswith(\"${marker}\")) | .id" | head -n 1)"
  if [ -n "${id}" ]; then
    gh api --silent -X PATCH "repos/${GITHUB_REPOSITORY}/issues/comments/${id}" -f body="${body}"
  else
    gh api --silent "repos/${GITHUB_REPOSITORY}/issues/${pr}/comments" -f body="${body}"
  fi
}

cmd_record() {
  local file="$1" row_file="$2"
  local tree local_branch="${RESULTS_BRANCH}-ci"
  # Retry: another job may push to the results branch between fetch and push.
  for attempt in 1 2 3 4 5; do
    tree="$(mktemp -d "${TMPDIR:-/tmp}/apefish-results-XXXXXX")"
    git -C "${REPO_ROOT}" branch -D "${local_branch}" >/dev/null 2>&1 || true
    if git -C "${REPO_ROOT}" fetch --quiet origin \
         "+refs/heads/${RESULTS_BRANCH}:refs/remotes/origin/${RESULTS_BRANCH}" 2>/dev/null; then
      git -C "${REPO_ROOT}" worktree add --quiet -b "${local_branch}" "${tree}" \
        "origin/${RESULTS_BRANCH}"
    else
      log "creating ${RESULTS_BRANCH} branch"
      git -C "${REPO_ROOT}" worktree add --quiet --detach "${tree}"
      git -C "${tree}" switch --quiet --orphan "${local_branch}"
    fi

    [ -f "${tree}/${file}" ] || head -n 1 "${row_file}" > "${tree}/${file}"
    tail -n 1 "${row_file}" >> "${tree}/${file}"
    git -C "${tree}" add "${file}"
    git -C "${tree}" -c user.name="github-actions[bot]" \
      -c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
      commit --quiet -m "Record ${file}: $(tail -n 1 "${row_file}" | cut -f 2-3)"

    local pushed=0
    git -C "${tree}" push --quiet origin "HEAD:${RESULTS_BRANCH}" && pushed=1
    git -C "${REPO_ROOT}" worktree remove --force "${tree}"
    git -C "${REPO_ROOT}" branch -D "${local_branch}" >/dev/null 2>&1 || true
    [ "${pushed}" -eq 1 ] && return 0
    log "push to ${RESULTS_BRANCH} rejected (attempt ${attempt}), retrying"
    sleep $((attempt * 3))
  done
  die "could not record to ${RESULTS_BRANCH}"
}

case "${1:-}" in
  status) shift; cmd_status "$@" ;;
  comment) shift; cmd_comment "$@" ;;
  record) shift; cmd_record "$@" ;;
  *) die "usage: report.sh status|comment|record ..." ;;
esac
