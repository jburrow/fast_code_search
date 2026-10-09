// ============================================
// DEPENDENCY EXPLORER (graph.html)
//
// Four views over /api/graph/*: a file's neighbourhood (what it imports on
// the left, what imports it on the right), the impact of changing it, the
// import chain between two files, and the folder-level module map. The
// source pane shows the selected file with its import lines marked and
// linked. Uses escapeHtml / readErrorBody (common.js), hljsLangForPath /
// splitHighlightedHtml (lib/keyword-helpers.js) and lib/graph-helpers.js.
// ============================================

(() => {
'use strict';

const $ = (s) => document.querySelector(s);
const esc = escapeHtml;
const base = graphBaseName;
const dirOf = graphFolderOf;
const NS = 'http://www.w3.org/2000/svg';
const MODES = ['hood', 'impact', 'path', 'map'];
const MAP_MAX_FOLDERS = 400;
const MAP_COLUMN_CAP = 40;
const SOURCE_WINDOW = 2000;
const HLJS_MAX_CHARS = 1_000_000;
const INCLUDE_MAX_CHARS = 6000;

const params = new URLSearchParams(location.search);
const S = {
    mode: MODES.includes(params.get('mode')) ? params.get('mode') : 'hood',
    center: params.get('file') || null,
    pathTo: params.get('to') || '',
    depth: Math.min(4, Math.max(1, parseInt(params.get('depth'), 10) || 2)),
    containment: false,
    preview: null,
    hl: null,
    expanded: new Set(),
    hist: [],
    filter: '',
    graph: null, // last graph response for the current mode
};
if (!params.has('mode')) {
    try { const m = localStorage.getItem('fcs-graph-mode'); if (MODES.includes(m)) S.mode = m; } catch (_) { /* ignore */ }
}

// ---------- API ----------

async function getJson(url) {
    const r = await fetch(url);
    if (!r.ok) {
        const err = new Error(await readErrorBody(r));
        err.status = r.status;
        throw err;
    }
    return r.json();
}

function graphApi(endpoint, query) {
    const qs = new URLSearchParams(query);
    if (S.containment) qs.set('containment', 'true');
    return getJson(`/api/graph/${endpoint}?${qs}`);
}

const fileCache = new Map();
const importsCache = new Map();
const nearCache = new Map();
function cached(map, key, load) {
    if (!map.has(key)) map.set(key, load().catch(e => { map.delete(key); throw e; }));
    return map.get(key);
}
const loadFile = (p) => cached(fileCache, p, () => getJson(`/api/file?file=${encodeURIComponent(p)}`));
const loadImports = (p) => cached(importsCache, p, () => graphApi('imports', { file: p }));
const loadNear = (p) => cached(nearCache, p + '|' + S.containment,
    () => graphApi('neighborhood', { file: p, depth: 1, limit: 60 }));

// ---------- layout + svg ----------

const W = 196, H = 42, GAPX = 72, GAPY = 12, PAD = 28;
const clip = (s, n) => s.length > n ? s.slice(0, n - 1) + '…' : s;
const clipLeft = (s, n) => s.length > n ? '…' + s.slice(s.length - n + 1) : s;
const plural = (n, word) => `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`;

function el(tag, attrs, parent) {
    const e = document.createElementNS(NS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(e);
    return e;
}

/** Curve from importer `a` to imported `b`; flow runs right to left. */
function edgePath(a, b) {
    const x1 = a.x, y1 = a.y + a.h / 2, x2 = b.x + b.w, y2 = b.y + b.h / 2;
    if (x1 > x2) {
        const mx = (x1 + x2) / 2;
        return `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2 + 6},${y2}`;
    }
    const lift = 40 + Math.abs(y1 - y2) * 0.2;
    return `M${a.x + a.w / 2},${a.y} C${a.x + a.w / 2},${a.y - lift} ${b.x + b.w / 2},${b.y - lift} ${b.x + b.w / 2},${b.y - 6}`;
}

let POS = new Map(); // key -> {x, y, w, h, col, ...item}

function columnsLayout(cols) {
    const keys = [...cols.keys()].sort((a, b) => a - b);
    const minC = keys[0];
    const maxH = Math.max(...keys.map(k => cols.get(k).length));
    POS = new Map();
    keys.forEach(k => {
        const items = cols.get(k);
        const colH = items.length * (H + GAPY) - GAPY;
        const y0 = PAD + 22 + ((maxH * (H + GAPY) - GAPY) - colH) / 2;
        items.forEach((it, i) => POS.set(it.key, { ...it, x: PAD + (k - minC) * (W + GAPX), y: y0 + i * (H + GAPY), w: W, h: H, col: k }));
    });
    return { width: PAD * 2 + keys.length * (W + GAPX) - GAPX, height: PAD * 2 + 22 + maxH * (H + GAPY), minC };
}

/** Order each column by the average row of its neighbours nearer the centre. */
function orderColumns(cols, centerCol, adjacency) {
    const keys = [...cols.keys()].sort((a, b) => Math.abs(a - centerCol) - Math.abs(b - centerCol));
    const rank = new Map();
    keys.forEach(k => {
        const items = cols.get(k);
        const toward = new Set((cols.get(k > centerCol ? k - 1 : k + 1) || []).map(x => x.key));
        items.forEach(it => {
            const ns = [...(adjacency.get(it.key) || [])].filter(p => rank.has(p) && toward.has(p));
            it._b = ns.length ? ns.reduce((s, p) => s + rank.get(p), 0) / ns.length : 1e9;
        });
        if (k !== centerCol) {
            items.sort((a, b) => (a.more ? 1 : 0) - (b.more ? 1 : 0) || a._b - b._b ||
                (b.deg || 0) - (a.deg || 0) || a.key.localeCompare(b.key));
        }
        items.forEach((it, i) => rank.set(it.key, i));
    });
}

function drawGraph(cols, opts) {
    const wrap = $('#graphwrap');
    wrap.innerHTML = '';
    if (!cols.size) return;
    const L = columnsLayout(cols);
    const sc = Math.max(opts.minScale || 0.8, Math.min(1, (wrap.clientWidth - 8) / L.width));
    const svg = el('svg', { width: Math.round(L.width * sc), height: Math.round(L.height * sc), viewBox: `0 0 ${L.width} ${L.height}`, role: 'group', 'aria-label': opts.aria });
    const defs = el('defs', {}, svg);
    [['dep', 'var(--dep)'], ['rdep', 'var(--rdep)'], ['cyc', 'var(--warn)'], ['n', 'var(--line-strong)']].forEach(([id, c]) => {
        const m = el('marker', { id: 'ar-' + id, viewBox: '0 0 8 8', refX: 7, refY: 4, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' }, defs);
        el('path', { d: 'M0,0 L8,4 L0,8 z', fill: c }, m);
    });
    (opts.colLabels || []).forEach(([k, t]) => {
        el('text', { x: PAD + (k - L.minC) * (W + GAPX), y: PAD + 6, class: 'collabel' }, svg).textContent = t;
    });
    const eg = el('g', {}, svg), ng = el('g', {}, svg);
    opts.edges.forEach(ed => {
        const a = POS.get(ed.a), b = POS.get(ed.b);
        if (!a || !b) return;
        const d = edgePath(a, b);
        const isHl = S.hl && ed.e && S.hl.from === ed.e.from && S.hl.to === ed.e.to;
        const p = el('path', { d, class: `gedge ${ed.cls}${isHl ? ' hl' : ''}`, 'marker-end': `url(#ar-${ed.marker || 'n'})` }, eg);
        if (ed.w) p.style.strokeWidth = ed.w;
        const hit = el('path', { d, class: 'ehit' }, eg);
        if (ed.e) hit.addEventListener('click', () => openImport(ed.e.from, ed.e.to));
        hit.addEventListener('mousemove', (ev) => tipShow(ev, ed.tip || edgeTip(ed.e)));
        hit.addEventListener('mouseleave', tipHide);
    });
    for (const [key, n] of POS) {
        const cls = ['gnode', ...(opts.nodeCls ? opts.nodeCls(n) : [])];
        if (n.path && n.path === S.preview && n.path !== S.center) cls.push('preview');
        const g = el('g', { class: cls.join(' '), transform: `translate(${n.x},${n.y})`, tabindex: 0, role: 'button', 'data-key': key }, ng);
        el('rect', { class: 'box', width: n.w, height: n.h }, g);
        if (n.path) el('rect', { class: 'side', width: 4, height: n.h, fill: 'transparent' }, g);
        el('text', { class: 't', x: 12, y: 18 }, g).textContent = clip(n.label || base(n.path), 24);
        el('text', { class: 's', x: 12, y: 33 }, g).textContent = n.sub != null ? clip(n.sub, 32) : nodeSub(n);
        g.setAttribute('aria-label', n.more ? `${n.label} ${n.sub}, expand` : (n.path || n.label));
        g.addEventListener('click', () => opts.onClick(n));
        g.addEventListener('dblclick', () => opts.onOpen && opts.onOpen(n));
        g.addEventListener('keydown', (ev) => keyNav(ev, n, opts));
        g.addEventListener('mousemove', (ev) => tipShow(ev, opts.tip ? opts.tip(n) : nodeTip(n)));
        g.addEventListener('mouseleave', tipHide);
    }
    wrap.appendChild(svg);
    const c = opts.focusKey ? POS.get(opts.focusKey) : null;
    if (c) wrap.scrollTo({ left: (c.x + c.w / 2) * sc - wrap.clientWidth / 2, top: c.y * sc - wrap.clientHeight / 2 + 40 });
    else wrap.scrollTo({ left: 0, top: 0 });
}

function nodeSub(n) {
    const counts = n.imports != null ? `  ↓${n.imported_by} ↑${n.imports}` : '';
    return clipLeft((dirOf(n.path) || '(root)') + '/', 29 - counts.length) + counts;
}
function nodeTip(n) {
    if (n.more) return `${n.count.toLocaleString()} more ${n.unit || 'files'} ${esc(n.sub)}<br><i>click to show ${n.count > 500 ? 'the next 500' : 'them'}</i>`;
    const counts = n.imports != null ? `<br>imported by ${n.imported_by} · imports ${n.imports}` : '';
    return `<b>${esc(n.path)}</b>${counts}${n.test ? '<br>test file' : ''}${n.cycle ? '<br>part of an import cycle' : ''}<br><i>click to read · double-click to centre</i>`;
}
function edgeTip(e) {
    return e ? `${esc(e.from)} imports ${esc(base(e.to))}<br><i>click to open the import line</i>` : '';
}

function keyNav(ev, n, opts) {
    if (ev.key === 'Enter') { ev.preventDefault(); (opts.onOpen || opts.onClick)(n); return; }
    if (ev.key === ' ') { ev.preventDefault(); opts.onClick(n); return; }
    if (ev.key === 'Backspace') { ev.preventDefault(); back(); return; }
    const dirs = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[ev.key];
    if (!dirs) return;
    ev.preventDefault();
    let best = null, bd = Infinity;
    for (const [k, m] of POS) {
        const dx = m.x - n.x, dy = m.y - n.y;
        if (dirs[0] && Math.sign(dx) !== dirs[0]) continue;
        if (dirs[1] && (Math.sign(dy) !== dirs[1] || Math.abs(dx) > 1)) continue;
        const d = dirs[0] ? Math.abs(dx) * 4 + Math.abs(dy) : Math.abs(dy);
        if (d < bd && k !== n.key) { bd = d; best = k; }
    }
    if (best) document.querySelector(`.gnode[data-key="${CSS.escape(best)}"]`)?.focus();
}

const tip = $('#tip');
function tipShow(ev, html) {
    if (!html) return;
    tip.innerHTML = html;
    tip.hidden = false;
    const r = tip.getBoundingClientRect();
    let x = ev.clientX + 14, y = ev.clientY + 14;
    if (x + r.width > innerWidth - 8) x = ev.clientX - r.width - 14;
    if (y + r.height > innerHeight - 8) y = ev.clientY - r.height - 14;
    tip.style.left = Math.max(4, x) + 'px';
    tip.style.top = Math.max(4, y) + 'px';
}
function tipHide() { tip.hidden = true; }

// ---------- file-level views (neighbourhood, impact) ----------

/** Columns from a NodeSet response: one per signed depth, "+N more" last. */
function nodeSetColumns(data, centerKey) {
    const cols = new Map();
    const add = (k, item) => { if (!cols.has(k)) cols.set(k, []); cols.get(k).push(item); };
    data.nodes.forEach(n => add(n.depth, { key: n.path, ...n, deg: n.imports + n.imported_by }));
    data.hidden.forEach(h => add(h.depth, {
        key: 'more:' + h.depth, more: h.depth, count: h.count, label: `+${h.count.toLocaleString()} more`,
        sub: h.folder != null ? `in ${h.folder || '(root)'}/` : `in ${h.folders.toLocaleString()} folders`,
    }));
    const adjacency = new Map();
    data.edges.forEach(e => {
        if (!adjacency.has(e.from)) adjacency.set(e.from, new Set());
        if (!adjacency.has(e.to)) adjacency.set(e.to, new Set());
        adjacency.get(e.from).add(e.to);
        adjacency.get(e.to).add(e.from);
    });
    orderColumns(cols, 0, adjacency);
    if (!cols.has(0)) cols.set(0, [{ key: centerKey, path: centerKey }]);
    return cols;
}

const expandMore = (n) => { S.expanded.add(n.more); loadView(true); };
const fileClick = (n) => n.more != null ? expandMore(n) : setPreview(n.path);
const fileOpen = (n) => n.more != null ? expandMore(n) : go(n.path);

function drawHood(data) {
    const depthOf = new Map(data.nodes.map(n => [n.path, n.depth]));
    const cols = nodeSetColumns(data, data.file);
    const edges = [];
    for (const e of data.edges) {
        const ca = depthOf.get(e.from), cb = depthOf.get(e.to);
        if (Math.abs(ca - cb) !== 1) continue;
        const backward = ca < cb; // importer left of what it imports: against the flow
        const cls = e.containment ? 'n mod' : e.cycle ? 'cyc' : backward ? 'n' : (cb < 0 ? 'dep' : 'rdep');
        edges.push({ a: e.from, b: e.to, e, cls, marker: e.cycle && !e.containment ? 'cyc' : (backward || e.containment ? 'n' : cls) });
    }
    const label = (k) => k === 0 ? 'selected' : k < 0 ? (k === -1 ? 'imports' : `imports ×${-k}`) : (k === 1 ? 'imported by' : `imported by ×${k}`);
    drawGraph(cols, {
        aria: `Import neighbourhood of ${data.file}`, edges, focusKey: data.file,
        colLabels: [...cols.keys()].map(k => [k, label(k)]),
        nodeCls: n => n.more != null ? ['more'] : [n.path === S.center ? 'center' : (n.depth < 0 ? 'dep' : 'rdep'), ...(n.test ? ['test'] : []), ...(n.cycle && n.path !== S.center ? ['cyc'] : [])],
        onClick: fileClick, onOpen: fileOpen,
    });
    const me = data.nodes.find(n => n.depth === 0) || { imports: 0, imported_by: 0 };
    return `<span class="summary"><b>${esc(base(data.file))}</b> imports <b>${me.imports}</b> files directly and is imported by <b>${me.imported_by}</b>. Within ${plural(data.depth, 'hop')}: <b>${data.upstream.toLocaleString()}</b> upstream, <b>${data.downstream.toLocaleString()}</b> downstream.</span>
        <label class="toggle" for="depthSel">Hops <select id="depthSel" class="btn">${[1, 2, 3, 4].map(d => `<option ${d === S.depth ? 'selected' : ''}>${d}</option>`).join('')}</select></label>`;
}

function drawImpact(data) {
    const depthOf = new Map(data.nodes.map(n => [n.path, n.depth]));
    const cols = nodeSetColumns(data, data.file);
    const edges = data.edges
        .filter(e => depthOf.get(e.from) === depthOf.get(e.to) + 1)
        .map(e => ({ a: e.from, b: e.to, e, cls: 'rdep' + (e.containment ? ' mod' : ''), marker: 'rdep' }));
    drawGraph(cols, {
        aria: `Files affected by changing ${data.file}`, edges, focusKey: data.file,
        colLabels: [...cols.keys()].map(k => [k, k === 0 ? 'changed' : (k === 1 ? 'direct' : `${k} hops away`)]),
        nodeCls: n => n.more != null ? ['more'] : [n.path === S.center ? 'center' : 'rdep', ...(n.test ? ['test'] : [])],
        onClick: fileClick, onOpen: fileOpen,
    });
    if (!data.affected) {
        return `<span class="summary">Nothing imports <b>${esc(base(data.file))}</b>, so a change here affects no other indexed file.</span>`;
    }
    const shownTests = data.tests.slice(0, 8);
    const testChips = shownTests.map(t => `<button class="chip" data-preview="${esc(t)}" title="${esc(t)}">${esc(base(t))}</button>`).join(' ');
    const moreTests = data.tests_total > shownTests.length ? ` and ${(data.tests_total - shownTests.length).toLocaleString()} more` : '';
    return `<span class="summary">Changing <b>${esc(base(data.file))}</b> can affect <b>${data.affected.toLocaleString()}</b> files over <b>${plural(data.levels, 'level')}</b>, including <b>${plural(data.tests_total, 'test file')}</b>${data.tests_total ? ': ' + testChips + moreTests : ''}.</span>`;
}

function drawPath(data) {
    const fields = `<span class="pathfields"><label for="pathFrom">From</label><input id="pathFrom" list="allfiles" value="${esc(S.center)}"><label for="pathTo">to</label><input id="pathTo" list="allfiles" value="${esc(S.pathTo)}" placeholder="pick a file…"><button class="btn" id="swapPath">Swap</button></span>`;
    if (!data) {
        $('#graphwrap').innerHTML = `<div class="empty">Pick a second file to see the shortest chain of imports between the two.</div>`;
        return fields;
    }
    if (!data.found) {
        $('#graphwrap').innerHTML = `<div class="empty">No import chain connects <b>${esc(base(data.from))}</b> and <b>${esc(base(data.to))}</b> in either direction.</div>`;
        return fields;
    }
    const seq = data.files;
    const cols = new Map(seq.map((p, i) => [-i, [{ key: p, path: p }]]));
    const edges = seq.slice(1).map((p, i) => ({ a: seq[i], b: p, e: { from: seq[i], to: p }, cls: 'dep', marker: 'dep' }));
    drawGraph(cols, {
        aria: 'Import chain', focusKey: seq[0], edges,
        colLabels: seq.map((p, i) => [-i, i === 0 ? 'from' : i === seq.length - 1 ? 'to' : `step ${i}`]),
        nodeCls: n => [(n.path === seq[0] || n.path === seq[seq.length - 1]) ? 'center' : 'dep'],
        onClick: n => setPreview(n.path), onOpen: n => go(n.path),
    });
    const steps = seq.length - 1;
    return fields + `<span class="summary">${data.reversed ? 'Reverse direction: ' : ''}<b>${plural(steps, 'import')}</b> from <b>${esc(base(seq[0]))}</b> to <b>${esc(base(seq[seq.length - 1]))}</b>. Click an arrow to open its import line.</span>`;
}

// ---------- module map ----------

function drawMap(full) {
    const map = capModuleMap(full, MAP_MAX_FOLDERS);
    const layers = layerFolders(map.folders.map(f => f.folder), map.edges);
    const byCol = new Map();
    map.folders.forEach(f => {
        const k = layers.get(f.folder);
        if (!byCol.has(k)) byCol.set(k, []);
        byCol.get(k).push(f);
    });
    // Display paths start with their indexed root's name (unless the server
    // hides it); when there is only one root, leave it off the labels.
    const roots = new Set(full.folders.map(f => f.folder.split('/')[0]));
    const strip = full.root_name_shown !== false && roots.size === 1 ? [...roots][0] : null;
    const name = (d) => {
        if (d === '' || d === strip) return '(root)';
        return (strip && d.startsWith(strip + '/') ? d.slice(strip.length + 1) : d) + '/';
    };
    const cols = new Map();
    for (const [k, list] of byCol) {
        list.sort((a, b) => (b.imports + b.imported_by) - (a.imports + a.imported_by) || a.folder.localeCompare(b.folder));
        const shown = S.expanded.has(k) || list.length <= MAP_COLUMN_CAP ? list : list.slice(0, MAP_COLUMN_CAP - 1);
        const items = shown.map(f => ({
            key: 'dir:' + f.folder, dir: f.folder, cycle: f.cycle, deg: f.imports + f.imported_by,
            label: clipLeft(name(f.folder), 24),
            sub: `${plural(f.files, 'file')} · ↓${f.imported_by} ↑${f.imports}`,
        }));
        if (shown.length < list.length) {
            const rest = list.length - shown.length;
            items.push({ key: 'more:' + k, more: k, count: rest, unit: 'folders', label: `+${rest} more`, sub: 'in this layer' });
        }
        cols.set(k, items);
    }
    const edges = map.edges.map(e => ({
        a: 'dir:' + e.from, b: 'dir:' + e.to, cls: e.cycle ? 'cyc' : 'dep', marker: e.cycle ? 'cyc' : 'dep',
        w: (1 + Math.log2(e.count) * 0.9).toFixed(2),
        tip: `${esc(name(e.from))} imports ${esc(name(e.to))}<br>${plural(e.count, 'file-level import')}`,
    }));
    const centerDir = S.center != null ? dirOf(S.center) : null;
    drawGraph(cols, {
        aria: 'Folder-level import map', edges, focusKey: null, minScale: 0.7,
        colLabels: [...cols.keys()].map(k => [k, k === 0 ? 'foundations' : `layer ${k}`]),
        nodeCls: n => n.more != null ? ['more'] : [n.cycle ? 'cyc' : '', n.dir === centerDir ? 'center' : ''].filter(Boolean),
        onClick: n => {
            if (n.more != null) { S.expanded.add(n.more); drawCurrent(); return; }
            S.filter = n.dir ? n.dir + '/' : '';
            $('#filter').value = S.filter;
            loadFiles();
        },
        onOpen: n => n.more != null ? (S.expanded.add(n.more), drawCurrent()) : openFolder(n.dir),
        tip: n => n.more != null ? nodeTip(n) : `<b>${esc(name(n.dir))}</b><br>${esc(n.sub)}${n.cycle ? '<br><b>part of an import cycle between folders</b>' : ''}<br><i>click to list its files · double-click to centre on its most connected file</i>`,
    });
    const cyc = full.folders.filter(f => f.cycle);
    const hidden = map.hidden ? ` Showing the ${map.folders.length} most connected of ${full.folders.length.toLocaleString()} folders.` : '';
    return `<span class="summary"><b>${full.folders.length.toLocaleString()}</b> folders, layered so each one only imports folders to its left.${hidden} ${cyc.length
        ? `<b>${cyc.length}</b> folders form import cycles (red)${cyc.length <= 6 ? ': ' + cyc.map(f => `<b>${esc(name(f.folder))}</b>`).join(', ') : ''}.`
        : 'No cycles between folders.'}</span>`;
}

async function openFolder(dir) {
    try {
        const res = await graphApi('files', { q: dir ? dir + '/' : '', limit: 500 });
        const hit = res.files.find(f => dirOf(f.path) === dir) || res.files[0];
        if (hit) { S.mode = 'hood'; go(hit.path); }
    } catch (e) { showGraphError(e); }
}

// ---------- source pane ----------

function highlightLines(text, lang) {
    const plain = () => text.split('\n').map(esc);
    if (typeof hljs === 'undefined' || text.length > HLJS_MAX_CHARS || !hljs.getLanguage(lang)) return plain();
    try {
        const lines = splitHighlightedHtml(hljs.highlight(text, { language: lang, ignoreIllegals: true }).value);
        return lines.length === text.split('\n').length ? lines : plain();
    } catch (_) {
        return plain();
    }
}
const hlCache = new Map();
function linesOf(p, content) {
    if (!hlCache.has(p)) {
        if (hlCache.size > 30) hlCache.delete(hlCache.keys().next().value);
        hlCache.set(p, highlightLines(content, hljsLangForPath(p)));
    }
    return hlCache.get(p);
}

let sourceSeq = 0;
let sourceView = null; // {p, lines, imports, from, to}

/**
 * Show `p` in the source pane. `line` (1-based) or `importOf` (a file `p`
 * imports) picks the line to scroll to.
 */
async function renderSource(p, { line = null, importOf = null } = {}) {
    const seq = ++sourceSeq;
    const head = $('#srchead'), code = $('#code');
    if (!p) { head.innerHTML = ''; code.innerHTML = ''; return; }
    if (!sourceView || sourceView.p !== p) {
        head.innerHTML = `<div class="p">${esc(p)}</div><div class="m">Loading…</div>`;
    }
    let file, imports, near;
    try {
        [file, imports, near] = await Promise.all([
            loadFile(p),
            loadImports(p).catch(() => ({ imports: [] })),
            loadNear(p).catch(() => null),
        ]);
    } catch (e) {
        if (seq !== sourceSeq) return;
        head.innerHTML = `<div class="p">${esc(p)}</div><div class="m" style="color:var(--warn)">${esc(e.message)}</div>`;
        code.innerHTML = '';
        return;
    }
    if (seq !== sourceSeq) return;

    const resolved = imports.imports.filter(i => i.target && (S.containment || !i.containment));
    const external = imports.imports.filter(i => !i.target);
    const chipList = (depth, cls) => {
        if (!near) return '<span class="none">unavailable</span>';
        const nodes = near.nodes.filter(n => n.depth === depth);
        const hidden = near.hidden.find(h => h.depth === depth);
        if (!nodes.length) return `<span class="none">${depth > 0 ? 'nothing indexed' : 'no indexed files'}</span>`;
        return nodes.map(n => `<button class="chip${cls}" data-preview="${esc(n.path)}"${depth > 0 ? ` data-import-of="${esc(p)}"` : ''} title="${esc(n.path)}">${esc(base(n.path))}</button>`).join('') +
            (hidden ? `<span class="none">+${hidden.count.toLocaleString()} more</span>` : '');
    };
    const searchHref = '/?include=' + encodeURIComponent(searchIncludeFor([p]));
    head.innerHTML = `<div class="p">${esc(p)}</div>
        <div class="m"><span>${esc(hljsLangForPath(p))} · ${plural(file.line_count, 'line')}</span><span>${plural(resolved.length, 'resolved import')} · ${external.length.toLocaleString()} external or unresolved</span>
        ${p !== S.center ? '<button class="btn" id="centreHere">Centre graph here</button>' : ''}<a class="btn" href="${esc(searchHref)}" title="Keyword search limited to this file">Search in file</a></div>
        <div class="chips"><em>Imported by</em>${chipList(1, '')}</div>
        <div class="chips"><em>Imports</em>${chipList(-1, ' out')}</div>`;

    const lines = linesOf(p, file.content);
    const byLine = new Map();
    imports.imports.forEach(i => { if (!byLine.has(i.line)) byLine.set(i.line, i); });
    let target = line;
    if (importOf) target = imports.imports.find(i => i.target === importOf)?.line ?? null;
    const t = Math.min(Math.max(1, target || 1), lines.length);
    sourceView = { p, lines, byLine, from: Math.max(1, t - SOURCE_WINDOW), to: Math.min(lines.length, t + SOURCE_WINDOW) };
    paintSource();
    if (target != null) {
        const ln = document.getElementById('L' + target);
        if (ln) { code.scrollTop = ln.offsetTop - code.clientHeight / 3; ln.classList.add('flash'); }
    } else {
        code.scrollTop = 0;
    }
}

function paintSource() {
    const { lines, byLine, from, to } = sourceView;
    const parts = [];
    if (from > 1) parts.push(`<button class="more-lines" data-more="up">Show ${Math.min(SOURCE_WINDOW, from - 1).toLocaleString()} earlier lines</button>`);
    for (let n = from; n <= to; n++) {
        const imp = byLine.get(n);
        let cls = '', extra = '';
        if (imp && imp.target && (S.containment || !imp.containment)) {
            cls = ' imp';
            extra = `<button class="go${imp.containment ? ' mod' : ''}" data-preview="${esc(imp.target)}" title="Open ${esc(imp.target)}">→ ${esc(base(imp.target))}</button>`;
        } else if (imp && !imp.target) {
            cls = ' ext';
            extra = `<span class="extag" title="${esc(imp.spec)}: an external package, the standard library, or a file outside the index">external</span>`;
        }
        parts.push(`<div class="ln${cls}" id="L${n}"><span class="n">${n}</span><span class="c">${lines[n - 1] || ' '}${extra}</span></div>`);
    }
    if (to < lines.length) parts.push(`<button class="more-lines" data-more="down">Show ${Math.min(SOURCE_WINDOW, lines.length - to).toLocaleString()} more lines</button>`);
    $('#code').innerHTML = parts.join('');
}

// ---------- files list ----------

let filesSeq = 0;
async function loadFiles() {
    const seq = ++filesSeq;
    let res;
    try {
        res = await graphApi('files', { q: S.filter, limit: 300 });
    } catch (e) {
        if (seq === filesSeq) $('#files').innerHTML = `<div class="empty err">${esc(e.message)}</div>`;
        return null;
    }
    if (seq !== filesSeq) return null;
    const rows = res.files.map(f => `<button class="file${f.path === S.center ? ' on' : ''}" data-go="${esc(f.path)}" title="${esc(f.path)}"><span class="nm">${esc(base(f.path))}</span><span class="deg">${f.imported_by} · ${f.imports}</span><span class="dr">${esc(dirOf(f.path) || '(root)')}/&lrm;</span></button>`);
    const note = res.total > res.files.length ? `<div class="note">Showing the ${res.files.length} most connected of ${res.total.toLocaleString()}. Filter to find others.</div>` : '';
    $('#files').innerHTML = rows.length ? note + rows.join('') : '<div class="empty">No files match.</div>';
    $('#allfiles').innerHTML = res.files.map(f => `<option value="${esc(f.path)}">`).join('');
    if (!S.filter) $('#stats').textContent = `${res.total.toLocaleString()} files in the import graph`;
    return res;
}
function markCurrentFile() {
    document.querySelectorAll('#files .file').forEach(b => b.classList.toggle('on', b.dataset.go === S.center));
}

// ---------- view loading ----------

let viewSeq = 0;
function showGraphError(e) {
    const msg = e.status === 404
        ? `<b>${esc(S.center)}</b> is not in the import graph. Pick a file on the left.`
        : esc(e.message);
    $('#graphwrap').innerHTML = `<div class="empty err">${msg}</div>`;
}

async function loadView(keepScroll) {
    const seq = ++viewSeq;
    const wrap = $('#graphwrap');
    const sl = wrap.scrollLeft, st = wrap.scrollTop;
    const expand = [...S.expanded].join(',');
    let data = null;
    try {
        if (S.mode === 'map') data = await graphApi('modules', {});
        else if (!S.center) data = null;
        else if (S.mode === 'hood') data = await graphApi('neighborhood', { file: S.center, depth: S.depth, expand });
        else if (S.mode === 'impact') data = await graphApi('impact', { file: S.center, expand });
        else if (S.mode === 'path' && S.pathTo) data = await graphApi('path', { from: S.center, to: S.pathTo });
    } catch (e) {
        if (seq !== viewSeq) return;
        S.graph = null;
        renderChrome('');
        showGraphError(e);
        return;
    }
    if (seq !== viewSeq) return;
    S.graph = data;
    drawCurrent();
    if (keepScroll) { wrap.scrollLeft = sl; wrap.scrollTop = st; }
}

/** Redraw the current view from the last response (no fetch). */
function drawCurrent() {
    const data = S.graph;
    let summary = '';
    if (S.mode === 'map') summary = data ? drawMap(data) : '';
    else if (S.mode === 'path') summary = drawPath(data);
    else if (!data) $('#graphwrap').innerHTML = '<div class="empty">No file selected. Pick one on the left.</div>';
    else summary = S.mode === 'hood' ? drawHood(data) : drawImpact(data);
    renderChrome(summary);
}

function shownFiles() {
    const data = S.graph;
    if (!data) return [];
    if (S.mode === 'path') return data.files || [];
    if (S.mode === 'hood' || S.mode === 'impact') return data.nodes.map(n => n.path);
    return [];
}

function renderChrome(summary) {
    document.querySelectorAll('.tabs button').forEach(b => b.setAttribute('aria-selected', String(b.dataset.mode === S.mode)));
    const title = { hood: 'Neighbourhood', impact: 'Impact of a change', path: 'Import chain', map: 'Module map' }[S.mode];
    $('#graphTitle').textContent = title + (S.mode === 'map' ? ' · folders' : S.center ? ' · ' + S.center : '');
    const recent = S.hist.slice(-4);
    let search = '';
    const files = shownFiles();
    if (files.length) {
        let list = files, include = searchIncludeFor(list);
        while (include.length > INCLUDE_MAX_CHARS && list.length > 1) {
            list = list.slice(0, Math.floor(list.length * 0.8));
            include = searchIncludeFor(list);
        }
        search = `<a class="btn" href="/?include=${esc(encodeURIComponent(include))}" title="Keyword search limited to the files in this graph">Search these ${list.length} files</a>`;
    }
    $('#sub').innerHTML = `<button class="btn" id="backBtn" ${S.hist.length ? '' : 'disabled'} aria-label="Back">← Back</button>
        <nav class="crumbs" aria-label="History">${recent.map(p => `<button data-go="${esc(p)}" title="${esc(p)}">${esc(base(p))}</button><i>›</i>`).join('')}${S.center ? `<span class="cur">${esc(base(S.center))}</span>` : ''}</nav>${summary}${search}`;
    $('#legend').innerHTML = S.mode === 'map'
        ? `<span><i class="sw" style="border-color:var(--dep)"></i>folder imports folder (thicker = more imports)</span><span><i class="sw" style="border-color:var(--warn)"></i>import cycle</span><span>↓ imported by · ↑ imports</span>`
        : `<span><i class="sw" style="border-color:var(--dep)"></i>imports (upstream)</span><span><i class="sw" style="border-color:var(--rdep)"></i>imported by (downstream)</span><span><i class="sw" style="border-color:var(--warn)"></i>cycle</span>${S.containment ? '<span><i class="sw" style="border-color:var(--line-strong);border-top-style:dotted"></i><code>mod</code> declaration</span>' : ''}<span>dashed box = test file · ↓ imported by · ↑ imports</span>`;
    syncUrl();
}

function syncUrl() {
    const q = new URLSearchParams();
    if (S.center) q.set('file', S.center);
    if (S.mode !== 'hood') q.set('mode', S.mode);
    if (S.mode === 'path' && S.pathTo) q.set('to', S.pathTo);
    if (S.depth !== 2) q.set('depth', String(S.depth));
    const url = location.pathname + (q.toString() ? '?' + q : '');
    if (url !== location.pathname + location.search) history.replaceState(null, '', url);
    try { localStorage.setItem('fcs-graph-mode', S.mode); } catch (_) { /* ignore */ }
}

// ---------- navigation ----------

function go(p, opts = {}) {
    if (!p) return;
    if (S.center && p !== S.center) {
        S.hist.push(S.center);
        if (S.hist.length > 30) S.hist.shift();
    }
    S.center = p;
    S.preview = null;
    S.hl = null;
    S.expanded.clear();
    if (S.mode === 'map') S.mode = 'hood';
    markCurrentFile();
    loadView();
    renderSource(p, opts);
}
function back() {
    const p = S.hist.pop();
    if (!p) return;
    S.center = p;
    S.preview = null;
    S.hl = null;
    S.expanded.clear();
    markCurrentFile();
    loadView();
    renderSource(p);
}
function setPreview(p, opts = {}, hl = null) {
    S.preview = p;
    S.hl = hl;
    if (S.mode !== 'map') {
        const wrap = $('#graphwrap');
        const sl = wrap.scrollLeft, st = wrap.scrollTop;
        drawCurrent();
        wrap.scrollLeft = sl;
        wrap.scrollTop = st;
    }
    renderSource(p, opts);
}
/** Open `from` at the line where it imports `to`. */
function openImport(from, to) { setPreview(from, { importOf: to }, { from, to }); }

// ---------- events ----------

document.addEventListener('click', (ev) => {
    const pv = ev.target.closest('[data-preview]');
    if (pv) {
        const p = pv.dataset.preview;
        // An "imported by" chip opens the importer at its import line.
        if (pv.dataset.importOf) openImport(p, pv.dataset.importOf);
        else setPreview(p);
        return;
    }
    const g = ev.target.closest('[data-go]');
    if (g) { go(g.dataset.go); return; }
    const more = ev.target.closest('[data-more]');
    if (more && sourceView) {
        const keep = $('#code').scrollHeight - $('#code').scrollTop;
        if (more.dataset.more === 'up') sourceView.from = Math.max(1, sourceView.from - SOURCE_WINDOW);
        else sourceView.to = Math.min(sourceView.lines.length, sourceView.to + SOURCE_WINDOW);
        paintSource();
        if (more.dataset.more === 'up') $('#code').scrollTop = $('#code').scrollHeight - keep;
        return;
    }
    if (ev.target.closest('#backBtn')) back();
    if (ev.target.closest('#centreHere') && S.preview) go(S.preview);
    if (ev.target.closest('#swapPath') && S.pathTo) {
        const a = S.center;
        S.center = S.pathTo;
        S.pathTo = a;
        markCurrentFile();
        loadView();
        renderSource(S.center);
    }
    const tab = ev.target.closest('.tabs button');
    if (tab && tab.dataset.mode !== S.mode) {
        S.mode = tab.dataset.mode;
        S.expanded.clear();
        S.graph = null;
        loadView();
    }
});

document.addEventListener('change', (ev) => {
    const t = ev.target;
    if (t.id === 'depthSel') { S.depth = +t.value; S.expanded.clear(); loadView(); }
    if (t.id === 'modToggle') {
        S.containment = t.checked;
        loadView(true);
        loadFiles();
        if (sourceView) renderSource(sourceView.p);
    }
    if (t.id === 'pathFrom' && t.value.trim()) { S.center = t.value.trim(); markCurrentFile(); loadView(); renderSource(S.center); }
    if (t.id === 'pathTo' && t.value.trim()) { S.pathTo = t.value.trim(); loadView(); }
});

$('#filter').addEventListener('input', debounce((ev) => { S.filter = ev.target.value; loadFiles(); }, 150));

window.addEventListener('resize', debounce(() => { if (S.graph || S.mode === 'path') drawCurrent(); }, 200));

// ---------- start ----------

(async () => {
    renderChrome('');
    const res = await loadFiles();
    if (!S.center && res && res.files.length) S.center = res.files[0].path;
    if (!S.center && S.mode !== 'map') {
        $('#graphwrap').innerHTML = '<div class="empty">The import graph is empty. Once indexing finishes, files that import each other show up here.</div>';
        renderChrome('');
        return;
    }
    markCurrentFile();
    loadView();
    if (S.center) renderSource(S.center);
})();
})();
