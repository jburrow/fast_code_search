#!/usr/bin/env bash
# Same-machine A/B of the corpus benchmark: a base commit against the
# working tree, interleaved rounds, medians, and the change per metric.
# Numbers from different machines (or GitHub runners) vary by up to 2x;
# numbers from one machine in one sitting are what a PR should quote.
#
#   scripts/bench/ab.sh [--base origin/main] [--rounds 3] [--threshold 10]
#
# Builds examples/corpus_bench at the base (in a temporary worktree) and
# at the working tree, fetches tokio 1.45.0 and Django 5.2 into
# bench-corpus/ when missing (the Benchmarks workflow's corpus), runs them
# alternately, and prints a markdown table. Exits 1 when any metric is
# more than --threshold percent slower, so a slowdown is never missed;
# a slowdown can still be the right trade, but then the PR says so.
set -euo pipefail
cd "$(dirname "$0")/../.."

BASE=origin/main; ROUNDS=3; THRESHOLD=10
while [ $# -gt 0 ]; do
  case "$1" in
    --base) BASE="$2"; shift 2;;
    --rounds) ROUNDS="$2"; shift 2;;
    --threshold) THRESHOLD="$2"; shift 2;;
    *) echo "usage: $0 [--base REF] [--rounds N] [--threshold PCT]" >&2; exit 2;;
  esac
done

WORK=${AB_WORK:-target/ab}
mkdir -p "$WORK/bin" "$WORK/results"
rm -f "$WORK"/results/*

corpus() { # name repo ref
  [ -d "bench-corpus/$1" ] && return
  git clone -q --depth 1 --branch "$3" "https://github.com/$2.git" "bench-corpus/$1"
}
corpus tokio tokio-rs/tokio tokio-1.45.0
corpus django django/django 5.2

base_sha=$(git rev-parse --short "$BASE")
if [ ! -x "$WORK/bin/base-$base_sha" ]; then
  rm -rf "$WORK/base-tree"
  git worktree add -q --detach "$WORK/base-tree" "$base_sha"
  (cd "$WORK/base-tree" && CARGO_TARGET_DIR="$PWD/../target-base" cargo build -q --release --example corpus_bench)
  cp "$WORK/target-base/release/examples/corpus_bench" "$WORK/bin/base-$base_sha"
  git worktree remove --force "$WORK/base-tree"
fi
cargo build -q --release --example corpus_bench
cp target/release/examples/corpus_bench "$WORK/bin/head"

for round in $(seq 1 "$ROUNDS"); do
  for side in "base-$base_sha" head; do
    "$WORK/bin/$side" bench-corpus/tokio bench-corpus/django --label ab \
      --iterations 20 --bencher > "$WORK/results/${side%%-*}-$round.txt" 2>/dev/null
  done
  echo "round $round of $ROUNDS done" >&2
done

python3 - "$WORK/results" "$base_sha" "$THRESHOLD" <<'PY'
import glob, re, statistics, sys
results, base_sha, threshold = sys.argv[1], sys.argv[2], float(sys.argv[3])
data = {}
for side in ("base", "head"):
    for f in glob.glob(f"{results}/{side}-*.txt"):
        for m in re.finditer(r"corpus_bench/ab/(\S+) \.\.\. bench: (\d+)", open(f).read()):
            data.setdefault(m.group(1), {}).setdefault(side, []).append(int(m.group(2)))
def fmt(name, v):
    if name.endswith("_bytes"):
        return f"{v / 1e6:.0f} MB"
    return f"{v / 1e9:.2f} s" if v >= 1e9 else f"{v / 1e6:.1f} ms"
slower = []
print(f"| Metric | base {base_sha} | this tree | change |")
print("|---|---|---|---|")
for name, sides in data.items():
    if "base" not in sides or "head" not in sides:
        continue
    b, h = statistics.median(sides["base"]), statistics.median(sides["head"])
    if b < 1e5 and not name.endswith("_bytes"):  # under 0.1 ms: noise
        continue
    pct = (h - b) / b * 100 if b else 0.0
    flag = " **slower**" if pct > threshold else ""
    if flag:
        slower.append(name)
    print(f"| {name} | {fmt(name, b)} | {fmt(name, h)} | {pct:+.0f}%{flag} |")
print(f"\nMedian of {len(next(iter(data.values()))['head'])} interleaved rounds on this machine.")
if slower:
    print(f"\nMore than {threshold:.0f}% slower: {', '.join(slower)}. Fix it, or say in the PR "
          "(Performance section) and CHANGELOG what it buys.", file=sys.stderr)
    sys.exit(1)
PY
