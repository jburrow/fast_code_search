# Contributing to fast_code_search

Thank you for your interest in contributing to fast_code_search! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Please be respectful and constructive in all interactions. We aim to maintain a welcoming and inclusive environment for all contributors.

## Getting Started

### Prerequisites

- Rust 1.89 or later (the pinned toolchain in `rust-toolchain.toml` is used by CI)
- Protocol Buffers compiler (`protoc`)
- Git

### Setting Up Your Development Environment

1. Fork and clone the repository:
```bash
git clone https://github.com/YOUR_USERNAME/fast_code_search.git
cd fast_code_search
```

2. Install dependencies:
```bash
# On Debian/Ubuntu
sudo apt-get install protobuf-compiler

# On macOS
brew install protobuf
```

3. Build the project:
```bash
cargo build
```

4. Run tests to verify your setup:
```bash
cargo test
```

## Development Workflow

### Making Changes

1. Create a new branch for your feature or bugfix:
```bash
git checkout -b feature/your-feature-name
```

2. Make your changes following our coding standards (see below)

3. Run tests to ensure your changes don't break existing functionality:
```bash
cargo test
```

4. Run the linter and formatter:
```bash
cargo clippy
cargo fmt
```

5. Commit your changes with clear, descriptive commit messages:
```bash
git commit -m "Add feature: brief description"
```

### Coding Standards

- **Follow Rust conventions**: Use `rustfmt` and address all `clippy` warnings
- **Write tests**: All new features should include unit tests
- **Document public APIs**: Add doc comments for public functions and types
- **Keep it minimal**: Make the smallest changes necessary to achieve your goal
- **Avoid breaking changes**: Maintain backward compatibility when possible

### Code Style

This project uses standard Rust formatting:
```bash
# Format code
cargo fmt

# Check for issues
cargo clippy -- -D warnings
```

### Testing

- Write unit tests for new functionality in the same file using `#[cfg(test)]` modules
- Ensure all tests pass before submitting:
```bash
cargo test
```

- For performance-critical changes, consider adding benchmarks

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in imperative mood (e.g., "Add", "Fix", "Update")
- Keep the first line under 72 characters
- Add detailed explanation in the body if needed

Example:
```
Add support for C++ symbol extraction

Integrates tree-sitter-cpp to extract symbols from C++ files.
Includes tests for class and function definitions.
```

## Pull Request Process

1. Update documentation if you've changed functionality
2. Ensure all tests pass and code is formatted
3. Update the README.md if you've added new features
4. Update `CHANGELOG.md` for user-visible changes
5. Submit your pull request with a clear description of changes
6. Address any review comments promptly

Instruction-file ownership and anti-drift policy: [Instruction Files Blueprint](docs/INSTRUCTION_FILES_BLUEPRINT.md)

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Documentation update
- [ ] Refactoring

## Testing
Describe how you tested your changes

## Checklist
- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] `CHANGELOG.md` updated for user-visible changes
```

## Areas for Contribution

The current plan lives in [docs/plans/2026-09-04-keyword-engine-roadmap.md](docs/plans/2026-09-04-keyword-engine-roadmap.md);
every task there has file references and an acceptance criterion. Open items as of
September 2026:

### Engine
- Sectioned, mmap-able persistence format (lazy load, no double materialisation
  on save/load) — roadmap 6.1.
- Memory diet: intern paths once, collapse the per-file `OnceLock`s — roadmap 6.3.
- Multi-line regex (`(?s)`, `\n`) and symbol *reference* results — roadmap Phase 7.

### Benchmarks and tests
- A pinned mid-size real corpus for CI benchmarks (search, indexing, incremental
  update, symbol search, reconciliation) — roadmap 6.5.
- Explicit CORS-header and `/ws/progress` tests.

### Languages
- Symbol coverage is driven by each grammar's `tags.scm` plus a small
  supplementary walker in `src/symbols/extractor.rs`; adding a language means
  adding the grammar crate, mapping its extension, and (if the crate ships one)
  its `TAGS_QUERY`. Bash has no tags query and relies on the walker.

## Architecture Overview

Understanding the architecture will help you contribute effectively:

- **`src/index/`**: Indexing components (trigram index, file store)
- **`src/search/`**: Search engine with scoring
- **`src/symbols/`**: Tree-sitter integration for symbol extraction
- **`src/server/`**: gRPC server implementation
- **`proto/`**: Protocol buffer definitions

See DEVELOPMENT.md for detailed architecture documentation.

## Questions or Need Help?

- Open an issue for bugs or feature requests
- Tag maintainers in your PR for review
- Check existing issues and PRs before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (see LICENSE file).
