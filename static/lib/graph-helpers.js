// ============================================
// DEPENDENCY EXPLORER - pure helpers
//
// Browser script (loaded by graph.html before graph.js; every function is a
// global) that also exports for Node so `npm test` can exercise it without a
// DOM. Nothing here touches document, window or localStorage.
// ============================================

/** Last path component. */
function graphBaseName(path) {
    const s = String(path || '');
    return s.slice(s.lastIndexOf('/') + 1);
}

/** Folder part of a display path ('' for a top-level file). */
function graphFolderOf(path) {
    const s = String(path || '');
    const i = s.lastIndexOf('/');
    return i < 0 ? '' : s.slice(0, i);
}

/**
 * The keyword page's `include` filter for exactly these files: their display
 * paths joined with `;`, glob metacharacters wrapped in a character class so
 * they match literally. A path containing `;` cannot be expressed (the filter
 * splits on it) and is left out.
 * @param {string[]} paths
 * @returns {string}
 */
function searchIncludeFor(paths) {
    return [...new Set(paths)]
        .filter(p => p && !p.includes(';'))
        .map(p => p.replace(/[*?[\]{}]/g, c => `[${c}]`))
        .join(';');
}

/**
 * Layer a folder graph left to right so every folder only imports folders in
 * lower layers: each folder's layer is the longest chain of imports below
 * it. Folders in one import cycle (joined by `cycle` edges) share a layer.
 * Iterative, so a long chain of folders cannot overflow the stack.
 * @param {string[]} folders
 * @param {{from: string, to: string, cycle?: boolean}[]} edges
 * @returns {Map<string, number>} folder -> layer (0 = imports nothing)
 */
function layerFolders(folders, edges) {
    // Union the members of each cycle so the condensed graph is acyclic.
    const parent = new Map(folders.map(f => [f, f]));
    const find = (f) => {
        let r = f;
        while (parent.get(r) !== r) r = parent.get(r);
        while (parent.get(f) !== r) { const n = parent.get(f); parent.set(f, r); f = n; }
        return r;
    };
    for (const e of edges) {
        if (e.cycle && parent.has(e.from) && parent.has(e.to)) parent.set(find(e.from), find(e.to));
    }
    const succ = new Map(), preds = new Map(), outDeg = new Map();
    for (const f of folders) {
        const r = find(f);
        if (!succ.has(r)) { succ.set(r, new Set()); preds.set(r, []); outDeg.set(r, 0); }
    }
    for (const e of edges) {
        if (!parent.has(e.from) || !parent.has(e.to)) continue;
        const a = find(e.from), b = find(e.to);
        if (a === b || succ.get(a).has(b)) continue;
        succ.get(a).add(b);
        preds.get(b).push(a);
        outDeg.set(a, outDeg.get(a) + 1);
    }
    // Peel sinks first: a component's layer is one more than its highest successor.
    const layer = new Map();
    const queue = [...outDeg.keys()].filter(r => outDeg.get(r) === 0);
    for (const r of queue) layer.set(r, 0);
    for (let i = 0; i < queue.length; i++) {
        const r = queue[i];
        for (const p of preds.get(r)) {
            layer.set(p, Math.max(layer.get(p) || 0, layer.get(r) + 1));
            outDeg.set(p, outDeg.get(p) - 1);
            if (outDeg.get(p) === 0) queue.push(p);
        }
    }
    return new Map(folders.map(f => [f, layer.get(find(f)) || 0]));
}

/**
 * Keep the most connected folders of a module map so it stays drawable on a
 * very large repository, with the edges among them.
 * @returns {{folders: object[], edges: object[], hidden: number}}
 */
function capModuleMap(map, max) {
    if (map.folders.length <= max) return { folders: map.folders, edges: map.edges, hidden: 0 };
    const kept = [...map.folders]
        .sort((a, b) => (b.imports + b.imported_by) - (a.imports + a.imported_by) || a.folder.localeCompare(b.folder))
        .slice(0, max);
    const names = new Set(kept.map(f => f.folder));
    return {
        folders: kept,
        edges: map.edges.filter(e => names.has(e.from) && names.has(e.to)),
        hidden: map.folders.length - kept.length,
    };
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { graphBaseName, graphFolderOf, searchIncludeFor, layerFolders, capModuleMap };
}
