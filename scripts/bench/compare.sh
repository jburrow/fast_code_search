#!/usr/bin/env bash
# Compare fast_code_search with scan-based tools on the same tree and the
# same queries, on this machine. Warm-cache wall time per query, median of
# N runs, for: ripgrep (`rg`), ugrep if present, and fcs against a server
# started on the tree (plus the server's own reported engine time).
#
#   scripts/bench/compare.sh <dir> [more dirs] [--runs 7] [--json out.json] [--markdown out.md]
#
# Needs: fast_code_search_server and fcs (target/release or PATH), rg;
# ugrep is optional. Ports 8093/50093 are used for the temporary server.
set -euo pipefail
cd "$(dirname "$0")/../.."

RUNS=7; JSON=""; MD=""; DIRS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --runs) RUNS="$2"; shift 2;;
    --json) JSON="$2"; shift 2;;
    --markdown) MD="$2"; shift 2;;
    *) DIRS+=("$1"); shift;;
  esac
done
[ ${#DIRS[@]} -gt 0 ] || { echo "usage: $0 <dir> [...] [--runs N] [--json F] [--markdown F]" >&2; exit 2; }

bin() { for d in target/release target/debug; do [ -x "$d/$1" ] && { echo "$PWD/$d/$1"; return; }; done; command -v "$1"; }
SERVER=$(bin fast_code_search_server); FCS=$(bin fcs)
RG=$(command -v rg || true); UG=$(command -v ugrep || true)
[ -n "$RG" ] || { echo "ripgrep (rg) is required" >&2; exit 2; }

# A FUSE mount (NTFS/exFAT on an external drive, network shares) makes every
# stat slow and penalises scan tools by an order of magnitude; the index is
# unaffected because it never touches the tree at query time. Refuse to
# produce a misleading table: copy the tree to a local filesystem first.
for d in "${DIRS[@]}"; do
  fs=$(findmnt -no FSTYPE -T "$d" 2>/dev/null || echo unknown)
  case "$fs" in fuse*|nfs*|cifs|smb*) echo "$d is on a $fs mount; copy it to a local filesystem (ext4/APFS/tmpfs) before comparing" >&2; exit 2;; esac
done

# Queries: (label, literal or regex, kind). The same string is given to every
# tool; fcs gets --regex for regex rows, rg/ugrep get -e (regex) for all.
# The scan tools are not given a match limit: ripgrep's -m is per file and
# ugrep's stops the whole search, so neither compares with "the best 50",
# and a scan tool cannot know the best 50 without reading everything anyway.
# Their output goes to /dev/null, as does fcs's.
QUERIES=(
  "common word|return|literal"
  "identifier|Config|literal"
  "rare identifier|xq9zv_nothing_here|literal"
  "two words|fn main|literal"
  "regex with literal|fn\\s+\\w+\\(|regex"
  "regex, no literal|[a-z]+_[a-z]+_[a-z]+\\(|regex"
  "case-insensitive|(?i)unwrap\\(|regex"
)

now_ms() { date +%s%N | awk '{printf "%.3f", $1/1e6}'; }
median() { sort -n | awk '{a[NR]=$1} END {if (NR%2) print a[(NR+1)/2]; else print (a[NR/2]+a[NR/2+1])/2}'; }

# Output goes to a real file, not /dev/null: ugrep notices a /dev/null
# stdout and stops at the first match as if -q were given, which timed as a
# 6 ms "scan" of forty megabytes. Every tool pays the same write cost.
SINK=$(mktemp)
time_cmd() { # runs a command N times, prints median ms
  local n=$1; shift
  for _ in $(seq 1 "$n"); do local t0; t0=$(now_ms); "$@" >"$SINK" 2>&1 || true; local t1; t1=$(now_ms); awk -v a="$t0" -v b="$t1" 'BEGIN{print b-a}'; done | median
}

# Start a server on the tree.
CFG=$(mktemp); INDEX=$(mktemp -u).fcsidx
{
  echo '[server]'; echo 'address = "127.0.0.1:50093"'; echo 'web_address = "127.0.0.1:8093"'
  echo '[indexer]'; printf 'paths = ['; for d in "${DIRS[@]}"; do printf '"%s",' "$(cd "$d" && pwd)"; done; echo ']'
  echo 'watch = false'; echo "index_path = \"$INDEX\""; echo 'save_after_build = false'
} > "$CFG"
"$SERVER" --config "$CFG" > /tmp/fcs-compare-server.log 2>&1 &
SPID=$!
trap 'kill $SPID 2>/dev/null; rm -f "$CFG" "$INDEX" "$SINK"' EXIT
# Wait for the build to finish, not merely for the server to be searchable
# (it serves results while imports are still being resolved).
READY=""
for _ in $(seq 1 900); do READY=$(curl -s http://127.0.0.1:8093/api/ready); echo "$READY" | grep -q '"status":"completed"' && break; sleep 1; done
echo "$READY" | grep -q '"status":"completed"' || { echo "server did not finish indexing in time: $READY" >&2; exit 1; }
FILES=$(echo "$READY" | python3 -c 'import json,sys; print(json.load(sys.stdin)["num_files"])')
TREE_FILES=$("$RG" --files "${DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ')
TREE_NAMES=$(for d in "${DIRS[@]}"; do basename "$d"; done | paste -sd+ -)
echo "server ready: $FILES files indexed, $TREE_FILES files in the tree" >&2

# Warm the page cache for the scan tools (the comparison is warm vs warm).
"$RG" -c 'return' "${DIRS[@]}" >/dev/null 2>&1 || true
[ -n "$UG" ] && "$UG" -c 'return' -r "${DIRS[@]}" >/dev/null 2>&1 || true

ROWS=()
for entry in "${QUERIES[@]}"; do
  IFS='|' read -r label q kind <<< "$entry"
  if [ "$kind" = literal ]; then rgargs=(-F -i); fcsargs=(); else rgargs=(); fcsargs=(-e); fi
  rg_ms=$(time_cmd "$RUNS" "$RG" "${rgargs[@]}" -n -e "$q" "${DIRS[@]}")
  ug_ms=""; [ -n "$UG" ] && ug_ms=$(time_cmd "$RUNS" "$UG" "${rgargs[@]}" -n -r -e "$q" "${DIRS[@]}")
  fcs_ms=$(time_cmd "$RUNS" "$FCS" --server http://127.0.0.1:8093 --no-offline -n 50 "${fcsargs[@]}" -- "$q")
  # engine-only time from the API (excludes process start and HTTP)
  enc=$(python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))' "$q")
  rx=""; [ "$kind" = regex ] && rx="&regex=true"
  eng_ms=$(for _ in $(seq 1 "$RUNS"); do curl -s "http://127.0.0.1:8093/api/search?q=$enc&max=50$rx" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("elapsed_ms", 0))'; done | median)
  ROWS+=("$label|$q|$kind|$rg_ms|$ug_ms|$fcs_ms|$eng_ms")
  echo "$label: rg ${rg_ms} ms, ugrep ${ug_ms:-n/a} ms, fcs ${fcs_ms} ms (engine ${eng_ms} ms)" >&2
done

MACHINE=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ *//' || sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)
CORES=$(nproc 2>/dev/null || sysctl -n hw.ncpu)
RGV=$("$RG" --version | head -1); UGV=""; [ -n "$UG" ] && UGV=$("$UG" --version | head -1)

{
  echo "### Scan tools vs fast_code_search (warm cache, median of $RUNS runs, wall time per query)"
  echo
  echo "Tree: $TREE_NAMES ($TREE_FILES files as ripgrep counts them, $FILES indexed). Machine: $MACHINE, $CORES logical CPUs. $RGV${UGV:+; $UGV}."
  echo "\`fcs\` includes process start and the HTTP round trip; \"engine\" is the server's own elapsed_ms."
  echo
  echo "| Query | Kind | ripgrep | ugrep | fcs (client) | fcs (engine) |"
  echo "|---|---|---|---|---|---|"
  for r in "${ROWS[@]}"; do IFS='|' read -r label q kind rg ug f e <<< "$r"; printf '| %s (`%s`) | %s | %.1f ms | %s | %.1f ms | %.2f ms |\n' "$label" "$q" "$kind" "$rg" "${ug:+$(printf '%.1f ms' "$ug")}" "$f" "$e"; done
} | tee "${MD:-/dev/stderr}" >/dev/null
[ -n "$MD" ] && cat "$MD" >&2

if [ -n "$JSON" ]; then
  python3 - "$JSON" "$MACHINE" "$CORES" "$FILES" "$TREE_FILES" "$TREE_NAMES" "$RUNS" "$RGV" "$UGV" "${ROWS[@]}" <<'PY'
import json, sys
out, machine, cores, files, tree_files, tree, runs, rgv, ugv, *rows = sys.argv[1:]
res = []
for r in rows:
    label, q, kind, rg, ug, f, e = r.split("|")
    res.append({"label": label, "query": q, "kind": kind, "ripgrep_ms": float(rg), "ugrep_ms": float(ug) if ug else None, "fcs_client_ms": float(f), "fcs_engine_ms": float(e)})
json.dump({"schema": 1, "machine": {"cpu_model": machine, "logical_cpus": int(cores)}, "tree": tree, "tree_files": int(tree_files), "files_indexed": int(files), "runs": int(runs), "tools": {"ripgrep": rgv, "ugrep": ugv or None}, "queries": res}, open(out, "w"), indent=1)
PY
fi
