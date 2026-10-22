//! Unified diffs (`git diff`, `diff -u`): parse them, and rebuild the side
//! of a file the index does not hold from the side it does.

/// One file of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path before the change (`None` for a new file).
    pub old_path: Option<String>,
    /// Path after the change (`None` for a deleted file).
    pub new_path: Option<String>,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// The path the change is reported under: the new one, else the old.
    pub fn path(&self) -> &str {
        self.new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .unwrap_or_default()
    }

    /// 0-based lines of the old side that the diff removes or replaces. A
    /// pure insertion counts the line it lands after, so the definition it
    /// lands in is still seen as changed.
    pub fn old_rows(&self) -> Vec<usize> {
        self.rows(false)
    }

    /// 0-based lines of the new side that the diff adds or replaces (a
    /// pure deletion counts the line before it).
    pub fn new_rows(&self) -> Vec<usize> {
        self.rows(true)
    }

    fn rows(&self, new: bool) -> Vec<usize> {
        let mut out = Vec::new();
        for h in &self.hunks {
            let start = if new { h.new_start } else { h.old_start };
            let mine = |l: &Line| match l {
                Line::Context(_) => true,
                Line::Added(_) => new,
                Line::Removed(_) => !new,
            };
            // A side with no lines names the line before the change.
            let mut row = if h.lines.iter().any(mine) {
                start.saturating_sub(1)
            } else {
                start
            };
            // Lines changed on the other side only, not yet placed here.
            let mut pending = false;
            for l in &h.lines {
                match l {
                    Line::Context(_) => {
                        if std::mem::take(&mut pending) {
                            out.push(row.saturating_sub(1));
                        }
                        row += 1;
                    }
                    l if mine(l) => {
                        pending = false;
                        out.push(row);
                        row += 1;
                    }
                    _ => pending = true,
                }
            }
            if pending {
                out.push(row.saturating_sub(1));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The new file's text when the diff creates it, the old file's when it
    /// deletes it: every line is in the diff.
    pub fn whole_side(&self, new: bool) -> String {
        let mut s = String::new();
        for h in &self.hunks {
            for l in &h.lines {
                match (l, new) {
                    (Line::Added(t), true) | (Line::Removed(t), false) | (Line::Context(t), _) => {
                        s.push_str(t);
                        s.push('\n');
                    }
                    _ => {}
                }
            }
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// 1-based first line on each side (0 for an empty side).
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Context(String),
    Removed(String),
    Added(String),
}

/// Strip git's `a/` / `b/` prefix; `/dev/null` is no file. Trailing tab
/// and timestamp (`diff -u`) are dropped.
fn header_path(raw: &str, prefix: &str) -> Option<String> {
    let raw = raw.split('\t').next().unwrap_or(raw).trim_end();
    if raw == "/dev/null" {
        return None;
    }
    let raw = raw.trim_matches('"');
    Some(raw.strip_prefix(prefix).unwrap_or(raw).to_string())
}

fn range(s: &str) -> Option<(usize, usize)> {
    let s = &s[1..];
    let mut it = s.split(',');
    let start = it.next()?.parse().ok()?;
    let count = match it.next() {
        Some(c) => c.parse().ok()?,
        None => 1,
    };
    Some((start, count))
}

/// Parse a unified diff. Anything that is not part of a file's header or
/// hunks (commit messages, `diff --git`, `index` lines) is skipped.
pub fn parse(text: &str) -> Result<Vec<FileDiff>, String> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut lines = text.lines().peekable();
    let mut n = 0usize;
    while let Some(line) = lines.next() {
        n += 1;
        let Some(old) = line.strip_prefix("--- ") else {
            continue;
        };
        let Some(new) = lines.peek().and_then(|l| l.strip_prefix("+++ ")) else {
            continue;
        };
        let new = new.to_string();
        lines.next();
        n += 1;
        let mut file = FileDiff {
            old_path: header_path(old, "a/"),
            new_path: header_path(&new, "b/"),
            hunks: Vec::new(),
        };
        while let Some(h) = lines.peek().and_then(|l| l.strip_prefix("@@ ")) {
            let header = h.to_string();
            lines.next();
            n += 1;
            let mut parts = header.split_whitespace();
            let (Some((old_start, old_count)), Some((new_start, new_count))) =
                (parts.next().and_then(range), parts.next().and_then(range))
            else {
                return Err(format!("line {n}: malformed hunk header @@ {header}"));
            };
            let (mut old_left, mut new_left) = (old_count, new_count);
            let mut hunk = Hunk {
                old_start,
                new_start,
                lines: Vec::new(),
            };
            while old_left > 0 || new_left > 0 {
                let Some(l) = lines.next() else {
                    return Err(format!("line {n}: diff ends inside a hunk"));
                };
                n += 1;
                if l.starts_with('\\') {
                    continue; // "\ No newline at end of file"
                }
                let (tag, body) = l.split_at(l.len().min(1));
                let body = body.strip_suffix('\r').unwrap_or(body).to_string();
                match tag {
                    "+" => {
                        new_left = new_left.saturating_sub(1);
                        hunk.lines.push(Line::Added(body));
                    }
                    "-" => {
                        old_left = old_left.saturating_sub(1);
                        hunk.lines.push(Line::Removed(body));
                    }
                    // An empty context line may lose its leading space.
                    " " | "" => {
                        old_left = old_left.saturating_sub(1);
                        new_left = new_left.saturating_sub(1);
                        hunk.lines.push(Line::Context(body));
                    }
                    _ => return Err(format!("line {n}: unexpected line in hunk: {l}")),
                }
            }
            while lines.peek().is_some_and(|l| l.starts_with('\\')) {
                lines.next();
                n += 1;
            }
            file.hunks.push(hunk);
        }
        files.push(file);
    }
    Ok(files)
}

fn split_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// Lines `apply` must find in place (context plus removed lines going
/// forward, context plus added lines going back). A side with none, as in
/// a pure insertion, applies anywhere, so it says little about which side
/// a text is.
pub fn anchored_lines(file: &FileDiff, forward: bool) -> usize {
    file.hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| match l {
            Line::Context(_) => true,
            Line::Removed(_) => forward,
            Line::Added(_) => !forward,
        })
        .count()
}

/// Apply `file`'s hunks to `text` (`forward`: old side to new side;
/// otherwise new to old). `None` when `text` is not the side the hunks
/// start from.
pub fn apply(file: &FileDiff, text: &str, forward: bool) -> Option<String> {
    let src = split_lines(text);
    let mut out: Vec<&str> = Vec::with_capacity(src.len());
    let mut at = 0usize;
    for h in &file.hunks {
        let (start, from_side): (usize, Vec<&str>) = if forward {
            (
                h.old_start,
                h.lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Context(t) | Line::Removed(t) => Some(t.as_str()),
                        Line::Added(_) => None,
                    })
                    .collect(),
            )
        } else {
            (
                h.new_start,
                h.lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Context(t) | Line::Added(t) => Some(t.as_str()),
                        Line::Removed(_) => None,
                    })
                    .collect(),
            )
        };
        // An empty side's start is the line before the insertion.
        let begin = if from_side.is_empty() {
            start
        } else {
            start.checked_sub(1)?
        };
        if begin < at || begin + from_side.len() > src.len() {
            return None;
        }
        if src[begin..begin + from_side.len()] != from_side[..] {
            return None;
        }
        out.extend_from_slice(&src[at..begin]);
        for l in &h.lines {
            match (l, forward) {
                (Line::Context(t), _) | (Line::Added(t), true) | (Line::Removed(t), false) => {
                    out.push(t.as_str())
                }
                _ => {}
            }
        }
        at = begin + from_side.len();
    }
    out.extend_from_slice(&src[at..]);
    let mut s = out.join("\n");
    if text.ends_with('\n') || !out.is_empty() {
        s.push('\n');
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/pkg/a.py b/pkg/a.py
index 1111111..2222222 100644
--- a/pkg/a.py
+++ b/pkg/a.py
@@ -1,4 +1,4 @@
 def helper(x):
-    return x + 1
+    return x + 2


@@ -8,2 +8,3 @@ def other():
     pass
+    pass
 # end
--- /dev/null
+++ b/pkg/new.py
@@ -0,0 +1,2 @@
+def fresh():
+    return 1
";

    const OLD: &str =
        "def helper(x):\n    return x + 1\n\n\ndef other():\n    '''doc'''\n\n    pass\n# end\n";

    #[test]
    fn parses_files_hunks_and_rows() {
        let files = parse(DIFF).unwrap();
        assert_eq!(files.len(), 2);
        let a = &files[0];
        assert_eq!(a.old_path.as_deref(), Some("pkg/a.py"));
        assert_eq!(a.path(), "pkg/a.py");
        assert_eq!(a.hunks.len(), 2);
        assert_eq!(a.old_rows(), vec![1, 7]);
        assert_eq!(a.new_rows(), vec![1, 8]);
        let n = &files[1];
        assert_eq!(n.old_path, None);
        assert_eq!(n.path(), "pkg/new.py");
        assert_eq!(n.whole_side(true), "def fresh():\n    return 1\n");
        assert_eq!(n.new_rows(), vec![0, 1]);
    }

    #[test]
    fn rebuilds_either_side() {
        let files = parse(DIFF).unwrap();
        let new = apply(&files[0], OLD, true).expect("old side applies forward");
        assert!(new.contains("x + 2") && new.contains("    pass\n    pass\n# end"));
        assert_eq!(apply(&files[0], &new, false).as_deref(), Some(OLD));
        // The new side does not apply forward again: the old lines are gone.
        assert_eq!(apply(&files[0], &new, true), None);
        // An insertion into an empty file.
        assert_eq!(
            apply(&files[1], "", true).as_deref(),
            Some("def fresh():\n    return 1\n")
        );
    }

    #[test]
    fn zero_context_diffs_and_malformed_input() {
        let d = "--- a/x.rs\n+++ b/x.rs\n@@ -2 +2 @@\n-b\n+B\n@@ -4,0 +5 @@\n+e\n";
        let f = &parse(d).unwrap()[0];
        assert_eq!(
            apply(f, "a\nb\nc\nd\n", true).as_deref(),
            Some("a\nB\nc\nd\ne\n")
        );
        assert_eq!(f.new_rows(), vec![1, 4]);
        assert_eq!(f.old_rows(), vec![1, 3]);
        assert!(parse("--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n").is_err());
        assert!(parse("not a diff").unwrap().is_empty());
    }
}
