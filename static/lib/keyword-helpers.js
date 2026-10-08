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
    const ext = (String(filePath || '').split('.').pop() || '').toLowerCase();
    const MAP = {
        rs: 'rust', py: 'python', js: 'javascript', mjs: 'javascript', cjs: 'javascript',
        ts: 'typescript', tsx: 'typescript', jsx: 'javascript',
        go: 'go', rb: 'ruby', java: 'java', cs: 'csharp', cpp: 'cpp', cc: 'cpp',
        cxx: 'cpp', c: 'c', h: 'c', hpp: 'cpp', php: 'php', sh: 'bash',
        bash: 'bash', zsh: 'bash', toml: 'toml', yaml: 'yaml', yml: 'yaml',
        json: 'json', xml: 'xml', html: 'xml', css: 'css', scss: 'scss',
        md: 'markdown', sql: 'sql', kt: 'kotlin', swift: 'swift', r: 'r',
        lua: 'lua', pl: 'perl', pm: 'perl', hs: 'haskell', ex: 'elixir',
        exs: 'elixir', erl: 'erlang', scala: 'scala', dart: 'dart',
        proto: 'protobuf', dockerfile: 'dockerfile', makefile: 'makefile',
    };
    return MAP[ext] || 'plaintext';
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

// ---------- Query syntax (mirrors src/search/query_syntax.rs) ----------

function isYesToken(v) {
    return ['yes', 'y', 'true', '1', 'on'].includes(String(v).toLowerCase());
}

/**
 * Split on whitespace, keeping "quoted phrases" together (quotes removed).
 * Each token records where its first quote stood (`quoteAt`), which decides
 * whether a leading `-` negates and whether an operator prefix counts.
 * @returns {Array<{text: string, quoteAt: number|null}>}
 */
function tokenizeQuery(raw) {
    const out = [];
    let cur = '';
    let quoteAt = null;
    let inQuotes = false;
    for (const c of String(raw || '')) {
        if (c === '"') {
            inQuotes = !inQuotes;
            if (quoteAt === null) quoteAt = cur.length;
        } else if (!inQuotes && /\s/.test(c)) {
            if (cur) {
                out.push({ text: cur, quoteAt });
                cur = '';
            }
            quoteAt = null;
        } else {
            cur += c;
        }
    }
    if (cur) out.push({ text: cur, quoteAt });
    return out;
}

/**
 * Does a leading `-` negate this token? Only when a letter, `_` or a quote
 * follows it: `-Wall` and `-"a b"` negate, `->`, `-1.5` and `"-foo"` do not.
 */
function tokenNegates(token) {
    if (!token.text.startsWith('-')) return false;
    if (token.quoteAt === 0) return false; // `"-foo"` is the literal term `-foo`
    if (token.quoteAt === 1) return true;  // `-"a b"`
    return /^[A-Za-z_]/.test(token.text.slice(1));
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
 * Parse a plain-text query exactly like src/search/query_syntax.rs:
 * whitespace-separated tokens, "quoted phrases" kept together (quotes
 * removed); `file:`/`lang:`/`-file:`/`-lang:` become path filters (dropped
 * here), `case:`/`word:` set options, `-term` excludes. A `-` negates only
 * before a letter, `_` or a quote, so `->`, `-1.5` and a lone `-` are
 * terms; a quoted token is literal (`"file:"` is the term `file:`) while a
 * quote after the colon still belongs to the operator (`file:"my dir"`).
 * An operator without an argument, or `lang:` with a name the server
 * rejects, is a plain term.
 * @returns {{terms: string[], excludeTerms: string[], caseSensitive: boolean, wholeWord: boolean}}
 */
function parseQueryTerms(raw) {
    const terms = [];
    const excludeTerms = [];
    let caseSensitive = false;
    let wholeWord = false;

    for (const token of tokenizeQuery(raw)) {
        const negated = tokenNegates(token);
        const body = negated ? token.text.slice(1) : token.text;
        const quoteAt = negated && token.quoteAt !== null ? token.quoteAt - 1 : token.quoteAt;
        // `prefix` is an operator only when no quote precedes its colon and
        // an argument follows it.
        const operator = (prefix) => {
            if (!body.startsWith(prefix)) return null;
            const arg = body.slice(prefix.length);
            if (quoteAt !== null && quoteAt < prefix.length) return null;
            return arg ? arg : null;
        };
        if (operator('file:') !== null) continue;
        const lang = operator('lang:');
        if (lang !== null && isKnownLang(lang)) continue;
        const cs = operator('case:');
        if (cs !== null) { caseSensitive = isYesToken(cs); continue; }
        const ww = operator('word:');
        if (ww !== null) { wholeWord = isYesToken(ww); continue; }
        if (negated) excludeTerms.push(body);
        else if (body) terms.push(body);
    }
    return { terms, excludeTerms, caseSensitive, wholeWord };
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
        const re = compileRustRegex(query);
        return re ? { re } : null;
    }
    // A references query is one identifier, matched exactly and case-sensitively.
    const parsed = opts.references
        ? { terms: [query.trim()], caseSensitive: true, wholeWord: true }
        : parseQueryTerms(query);
    const terms = parsed.terms.filter(Boolean);
    if (!terms.length) return null;
    // Longest first so "foobar" wins over "foo" in the alternation.
    terms.sort((a, b) => b.length - a.length);
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
 * @returns {Array<{label: string, query: string, regex: boolean}>}
 */
function noResultSuggestions(query, isRegex) {
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
    const caseable = /[A-Za-z]/.test(q) && !regexIgnoresCase(q);
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
        parseColorToRgb,
        relativeLuminance,
        contrastRatio,
        byteRangeToCharRange,
        mergeRanges,
        isYesToken,
        tokenizeQuery,
        tokenNegates,
        parseQueryTerms,
        compileRustRegex,
        buildQueryMatcher,
        matcherRanges,
        splitHighlightedHtml,
        escapeRegex,
        regexToLiteral,
        relaxRegexSpacing,
        regexIgnoresCase,
        looksLikeRegex,
        noResultSuggestions,
        regexErrorSuggestions,
    };
}
