//! Ranking weights and shared scoring helpers.
//!
//! Every boost the engine applies lives here so the README's ranking table,
//! the line-level scorers and the file-level fast-ranking metadata agree,
//! and so tuning is a one-file change.

/// Multiplicative boosts applied to a line-level match score.
#[derive(Debug, Clone, Copy)]
pub struct RankingWeights {
    /// Query appears with the exact case typed.
    pub exact_case: f64,
    /// Line holds a symbol definition.
    pub symbol_definition: f64,
    /// File lives under `src/` or `lib/`.
    pub src_lib_dir: f64,
    /// Match starts at the (trimmed) start of the line.
    pub line_start: f64,
    /// Floor for the inverse-line-length factor.
    pub min_line_length_factor: f64,
    /// Symbol search: the symbol name equals the query.
    pub symbol_exact_name: f64,
    /// Symbol search: the symbol name starts with the query.
    pub symbol_prefix_name: f64,
    /// Symbol search: variables/constants rank slightly below type and
    /// function definitions with the same name match.
    pub symbol_value_kind: f64,
    /// Score of a synthetic filename hit (a file whose name matches the
    /// query but whose content does not).
    pub filename_hit: f64,
    /// Per-dependent scale in `1 + log10(dependents) * dependency_log10`.
    pub dependency_log10: f64,
}

impl RankingWeights {
    /// The weights documented in the README ranking table.
    pub const DEFAULT: RankingWeights = RankingWeights {
        exact_case: 2.0,
        symbol_definition: 3.0,
        src_lib_dir: 1.5,
        line_start: 1.5,
        min_line_length_factor: 0.3,
        symbol_exact_name: 2.0,
        symbol_prefix_name: 1.5,
        symbol_value_kind: 0.9,
        filename_hit: 3.0,
        dependency_log10: 0.5,
    };

    /// `1 + log10(dependents) * dependency_log10`; 1.0 for files nobody imports.
    #[inline]
    pub fn dependency_boost(&self, dependents: u32) -> f64 {
        if dependents > 0 {
            1.0 + (dependents as f64).log10() * self.dependency_log10
        } else {
            1.0
        }
    }

    /// Gentle inverse-length factor: `1 / (1 + ln(1 + len/100))`, floored so a
    /// long definition line is never obliterated by a short comment.
    #[inline]
    pub fn line_length_factor(&self, line_len: usize) -> f64 {
        (1.0 / (1.0 + (line_len as f64 / 100.0).ln_1p())).max(self.min_line_length_factor)
    }
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// File-level additive scores used by fast ranking (see `FileMetadata::compute`).
#[derive(Debug, Clone, Copy)]
pub struct FileScoreWeights {
    pub base: f32,
    pub src_lib_dir: f32,
    pub code_extension: f32,
    pub doc_extension: f32,
    /// Cap on `log2(symbol_count)`.
    pub symbol_log2_cap: f32,
    /// Cap on `log2(dependents)`.
    pub dependency_log2_cap: f32,
    /// Multiplier for test/example paths.
    pub test_example_penalty: f32,
    /// Multiplier when the query matches the file stem.
    pub filename_match: f32,
}

impl FileScoreWeights {
    pub const DEFAULT: FileScoreWeights = FileScoreWeights {
        base: 1.0,
        src_lib_dir: 2.0,
        code_extension: 1.5,
        doc_extension: 0.5,
        symbol_log2_cap: 4.0,
        dependency_log2_cap: 5.0,
        test_example_penalty: 0.7,
        filename_match: 5.0,
    };
}

impl Default for FileScoreWeights {
    fn default() -> Self {
        Self::DEFAULT
    }
}
