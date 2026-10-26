# Latest results

<div id="live-results"><em>Loading the latest results from the benchmark workflow…</em></div>
<script>
(function () {
  const el = document.getElementById("live-results");
  const ms = ns => ns < 1e4 ? (ns / 1e3).toFixed(1) + " µs" : (ns / 1e6).toFixed(2) + " ms";
  fetch("../../dev/bench/results/latest.json", { cache: "no-store" })
    .then(r => { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(d => {
      const c = d.corpus, m = c.machine, b = c.build, rq = d.ranking_quality;
      let h = `<p><strong>From <a href="${d.run_url}">the latest benchmark run</a></strong> on commit <code>${d.commit.slice(0,7)}</code> (${d.generated_at.slice(0,10)}), ${m.cpu_model}, ${m.logical_cpus} logical CPUs, ${(m.mem_total_bytes/1e9).toFixed(0)} GB RAM, ${m.os}.</p>`;
      h += `<table><thead><tr><th>Build (${c.corpus.files.toLocaleString()} files, ${(c.corpus.bytes/1e6).toFixed(1)} MB)</th><th>Value</th></tr></thead><tbody>`;
      h += `<tr><td>Build</td><td>${(b.build_ns/1e9).toFixed(2)} s (${Math.round(b.files_per_second).toLocaleString()} files/s)</td></tr>`;
      h += `<tr><td>Resident memory after build</td><td>${(b.rss_after_bytes/1e6).toFixed(0)} MB</td></tr>`;
      h += `<tr><td>Index save / load</td><td>${ms(b.save_ns)} / ${ms(b.load_ns)} (${(b.index_bytes/1e6).toFixed(1)} MB on disk)</td></tr></tbody></table>`;
      h += `<table><thead><tr><th>Query</th><th>p50</th><th>p95</th></tr></thead><tbody>`;
      for (const q of c.queries) h += `<tr><td>${q.name}</td><td>${ms(q.p50_ns)}</td><td>${ms(q.p95_ns)}</td></tr>`;
      h += `</tbody></table>`;
      if (rq) h += `<p><strong>Ranking quality:</strong> precision@1 ${rq.precision_at_1.toFixed(2)}, precision@5 ${rq.precision_at_5.toFixed(2)} over ${rq.queries} labelled queries.</p>`;
      const cmp = d.comparison;
      if (cmp) {
        const f = v => v == null ? "—" : v.toFixed(1) + " ms";
        h += `<h3>Against scan tools (${cmp.tree_files.toLocaleString()} files, warm cache, median of ${cmp.runs})</h3>`;
        h += `<table><thead><tr><th>Query</th><th>ripgrep</th><th>ugrep</th><th>fcs (client)</th><th>fcs (engine)</th></tr></thead><tbody>`;
        for (const q of cmp.queries) h += `<tr><td>${q.label} <code>${q.query.replace(/</g,"&lt;")}</code></td><td>${f(q.ripgrep_ms)}</td><td>${f(q.ugrep_ms)}</td><td>${f(q.fcs_client_ms)}</td><td>${q.fcs_engine_ms.toFixed(2)} ms</td></tr>`;
        h += `</tbody></table><p>${cmp.tools.ripgrep}${cmp.tools.ugrep ? "; " + cmp.tools.ugrep : ""}. The fcs client time includes process start and the HTTP round trip; see <a href="comparison.html">the comparison page</a> for what these numbers do and do not mean.</p>`;
      }
      el.innerHTML = h;
    })
    .catch(() => { el.innerHTML = "<p><em>Live results are not available (the benchmark workflow has not published them yet); the tables below are from the pinned run.</em></p>"; });
})();
</script>

## Pinned run

<!-- bench:latest-run -->
From [run 40411254740](https://github.com/jburrow/fast_code_search/actions/runs/40411254740)
on commit `b1f9287` (2026-10-26, version 0.19.0), GitHub-hosted `ubuntu-latest` (AMD EPYC 7763, 4 vCPU, Ubuntu 24.04).
<!-- /bench:latest-run -->

The [README](https://github.com/jburrow/fast_code_search#benchmarks) carries the
same numbers; the per-commit trend is at
[dev/bench](https://jburrow.github.io/fast_code_search/dev/bench/).

Text and regex queries that match often take tens of milliseconds because
fcs ranks up to 32,768 matches before it returns page one. Before v0.17.0 it
ranked 512, and the same queries took about 1 ms but page one held only 10%
of the exhaustive top 50 (81% now). [Search limits](search-limits.md) has the
measurements and the cost on small repositories.

## Real corpus: tokio 1.45.0 + Django 5.2

<!-- bench:latest-corpus -->
6,268 files, 41.4 MB of text, 385k symbol references, 136k distinct trigrams, 9,784 dependency edges.

| Build | Value |
|---|---|
| Discovery | 114 ms |
| Build (read, trigrams, symbols, imports, merge) | 4.76 s, 1,318 files/s, 8.7 MB/s |
| Resident memory after build | 103 MB (process before build: 8.9 MB) |
| Index save | 113 ms (26.0 MB on disk) |
| Index load, reconciling | 84.3 ms |

| Query | p50 | p95 |
|---|---|---|
| text, common word | 23.7 ms | 25.3 ms |
| text, identifier | 7.18 ms | 7.60 ms |
| text, no match | 581 ns | 762 ns |
| text, short (two characters) | 29.0 ms | 30.3 ms |
| text, full ranking | 23.8 ms | 25.3 ms |
| text, path-filtered | 10.8 ms | 11.3 ms |
| regex with literal | 25.0 ms | 26.2 ms |
| regex, case-insensitive | 3.12 ms | 3.62 ms |
| regex, no literal (full scan) | 36.8 ms | 38.4 ms |
| symbol, exact | 3.63 ms | 3.93 ms |
| symbol, prefix | 2.25 ms | 2.52 ms |
| references | 2.26 ms | 2.38 ms |
| incremental: modify one file | 5.65 ms | 45.4 ms |

Ranking quality in the same run: precision@1 1.00, precision@5 1.00.
<!-- /bench:latest-corpus -->

## Synthetic (Criterion), last CI run

From [run 40113519420](https://github.com/jburrow/fast_code_search/actions/runs/40113519420)
on commit `140425f` (v0.19.0, 2026-10-24), the last run before the
Benchmarks workflow replaced these suites with instruction counts (see
[methodology](methodology.md)). They still run locally with `cargo bench`.

| Benchmark | Corpus | Mean |
|---|---|---|
| text search, common query | 100 files | 0.78 ms |
| text search, rare query | 100 files | 8.5 µs |
| text search, no match | 100 files | 0.45 µs |
| regex, literal | 100 files | 0.54 ms |
| regex, alternation | 100 files | 0.73 ms |
| regex, no literal | 100 files | 0.69 ms |
| index build | 100 files | 62 ms |
| index save / load | 1,000 files | 8.3 ms / 5.5 ms |

Values are Criterion means; the corpus is generated by
`benches/search_benchmark.rs`.
