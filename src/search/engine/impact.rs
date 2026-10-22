//! Diff impact analysis over the index: for each symbol a diff changes, the
//! files that use it (`fcs impact`, `POST /api/impact`, MCP `diff_impact`).
//!
//! Functions, classes, types and constants are looked up with a
//! name-guided walk of the import graph: from the changed file along
//! "imported by" edges, into files that mention the name (or into package
//! entry files, which re-export), following `X as Y` renames. Members
//! (methods, fields) are used through a value whose type fcs does not know,
//! so they are looked up by name among every file that transitively
//! imports theirs, with the files that also name the class first.
//!
//! Every use found is returned, in tiers that say how sure it is (a
//! recorded reference, a mention in code, a mention in a string or
//! comment); a cap on what is listed is always reported with the totals.
//! How this was measured against Jedi and the TypeScript compiler:
//! `docs/book/src/reference/impact.md`.

use super::SearchEngine;
use crate::dependencies::is_local_spec;
use crate::dependencies::scope::has_import_graph;
use crate::impact::defs::{self, Change, ChangedSymbol, Def, DefKind};
use crate::impact::diff::{self, FileDiff};
use crate::symbols::SymbolType;
use roaring::RoaringBitmap;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Files listed per changed symbol when the caller does not say.
pub const DEFAULT_IMPACT_FILES: usize = 200;
/// Use lines listed per file when the caller does not say.
pub const DEFAULT_IMPACT_LINES: usize = 3;
/// Characters of a use line shown.
const LINE_CHARS: usize = 200;
/// Changed symbols analysed per request; the rest are listed, not searched.
pub const MAX_IMPACT_SYMBOLS: usize = 500;

/// What to analyse and how much to list.
#[derive(Debug, Clone)]
pub struct ImpactOptions {
    /// Folder the diff's paths are relative to (the repository root), when
    /// the caller knows it; else paths are matched as display paths and
    /// unique path suffixes.
    pub root: Option<PathBuf>,
    /// Files listed per symbol (0 = every file).
    pub max_files: usize,
    /// Use lines listed per file (0 = every line).
    pub max_lines: usize,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            root: None,
            max_files: DEFAULT_IMPACT_FILES,
            max_lines: DEFAULT_IMPACT_LINES,
        }
    }
}

/// How sure a use is. Ordered best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UseTier {
    /// A reference fcs recorded at index time (call, type, value, attribute).
    Reference,
    /// The name as a whole word in code, not at a recorded reference
    /// position (an import line, a default value, a JSX tag, ...).
    Code,
    /// The name only in a string or a comment (dotted paths in settings
    /// are real uses; prose is not).
    Text,
}

impl UseTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Code => "code",
            Self::Text => "text",
        }
    }
}

/// How the files to look in were chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactScope {
    /// The name-guided walk over importers.
    Importers,
    /// Every file that transitively imports the changed file (members).
    AllImporters,
    /// Every indexed file: the language has no import graph.
    Repository,
    /// Not searched (see the symbol's notes).
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseLine {
    /// 1-based.
    pub line: usize,
    /// 1-based byte column of the name, as ripgrep counts.
    pub column: usize,
    pub text: String,
    pub tier: UseTier,
    /// The reference kind (`call`, `type`, `value`, `attribute`), for
    /// recorded references.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFile {
    pub path: String,
    /// The best tier among the file's lines.
    pub tier: UseTier,
    /// Members only: the file also names the class (or impl type).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names_owner: Option<bool>,
    /// The file is changed by the same diff.
    pub in_diff: bool,
    pub test: bool,
    pub lines: Vec<UseLine>,
    /// Use lines in the file (`lines` may hold fewer).
    pub total_lines: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierCounts {
    pub reference: usize,
    pub code: usize,
    pub text: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolImpact {
    /// The changed file, as the diff names it.
    pub file: String,
    pub name: String,
    pub qualname: String,
    pub kind: DefKind,
    pub change: Change,
    /// A changed line is in the signature, not only the body.
    pub signature: bool,
    /// 1-based line of the definition, in the side of the file shown.
    pub line: usize,
    /// The names searched: the symbol's own (its class's for a
    /// constructor) and renames found on the way.
    pub names: Vec<String>,
    pub scope: ImpactScope,
    /// Files that use it (`users` may list fewer).
    pub total_files: usize,
    pub tiers: TierCounts,
    pub users: Vec<UserFile>,
    /// Limits and gaps that apply to this symbol.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactFileSummary {
    pub path: String,
    /// `modified`, `added`, `deleted` or `renamed`.
    pub status: String,
    /// The indexed file it was matched to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_path: Option<String>,
    /// Changed symbols found in it.
    pub symbols: usize,
    /// Changed lines outside every definition (imports, module code).
    pub module_lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub files: Vec<ImpactFileSummary>,
    pub symbols: Vec<SymbolImpact>,
    /// Files that use at least one changed symbol (other than the changed
    /// files themselves).
    pub total_files: usize,
    pub notes: Vec<String>,
}

/// A name being looked for, with what settles a file cheaply.
struct Needle {
    text: String,
    /// The trigram candidates: a file outside has no occurrence.
    candidates: RoaringBitmap,
    /// The interned reference name, when any file references it.
    ref_id: Option<u32>,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Byte offset of the first whole-word `name` in `text`: not preceded or
/// followed by an identifier character.
fn find_word(text: &str, name: &str) -> Option<usize> {
    text.match_indices(name).map(|(at, _)| at).find(|&at| {
        !text[..at].chars().next_back().is_some_and(is_ident)
            && !text[at + name.len()..].chars().next().is_some_and(is_ident)
    })
}

/// Import and export lines that rename `name`: `import X as Y`,
/// `from m import X as Y`, `export { X as Y }`, `use a::X as Y;`, and a
/// lone `X as Y,` line inside a multi-line import.
fn renames(content: &str, name: &str) -> Vec<String> {
    let re = regex::Regex::new(&format!(
        r"(^|[^\w$]){}\s+as\s+([A-Za-z_$][\w$]*)",
        regex::escape(name)
    ))
    .expect("escaped name");
    let mut out = Vec::new();
    for line in content.lines() {
        if !line.contains(name) || !line.contains(" as ") {
            continue;
        }
        let t = line.trim_start();
        let import_line = [
            "import ",
            "from ",
            "export ",
            "use ",
            "pub use ",
            "pub(crate) use ",
        ]
        .iter()
        .any(|p| t.starts_with(p))
            || t.starts_with(name);
        if !import_line {
            continue;
        }
        for cap in re.captures_iter(line) {
            let alias = cap[2].to_string();
            if alias != name && !out.contains(&alias) {
                out.push(alias);
            }
        }
    }
    out
}

fn is_entry_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(name, "__init__.py" | "mod.rs" | "lib.rs" | "main.rs")
        || name.strip_prefix("index.").is_some_and(|ext| {
            matches!(
                ext,
                "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts"
            )
        })
}

/// Languages whose files record no references (their grammar's tags
/// query has none): only mentions can be found.
fn records_references(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !matches!(
        ext.as_str(),
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "sh" | "bash"
    ) && crate::symbols::SymbolExtractor::new(Path::new(path)).is_supported()
}

/// Definitions for a language impact analysis does not parse itself: the
/// index's own definitions, each taken to run to the next one.
fn approximate_definitions(path: &str, src: &str) -> Vec<Def> {
    let extractor = crate::symbols::SymbolExtractor::new(Path::new(path));
    let Ok(mut symbols) = extractor.extract(src) else {
        return Vec::new();
    };
    symbols.retain(|s| s.is_definition && s.symbol_type != SymbolType::FileName);
    symbols.sort_by_key(|s| s.line);
    let last = src.lines().count().saturating_sub(1);
    let starts: Vec<usize> = symbols.iter().map(|s| s.line).collect();
    symbols
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let end = starts
                .iter()
                .skip(i + 1)
                .find(|&&l| l > s.line)
                .map_or(last, |l| l - 1);
            let kind = match s.symbol_type {
                SymbolType::Method => DefKind::Method,
                SymbolType::Field | SymbolType::Property => DefKind::Field,
                SymbolType::Class => DefKind::Class,
                SymbolType::Variable | SymbolType::Constant => DefKind::Constant,
                SymbolType::Function | SymbolType::Macro => DefKind::Function,
                _ => DefKind::Type,
            };
            Def {
                qualname: s.name.clone(),
                name: s.name.clone(),
                kind,
                start: s.line,
                end,
                name_row: s.line,
                header_end: s.line,
            }
        })
        .collect()
}

/// Is the text at (`row`, byte `col`) inside a string or a comment? A
/// one-pass lexer by file extension (comment markers, quote styles,
/// strings that span lines), not a parse: cheap enough to run on every
/// file that mentions a name.
struct TextClassifier {
    syntax: Syntax,
    /// Lexer state at the start of each line.
    starts: Vec<Lex>,
}

#[derive(Clone, Copy)]
struct Syntax {
    hash_comments: bool,
    slash_comments: bool,
    single_quotes: bool,
    triple_quotes: bool,
    /// Backtick strings that may span lines (JS/TS templates, Go raw strings).
    backticks: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lex {
    Code,
    LineComment,
    BlockComment,
    /// Inside a string closed by this quote byte; `triple` for `"""`/`'''`.
    Str {
        quote: u8,
        triple: bool,
    },
}

impl Syntax {
    fn of(path: &str) -> Self {
        let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        let ext = ext.to_ascii_lowercase();
        let hash = matches!(
            ext.as_str(),
            "py" | "pyi"
                | "pyw"
                | "rb"
                | "sh"
                | "bash"
                | "zsh"
                | "pl"
                | "pm"
                | "r"
                | "yaml"
                | "yml"
                | "toml"
                | "ex"
                | "exs"
                | "nim"
                | "jl"
                | "cmake"
                | "mk"
        );
        let slash = matches!(
            ext.as_str(),
            "rs" | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "mts"
                | "cts"
                | "go"
                | "java"
                | "c"
                | "h"
                | "cc"
                | "cpp"
                | "cxx"
                | "hpp"
                | "hh"
                | "cs"
                | "kt"
                | "kts"
                | "swift"
                | "scala"
                | "dart"
                | "zig"
                | "proto"
                | "css"
                | "scss"
                | "less"
                | "groovy"
                | "gradle"
        );
        let php = ext == "php";
        let known = hash || slash || php;
        Self {
            hash_comments: hash || php || !known,
            slash_comments: slash || php || !known,
            // Rust's `'a` is a lifetime, not a string.
            single_quotes: ext != "rs",
            triple_quotes: matches!(ext.as_str(), "py" | "pyi" | "pyw") || !known,
            backticks: matches!(
                ext.as_str(),
                "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "go"
            ),
        }
    }

    /// Lex `line` from `state` up to byte `upto` (or the whole line), and
    /// return the state there.
    fn lex(&self, mut state: Lex, line: &[u8], upto: usize) -> Lex {
        let end = upto.min(line.len());
        let mut i = 0;
        while i < end {
            let b = line[i];
            match state {
                Lex::LineComment => return state,
                Lex::BlockComment => {
                    if b == b'*' && line.get(i + 1) == Some(&b'/') {
                        state = Lex::Code;
                        i += 1;
                    }
                }
                Lex::Str { quote, triple } => {
                    if b == b'\\' && quote != b'`' {
                        i += 1;
                    } else if b == quote {
                        if !triple {
                            state = Lex::Code;
                        } else if line.get(i + 1) == Some(&quote) && line.get(i + 2) == Some(&quote)
                        {
                            state = Lex::Code;
                            i += 2;
                        }
                    }
                }
                Lex::Code => {
                    let next = line.get(i + 1).copied();
                    if (b == b'#' && self.hash_comments)
                        || (b == b'/' && self.slash_comments && next == Some(b'/'))
                    {
                        return Lex::LineComment;
                    } else if b == b'/' && self.slash_comments && next == Some(b'*') {
                        state = Lex::BlockComment;
                        i += 1;
                    } else if b == b'"'
                        || (b == b'\'' && self.single_quotes)
                        || (b == b'`' && self.backticks)
                    {
                        let triple = self.triple_quotes
                            && b != b'`'
                            && next == Some(b)
                            && line.get(i + 2) == Some(&b);
                        state = Lex::Str { quote: b, triple };
                        if triple {
                            i += 2;
                        }
                    }
                }
            }
            i += 1;
        }
        state
    }
}

impl TextClassifier {
    fn new(path: &str, src: &str) -> Self {
        let syntax = Syntax::of(path);
        let mut starts = Vec::new();
        let mut state = Lex::Code;
        for line in src.lines() {
            starts.push(state);
            state = match syntax.lex(state, line.as_bytes(), usize::MAX) {
                Lex::LineComment => Lex::Code,
                // Only triple-quoted and backtick strings span lines.
                Lex::Str { quote, triple } if !triple && quote != b'`' => Lex::Code,
                s => s,
            };
        }
        Self { syntax, starts }
    }

    fn is_text(&self, row: usize, col: usize, line: &str) -> bool {
        let start = self.starts.get(row).copied().unwrap_or(Lex::Code);
        self.syntax.lex(start, line.as_bytes(), col) != Lex::Code
    }
}

fn truncate(line: &str) -> String {
    let t = line.trim_end();
    if t.chars().count() <= LINE_CHARS {
        return t.to_string();
    }
    let mut s: String = t.chars().take(LINE_CHARS).collect();
    s.push('…');
    s
}

/// Work shared by the symbols of one request: the transitive importers of
/// a changed file (members of one class share them) and parsed files used
/// to tell code from strings and comments.
#[derive(Default)]
struct Cache {
    importers: FxHashMap<u32, std::rc::Rc<Vec<u32>>>,
    parsed: FxHashMap<u32, std::rc::Rc<TextClassifier>>,
}

/// Parsed files kept per request.
const PARSED_CACHE: usize = 4096;

/// The two sides of one changed file.
struct Sides {
    diff: FileDiff,
    id: Option<u32>,
    old: Option<String>,
    new: Option<String>,
    note: Option<String>,
}

impl SearchEngine {
    fn needle(&self, name: &str) -> Needle {
        Needle {
            text: name.to_string(),
            candidates: self.text_candidates(&name.to_lowercase()).into_owned(),
            ref_id: self.reference_name_id(name),
        }
    }

    fn content_of(&self, id: u32) -> Option<std::borrow::Cow<'_, str>> {
        self.file_store.get(id)?.as_str().ok()
    }

    fn references_name(&self, id: u32, needles: &[Needle]) -> bool {
        let refs = self.references_of(id);
        needles.iter().any(|n| {
            n.ref_id
                .is_some_and(|rid| n.candidates.contains(id) && refs.iter().any(|r| r.name == rid))
        })
    }

    /// Does file `id` mention any of `needles` as a whole word? Bitmap
    /// first, then recorded references, then the text.
    fn mentions(&self, id: u32, needles: &[Needle]) -> bool {
        if !needles.iter().any(|n| n.candidates.contains(id)) {
            return false;
        }
        if self.references_name(id, needles) {
            return true;
        }
        let Some(content) = self.content_of(id) else {
            return false;
        };
        needles
            .iter()
            .any(|n| n.candidates.contains(id) && find_word(&content, &n.text).is_some())
    }

    /// The indexed file a diff path names: under `root` when given, else
    /// as a display path, else as a unique path suffix. `Err` lists the
    /// files an ambiguous suffix matches.
    fn resolve_diff_path(&self, root: Option<&Path>, path: &str) -> Result<Option<u32>, String> {
        if let Some(root) = root {
            if let Some(id) = self.find_file_id_exact(&root.join(path)) {
                return Ok(Some(id));
            }
        }
        if let Some(id) = self.find_file_id_exact(Path::new(path)) {
            return Ok(Some(id));
        }
        if let Some(id) = self.find_by_display_path(path) {
            return Ok(Some(id));
        }
        let suffix = format!("/{}", path.replace('\\', "/"));
        let mut found = Vec::new();
        for id in 0..self.file_store.len() as u32 {
            if let Some(f) = self.file_store.get(id) {
                if f.path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with(&suffix)
                {
                    found.push(id);
                }
            }
        }
        match found.len() {
            0 => Ok(None),
            1 => Ok(Some(found[0])),
            n => {
                let shown: Vec<String> = found
                    .iter()
                    .take(5)
                    .filter_map(|&id| self.display_path(id).map(|p| p.into_owned()))
                    .collect();
                Err(format!(
                    "{path} matches {n} indexed files ({}{}); pass the repository root so it is unambiguous",
                    shown.join(", "),
                    if n > 5 { ", ..." } else { "" }
                ))
            }
        }
    }

    /// Rebuild both sides of `d` from the indexed copy (whichever side it
    /// is) or from the diff alone.
    fn sides(&self, d: FileDiff, root: Option<&Path>) -> Sides {
        let lookup = d
            .new_path
            .as_deref()
            .map(|p| self.resolve_diff_path(root, p))
            .filter(|r| !matches!(r, Ok(None)))
            .or_else(|| {
                d.old_path
                    .as_deref()
                    .map(|p| self.resolve_diff_path(root, p))
            });
        let (id, mut note) = match lookup {
            Some(Ok(id)) => (id, None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, None),
        };
        let content = id
            .and_then(|id| self.content_of(id))
            .map(|c| c.into_owned());
        let (old, new) = match (&content, d.old_path.is_none(), d.new_path.is_none()) {
            (_, true, _) => (None, Some(d.whole_side(true))),
            (_, _, true) => (Some(d.whole_side(false)), None),
            (Some(c), _, _) => {
                let back = diff::apply(&d, c, false);
                let forward = diff::apply(&d, c, true);
                // When both apply (a pure insertion or deletion applies
                // anywhere), the side whose lines were found in place wins;
                // a tie goes to "the index is the new side", as for a
                // watched working tree and `git diff`.
                let index_is_old = forward.is_some()
                    && (back.is_none()
                        || diff::anchored_lines(&d, true) > diff::anchored_lines(&d, false));
                if index_is_old {
                    (Some(c.clone()), forward)
                } else if let Some(old) = back {
                    (Some(old), Some(c.clone()))
                } else {
                    note.get_or_insert_with(|| {
                        "the indexed copy matches neither side of the diff (index out of date, or a diff against another revision): its changes are not analysed".to_string()
                    });
                    (None, None)
                }
            }
            (None, _, _) => {
                note.get_or_insert_with(|| {
                    "not in the index: its changes are not analysed".to_string()
                });
                (None, None)
            }
        };
        Sides {
            diff: d,
            id,
            old,
            new,
            note,
        }
    }

    /// The impact of the unified diff `diff_text`.
    pub fn impact(&self, diff_text: &str, opts: &ImpactOptions) -> Result<ImpactReport, String> {
        let files = diff::parse(diff_text)?;
        if files.is_empty() {
            return Err("no file changes found in the diff (expected unified diff format, as `git diff` prints)".into());
        }
        let root = opts.root.as_deref();
        let sides: Vec<Sides> = files.into_iter().map(|d| self.sides(d, root)).collect();
        let in_diff: FxHashSet<u32> = sides.iter().filter_map(|s| s.id).collect();
        let mut report = ImpactReport {
            files: Vec::new(),
            symbols: Vec::new(),
            total_files: 0,
            notes: Vec::new(),
        };
        let mut changed: Vec<(usize, ChangedSymbol, bool)> = Vec::new();
        for (i, s) in sides.iter().enumerate() {
            let path = s.diff.path().to_string();
            let status = match (&s.diff.old_path, &s.diff.new_path) {
                (None, _) => "added",
                (_, None) => "deleted",
                (Some(a), Some(b)) if a != b => "renamed",
                _ => "modified",
            };
            let parse = |text: &Option<String>| -> (Vec<Def>, bool) {
                match text {
                    Some(t) if defs::language(&path).is_some() => {
                        (defs::definitions(&path, t), false)
                    }
                    Some(t) => (approximate_definitions(&path, t), true),
                    None => (Vec::new(), false),
                }
            };
            let analysable = s.old.is_some() || s.new.is_some();
            let (old_defs, approx) = parse(&s.old);
            let (new_defs, approx_new) = parse(&s.new);
            let (syms, outside) = if analysable {
                defs::changed_symbols(
                    &old_defs,
                    &new_defs,
                    &if s.old.is_some() {
                        s.diff.old_rows()
                    } else {
                        Vec::new()
                    },
                    &if s.new.is_some() {
                        s.diff.new_rows()
                    } else {
                        Vec::new()
                    },
                )
            } else {
                (Vec::new(), 0)
            };
            let mut note = s.note.clone();
            if analysable && (approx || approx_new) && !syms.is_empty() {
                note = Some(
                    "language not parsed for definition spans: a change is put in the nearest definition above it".into(),
                );
            }
            report.files.push(ImpactFileSummary {
                path: path.clone(),
                status: status.to_string(),
                indexed_path: s
                    .id
                    .and_then(|id| self.display_path(id).map(|p| p.into_owned())),
                symbols: syms.len(),
                module_lines: outside,
                note,
            });
            for c in syms {
                changed.push((i, c, approx || approx_new));
            }
        }
        if changed.len() > MAX_IMPACT_SYMBOLS {
            report.notes.push(format!(
                "the diff changes {} symbols; the first {MAX_IMPACT_SYMBOLS} were searched, the rest are listed without users",
                changed.len()
            ));
        }
        let mut all_users: FxHashSet<u32> = FxHashSet::default();
        let mut cache = Cache::default();
        for (n, (i, c, approx)) in changed.into_iter().enumerate() {
            let s = &sides[i];
            let search = n < MAX_IMPACT_SYMBOLS;
            let mut sym =
                self.symbol_impact(s, &c, &in_diff, opts, search, &mut all_users, &mut cache);
            if approx {
                sym.notes.push(
                    "definition span approximated (this language is not parsed for spans)".into(),
                );
            }
            report.symbols.push(sym);
        }
        report.total_files = all_users.len();
        Ok(report)
    }

    #[allow(clippy::too_many_arguments)]
    fn symbol_impact(
        &self,
        s: &Sides,
        c: &ChangedSymbol,
        in_diff: &FxHashSet<u32>,
        opts: &ImpactOptions,
        search: bool,
        all_users: &mut FxHashSet<u32>,
        cache: &mut Cache,
    ) -> SymbolImpact {
        let path = s.diff.path().to_string();
        let def = &c.def;
        let mut notes = Vec::new();
        // A constructor is used through its class.
        let ctor = matches!(
            def.name.as_str(),
            "__init__" | "__new__" | "constructor" | "new"
        ) && def.kind.is_member()
            && def.owner().is_some();
        let (name, member) = if ctor && def.name != "new" {
            (def.owner().unwrap_or(&def.name).to_string(), false)
        } else {
            (def.name.clone(), def.kind.is_member())
        };
        let mut out = SymbolImpact {
            file: path.clone(),
            name: def.name.clone(),
            qualname: def.qualname.clone(),
            kind: def.kind,
            change: c.change,
            signature: c.signature,
            line: def.name_row + 1,
            names: vec![name.clone()],
            scope: ImpactScope::None,
            total_files: 0,
            tiers: TierCounts::default(),
            users: Vec::new(),
            notes: Vec::new(),
        };
        if !search {
            out.notes
                .push("not searched: too many changed symbols in one diff".into());
            return out;
        }
        if name.len() < 2 {
            out.notes.push("name too short to search".into());
            return out;
        }
        if def.name.starts_with("__") && def.name.ends_with("__") && !ctor {
            notes.push(format!(
                "{} is called by the language (operators, str(), iteration), not by name: only explicit uses are found",
                def.name
            ));
        }
        let Some(anchor) = s.id else {
            out.notes.push(if c.change == Change::Added {
                "new file, not in the index yet: nothing can use it".into()
            } else {
                "the file is not in the index: users cannot be traced".into()
            });
            return out;
        };
        let graph = has_import_graph(Path::new(&path));
        let mut needles = vec![self.needle(&name)];
        let owner = member
            .then(|| def.owner().map(|o| self.needle(o)))
            .flatten();

        // Which files to look in, and which of them mention the name.
        let (scope, hits): (ImpactScope, Vec<u32>) = if !graph {
            notes.push(
                "no import graph for this language: name matches anywhere in the index, so other things with the same name are listed too".into(),
            );
            let hits = needles[0]
                .candidates
                .iter()
                .filter(|&id| id != anchor && self.file_store.get(id).is_some())
                .filter(|&id| {
                    if member {
                        self.references_name(id, &needles)
                    } else {
                        self.mentions(id, &needles)
                    }
                })
                .collect();
            (ImpactScope::Repository, hits)
        } else if member {
            notes.push(format!(
                "{} is a member: found by name on any receiver, among every file that imports {path} directly or not; files that also name {} come first",
                def.name,
                def.owner().unwrap_or("its type")
            ));
            let importers = cache
                .importers
                .entry(anchor)
                .or_insert_with(|| {
                    let mut seen = FxHashSet::from_iter([anchor]);
                    let mut queue = VecDeque::from([anchor]);
                    while let Some(cur) = queue.pop_front() {
                        for n in self.dependency_index.get_dependents(cur) {
                            if seen.insert(n) {
                                queue.push_back(n);
                            }
                        }
                    }
                    seen.remove(&anchor);
                    std::rc::Rc::new(seen.into_iter().collect())
                })
                .clone();
            let mut hits: Vec<u32> = importers
                .iter()
                .copied()
                .filter(|&id| needles[0].candidates.contains(id))
                .filter(|&id| {
                    self.references_name(id, &needles)
                        || (def.kind == DefKind::Field && self.mentions(id, &needles))
                })
                .collect();
            hits.sort_unstable();
            (ImpactScope::AllImporters, hits)
        } else {
            let (hits, renamed) = self.guided_walk(anchor, &mut needles);
            if !renamed.is_empty() {
                notes.push(format!("followed renames: {}", renamed.join(", ")));
            }
            (ImpactScope::Importers, hits)
        };
        if graph {
            let local = self.local_unresolved(hits.iter().copied().chain([anchor]));
            if local > 0 {
                notes.push(format!(
                    "{local} project imports in these files resolve to no indexed file: users behind them may be missing"
                ));
            }
        }
        if !records_references(&path) {
            notes.push("no references are recorded for this language: mentions only".into());
        }
        if member && def.kind == DefKind::Method {
            notes.push(
                "methods are listed where fcs recorded a reference; a method passed as a value or called through getattr is not found".into(),
            );
        }

        // One entry per file, with its use lines.
        let mut users: Vec<(UserFile, f32)> = Vec::new();
        for id in hits {
            let Some(content) = self.content_of(id) else {
                continue;
            };
            let Some(display) = self.display_path(id).map(|p| p.into_owned()) else {
                continue;
            };
            let refs = self.references_of(id);
            let ref_rows: FxHashMap<u32, &'static str> = refs
                .iter()
                .filter(|r| needles.iter().any(|n| n.ref_id == Some(r.name)))
                .map(|r| (r.line, r.kind().as_str()))
                .collect();
            let refs_only = member && def.kind == DefKind::Method;
            let mut classifier: Option<std::rc::Rc<TextClassifier>> = None;
            let mut lines = Vec::new();
            for (row, line) in content.lines().enumerate() {
                let Some(col) = needles.iter().find_map(|n| find_word(line, &n.text)) else {
                    continue;
                };
                let (tier, kind) = match ref_rows.get(&(row as u32)) {
                    Some(k) => (UseTier::Reference, Some(*k)),
                    // A method is listed only where it is referenced.
                    None if refs_only => continue,
                    None => {
                        let cls = classifier.get_or_insert_with(|| {
                            if let Some(c) = cache.parsed.get(&id) {
                                return c.clone();
                            }
                            let c = std::rc::Rc::new(TextClassifier::new(&display, &content));
                            if cache.parsed.len() < PARSED_CACHE {
                                cache.parsed.insert(id, c.clone());
                            }
                            c
                        });
                        if cls.is_text(row, col, line) {
                            (UseTier::Text, None)
                        } else {
                            (UseTier::Code, None)
                        }
                    }
                };
                lines.push(UseLine {
                    line: row + 1,
                    column: col + 1,
                    text: truncate(line),
                    tier,
                    kind: kind.map(str::to_string),
                });
            }
            if lines.is_empty() {
                continue;
            }
            lines.sort_by_key(|l| (l.tier, l.line));
            let tier = lines[0].tier;
            let total_lines = lines.len();
            if opts.max_lines > 0 {
                lines.truncate(opts.max_lines);
            }
            let names_owner = owner.as_ref().map(|o| {
                o.candidates.contains(id)
                    && (self.references_name(id, std::slice::from_ref(o))
                        || find_word(&content, &o.text).is_some())
            });
            let score = self.get_file_metadata(id).base_score;
            users.push((
                UserFile {
                    test: crate::dependencies::graph::is_test_path(&display),
                    path: display,
                    tier,
                    names_owner,
                    in_diff: in_diff.contains(&id),
                    lines,
                    total_lines,
                },
                score,
            ));
            all_users.insert(id);
        }
        // Best tier first; for members, files naming the class first; then
        // shipped code above tests, widely used files first, then path.
        users.sort_by(|(a, sa), (b, sb)| {
            a.tier
                .cmp(&b.tier)
                .then(
                    b.names_owner
                        .unwrap_or(false)
                        .cmp(&a.names_owner.unwrap_or(false)),
                )
                .then(a.test.cmp(&b.test))
                .then(sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.path.cmp(&b.path))
        });
        for (u, _) in &users {
            match u.tier {
                UseTier::Reference => out.tiers.reference += 1,
                UseTier::Code => out.tiers.code += 1,
                UseTier::Text => out.tiers.text += 1,
            }
        }
        out.total_files = users.len();
        out.users = users.into_iter().map(|(u, _)| u).collect();
        if opts.max_files > 0 && out.users.len() > opts.max_files {
            notes.push(format!(
                "listing {} of {} files; raise the file limit (fcs impact -n N, max_files; 0 = all) to see the rest",
                opts.max_files, out.total_files
            ));
            out.users.truncate(opts.max_files);
        }
        out.scope = scope;
        out.names = needles.iter().map(|n| n.text.clone()).collect();
        out.notes = notes;
        out
    }

    /// The name-guided walk: importers of `anchor`, transitively, through
    /// files that mention one of `needles` or are package entry files.
    /// Renames found in the files reached add needles. Returns the files
    /// that mention a name, and the renames followed.
    fn guided_walk(&self, anchor: u32, needles: &mut Vec<Needle>) -> (Vec<u32>, Vec<String>) {
        let mut seen = FxHashSet::from_iter([anchor]);
        let mut queue = VecDeque::from([anchor]);
        let mut hits = Vec::new();
        let mut renamed = Vec::new();
        while let Some(cur) = queue.pop_front() {
            for n in self.dependency_index.get_dependents(cur) {
                if !seen.insert(n) {
                    continue;
                }
                let mentions = self.mentions(n, needles);
                let entry = self
                    .file_store
                    .get(n)
                    .is_some_and(|f| is_entry_file(&f.path));
                if mentions {
                    hits.push(n);
                }
                if !(mentions || entry) {
                    continue;
                }
                // A file that renames the name re-exports it under the new
                // one: its importers are searched for both.
                if let Some(content) = self.content_of(n) {
                    let current: Vec<String> = needles.iter().map(|x| x.text.clone()).collect();
                    for name in &current {
                        if !content.contains(name.as_str()) {
                            continue;
                        }
                        for alias in renames(&content, name) {
                            if !needles.iter().any(|x| x.text == alias) && alias.len() >= 2 {
                                renamed.push(format!("{name} as {alias}"));
                                needles.push(self.needle(&alias));
                            }
                        }
                    }
                }
                queue.push_back(n);
            }
        }
        hits.sort_unstable();
        (hits, renamed)
    }

    /// Imports written as project paths that resolve to no file, in `ids`.
    fn local_unresolved(&self, ids: impl Iterator<Item = u32>) -> usize {
        let ids: FxHashSet<u32> = ids.collect();
        self.local_unresolved_where(|id| ids.contains(&id))
    }

    /// Imports written as project paths that resolve to no file, in the
    /// files `keep` accepts. Reads the parked imports in place (no copy).
    pub(super) fn local_unresolved_where(&self, keep: impl Fn(u32) -> bool) -> usize {
        // Only the parked imports written as project paths are read; the
        // slot is checked again, as it may have been resolved since.
        let waiting = self
            .waiting_local
            .iter()
            .filter_map(|&i| self.waiting_imports.get(i)?.as_ref())
            .filter(|(id, _, spec)| keep(*id) && is_local_spec(spec))
            .count();
        let pending: usize = self
            .pending_imports
            .iter()
            .filter(|(id, _, _)| keep(*id))
            .map(|(_, _, specs)| specs.iter().filter(|s| is_local_spec(s)).count())
            .sum();
        waiting + pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_on_import_and_export_lines_only() {
        let src = "from pkg.a import helper as run, other\nexport { MenuListItem as ListItem } from './menu';\nconst x = helper as unknown;\nfrom m import (\n    helper as h2,\n)\n";
        assert_eq!(
            renames(src, "helper"),
            vec!["run".to_string(), "h2".to_string()]
        );
        assert_eq!(renames(src, "MenuListItem"), vec!["ListItem".to_string()]);
        assert!(renames(src, "other").is_empty());
    }

    #[test]
    fn entry_files() {
        for p in [
            "pkg/__init__.py",
            "src/mod.rs",
            "src/lib.rs",
            "lib/index.ts",
            "a/index.jsx",
        ] {
            assert!(is_entry_file(Path::new(p)), "{p}");
        }
        for p in ["pkg/a.py", "index.md", "src/main.py"] {
            assert!(!is_entry_file(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn whole_words_respect_identifier_boundaries() {
        assert_eq!(find_word("x = helper(1)", "helper"), Some(4));
        assert_eq!(find_word("helper", "helper"), Some(0));
        assert_eq!(find_word("helper_unrelated(); helper", "helper"), Some(20));
        assert_eq!(find_word("my_helper()", "helper"), None);
        assert_eq!(find_word("$helper", "helper"), None);
        assert_eq!(find_word("é helper", "helper"), Some(3));
    }

    #[test]
    fn text_classifier_sees_strings_and_comments() {
        let src = "x = helper()\n# helper here\ny = \"pkg.helper\"\n";
        let c = TextClassifier::new("a.py", src);
        assert!(!c.is_text(0, 4, "x = helper()"));
        assert!(c.is_text(1, 2, "# helper here"));
        assert!(c.is_text(2, 9, "y = \"pkg.helper\""));
        let guess = TextClassifier::new("a.unknown", src);
        assert!(guess.is_text(1, 2, "# helper here"));
        assert!(!guess.is_text(0, 4, "x = helper()"));
        // Strings and comments that span lines.
        let src = "s = \"\"\"\nhelper\n\"\"\"\nhelper()\n";
        let c = TextClassifier::new("a.py", src);
        assert!(c.is_text(1, 0, "helper"));
        assert!(!c.is_text(3, 0, "helper()"));
        let src = "/* a\n helper */ helper();\nconst t = `\n${helper}`;\nhelper('x // y');\n";
        let c = TextClassifier::new("a.ts", src);
        assert!(c.is_text(1, 1, " helper */ helper();"));
        assert!(!c.is_text(1, 11, " helper */ helper();"));
        assert!(c.is_text(3, 2, "${helper}`;"));
        assert!(!c.is_text(4, 0, "helper('x // y');"));
        // A Rust lifetime opens no string.
        let src = "fn f<'a>(x: &'a str) { helper(x) }\n";
        let c = TextClassifier::new("a.rs", src);
        assert!(!c.is_text(0, 23, "fn f<'a>(x: &'a str) { helper(x) }"));
    }

    /// A small Python project: `pkg/a.py` defines `helper` and `Model`;
    /// `pkg/__init__.py` re-exports both.
    fn project() -> (tempfile::TempDir, SearchEngine) {
        let dir = tempfile::TempDir::new().unwrap();
        let files = [
            ("pkg/__init__.py", "from .a import helper, Model\n"),
            (
                "pkg/a.py",
                "def helper(x):\n    return x + 1\n\n\nclass Model:\n    def save(self):\n        return helper(1)\n",
            ),
            // Through the package entry, by name.
            ("app/b.py", "from pkg import helper\n\n\ndef run():\n    return helper(2)\n"),
            // Imports the package but never names helper.
            ("app/c.py", "from pkg import Model\n\n\ndef make():\n    m = Model()\n    m.save()\n"),
            // Only a dotted path in a string.
            ("app/settings.py", "import pkg\nHOOK = \"pkg.helper\"\n"),
            // Renamed on import, then used under the new name elsewhere.
            ("app/alias.py", "from pkg.a import helper as run_it\n\n\ndef go():\n    return run_it(3)\n"),
            ("app/user_of_alias.py", "from app.alias import run_it\nrun_it(4)\n"),
            // Names helper but imports nothing from pkg: not a user.
            ("other/d.py", "def helper():\n    pass\nhelper()\n"),
            // A method call on a value whose type is unknown.
            ("app/e.py", "import pkg.a\n\n\ndef keep(obj):\n    obj.save()\n"),
        ];
        let mut engine = SearchEngine::new();
        engine.add_root_path(dir.path());
        for (rel, body) in files {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
        }
        for (rel, _) in files {
            engine.index_file(dir.path().join(rel)).unwrap();
        }
        engine.resolve_imports();
        engine.finalize();
        (dir, engine)
    }

    fn opts(dir: &tempfile::TempDir) -> ImpactOptions {
        ImpactOptions {
            root: Some(dir.path().to_path_buf()),
            ..Default::default()
        }
    }

    fn users(s: &SymbolImpact) -> Vec<(String, UseTier)> {
        s.users
            .iter()
            .map(|u| (u.path.split_once('/').unwrap().1.to_string(), u.tier))
            .collect()
    }

    const HELPER_DIFF: &str =
        "--- a/pkg/a.py\n+++ b/pkg/a.py\n@@ -2 +2 @@\n-    return x + 1\n+    return x + 2\n";

    #[test]
    fn function_users_through_entry_files_and_renames() {
        let (dir, engine) = project();
        // The index holds the old side; the diff is a local change.
        let r = engine.impact(HELPER_DIFF, &opts(&dir)).unwrap();
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.files[0].symbols, 1);
        let s = &r.symbols[0];
        assert_eq!(
            (s.qualname.as_str(), s.kind, s.change),
            ("helper", DefKind::Function, Change::Modified)
        );
        assert!(!s.signature);
        assert_eq!(s.scope, ImpactScope::Importers);
        let mut got = users(s);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("app/alias.py".into(), UseTier::Reference),
                ("app/b.py".into(), UseTier::Reference),
                ("app/settings.py".into(), UseTier::Text),
                ("app/user_of_alias.py".into(), UseTier::Reference),
                ("pkg/__init__.py".into(), UseTier::Code),
            ]
        );
        assert_eq!(s.names, vec!["helper".to_string(), "run_it".to_string()]);
        assert!(
            s.notes.iter().any(|n| n.contains("helper as run_it")),
            "{:?}",
            s.notes
        );
        // Best tier first.
        assert_eq!(s.users[0].tier, UseTier::Reference);
        assert_eq!(s.tiers.reference, 3);
        assert_eq!(r.total_files, 5);
    }

    #[test]
    fn the_same_diff_works_when_the_index_holds_the_new_side() {
        let (dir, engine) = project();
        // Reverse the diff: the index is now the "new" side.
        let reversed =
            "--- a/pkg/a.py\n+++ b/pkg/a.py\n@@ -2 +2 @@\n-    return x + 2\n+    return x + 1\n";
        let r = engine.impact(reversed, &opts(&dir)).unwrap();
        assert_eq!(r.symbols.len(), 1);
        assert_eq!(r.symbols[0].total_files, 5);
        // A diff matching neither side says so instead of guessing.
        let other = "--- a/pkg/a.py\n+++ b/pkg/a.py\n@@ -2 +2 @@\n-    return 7\n+    return 8\n";
        let r = engine.impact(other, &opts(&dir)).unwrap();
        assert!(r.symbols.is_empty());
        assert!(r.files[0]
            .note
            .as_deref()
            .unwrap()
            .contains("matches neither side"));
    }

    #[test]
    fn methods_are_found_by_name_with_class_named_files_first() {
        let (dir, engine) = project();
        let diff = "--- a/pkg/a.py\n+++ b/pkg/a.py\n@@ -7 +7 @@\n-        return helper(1)\n+        return helper(5)\n";
        let r = engine.impact(diff, &opts(&dir)).unwrap();
        let s = &r.symbols[0];
        assert_eq!(
            (s.qualname.as_str(), s.kind),
            ("Model.save", DefKind::Method)
        );
        assert_eq!(s.scope, ImpactScope::AllImporters);
        let got: Vec<(String, Option<bool>)> = s
            .users
            .iter()
            .map(|u| (u.path.split_once('/').unwrap().1.to_string(), u.names_owner))
            .collect();
        assert_eq!(
            got,
            vec![
                ("app/c.py".into(), Some(true)),
                ("app/e.py".into(), Some(false))
            ]
        );
        assert!(s.notes.iter().any(|n| n.contains("member")));
    }

    #[test]
    fn added_removed_and_unindexed_files() {
        let (dir, engine) = project();
        let diff = "--- /dev/null\n+++ b/pkg/new.py\n@@ -0,0 +1,2 @@\n+def fresh():\n+    return 1\n--- a/pkg/a.py\n+++ b/pkg/a.py\n@@ -1,3 +0,0 @@\n-def helper(x):\n-    return x + 1\n-\n";
        let r = engine.impact(diff, &opts(&dir)).unwrap();
        assert_eq!(r.files[0].status, "added");
        let fresh = r.symbols.iter().find(|s| s.name == "fresh").unwrap();
        assert_eq!(fresh.change, Change::Added);
        assert!(fresh.notes[0].contains("not in the index"));
        let helper = r.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(helper.change, Change::Removed);
        assert!(helper.signature);
        // Its users still reference it: they break.
        assert!(helper.total_files >= 4);
        assert!(engine.impact("nothing here", &opts(&dir)).is_err());
    }

    #[test]
    fn languages_without_a_graph_search_the_whole_index() {
        let dir = tempfile::TempDir::new().unwrap();
        let files = [
            (
                "lib.go",
                "package lib\n\nfunc Helper() int {\n\treturn 1\n}\n",
            ),
            (
                "main.go",
                "package main\n\nfunc main() {\n\tlib.Helper()\n}\n",
            ),
        ];
        let mut engine = SearchEngine::new();
        engine.add_root_path(dir.path());
        for (rel, body) in files {
            std::fs::write(dir.path().join(rel), body).unwrap();
            engine.index_file(dir.path().join(rel)).unwrap();
        }
        engine.finalize();
        let diff = "--- a/lib.go\n+++ b/lib.go\n@@ -4 +4 @@\n-\treturn 1\n+\treturn 2\n";
        let r = engine.impact(diff, &opts(&dir)).unwrap();
        let s = &r.symbols[0];
        assert_eq!(s.name, "Helper");
        assert_eq!(s.scope, ImpactScope::Repository);
        assert_eq!(users(s), vec![("main.go".into(), UseTier::Reference)]);
        assert!(s.notes.iter().any(|n| n.contains("no import graph")));
        assert!(s.notes.iter().any(|n| n.contains("approximated")));
    }

    #[test]
    fn listing_caps_are_reported() {
        let (dir, engine) = project();
        let o = ImpactOptions {
            max_files: 2,
            max_lines: 1,
            ..opts(&dir)
        };
        let s = &engine.impact(HELPER_DIFF, &o).unwrap().symbols[0];
        assert_eq!(s.users.len(), 2);
        assert_eq!(s.total_files, 5);
        assert!(s.notes.iter().any(|n| n.contains("listing 2 of 5")));
    }
}
