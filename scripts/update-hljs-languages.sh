#!/usr/bin/env bash
# Rebuilds static/highlight-languages.min.js: the Highlight.js grammars the
# web UI maps file types to (hljsLangForPath in static/lib/keyword-helpers.js)
# that static/highlight.min.js does not bundle. The grammars are copied
# unmodified from the @highlightjs/cdn-assets package matching that bundle.
#
#   scripts/update-hljs-languages.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=11.11.1
LANGS="dart dockerfile elixir erlang haskell protobuf scala"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

(cd "$WORK" && npm pack --silent "@highlightjs/cdn-assets@$VERSION" >/dev/null && tar -xzf ./*.tgz)
OUT="$ROOT/static/highlight-languages.min.js"
{
  printf '/*!\n  Extra Highlight.js %s grammars not in highlight.min.js, unmodified from\n' "$VERSION"
  printf '  @highlightjs/cdn-assets@%s languages/*.min.js (BSD-3-Clause).\n' "$VERSION"
  printf '  Load after highlight.min.js. Regenerate with scripts/update-hljs-languages.sh.\n */\n'
  for lang in $LANGS; do cat "$WORK/package/languages/$lang.min.js"; echo; done
} > "$OUT"
echo "$OUT ($(wc -c < "$OUT") bytes)"
