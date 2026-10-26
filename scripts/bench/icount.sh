#!/usr/bin/env bash
# Instruction counts of the corpus benchmark's operations (build, save,
# load, each query, the incremental updates), counted by callgrind.
#
#   scripts/bench/icount.sh [--json FILE] [--compare BASE.json]
#                           [--threshold PCT] [--accept NAMES]
#                           [--markdown FILE] [DIR...]
#
# Unlike wall time, an instruction count barely depends on the machine or
# what else it is doing, so counts from two GitHub runners on two days can
# be compared. The Benchmarks workflow runs this on every push to `main`
# and compares with the last published counts: it fails when an operation
# takes more than --threshold percent more instructions. Counts are
# published only from a passing run, so the base does not creep up: a
# failure stays red until the work is removed, or the increase is accepted
# as a decision (CLAUDE.md, "A slowdown is a decision") with a line
#
#   Accept-instructions: index_build, text/common     (or: all)
#
# in the commit message, which the workflow passes as --accept.
# Instructions are not time (cache misses, I/O, parallelism are not in
# them), so this catches added work, and the wall-clock A/B (ab.sh) stays
# the measure a PR quotes.
#
# DIR defaults to tokio 1.45.0 + Django 5.2 in bench-corpus/ (fetched when
# missing). Needs valgrind. Takes about 10 minutes on a GitHub runner, 17 on a
# 4 vCPU VM.
#
# Exit status: 0, or 1 when --compare finds an operation over the
# threshold, or 2 on a usage or run error.
set -euo pipefail
cd "$(dirname "$0")/../.."

JSON=""; COMPARE=""; THRESHOLD=3; ACCEPT=""; MARKDOWN=""; DIRS=()
usage="usage: $0 [--json FILE] [--compare BASE.json] [--threshold PCT] [--accept NAMES] [--markdown FILE] [DIR...]"
while [ $# -gt 0 ]; do
  case "$1" in
    --json) JSON="$2"; shift 2;;
    --compare) COMPARE="$2"; shift 2;;
    --threshold) THRESHOLD="$2"; shift 2;;
    --accept) ACCEPT="$2"; shift 2;;
    --markdown) MARKDOWN="$2"; shift 2;;
    -*) echo "$usage" >&2; exit 2;;
    *) DIRS+=("$1"); shift;;
  esac
done
command -v valgrind >/dev/null || { echo "valgrind is not installed" >&2; exit 2; }
if [ ${#DIRS[@]} -eq 0 ]; then
  [ -d bench-corpus/tokio ] || git clone -q --depth 1 --branch tokio-1.45.0 \
    https://github.com/tokio-rs/tokio.git bench-corpus/tokio
  [ -d bench-corpus/django ] || git clone -q --depth 1 --branch 5.2 \
    https://github.com/django/django.git bench-corpus/django
  DIRS=(bench-corpus/tokio bench-corpus/django)
fi

cargo build --release --example corpus_bench >&2
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# One rayon thread: with four, idle workers spin a varying number of
# times and rayon splits work by the thread count, so the same commit moved
# by up to 2% between runs (incremental/modify_file, 2026-10-25); with one
# it moved by 0.3% or less (symbol searches aside, below). Callgrind runs
# threads one at a time anyway, so this counts the same work, without the
# cost of spreading it. ICOUNT_THREADS overrides it, for experiments only:
# counts from different thread counts do not compare. The markers are never inlined (see examples/corpus_bench.rs); callgrind
# zeroes on entering icount_start and dumps on entering icount_end.
RAYON_NUM_THREADS=${ICOUNT_THREADS:-1} valgrind --tool=callgrind \
  --callgrind-out-file="$WORK/cg.out" \
  --zero-before='*icount_start*' --dump-before='*icount_end*' \
  --dump-line=no --compress-strings=no --compress-pos=no \
  target/release/examples/corpus_bench "${DIRS[@]}" --icount --iterations 1 \
  --label icount >"$WORK/names.txt" 2>"$WORK/valgrind.log" || {
    tail -30 "$WORK/valgrind.log" >&2; exit 2; }

python3 - "$WORK" "$JSON" "$COMPARE" "$THRESHOLD" "$MARKDOWN" "$(git rev-parse HEAD)" "$ACCEPT" <<'PY'
import glob, json, os, re, sys

def fail(message):
    print(message, file=sys.stderr)
    sys.exit(2)

work, out, compare, threshold, markdown, commit, accept = sys.argv[1:]
threshold = float(threshold)
# Operations whose count moves more than the threshold between runs of the
# same commit, with the threshold they get instead. Measured 2026-10-25 on
# tokio + Django, 8 runs of one commit: symbol/exact and symbol/prefix moved
# by up to 4.2% (one thread, median of five) and 7.7% (four threads, one
# run); every other operation by 0.5% or less with one thread. The cause is
# not found yet (it is not threads); until it is, these two catch only
# large increases.
NOISY = {"symbol/exact": 10.0, "symbol/prefix": 10.0}
accepted = {a.strip() for a in accept.replace(",", " ").split()}
names = [l.split(" ", 1)[1].strip() for l in open(f"{work}/names.txt") if l.startswith("icount ")]

def part(path):
    m = re.search(r"\.(\d+)$", path)
    return int(m.group(1)) if m else 0

dumps = sorted((p for p in glob.glob(f"{work}/cg.out*") if part(p) > 0), key=part)
if len(dumps) != len(names):
    fail(f"{len(names)} operations but {len(dumps)} callgrind dumps")
# An operation counted more than once (each query, each incremental
# update) is reported as the median of its counts.
runs = {}
for name, path in zip(names, dumps):
    for line in open(path, errors="replace"):
        if line.startswith(("totals:", "summary:")):
            runs.setdefault(name, []).append(int(line.split()[1]))
            break
    else:
        fail(f"no totals in {path}")
counts = {n: sorted(c)[len(c) // 2] for n, c in runs.items()}

result = {"schema": 1, "commit": commit, "instructions": counts}
if out:
    json.dump(result, open(out, "w"), indent=1)

lines, slower = [], []
base = json.load(open(compare))["instructions"] if compare and os.path.exists(compare) else None
if base is None:
    lines += ["| Operation | Instructions |", "|---|---|"]
    lines += [f"| {n} | {c:,} |" for n, c in counts.items()]
else:
    lines += ["| Operation | Base | This commit | Change |", "|---|---|---|---|"]
    for n, c in counts.items():
        b = base.get(n)
        if not b:
            lines.append(f"| {n} | (new) | {c:,} | |")
            continue
        change = 100.0 * (c - b) / b
        mark = ""
        if change > max(threshold, NOISY.get(n, 0.0)):
            if n in accepted or "all" in accepted:
                mark = " more, accepted"
            else:
                mark = " **more**"
                slower.append(n)
        lines.append(f"| {n} | {b:,} | {c:,} | {change:+.1f}%{mark} |")
    noisy = ", ".join(f"{n} {t:g}%" for n, t in NOISY.items())
    lines += ["", f"**more** = over {threshold:g}% more instructions than the base ({noisy}, which vary more between runs)."]
text = "\n".join(lines) + "\n"
print(text, end="")
if markdown:
    open(markdown, "w").write(text)
if slower:
    print(f"over the threshold: {', '.join(slower)}", file=sys.stderr)
    sys.exit(1)
PY
