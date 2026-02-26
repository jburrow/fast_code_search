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
| User-visible change log policy and history | `CHANGELOG.md` | Required for user-facing changes |

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
  - Full build/test/lint/release workflow
  - Platform-specific setup details (Linux/macOS/Windows)
  - Performance and debugging references
- Should be the canonical command reference used by other docs.

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

## Drift Prevention Checklist

When changing workflows or architecture:

1. Update canonical file first.
2. Update any cross-references in secondary files.
3. Verify commands still work (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`).
4. Confirm user-visible changes are recorded in `CHANGELOG.md`.

## Recommended Lightweight Review Gate

For PR review template/checklist:

- [ ] Updated canonical instruction source (if workflow changed)
- [ ] Updated cross-links in related docs
- [ ] Kept command examples consistent with canonical guide
- [ ] Updated `CHANGELOG.md` for user-visible changes

## Minimal Maintenance Routine

- Monthly or pre-release doc consistency pass:
  - README quick-start commands
  - CONTRIBUTING checklist requirements
  - Copilot instructions task cheatsheet
  - Development guide build/release targets

This blueprint is intentionally small. Keep it as policy and structure guidance, not another duplicated command reference.