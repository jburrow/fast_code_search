// ============================================
// KEYWORD SEARCH - pure helpers
//
// Browser script (loaded by index.html before keyword.js; every function is
// a global) that also exports for Node so `npm test` can exercise it without
// a DOM. Nothing here touches document, window or localStorage.
// ============================================

// ---------- Form / URL state validation ----------

/** The server clamps `max` to 1..MAX_RESULTS_LIMIT. */
const MAX_RESULTS_LIMIT = 1000;
const CONTEXT_LINES_LIMIT = 10;

/**
 * Parse a requested result cap (URL param, stored setting, select value)
 * into an integer in 1..MAX_RESULTS_LIMIT, or null when it is not a number.
 * @param {*} value
 * @returns {number|null}
 */
function clampMaxResults(value) {
    const n = parseInt(value, 10);
    if (!Number.isFinite(n)) return null;
    return Math.min(MAX_RESULTS_LIMIT, Math.max(1, n));
}

/**
 * Pick the select option for a requested cap: the smallest option that is
 * >= the request, or the largest option when none is.
 * @param {number} requested - already clamped
 * @param {Array<number|string>} options - the select's option values
 * @returns {number|undefined} undefined when there are no numeric options
 */
function pickMaxResultsOption(requested, options) {
    const nums = options.map(o => parseInt(o, 10)).filter(Number.isFinite).sort((a, b) => a - b);
    return nums.find(o => o >= requested) ?? nums[nums.length - 1];
}

/**
 * Parse a context-line count into an integer in 0..CONTEXT_LINES_LIMIT
 * (0 when it is not a number).
 * @param {*} value
 * @returns {number}
 */
function clampContextLines(value) {
    const n = parseInt(value, 10);
    if (!Number.isFinite(n)) return 0;
    return Math.min(CONTEXT_LINES_LIMIT, Math.max(0, n));
}

/** `true`, `1`, `yes`, `on` (any case) are true; everything else is false. */
function parseBoolParam(value) {
    return ['true', '1', 'yes', 'on'].includes(String(value).toLowerCase());
}

// ---------- Icons ----------

/**
 * An inline SVG icon referencing the sprite in index.html (`#i-<name>`,
 * underscores in `name` become dashes). Sized by attributes, coloured by
 * the surrounding text (fill: currentColor via .icon).
 * @param {string} name - sprite symbol name, e.g. 'open_in_new'
 * @param {number} size - width and height in px
 */
function iconSvg(name, size) {
    const id = 'i-' + String(name).replace(/_/g, '-');
    return `<svg class="icon" width="${size}" height="${size}" aria-hidden="true"><use href="#${id}"/></svg>`;
}

// ---------- Languages and colours ----------

/**
 * Map a file path's extension to a highlight.js language name.
 * Falls back to 'plaintext' when unknown.
 */
function hljsLangForPath(filePath) {
    const name = (String(filePath || '').split('/').pop() || '').toLowerCase();
    // Files known by name rather than extension (Dockerfile, Dockerfile.dev).
    if (name === 'dockerfile' || name.startsWith('dockerfile.') || name.endsWith('.dockerfile')) return 'dockerfile';
    if (name === 'makefile' || name === 'gnumakefile' || name.endsWith('.mk')) return 'makefile';
    const ext = name.includes('.') ? name.split('.').pop() : '';
    const MAP = {
        rs: 'rust', py: 'python', pyi: 'python', pyw: 'python',
        js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'javascript',
        ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
        go: 'go', rb: 'ruby', java: 'java', cs: 'csharp', cpp: 'cpp', cc: 'cpp',
        cxx: 'cpp', c: 'c', h: 'c', hpp: 'cpp', hh: 'cpp', hxx: 'cpp',
        m: 'objectivec', mm: 'objectivec', php: 'php', sh: 'bash',
        bash: 'bash', zsh: 'bash', toml: 'toml', yaml: 'yaml', yml: 'yaml',
        json: 'json', jsonc: 'json', xml: 'xml', html: 'xml', htm: 'xml',
        svg: 'xml', vue: 'xml', svelte: 'xml', css: 'css', scss: 'scss', less: 'less',
        md: 'markdown', sql: 'sql', kt: 'kotlin', kts: 'kotlin', swift: 'swift', r: 'r',
        lua: 'lua', pl: 'perl', pm: 'perl', hs: 'haskell', ex: 'elixir',
        exs: 'elixir', erl: 'erlang', hrl: 'erlang', scala: 'scala', sc: 'scala',
        dart: 'dart', proto: 'protobuf', ini: 'ini', cfg: 'ini', conf: 'ini',
        diff: 'diff', patch: 'diff', graphql: 'graphql', gql: 'graphql',
        vb: 'vbnet', wat: 'wasm', wast: 'wasm',
    };
    return MAP[ext] || 'plaintext';
}

/**
 * The URL fragment that reopens the file viewer on `filePath` at `line`:
 * `#file=src%2Fmain.rs&line=42` (line 1 is left out).
 */
function fileViewHash(filePath, line) {
    const params = new URLSearchParams();
    params.set('file', filePath);
    if (line > 1) params.set('line', String(line));
    return '#' + params.toString();
}

/** Parse a fileViewHash fragment: `{ file, line }`, or null if it names no file. */
function parseFileViewHash(hash) {
    const params = new URLSearchParams(String(hash || '').replace(/^#/, ''));
    const file = params.get('file');
    if (!file) return null;
    const line = parseInt(params.get('line'), 10);
    return { file, line: line > 0 ? line : 1 };
}

// ---------- Go to definition ----------

const IDENT_CHAR = /[\p{L}\p{N}_$]/u;

/**
 * The identifier around `offset` in `text` (a caret position: the character
 * at `offset`, or the one before it when the caret sits at a word's end).
 * @returns {{start: number, end: number, word: string} | null}
 */
function identifierAt(text, offset) {
    const chars = Array.from(text);
    // Work in code points so a surrogate pair is one character, then map back.
    let cp = Array.from(text.slice(0, Math.max(0, offset))).length;
    if (!(cp < chars.length && IDENT_CHAR.test(chars[cp]))) {
        if (cp > 0 && IDENT_CHAR.test(chars[cp - 1])) cp -= 1;
        else return null;
    }
    let a = cp;
    let b = cp + 1;
    while (a > 0 && IDENT_CHAR.test(chars[a - 1])) a--;
    while (b < chars.length && IDENT_CHAR.test(chars[b])) b++;
    const word = chars.slice(a, b).join('');
    if (/^\p{N}/u.test(word)) return null; // a number, not a name
    const start = chars.slice(0, a).join('').length;
    return { start, end: start + word.length, word };
}

/** The query that finds where `name` is defined (with `symbols=true`). */
function definitionQuery(name) {
    return `word:yes case:yes ${name}`;
}

/**
 * Turn symbol-search results into definition targets for `name`: only hits
 * whose matched text is exactly `name`, one per file and line, those in
 * `fromFile` first (the usual case for a local helper), then by score.
 * @returns {Array<{file: string, line: number, content: string}>}
 */
function rankDefinitions(results, name, fromFile) {
    const seen = new Set();
    const defs = [];
    for (const r of results || []) {
        if (!r || !(r.line_number > 0)) continue;
        const content = String(r.content || '');
        const [a, b] = byteRangeToCharRange(content, Number(r.match_start) || 0, Number(r.match_end) || 0);
        if (content.slice(a, b) !== name) continue;
        // A whole name: `SearchEngine` is not defined by `SemanticSearchEngine`.
        const before = Array.from(content.slice(0, a)).pop() || '';
        const after = Array.from(content.slice(b))[0] || '';
        if (IDENT_CHAR.test(before) || IDENT_CHAR.test(after)) continue;
        const key = `${r.file_path}:${r.line_number}`;
        if (seen.has(key)) continue;
        seen.add(key);
        defs.push({ file: r.file_path, line: r.line_number, content: content.trim(), score: Number(r.score) || 0 });
    }
    defs.sort((x, y) => (y.file === fromFile) - (x.file === fromFile) || y.score - x.score);
    return defs.map(({ file, line, content }) => ({ file, line, content }));
}

/**
 * Does the dependency explorer have an import graph for this file? Imports
 * are resolved for Rust, Python and JavaScript/TypeScript only.
 */
function hasImportGraph(filePath) {
    return /\.(rs|py|pyi|pyw|js|jsx|mjs|cjs|ts|tsx|mts|cts)$/i.test(String(filePath || ''));
}

/** The dependency explorer centred on `filePath`. */
function graphExplorerUrl(filePath) {
    return '/graph.html?file=' + encodeURIComponent(filePath);
}

/**
 * Parse a CSS color string to an RGB object for contrast calculations.
 * Supports #rgb, #rrggbb, rgb() and rgba(); null for anything else.
 */
function parseColorToRgb(color) {
    if (!color) return null;
    const value = color.trim();

    if (value.startsWith('#')) {
        const hex = value.slice(1);
        if (hex.length === 3) {
            return {
                r: parseInt(hex[0] + hex[0], 16),
                g: parseInt(hex[1] + hex[1], 16),
                b: parseInt(hex[2] + hex[2], 16),
            };
        }
        if (hex.length === 6) {
            return {
                r: parseInt(hex.slice(0, 2), 16),
                g: parseInt(hex.slice(2, 4), 16),
                b: parseInt(hex.slice(4, 6), 16),
            };
        }
        return null;
    }

    const rgbMatch = value.match(/^rgba?\((\d+)[,\s]+(\d+)[,\s]+(\d+)/i);
    if (!rgbMatch) return null;
    return {
        r: Number(rgbMatch[1]),
        g: Number(rgbMatch[2]),
        b: Number(rgbMatch[3]),
    };
}

function toLinearChannel(v) {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** WCAG relative luminance of an {r,g,b} colour (0 = black, 1 = white). */
function relativeLuminance(rgb) {
    return 0.2126 * toLinearChannel(rgb.r)
        + 0.7152 * toLinearChannel(rgb.g)
        + 0.0722 * toLinearChannel(rgb.b);
}

/** WCAG contrast ratio between two relative luminances (1..21). */
function contrastRatio(l1, l2) {
    const lighter = Math.max(l1, l2);
    const darker = Math.min(l1, l2);
    return (lighter + 0.05) / (darker + 0.05);
}

// ---------- Match ranges ----------

/**
 * Convert a UTF-8 byte range in `str` to a UTF-16 code-unit range usable with
 * String.prototype.slice. Out-of-range input is clamped to the string.
 * @returns {[number, number]}
 */
function byteRangeToCharRange(str, byteStart, byteEnd) {
    let bytes = 0;
    let charStart = -1;
    let charEnd = -1;
    for (let i = 0; i < str.length;) {
        if (charStart < 0 && bytes >= byteStart) charStart = i;
        if (bytes >= byteEnd) { charEnd = i; break; }
        const cp = str.codePointAt(i);
        bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
        i += cp > 0xffff ? 2 : 1;
    }
    if (charStart < 0) charStart = str.length;
    if (charEnd < 0) charEnd = str.length;
    return [charStart, Math.max(charStart, charEnd)];
}

/** Sort ranges, drop empty ones and merge overlaps. */
function mergeRanges(ranges) {
    const sorted = ranges
        .filter(r => r && r[1] > r[0])
        .sort((a, b) => a[0] - b[0]);
    const out = [];
    for (const r of sorted) {
        const last = out[out.length - 1];
        if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
        else out.push([r[0], r[1]]);
    }
    return out;
}

// ---------- Path matches (the "Files" group) ----------

/**
 * Split a path into runs for display: `hit` marks the server's matched byte
 * ranges (`match_ranges` of a path match), `name` the file name part.
 * @param {string} path
 * @param {Array<[number, number]>} byteRanges
 * @returns {Array<{text: string, hit: boolean, name: boolean}>}
 */
function pathMatchRuns(path, byteRanges) {
    const hits = mergeRanges((byteRanges || []).map(r => byteRangeToCharRange(path, r[0], r[1])));
    const nameStart = path.lastIndexOf('/') + 1;
    const cuts = new Set([0, path.length, nameStart]);
    for (const [s, e] of hits) { cuts.add(s); cuts.add(e); }
    const points = [...cuts].filter(i => i >= 0 && i <= path.length).sort((a, b) => a - b);
    const runs = [];
    for (let i = 0; i + 1 < points.length; i++) {
        const [s, e] = [points[i], points[i + 1]];
        if (e <= s) continue;
        runs.push({
            text: path.slice(s, e),
            hit: hits.some(([hs, he]) => s >= hs && e <= he),
            name: s >= nameStart,
        });
    }
    return runs;
}

/** Header of the Files group: "1 PATH MATCHES" / "12 PATHS MATCH". */
function fileMatchesLabel(total) {
    const n = Number(total).toLocaleString('en-US');
    return total === 1 ? `${n} PATH MATCHES` : `${n} PATHS MATCH`;
}

// ---------- Query syntax (mirrors src/search/query_syntax.rs) ----------

function isYesToken(v) {
    return ['yes', 'y', 'true', '1', 'on'].includes(String(v).toLowerCase());
}

const QUERY_OPERATORS = ['-file:', 'file:', '-lang:', 'lang:', 'case:', 'word:'];

/**
 * Split on whitespace, like src/search/query_syntax.rs. Each token keeps its
 * `[start, end)` in the query, the operator prefix it starts with (`op`, or
 * '') and that operator's argument (`arg`, null when there is none). An
 * operator's argument may be quoted to hold spaces (`file:"my dir"`); any
 * other quote is text.
 * @returns {Array<{start: number, end: number, op: string, arg: string|null}>}
 */
function tokenizeQuery(raw) {
    const s = String(raw || '');
    const out = [];
    let i = 0;
    while (i < s.length) {
        if (/\s/.test(s[i])) { i++; continue; }
        const start = i;
        const op = QUERY_OPERATORS.find(o => s.startsWith(o, start)) || '';
        const afterOp = start + op.length;
        if (op && s[afterOp] === '"') {
            const close = s.indexOf('"', afterOp + 1);
            if (close > afterOp + 1 && (close + 1 === s.length || /\s/.test(s[close + 1]))) {
                out.push({ start, end: close + 1, op, arg: s.slice(afterOp + 1, close) });
                i = close + 1;
                continue;
            }
        }
        let end = start;
        while (end < s.length && !/\s/.test(s[end])) end++;
        const arg = s.slice(afterOp, end);
        out.push({ start, end, op, arg: op && arg ? arg : null });
        i = end;
    }
    return out;
}

/** Language names the server knows (others are accepted when alphanumeric). */
const KNOWN_LANGS = new Set([
    'rs', 'rust', 'py', 'python', 'js', 'javascript', 'ts', 'typescript', 'go', 'golang',
    'c', 'cpp', 'c++', 'cxx', 'java', 'cs', 'csharp', 'c#', 'rb', 'ruby', 'php', 'sh', 'bash',
    'shell', 'md', 'markdown', 'json', 'yaml', 'yml', 'toml', 'html', 'css',
]);

function isKnownLang(name) {
    const n = name.toLowerCase();
    return KNOWN_LANGS.has(n) || /^[a-z0-9]+$/.test(n);
}

/**
 * Parse a plain-text query exactly like src/search/query_syntax.rs: the
 * `file:`/`lang:`/`-file:`/`-lang:` filters are dropped here, `case:` and
 * `word:` set options, and everything else is one literal, as typed
 * (quotes and a leading `-` included). Runs of text separated by operators
 * are joined by one space. An operator without an argument, or `lang:`
 * with a name the server rejects, is text.
 * @returns {{text: string, terms: string[], caseSensitive: boolean, wholeWord: boolean}}
 */
function parseQueryTerms(raw) {
    const s = String(raw || '');
    const runs = [];
    let runOpen = false;
    let caseSensitive = false;
    let wholeWord = false;
    for (const t of tokenizeQuery(s)) {
        let isOp = t.arg !== null;
        if (isOp && (t.op === 'lang:' || t.op === '-lang:')) isOp = isKnownLang(t.arg);
        if (isOp) {
            if (t.op === 'case:') caseSensitive = isYesToken(t.arg);
            if (t.op === 'word:') wholeWord = isYesToken(t.arg);
            runOpen = false;
            continue;
        }
        if (runOpen) runs[runs.length - 1][1] = t.end;
        else runs.push([t.start, t.end]);
        runOpen = true;
    }
    const text = runs.map(([a, b]) => s.slice(a, b)).join(' ');
    return { text, terms: text ? [text] : [], caseSensitive, wholeWord };
}

// ---------- Regex and term matching ----------

/**
 * Best-effort translation of a Rust `regex` pattern to a JS RegExp: leading
 * inline flags `(?is)` become RegExp flags and `(?P<name>` becomes `(?<name>`.
 * Always global; returns null when the pattern does not compile in JS.
 */
function compileRustRegex(pattern) {
    let flags = 'g';
    let src = String(pattern);
    const lead = /^\(\?([a-z]+)\)/.exec(src);
    if (lead) {
        if (lead[1].includes('i')) flags += 'i';
        if (lead[1].includes('s')) flags += 's';
        if (lead[1].includes('m')) flags += 'm';
        src = src.slice(lead[0].length);
    }
    src = src.replace(/\(\?P</g, '(?<');
    try {
        return new RegExp(src, flags + 'u');
    } catch (_) {
        try { return new RegExp(src, flags); } catch (_) { return null; }
    }
}

/**
 * Build a matcher for lines without server offsets (context lines, file
 * previews). In regex mode the actual pattern is used; otherwise every plain
 * term from the query is matched literally, case-insensitively unless
 * `case:yes` is present. Returns null when nothing can be highlighted.
 * @param {string} query
 * @param {{regex?: boolean, references?: boolean}} [opts]
 * @returns {{re: RegExp}|null}
 */
function buildQueryMatcher(query, opts = {}) {
    if (!query) return null;
    if (opts.regex) {
        // MATCH CASE off sends the server `case=false`, i.e. `(?i)`.
        const re = compileRustRegex(opts.caseSensitive === false ? `(?i)${query}` : query);
        return re ? { re } : null;
    }
    // A references query is one identifier, matched exactly and case-sensitively.
    const parsed = opts.references
        ? { terms: [query.trim()], caseSensitive: true, wholeWord: true }
        : parseQueryTerms(query);
    if (opts.caseSensitive) parsed.caseSensitive = true;
    const terms = parsed.terms.filter(Boolean);
    if (!terms.length) return null;
    let src = terms.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
    if (parsed.wholeWord) src = `(?<![A-Za-z0-9_])(?:${src})(?![A-Za-z0-9_])`;
    try {
        return { re: new RegExp(src, parsed.caseSensitive ? 'g' : 'gi') };
    } catch (_) {
        return null;
    }
}

/** Character ranges in `text` matched by `matcher` (at most 500). */
function matcherRanges(text, matcher) {
    if (!matcher || !text) return [];
    const re = matcher.re;
    re.lastIndex = 0;
    const out = [];
    let m;
    while ((m = re.exec(text)) !== null) {
        if (m[0].length === 0) { re.lastIndex++; continue; }
        out.push([m.index, m.index + m[0].length]);
        if (out.length > 500) break;
    }
    re.lastIndex = 0;
    return out;
}

// ---------- Results header ----------

/**
 * The results-count badge. `total_matches` is present when the scan ran to
 * completion; when the match budget or deadline stopped it early the server
 * sets `truncated_by_budget` and the total is unknown, which is not something
 * a bigger page size fixes.
 * @param {object} data last page of the search response
 * @param {number} n results loaded so far (all pages)
 * @returns {{text: string, title: string}}
 */
function resultsCountLabel(data, n) {
    const files = Array.isArray(data.files) && data.files.length
        ? (typeof data.total_files === 'number' ? data.total_files : data.files.length)
        : 0;
    const label = contentCountLabel(data, n);
    if (!files) return label;
    // Path matches sit above the content results; count them too, or
    // "1 RESULT" over a box of two files reads as wrong.
    const f = `${Number(files).toLocaleString('en-US')} FILE${files !== 1 ? 'S' : ''}`;
    return { text: `${f} · ${label.text}`, title: label.title };
}

function contentCountLabel(data, n) {
    const num = (k) => Number(k).toLocaleString('en-US');
    const plural = (k) => `${num(k)} RESULT${k !== 1 ? 'S' : ''}`;
    if (typeof data.total_matches === 'number') {
        return n < data.total_matches
            ? { text: `${num(n)} OF ${plural(data.total_matches)}`, title: 'Use LOAD MORE to fetch the next page' }
            : { text: plural(n), title: '' };
    }
    if (data.truncated_by_budget) {
        const why = limitsHitDetail(data, ['match_budget', 'per_file_cap', 'timeout']);
        return {
            text: `${num(n)}+ RESULTS`,
            title: why
                ? `The total is unknown: ${why}. LOAD MORE runs it again with a higher limit.`
                : 'The search stopped at its match limit or time limit, so the total is unknown. LOAD MORE runs it again with a higher limit.',
        };
    }
    return data.has_more
        ? { text: `${num(n)}+ RESULTS`, title: 'More results are available' }
        : { text: plural(n), title: '' };
}

/**
 * The server's own words for the limits that cut a search
 * (`limits_hit`), joined, keeping only the named limits. '' when none.
 * @param {object} data search response
 * @param {string[]} names e.g. ['timeout']
 * @returns {string}
 */
function limitsHitDetail(data, names) {
    const hits = Array.isArray(data?.limits_hit) ? data.limits_hit : [];
    return hits.filter((h) => names.includes(h.limit)).map((h) => h.detail).join('; ');
}

/**
 * The request LOAD MORE sends after `loaded` hits, `pageSize` at a time.
 *
 * Offset paging is not stable when a search stops at its match budget: the
 * budget grows with `offset + max`, so a later page scans more files and
 * ranks a different set, and page 2 can repeat or skip hits from page 1.
 * Below the server's cap the whole list is fetched again at the bigger size
 * (`offset` 0) and replaces what is shown, so nothing repeats or goes
 * missing; hits already shown may move. Past the cap only offset paging is
 * possible.
 * @param {number} loaded hits on screen
 * @param {number} pageSize the search's page size (`max`)
 * @returns {{offset: number, max: number, redraw: boolean}}
 */
function loadMoreRequest(loaded, pageSize) {
    if (loaded < MAX_RESULTS_LIMIT) {
        return { offset: 0, max: Math.min(MAX_RESULTS_LIMIT, loaded + pageSize), redraw: true };
    }
    return { offset: loaded, max: pageSize, redraw: false };
}

/** "36 ms", or one decimal under 10 ms ("4.2 ms"). */
function formatDurationMs(ms) {
    const v = Number(ms) || 0;
    return `${v < 10 ? v.toFixed(1) : Math.round(v).toLocaleString('en-US')} ms`;
}

/**
 * The muted text beside the count: the time and whether case was ignored
 * ("36 ms · ignoring case"). The ranking mode and candidate-file count are
 * internals, so they live in the tooltip. The one case worth showing is fast
 * ranking skipping files, since results may then be missing ("Searched 2,000
 * of 12,000 files · 36 ms · ignoring case").
 * @param {object} data search response
 * @param {number} durationMs
 * @param {boolean} [ignoresCase] omitted = say nothing about case
 * @returns {{text: string, title: string}}
 */
function searchMetaLabel(data, durationMs, ignoresCase) {
    const caseNote = ignoresCase === undefined ? '' : (ignoresCase ? ' · ignoring case' : ' · matching case');
    // A time limit is the one cut LOAD MORE cannot undo, so it is shown.
    const timedOut = limitsHitDetail(data, ['timeout']) ? ' · stopped at time limit' : '';
    const time = formatDurationMs(durationMs) + caseNote + timedOut;
    const total = data.total_candidates;
    if (typeof total !== 'number') return { text: time, title: '' };
    const searched = typeof data.candidates_searched === 'number' ? data.candidates_searched : total;
    const num = (k) => k.toLocaleString('en-US');
    if (searched < total) {
        return {
            text: `Searched ${num(searched)} of ${num(total)} files · ${time}`,
            title: 'Fast ranking only opened the most likely files. SEARCH MORE FILES opens more of them; Ranking: Full under FILTER opens them all.',
        };
    }
    return {
        text: time,
        title: `${num(total)} file${total !== 1 ? 's' : ''} could contain the query`,
    };
}

/**
 * What SEARCH MORE FILES asks for when fast ranking skipped files: every
 * candidate, or as many as the server allows (`max_files_allowed`). Null
 * when nothing was skipped or the server's limit is already reached.
 * @param {object} data search response
 * @returns {{maxFiles: number, label: string}|null}
 */
function moreFilesRequest(data) {
    const total = data?.total_candidates;
    const searched = data?.candidates_searched;
    if (typeof total !== 'number' || typeof searched !== 'number' || searched >= total) return null;
    const allowed = typeof data.max_files_allowed === 'number' ? data.max_files_allowed : Infinity;
    if (searched >= allowed) return null;
    const n = Math.min(total, allowed);
    return {
        maxFiles: n,
        label: n === total ? 'SEARCH ALL FILES' : `SEARCH ${n.toLocaleString('en-US')} FILES`,
    };
}

/**
 * The `case` parameter for a search: 'true' with MATCH CASE on; with it
 * off, 'false' for a regex (the server matches a regex's case by default)
 * and null otherwise (plain text ignores case unless the query says
 * `case:yes`; references are always exact).
 * @param {{matchCase?: boolean, isRegex?: boolean, symbolsOnly?: boolean, isReferences?: boolean}} s
 * @returns {'true'|'false'|null}
 */
function searchCaseParam(s) {
    if (s.matchCase) return 'true';
    if (s.isRegex && !s.symbolsOnly && !s.isReferences) return 'false';
    return null;
}

/**
 * Does this search ignore case? For the results header, so it is always
 * clear which way case went.
 * @param {{query: string, matchCase?: boolean, isRegex?: boolean, isReferences?: boolean}} s
 */
function searchIgnoresCase(s) {
    if (s.matchCase || s.isReferences) return false;
    if (s.isRegex) return true;
    return !parseQueryTerms(s.query).caseSensitive;
}

/** "just now", "6 min ago", "2 h 5 min ago", "3 days ago". */
function formatAge(ms) {
    const secs = Math.max(0, Math.floor((Number(ms) || 0) / 1000));
    if (secs < 60) return 'just now';
    if (secs < 3600) return `${Math.floor(secs / 60)} min ago`;
    if (secs < 86400) return `${Math.floor(secs / 3600)} h ${Math.floor((secs % 3600) / 60)} min ago`;
    return `${Math.floor(secs / 86400)} days ago`;
}

/**
 * The freshness note beside the timing, or null when there is nothing to
 * warn about: a file watcher keeps the index current, or no scan has
 * finished yet (the progress panel covers the first build).
 * @param {{watching?: boolean, synced_at_ms?: number}|undefined} index `index` of a search response
 * @param {number} nowMs
 * @returns {{text: string, title: string}|null}
 */
function indexFreshnessLabel(index, nowMs) {
    if (!index || index.watching || typeof index.synced_at_ms !== 'number') return null;
    const when = new Date(index.synced_at_ms).toLocaleString();
    return {
        text: `Index synced ${formatAge(nowMs - index.synced_at_ms)} · watching off`,
        title: `Files written after ${when} may be missing from results or out of date, because file `
            + 'watching is off. Restart the server to rescan, or set watch = true under [indexer] '
            + 'to keep the index current.',
    };
}

// ---------- highlight.js output ----------

/**
 * Split highlight.js output into one HTML string per source line. hljs
 * spans may start on one line and end on a later one (block comments,
 * multi-line strings); such spans are closed at the line break and
 * re-opened on the next line so every line is a self-contained fragment.
 */
function splitHighlightedHtml(html) {
    const lines = [];
    const open = [];
    let cur = '';
    const re = /<span[^>]*>|<\/span>|[^<]+|</g;
    let m;
    while ((m = re.exec(html)) !== null) {
        const tok = m[0];
        if (tok.startsWith('<span')) {
            open.push(tok);
            cur += tok;
        } else if (tok === '</span>') {
            open.pop();
            cur += tok;
        } else {
            const parts = tok.split('\n');
            for (let i = 0; i < parts.length; i++) {
                if (i > 0) {
                    cur += '</span>'.repeat(open.length);
                    lines.push(cur);
                    cur = open.join('');
                }
                cur += parts[i];
            }
        }
    }
    lines.push(cur);
    return lines;
}

// Languages whose block comments are `/* ... */`.
const C_BLOCK_COMMENT_LANGS = new Set([
    'rust', 'javascript', 'typescript', 'java', 'c', 'cpp', 'csharp', 'go',
    'kotlin', 'swift', 'php', 'css', 'scss', 'less', 'objectivec',
]);

/**
 * A snippet cut from the middle of a file can start inside a block comment
 * (a doc comment whose opening is above the window). Highlighted on its own,
 * its prose is then coloured as code. Returns the text to put in front of
 * the snippet so the highlighter starts in the right state ('' if none):
 * the snippet is inside a C-style block comment when the first comment
 * delimiter in it closes one, or its first non-blank line is a ` * `
 * continuation line.
 * @param {string[]} lines - the snippet's lines
 * @param {string} lang - highlight.js language name
 */
function snippetHighlightPrefix(lines, lang) {
    if (!C_BLOCK_COMMENT_LANGS.has(lang)) return '';
    let firstText = null;
    for (const line of lines) {
        const open = line.indexOf('/*');
        const close = line.indexOf('*/');
        if (close !== -1 && (open === -1 || close < open)) return '/*\n';
        if (open !== -1) break;
        if (firstText === null && line.trim() !== '') firstText = line.trim();
    }
    // ` * text` or a bare `*`, but not `*ptr = ...` or `**kwargs`.
    return firstText === '*' || (firstText || '').startsWith('* ') ? '/*\n' : '';
}

// ---------- Regex search help ----------

/** Characters the server's regex engine treats as syntax. */
const REGEX_META = /[\\.^$|?*+()[\]{}]/g;

/** Escape every regex metacharacter so `text` matches literally. */
function escapeRegex(text) {
    return String(text).replace(REGEX_META, '\\$&');
}

/**
 * The literal text a regex matches when it has no regex features beyond
 * escaped punctuation (`\{ "success": True` → `{ "success": True`), or
 * null when it uses classes, groups, repetition and the like.
 */
function regexToLiteral(pattern) {
    let out = '';
    const src = String(pattern || '');
    for (let i = 0; i < src.length; i++) {
        const c = src[i];
        if (c === '\\') {
            const next = src[i + 1];
            if (next === undefined || /[A-Za-z0-9]/.test(next)) return null;
            out += next;
            i++;
        } else if ('.^$|?*+()[]{}'.includes(c)) {
            return null;
        } else {
            out += c;
        }
    }
    return out;
}

/**
 * `pattern` with every run of literal spaces (outside `[...]`) replaced by
 * `\s*`, so `\{ "a": 1` also finds `{"a":1` and `{  "a":  1`. Null when the
 * pattern has no such spaces.
 */
function relaxRegexSpacing(pattern) {
    const src = String(pattern || '');
    let out = '';
    let inClass = false;
    let changed = false;
    for (let i = 0; i < src.length; i++) {
        const c = src[i];
        if (c === '\\') {
            out += c + (src[i + 1] ?? '');
            i++;
        } else if (inClass) {
            if (c === ']') inClass = false;
            out += c;
        } else if (c === '[') {
            inClass = true;
            out += c;
        } else if (c === ' ') {
            while (src[i + 1] === ' ') i++;
            out += '\\s*';
            changed = true;
        } else {
            out += c;
        }
    }
    return changed ? out : null;
}

/** Does the pattern already switch case-insensitivity on at its start? */
function regexIgnoresCase(pattern) {
    return /^\(\?[a-zA-Z]*i[a-zA-Z]*\)/.test(String(pattern || ''));
}

/** Does a plain-text query look like it was meant as a regex? */
function looksLikeRegex(query) {
    return /\\[wdsbWDSB.(){}[\]|+*?^$]|\.[*+?]|\[[^\]\s]+\]|\w\|\w|^\^|\$$/.test(String(query || ''));
}

/**
 * Alternative searches to offer when a search finds nothing. Each is
 * `{label, query, regex}`; the caller runs them and shows the ones that
 * find something.
 * @param {string} query
 * @param {boolean} isRegex
 * @param {boolean} [matchCase] MATCH CASE on (default true: suggest ignoring case)
 * @returns {Array<{label: string, query: string, regex: boolean}>}
 */
function noResultSuggestions(query, isRegex, matchCase = true) {
    const q = String(query || '').trim();
    if (!q) return [];
    const out = [];
    const add = (label, text, regex) => {
        const same = (a, b) => a.query === b.query && a.regex === b.regex;
        const candidate = { label, query: text, regex };
        if (text && !same(candidate, { query: q, regex: isRegex }) && !out.some(s => same(s, candidate))) {
            out.push(candidate);
        }
    };
    if (!isRegex) {
        if (looksLikeRegex(q)) add('Search as regex', q, true);
        return out;
    }
    const relaxed = relaxRegexSpacing(q);
    // With MATCH CASE off the regex already ignores case.
    const caseable = matchCase && /[A-Za-z]/.test(q) && !regexIgnoresCase(q);
    if (relaxed) add('Flexible spacing', relaxed, true);
    if (caseable) add('Ignore case', `(?i)${q}`, true);
    if (relaxed && caseable) add('Flexible spacing, ignore case', `(?i)${relaxed}`, true);
    const literal = regexToLiteral(q);
    if (literal !== null && literal.trim().length >= 3) add('Search as plain text', literal, false);
    return out;
}

/**
 * Ways out of a regex that does not compile: search the same text without
 * regex, or with every metacharacter escaped.
 */
function regexErrorSuggestions(pattern) {
    const q = String(pattern || '').trim();
    if (!q) return [];
    const out = [{ label: 'Search as plain text', query: q, regex: false }];
    const escaped = escapeRegex(q);
    if (escaped !== q) out.push({ label: 'Escape special characters', query: escaped, regex: true });
    return out;
}

/**
 * A path-filter box's text as the API's `;`-separated list. The box also
 * takes commas, except inside `{…}` where a comma is glob alternation
 * (`*.{rs,toml}`). Only the web UI accepts commas; the API, CLI and MCP keep
 * `;`, so a comma there stays literal as in ripgrep's -g.
 */
function pathFilterParam(text) {
    let depth = 0;
    let out = '';
    for (const ch of String(text || '')) {
        if (ch === '{') depth++;
        else if (ch === '}' && depth > 0) depth--;
        out += ch === ',' && depth === 0 ? ';' : ch;
    }
    return out.split(';').map(s => s.trim()).filter(Boolean).join(';');
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        MAX_RESULTS_LIMIT,
        CONTEXT_LINES_LIMIT,
        clampMaxResults,
        pickMaxResultsOption,
        clampContextLines,
        parseBoolParam,
        iconSvg,
        hljsLangForPath,
        hasImportGraph,
        graphExplorerUrl,
        parseColorToRgb,
        relativeLuminance,
        contrastRatio,
        byteRangeToCharRange,
        mergeRanges,
        pathMatchRuns,
        fileMatchesLabel,
        isYesToken,
        tokenizeQuery,
        parseQueryTerms,
        moreFilesRequest,
        searchCaseParam,
        searchIgnoresCase,
        compileRustRegex,
        buildQueryMatcher,
        matcherRanges,
        splitHighlightedHtml,
        snippetHighlightPrefix,
        fileViewHash,
        identifierAt,
        definitionQuery,
        rankDefinitions,
        parseFileViewHash,
        resultsCountLabel,
        limitsHitDetail,
        loadMoreRequest,
        formatDurationMs,
        searchMetaLabel,
        formatAge,
        indexFreshnessLabel,
        escapeRegex,
        regexToLiteral,
        relaxRegexSpacing,
        regexIgnoresCase,
        looksLikeRegex,
        noResultSuggestions,
        regexErrorSuggestions,
        pathFilterParam,
    };
}
