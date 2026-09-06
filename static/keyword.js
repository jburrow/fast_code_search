// ============================================
// KEYWORD SEARCH - View-specific logic
// Uses: common.js
// ============================================

const API_BASE = '';

// DOM Elements
const queryInput = document.getElementById('query');
const maxResultsSelect = document.getElementById('max-results');
const includeFilterInput = document.getElementById('include-filter');
const excludeFilterInput = document.getElementById('exclude-filter');
const regexModeCheckbox = document.getElementById('regex-mode');
const symbolsModeCheckbox = document.getElementById('symbols-mode');
const referencesModeCheckbox = document.getElementById('references-mode');
const rankModeSelect = document.getElementById('rank-mode');
const contextLinesSelect = document.getElementById('context-lines');
const resultsContainer = document.getElementById('results');
const resultsHeader = document.getElementById('results-header');
const resultsCount = document.getElementById('results-count');
const rankingInfoEl = document.getElementById('ranking-info');
const searchTimeEl = document.getElementById('search-time');
const searchHistoryDropdown = document.getElementById('search-history-dropdown');

// Stats elements
const statFiles = document.getElementById('stat-files');
const statSize = document.getElementById('stat-size');
const statTrigrams = document.getElementById('stat-trigrams');
const statDeps = document.getElementById('stat-deps');

// Progress elements
const progressPanel = document.getElementById('progress-panel');
const progressBar = document.getElementById('progress-bar');
const progressPercent = document.getElementById('progress-percent');
const progressStatus = document.getElementById('progress-status');
const progressMessage = document.getElementById('progress-message');

// Search state
const DEBOUNCE_MS = 300;

// ============================================
// LOCAL STORAGE PERSISTENCE
// ============================================

const LS_SETTINGS_KEY = 'fcs_settings';
const LS_HISTORY_KEY = 'fcs_history';
const MAX_HISTORY = 50;

/**
 * Save all current settings to localStorage.
 */
function saveSettingsToStorage() {
    try {
        const settings = {
            max: maxResultsSelect?.value || '50',
            rank: rankModeSelect?.value || 'auto',
            context: contextLinesSelect?.value || '0',
            include: includeFilterInput?.value.trim() || '',
            exclude: excludeFilterInput?.value.trim() || '',
            regex: regexModeCheckbox?.checked || false,
            symbols: symbolsModeCheckbox?.checked || false,
            references: referencesModeCheckbox?.checked || false,
        };
        localStorage.setItem(LS_SETTINGS_KEY, JSON.stringify(settings));
    } catch (_) { /* storage unavailable */ }
}

/**
 * Load settings from localStorage and apply to form fields.
 * URL params take precedence over stored settings (applied afterwards).
 */
function loadSettingsFromStorage() {
    try {
        const raw = localStorage.getItem(LS_SETTINGS_KEY);
        if (!raw) return;
        const s = JSON.parse(raw);
        if (s.max != null) applyMaxResults(s.max);
        if (s.rank != null) setSelectValue(rankModeSelect, s.rank);
        if (s.context != null) setSelectValue(contextLinesSelect, s.context);
        if (includeFilterInput && s.include) includeFilterInput.value = s.include;
        if (excludeFilterInput && s.exclude) excludeFilterInput.value = s.exclude;
        if (regexModeCheckbox) regexModeCheckbox.checked = s.regex === true;
        if (symbolsModeCheckbox) symbolsModeCheckbox.checked = s.symbols === true;
        if (referencesModeCheckbox) referencesModeCheckbox.checked = s.references === true;
    } catch (_) { /* ignore parse errors */ }
    syncToggleVisuals();
}

// ============================================
// FORM STATE HELPERS
// ============================================

const MAX_RESULTS_LIMIT = 1000; // server clamps `max` to 1..1000

/**
 * Select `value` in `select` only when such an option exists, so an unknown
 * value from a URL or stale storage can never leave the select in a state
 * that serialises to NaN or an unsupported mode.
 * @returns {boolean} true when applied
 */
function setSelectValue(select, value) {
    if (!select) return false;
    const v = String(value);
    const has = Array.from(select.options).some(o => o.value === v);
    if (has) select.value = v;
    return has;
}

/**
 * Apply a requested result cap: parse, clamp to 1..1000, then pick the
 * smallest select option that is >= the request (or the largest option).
 */
function applyMaxResults(value) {
    if (!maxResultsSelect) return;
    const n = parseInt(value, 10);
    if (!Number.isFinite(n)) return;
    const clamped = Math.min(MAX_RESULTS_LIMIT, Math.max(1, n));
    const options = Array.from(maxResultsSelect.options)
        .map(o => parseInt(o.value, 10))
        .filter(Number.isFinite)
        .sort((a, b) => a - b);
    const pick = options.find(o => o >= clamped) ?? options[options.length - 1];
    if (pick !== undefined) maxResultsSelect.value = String(pick);
}

/** The validated result cap to send: always an integer in 1..1000. */
function currentMaxResults() {
    const n = parseInt(maxResultsSelect?.value, 10);
    if (!Number.isFinite(n)) return 50;
    return Math.min(MAX_RESULTS_LIMIT, Math.max(1, n));
}

/** The validated context-line count to send (0..10). */
function currentContextLines() {
    const n = parseInt(contextLinesSelect?.value || '0', 10);
    if (!Number.isFinite(n)) return 0;
    return Math.min(10, Math.max(0, n));
}

function parseBoolParam(value) {
    return ['true', '1', 'yes', 'on'].includes(String(value).toLowerCase());
}

/** The mode checkboxes; each is mutually exclusive with the others. */
function modeCheckboxes() {
    return [regexModeCheckbox, symbolsModeCheckbox, referencesModeCheckbox].filter(Boolean);
}

/**
 * Derive the toggle label styling from the checkbox state. This is the only
 * place that sets it, so a mode set from the URL, storage, history navigation
 * or a click always looks the way it behaves.
 */
function syncToggleVisuals() {
    modeCheckboxes().forEach(cb => {
        const label = cb.closest('label');
        if (label) label.classList.toggle('toggle-on', cb.checked);
    });
}

// ============================================
// SEARCH HISTORY
// ============================================

// ============================================
// SEARCH HISTORY (uses shared utilities from common.js)
// ============================================

function loadHistory() {
    return loadSearchHistory(LS_HISTORY_KEY);
}

function saveToHistory(query) {
    saveSearchHistory(LS_HISTORY_KEY, query, MAX_HISTORY);
}

function clearHistory() {
    clearSearchHistory(LS_HISTORY_KEY);
    hideHistoryDropdown();
}

function showHistoryDropdown(filter) {
    showSearchHistoryDropdown(searchHistoryDropdown, queryInput, LS_HISTORY_KEY,
        (selectedQuery) => {
            queryInput.value = selectedQuery;
            submitSearch();
        },
        () => showHistoryDropdown(queryInput.value.trim())
    );
}

function hideHistoryDropdown() {
    hideSearchHistoryDropdown(searchHistoryDropdown);
}

function navigateHistoryDropdown(dir) {
    return navigateSearchHistoryDropdown(searchHistoryDropdown, queryInput, dir);
}

// ============================================
// URL STATE
// ============================================

/**
 * Populate form fields from URL query parameters.
 *
 * A parameter that is present fully determines its field: `regex=false`
 * turns regex mode OFF even if it was on from storage. A parameter that is
 * absent leaves the field alone on the initial load (so stored settings
 * apply) but resets it to the default when `absentIsDefault` is set, which
 * is what history navigation needs since the URL is the complete state of
 * the entry being restored.
 *
 * @param {URLSearchParams} params
 * @param {{absentIsDefault?: boolean}} [opts]
 */
function applyUrlState(params, opts = {}) {
    const absentIsDefault = opts.absentIsDefault === true;
    const text = (input, key) => {
        if (!input) return;
        if (params.has(key)) input.value = params.get(key);
        else if (absentIsDefault) input.value = '';
    };
    const select = (sel, key, fallback) => {
        if (!sel) return;
        if (params.has(key)) {
            if (!setSelectValue(sel, params.get(key)) && absentIsDefault) setSelectValue(sel, fallback);
        } else if (absentIsDefault) {
            setSelectValue(sel, fallback);
        }
    };
    const mode = (cb, key) => {
        if (!cb) return;
        if (params.has(key)) cb.checked = parseBoolParam(params.get(key));
        else if (absentIsDefault) cb.checked = false;
    };

    text(queryInput, 'q');
    text(includeFilterInput, 'include');
    text(excludeFilterInput, 'exclude');
    if (params.has('max')) applyMaxResults(params.get('max'));
    else if (absentIsDefault) setSelectValue(maxResultsSelect, '50');
    select(rankModeSelect, 'rank', 'auto');
    select(contextLinesSelect, 'context', '0');
    mode(regexModeCheckbox, 'regex');
    mode(symbolsModeCheckbox, 'symbols');
    mode(referencesModeCheckbox, 'references');
    syncToggleVisuals();

    // Auto-open the VISIBLE filter panel when a shared URL carries filter params,
    // so the applied filters are discoverable.
    const hasAdvanced = params.has('include') || params.has('exclude') ||
        params.has('rank') || params.has('max') || params.has('context');
    if (hasAdvanced) {
        const filterPanel = document.getElementById('filter-panel');
        if (filterPanel) filterPanel.classList.add('open');
    }
}

/** Serialise the current form state; defaults are omitted to keep URLs short. */
function currentUrlParams() {
    const query = queryInput.value.trim();
    const params = new URLSearchParams();

    if (query) params.set('q', query);

    const max = currentMaxResults();
    if (max !== 50) params.set('max', String(max));

    const include = includeFilterInput?.value.trim() || '';
    if (include) params.set('include', include);

    const exclude = excludeFilterInput?.value.trim() || '';
    if (exclude) params.set('exclude', exclude);

    if (regexModeCheckbox?.checked) params.set('regex', 'true');
    if (symbolsModeCheckbox?.checked) params.set('symbols', 'true');
    if (referencesModeCheckbox?.checked) params.set('references', 'true');

    const rank = rankModeSelect?.value || 'auto';
    if (rank !== 'auto') params.set('rank', rank);

    const context = currentContextLines();
    if (context !== 0) params.set('context', String(context));

    return params;
}

/**
 * Write the current form state into the URL.
 *
 * Every distinct search gets its own history entry so Back restores the
 * previous query. Typing produces one entry per "edit session": the first
 * debounced keystroke after a submitted search pushes, later keystrokes
 * replace that same entry (flagged `typed` in history.state), and an
 * explicit submit turns the entry into a submitted one.
 *
 * @param {'input'|'submit'|'option'|'history'} trigger
 */
function writeUrlState(trigger) {
    if (trigger === 'history') return; // the URL already is the state
    const qs = currentUrlParams().toString();
    const url = qs ? `${location.pathname}?${qs}` : location.pathname;
    const current = location.search.replace(/^\?/, '');
    const typed = trigger === 'input';
    const state = { typed };
    if (qs === current) {
        history.replaceState(typed && history.state?.typed ? state : { typed: false }, '', url);
    } else if (typed && history.state?.typed) {
        history.replaceState(state, '', url);
    } else {
        history.pushState(state, '', url);
    }
}

// Back/Forward: the URL is the full state of that entry, so absent params
// mean defaults (a mode that is not in the URL is off).
window.addEventListener('popstate', () => {
    debouncedSearch.cancel();
    applyUrlState(new URLSearchParams(location.search), { absentIsDefault: true });
    performSearch({ trigger: 'history' });
});

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    resultsContainerId: 'results',
    searchSectionId: 'search-section',
    additionalInputIds: ['include-filter', 'exclude-filter', 'max-results', 'regex-mode', 'symbols-mode', 'references-mode', 'rank-mode', 'context-lines'],
    onReadyChange: (isReady, status) => {
        if (isReady && queryInput.value.trim()) {
            // If user typed while waiting, trigger search now
            performSearch({ trigger: 'option' });
        }
    }
});

// ============================================
// BACKEND HEALTH
// ============================================

let keywordAvailable = false;

/**
 * Check that the keyword backend is serving this page and probe the semantic
 * backend for the status badge.  Updates the banner and sets keywordAvailable.
 */
async function checkBackendHealth() {
    const hostname = window.location.hostname;

    // Probe the server serving this page.
    // If it responds OK we consider the keyword backend available regardless of
    // whether the server_type field is present (older binaries omit it).
    let currentServerType = null;
    let healthOk = false;
    try {
        const resp = await fetch('/api/health', { signal: AbortSignal.timeout(2000) });
        if (resp.ok) {
            healthOk = true;
            const data = await resp.json();
            currentServerType = data.server_type ?? null;
        }
    } catch (e) { /* offline */ }

    // Available if health OK and not explicitly identified as a different server type
    keywordAvailable = healthOk && currentServerType !== 'semantic';

    if (!keywordAvailable) {
        searchReadiness.setOffline(true);
    }

    // Check semantic backend for the badge (non-blocking side-info).
    // Use the page's own protocol so this works under https (mixed-content
    // requests to http:// are blocked). The port is overridable via
    // window.SEMANTIC_PORT for non-default deployments.
    let semanticUp = false;
    try {
        const semanticPort = window.SEMANTIC_PORT || 8081;
        const resp = await fetch(`${window.location.protocol}//${hostname}:${semanticPort}/api/health`, { signal: AbortSignal.timeout(2000) });
        semanticUp = resp.ok;
    } catch (e) { /* offline */ }

    renderBackendStatus(keywordAvailable, semanticUp);
}

function renderBackendStatus(keywordUp, semanticUp) {
    const banner = document.getElementById('backend-banner');
    if (!banner) return;

    updateBackendBadge('keyword-status-badge', keywordUp, 'KEYWORD');
    updateBackendBadge('semantic-status-badge', semanticUp, 'SEMANTIC');

    if (keywordUp) {
        banner.style.display = 'none';
        return;
    }

    const msgEl = document.getElementById('backend-banner-msg');
    banner.style.background = '#ffdad6';
    banner.style.color = '#93000a';
    banner.style.display = 'flex';
    if (msgEl) {
        msgEl.textContent = 'Keyword search backend not running \u2014 start fast_code_search on port 8080';
    }
}

function updateBackendBadge(id, isUp, label) {
    const el = document.getElementById(id);
    if (!el) return;
    el.textContent = `${label}: ${isUp ? '\u2713' : '\u2717'}`;
    el.style.background = isUp ? '#a9efed' : '#ffdad6';
    el.style.borderColor = isUp ? '#1e6868' : '#ba1a1a';
    el.style.color = isUp ? '#00201f' : '#93000a';
}

// ============================================
// STATS & STATUS
// ============================================

async function fetchStats() {
    try {
        const response = await fetch(`${API_BASE}/api/stats`);
        if (!response.ok) throw new Error('Failed to fetch stats');
        
        const stats = await response.json();
        updateStat('stat-files', formatNumber(stats.num_files));
        updateStat('stat-content', formatBytes(stats.total_content_bytes || 0));
        updateStat('stat-size', formatBytes(stats.total_size));
        updateStat('stat-trigrams', formatNumber(stats.num_trigrams));
        updateStat('stat-deps', formatNumber(stats.dependency_edges || 0));
    } catch (error) {
        console.error('Failed to fetch stats:', error);
        ['stat-files', 'stat-content', 'stat-size', 'stat-trigrams', 'stat-deps'].forEach(id => updateStat(id, '-'));
    }
}

// Progress WebSocket instance (real-time updates)
const progressWS = new ProgressWebSocket({
    onUpdate: updateProgressUI,
    onConnected: () => {
        // WS connected means the server is reachable — clear any offline state
        // and re-check health so a banner shown at page load (server was down
        // then) is cleared once the server recovers.
        searchReadiness.setOffline(false);
        checkBackendHealth();
    },
    onDisconnected: () => {},
    onServerOffline: () => {
        // Consecutive WS failures — the keyword search server is not running.
        searchReadiness.setOffline(true);
        // Refresh the banner so it reflects the now-offline server.
        checkBackendHealth();
    },
    onError: (err) => {
        console.error('Progress WebSocket error:', err);
        // Progress will continue via reconnection
    }
});

let _progressHideTimer = null;

function updateProgressUI(status) {
    const isIdle = status.status === 'idle';
    const isCompleted = status.status === 'completed';

    // Update search readiness based on status
    searchReadiness.update(status);

    // Show the panel while indexing. On 'completed', keep it visible briefly then
    // hide it — a permanent 100% "Complete" bar otherwise reads as "stuck". 'idle'
    // hides immediately.
    if (isCompleted) {
        toggleElement('progress-panel', true, 'flex');
        clearTimeout(_progressHideTimer);
        _progressHideTimer = setTimeout(
            () => toggleElement('progress-panel', false, 'flex'),
            4000
        );
    } else {
        clearTimeout(_progressHideTimer);
        toggleElement('progress-panel', !isIdle, 'flex');
    }

    if (progressBar) {
        progressBar.style.width = `${status.progress_percent}%`;
        progressBar.className = `progress-fill ${isCompleted ? 'completed' : ''}`;
    }
    
    updateStat('progress-percent', `${status.progress_percent}%`);
    
    if (progressStatus) {
        const labels = {
            'idle': 'Ready',
            'loading_index': 'Loading',
            'discovering': 'Discovering',
            'indexing': 'Indexing',
            'reconciling': 'Reconciling',
            'resolving_imports': 'Resolving',
            'completed': 'Complete'
        };
        progressStatus.textContent = labels[status.status] || status.status;
        progressStatus.className = `status-badge status-${status.status}`;
    }
    
    if (progressMessage) {
        progressMessage.textContent = status.message || '';
    }
    
    // Update stats from WebSocket message (no separate HTTP request needed)
    if (status.num_files !== undefined) {
        updateStat('stat-files', formatNumber(status.num_files));
        updateStat('stat-content', formatBytes(status.total_content_bytes || 0));
        updateStat('stat-size', formatBytes(status.total_size || 0));
        updateStat('stat-trigrams', formatNumber(status.num_trigrams || 0));
        updateStat('stat-deps', formatNumber(status.dependency_edges || 0));
    }
}

// ============================================
// SYNTAX HIGHLIGHTING HELPERS
// ============================================

/**
 * Map a file path's extension to a highlight.js language name.
 * Falls back to 'plaintext' when unknown.
 */
function hljsLangForPath(filePath) {
    const ext = (filePath.split('.').pop() || '').toLowerCase();
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

const LANG_BADGE_STYLE_CACHE = new Map();

/**
 * Parse a CSS color string to RGB object for contrast calculations.
 * Supports #rgb, #rrggbb, rgb(), and rgba().
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

function relativeLuminance(rgb) {
    return 0.2126 * toLinearChannel(rgb.r)
        + 0.7152 * toLinearChannel(rgb.g)
        + 0.0722 * toLinearChannel(rgb.b);
}

function contrastRatio(l1, l2) {
    const lighter = Math.max(l1, l2);
    const darker = Math.min(l1, l2);
    return (lighter + 0.05) / (darker + 0.05);
}

/**
 * Build a readable language badge style from the configured language color.
 */
function getLangBadgeStyle(langClass) {
    if (!langClass) return 'background:#e7e3ce;color:#1d1c0f;border:1px solid #cbc8aa';
    if (LANG_BADGE_STYLE_CACHE.has(langClass)) return LANG_BADGE_STYLE_CACHE.get(langClass);

    const cssValue = getComputedStyle(document.documentElement)
        .getPropertyValue(`--lang-${langClass}`)
        .trim() || '#e7e3ce';
    const rgb = parseColorToRgb(cssValue);
    if (!rgb) {
        const fallback = `background:${cssValue};color:#000;border:1px solid rgba(0,0,0,0.2)`;
        LANG_BADGE_STYLE_CACHE.set(langClass, fallback);
        return fallback;
    }

    const bgLum = relativeLuminance(rgb);
    const blackContrast = contrastRatio(bgLum, 0);
    const whiteContrast = contrastRatio(bgLum, 1);
    const textColor = whiteContrast > blackContrast ? '#fff' : '#000';
    const borderColor = textColor === '#fff' ? 'rgba(255,255,255,0.38)' : 'rgba(0,0,0,0.2)';

    const style = `background:${cssValue};color:${textColor};border:1px solid ${borderColor}`;
    LANG_BADGE_STYLE_CACHE.set(langClass, style);
    return style;
}

/**
 * Highlight a `<pre>` element using highlight.js.
 * @param {HTMLElement} el - the <pre> element
 * @param {string} filePath - used to pick the language
 */
function applyHljs(el, filePath) {
    if (typeof hljs === 'undefined') return;
    const lang = hljsLangForPath(filePath);
    el.className = `language-${lang}`;
    hljs.highlightElement(el);
}

// ============================================
// MATCH HIGHLIGHTING
//
// The server reports the matched range of every hit as UTF-8 byte offsets
// (`match_start`/`match_end` into `content`, `line_match_start`/`line_match_end`
// into the full line). The match line is highlighted from those offsets, so
// regex hits, case-sensitive hits and `file:`/`lang:`-qualified queries are
// marked exactly where the engine matched. Context lines and file previews
// carry no offsets; for those the query is tokenised the way the server's
// query_syntax.rs does (operators dropped, "phrases" unquoted, case:yes
// honoured) and every remaining term is marked.
// ============================================

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

/**
 * Wrap the given character ranges of `el`'s text content in
 * <mark class="highlight">, walking text nodes with a running offset so the
 * marks survive (and nest inside) highlight.js spans.
 * @param {Element} el
 * @param {Array<[number, number]>} ranges - [start, end) offsets into el.textContent
 */
function markRanges(el, ranges) {
    const merged = mergeRanges(ranges);
    if (!merged.length) return;
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    const nodes = [];
    let node;
    while ((node = walker.nextNode())) nodes.push(node);

    let pos = 0;
    let ri = 0;
    for (const textNode of nodes) {
        const text = textNode.nodeValue;
        const start = pos;
        const end = pos + text.length;
        pos = end;
        while (ri < merged.length && merged[ri][1] <= start) ri++;
        if (ri >= merged.length) break;
        if (merged[ri][0] >= end) continue;

        const frag = document.createDocumentFragment();
        let last = 0;
        for (let k = ri; k < merged.length && merged[k][0] < end; k++) {
            const s = Math.max(merged[k][0], start) - start;
            const e = Math.min(merged[k][1], end) - start;
            if (s > last) frag.appendChild(document.createTextNode(text.slice(last, s)));
            const mark = document.createElement('mark');
            mark.className = 'highlight';
            mark.textContent = text.slice(s, e);
            frag.appendChild(mark);
            last = e;
        }
        if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
        textNode.parentNode.replaceChild(frag, textNode);
    }
}

function isYesToken(v) {
    return ['yes', 'y', 'true', '1', 'on'].includes(v.toLowerCase());
}

/**
 * Tokenise a plain-text query exactly like src/search/query_syntax.rs:
 * whitespace-separated, "quoted phrases" kept together (quotes removed),
 * `file:`/`lang:`/`-file:`/`-lang:` and `-term` dropped, `case:`/`word:`
 * consumed as options. A lone `-` or `-123` is a term, not a negation.
 * @returns {{terms: string[], caseSensitive: boolean, wholeWord: boolean}}
 */
function parseQueryTerms(raw) {
    const tokens = [];
    let cur = '';
    let inQuotes = false;
    for (const c of raw || '') {
        if (c === '"') inQuotes = !inQuotes;
        else if (!inQuotes && /\s/.test(c)) { if (cur) { tokens.push(cur); cur = ''; } }
        else cur += c;
    }
    if (cur) tokens.push(cur);

    const terms = [];
    let caseSensitive = false;
    let wholeWord = false;
    for (const tok of tokens) {
        let negated = false;
        let body = tok;
        if (tok.startsWith('-')) {
            const rest = tok.slice(1);
            if (rest && !/^\d+$/.test(rest)) { negated = true; body = rest; }
        }
        if (body.startsWith('file:') || body.startsWith('lang:')) continue;
        if (body.startsWith('case:')) { caseSensitive = isYesToken(body.slice(5)); continue; }
        if (body.startsWith('word:')) { wholeWord = isYesToken(body.slice(5)); continue; }
        if (negated) continue;
        if (body) terms.push(body);
    }
    return { terms, caseSensitive, wholeWord };
}

/**
 * Best-effort translation of a Rust `regex` pattern to a JS RegExp: leading
 * inline flags `(?is)` become RegExp flags and `(?P<name>` becomes `(?<name>`.
 * Returns null when the pattern does not compile in JS.
 */
function compileRustRegex(pattern) {
    let flags = 'g';
    let src = pattern;
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

/** Character ranges in `text` matched by `matcher`. */
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

/**
 * Highlight query terms inside an already syntax-highlighted element, plus
 * any extra character ranges (the server-reported match).
 */
function highlightTermsIn(el, matcher, extraRanges = []) {
    const ranges = matcherRanges(el.textContent, matcher).concat(extraRanges);
    markRanges(el, ranges);
}

/** Highlight `el`'s text with highlight.js for `lang`, keeping it plain on failure. */
function applyHljsToInline(el, lang) {
    if (typeof hljs === 'undefined') return;
    const code = document.createElement('code');
    code.className = `language-${lang || 'plaintext'}`;
    code.textContent = el.textContent;
    try {
        hljs.highlightElement(code);
        el.innerHTML = code.innerHTML;
    } catch (_) { /* leave plain */ }
}

/** The matcher for the query currently shown in the results (null = none). */
let _currentMatcher = null;

function currentQueryMatcher() {
    const query = queryInput.value.trim();
    return buildQueryMatcher(query, {
        regex: regexModeCheckbox?.checked || false,
        references: referencesModeCheckbox?.checked || false,
    });
}

// ============================================
// FILE VIEW HELPER (shared by modal and tooltip)
// ============================================

/**
 * Fetch a file and render it into `container` with syntax highlighting,
 * query-term highlighting, and the matched line scrolled into view within
 * the container (works for both the full-screen modal and the fixed tooltip).
 */
async function populateFileView(container, filePath, highlightLine, matcher, signal) {
    const response = await fetch(
        `${API_BASE}/api/file?file=${encodeURIComponent(filePath)}`,
        signal ? { signal } : {}
    );
    if (!response.ok) {
        const text = await response.text();
        const statusMessages = { 404: 'File not found', 403: 'Access denied', 503: 'Server busy' };
        throw new Error(statusMessages[response.status] || text || response.statusText);
    }
    const data = await response.json();
    const lang = hljsLangForPath(filePath);
    const linesHtml = data.content.split('\n')
        .map((line, idx) => renderFileLine(line, idx + 1, highlightLine))
        .join('');

    container.innerHTML =
        `<div class="file-meta">${data.line_count.toLocaleString()} lines · ${formatBytes(data.size_bytes)}</div>` +
        `<div class="file-code" data-lang="${lang}">${linesHtml}</div>`;

    if (typeof hljs !== 'undefined') {
        container.querySelectorAll('.file-line-content').forEach(span => {
            const code = document.createElement('code');
            code.className = `language-${lang}`;
            code.textContent = span.textContent;
            hljs.highlightElement(code);
            span.innerHTML = code.innerHTML;
        });
    }

    if (matcher) {
        container.querySelectorAll('.file-line-content').forEach(span => highlightTermsIn(span, matcher));
    }

    // Scroll the matched line to the centre of the container.
    // Using getBoundingClientRect so it works for both fixed-position tooltips
    // and normal flow modal bodies.
    const targetLine = container.querySelector(`#file-line-${highlightLine}`);
    if (targetLine) {
        const cRect = container.getBoundingClientRect();
        const lRect = targetLine.getBoundingClientRect();
        container.scrollTop += lRect.top - cRect.top - container.clientHeight / 2 + lRect.height / 2;
    }
}

// ============================================
// CONTEXT TOOLTIP
// ============================================

let _ctxTooltip = null;
let _ctxHideTimer = null;
let _ctxFetchController = null;
let _ctxHoverTimer = null;
// Cache of /api/context responses keyed by `${filePath}::${lineNumber}` so
// sweeping the cursor over results doesn't re-fetch the same windows.
const _ctxContextCache = new Map();

function getOrCreateTooltip() {
    if (!_ctxTooltip) {
        _ctxTooltip = document.createElement('div');
        _ctxTooltip.id = 'ctx-tooltip';
        _ctxTooltip.className = 'ctx-tooltip';
        // Keep tooltip visible when mouse moves into it
        _ctxTooltip.addEventListener('mouseenter', () => clearTimeout(_ctxHideTimer));
        _ctxTooltip.addEventListener('mouseleave', hideContextTooltip);
        document.body.appendChild(_ctxTooltip);
    }
    return _ctxTooltip;
}

function hideContextTooltip() {
    _ctxHideTimer = setTimeout(() => {
        if (_ctxTooltip) {
            _ctxTooltip.style.display = 'none';
        }
    }, 150);
}

function hideContextTooltipImmediately() {
    clearTimeout(_ctxHideTimer);
    if (_ctxFetchController) {
        _ctxFetchController.abort();
        _ctxFetchController = null;
    }
    if (_ctxTooltip) {
        _ctxTooltip.style.display = 'none';
    }
}

// Lines of context shown above/below the match in the hover preview.
const CTX_TOOLTIP_CONTEXT = 12;

function renderContextBody(data, highlightLine) {
    const start = data.start_line || 1;
    const rows = (data.lines || []).map((line, i) => {
        const ln = start + i;
        const isMatch = ln === highlightLine;
        return `<div class="file-line${isMatch ? ' file-line-highlight' : ''}">` +
            `<span class="file-line-num">${ln}</span>` +
            `<span class="file-line-content">${escapeHtml(line)}</span>` +
            `</div>`;
    }).join('');
    return `<div class="ctx-file-body">${rows}</div>`;
}

function highlightContextTooltip(tooltip) {
    const matcher = _currentMatcher || currentQueryMatcher();
    tooltip.querySelectorAll('.file-line-content').forEach(span => {
        if (typeof hljs !== 'undefined') {
            const code = document.createElement('code');
            code.textContent = span.textContent;
            try { hljs.highlightElement(code); span.innerHTML = code.innerHTML; } catch (e) { /* leave plain */ }
        }
        if (matcher) highlightTermsIn(span, matcher);
    });
}

async function showContextTooltip(resultItem, filePath, lineNumber) {
    clearTimeout(_ctxHideTimer);
    if (_ctxFetchController) _ctxFetchController.abort();
    _ctxFetchController = new AbortController();
    const signal = _ctxFetchController.signal;

    const tooltip = getOrCreateTooltip();
    const headerHtml = `<div class="ctx-header">${escapeHtml(filePath)} : ${lineNumber}</div>`;
    const cacheKey = `${filePath}::${lineNumber}`;

    // Cache hit: render immediately, no fetch.
    const cached = _ctxContextCache.get(cacheKey);
    if (cached) {
        tooltip.innerHTML = headerHtml + renderContextBody(cached, lineNumber);
        tooltip.style.display = 'flex';
        highlightContextTooltip(tooltip);
        positionTooltip(tooltip, resultItem);
        return;
    }

    tooltip.innerHTML = headerHtml + `<div class="ctx-file-body"><div class="ctx-loading">Loading…</div></div>`;
    positionTooltip(tooltip, resultItem);
    tooltip.style.display = 'flex';

    try {
        // Use the lightweight /api/context endpoint (a small window) instead of
        // fetching and highlighting the ENTIRE file on every hover.
        const url = `${API_BASE}/api/context?file=${encodeURIComponent(filePath)}&line=${lineNumber}&context=${CTX_TOOLTIP_CONTEXT}`;
        const resp = await fetch(url, { signal });
        if (!resp.ok) throw new Error(await readErrorBody(resp));
        const data = await resp.json();
        if (signal.aborted) return;
        _ctxContextCache.set(cacheKey, data);
        tooltip.innerHTML = headerHtml + renderContextBody(data, lineNumber);
        highlightContextTooltip(tooltip);
        positionTooltip(tooltip, resultItem);
    } catch (e) {
        if (e.name === 'AbortError') return;
        if (_ctxTooltip) _ctxTooltip.style.display = 'none';
    }
}

function positionTooltip(tooltip, anchor) {
    const GAP = 8;
    const MAX_W = Math.min(1100, Math.round(window.innerWidth * 0.9));
    const MIN_W = 280;
    const rect = anchor.getBoundingClientRect(); // viewport-relative
    const vW = window.innerWidth;
    const vH = window.innerHeight;

    // Available space on each side (inner edges, accounting for gap from viewport edge)
    const availLeft  = rect.left - GAP * 2;         // width if we fill left-of-button
    const availRight = vW - rect.right - GAP * 2;   // width if we fill right-of-button

    let w, left;
    if (availLeft >= MIN_W || availLeft >= availRight) {
        // Fill the space to the LEFT of the button.
        // Right edge sits gap-away from button; left edge = GAP from viewport.
        w    = Math.min(availLeft, MAX_W);
        left = rect.left - w - GAP;          // = GAP when w is not capped by MAX_W
    } else {
        // More usable space to the RIGHT — place there instead.
        w    = Math.min(Math.max(MIN_W, availRight), MAX_W);
        left = rect.right + GAP;
    }

    // Clamp so neither edge escapes the viewport.
    left = Math.max(GAP, Math.min(left, vW - w - GAP));

    // Override every CSS box property that could constrain the width.
    tooltip.style.minWidth  = '0';
    tooltip.style.maxWidth  = 'none';
    tooltip.style.right     = 'auto';
    tooltip.style.width     = `${w}px`;
    tooltip.style.left      = `${left}px`;

    // Fill viewport height.
    const ttH = vH - GAP * 2;
    tooltip.style.height = `${ttH}px`;
    tooltip.style.top = `${GAP}px`;
}

// ============================================
// SEARCH
// ============================================

function getMatchTypeLabel(matchType) {
    switch (matchType) {
        case 'SYMBOL_DEFINITION':
        case 1:
            return { text: 'Symbol', isSymbol: true };
        case 'SYMBOL_REFERENCE':
        case 2:
            return { text: 'Reference', isSymbol: true };
        default:
            return { text: 'Text', isSymbol: false };
    }
}

// Tracks the in-flight search so a slow earlier request can be aborted before a
// newer one renders (prevents stale results overwriting fresh ones).
let _searchAbort = null;

// Index of the keyboard-selected result group (-1 = none). Reset on each render.
let _selectedGroupIndex = -1;

/**
 * The search whose results are on screen: its request parameters (without
 * `offset`), the hits loaded so far across pages, and the last response.
 * "Load more" appends the next page to `results` and re-renders.
 */
let _currentSearch = null;

/** Build the /api/search query string for the current form state. */
function buildSearchParams(s) {
    const params = new URLSearchParams({ q: s.query, max: String(s.maxResults) });
    if (s.includeFilter) params.set('include', s.includeFilter);
    if (s.excludeFilter) params.set('exclude', s.excludeFilter);
    // The three modes are mutually exclusive in the UI (the server lets
    // references and symbols silently win over regex).
    if (s.isReferences) params.set('references', 'true');
    else if (s.symbolsOnly) params.set('symbols', 'true');
    else if (s.isRegex) params.set('regex', 'true');
    if (s.rankMode !== 'auto') params.set('rank', s.rankMode);
    if (s.contextLines > 0) params.set('context', String(s.contextLines));
    return params;
}

/**
 * Fetch one page of results. Non-OK responses surface the server's error
 * body (e.g. "Invalid regex pattern: …") instead of a bare status text.
 */
async function fetchSearchPage(params, signal) {
    const response = await fetch(`${API_BASE}/api/search?${params}`, { signal });
    if (!response.ok) {
        throw new Error(await readErrorBody(response));
    }
    return response.json();
}

/**
 * Run the search for the current form state.
 * @param {{trigger?: 'input'|'submit'|'option'|'history'}} [opts]
 *   `trigger` decides how the URL/history is written (see writeUrlState);
 *   'history' means the URL already holds this state (popstate, initial load).
 */
async function performSearch(opts = {}) {
    const trigger = opts.trigger || 'option';

    // Don't search if index isn't ready yet
    if (!searchReadiness.isReady) {
        return;
    }

    // Cancel any in-flight request: whether this call ends up searching or just
    // clearing results, the previous request must not win a render race.
    if (_searchAbort) _searchAbort.abort();
    _searchAbort = new AbortController();
    const signal = _searchAbort.signal;

    const query = queryInput.value.trim();
    const search = {
        query,
        maxResults: currentMaxResults(),
        includeFilter: includeFilterInput?.value.trim() || '',
        excludeFilter: excludeFilterInput?.value.trim() || '',
        isRegex: regexModeCheckbox?.checked || false,
        symbolsOnly: symbolsModeCheckbox?.checked || false,
        isReferences: referencesModeCheckbox?.checked || false,
        rankMode: rankModeSelect?.value || 'auto',
        contextLines: currentContextLines(),
        results: [],
        last: null,
        loading: false,
    };

    // Keep URL/history in sync so searches can be shared and navigated
    writeUrlState(trigger);
    // Persist settings to localStorage
    saveSettingsToStorage();

    if (!query) {
        _currentSearch = null;
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter a search query to find code</p></div>';
        return;
    }

    if (query.length < 3) {
        _currentSearch = null;
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter at least 3 characters to search</p></div>';
        return;
    }

    resultsContainer.innerHTML = '<div class="loading">Searching...</div>';
    resultsHeader.style.display = 'none';

    const startTime = performance.now();

    try {
        search.params = buildSearchParams(search);
        const data = await fetchSearchPage(search.params, signal);
        if (signal.aborted) return;
        search.results = data.results.slice();
        search.last = data;
        _currentSearch = search;
        const duration = data.elapsed_ms !== undefined ? data.elapsed_ms : (performance.now() - startTime);
        renderSearch(search, duration);
    } catch (error) {
        // A superseded request was aborted on purpose — ignore it so the newer
        // search's results/UI are not clobbered by a stale error.
        if (error.name === 'AbortError') return;
        console.error('Search error:', error);
        showError('results', error.message);
    }
}

/** Fetch the next page (`offset` = hits loaded so far) and append it. */
async function loadMoreResults() {
    const search = _currentSearch;
    if (!search || search.loading || !search.last?.has_more) return;
    search.loading = true;

    const btn = document.getElementById('load-more-btn');
    if (btn) {
        btn.disabled = true;
        btn.textContent = 'LOADING…';
    }

    if (_searchAbort) _searchAbort.abort();
    _searchAbort = new AbortController();
    const signal = _searchAbort.signal;

    try {
        const params = new URLSearchParams(search.params);
        params.set('offset', String(search.results.length));
        const data = await fetchSearchPage(params, signal);
        if (signal.aborted || _currentSearch !== search) return;
        const firstNewGroup = groupResultsByFile(search.results).length;
        search.results = search.results.concat(data.results);
        search.last = data;
        renderSearch(search, data.elapsed_ms, { preserveSelection: true });
        // Move focus to the first newly loaded group so keyboard users land
        // where the new content starts.
        const groups = getResultGroups();
        const target = groups[Math.min(firstNewGroup, groups.length - 1)];
        if (target && typeof target.focus === 'function') target.focus({ preventScroll: true });
    } catch (error) {
        if (error.name === 'AbortError') return;
        console.error('Load more error:', error);
        const row = document.getElementById('load-more-row');
        if (row) {
            row.innerHTML = `<div class="error-message"><strong>Error:</strong> ${escapeHtml(error.message)}</div>`;
        }
    } finally {
        search.loading = false;
    }
}

/**
 * The results-count label. `total_matches` is present when the scan ran to
 * completion; when the match budget or deadline stopped it early the server
 * sets `truncated_by_budget` and the total is unknown, which is not something
 * a bigger page size fixes.
 */
function resultsCountLabel(search) {
    const data = search.last;
    const n = search.results.length;
    const plural = (k) => `${k} RESULT${k !== 1 ? 'S' : ''}`;
    if (typeof data.total_matches === 'number') {
        return n < data.total_matches
            ? { text: `${n} OF ${data.total_matches.toLocaleString()} RESULTS`, title: 'Use LOAD MORE to fetch the next page' }
            : { text: `${plural(n)} FOUND`, title: '' };
    }
    if (data.truncated_by_budget) {
        return {
            text: `${n}+ RESULTS (SCAN BUDGET REACHED)`,
            title: 'The search stopped at its match budget or deadline, so the total is unknown. LOAD MORE continues from where it stopped.',
        };
    }
    return data.has_more
        ? { text: `${n}+ RESULTS`, title: 'More results are available' }
        : { text: `${plural(n)} FOUND`, title: '' };
}

/** Render the current search (all pages loaded so far) into the results area. */
function renderSearch(search, durationMs, opts = {}) {
    const data = search.last;
    const query = search.query;

    resultsHeader.style.display = 'flex';
    const label = resultsCountLabel(search);
    resultsCount.textContent = label.text;
    resultsCount.title = label.title;
    searchTimeEl.textContent = `LATENCY: ${Number(durationMs || 0).toFixed(1)}ms`;

    // Show ranking info if available
    if (data.rank_mode && data.total_candidates !== undefined) {
        // Plain mono labels (no emoji) to match the brutalist design language.
        const modeLabel = data.rank_mode === 'fast' ? 'FAST' : (data.rank_mode === 'full' ? 'FULL' : 'AUTO');
        const candidateInfo = data.candidates_searched !== data.total_candidates
            ? `${data.candidates_searched.toLocaleString()}/${data.total_candidates.toLocaleString()} files`
            : `${data.total_candidates.toLocaleString()} files`;
        rankingInfoEl.textContent = `${modeLabel} (${candidateInfo})`;
        rankingInfoEl.title = `Ranking mode: ${data.rank_mode}\nTotal candidates: ${data.total_candidates}\nSearched: ${data.candidates_searched}`;
    } else {
        rankingInfoEl.textContent = '';
        rankingInfoEl.title = '';
    }

    if (search.results.length === 0) {
        resultsContainer.innerHTML = `<div class="empty-state no-results"><p>No results found for "${escapeHtml(query)}"</p></div>`;
        return;
    }

    const groupedResults = groupResultsByFile(search.results);
    if (!opts.preserveSelection) _selectedGroupIndex = -1; // reset keyboard selection on each new search
    // Matcher for lines without server offsets (context lines, previews).
    _currentMatcher = buildQueryMatcher(query, { regex: search.isRegex, references: search.isReferences });

    const groupsHtml = groupedResults.map(group => {
        const firstHit = group.hits[0];
        const depCount = Math.max(...group.hits.map(hit => hit.dependency_count || 0));
        const lang = hljsLangForPath(group.filePath);
        const ext = (group.filePath.split('.').pop() || '').toLowerCase();
        const langClass = langClassForPath(group.filePath);

        // Split path into directory + filename for display
        const pathParts = group.filePath.split('/');
        const fileName = pathParts.pop();
        const dirPath = pathParts.length ? pathParts.join('/') + '/' : '';

        // File type icon based on extension
        const fileIcon = ext === 'md' ? 'description' : (ext === 'yaml' || ext === 'yml' || ext === 'toml' || ext === 'json' ? 'settings_suggest' : 'code');

        // Language badge style
        const langBadgeStyle = getLangBadgeStyle(langClass);

        // Dependency badge
        const depBadge = depCount > 0
            ? `<span class="deps-badge" style="cursor:pointer;padding:2px 6px;background:#ebe77f;color:#000;font-size:10px;font-family:'JetBrains Mono',monospace;border:1px solid rgba(0,0,0,0.2)"
                data-file-path="${escapeHtml(group.filePath)}">${depCount} deps</span>`
            : '';

        // A filename hit has line_number 0: the viewer opens at line 1 for it.
        const groupViewLine = Math.max(1, firstHit.line_number || 0);

        const hitsHtml = group.hits.map((result, idx) => {
            const matchType = getMatchTypeLabel(result.match_type);
            const typeBadgeStyle = matchType.isSymbol
                ? 'background:#a9efed;color:#00201f;border:1px solid #1e6868'
                : 'background:#e7e3ce;color:#494831;border:1px solid #cbc8aa';
            const isFilenameHit = !result.line_number;
            const viewLine = isFilenameHit ? 1 : result.line_number;
            const lineLabel = isFilenameHit ? 'filename' : `line ${result.line_number}`;
            const truncatedBadge = result.content_truncated
                ? `<span class="truncated-badge" title="The line is longer than the 500-byte content window; open the file to see all of it">… truncated</span>`
                : '';

            // Build code content — with context lines if available, otherwise just the match line
            let codeContent;
            if (result.context_lines && result.context_lines.length > 0) {
                const startLine = result.context_start_line || 1;
                codeContent = result.context_lines.map((line, i) => {
                    const lineNum = startLine + i;
                    const isMatch = lineNum === result.line_number;
                    const lineStyle = isMatch
                        ? 'display:flex;background:var(--hl-line-bg);border-left:3px solid var(--hl-left-border)'
                        : 'display:flex;border-left:3px solid transparent';
                    // The match line carries the server's byte offsets into the
                    // full line so the highlight pass can mark the exact hit.
                    const offsetAttrs = isMatch
                        ? ` data-lms="${Number(result.line_match_start) || 0}" data-lme="${Number(result.line_match_end) || 0}"`
                        : '';
                    return `<div style="${lineStyle}">` +
                        `<span style="flex-shrink:0;width:3.5em;text-align:right;padding-right:0.75em;color:#9e9c80;font-size:0.75em;user-select:none;line-height:1.5em">${lineNum}</span>` +
                        `<span class="ctx-line-content${isMatch ? ' match-line' : ''}"${offsetAttrs} style="flex:1;white-space:pre;overflow-x:auto">${escapeHtml(line)}</span>` +
                        `</div>`;
                }).join('');
            } else {
                codeContent = escapeHtml(result.content);
            }
            const matchOffsetAttrs = result.context_lines
                ? ''
                : ` data-ms="${Number(result.match_start) || 0}" data-me="${Number(result.match_end) || 0}"`;

            const preClass = result.context_lines
                ? `result-code result-code-ctx language-${lang}`
                : `result-code language-${lang}`;

            const hitContainerStyle = idx === 0
                ? 'background:#fff'
                : 'background:#fff;border-top:1px solid #d4d0ba';

            return `
                <div style="${hitContainerStyle}">
                    <div class="px-4 py-1.5 flex justify-between items-center" style="background:#f8f4df;border-bottom:1px solid #e3dec8">
                        <div class="flex items-center gap-2 min-w-0">
                            <span class="font-label text-xs" style="color:#7a785f;flex-shrink:0">${lineLabel}</span>
                            <span style="${typeBadgeStyle};padding:2px 6px;font-size:10px;font-family:'JetBrains Mono',monospace">${matchType.text}</span>
                            ${truncatedBadge}
                        </div>
                        <div class="flex items-center gap-3 flex-shrink-0">
                            <span style="cursor:help;font-family:'JetBrains Mono',monospace;font-size:10px;color:#7a785f;text-transform:uppercase"
                                title="Score = base × multipliers&#10;&#10;• Exact case match: 2×&#10;• Symbol definition: 3×&#10;• In /src/ or /lib/: 1.5×&#10;• Match at start of line: 1.5×&#10;• Shorter lines preferred (log scale, min 0.3×)&#10;• Dependency boost: 1 + 0.5·log10(import count)&#10;&#10;Higher scores rank first.">
                                ${result.score.toFixed(2)}
                            </span>
                            <button class="view-file-btn material-symbols-outlined hover:text-primary transition-colors"
                                style="font-size:18px;cursor:pointer;color:#7a785f;background:none;border:none;padding:0"
                                data-file-path="${escapeHtml(result.file_path)}"
                                data-line-number="${viewLine}"
                                title="View full file at this line">open_in_new</button>
                        </div>
                    </div>
                    <div class="overflow-x-auto" style="background:#fff">
                        <pre class="${preClass}"${matchOffsetAttrs} data-has-context="${result.context_lines ? 'true' : 'false'}" data-lang="${lang}">${codeContent}</pre>
                    </div>
                </div>
            `;
        }).join('');

        return `
            <div class="result-group bg-white border border-black overflow-hidden" style="box-shadow:2px 2px 0 #000" data-file-path="${escapeHtml(group.filePath)}" data-line-number="${groupViewLine}">
                <!-- File header -->
                <div class="border-b border-black px-4 py-2 flex justify-between items-center" style="background:#dedac6">
                    <div class="flex items-center gap-2 min-w-0">
                        <span class="material-symbols-outlined" style="font-size:16px;flex-shrink:0">${fileIcon}</span>
                        <span class="font-label text-xs font-bold tracking-tight truncate" title="${escapeHtml(group.filePath)}">
                            ${dirPath ? `<span style="color:#7a785f;font-weight:400">${escapeHtml(dirPath)}</span>` : ''}<span style="color:#646100;font-weight:700">${escapeHtml(fileName)}</span>
                        </span>
                        <span style="padding:2px 6px;background:#e6e2cc;border:1px solid #cbc8aa;color:#494831;font-size:10px;font-family:'JetBrains Mono',monospace">${group.hits.length} hit${group.hits.length !== 1 ? 's' : ''}</span>
                    </div>
                    <div class="flex items-center gap-2 flex-shrink-0">
                        ${ext ? `<span style="${langBadgeStyle};padding:2px 6px;font-size:10px;font-family:'JetBrains Mono',monospace;text-transform:uppercase">${escapeHtml(ext)}</span>` : ''}
                        ${depBadge}
                        <button class="copy-path-btn material-symbols-outlined hover:text-primary transition-colors"
                            style="font-size:16px;cursor:pointer;color:#7a785f;background:none;border:none;padding:0"
                            data-file-path="${escapeHtml(group.filePath)}"
                            title="Copy file path">content_copy</button>
                        <button class="view-file-btn material-symbols-outlined hover:text-primary transition-colors"
                            style="font-size:18px;cursor:pointer;color:#7a785f;background:none;border:none;padding:0"
                            data-file-path="${escapeHtml(group.filePath)}"
                            data-line-number="${groupViewLine}"
                            title="View full file">open_in_new</button>
                    </div>
                </div>
                ${hitsHtml}
            </div>
        `;
    }).join('');

    // Paging: the next page starts at offset = hits loaded so far.
    const loadMoreHtml = data.has_more
        ? `<div id="load-more-row" class="load-more-row">
                <button id="load-more-btn" type="button" class="load-more-btn"
                    title="Fetch the next ${search.maxResults} results (offset ${search.results.length})">LOAD MORE</button>
           </div>`
        : '';

    resultsContainer.innerHTML = groupsHtml + loadMoreHtml;

    const loadMoreBtn = document.getElementById('load-more-btn');
    if (loadMoreBtn) loadMoreBtn.addEventListener('click', loadMoreResults);

    // Syntax-highlight each result, then mark the hit: the match line from
    // the server's byte offsets, every line from the query terms.
    resultsContainer.querySelectorAll('pre.result-code').forEach(pre => {
        const hasContext = pre.dataset.hasContext === 'true';
        const lang = pre.dataset.lang || 'plaintext';
        if (hasContext) {
            pre.querySelectorAll('.ctx-line-content').forEach(span => {
                const text = span.textContent;
                applyHljsToInline(span, lang);
                const extra = span.dataset.lms !== undefined
                    ? [byteRangeToCharRange(text, Number(span.dataset.lms), Number(span.dataset.lme))]
                    : [];
                highlightTermsIn(span, _currentMatcher, extra);
            });
        } else {
            const text = pre.textContent;
            if (typeof hljs !== 'undefined') {
                try { hljs.highlightElement(pre); } catch (_) { /* leave plain */ }
            }
            const extra = pre.dataset.ms !== undefined
                ? [byteRangeToCharRange(text, Number(pre.dataset.ms), Number(pre.dataset.me))]
                : [];
            highlightTermsIn(pre, _currentMatcher, extra);
        }
    });

    // Attach View button click handler and context tooltip
    resultsContainer.querySelectorAll('.view-file-btn').forEach(btn => {
        const filePath = btn.dataset.filePath;
        const lineNumber = parseInt(btn.dataset.lineNumber, 10) || 1;
        btn.addEventListener('click', () => showFileModal(filePath, lineNumber));
        // Hover-intent delay: only fetch a preview if the cursor lingers ~200ms,
        // so sweeping down a results list doesn't fire a burst of requests.
        btn.addEventListener('mouseenter', () => {
            clearTimeout(_ctxHoverTimer);
            _ctxHoverTimer = setTimeout(
                () => showContextTooltip(btn, filePath, lineNumber),
                200
            );
        });
        btn.addEventListener('mouseleave', () => {
            clearTimeout(_ctxHoverTimer);
            hideContextTooltip();
        });
    });

    // Copy-path buttons: copy the file path to the clipboard with brief feedback.
    resultsContainer.querySelectorAll('.copy-path-btn').forEach(btn => {
        btn.addEventListener('click', async () => {
            try {
                await navigator.clipboard.writeText(btn.dataset.filePath || '');
                const prev = btn.textContent;
                btn.textContent = 'check';
                setTimeout(() => { btn.textContent = prev; }, 1200);
            } catch (e) {
                console.error('Copy failed:', e);
            }
        });
    });

    // Attach dependency-badge handlers via dataset (no inline JS handlers, so
    // file paths containing quotes can never inject code).
    resultsContainer.querySelectorAll('.deps-badge').forEach(badge => {
        const filePath = badge.dataset.filePath;
        badge.addEventListener('mouseenter', () => showDepsTooltip(badge, filePath));
        badge.addEventListener('mouseleave', hideDepsTooltip);
        badge.addEventListener('click', () => {
            hideDepsTooltipImmediately();
            showDependents(filePath);
        });
    });

    if (opts.preserveSelection && _selectedGroupIndex >= 0) {
        highlightSelectedGroup(getResultGroups());
    }
}

const debouncedSearch = debounce(() => performSearch({ trigger: 'input' }), DEBOUNCE_MS);

/** A settings change (select, toggle, filter Enter): search now, new history entry. */
function optionChanged() {
    debouncedSearch.cancel();
    performSearch({ trigger: 'option' });
}

/**
 * Explicit submit: record the query in history (only here, NOT in performSearch,
 * so the debounced keystroke path doesn't pollute history with prefixes like
 * "per", "perfo", …) and run the search immediately.
 */
function submitSearch() {
    debouncedSearch.cancel();
    const q = queryInput.value.trim();
    if (q) saveToHistory(q);
    performSearch({ trigger: 'submit' });
}

// ============================================
// RESULTS KEYBOARD NAVIGATION
// ============================================

function getResultGroups() {
    return Array.from(resultsContainer.querySelectorAll('.result-group'));
}

function highlightSelectedGroup(groups) {
    groups.forEach((g, i) => {
        if (i === _selectedGroupIndex) {
            g.style.outline = '3px solid #646100';
            g.style.outlineOffset = '2px';
            g.scrollIntoView({ block: 'nearest' });
        } else {
            g.style.outline = '';
            g.style.outlineOffset = '';
        }
    });
}

function moveResultSelection(delta) {
    const groups = getResultGroups();
    if (!groups.length) return;
    _selectedGroupIndex = _selectedGroupIndex < 0
        ? 0
        : Math.max(0, Math.min(groups.length - 1, _selectedGroupIndex + delta));
    highlightSelectedGroup(groups);
}

function openSelectedGroup() {
    const g = getResultGroups()[_selectedGroupIndex];
    if (!g) return;
    showFileModal(g.dataset.filePath, parseInt(g.dataset.lineNumber, 10) || 1);
}

// Global shortcuts: '/' focuses search; j/k or arrows move the result selection;
// Enter opens the selected file. Ignored while typing in a field or with a modal open.
document.addEventListener('keydown', (e) => {
    const tag = (document.activeElement && document.activeElement.tagName) || '';
    const typing = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    const modalOpen = document.getElementById('file-modal') || document.getElementById('dep-modal');

    if (e.key === '/' && !typing && !modalOpen) {
        e.preventDefault();
        queryInput.focus();
        return;
    }
    if (typing || modalOpen) return;

    if (e.key === 'ArrowDown' || e.key === 'j') {
        e.preventDefault();
        moveResultSelection(1);
    } else if (e.key === 'ArrowUp' || e.key === 'k') {
        e.preventDefault();
        moveResultSelection(-1);
    } else if (e.key === 'Enter') {
        openSelectedGroup();
    }
});

function groupResultsByFile(results) {
    const groups = [];
    const indexByPath = new Map();

    results.forEach((result) => {
        const key = normalizeFileGroupKey(result.file_path);
        let groupIndex = indexByPath.get(key);

        if (groupIndex === undefined) {
            groupIndex = groups.length;
            indexByPath.set(key, groupIndex);
            groups.push({
                // Use the normalized string ONLY as the grouping key. The displayed
                // and fetched path must be the ORIGINAL file_path — the lowercased
                // key broke the header display ("searchengine.rs") and made the
                // group view/deps buttons 404 on case-sensitive systems.
                filePath: result.file_path,
                hits: [],
            });
        }

        groups[groupIndex].hits.push(result);
    });

    return groups;
}

function normalizeFileGroupKey(filePath) {
    if (!filePath) return '';

    // Normalize separators and case so Windows path variants collapse into one group.
    const normalizedSlashes = filePath.replace(/\\/g, '/');
    return normalizedSlashes.toLowerCase();
}

// ============================================
// DEPS BADGE POPOVER
// ============================================

let _depsPopover = null;
let _depsHideTimer = null;
let _depsAbortController = null;

function getOrCreateDepsPopover() {
    if (!_depsPopover) {
        _depsPopover = document.createElement('div');
        _depsPopover.id = 'deps-popover';
        _depsPopover.className = 'deps-popover';
        _depsPopover.addEventListener('mouseenter', () => clearTimeout(_depsHideTimer));
        _depsPopover.addEventListener('mouseleave', hideDepsTooltip);
        document.body.appendChild(_depsPopover);
    }
    return _depsPopover;
}

function hideDepsTooltip() {
    _depsHideTimer = setTimeout(() => {
        if (_depsPopover) _depsPopover.style.display = 'none';
    }, 150);
}

function hideDepsTooltipImmediately() {
    clearTimeout(_depsHideTimer);
    if (_depsAbortController) {
        _depsAbortController.abort();
        _depsAbortController = null;
    }
    if (_depsPopover) _depsPopover.style.display = 'none';
}

function openDependencyFile(filePath) {
    hideDepsTooltipImmediately();
    showFileModal(filePath);
}

function positionDepsPopover(popover, anchor) {
    const GAP = 6;
    const rect = anchor.getBoundingClientRect();
    const vW = window.innerWidth;
    const vH = window.innerHeight;

    // Preferred: appear above the badge; fall back to below if not enough space
    const popH = Math.min(popover.scrollHeight || 300, vH * 0.6);

    let top;
    if (rect.top - popH - GAP >= GAP) {
        top = rect.top - popH - GAP;
    } else {
        top = rect.bottom + GAP;
    }

    // Horizontal: align left edge of popover with badge, clamped to viewport
    const maxW = Math.min(380, vW - GAP * 2);
    let left = rect.left;
    left = Math.max(GAP, Math.min(left, vW - maxW - GAP));

    popover.style.maxWidth = `${maxW}px`;
    popover.style.left = `${left}px`;
    popover.style.top = `${top}px`;
    popover.style.height = 'auto';
}

async function showDepsTooltip(badgeEl, filePath) {
    clearTimeout(_depsHideTimer);
    if (_depsAbortController) _depsAbortController.abort();
    _depsAbortController = new AbortController();
    const signal = _depsAbortController.signal;

    const popover = getOrCreateDepsPopover();
    const basename = filePath.split('/').pop();
    popover.innerHTML =
        `<div class="deps-popover-header">${escapeHtml(basename)}</div>` +
        `<div class="deps-popover-loading">Loading…</div>`;
    popover.style.display = 'block';
    positionDepsPopover(popover, badgeEl);

    try {
        const [depRes, imptRes] = await Promise.all([
            fetch(`${API_BASE}/api/dependents?file=${encodeURIComponent(filePath)}`, { signal }),
            fetch(`${API_BASE}/api/dependencies?file=${encodeURIComponent(filePath)}`, { signal }),
        ]);
        if (signal.aborted) return;

        const [depData, imptData] = await Promise.all([depRes.json(), imptRes.json()]);
        if (signal.aborted) return;

        const MAX = 8;

        // Data-attribute driven: no interpolated inline handlers. Paths are stored
        // in data-* attributes (escaped) and read back via dataset, so a path
        // containing quotes can never break out into executable code.
        function buildSection(files, label, moreAction) {
            const shown = files.slice(0, MAX);
            const extra = files.length - shown.length;
            const items = shown.map(f => {
                const name = f.split('/').pop();
                return `<li><button type="button" class="deps-popover-link" title="${escapeHtml(f)}" data-dep-file="${escapeHtml(f)}">${escapeHtml(name)}</button></li>`;
            }).join('');
            const more = extra > 0
                ? `<span class="deps-more" data-more-action="${escapeHtml(moreAction)}">…and ${extra} more</span>`
                : '';
            return `<div class="deps-popover-section">` +
                `<div class="deps-section-title">${escapeHtml(label)} (${files.length})</div>` +
                `<ul>${items || '<li style="color:#7a785f;font-style:italic">none</li>'}</ul>` +
                more +
                `</div>`;
        }

        const dependentsSection = buildSection(depData.files || [], 'Imported by', 'dependents');
        const importsSection = buildSection(imptData.files || [], 'Imports', 'dependencies');

        popover.innerHTML =
            `<div class="deps-popover-header">${escapeHtml(basename)}</div>` +
            dependentsSection +
            importsSection;

        // Wire popover links/“more” via delegation using dataset values.
        popover.querySelectorAll('.deps-popover-link').forEach(link => {
            link.addEventListener('click', () => openDependencyFile(link.dataset.depFile));
        });
        popover.querySelectorAll('.deps-more').forEach(more => {
            more.addEventListener('click', () => {
                hideDepsTooltipImmediately();
                if (more.dataset.moreAction === 'dependencies') {
                    showDependencies(filePath);
                } else {
                    showDependents(filePath);
                }
            });
        });

        positionDepsPopover(popover, badgeEl);
    } catch (e) {
        if (e.name === 'AbortError') return;
        if (_depsPopover) _depsPopover.style.display = 'none';
    }
}

// ============================================
// DEPENDENCY MODAL
// ============================================

async function showDependents(filePath) {
    hideDepsTooltipImmediately();
    try {
        const response = await fetch(`${API_BASE}/api/dependents?file=${encodeURIComponent(filePath)}`);
        if (!response.ok) throw new Error(`Failed to fetch dependents`);
        const data = await response.json();
        showDependencyModal('Dependents', filePath, data.files, 'Files that import this file:');
    } catch (error) {
        console.error('Error fetching dependents:', error);
        alert('Failed to load dependents: ' + error.message);
    }
}

async function showDependencies(filePath) {
    hideDepsTooltipImmediately();
    try {
        const response = await fetch(`${API_BASE}/api/dependencies?file=${encodeURIComponent(filePath)}`);
        if (!response.ok) throw new Error(`Failed to fetch dependencies`);
        const data = await response.json();
        showDependencyModal('Dependencies', filePath, data.files, 'Files imported by this file:');
    } catch (error) {
        console.error('Error fetching dependencies:', error);
        alert('Failed to load dependencies: ' + error.message);
    }
}

function showDependencyModal(title, filePath, files, description) {
    const existingModal = document.getElementById('dep-modal');
    if (existingModal) existingModal.remove();

    const fileList = files.length > 0
        ? files.map(f => `<li style="padding:0.25rem 0;font-family:monospace;font-size:0.85rem;color:#1d1c0f">${escapeHtml(f)}</li>`).join('')
        : '<li style="color:#7a785f;">No files found</li>';

    const modal = document.createElement('div');
    modal.id = 'dep-modal';
    modal.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.6);display:flex;align-items:center;justify-content:center;z-index:1000;';
    modal.innerHTML = `
        <div style="background:#f2eed9;border:1px solid #cbc8aa;box-shadow:6px 6px 0 #000;padding:1.5rem;max-width:600px;width:90%;max-height:80vh;overflow:auto;">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;">
                <h2 style="font-size:1.1rem;font-family:'JetBrains Mono',monospace;color:#1d1c0f">${escapeHtml(title)} (${files.length})</h2>
                <button class="dep-modal-close" style="background:none;border:1px solid #000;width:1.75rem;height:1.75rem;font-size:1.1rem;cursor:pointer;color:#1d1c0f;display:flex;align-items:center;justify-content:center;">&times;</button>
            </div>
            <p style="font-family:monospace;font-size:0.85rem;color:#1d4f6e;margin-bottom:0.5rem;word-break:break-all">${escapeHtml(filePath)}</p>
            <p style="color:#494831;font-size:0.85rem;margin-bottom:0.75rem;">${description}</p>
            <ul style="list-style:none;padding:0;">${fileList}</ul>
        </div>
    `;

    modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
    const closeBtn = modal.querySelector('.dep-modal-close');
    if (closeBtn) closeBtn.addEventListener('click', closeModal);
    document.addEventListener('keydown', handleModalEscape);
    document.body.appendChild(modal);
}

function handleModalEscape(e) {
    if (e.key === 'Escape') closeModal();
}

function closeModal() {
    const modal = document.getElementById('dep-modal');
    if (modal) modal.remove();
    document.removeEventListener('keydown', handleModalEscape);
}

// ============================================
// FILE VIEWER MODAL
// ============================================

function renderFileLine(line, lineNum, highlightLine) {
    const isHighlighted = lineNum === highlightLine;
    const cls = isHighlighted ? 'file-line file-line-highlight' : 'file-line';
    return `<div class="${cls}" id="file-line-${lineNum}">` +
        `<span class="file-line-num">${lineNum}</span>` +
        `<span class="file-line-content">${escapeHtml(line)}</span>` +
        `</div>`;
}

// Stored so the overlay click listener can be removed on close
let _fileModalOverlayListener = null;

async function showFileModal(filePath, highlightLine) {
    // Ensure hover preview is not left visible behind the modal.
    hideContextTooltipImmediately();

    // Clean up any existing file modal and its listeners
    const existingModal = document.getElementById('file-modal');
    if (existingModal) {
        if (_fileModalOverlayListener) existingModal.removeEventListener('click', _fileModalOverlayListener);
        existingModal.remove();
    }
    document.removeEventListener('keydown', handleFileModalEscape);

    // Create modal scaffold immediately (with loading state)
    const modal = document.createElement('div');
    modal.id = 'file-modal';
    modal.className = 'file-modal-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'file-modal-dialog';

    const header = document.createElement('div');
    header.className = 'file-modal-header';

    const pathSpan = document.createElement('span');
    pathSpan.className = 'file-modal-path';
    pathSpan.textContent = filePath;

    const closeBtn = document.createElement('button');
    closeBtn.className = 'file-modal-close';
    closeBtn.title = 'Close (Esc)';
    closeBtn.textContent = '×';
    closeBtn.addEventListener('click', closeFileModal);

    header.appendChild(pathSpan);
    header.appendChild(closeBtn);

    const body = document.createElement('div');
    body.className = 'file-modal-body';
    body.id = 'file-modal-body';
    body.innerHTML = '<div class="loading">Loading file…</div>';

    dialog.appendChild(header);
    dialog.appendChild(body);
    modal.appendChild(dialog);

    _fileModalOverlayListener = (e) => { if (e.target === modal) closeFileModal(); };
    modal.addEventListener('click', _fileModalOverlayListener);
    document.addEventListener('keydown', handleFileModalEscape);
    document.body.appendChild(modal);

    try {
        await populateFileView(body, filePath, highlightLine, _currentMatcher || currentQueryMatcher());
    } catch (error) {
        body.innerHTML = `<div class="error-message"><strong>Error:</strong> ${escapeHtml(error.message)}</div>`;
    }
}

function handleFileModalEscape(e) {
    if (e.key === 'Escape') closeFileModal();
}

function closeFileModal() {
    const modal = document.getElementById('file-modal');
    if (modal) {
        if (_fileModalOverlayListener) {
            modal.removeEventListener('click', _fileModalOverlayListener);
            _fileModalOverlayListener = null;
        }
        modal.remove();
    }
    document.removeEventListener('keydown', handleFileModalEscape);
}

/**
 * Run a one-time startup search when a query was provided in the URL.
 * If indexing is still in progress, performSearch() will no-op and the
 * readiness callback will execute it once the index becomes searchable.
 */
function runInitialSearchFromUrl() {
    const urlQuery = new URLSearchParams(window.location.search).get('q') || '';
    if (!urlQuery.trim()) return;
    if (!queryInput.value.trim()) return;
    performSearch({ trigger: 'history' });
}

// ============================================
// EVENT LISTENERS
// ============================================

queryInput.addEventListener('input', (e) => {
    debouncedSearch();
    showHistoryDropdown(e.target.value.trim());
});
queryInput.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') {
        navigateHistoryDropdown(1);
        e.preventDefault();
        return;
    }
    if (e.key === 'ArrowUp') {
        navigateHistoryDropdown(-1);
        e.preventDefault();
        return;
    }
    if (e.key === 'Escape') {
        hideHistoryDropdown();
        return;
    }
    if (e.key === 'Enter') {
        hideHistoryDropdown();
        // Explicit submit: cancels the pending debounce and records history.
        submitSearch();
    }
});
queryInput.addEventListener('focus', () => {
    showHistoryDropdown(queryInput.value.trim());
});
document.addEventListener('click', (e) => {
    if (e.target !== queryInput && !searchHistoryDropdown?.contains(e.target)) {
        hideHistoryDropdown();
    }
});

maxResultsSelect.addEventListener('change', optionChanged);

// The mode toggles are mutually exclusive: turning one on turns the others off.
modeCheckboxes().forEach(cb => {
    cb.addEventListener('change', () => {
        if (cb.checked) modeCheckboxes().forEach(other => { if (other !== cb) other.checked = false; });
        syncToggleVisuals();
        optionChanged();
    });
});
if (rankModeSelect) rankModeSelect.addEventListener('change', optionChanged);
if (contextLinesSelect) contextLinesSelect.addEventListener('change', optionChanged);

if (includeFilterInput) {
    includeFilterInput.addEventListener('input', debouncedSearch);
    includeFilterInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') optionChanged(); });
}
if (excludeFilterInput) {
    excludeFilterInput.addEventListener('input', debouncedSearch);
    excludeFilterInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') optionChanged(); });
}

// ============================================
// INITIALIZATION
// ============================================

// Store original placeholder for restoration when readiness state changes.
searchReadiness.storeDefaultPlaceholder();

// Load persisted settings from localStorage first (URL params will override below)
loadSettingsFromStorage();

// Restore state from URL on page load; a URL param that is present wins over
// storage (including `regex=false`), an absent one leaves the stored value.
applyUrlState(new URLSearchParams(location.search));
// Give the initial history entry a state object so Back can return to it.
history.replaceState({ typed: false }, '', location.href);

// Probe the backend; start the WebSocket only after confirmation.
// Inputs start enabled (optimistic) — health check disables them only on failure.
checkBackendHealth().then(() => {
    progressWS.start();
    fetchStats();
    runInitialSearchFromUrl();
});
