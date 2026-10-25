## What

<!-- One or two sentences: what changes and why. Link the roadmap task if there is one. -->

## How it was verified

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test` (and `--features semantic` if the semantic engine is touched)
- [ ] New behaviour has a test, or the PR says why it cannot
- [ ] CHANGELOG `[Unreleased]` updated for user-visible changes
- [ ] Every doc that states a changed number or behaviour updated
      (see "Docs that repeat numbers" in `docs/DEVELOPMENT.md`), or "None"

## Performance

<!-- Required when indexing, search, persistence or the watcher change:
     paste the table from `scripts/bench/ab.sh` (same machine, base vs this
     branch). For indexing, persistence or memory changes, add the
     302k-file corpus (`--corpus small,real`). Name any metric that got
     slower and what it buys; the same goes in the CHANGELOG entry.
     Otherwise write "Not affected". -->

## Notes for the reviewer

<!-- Anything non-obvious: trade-offs, follow-ups, things you are unsure about. -->
