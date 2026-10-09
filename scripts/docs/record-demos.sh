#!/usr/bin/env bash
# Re-records the feature demos in docs/images/demos/.
#
# Indexes a clean export of this repository's HEAD (so the demos only ever
# show this project's own code), starts a server on a spare port, drives the
# web UI with Playwright (scripts/docs/record-demos.mjs) and turns each
# recording into a GIF with ffmpeg.
#
#   scripts/docs/record-demos.sh            # all demos
#   scripts/docs/record-demos.sh graph      # one demo (or `stills` for web-ui.png)
#
# Needs: cargo (and protoc), node with the `playwright` package (set
# NODE_PATH for a global install), a Chromium Playwright can launch, ffmpeg.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ONLY="${1:-}"
PORT="${FCS_DEMO_PORT:-18080}"
OUT="$ROOT/docs/images/demos"
WORK="$(mktemp -d)"
trap 'kill "${SERVER_PID:-}" 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --release --bin fast_code_search_server --bin fcs --manifest-path "$ROOT/Cargo.toml"

# The folder is named after the project so paths read the same as a real checkout.
mkdir -p "$WORK/fast_code_search" "$WORK/index"
git -C "$ROOT" archive HEAD | tar -x -C "$WORK/fast_code_search"

cat > "$WORK/demo.toml" <<EOF
[server]
web_address = "127.0.0.1:$PORT"
enable_grpc = false
[indexer]
paths = ["$WORK/fast_code_search"]
show_root_name = false
index_path = "$WORK/index/index.bin"
EOF

"$ROOT/target/release/fast_code_search_server" --config "$WORK/demo.toml" > "$WORK/server.log" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:$PORT/api/status" | grep -q '"status":"completed"'; then break; fi
  sleep 1
done
if [ -f "$WORK/raw/web-ui.png" ]; then
  cp "$WORK/raw/web-ui.png" "$ROOT/docs/images/web-ui.png"
  echo "$ROOT/docs/images/web-ui.png"
fi
sleep 2

FCS_URL="http://127.0.0.1:$PORT" node "$ROOT/scripts/docs/record-demos.mjs" "$WORK/raw" $ONLY

mkdir -p "$OUT"
shopt -s nullglob
for webm in "$WORK"/raw/*.webm; do
  name="$(basename "$webm" .webm)"
  # Two passes: a palette built from the clip, then the clip mapped onto it.
  start="$(cat "$WORK/raw/$name.start" 2>/dev/null || echo 0)"
  ffmpeg -loglevel error -y -ss "$start" -i "$webm" \
    -vf "fps=8,scale=880:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    "$OUT/$name.gif"
  echo "$OUT/$name.gif ($(du -k "$OUT/$name.gif" | cut -f1) KB)"
done
if [ -f "$WORK/raw/web-ui.png" ]; then
  cp "$WORK/raw/web-ui.png" "$ROOT/docs/images/web-ui.png"
  echo "$ROOT/docs/images/web-ui.png"
fi
