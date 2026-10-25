#!/usr/bin/env python3
"""Render the benchmark numbers the docs quote from one Benchmarks run.

    scripts/docs/bench-tables.py            # rewrite the marked blocks
    scripts/docs/bench-tables.py --check    # fail if any block is stale (CI)
    scripts/docs/bench-tables.py --from-pages COMMIT
                                            # pin the run for COMMIT first

The README, the book's latest-results and comparison pages and the site
quote benchmark numbers. They used to be copied by hand, and from
September to October 2026 they showed a common-word search at about 1 ms
after v0.17.0 had made it about 25 ms. Now every quoted number sits
between markers

    <!-- bench:NAME -->  ...  <!-- /bench:NAME -->

and is rendered from docs/bench/published.json: the results file one
Benchmarks run published (dev/bench/results/<date>-<commit>.json on the
site), which names its run, commit and machine. Refreshing the numbers is
`--from-pages <commit>` and a commit; the Lint job runs `--check`.
"""

import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PUBLISHED = ROOT / "docs/bench/published.json"
FILES = [
    "README.md",
    "docs/book/src/benchmarks/latest.md",
    "docs/book/src/benchmarks/comparison.md",
    "docs/site/index.html",
]

QUERY_NAMES = {
    "text/common": "text, common word",
    "text/identifier": "text, identifier",
    "text/no_match": "text, no match",
    "text/short": "text, short (two characters)",
    "text/full_rank": "text, full ranking",
    "text/filtered": "text, path-filtered",
    "regex/literal": "regex with literal",
    "regex/case_insensitive": "regex, case-insensitive",
    "regex/no_literal": "regex, no literal (full scan)",
    "symbol/exact": "symbol, exact",
    "symbol/prefix": "symbol, prefix",
    "symbol/references": "references",
    "incremental/modify_file": "incremental: modify one file",
}


def sig3(x):
    """Three significant figures, no exponent: 24.6, 7.11, 0.571."""
    if x >= 100:
        return f"{x:,.0f}"
    if x >= 10:
        return f"{x:.1f}"
    if x >= 1:
        return f"{x:.2f}"
    return f"{x:.3g}"


def dur(ns):
    if ns < 1e3:
        return f"{ns:.0f} ns"
    if ns < 1e6:
        return f"{sig3(ns / 1e3)} µs"
    if ns < 1e9:
        return f"{sig3(ns / 1e6)} ms"
    return f"{sig3(ns / 1e9)} s"


def mb(n):
    return f"{n / 1e6:.1f} MB" if n < 1e8 else f"{n / 1e6:,.0f} MB"


def thousands(n):
    return f"{round(n / 1e3):,}k" if n >= 1e4 else f"{n:,}"


def short_cpu(model):
    # "AMD EPYC 7763 64-Core Processor" -> "AMD EPYC 7763"
    return re.sub(r"\s+\d+-Core Processor$", "", model).strip()


def render(d):
    c = d["corpus"]
    b, m, tree = c["build"], c["machine"], c["corpus"]
    q = {x["name"]: x for x in c["queries"]}
    run_id = d["run_url"].rstrip("/").rsplit("/", 1)[-1]
    commit = d["commit"][:7]
    date = d["generated_at"][:10]
    version = m.get("fast_code_search", "?")
    run = f"[run {run_id}]({d['run_url']})"
    where = (f"GitHub-hosted `ubuntu-latest` ({short_cpu(m['cpu_model'])}, "
             f"{m['logical_cpus']} vCPU, {m['os'].rsplit('.', 1)[0]})")
    corpus_line = (f"{tree['files']:,} files, {mb(tree['bytes'])} of text, "
                   f"{thousands(tree['symbol_references'])} symbol references")
    p50 = lambda n: dur(q[n]["p50_ns"])
    p95 = lambda n: dur(q[n]["p95_ns"])
    blocks = {}

    blocks["readme-run"] = (
        f"The numbers below are from {run}\n"
        f"on commit `{commit}` ({date}, version {version}), {where}:\n"
        f"{corpus_line}.\n")
    blocks["readme-table"] = "\n".join([
        "| Measure | Value |",
        "|---------|-------|",
        f"| Full build (read, trigrams, symbols, imports, merge) | {dur(b['build_ns'])} ({b['files_per_second']:,.0f} files/s) |",
        f"| Resident memory after build | {mb(b['rss_after_bytes'])} |",
        f"| Index save / reconciling load | {dur(b['save_ns'])} / {dur(b['load_ns'])} ({mb(b['index_bytes'])} on disk) |",
        f"| Text search, common word, p50 / p95 | {p50('text/common')} / {p95('text/common')} |",
        f"| Text search, identifier, p50 | {p50('text/identifier')} |",
        f"| Regex with literal / case-insensitive / no literal, p50 | {p50('regex/literal')} / {p50('regex/case_insensitive')} / {p50('regex/no_literal')} |",
        f"| Symbol search, p50 | {p50('symbol/exact')} |",
        f"| Reference search, p50 | {p50('symbol/references')} |",
        f"| Incremental update of one file, p50 / p95 | {p50('incremental/modify_file')} / {p95('incremental/modify_file')} |",
    ]) + "\n"

    blocks["latest-run"] = (
        f"From {run}\non commit `{commit}` ({date}, version {version}), {where}.\n")
    rows = [
        "| Build | Value |",
        "|---|---|",
        f"| Discovery | {dur(b['discovery_ns'])} |",
        f"| Build (read, trigrams, symbols, imports, merge) | {dur(b['build_ns'])}, {b['files_per_second']:,.0f} files/s, {tree['bytes'] / 1e6 / (b['build_ns'] / 1e9):.1f} MB/s |",
        f"| Resident memory after build | {mb(b['rss_after_bytes'])} (process before build: {mb(b['rss_before_bytes'])}) |",
        f"| Index save | {dur(b['save_ns'])} ({mb(b['index_bytes'])} on disk) |",
        f"| Index load, reconciling | {dur(b['load_ns'])} |",
        "",
        "| Query | p50 | p95 |",
        "|---|---|---|",
    ]
    rows += [f"| {label} | {p50(n)} | {p95(n)} |" for n, label in QUERY_NAMES.items() if n in q]
    rq = d.get("ranking_quality")
    blocks["latest-corpus"] = (
        f"{corpus_line.replace(' symbol references', '')} symbol references, "
        f"{thousands(tree['trigrams'])} distinct trigrams, "
        f"{tree['dependency_edges']:,} dependency edges.\n\n"
        + "\n".join(rows) + "\n"
        + (f"\nRanking quality in the same run: precision@1 {rq['precision_at_1']:.2f}, "
           f"precision@5 {rq['precision_at_5']:.2f}.\n" if rq else ""))

    cmp = d["comparison"]
    tools = cmp["tools"]
    rg = tools["ripgrep"]
    ug = (tools.get("ugrep") or "").split(" x86_64")[0]
    blocks["comparison-run"] = (
        f"From {run}\non commit `{commit}` ({date}, version {version}): GitHub-hosted `ubuntu-latest`,\n"
        f"{short_cpu(cmp['machine']['cpu_model'])}, {cmp['machine']['logical_cpus']} vCPU, {rg}, {ug}.\n")
    f1 = lambda v: "—" if v is None else f"{v:.1f} ms"
    rows = [
        f"**tokio 1.45.0 + Django 5.2**, {cmp['tree_files']:,} files.",
        "",
        "| Query | ripgrep | ugrep | fcs (client) | fcs (engine) |",
        "|---|---|---|---|---|",
    ]
    for x in cmp["queries"]:
        rows.append(f"| {x['label']} `{x['query']}` | {f1(x.get('ripgrep_ms'))} | {f1(x.get('ugrep_ms'))} | "
                    f"{f1(x.get('fcs_client_ms'))} | {x['fcs_engine_ms']:.2f} ms |")
    blocks["comparison-table"] = "\n".join(rows) + "\n"

    stat = lambda v, label: (
        '      <div class="bench-stat">\n'
        f'        <div class="bench-stat-value">{v}</div>\n'
        f'        <div class="bench-stat-label">{label}</div>\n'
        '      </div>\n')
    blocks["site-stats"] = (
        stat(p50("text/identifier"), f"p50 identifier search, ranked, tokio + Django ({tree['files']:,} files)")
        + stat(f"{b['files_per_second']:,.0f} files/s", "Index build, symbols included")
        + stat(mb(b["rss_after_bytes"]), f"Resident for {mb(tree['bytes'])} of source"))
    blocks["site-run"] = (
        f'    <p class="section-subtitle">From <a href="{d["run_url"]}">benchmark run {run_id}</a> '
        f"(commit {commit}, {date}, GitHub-hosted ubuntu-latest). A common word takes "
        f"{p50('text/common')} because fcs ranks up to 32,768 matches before returning page one; the "
        '<a href="https://github.com/jburrow/fast_code_search/blob/main/README.md#benchmarks">README</a> '
        "carries the full table.</p>\n")
    return blocks


MARK = re.compile(r"(<!-- bench:([a-z-]+) -->\n)(.*?)(<!-- /bench:\2 -->)", re.S)


def apply(text, blocks, path, used):
    def sub(m):
        name = m.group(2)
        if name not in blocks:
            sys.exit(f"{path}: unknown block bench:{name}")
        used.add(name)
        return m.group(1) + blocks[name] + m.group(4)
    return MARK.sub(sub, text)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="fail if a block is stale")
    ap.add_argument("--from-pages", metavar="COMMIT",
                    help="copy that commit's results from the gh-pages branch to docs/bench/published.json")
    args = ap.parse_args()
    if args.from_pages:
        subprocess.run(["git", "fetch", "-q", "origin", "gh-pages"], cwd=ROOT, check=True)
        names = subprocess.run(["git", "ls-tree", "--name-only", "FETCH_HEAD", "dev/bench/results/"],
                               cwd=ROOT, check=True, capture_output=True, text=True).stdout.split()
        hit = [n for n in names if n.endswith(f"-{args.from_pages[:7]}.json")]
        if len(hit) != 1:
            sys.exit(f"no single published result for {args.from_pages[:7]} on gh-pages: {hit}")
        raw = subprocess.run(["git", "show", f"FETCH_HEAD:{hit[0]}"], cwd=ROOT, check=True,
                             capture_output=True, text=True).stdout
        PUBLISHED.parent.mkdir(parents=True, exist_ok=True)
        PUBLISHED.write_text(json.dumps(json.loads(raw), indent=1) + "\n", encoding="utf-8")
        print(f"pinned {hit[0]}")
    blocks = render(json.loads(PUBLISHED.read_text(encoding="utf-8")))
    used, stale = set(), []
    for f in FILES:
        path = ROOT / f
        old = path.read_text(encoding="utf-8")
        new = apply(old, blocks, f, used)
        if new != old:
            stale.append(f)
            if not args.check:
                path.write_text(new, encoding="utf-8")
    missing = sorted(set(blocks) - used)
    if missing:
        sys.exit(f"blocks with no marker in {FILES}: {missing}")
    if args.check and stale:
        sys.exit("benchmark numbers are stale in " + ", ".join(stale)
                 + ": run scripts/docs/bench-tables.py and commit")
    print(("checked " if args.check else "rendered ") + f"{len(used)} blocks from {PUBLISHED.relative_to(ROOT)}"
          + ("" if args.check else f"; changed {', '.join(stale) or 'nothing'}"))


if __name__ == "__main__":
    main()
