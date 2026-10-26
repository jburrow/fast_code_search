//! Graph-scoped search (`deps:` / `rdeps:`): resolve each filter's anchor
//! against the index, walk the import graph from it, and keep the files
//! reached as a bitmap the candidates are intersected with.
//!
//! The scope only decides which files are searched; matching inside them is
//! unchanged, so a scoped search finds what `rg` finds over the same files.

use super::SearchEngine;
use crate::dependencies::graph::Direction;
use crate::dependencies::scope::has_import_graph;
use crate::search::query_syntax::{GraphDirection, GraphFilter};
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use std::path::Path;

/// Most hops `:N` may ask for (`:*` has no limit).
pub const MAX_SCOPE_HOPS: u32 = 10;

/// The files a search is limited to by its graph filters.
#[derive(Debug, Clone, Default)]
pub struct GraphScope {
    /// Files the positive filters reached (`None` when there are only
    /// negated ones, which leave everything else in).
    include: Option<RoaringBitmap>,
    /// Files the negated filters reached.
    exclude: RoaringBitmap,
    /// Hops from the nearest anchor, for files in `include`.
    hops: FxHashMap<u32, u32>,
    /// The filters, as typed (normalized).
    pub filters: Vec<String>,
    /// Imports written as project paths (`./x`, `crate::x`) that resolve to
    /// no file, in files of the scope: edges the scope may be missing.
    pub unresolved_local: usize,
}

impl GraphScope {
    /// Whether file `id` is in the scope.
    pub fn allows(&self, id: u32) -> bool {
        self.include.as_ref().is_none_or(|i| i.contains(id)) && !self.exclude.contains(id)
    }

    /// `candidates` limited to the scope.
    pub fn restrict(&self, candidates: &RoaringBitmap) -> RoaringBitmap {
        let mut out = match &self.include {
            Some(include) => candidates & include,
            None => candidates.clone(),
        };
        out -= &self.exclude;
        out
    }

    /// Hops from the nearest anchor (0 = an anchor or its package entry),
    /// or `None` when the scope has no positive filter.
    pub fn hops(&self, id: u32) -> Option<u32> {
        self.hops.get(&id).copied()
    }

    /// Score factor for file `id`: nearer files rank a little higher.
    pub fn boost(&self, id: u32) -> f64 {
        self.hops(id)
            .map_or(1.0, |h| 1.0 / (1.0 + 0.15 * f64::from(h)))
    }

    /// Files in the scope, when it has a positive filter.
    pub fn file_count(&self) -> Option<usize> {
        self.include
            .as_ref()
            .map(|i| (i.len() - i.intersection_len(&self.exclude)) as usize)
    }
}

impl SearchEngine {
    /// `limits` with the scope of the query's own `deps:` / `rdeps:`, unless
    /// the caller (the shared dispatch) already set one. Every search that
    /// takes a parsed query honours its graph filters this way.
    pub(super) fn limits_with_query_scope(
        &self,
        parsed: &crate::search::query_syntax::ParsedQuery,
        limits: super::SearchLimits,
    ) -> anyhow::Result<super::SearchLimits> {
        if parsed.graph.is_empty() || limits.scope.is_some() {
            return Ok(limits);
        }
        let scope = self
            .graph_scope(&parsed.graph)
            .map_err(anyhow::Error::msg)?
            .map(std::sync::Arc::new);
        Ok(limits.with_scope(scope))
    }

    /// The scope for `filters`, or `None` when there are none. An anchor
    /// that matches no file, several files, or a file in a language without
    /// import edges is an error, never a silently empty or one-file scope.
    pub fn graph_scope(&self, filters: &[GraphFilter]) -> Result<Option<GraphScope>, String> {
        if filters.is_empty() {
            return Ok(None);
        }
        let mut scope = GraphScope::default();
        // Same kind: union. Different kinds: intersection.
        let mut by_kind: FxHashMap<GraphDirection, FxHashMap<u32, u32>> = FxHashMap::default();
        for f in filters {
            let (anchors, max_hops, shown) = self.resolve_anchor(f)?;
            let direction = match f.direction {
                GraphDirection::Deps => Direction::Imports,
                GraphDirection::Rdeps => Direction::ImportedBy,
            };
            // A negated filter excludes, so it must not over-reach: no
            // pass-through across package entry files.
            let reached = self.dependency_index.scope_walk(
                &anchors,
                direction,
                max_hops,
                !f.negate,
                &|user, definer| self.names_something_defined_in(user, definer),
            );
            let hops_text = max_hops.map_or("*".to_string(), |h| h.to_string());
            let dash = if f.negate { "-" } else { "" };
            scope
                .filters
                .push(format!("{dash}{}:{shown}:{hops_text}", f.direction.name()));
            if f.negate {
                scope.exclude.extend(reached.keys().copied());
            } else {
                let set = by_kind.entry(f.direction).or_default();
                for (id, h) in reached {
                    let e = set.entry(id).or_insert(h);
                    *e = (*e).min(h);
                }
            }
        }
        let mut kinds = by_kind.into_values();
        if let Some(first) = kinds.next() {
            let mut hops = first;
            for other in kinds {
                hops.retain(|id, h| match other.get(id) {
                    Some(&o) => {
                        *h = (*h).min(o);
                        true
                    }
                    None => false,
                });
            }
            scope.include = Some(hops.keys().copied().collect());
            scope.hops = hops;
        }
        if scope.include.is_some() {
            scope.unresolved_local = self.local_unresolved_where(|id| scope.allows(id));
        }
        Ok(Some(scope))
    }

    /// Whether file `user` names something file `definer` defines: one of
    /// its top-level kinds of definition (functions, types, constants,
    /// modules, macros; not methods or fields, whose names are too common)
    /// as a whole word, or the module itself in a path (`ranking::`,
    /// `.ranking`, `/ranking`). A definer with nothing to check counts as
    /// named, so the scope stays on the safe side.
    fn names_something_defined_in(&self, user: u32, definer: u32) -> bool {
        use crate::symbols::SymbolType as T;
        let names: Vec<&str> = self
            .symbol_cache
            .get(definer as usize)
            .into_iter()
            .flatten()
            .filter(|s| {
                s.is_definition
                    && !matches!(
                        s.symbol_type,
                        T::FileName | T::Method | T::Field | T::Property | T::Variable
                    )
            })
            .map(|s| s.name.as_str())
            .filter(|n| n.len() >= 2)
            .collect();
        let stem = self
            .file_store
            .get(definer)
            .and_then(|f| f.path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .filter(|s| !matches!(s.as_str(), "mod" | "lib" | "main" | "__init__" | "index"));
        if names.is_empty() && stem.is_none() {
            return true;
        }
        let Some(content) = self.file_store.get(user).and_then(|f| f.as_str().ok()) else {
            return true;
        };
        let bytes = content.as_bytes();
        let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        // Every whole-word occurrence of `name`, as (start, end).
        let occurrences = |name: &'_ str| {
            memchr::memmem::find_iter(bytes, name.as_bytes())
                .map(move |at| (at, at + name.len()))
                .filter(move |&(at, end)| {
                    (at == 0 || !word(bytes[at - 1])) && bytes.get(end).is_none_or(|&b| !word(b))
                })
                .collect::<Vec<_>>()
        };
        if names.iter().any(|n| !occurrences(n).is_empty()) {
            return true;
        }
        stem.is_some_and(|stem| {
            occurrences(&stem).into_iter().any(|(at, end)| {
                let before = &bytes[..at];
                let after = &bytes[end..];
                // `from .ranking`, `pkg.ranking`, `./ranking`,
                // `super::ranking`, `ranking::Weight`.
                before.ends_with(b".")
                    || before.ends_with(b"/")
                    || before.ends_with(b"::")
                    || after.starts_with(b"::")
            })
        })
    }

    /// The anchor files of `f`, its hop limit, and the anchor as shown.
    fn resolve_anchor(&self, f: &GraphFilter) -> Result<(Vec<u32>, Option<u32>, String), String> {
        let op = format!("{}{}:", if f.negate { "-" } else { "" }, f.direction.name());
        let arg = f.arg.replace('\\', "/");
        let arg = arg.trim_start_matches("./");
        // A path as results print it (with or without `:N`) is found by
        // lookup; only a partial path or a folder reads every indexed path.
        if let Some(found) = self.anchor_files(arg, &op, false)? {
            return Ok((found, Some(1), arg.to_string()));
        }
        if let Some((path, hops)) = arg.rsplit_once(':') {
            let max_hops = match hops {
                "*" => Some(None),
                n => n
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=MAX_SCOPE_HOPS).contains(n))
                    .map(Some),
            };
            if let Some(max_hops) = max_hops {
                if let Some(found) = self.anchor_files(path, &op, false)? {
                    return Ok((found, max_hops, path.to_string()));
                }
            }
        }
        if let Some(found) = self.anchor_files(arg, &op, true)? {
            return Ok((found, Some(1), arg.to_string()));
        }
        // Not a file as written: try a trailing `:N` / `:*`.
        if let Some((path, hops)) = arg.rsplit_once(':') {
            let max_hops = match hops {
                "*" => None,
                n => match n.parse::<u32>() {
                    Ok(n) if (1..=MAX_SCOPE_HOPS).contains(&n) => Some(n),
                    Ok(_) => {
                        return Err(format!(
                            "{op}{arg}: hops must be 1 to {MAX_SCOPE_HOPS}, or * for any distance"
                        ))
                    }
                    Err(_) => return Err(format!("{op}{arg}: no indexed file matches {arg}")),
                },
            };
            if let Some(found) = self.anchor_files(path, &op, true)? {
                return Ok((found, max_hops, path.to_string()));
            }
            return Err(format!("{op}{arg}: no indexed file matches {path}"));
        }
        Err(format!("{op}{arg}: no indexed file matches {arg}"))
    }

    /// Files `path` names: a folder (trailing `/`) is every file under it;
    /// a file is the exact path, else the one file whose path ends with it.
    /// `Ok(None)` when nothing matches. Without `scan`, only an exact
    /// lookup of a display or absolute path is tried.
    fn anchor_files(&self, path: &str, op: &str, scan: bool) -> Result<Option<Vec<u32>>, String> {
        if path.is_empty() || path == "/" {
            return Ok(None);
        }
        let folder = path.ends_with('/');
        let suffix = format!("/{path}");
        let mut exact = Vec::new();
        let mut partial = Vec::new();
        // A path as results print it, or an absolute one, is found without
        // reading every path in the index (milliseconds at a million files).
        if !folder {
            let found = if Path::new(path).is_absolute() {
                self.find_file_id_exact(Path::new(path))
            } else {
                self.find_by_display_path(path)
            };
            if let (Some(id), Some(file)) = (found, found.and_then(|id| self.file_store.get(id))) {
                let shown = self
                    .display_path(id)
                    .map(|p| p.into_owned())
                    .unwrap_or_default();
                exact.push((id, shown, has_import_graph(&file.path)));
            }
        }
        let upto = if scan && exact.is_empty() {
            self.file_store.len() as u32
        } else {
            0
        };
        for id in 0..upto {
            let Some(file) = self.file_store.get(id) else {
                continue;
            };
            let (qualified, start) = match self.file_metadata.get(id as usize) {
                Some(m) if m.is_computed() => {
                    (m.qualified_path().to_string(), m.display_start as usize)
                }
                _ => self.qualified_display_path(&file.path),
            };
            let shown = &qualified[start.min(qualified.len())..];
            // The path below the root folder, whether or not it is shown.
            let relative = qualified.split_once('/').map_or("", |(_, r)| r);
            let absolute = file.path.to_string_lossy().replace('\\', "/");
            let candidates = [qualified.as_str(), shown, relative, absolute.as_str()];
            if folder {
                if candidates.iter().any(|c| c.starts_with(path)) || qualified.contains(&suffix) {
                    partial.push((id, shown.to_string(), has_import_graph(&file.path)));
                }
            } else if candidates.contains(&path) {
                exact.push((id, shown.to_string(), has_import_graph(&file.path)));
            } else if qualified.ends_with(&suffix) {
                partial.push((id, shown.to_string(), has_import_graph(&file.path)));
            }
        }
        let matched = if exact.is_empty() { partial } else { exact };
        if matched.is_empty() {
            return Ok(None);
        }
        if folder {
            let ids: Vec<u32> = matched.iter().filter(|m| m.2).map(|m| m.0).collect();
            if ids.is_empty() {
                return Err(format!(
                    "{op}{path}: no file under it has import edges \
                     (the graph covers Rust, Python, JavaScript and TypeScript)"
                ));
            }
            return Ok(Some(ids));
        }
        if matched.len() > 1 {
            let mut names: Vec<&str> = matched.iter().map(|m| m.1.as_str()).collect();
            names.sort_unstable();
            let more = names.len().saturating_sub(5);
            names.truncate(5);
            let tail = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            return Err(format!(
                "{op}{path} matches {} files: {}{tail}. Give more of the path.",
                matched.len(),
                names.join(", ")
            ));
        }
        let (id, shown, graph) = &matched[0];
        if !graph {
            let ext = shown.rsplit_once('.').map_or("these", |(_, e)| e);
            return Err(format!(
                "{op}{shown}: no import graph for .{ext} files \
                 (the graph covers Rust, Python, JavaScript and TypeScript)"
            ));
        }
        Ok(Some(vec![*id]))
    }
}
