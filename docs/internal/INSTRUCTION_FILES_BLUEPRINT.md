# Instruction Files Blueprint

This document defines how instruction content is organized across repository docs to reduce drift and conflicting guidance for contributors and coding agents.

## Goals

- Keep one canonical source for each type of instruction.
- Avoid duplicating command blocks across multiple files.
- Make onboarding and contribution expectations predictable.
- Keep human contributor docs and agent docs aligned.

## Canonical Sources (Single Source of Truth)

| Topic | Canonical File | Notes |
|------|-----------------|-------|
| User-facing overview, quick start, feature summary | `README.md` | Keep concise and task-oriented |
| Contributor workflow, quality gates, release process | `docs/DEVELOPMENT.md` | Operational source for maintainers |
| Contribution policy and PR checklist | `CONTRIBUTING.md` | Process and review expectations |
| Agent-oriented repo map and coding conventions | `.github/copilot-instructions.md` | Keep aligned with canonical docs |
| Hard rules for agents (search behaviour, performance, main health) | `CLAUDE.md` | Short; rules only, pointers for everything else |
| User-visible change log policy and history | `CHANGELOG.md` | Required for user-facing changes |
| Facts repeated across docs (benchmark numbers, backtest count, limits, version) | `docs/DEVELOPMENT.md` ("Docs that repeat numbers") | Lists every place each fact appears |

## File Roles and Scope

### `README.md`

- Should include:
  - What the project does
  - Minimal setup and run commands
  - Links to deeper guides
- Should not include:
  - Long maintainer workflows
  - Detailed release mechanics

### `CONTRIBUTING.md`

- Should include:
  - PR lifecycle and expectations
  - Required checks before opening PR
  - Testing/documentation checklist
- Should not include:
  - Duplicated architecture deep dives
  - Full command encyclopedias

### `docs/DEVELOPMENT.md`

- Should include:
  - Full build/test/lint/release workflow, including the release health check
  - The table of facts repeated across docs and where each appears
  - Platform-specific setup details (Linux/macOS/Windows)
  - Performance and debugging references
- Should be the canonical command reference used by other docs.

### `CLAUDE.md`

- Should include:
  - Rules that past mistakes taught (every match in rank order, match ripgrep, benchmark A/B before merging hot-path changes, don't merge on a red main)
  - Pointers to the canonical docs
- Should not include:
  - Code maps, command references or anything `.github/copilot-instructions.md` or `docs/DEVELOPMENT.md` already own

### `.github/copilot-instructions.md`

- Should include:
  - High-signal code map and patterns for agents
  - Pointers to canonical docs for workflow details
  - Agent-specific safeguards (for example, server-process handling)
- Should not redefine release/test policy differently from contributor docs.

## Command Ownership Model

When a command appears in multiple files, `docs/DEVELOPMENT.md` owns the authoritative version.

- In other files, prefer:
  - short examples, and
  - links back to development guide sections.

## Policy Alignment Rules

Apply these rules to avoid conflicts:

1. **Testing policy**: use “integration tests prioritized, plus targeted unit tests for module logic.”
2. **Docs policy**: user-visible changes require `CHANGELOG.md` updates.
3. **API/proto policy**: gRPC changes must consider both `proto/search.proto` and `proto/semantic_search.proto` where relevant.
4. **Port references**: keep keyword vs semantic ports consistent across all docs.
5. **Platform setup**: include Windows path for required tools where Linux/macOS commands are shown.
6. **Repeated facts**: a number or behaviour stated in more than one doc is updated everywhere in the PR that changes it, and benchmark numbers name the workflow run and commit they come from.

## Drift Prevention Checklist

When changing workflows or architecture:

1. Update canonical file first.
2. Update any cross-references in secondary files.
3. Verify commands still work (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`).
4. Confirm user-visible changes are recorded in `CHANGELOG.md`.
5. Grep the docs for any old value you changed (a number, a default, a flag) and update every hit listed in "Docs that repeat numbers".

## Recommended Lightweight Review Gate

For PR review template/checklist:

- [ ] Updated canonical instruction source (if workflow changed)
- [ ] Updated cross-links in related docs
- [ ] Kept command examples consistent with canonical guide
- [ ] Updated `CHANGELOG.md` for user-visible changes
- [ ] Updated every doc that repeats a changed number or behaviour

## Minimal Maintenance Routine

- Monthly or pre-release doc consistency pass:
  - README quick-start commands
  - CONTRIBUTING checklist requirements
  - Copilot instructions task cheatsheet
  - Development guide build/release targets
  - Published benchmark numbers (README, book latest/comparison, site) against the latest Benchmarks run on `main`

This blueprint is intentionally small. Keep it as policy and structure guidance, not another duplicated command reference.