# Making fast_code_search feel premium: plan

Goal: a visitor to github.com/jburrow/fast_code_search should, within a
minute, understand what the project does, believe the performance claims,
install it, and find a well-organised answer to any follow-up question. The
engineering already supports that (CI on three platforms, tracked
benchmarks, tagged releases with assets, a real changelog); the public
surface does not yet show it.

## Where we are (audit, 2026-09-07)

**Repository page.** Description is "rapid, in-memory, text search"; no
topics, no homepage link, no social-preview image, Discussions off. Two
stars is not the problem; the page giving no reason to look further is.

**README** (355 lines). Good bones: pitch, screenshot, quick start, engine
table, benchmarks, API examples. Weaknesses: the synthetic benchmark table
cites a v0.9.0 CI run and the real-corpus table a laptop run from a
different commit, so the numbers contradict each other and the tracked
trends; the semantic engine gets equal billing although it is optional and
behind a feature flag; there is no demo GIF; install is "build from source"
only, though release archives exist; and it tries to be README, user guide
and API reference at once.

**Docs.** 15 markdown files with no structure a reader can navigate: user
guides (`CLI.md`, `RUN-AT-STARTUP.md`, `DEPLOYMENT.md`) sit beside internal
engineering notes (`plans/`, `INSTRUCTION_FILES_BLUEPRINT.md`) and seven
semantic-search planning documents. Nothing is generated from source, so
the CLI flags, config keys and API parameters drift from the code.

**Site** (jburrow.github.io/fast_code_search). A one-page marketing site
with a benchmark iframe. Its headline stats ("< 1 ms", "10 GB+", "600k+
files", "12+ languages") are hard-coded and not tied to any measurement.

**Benchmarks.** Criterion micro-benchmarks trended by
github-action-benchmark; a real-corpus run (tokio + Django) whose output
only lives in a job summary; a nightly rust-lang/rust run likewise. No
methodology page, no machine specification, no comparison with the tools
the README name-checks (ripgrep, Zoekt), no memory or throughput charts.

**Community.** `CONTRIBUTING.md`, `SECURITY.md` (18 lines), a PR template and
CODEOWNERS exist. Missing: code of conduct, issue templates, discussion
categories, a public roadmap, release notes on the GitHub release itself.

**Distribution.** Release archives for five targets. Not on crates.io, no
`cargo binstall` metadata, no Homebrew tap, no container image.

## Principles

1. **Every number is a link.** No performance figure appears anywhere
   without a link to the run that produced it and the machine it ran on.
   Numbers that cannot be backed are removed.
2. **Reference is generated, not written.** CLI flags, configuration keys,
   REST parameters and the gRPC schema are rendered from the source of
   truth in CI, so they cannot go stale.
3. **One document per question.** A reader arrives with one question
   (install? tune memory? API field?) and lands on one page that answers it.
4. **Show, then tell.** A 20-second recording of the CLI and web UI before
   any prose.
5. **Internal notes stay internal.** Plans, reviews and agent instructions
   live under `docs/internal/`, linked from the contributing guide only.

## Workstreams

### 1. First impression: README and repository metadata

Deliverables:
- README rewritten as a landing page (target 150 lines): one-sentence
  pitch; demo GIF; three install paths (release archive, `cargo install`,
  source); a three-command quick start ending in a real search; a "how it
  compares" table (grep/ripgrep, Zoekt, an IDE index) with honest trade-offs;
  a benchmark summary block that CI regenerates; a documentation map; a
  short architecture paragraph with one SVG diagram; badges that mean
  something (CI, release version, crates.io, docs, MSRV, license).
- Repository settings: description ("In-memory trigram code search server
  with a CLI, web UI, REST and gRPC APIs: millisecond queries over
  multi-gigabyte trees"), topics (`code-search`, `rust`, `trigram`,
  `tree-sitter`, `grpc`, `developer-tools`, `search-engine`, `ripgrep`,
  `sourcegraph`, `zoekt`), homepage set to the docs site, social preview
  image (1280×640, wordmark plus the UI), Discussions on with
  Q&A / Ideas / Show-and-tell categories.
- `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), issue templates (bug,
  performance regression with a required `corpus_bench` output, feature,
  question → Discussions), a fuller `SECURITY.md` (supported versions,
  private reporting via GitHub's advisory form, response targets).
- Release notes: `release.yml` fills the GitHub release body from the
  matching `CHANGELOG.md` section and lists the assets with checksums, so a
  release page is readable without opening the changelog.

Acceptance: a first-time visitor can install and run a search from the
README alone in under five minutes; every badge is green or explains
itself; no number in the README lacks a link.

### 2. Documentation site

Deliverables:
- An mdBook site published at the existing Pages URL (the current landing
  page becomes the book's front page, re-styled to match the web UI's
  look). Structure:
  - **Start here**: what it is, install, first index, first search (CLI,
    web UI, editor), run at startup.
  - **Guides**: configuration cookbook (monorepos, many repos, excluding
    build output, memory budgets, inotify limits), deployment (shared
    server, Docker, reverse proxy, TLS, auth), performance tuning,
    troubleshooting (a symptom → cause → fix table), upgrading (index
    format changes, what triggers a rebuild).
  - **Reference**: CLI (generated from clap), configuration keys (generated
    from the `--init` template and the config struct docs), query syntax
    (one page, with examples per operator), REST API (an OpenAPI 3
    document checked into the repo and rendered; examples for every
    endpoint), gRPC (generated from `proto/search.proto` comments),
    on-disk index format, exit codes and error envelope.
  - **How it works**: indexing pipeline, trigram candidate selection and
    regex analysis, ranking (tiers, fast vs full mode), incremental
    updates and the watcher, persistence, memory model. Each with a
    diagram.
  - **Benchmarks**: methodology, latest results (auto-generated), trends,
    comparison.
  - **Project**: changelog, roadmap, contributing, security, code of
    conduct, design notes (the `docs/design` papers) and engineering notes
    (`docs/internal`).
- A `scripts/docs/` generator run in CI: `fcs --help` → markdown, config
  template → key table, proto → reference page, benchmark JSON → tables.
  The build fails if generated pages differ from what is committed (same
  pattern as the Tailwind check), so reference drift is caught in PRs.
- Doc tests: every shell snippet in "Start here" is executed by a CI job
  against a release build, so the quick start cannot rot.
- Move `docs/plans`, `INSTRUCTION_FILES_BLUEPRINT.md` and the semantic
  planning documents to `docs/internal/`; keep `docs/semantic` to the user
  guide only.

Acceptance: the site has a search box, a sidebar, and a page for every
question in the troubleshooting table; generated pages are byte-identical
to source on `main`; the README links only to the site, never to raw
markdown.

### 3. Benchmarks people can trust

Deliverables:
- A methodology page: what each suite measures, corpora and their pinned
  commits, warm vs cold, how percentiles are computed, the runner's CPU,
  RAM and OS captured automatically into every result file, the
  regression policy (2× on Criterion, 20 % on real-corpus p95).
- `scripts/bench/` with one entry point that runs all three tiers
  (synthetic Criterion; real corpora tokio + Django; large: rust-lang/rust
  nightly and, weekly, a multi-repository workspace of about 500k files)
  and writes a versioned JSON result per run.
- Results published to the site by CI: a table of the latest `main`
  numbers with the run link and machine spec, trend charts per metric
  (search p50/p95, build files/s, RSS per file, load time, incremental
  update latency, searches/s at 8 concurrent clients), and a per-release
  table so a reader can see what changed between versions.
- A comparison page against ripgrep and ugrep (cold and warm tree, same
  queries, same machine, scripts included) and Zoekt (index build time,
  index size, query latency) with the trade-offs written plainly. The
  README's "why a server" argument then cites measurements instead of
  ripgrep's 2016 blog post.
- README and site headline numbers are generated from the latest run; the
  hand-typed "10 GB+ / 600k+" claims are replaced by measured values from
  the large-corpus tier, or dropped.

Acceptance: every benchmark figure anywhere resolves to a run URL and a
machine spec; a reader can reproduce any table with one command; the
site's headline stats change when `main` does.

### 4. Distribution and quality signals

Deliverables:
- Publish to crates.io (`cargo install fast_code_search` installs both
  binaries; add crate metadata, `readme`, `categories`, `keywords`,
  `include` list) and docs.rs for the library API.
- `cargo binstall` metadata pointing at the release archives; a Homebrew
  tap (`jburrow/homebrew-tap`) with a formula generated by the release
  workflow; a container image on GHCR built per release with a documented
  `docker run` line.
- Signed release checksums (the release workflow already produces
  `.sha256`; add a signed manifest with `cosign` or `minisign`).
- Coverage badge from the existing coverage job; MSRV and
  "unsafe forbidden" badges only if true.
- A public roadmap page (issues with a `roadmap` label rendered on the
  site, or a GitHub Project board linked from the README).

Acceptance: three one-line install commands work on a clean machine (Linux,
macOS, Windows); the release page carries notes, assets, checksums and a
signature.

### 5. Visual identity

Deliverables:
- Wordmark and icon (the web UI's yellow-on-black brutalist style is
  distinctive; carry it to the README header, the site, the social preview
  and the favicon).
- One demo GIF (CLI search, web UI search, hover preview, references), 20
  seconds, recorded with a script so it can be re-recorded per release.
- Consistent screenshots: light theme, 1440 px, real repositories, no
  personal paths.
- Architecture, indexing pipeline and ranking diagrams as SVG that render in
  light and dark GitHub themes.

### 6. Housekeeping

- Root directory: keep only what a contributor needs to see
  (`onnxruntime/`, `skills-lock.json`, `tailwind.config.js` and `package.json`
  move under `tools/` or `web/` where they belong, or are gitignored if
  local).
- Naming: pick one spelling (`fast_code_search` in code and URLs, "Fast
  Code Search" in prose) and apply it everywhere.
- Semantic engine positioning: label it "experimental, optional" in the
  README's one line about it and give it its own site section, so the
  keyword engine's story is not diluted.
- Claims audit: grep the README, site and docs for numbers, superlatives
  and "10GB+" style claims; keep only what workstream 3 measures.

## Sequencing

| Phase | Weeks | Content | Outcome |
|---|---|---|---|
| 1. Foundations | 1 | README rewrite, repo metadata, community files, release notes automation, claims audit, housekeeping | The GitHub page reads as a finished product |
| 2. Docs site | 2 | mdBook, generated reference, OpenAPI, quick-start doc tests, content migration | One place for every question |
| 3. Benchmarks | 3 | Harness, methodology, auto-published results, comparison page, headline numbers from data | Claims a sceptic can check |
| 4. Distribution and polish | 4 | crates.io, binstall, Homebrew, GHCR, signing, GIF, diagrams, wordmark, roadmap | Install anywhere in one line; looks the part |

Phases 2 and 3 can run in parallel; both depend on phase 1's claims audit
and housekeeping so nothing is documented twice.

## What only you can do

- GitHub settings: description, topics, homepage, social preview upload,
  Discussions, branch protection on `main` (require CI), the Pages source.
- Accounts and tokens: crates.io API token (as a repository secret for the
  release workflow), a `homebrew-tap` repository, GHCR package visibility.
- Approve the wordmark and the demo script before they are recorded.
- Decide whether the semantic engine stays in this repository's story or
  moves to its own page with an "experimental" label (the plan assumes the
  latter).

## Definition of done

- [ ] README under 200 lines with GIF, three install paths, generated
      benchmark block, documentation map; every number linked.
- [ ] Repository description, topics, homepage, social preview,
      Discussions, code of conduct, issue templates, fuller security policy.
- [ ] Docs site with Start here / Guides / Reference / How it works /
      Benchmarks / Project; reference pages generated and drift-checked in
      CI; quick start executed in CI.
- [ ] Benchmark methodology page; three tiers with machine specs in every
      result; results, trends and per-release tables generated on the site;
      comparison page with scripts.
- [ ] `cargo install`, `cargo binstall`, Homebrew and Docker install paths;
      signed releases with generated notes.
- [ ] Wordmark, demo GIF, diagrams; internal notes under `docs/internal`;
      claims audit passed.
