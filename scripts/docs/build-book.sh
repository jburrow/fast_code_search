#!/usr/bin/env bash
# Assemble and build the documentation book (docs/book) with mdBook.
#
# The canonical user documents stay where README, release archives and the
# code link to them (docs/CLI.md, docs/RUN-AT-STARTUP.md, docs/API.md, …);
# this script copies them into docs/book/src/imported/ with their relative
# links rewritten for the book, regenerates the reference pages, then runs
# `mdbook build`. Output: docs/book/book/.
#
#   scripts/docs/build-book.sh            # build
#   scripts/docs/build-book.sh --serve    # build and serve on :3000 with reload
set -euo pipefail
cd "$(dirname "$0")/../.."

python3 scripts/docs/import-docs.py
scripts/docs/generate-reference.sh

if [ "${1:-}" = "--serve" ]; then
  exec mdbook serve docs/book --open
fi
mdbook build docs/book
echo "book built: docs/book/book/index.html"
