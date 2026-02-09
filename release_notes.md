### Added
- **Two-phase ranking system for large-scale search**: Dramatically improves search performance on codebases with 100k+ files
  - Fast mode: Ranks candidates by pre-computed file metadata (symbols, imports, path), reads only top 2000 files
  - Full mode: Reads all candidate files for complete scoring
  - Auto mode (default): Automatically switches to Fast when >5,000 candidates
  - File-level scoring factors: symbol density (+4 max), src/lib location (+2), import count (+5 max), test/example penalty (0.7x)
  - New `rank` API parameter: `auto`, `fast`, or `full`
  - Response includes `rank_mode`, `total_candidates`, and `candidates_searched` metadata

- **Ranking mode UI toggle**: New dropdown in Advanced Options to select ranking mode
  - Results header shows actual mode used and files searched (e.g., "Fast (2000/100,000 files)")

- **Documentation page**: New `/docs.html` page in Web UI explaining the ranking system, API reference, and path filter patterns

- **Lazy file store**: Memory-mapped file content is now loaded on-demand rather than all at once
  - Reduces memory usage during index loading
  - ~8x faster index loading via parallel file mapping

### Changed
- Search methods now use file metadata ranking instead of early termination for large candidate sets
- `search_regex()` and `search_symbols()` also use fast ranking for consistency


