# Roadmap

Work is planned in the open. Larger initiatives have a plan document
under `docs/internal/plans/`; individual items are GitHub issues labelled
[`roadmap`](https://github.com/jburrow/fast_code_search/issues?q=is%3Aissue+is%3Aopen+label%3Aroadmap).

## Now

- **Documentation site** (this book): generated reference pages, a
  quick start executed in CI, guides for the questions people ask.
- **Benchmarks people can trust**: done for methodology, machine specs in
  every result, results generated onto the site, the
  [ranking-quality suite](../benchmarks/ranking-quality.md) and the
  [ripgrep/ugrep comparison](../benchmarks/comparison.md). Still open: a
  Zoekt comparison on a shared corpus and query set, and per-commit
  charts of memory and throughput.

## Next

- **Distribution**: done for `cargo binstall` metadata, the container
  image on GHCR, Sigstore-signed release archives, and release-workflow
  jobs for crates.io and a Homebrew tap that run once their tokens are
  configured. Still open: the first crates.io publish and the tap
  repository itself.
- **Visual identity**: architecture and ranking diagrams are in the book;
  still open: a wordmark and a demo recording.
- **Reference capture** for more languages (Go, Java, C# call sites) and
  cross-language import resolution improvements.

## Done recently

- 0.12: result diversity, inline SVG icons, on-demand rendering, JS test
  harness and Tailwind check in CI.
- 0.11: the review fixes (save races, regex candidate generation, phrase
  ranking, incremental batching, per-directory watches), the `fcs`
  client, the run-at-startup guide.

The full plan: [Repository presence plan](../imported/plan-repository.md).
