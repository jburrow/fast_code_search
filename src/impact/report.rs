//! Text renderings of an [`ImpactReport`]: grouped (the terminal and MCP),
//! vimgrep (`path:line:col:text`, for editors' quickfix lists) and the
//! one-line summary.

use crate::search::engine::{ImpactReport, ImpactScope, SymbolImpact};
use std::fmt::Write as _;

fn paint(color: bool, code: &str, s: &str) -> String {
    if color {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// `3 changed symbols in 2 files; 14 other files use them`.
pub fn summary(r: &ImpactReport) -> String {
    format!(
        "{} in {}; {} use {}",
        count(r.symbols.len(), "changed symbol"),
        count(r.files.len(), "file"),
        count(r.total_files, "other file"),
        if r.symbols.len() == 1 { "it" } else { "them" },
    )
}

/// The notes that say a result is incomplete: report and file notes, and
/// each symbol's caps (`listing N of M`), each once. Piped output, which
/// carries only use lines, prints these on stderr.
pub fn caps(r: &ImpactReport) -> Vec<String> {
    let mut out: Vec<String> = r.notes.clone();
    for f in &r.files {
        if let Some(n) = &f.note {
            out.push(format!("{}: {n}", f.path));
        }
    }
    for s in &r.symbols {
        for n in s
            .notes
            .iter()
            .filter(|n| n.starts_with("listing ") || n.starts_with("not searched"))
        {
            out.push(format!("{}: {n}", s.qualname));
        }
    }
    out.dedup();
    out
}

/// `1 file`, `2 files`.
fn count(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

fn scope_note(s: &SymbolImpact) -> &'static str {
    match s.scope {
        ImpactScope::Importers => "importers that name it",
        ImpactScope::AllImporters => "every transitive importer, by name",
        ImpactScope::Repository => "whole index, by name",
        ImpactScope::None => "not searched",
    }
}

/// The report for a reader: each changed symbol, then the files that use
/// it, best first, with their use lines. `path` maps a display path to what
/// is printed. Every cap and gap is said where it applies.
pub fn grouped(r: &ImpactReport, path: &dyn Fn(&str) -> String, color: bool) -> String {
    let mut out = String::new();
    // Changed files under their indexed path, as their users are shown.
    let shown: std::collections::HashMap<&str, &str> = r
        .files
        .iter()
        .filter_map(|f| f.indexed_path.as_deref().map(|p| (f.path.as_str(), p)))
        .collect();
    let changed = |p: &str| path(shown.get(p).copied().unwrap_or(p));
    for f in &r.files {
        let mut line = format!("{} {}", f.status, changed(&f.path));
        if f.symbols > 0 || f.module_lines > 0 {
            let _ = write!(line, ": {}", count(f.symbols, "changed symbol"));
            if f.module_lines > 0 {
                let _ = write!(
                    line,
                    ", {} changed lines outside any definition (not traced)",
                    f.module_lines
                );
            }
        }
        if let Some(n) = &f.note {
            let _ = write!(line, " ({n})");
        }
        let _ = writeln!(out, "{}", paint(color, "2", &line));
    }
    for n in &r.notes {
        let _ = writeln!(out, "note: {n}");
    }
    for s in &r.symbols {
        let mut head = format!(
            "{}:{}  {} {}  ({}{})",
            changed(&s.file),
            s.line,
            s.kind.as_str(),
            s.qualname,
            s.change.as_str(),
            if s.signature { ", signature" } else { "" }
        );
        if s.names.len() > 1 {
            let _ = write!(head, "  also as {}", s.names[1..].join(", "));
        } else if s.names.first() != Some(&s.name) {
            let _ = write!(head, "  searched as {}", s.names.join(", "));
        }
        let _ = writeln!(out, "\n{}", paint(color, "1", &head));
        let t = &s.tiers;
        let _ = writeln!(
            out,
            "  {} ({} reference, {} code, {} text only); searched {}",
            count(s.total_files, "file"),
            t.reference,
            t.code,
            t.text,
            scope_note(s)
        );
        for n in &s.notes {
            let _ = writeln!(out, "  note: {n}");
        }
        for u in &s.users {
            let mut tags = vec![u.tier.as_str()];
            if u.names_owner == Some(true) {
                tags.push("names class");
            }
            if u.test {
                tags.push("test");
            }
            if u.in_diff {
                tags.push("in diff");
            }
            let _ = writeln!(
                out,
                "  {}  {}",
                paint(color, "1;35", &path(&u.path)),
                paint(color, "2", &format!("[{}]", tags.join(", ")))
            );
            for l in &u.lines {
                let _ = writeln!(
                    out,
                    "    {}: {}",
                    paint(color, "32", &l.line.to_string()),
                    l.text.trim()
                );
            }
            if u.total_lines > u.lines.len() {
                let _ = writeln!(
                    out,
                    "    {}",
                    paint(
                        color,
                        "2",
                        &format!("… {} more lines", u.total_lines - u.lines.len())
                    )
                );
            }
        }
    }
    out
}

/// One `path:line:col:text` line per use, each line once, in report order.
pub fn vimgrep(r: &ImpactReport, path: &dyn Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut seen = std::collections::HashSet::new();
    for s in &r.symbols {
        for u in &s.users {
            for l in &u.lines {
                if seen.insert((u.path.as_str(), l.line)) {
                    let _ = writeln!(out, "{}:{}:{}:{}", path(&u.path), l.line, l.column, l.text);
                }
            }
        }
    }
    out
}
