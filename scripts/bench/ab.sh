#!/usr/bin/env bash
# Same-machine A/B of the corpus benchmark: a base commit against the
# working tree, interleaved rounds, medians, and the change per metric.
# Numbers from different machines (or GitHub runners) vary by up to 2x;
# numbers from one machine in one sitting are what a PR should quote.
#
#   scripts/bench/ab.sh [--base origin/main] [--rounds 5] [--threshold 10]
#                       [--corpus small|real|small,real] [--loaded]
#
# Builds examples/corpus_bench at the base (in a temporary worktree) and
# at the working tree, fetches the corpora into bench-corpus/ when missing,
# runs them alternately, and prints a markdown table per corpus:
#
#   small  tokio 1.45.0 + Django 5.2 (about 6,300 files; the Benchmarks
#          workflow's corpus). The default.
#   real   Linux v6.11, LLVM 19.1.0, Rust 1.82.0, Kubernetes v1.31.0
#          (about 302,000 files, 3 GB; about 4 GB of disk once cloned).
#          A build takes minutes, so a round takes about 17 minutes on 4
#          cores: use it for changes to indexing, persistence or memory,
#          and say how many rounds the table had. Exits 1 when a metric is
# more than --threshold percent slower AND every round of this tree was
# slower than every round of the base, so a slowdown is never missed;
# a slowdown can still be the right trade, but then the PR says so.
# Base against itself (A/A) on a 4-core VM moved single metrics by up to
# 22% with 2 rounds; the overlap test is what keeps that noise out.
set -euo pipefail
cd "$(dirname "$0")/../.."

BASE=origin/main; ROUNDS=5; THRESHOLD=10; CORPORA=small; EXTRA=()
usage="usage: $0 [--base REF] [--rounds N] [--threshold PCT] [--corpus small|real|small,real] [--loaded]"
while [ $# -gt 0 ]; do
  case "$1" in
    --base) BASE="$2"; shift 2;;
    --rounds) ROUNDS="$2"; shift 2;;
    --threshold) THRESHOLD="$2"; shift 2;;
    --corpus) CORPORA="$2"; shift 2;;
    # Also time every query on the index as loaded from disk (`loaded/...`
    # rows): for changes to how a loaded index is held.
    --loaded) EXTRA+=(--loaded-queries); shift;;
    *) echo "$usage" >&2; exit 2;;
  esac
done
for c in ${CORPORA//,/ }; do
  case "$c" in small|real) ;; *) echo "$usage" >&2; exit 2;; esac
done

WORK=${AB_WORK:-target/ab}
mkdir -p "$WORK/bin" "$WORK/results"
rm -f "$WORK"/results/*

corpus() { # name repo ref
  [ -d "bench-corpus/$1" ] && return
  git clone -q --depth 1 --branch "$3" "https://github.com/$2.git" "bench-corpus/$1"
}
paths() { # corpus -> its folders
  case "$1" in
    small) echo bench-corpus/tokio bench-corpus/django;;
    real) echo bench-corpus/linux bench-corpus/llvm-project bench-corpus/rust bench-corpus/kubernetes;;
  esac
}
for c in ${CORPORA//,/ }; do
  case "$c" in
    small)
      corpus tokio tokio-rs/tokio tokio-1.45.0
      corpus django django/django 5.2;;
    real)
      corpus linux torvalds/linux v6.11
      corpus llvm-project llvm/llvm-project llvmorg-19.1.0
      corpus rust rust-lang/rust 1.82.0
      corpus kubernetes kubernetes/kubernetes v1.31.0;;
  esac
done

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
  for c in ${CORPORA//,/ }; do
    for side in "base-$base_sha" head; do
      # shellcheck disable=SC2046 # one argument per folder
      "$WORK/bin/$side" $(paths "$c") --label "ab-$c" \
        --iterations 20 --bencher "${EXTRA[@]}" > "$WORK/results/${side%%-*}-$c-$round.txt" 2>/dev/null
    done
  done
  echo "round $round of $ROUNDS done" >&2
done

status=0
for c in ${CORPORA//,/ }; do
[ "$CORPORA" = small ] || echo "### $c: $(paths "$c" | sed 's|bench-corpus/||g')"
python3 - "$WORK/results" "$base_sha" "$THRESHOLD" "$c" <<'PY' || status=1
import glob, re, statistics, sys
results, base_sha, threshold, corpus = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
data = {}
for side in ("base", "head"):
    for f in glob.glob(f"{results}/{side}-{corpus}-*.txt"):
        for m in re.finditer(rf"corpus_bench/ab-{corpus}/(\S+) \.\.\. bench: (\d+)", open(f).read()):
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
    # Slower only when the median moved past the threshold and the rounds
    # do not overlap; one noisy round on either side is not a slowdown.
    flag = " **slower**" if pct > threshold and min(sides["head"]) > max(sides["base"]) else ""
    if flag:
        slower.append(name)
    print(f"| {name} | {fmt(name, b)} | {fmt(name, h)} | {pct:+.0f}%{flag} |")
rounds = len(next(iter(data.values()))['head'])
print(f"\nMedian of {rounds} interleaved rounds on this machine; "
      f"**slower** = over {threshold:.0f}% and every round slower than every base round.")
if rounds < 3:
    print(f"With {rounds} round(s) every round can land on one side by chance, so a "
          "**slower** flag is only a lead: rerun with --rounds 3 or more before acting on it.")
if slower:
    print(f"\nMore than {threshold:.0f}% slower: {', '.join(slower)}. Fix it, or say in the PR "
          "(Performance section) and CHANGELOG what it buys.", file=sys.stderr)
    sys.exit(1)
PY
echo
done
exit $status
