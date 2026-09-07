#!/usr/bin/env bash
# Apply the repository presentation settings from
# docs/plans/2026-09-07-premium-repository-plan.md: description, homepage,
# topics, Discussions, and the merge/branch housekeeping a visitor notices.
#
# Needs a token with "Administration: write" (fine-grained) or "repo"
# (classic) on the repository, in $GITHUB_TOKEN, or an authenticated `gh`.
#
#   GITHUB_TOKEN=ghp_... scripts/github/apply-repo-settings.sh
#   gh auth login && scripts/github/apply-repo-settings.sh
#
# The social-preview image cannot be set through the API: upload
# docs/images/social-preview.png at
# https://github.com/jburrow/fast_code_search/settings (Social preview).
set -euo pipefail

REPO="${REPO:-jburrow/fast_code_search}"
API="https://api.github.com/repos/$REPO"
DESCRIPTION="In-memory trigram code search server with a CLI, web UI, REST and gRPC APIs: millisecond queries over multi-gigabyte trees"
HOMEPAGE="https://jburrow.github.io/fast_code_search/"
TOPICS='["code-search","rust","trigram","tree-sitter","grpc","developer-tools","search-engine","ripgrep","sourcegraph","zoekt","full-text-search","cli"]'

if [ -z "${GITHUB_TOKEN:-}" ] && command -v gh >/dev/null 2>&1; then
  GITHUB_TOKEN="$(gh auth token 2>/dev/null || true)"
fi
if [ -z "${GITHUB_TOKEN:-}" ]; then
  echo "error: set GITHUB_TOKEN or run 'gh auth login' first" >&2
  exit 2
fi

call() { # method path [json]
  local method="$1" path="$2" body="${3:-}"
  if [ -n "$body" ]; then
    curl -sS -f -X "$method" "$API$path" \
      -H "Authorization: Bearer $GITHUB_TOKEN" \
      -H "Accept: application/vnd.github+json" \
      -H "X-GitHub-Api-Version: 2022-11-28" \
      -d "$body"
  else
    curl -sS -f -X "$method" "$API$path" \
      -H "Authorization: Bearer $GITHUB_TOKEN" \
      -H "Accept: application/vnd.github+json" \
      -H "X-GitHub-Api-Version: 2022-11-28"
  fi
}

echo "== description, homepage, Discussions, merge housekeeping"
call PATCH "" "$(python3 - "$DESCRIPTION" "$HOMEPAGE" <<'PY'
import json, sys
print(json.dumps({
    "description": sys.argv[1],
    "homepage": sys.argv[2],
    "has_discussions": True,
    "has_wiki": False,               # docs live on the site, not a wiki
    "delete_branch_on_merge": True,
    "allow_update_branch": True,
}))
PY
)" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(" ", d["description"]); print("  homepage:", d["homepage"]); print("  discussions:", d["has_discussions"])'

echo "== topics"
call PUT "/topics" "{\"names\": $TOPICS}" | python3 -c 'import json,sys; print(" ", ", ".join(json.load(sys.stdin)["names"]))'

echo
echo "Done. Still manual: upload docs/images/social-preview.png under Settings > Social preview,"
echo "and (optional) Settings > Pages: confirm the source is the gh-pages branch."
