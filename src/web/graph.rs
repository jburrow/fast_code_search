//! `/api/graph/*`: the import graph for the dependency explorer page.
//!
//! Every endpoint answers from the in-memory [`DependencyIndex`] under the
//! engine read lock. Lists are sorted (most-connected first, then by path)
//! and capped, with counts of what was left out, so a hub imported by half
//! of a large repository still returns a small response. Rust
//! parent-to-child module edges (`mod foo;`) are left out unless
//! `containment=true`: they are structure, not dependencies, and would put
//! every parent and child module in a cycle.

use super::api::{require_file_param, try_read_engine, ApiError, ApiQuery};
use super::WebState;
use crate::dependencies::graph::{is_test_path, Direction};
use crate::dependencies::DependencyIndex;
use crate::search::SearchEngine;
use axum::{extract::State, http::StatusCode, Json};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// Files shown per level before the rest are folded into a `hidden` entry.
const DEFAULT_LEVEL_LIMIT: usize = 12;
const MAX_LEVEL_LIMIT: usize = 500;
/// Test files named in an impact response.
const MAX_TESTS_LISTED: usize = 200;

/// Derived whole-graph views, cached until the engine changes.
#[derive(Debug, Default)]
pub struct GraphCache {
    /// (generation, edge count, containment) the entries were built for.
    key: Option<(u64, usize, bool)>,
    cycles: Arc<FxHashMap<u32, u32>>,
    modules: Option<Arc<ModulesResponse>>,
}

fn cache_key(engine: &SearchEngine, containment: bool) -> (u64, usize, bool) {
    (
        engine.generation(),
        engine.dependency_index.total_edges(),
        containment,
    )
}

/// File ids in an import cycle -> cycle index, computed once per engine state.
fn cycles(state: &WebState, engine: &SearchEngine, containment: bool) -> Arc<FxHashMap<u32, u32>> {
    let key = cache_key(engine, containment);
    let mut cache = state.graph_cache.lock().unwrap_or_else(|e| e.into_inner());
    if cache.key != Some(key) {
        *cache = GraphCache {
            key: Some(key),
            cycles: Arc::new(engine.dependency_index.cycles(containment)),
            modules: None,
        };
    }
    cache.cycles.clone()
}

fn not_found(file: &str) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("File not found: {file}"))
}

/// Resolve a `file=` parameter to an id the graph knows.
pub(super) fn lookup(engine: &SearchEngine, file: &str) -> Result<u32, (StatusCode, String)> {
    engine
        .find_file_id(file)
        .filter(|&id| engine.dependency_index.path_of(id).is_some())
        .ok_or_else(|| not_found(file))
}

async fn blocking<T, F>(state: WebState, f: F) -> Result<Json<T>, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&WebState, &SearchEngine) -> Result<T, (StatusCode, String)> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let engine = try_read_engine(&state.engine)?;
        f(&state, &engine).map(Json)
    })
    .await
    .map_err(|e| {
        ApiError::from((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {e}"),
        ))
    })?
    .map_err(ApiError::from)
}

// ---------------------------------------------------------------- shared

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub path: String,
    /// Signed distance from the selected file: negative for files it
    /// (transitively) imports, positive for files that import it, 0 for it.
    pub depth: i32,
    pub imports: usize,
    pub imported_by: usize,
    pub test: bool,
    /// Part of an import cycle (with `containment` as requested).
    pub cycle: bool,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    /// Both ends are in the same import cycle.
    pub cycle: bool,
    /// A Rust parent module reaching its own child (`mod foo;`).
    pub containment: bool,
}

/// Files at one depth that were not listed.
#[derive(Debug, Serialize)]
pub struct HiddenLevel {
    pub depth: i32,
    pub count: usize,
    /// The folder they share, when they are all in one.
    pub folder: Option<String>,
    pub folders: usize,
}

#[derive(Debug, Serialize)]
pub struct NodeSet {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub hidden: Vec<HiddenLevel>,
}

fn degree(index: &DependencyIndex, id: u32, containment: bool) -> (usize, usize) {
    (
        index.neighbours(id, Direction::Imports, containment).len(),
        index
            .neighbours(id, Direction::ImportedBy, containment)
            .len(),
    )
}

/// Display path of `id`, or `fallback` (what the caller asked for).
fn shown_path(engine: &SearchEngine, id: u32, fallback: &str) -> String {
    engine
        .display_path(id)
        .map_or_else(|| fallback.to_string(), Cow::into_owned)
}

fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// A placed file: id, imports, imported-by count, display path.
type Placed<'a> = (u32, usize, usize, Cow<'a, str>);

/// Turn `(id, signed depth)` placements into capped levels plus the edges
/// among the files that made the cut.
fn build_node_set(
    engine: &SearchEngine,
    placed: &[(u32, i32)],
    limit: usize,
    expand: &FxHashSet<i32>,
    containment: bool,
    cycles: &FxHashMap<u32, u32>,
) -> NodeSet {
    let index = &engine.dependency_index;
    let mut by_depth: FxHashMap<i32, Vec<Placed<'_>>> = FxHashMap::default();
    for &(id, depth) in placed {
        let Some(path) = engine.display_path(id) else {
            continue;
        };
        let (imports, imported_by) = degree(index, id, containment);
        by_depth
            .entry(depth)
            .or_default()
            .push((id, imports, imported_by, path));
    }
    let mut depths: Vec<i32> = by_depth.keys().copied().collect();
    depths.sort_unstable();

    let mut nodes = Vec::new();
    let mut hidden = Vec::new();
    let mut shown: FxHashMap<u32, String> = FxHashMap::default();
    for depth in depths {
        let mut level = by_depth.remove(&depth).unwrap_or_default();
        level.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then_with(|| a.3.cmp(&b.3)));
        let keep = if depth == 0 {
            level.len()
        } else if expand.contains(&depth) {
            MAX_LEVEL_LIMIT.min(level.len())
        } else {
            limit.min(level.len())
        };
        let rest = &level[keep..];
        if !rest.is_empty() {
            let folders: FxHashSet<&str> = rest.iter().map(|f| folder_of(&f.3)).collect();
            hidden.push(HiddenLevel {
                depth,
                count: rest.len(),
                folder: (folders.len() == 1).then(|| folder_of(&rest[0].3).to_string()),
                folders: folders.len(),
            });
        }
        for (id, imports, imported_by, path) in &level[..keep] {
            let (id, imports, imported_by) = (*id, *imports, *imported_by);
            shown.insert(id, path.to_string());
            nodes.push(GraphNode {
                path: path.to_string(),
                depth,
                imports,
                imported_by,
                test: is_test_path(path),
                cycle: cycles.contains_key(&id),
            });
        }
    }

    let mut edges = Vec::new();
    let mut ids: Vec<u32> = shown.keys().copied().collect();
    ids.sort_unstable();
    for &from in &ids {
        for to in index.neighbours(from, Direction::Imports, true) {
            let Some(to_path) = shown.get(&to) else {
                continue;
            };
            let is_containment = index.is_containment(from, to);
            if is_containment && !containment {
                continue;
            }
            edges.push(GraphEdge {
                from: shown[&from].clone(),
                to: to_path.clone(),
                cycle: cycles
                    .get(&from)
                    .is_some_and(|c| cycles.get(&to) == Some(c)),
                containment: is_containment,
            });
        }
    }
    NodeSet {
        nodes,
        edges,
        hidden,
    }
}

fn parse_expand(expand: &Option<String>) -> FxHashSet<i32> {
    expand
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

fn level_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_LEVEL_LIMIT)
        .clamp(1, MAX_LEVEL_LIMIT)
}

// ---------------------------------------------------------- neighborhood

#[derive(Debug, Deserialize)]
pub struct NeighborhoodQuery {
    file: String,
    /// Hops in each direction (1 to 4, default 2).
    depth: Option<usize>,
    /// Files listed per level (default 12, max 500).
    limit: Option<usize>,
    /// Comma-separated depths to list in full (up to 500 files), e.g. `-2,1`.
    expand: Option<String>,
    #[serde(default)]
    containment: bool,
}

#[derive(Debug, Serialize)]
pub struct NeighborhoodResponse {
    pub file: String,
    pub depth: usize,
    /// Files within `depth` hops on each side (before any capping).
    pub upstream: usize,
    pub downstream: usize,
    #[serde(flatten)]
    pub graph: NodeSet,
}

/// `GET /api/graph/neighborhood`: the files around one file, its imports on
/// the negative side and its importers on the positive side.
pub async fn neighborhood_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<NeighborhoodQuery>,
) -> Result<Json<NeighborhoodResponse>, ApiError> {
    require_file_param(&q.file)?;
    blocking(state, move |state, engine| {
        let id = lookup(engine, &q.file)?;
        let depth = q.depth.unwrap_or(2).clamp(1, 4);
        let index = &engine.dependency_index;
        let up = index.levels(id, Direction::Imports, Some(depth), q.containment);
        let down = index.levels(id, Direction::ImportedBy, Some(depth), q.containment);

        // A file reachable both ways (a cycle) sits on its nearer side.
        let mut side: FxHashMap<u32, i32> = FxHashMap::default();
        for (d, level) in up.iter().enumerate().skip(1) {
            for &n in level {
                side.insert(n, -(d as i32));
            }
        }
        for (d, level) in down.iter().enumerate().skip(1) {
            for &n in level {
                let d = d as i32;
                side.entry(n)
                    .and_modify(|s| {
                        if d < -*s {
                            *s = d
                        }
                    })
                    .or_insert(d);
            }
        }
        let mut placed: Vec<(u32, i32)> = side.into_iter().collect();
        placed.push((id, 0));
        let cycles = cycles(state, engine, q.containment);
        let graph = build_node_set(
            engine,
            &placed,
            level_limit(q.limit),
            &parse_expand(&q.expand),
            q.containment,
            &cycles,
        );
        Ok(NeighborhoodResponse {
            file: shown_path(engine, id, &q.file),
            depth,
            upstream: up.iter().skip(1).map(Vec::len).sum(),
            downstream: down.iter().skip(1).map(Vec::len).sum(),
            graph,
        })
    })
    .await
}

// ---------------------------------------------------------------- impact

#[derive(Debug, Deserialize)]
pub struct ImpactQuery {
    file: String,
    limit: Option<usize>,
    expand: Option<String>,
    #[serde(default)]
    containment: bool,
}

#[derive(Debug, Serialize)]
pub struct ImpactResponse {
    pub file: String,
    /// Every file that transitively imports `file`.
    pub affected: usize,
    /// Longest distance (in imports) to an affected file.
    pub levels: usize,
    pub tests_total: usize,
    /// Affected test files, nearest first (at most 200).
    pub tests: Vec<String>,
    #[serde(flatten)]
    pub graph: NodeSet,
}

/// `GET /api/graph/impact`: everything a change to one file can reach,
/// level by level, with the affected tests listed.
pub async fn impact_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<ImpactQuery>,
) -> Result<Json<ImpactResponse>, ApiError> {
    require_file_param(&q.file)?;
    blocking(state, move |state, engine| {
        let id = lookup(engine, &q.file)?;
        let levels = engine
            .dependency_index
            .levels(id, Direction::ImportedBy, None, q.containment);
        let placed: Vec<(u32, i32)> = levels
            .iter()
            .enumerate()
            .flat_map(|(d, level)| level.iter().map(move |&n| (n, d as i32)))
            .collect();
        let mut tests: Vec<String> = Vec::new();
        let mut tests_total = 0;
        for level in levels.iter().skip(1) {
            let mut paths: Vec<Cow<'_, str>> = level
                .iter()
                .filter_map(|&n| engine.display_path(n))
                .filter(|p| is_test_path(p))
                .collect();
            paths.sort_unstable();
            tests_total += paths.len();
            for p in paths {
                if tests.len() < MAX_TESTS_LISTED {
                    tests.push(p.into_owned());
                }
            }
        }
        let cycles = cycles(state, engine, q.containment);
        let graph = build_node_set(
            engine,
            &placed,
            level_limit(q.limit),
            &parse_expand(&q.expand),
            q.containment,
            &cycles,
        );
        Ok(ImpactResponse {
            file: shown_path(engine, id, &q.file),
            affected: levels.iter().skip(1).map(Vec::len).sum(),
            levels: levels.len() - 1,
            tests_total,
            tests,
            graph,
        })
    })
    .await
}

// ------------------------------------------------------------------ path

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    from: String,
    to: String,
    #[serde(default)]
    containment: bool,
}

#[derive(Debug, Serialize)]
pub struct PathResponse {
    pub from: String,
    pub to: String,
    pub found: bool,
    /// The chain runs from `to` back to `from` (no chain the asked way).
    pub reversed: bool,
    /// Files along the chain, each importing the next.
    pub files: Vec<String>,
}

/// `GET /api/graph/path`: the shortest chain of imports between two files,
/// trying the reverse direction when there is none the asked way.
pub async fn path_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<PathQuery>,
) -> Result<Json<PathResponse>, ApiError> {
    require_file_param(&q.from)?;
    require_file_param(&q.to)?;
    blocking(state, move |_, engine| {
        let from = lookup(engine, &q.from)?;
        let to = lookup(engine, &q.to)?;
        let index = &engine.dependency_index;
        let (chain, reversed) = match index.shortest_path(from, to, q.containment) {
            Some(c) => (Some(c), false),
            None => (index.shortest_path(to, from, q.containment), true),
        };
        let files: Vec<String> = chain
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| engine.display_path(id).map(Cow::into_owned))
            .collect();
        Ok(PathResponse {
            from: shown_path(engine, from, &q.from),
            to: shown_path(engine, to, &q.to),
            found: !files.is_empty(),
            reversed: reversed && !files.is_empty(),
            files,
        })
    })
    .await
}

// --------------------------------------------------------------- modules

#[derive(Debug, Deserialize)]
pub struct ModulesQuery {
    #[serde(default)]
    containment: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleFolder {
    pub folder: String,
    pub files: usize,
    /// Imports from files in this folder into other folders.
    pub imports: usize,
    /// Imports into this folder from other folders.
    pub imported_by: usize,
    /// Part of an import cycle between folders.
    pub cycle: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleEdge {
    pub from: String,
    pub to: String,
    /// Number of file-level imports this folder-level edge stands for.
    pub count: usize,
    pub cycle: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModulesResponse {
    pub folders: Vec<ModuleFolder>,
    pub edges: Vec<ModuleEdge>,
}

fn build_modules(engine: &SearchEngine, containment: bool) -> ModulesResponse {
    let index = &engine.dependency_index;
    let mut folder_ids: FxHashMap<String, usize> = FxHashMap::default();
    let mut folders: Vec<ModuleFolder> = Vec::new();
    let mut file_folder: FxHashMap<u32, usize> = FxHashMap::default();
    let mut ids: Vec<u32> = index.file_ids().collect();
    ids.sort_unstable();
    for &id in &ids {
        let Some(path) = engine.display_path(id) else {
            continue;
        };
        let folder = folder_of(&path);
        let f = *folder_ids.entry(folder.to_string()).or_insert_with(|| {
            folders.push(ModuleFolder {
                folder: folder.to_string(),
                files: 0,
                imports: 0,
                imported_by: 0,
                cycle: false,
            });
            folders.len() - 1
        });
        folders[f].files += 1;
        file_folder.insert(id, f);
    }
    let mut counts: FxHashMap<(usize, usize), usize> = FxHashMap::default();
    for &id in &ids {
        let Some(&a) = file_folder.get(&id) else {
            continue;
        };
        for to in index.neighbours(id, Direction::Imports, containment) {
            if let Some(&b) = file_folder.get(&to) {
                if a != b {
                    *counts.entry((a, b)).or_default() += 1;
                }
            }
        }
    }
    let mut adjacency = vec![Vec::new(); folders.len()];
    for (&(a, b), &n) in &counts {
        adjacency[a].push(b);
        folders[a].imports += n;
        folders[b].imported_by += n;
    }
    let comp = crate::dependencies::graph::strongly_connected_components(&adjacency);
    let mut sizes: FxHashMap<usize, usize> = FxHashMap::default();
    for &c in &comp {
        *sizes.entry(c).or_default() += 1;
    }
    for (i, f) in folders.iter_mut().enumerate() {
        f.cycle = sizes[&comp[i]] > 1;
    }
    let mut edges: Vec<ModuleEdge> = counts
        .into_iter()
        .map(|((a, b), count)| ModuleEdge {
            from: folders[a].folder.clone(),
            to: folders[b].folder.clone(),
            count,
            cycle: comp[a] == comp[b],
        })
        .collect();
    edges.sort_by(|x, y| x.from.cmp(&y.from).then_with(|| x.to.cmp(&y.to)));
    folders.sort_by(|x, y| x.folder.cmp(&y.folder));
    ModulesResponse { folders, edges }
}

/// `GET /api/graph/modules`: the graph collapsed to folders, with the
/// number of imports behind each folder-to-folder edge and cycles marked.
pub async fn modules_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<ModulesQuery>,
) -> Result<Json<ModulesResponse>, ApiError> {
    blocking(state, move |state, engine| {
        cycles(state, engine, q.containment); // resets the cache when stale
        let mut cache = state.graph_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = &cache.modules {
            return Ok((**m).clone());
        }
        let built = Arc::new(build_modules(engine, q.containment));
        cache.modules = Some(built.clone());
        Ok((*built).clone())
    })
    .await
}

// --------------------------------------------------------------- imports

#[derive(Debug, Deserialize)]
pub struct ImportsQuery {
    file: String,
}

#[derive(Debug, Serialize)]
pub struct ImportLine {
    /// 1-based line of the import statement.
    pub line: usize,
    /// The import as written (`crate::search::Engine`, `./util`, `numpy`).
    pub spec: String,
    /// The indexed file it resolves to, if any.
    pub target: Option<String>,
    pub containment: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportsResponse {
    pub file: String,
    pub imports: Vec<ImportLine>,
}

/// `GET /api/graph/imports`: one file's import statements with their lines
/// and what each resolves to. The file is re-parsed on request (the graph
/// keeps edges, not lines); unresolved imports are external packages, the
/// standard library, or files outside the index.
pub async fn imports_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<ImportsQuery>,
) -> Result<Json<ImportsResponse>, ApiError> {
    require_file_param(&q.file)?;
    blocking(state, move |_, engine| {
        let id = lookup(engine, &q.file)?;
        let index = &engine.dependency_index;
        let canonical = index.path_of(id).ok_or_else(|| not_found(&q.file))?;
        let mapped = engine
            .file_store
            .get(id)
            .ok_or_else(|| not_found(&q.file))?;
        let content = mapped.as_str().map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("File is not valid UTF-8: {e}"),
            )
        })?;
        let statements = crate::symbols::SymbolExtractor::new_for_source(canonical, Some(&content))
            .extract_imports(&content)
            .unwrap_or_default();
        let imports = statements
            .into_iter()
            .map(|s| {
                let target_id = index
                    .resolve_import_path(canonical, &s.path)
                    .and_then(|p| index.get_file_id(&p))
                    .filter(|&t| t != id);
                ImportLine {
                    line: s.line + 1,
                    target: target_id
                        .and_then(|t| engine.display_path(t))
                        .map(Cow::into_owned),
                    containment: target_id.is_some_and(|t| index.is_containment(id, t)),
                    spec: s.path,
                }
            })
            .collect();
        Ok(ImportsResponse {
            file: shown_path(engine, id, &q.file),
            imports,
        })
    })
    .await
}

// ----------------------------------------------------------------- files

#[derive(Debug, Deserialize)]
pub struct FilesQuery {
    /// Case-insensitive substring of the path; empty lists the most
    /// connected files.
    #[serde(default)]
    q: String,
    limit: Option<usize>,
    #[serde(default)]
    containment: bool,
}

#[derive(Debug, Serialize)]
pub struct GraphFile {
    pub path: String,
    pub imports: usize,
    pub imported_by: usize,
}

#[derive(Debug, Serialize)]
pub struct FilesResponse {
    pub total: usize,
    pub files: Vec<GraphFile>,
}

/// `GET /api/graph/files`: files for the explorer's picker, most connected
/// first.
pub async fn files_handler(
    State(state): State<WebState>,
    ApiQuery(q): ApiQuery<FilesQuery>,
) -> Result<Json<FilesResponse>, ApiError> {
    blocking(state, move |_, engine| {
        let index = &engine.dependency_index;
        let needle = q.q.trim().to_lowercase();
        // Rank by the raw edge counts (no per-file containment check, so a
        // keystroke over a million files stays cheap); only the listed files
        // get their exact counts.
        let mut matches: Vec<(u32, Cow<'_, str>, usize)> = index
            .file_ids()
            .filter_map(|id| {
                let path = engine.display_path(id)?;
                if !needle.is_empty() && !path.to_lowercase().contains(&needle) {
                    return None;
                }
                Some((id, path, index.degree(id)))
            })
            .collect();
        matches.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
        let total = matches.len();
        let files = matches
            .into_iter()
            .take(q.limit.unwrap_or(100).clamp(1, 1000))
            .map(|(id, path, _)| {
                let (imports, imported_by) = degree(index, id, q.containment);
                GraphFile {
                    path: path.into_owned(),
                    imports,
                    imported_by,
                }
            })
            .collect();
        Ok(FilesResponse { total, files })
    })
    .await
}
