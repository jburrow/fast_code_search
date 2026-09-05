## What

<!-- One or two sentences: what changes and why. Link the roadmap task if there is one. -->

## How it was verified

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test` (and `--features semantic` if the semantic engine is touched)
- [ ] New behaviour has a test, or the PR says why it cannot
- [ ] CHANGELOG `[Unreleased]` updated for user-visible changes

## Notes for the reviewer

<!-- Anything non-obvious: trade-offs, follow-ups, things you are unsure about. -->
