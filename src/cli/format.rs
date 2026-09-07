//! Rendering of search results for the terminal: grouped (human), vimgrep
//! (`path:line:col:text`, what editors' quickfix lists read) and JSON.

use crate::web::{SearchResponse, SearchResultJson};
use std::fmt::Write as _;

/// Output layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// File headings with `line:col: text` underneath (default on a terminal).
    Grouped,
    /// One `path:line:col:text` line per hit (default when piped).
    Vimgrep,
    /// The server's JSON response, one object.
    Json,
}

/// ANSI styling, decided once from `--color` and whether stdout is a TTY.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub color: bool,
}

impl Style {
    fn bold(&self, s: &str) -> String {
        if self.color {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn path(&self, s: &str) -> String {
        if self.color {
            format!("\x1b[1;35m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn dim(&self, s: &str) -> String {
        if self.color {
            format!("\x1b[2m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn hit(&self, s: &str) -> String {
        if self.color {
            format!("\x1b[1;31m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

/// `content` with the matched range emphasised. Offsets are byte offsets
/// into `content`; anything that is not a char boundary is left unstyled.
pub fn highlight(content: &str, start: usize, end: usize, style: &Style) -> String {
    if !style.color
        || start >= end
        || end > content.len()
        || !content.is_char_boundary(start)
        || !content.is_char_boundary(end)
    {
        return content.to_string();
    }
    format!(
        "{}{}{}",
        &content[..start],
        style.hit(&content[start..end]),
        &content[end..]
    )
}

/// Render `resp` as `format`. `path_of` maps the server's display path to
/// what should be printed (e.g. an absolute path).
pub fn render(
    resp: &SearchResponse,
    format: Format,
    style: &Style,
    files_only: bool,
    path_of: &dyn Fn(&str) -> String,
) -> String {
    match format {
        Format::Json => {
            let mut s = serde_json::to_string(resp).unwrap_or_default();
            s.push('\n');
            s
        }
        _ if files_only => {
            let mut out = String::new();
            let mut seen = std::collections::HashSet::new();
            for r in &resp.results {
                if seen.insert(r.file_path.as_str()) {
                    let _ = writeln!(out, "{}", path_of(&r.file_path));
                }
            }
            out
        }
        Format::Vimgrep => render_vimgrep(resp, style, path_of),
        Format::Grouped => render_grouped(resp, style, path_of),
    }
}

fn render_vimgrep(
    resp: &SearchResponse,
    style: &Style,
    path_of: &dyn Fn(&str) -> String,
) -> String {
    let mut out = String::new();
    for r in &resp.results {
        let path = path_of(&r.file_path);
        if let (Some(ctx), Some(start)) = (&r.context_lines, r.context_start_line) {
            for (i, line) in ctx.iter().enumerate() {
                let n = start + i;
                if n == r.line_number {
                    let _ = writeln!(
                        out,
                        "{}:{}:{}:{}",
                        path,
                        n,
                        r.match_column + 1,
                        highlight(&r.content, r.match_start, r.match_end, style)
                    );
                } else {
                    let _ = writeln!(out, "{}-{}-{}", path, n, line);
                }
            }
            let _ = writeln!(out, "--");
        } else {
            let _ = writeln!(
                out,
                "{}:{}:{}:{}",
                path,
                r.line_number,
                r.match_column + 1,
                highlight(&r.content, r.match_start, r.match_end, style)
            );
        }
    }
    out
}

fn render_grouped(
    resp: &SearchResponse,
    style: &Style,
    path_of: &dyn Fn(&str) -> String,
) -> String {
    let mut out = String::new();
    let mut current: Option<&str> = None;
    for r in &resp.results {
        if current != Some(r.file_path.as_str()) {
            if current.is_some() {
                out.push('\n');
            }
            let _ = writeln!(out, "{}", style.path(&path_of(&r.file_path)));
            current = Some(r.file_path.as_str());
        }
        let kind = match r.match_type.as_ref() {
            "SYMBOL_DEFINITION" => " [def]",
            "SYMBOL_REFERENCE" => " [ref]",
            _ => "",
        };
        if let (Some(ctx), Some(start)) = (&r.context_lines, r.context_start_line) {
            for (i, line) in ctx.iter().enumerate() {
                let n = start + i;
                if n == r.line_number {
                    let _ = writeln!(
                        out,
                        "{}{}: {}",
                        style.bold(&format!("{:>5}", n)),
                        style.dim(kind),
                        highlight(&r.content, r.match_start, r.match_end, style)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "{}  {}",
                        style.dim(&format!("{:>5}", n)),
                        style.dim(line)
                    );
                }
            }
            let _ = writeln!(out, "{}", style.dim("     --"));
        } else if r.line_number == 0 {
            let _ = writeln!(out, "{}", style.dim("     (filename match)"));
        } else {
            let _ = writeln!(
                out,
                "{}{}: {}",
                style.bold(&format!("{:>5}", r.line_number)),
                style.dim(kind),
                highlight(&r.content, r.match_start, r.match_end, style)
            );
        }
    }
    out
}

/// One-line summary for stderr (grouped output on a terminal).
pub fn summary(resp: &SearchResponse) -> String {
    let shown = resp.results.len();
    let files = resp
        .results
        .iter()
        .map(|r| r.file_path.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let more = if resp.has_more { "+" } else { "" };
    let total = match resp.total_matches {
        Some(t) if t > shown => format!(" of {t}"),
        _ => String::new(),
    };
    format!(
        "{shown}{more} matches{total} in {files} file{} ({:.1} ms{})",
        if files == 1 { "" } else { "s" },
        resp.elapsed_ms,
        resp.rank_mode
            .as_deref()
            .map(|m| format!(", {m} ranking"))
            .unwrap_or_default()
    )
}

/// Build a response from hits, for tests and offline searches.
pub fn response_from(query: &str, results: Vec<SearchResultJson>) -> SearchResponse {
    let n = results.len();
    SearchResponse {
        results,
        query: query.to_string(),
        total_results: n,
        has_more: false,
        offset: 0,
        total_matches: Some(n),
        truncated_by_budget: false,
        elapsed_ms: 0.0,
        rank_mode: None,
        total_candidates: None,
        candidates_searched: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, line: usize, content: &str, s: usize, e: usize) -> SearchResultJson {
        SearchResultJson {
            file_path: path.to_string(),
            content: content.to_string(),
            line_number: line,
            match_start: s,
            match_end: e,
            content_truncated: false,
            line_match_start: s,
            line_match_end: e,
            match_column: content[..s].chars().count(),
            score: 1.0,
            match_type: "TEXT".into(),
            dependency_count: 0,
            context_lines: None,
            context_start_line: None,
        }
    }

    #[test]
    fn vimgrep_lines_are_path_line_col_text() {
        let resp = response_from(
            "main",
            vec![
                hit("proj/src/a.rs", 3, "fn main() {}", 3, 7),
                hit("proj/src/b.rs", 10, "  main();", 2, 6),
            ],
        );
        let plain = Style { color: false };
        let out = render(&resp, Format::Vimgrep, &plain, false, &|p| p.to_string());
        assert_eq!(
            out,
            "proj/src/a.rs:3:4:fn main() {}\nproj/src/b.rs:10:3:  main();\n"
        );
    }

    #[test]
    fn grouped_output_groups_by_file_and_highlights() {
        let resp = response_from(
            "main",
            vec![
                hit("a.rs", 3, "fn main() {}", 3, 7),
                hit("a.rs", 9, "main()", 0, 4),
                hit("b.rs", 1, "x", 0, 1),
            ],
        );
        let plain = Style { color: false };
        let out = render(&resp, Format::Grouped, &plain, false, &|p| p.to_string());
        assert_eq!(
            out,
            "a.rs\n    3: fn main() {}\n    9: main()\n\nb.rs\n    1: x\n"
        );
        let color = Style { color: true };
        let out = render(&resp, Format::Grouped, &color, false, &|p| p.to_string());
        assert!(out.contains("fn \x1b[1;31mmain\x1b[0m() {}"), "{out:?}");
    }

    #[test]
    fn files_only_lists_each_file_once_in_order() {
        let resp = response_from(
            "x",
            vec![
                hit("b.rs", 1, "x", 0, 1),
                hit("a.rs", 1, "x", 0, 1),
                hit("b.rs", 2, "x", 0, 1),
            ],
        );
        let plain = Style { color: false };
        let out = render(&resp, Format::Grouped, &plain, true, &|p| {
            format!("/root/{p}")
        });
        assert_eq!(out, "/root/b.rs\n/root/a.rs\n");
    }

    #[test]
    fn highlight_never_splits_a_character() {
        let color = Style { color: true };
        // "é" is two bytes; an offset inside it is left unstyled.
        assert_eq!(highlight("héllo", 1, 2, &color), "héllo");
        assert_eq!(highlight("héllo", 1, 3, &color), "h\x1b[1;31mé\x1b[0mllo");
        assert_eq!(highlight("abc", 5, 9, &color), "abc");
    }

    #[test]
    fn json_output_round_trips() {
        let resp = response_from("q", vec![hit("a.rs", 1, "q", 0, 1)]);
        let plain = Style { color: false };
        let out = render(&resp, Format::Json, &plain, false, &|p| p.to_string());
        let back: SearchResponse = serde_json::from_str(&out).unwrap();
        assert_eq!(back.results[0].file_path, "a.rs");
    }

    #[test]
    fn context_lines_render_around_the_hit() {
        let mut h = hit("a.rs", 2, "fn main() {}", 3, 7);
        h.context_lines = Some(vec!["// before".into(), "fn main() {}".into(), "}".into()]);
        h.context_start_line = Some(1);
        let resp = response_from("main", vec![h]);
        let plain = Style { color: false };
        let out = render(&resp, Format::Vimgrep, &plain, false, &|p| p.to_string());
        assert_eq!(
            out,
            "a.rs-1-// before\na.rs:2:4:fn main() {}\na.rs-3-}\n--\n"
        );
    }
}
