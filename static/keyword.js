// ============================================
// KEYWORD SEARCH - View-specific logic
// Uses: common.js, lib/keyword-helpers.js (pure helpers, Node-tested)
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
const caseModeCheckbox = document.getElementById('case-mode');
const rankModeSelect = document.getElementById('rank-mode');
const contextLinesSelect = document.getElementById('context-lines');
const resultsContainer = document.getElementById('results');
const fileMatchesSection = document.getElementById('file-matches');
const resultsHeader = document.getElementById('results-header');
const resultsCount = document.getElementById('results-count');
const searchMetaEl = document.getElementById('search-meta');
const moreFilesBtn = document.getElementById('more-files-btn');
const searchHistoryDropdown = document.getElementById('search-history-dropdown');

// Stats elements
const statFiles = document.getElementById('stat-files');
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
// Context lines either side of each hit unless the reader picks otherwise.
const DEFAULT_CONTEXT = '1';
let _contextChosen = false;
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
            context: contextLinesSelect?.value || DEFAULT_CONTEXT,
            // Only a context the reader picked is restored, so a changed
            // default reaches browsers that saved the old one automatically.
            contextChosen: _contextChosen,
            include: includeFilterInput?.value.trim() || '',
            exclude: excludeFilterInput?.value.trim() || '',
            regex: regexModeCheckbox?.checked || false,
            symbols: symbolsModeCheckbox?.checked || false,
            references: referencesModeCheckbox?.checked || false,
            matchCase: caseModeCheckbox?.checked || false,
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
        if (s.context != null && s.contextChosen) {
            setSelectValue(contextLinesSelect, s.context);
            _contextChosen = true;
        }
        if (includeFilterInput && s.include) includeFilterInput.value = s.include;
        if (excludeFilterInput && s.exclude) excludeFilterInput.value = s.exclude;
        if (regexModeCheckbox) regexModeCheckbox.checked = s.regex === true;
        if (symbolsModeCheckbox) symbolsModeCheckbox.checked = s.symbols === true;
        if (referencesModeCheckbox) referencesModeCheckbox.checked = s.references === true;
        if (caseModeCheckbox) caseModeCheckbox.checked = s.matchCase === true;
    } catch (_) { /* ignore parse errors */ }
    syncToggleVisuals();
}

// ============================================
// FORM STATE HELPERS
// ============================================

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
    const clamped = clampMaxResults(value);
    if (clamped === null) return;
    const pick = pickMaxResultsOption(clamped, Array.from(maxResultsSelect.options, o => o.value));
    if (pick !== undefined) maxResultsSelect.value = String(pick);
}

/** The validated result cap to send: always an integer in 1..1000. */
function currentMaxResults() {
    return clampMaxResults(maxResultsSelect?.value) ?? 50;
}

/** The validated context-line count to send (0..10). */
function currentContextLines() {
    return clampContextLines(contextLinesSelect?.value || DEFAULT_CONTEXT);
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
    [...modeCheckboxes(), caseModeCheckbox].filter(Boolean).forEach(cb => {
        const label = cb.closest('label');
        if (label) label.classList.toggle('toggle-on', cb.checked);
    });
    syncFilterIndicators();
}

/**
 * Keep path filters visible wherever they apply: a count on the FILTER
 * button, a pill per filter in the results bar (each with ×), and the
 * panel's "Clear path filters". Filters persist in storage, so without
 * this a filter set last week silently narrows today's results.
 */
function syncFilterIndicators() {
    const fields = [
        [includeFilterInput, 'Only in'],
        [excludeFilterInput, 'Skipping'],
    ].filter(([input]) => input && input.value.trim());

    const count = document.getElementById('filter-count');
    if (count) {
        count.textContent = String(fields.length);
        count.hidden = fields.length === 0;
    }
    const toggle = document.getElementById('filter-toggle-btn');
    if (toggle) {
        toggle.title = fields.length
            ? `Filter and ranking options (${fields.length} path filter${fields.length === 1 ? '' : 's'} on)`
            : 'Show filter and ranking options';
    }
    const clear = document.getElementById('clear-filters-btn');
    if (clear) clear.hidden = fields.length === 0;

    const pills = document.getElementById('active-filters');
    if (!pills) return;
    pills.hidden = fields.length === 0;
    pills.innerHTML = fields.map(([input, label]) => `
        <span class="active-filter" title="${escapeHtml(label)}: ${escapeHtml(input.value.trim())}">
            <span>${escapeHtml(label)}: ${escapeHtml(input.value.trim())}</span>
            <button type="button" data-clear="${escapeHtml(input.id)}" aria-label="Remove the ${escapeHtml(label.toLowerCase())} filter">×</button>
        </span>`).join('');
}

/** Empty the given path-filter inputs and search again. */
function clearPathFilters(inputs) {
    inputs.filter(Boolean).forEach(input => { input.value = ''; });
    syncFilterIndicators();
    optionChanged();
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
    select(contextLinesSelect, 'context', DEFAULT_CONTEXT);
    mode(regexModeCheckbox, 'regex');
    mode(symbolsModeCheckbox, 'symbols');
    mode(referencesModeCheckbox, 'references');
    mode(caseModeCheckbox, 'case');
    syncToggleVisuals();

    // Auto-open the VISIBLE filter panel when a shared URL carries filter params,
    // so the applied filters are discoverable.
    const hasAdvanced = params.has('include') || params.has('exclude') ||
        params.has('rank') || params.has('max') || params.has('context');
    if (hasAdvanced) {
        const filterPanel = document.getElementById('filter-panel');
        if (filterPanel) filterPanel.classList.add('open');
        const toggle = document.getElementById('filter-toggle-btn');
        if (toggle) {
            toggle.setAttribute('aria-expanded', 'true');
            toggle.classList.add('toggle-on');
        }
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
    if (caseModeCheckbox?.checked) params.set('case', 'true');

    const rank = rankModeSelect?.value || 'auto';
    if (rank !== 'auto') params.set('rank', rank);

    const context = currentContextLines();
    if (context !== Number(DEFAULT_CONTEXT)) params.set('context', String(context));

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
    additionalInputIds: ['include-filter', 'exclude-filter', 'max-results', 'case-mode', 'regex-mode', 'symbols-mode', 'references-mode', 'rank-mode', 'context-lines'],
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
    let serverProblems = 0;
    try {
        const resp = await fetch('/api/health', { signal: AbortSignal.timeout(2000) });
        if (resp.ok) {
            healthOk = true;
            const data = await resp.json();
            currentServerType = data.server_type ?? null;
            serverProblems = Number(data.problems) || 0;
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
    // The browser logs a connection error for a refused probe that no
    // script can suppress, so the probe only runs where a semantic server
    // is expected (see semanticEnabled()).
    let semanticUp = false;
    if (semanticEnabled()) {
        try {
            const semanticPort = window.SEMANTIC_PORT || 8081;
            const resp = await fetch(`${window.location.protocol}//${hostname}:${semanticPort}/api/health`, { signal: AbortSignal.timeout(2000) });
            semanticUp = resp.ok;
        } catch (e) { /* offline */ }
    }

    renderBackendStatus(keywordAvailable, semanticUp, serverProblems);
}

/**
 * Whether this deployment runs the semantic server on port 8081, i.e.
 * whether to probe it for the status badge. In order of precedence:
 * `window.SEMANTIC_ENABLED`, a `<meta name="fcs-semantic-enabled">` tag,
 * then the presence of the SEMANTIC nav link. index.html ships the meta
 * set to false so a keyword-only install stays silent; the nav link keeps
 * working regardless (semantic.html probes for itself).
 */
function semanticEnabled() {
    if (typeof window.SEMANTIC_ENABLED === 'boolean') return window.SEMANTIC_ENABLED;
    const meta = document.querySelector('meta[name="fcs-semantic-enabled"]');
    if (meta) return ['true', '1', 'yes'].includes((meta.content || '').trim().toLowerCase());
    return !!document.querySelector('a[href="/semantic.html"]');
}

function renderBackendStatus(keywordUp, semanticUp, serverProblems = 0) {
    const banner = document.getElementById('backend-banner');
    if (!banner) return;

    updateBackendBadge('keyword-status-badge', keywordUp, 'Keyword');
    const semanticBadge = document.getElementById('semantic-status-badge');
    if (semanticBadge) semanticBadge.style.display = semanticEnabled() ? '' : 'none';
    updateBackendBadge('semantic-status-badge', semanticUp, 'Semantic');

    const msgEl = document.getElementById('backend-banner-msg');
    if (keywordUp && serverProblems > 0) {
        // The server is up but something at startup went wrong (a busy port,
        // an index owned by another server, a config warning): point at it.
        banner.style.background = 'var(--warn-wash)';
        banner.style.color = 'var(--accent-ink-2)';
        banner.style.display = 'flex';
        if (msgEl) {
            msgEl.textContent = `Server started with ${serverProblems} problem${serverProblems === 1 ? '' : 's'}. `;
            const link = document.createElement('a');
            link.href = '/diagnostics.html';
            link.textContent = 'See diagnostics';
            link.style.textDecoration = 'underline';
            msgEl.appendChild(link);
        }
        return;
    }
    if (keywordUp) {
        banner.style.display = 'none';
        return;
    }

    banner.style.background = 'var(--danger-soft)';
    banner.style.color = 'var(--on-danger-soft)';
    banner.style.display = 'flex';
    if (msgEl) {
        msgEl.textContent = 'Keyword search backend not running \u2014 start fast_code_search on port 8080';
    }
}

function updateBackendBadge(id, isUp, label) {
    const el = document.getElementById(id);
    if (!el) return;
    el.textContent = `${label}: ${isUp ? '\u2713' : '\u2717'}`;
    el.style.background = isUp ? 'var(--info-soft)' : 'var(--danger-soft)';
    el.style.borderColor = isUp ? 'var(--info)' : 'var(--danger)';
    el.style.color = isUp ? 'var(--on-info-soft)' : 'var(--on-danger-soft)';
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
/** Whether this page saw the index being built (so "complete" is news). */
let _sawIndexing = false;
// Fingerprint of the index as last reported over the WebSocket; a change
// means files were added, removed or re-indexed.
let _lastIndexSignature = null;

function updateProgressUI(status) {
    const isIdle = status.status === 'idle';
    const isCompleted = status.status === 'completed';

    // Update search readiness based on status
    searchReadiness.update(status);

    // Hover previews cached before an index change may be stale: drop them
    // when an indexing pass completes or the reported index contents change.
    if (status.num_files !== undefined) {
        const signature = `${status.num_files}|${status.total_content_bytes || 0}|${status.dependency_edges || 0}`;
        if (signature !== _lastIndexSignature) {
            _lastIndexSignature = signature;
            invalidateContextCache();
        }
    }
    if (isCompleted) invalidateContextCache();

    // Show the panel while indexing. On 'completed', keep it visible briefly then
    // hide it — a permanent 100% "Complete" bar otherwise reads as "stuck". 'idle'
    // hides immediately. An index that was already complete when the page
    // opened shows no bar at all: flashing it for a few seconds pushed the
    // search box down and then pulled it back up under the cursor.
    if (!isCompleted && !isIdle) _sawIndexing = true;
    if (isCompleted && !_sawIndexing) {
        clearTimeout(_progressHideTimer);
        toggleElement('progress-panel', false, 'flex');
    } else if (isCompleted) {
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
// MATCH HIGHLIGHTING
//
// The server reports the matched range of every hit as UTF-8 byte offsets
// (`match_start`/`match_end` into `content`, `line_match_start`/`line_match_end`
// into the full line). The match line is highlighted from those offsets, so
// regex hits, case-sensitive hits and `file:`/`lang:`-qualified queries are
// marked exactly where the engine matched. Context lines and file previews
// carry no offsets; for those the query is tokenised the way the server's
// query_syntax.rs does (operators dropped, "phrases" unquoted, case:yes
// honoured) and every remaining term is marked. The pure pieces
// (byteRangeToCharRange, parseQueryTerms, buildQueryMatcher, ...) live in
// lib/keyword-helpers.js, which has Node tests.
// ============================================

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

/**
 * Highlight query terms inside an already syntax-highlighted element, plus
 * any extra character ranges (the server-reported match).
 */
function highlightTermsIn(el, matcher, extraRanges = []) {
    const ranges = matcherRanges(el.textContent, matcher).concat(extraRanges);
    markRanges(el, ranges);
}

/** The matcher for the query currently shown in the results (null = none). */
let _currentMatcher = null;

function currentQueryMatcher() {
    const query = queryInput.value.trim();
    return buildQueryMatcher(query, {
        regex: regexModeCheckbox?.checked || false,
        references: referencesModeCheckbox?.checked || false,
        caseSensitive: caseModeCheckbox?.checked || false,
    });
}

// ============================================
// FILE VIEW HELPER (shared by modal and tooltip)
// ============================================

// Lines rendered on each side of the target line when a file opens, and
// added per "Show more" click. A 60k-line file used to be highlighted and
// laid out in full, line by line, before the modal could scroll.
const FILE_VIEW_WINDOW = 1000;

/**
 * Highlight a snippet cut from the middle of a file (search result context,
 * hover preview) in one pass, so constructs spanning lines keep their
 * colours, starting inside a block comment when the snippet does.
 * @param {string[]} lines
 * @returns {string[]} escaped HTML per line
 */
function highlightSnippetLines(lines, lang) {
    const prefix = snippetHighlightPrefix(lines, lang);
    const out = highlightLines(prefix + lines.join('\n'), lang);
    return prefix ? out.slice(1) : out;
}

/**
 * Fetch a file and show it in `container` in the shared code viewer
 * (code-view.js): syntax and query highlighting, the target line marked and
 * centred, and every line the query matches marked on the ruler. Returns the
 * CodeView, or nothing if `signal` aborted.
 */
async function populateFileView(container, filePath, highlightLine, matcher, signal, opts = {}) {
    let data = _fileContentCache.get(filePath);
    if (!data) {
        const response = await fetch(
            `${API_BASE}/api/file?file=${encodeURIComponent(filePath)}`,
            signal ? { signal } : {}
        );
        if (!response.ok) {
            const text = await response.text();
            const statusMessages = { 404: 'File not found', 403: 'Access denied', 503: 'Server busy' };
            throw new Error(statusMessages[response.status] || text || response.statusText);
        }
        data = await response.json();
        if (_fileContentCache.size >= FILE_CACHE_MAX) _fileContentCache.delete(_fileContentCache.keys().next().value);
        _fileContentCache.set(filePath, data);
    }
    if (signal && signal.aborted) return null;
    const lang = hljsLangForPath(filePath);
    const lines = highlightLines(data.content, lang);
    const target = Math.min(Math.max(1, highlightLine || 1), lines.length);

    container.innerHTML =
        `<div class="file-meta">${Number(data.line_count || lines.length).toLocaleString()} lines · ${formatBytes(data.size_bytes || 0)}` +
            (opts.hint === false ? '' : `<span class="file-meta-hint">${DEFN_KEY_LABEL}-click a name for its definition · double-click for more</span>`) + `</div>` +
        `<div class="file-view-code"></div>`;
    const view = new CodeView(container.querySelector('.file-view-code'), {
        lines,
        lang,
        target,
        mark: highlightLine > 0 ? target : 0,
        hits: matchLineNumbers(data.content, matcher),
        paintLine: matcher ? (span) => highlightTermsIn(span, matcher) : null,
        window: FILE_VIEW_WINDOW,
    });
    view.goTo(target);
    return view;
}

// ============================================
// CONTEXT TOOLTIP
// ============================================

let _ctxTooltip = null;
let _ctxHideTimer = null;
// A pinned preview (its icon was clicked, or the reader clicked or scrolled
// inside it) stays open until Escape, a click outside it, or its × button;
// an unpinned one closes when the pointer leaves.
let _ctxPinned = false;
let _ctxAnchor = null; // the icon the open preview belongs to
let _ctxFetchController = null;
let _ctxHoverTimer = null;
// Cache of /api/file responses keyed by path, so hovering the same file
// again (or opening it after previewing it) doesn't re-fetch it. Kept small,
// and dropped whenever the index may have changed (see
// invalidateContextCache), so a view never shows lines from before a re-index.
const _fileContentCache = new Map();
const FILE_CACHE_MAX = 8;

function invalidateContextCache() {
    _fileContentCache.clear();
}

function getOrCreateTooltip() {
    if (!_ctxTooltip) {
        _ctxTooltip = document.createElement('div');
        _ctxTooltip.id = 'ctx-tooltip';
        _ctxTooltip.className = 'ctx-tooltip';
        // Keep tooltip visible when mouse moves into it
        _ctxTooltip.addEventListener('mouseenter', () => clearTimeout(_ctxHideTimer));
        _ctxTooltip.addEventListener('mouseleave', hideContextTooltip);
        // Using the preview pins it, so it no longer closes on mouse-out.
        _ctxTooltip.addEventListener('pointerdown', () => pinContextTooltip());
        _ctxTooltip.addEventListener('wheel', () => pinContextTooltip(), { passive: true });
        document.body.appendChild(_ctxTooltip);
    }
    return _ctxTooltip;
}

function hideContextTooltip() {
    if (_ctxPinned) return;
    _ctxHideTimer = setTimeout(() => {
        if (_ctxTooltip) {
            _ctxTooltip.style.display = 'none';
        }
    }, 150);
}

function hideContextTooltipImmediately() {
    _ctxPinned = false;
    clearTimeout(_ctxHideTimer);
    clearTimeout(_ctxHoverTimer);
    if (_ctxFetchController) {
        _ctxFetchController.abort();
        _ctxFetchController = null;
    }
    if (_ctxTooltip) {
        _ctxTooltip.style.display = 'none';
    }
}

function pinContextTooltip() {
    _ctxPinned = true;
    clearTimeout(_ctxHideTimer);
    if (_ctxTooltip) _ctxTooltip.classList.add('ctx-pinned');
}

function isContextTooltipVisible() {
    return !!_ctxTooltip && _ctxTooltip.style.display !== 'none';
}

// The preview used to hide only on mouseleave or when a modal opened, so it
// stayed on screen across Escape, page scrolls and viewport changes (and
// covered the mobile layout). Dismiss it on Escape, on any scroll or click
// outside it, and on resize (its position was computed from the anchor).
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && isContextTooltipVisible()) hideContextTooltipImmediately();
});
window.addEventListener('scroll', (e) => {
    if (!isContextTooltipVisible() || _ctxPinned) return;
    if (e.target instanceof Node && _ctxTooltip.contains(e.target)) return; // scrolling the preview itself
    hideContextTooltipImmediately();
}, true);
document.addEventListener('click', (e) => {
    if (!isContextTooltipVisible()) return;
    if (e.target instanceof Node && _ctxTooltip.contains(e.target)) return;
    // The name menu opened from inside the preview acts on the preview.
    if (e.target instanceof Element && e.target.closest('.code-menu')) return;
    // A click on the icon that opened it pins it (see wireResultGroups).
    if (_ctxAnchor && e.target instanceof Node && _ctxAnchor.contains(e.target)) return;
    hideContextTooltipImmediately();
}, true);
window.addEventListener('resize', () => {
    if (isContextTooltipVisible()) hideContextTooltipImmediately();
});

/**
 * The hover preview: the whole file, scrollable, opened at the hit (a file
 * row opens at the top with no line marked), with buttons to step through
 * the query's matches. It used to fetch ±12 lines around the hit, so it
 * could not scroll, and wheeling over it scrolled the page and closed it.
 */
async function showContextTooltip(resultItem, filePath, lineNumber, wholeFile = false) {
    clearTimeout(_ctxHideTimer);
    if (_ctxFetchController) _ctxFetchController.abort();
    _ctxFetchController = new AbortController();
    const signal = _ctxFetchController.signal;

    const tooltip = getOrCreateTooltip();
    _ctxAnchor = resultItem;
    tooltip.innerHTML = `
        <div class="ctx-header">
            <span class="ctx-path">${escapeHtml(filePath)}${wholeFile ? '' : ` : ${lineNumber}`}</span>
            <span class="ctx-nav" hidden>
                <button type="button" data-step="-1" title="Previous match" aria-label="Previous match">↑</button>
                <span class="ctx-nav-pos"></span>
                <button type="button" data-step="1" title="Next match" aria-label="Next match">↓</button>
            </span>
            <button type="button" class="ctx-copy" title="Copy a link that opens this file at the selected line">Copy link</button>
            ${hasImportGraph(filePath) ? `<a class="ctx-explore" href="${escapeHtml(graphExplorerUrl(filePath))}" title="Open this file in the dependency explorer">Explore imports</a>` : ''}
            <button type="button" class="ctx-open" title="Show this file in the large viewer">Expand</button>
            <button type="button" class="ctx-close" title="Close (Esc)" aria-label="Close preview">×</button>
        </div>
        <div class="ctx-file-body"><div class="ctx-loading">Loading…</div></div>`;
    // The preview does what the large viewer does (go to definition, select
    // a line, copy a link, explore imports); Expand only makes it bigger, at
    // the line the preview is on (↑/↓ or a clicked line number may have
    // moved it).
    tooltip._file = filePath;
    tooltip._line = lineNumber;
    tooltip.querySelector('.ctx-open').addEventListener('click', () => {
        const line = tooltip._line;
        hideContextTooltipImmediately();
        showFileModal(filePath, line);
    });
    const copyBtn = tooltip.querySelector('.ctx-copy');
    copyBtn.addEventListener('click', async () => {
        const url = location.origin + location.pathname + location.search + fileViewHash(filePath, tooltip._line || 1);
        const ok = await copyTextToClipboard(url);
        copyBtn.textContent = ok ? 'Copied' : 'Copy failed';
        setTimeout(() => { copyBtn.textContent = 'Copy link'; }, 1500);
    });
    tooltip.querySelector('.ctx-close').addEventListener('click', hideContextTooltipImmediately);
    tooltip.classList.toggle('ctx-pinned', _ctxPinned);
    positionTooltip(tooltip, resultItem);
    tooltip.style.display = 'flex';

    const body = tooltip.querySelector('.ctx-file-body');
    try {
        const view = await populateFileView(body, filePath, wholeFile ? 0 : lineNumber,
            _currentMatcher || currentQueryMatcher(), signal);
        if (!view || signal.aborted) return;
        wireContextNav(tooltip, view, wholeFile ? 0 : lineNumber);
        // Clicking a line number selects it, as in the large viewer.
        body.addEventListener('click', (e) => {
            const num = e.target.closest('.file-line-num');
            const line = num ? parseInt(num.textContent, 10) : 0;
            if (!(line > 0)) return;
            view.setMark(line);
            tooltip._line = line;
        });
    } catch (e) {
        if (e.name === 'AbortError') return;
        body.innerHTML = `<div class="error-message">${escapeHtml(e.message)}</div>`;
    }
}

/**
 * "↑ 3 of 12 ↓": step the preview through the lines the query matches in
 * the whole file (not just the lines laid out), as the ruler marks them.
 */
function wireContextNav(tooltip, view, startLine) {
    const nav = tooltip.querySelector('.ctx-nav');
    const pos = tooltip.querySelector('.ctx-nav-pos');
    const lines = view.hits;
    if (!lines.length) return;
    let current = Math.max(0, lines.indexOf(startLine));
    const show = (i, scroll) => {
        current = (i + lines.length) % lines.length;
        view.linesEl.querySelectorAll('.ctx-current').forEach(l => l.classList.remove('ctx-current'));
        const row = scroll ? view.goTo(lines[current]) : view.row(lines[current]);
        if (row) row.classList.add('ctx-current');
        tooltip._line = lines[current];
        pos.textContent = `${(current + 1).toLocaleString()} of ${lines.length.toLocaleString()}`;
    };
    nav.hidden = false;
    show(current, false);
    view.onJump = (line) => show(lines.indexOf(line), false);
    nav.addEventListener('click', (e) => {
        const btn = e.target.closest('button[data-step]');
        if (btn) show(current + Number(btn.dataset.step), true);
    });
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
 * `offset`), the hits loaded so far, and the last response. "Load more"
 * fetches a longer list (see loadMoreRequest) and re-renders.
 */
let _currentSearch = null;

/** Build the /api/search query string for the current form state. */
function buildSearchParams(s) {
    const params = new URLSearchParams({ q: s.query, max: String(s.maxResults) });
    const include = pathFilterParam(s.includeFilter);
    const exclude = pathFilterParam(s.excludeFilter);
    if (include) params.set('include', include);
    if (exclude) params.set('exclude', exclude);
    // The three modes are mutually exclusive in the UI (the server lets
    // references and symbols silently win over regex).
    if (s.isReferences) params.set('references', 'true');
    else if (s.symbolsOnly) params.set('symbols', 'true');
    else if (s.isRegex) params.set('regex', 'true');
    if (s.rankMode !== 'auto') params.set('rank', s.rankMode);
    if (s.contextLines > 0) params.set('context', String(s.contextLines));
    // MATCH CASE means the same in every mode. Off, plain text needs nothing
    // (it ignores case unless the query says case:yes) but a regex, which
    // the server matches case-sensitively by default, is sent case=false.
    const caseParam = searchCaseParam(s);
    if (caseParam !== null) params.set('case', caseParam);
    // More candidate files than fast ranking opens by default.
    if (s.maxFiles) params.set('max_files', String(s.maxFiles));
    // Files whose path matches (text and regex searches only).
    if (!s.isReferences && !s.symbolsOnly) params.set('files', String(FILE_MATCHES_FETCH));
    return params;
}

// How many times a 503 + Retry-After (write lock held while the index is
// updated) is retried before the error is shown.
const SEARCH_RETRY_MAX = 2;

/** A delay that rejects with an AbortError when `signal` fires. */
function abortableDelay(ms, signal) {
    return new Promise((resolve, reject) => {
        if (signal?.aborted) { reject(new DOMException('Aborted', 'AbortError')); return; }
        const timer = setTimeout(() => {
            signal?.removeEventListener('abort', onAbort);
            resolve();
        }, ms);
        function onAbort() {
            clearTimeout(timer);
            reject(new DOMException('Aborted', 'AbortError'));
        }
        signal?.addEventListener('abort', onAbort, { once: true });
    });
}

/**
 * Fetch one page of results. Non-OK responses surface the server's error
 * body (e.g. "Invalid regex pattern: …") instead of a bare status text.
 *
 * A 503 with Retry-After means the index is being updated (the write lock
 * is briefly held per batch); it is retried up to SEARCH_RETRY_MAX times
 * under the same AbortController after the indicated delay, so a search
 * typed during indexing does not stick as a red error until the next key.
 * @param {URLSearchParams} params
 * @param {AbortSignal} signal
 * @param {(attempt: number, max: number) => void} [onRetry] - called before each wait
 */
async function fetchSearchPage(params, signal, onRetry) {
    for (let attempt = 0; ; attempt++) {
        const response = await fetch(`${API_BASE}/api/search?${params}`, { signal });
        if (response.ok) return response.json();
        const retryAfter = response.headers.get('Retry-After');
        if (response.status === 503 && retryAfter !== null && attempt < SEARCH_RETRY_MAX) {
            const secs = parseFloat(retryAfter);
            const delayMs = Number.isFinite(secs) ? Math.min(5000, Math.max(250, secs * 1000)) : 1000;
            if (onRetry) onRetry(attempt + 1, SEARCH_RETRY_MAX);
            await abortableDelay(delayMs, signal);
            continue;
        }
        throw new Error(await readErrorBody(response));
    }
}

/**
 * Run the search for the current form state.
 * @param {{trigger?: 'input'|'submit'|'option'|'history', maxFiles?: number}} [opts]
 *   `maxFiles` asks fast ranking to open that many files (SEARCH MORE FILES).
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
        matchCase: caseModeCheckbox?.checked || false,
        maxFiles: opts.maxFiles || null,
        rankMode: rankModeSelect?.value || 'auto',
        contextLines: currentContextLines(),
        results: [],
        last: null,
        loading: false,
    };

    clearFileMatches();

    // Keep URL/history in sync so searches can be shared and navigated
    writeUrlState(trigger);
    // Persist settings to localStorage
    saveSettingsToStorage();

    if (!query) {
        _currentSearch = null;
        resetRenderState();
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter a search query to find code</p></div>';
        return;
    }

    if (query.length < 3) {
        _currentSearch = null;
        resetRenderState();
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter at least 3 characters to search</p></div>';
        return;
    }

    resetRenderState();
    resultsContainer.innerHTML = '<div class="loading">Searching...</div>';
    resultsHeader.style.display = 'none';
    // A new search may follow a watcher-driven re-index the progress stream
    // did not report (e.g. this tab was hidden); previews are cheap to refetch.
    invalidateContextCache();

    const startTime = performance.now();

    try {
        search.params = buildSearchParams(search);
        const data = await fetchSearchPage(search.params, signal, (attempt, max) => {
            resultsContainer.innerHTML =
                `<div class="loading">Index updating, retrying… (${attempt}/${max})</div>`;
        });
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
        if (search.isRegex && /regex/i.test(error.message)) {
            showRegexError(search, error.message);
        } else {
            showError('results', error.message);
        }
    }
}

// ============================================
// REGEX SEARCH HELP (no-result suggestions, regex errors, cheat-sheet)
// ============================================

/** Switch the form to `suggestion` ({query, regex}) and search. */
function applySuggestion(suggestion) {
    queryInput.value = suggestion.query;
    if (regexModeCheckbox) regexModeCheckbox.checked = suggestion.regex;
    if (suggestion.regex) {
        modeCheckboxes().forEach(cb => { if (cb !== regexModeCheckbox) cb.checked = false; });
    }
    syncToggleVisuals();
    submitSearch();
}

/** A row of suggestion buttons; `counts` (optional) labels each with its hits. */
function suggestionButtonsHtml(suggestions, counts) {
    return suggestions.map((s, i) => {
        const count = counts ? ` <span class="suggestion-count">${escapeHtml(counts[i])}</span>` : '';
        return `<button type="button" class="suggestion-btn" data-suggestion="${i}">`
            + `<span class="suggestion-label">${escapeHtml(s.label)}${count}</span>`
            + `<code>${escapeHtml(s.query)}</code></button>`;
    }).join('');
}

function bindSuggestionButtons(container, suggestions) {
    container.querySelectorAll('[data-suggestion]').forEach(btn => {
        btn.addEventListener('click', () => applySuggestion(suggestions[Number(btn.dataset.suggestion)]));
    });
}

const REGEX_HELP_LINK = '<button type="button" class="regex-help-link" data-open-regex-help>Regex tips</button>';

function bindRegexHelpLinks(container) {
    container.querySelectorAll('[data-open-regex-help]').forEach(btn => {
        btn.addEventListener('click', () => toggleRegexHelp(true));
    });
}

/**
 * After a search with no results, try the alternatives from
 * noResultSuggestions (one result each, in parallel) and offer the ones that
 * find something. Runs only on an empty result, so normal searches cost
 * nothing extra; a newer search aborts the probes.
 */
async function showNoResultSuggestions(search) {
    const box = document.getElementById('search-suggestions');
    if (!box) return;
    const signal = _searchAbort?.signal;
    const candidates = noResultSuggestions(search.query, search.isRegex, search.matchCase);
    const caseTip = search.matchCase ? '<span class="label-case">Match case</span> is on. ' : '';
    const tips = search.isRegex
        ? `<p class="suggestion-hint">${caseTip}Regex matches one line at a time. ${REGEX_HELP_LINK}</p>`
        : (caseTip ? `<p class="suggestion-hint">${caseTip}</p>` : '');
    if (candidates.length === 0) {
        box.innerHTML = tips;
        bindRegexHelpLinks(box);
        return;
    }
    box.innerHTML = '<p class="suggestion-hint">Checking similar searches…</p>';
    const probes = await Promise.all(candidates.map(async (c) => {
        const params = buildSearchParams({ ...search, query: c.query, isRegex: c.regex, maxResults: 1, contextLines: 0 });
        try {
            const data = await fetchSearchPage(params, signal);
            const n = typeof data.total_matches === 'number' ? data.total_matches : data.results.length;
            if (n === 0) return null;
            const label = typeof data.total_matches === 'number' || !data.has_more ? `${n.toLocaleString()} found` : `${n}+ found`;
            return { suggestion: c, label };
        } catch (_) {
            return null; // an alternative that errors (or was aborted) is simply not offered
        }
    }));
    if (signal?.aborted || !box.isConnected) return;
    const found = probes.filter(Boolean);
    if (found.length === 0) {
        box.innerHTML = tips;
    } else {
        const suggestions = found.map(f => f.suggestion);
        box.innerHTML = '<p class="suggestion-hint">These similar searches find something:</p>'
            + `<div class="suggestion-list">${suggestionButtonsHtml(suggestions, found.map(f => f.label))}</div>`
            + tips;
        bindSuggestionButtons(box, suggestions);
    }
    bindRegexHelpLinks(box);
}

/**
 * A regex that does not compile: show the parser's message as written (its
 * caret points at the problem) with ways out: search the text literally,
 * escape it, or open the cheat-sheet.
 */
function showRegexError(search, message) {
    const suggestions = regexErrorSuggestions(search.query);
    resultsHeader.style.display = 'none';
    resultsContainer.innerHTML = `<div class="error-message regex-error">`
        + `<strong>This is not a valid regex.</strong>`
        + ` Characters such as <code>{ ( [ . * + ?</code> have a special meaning; put <code>\\</code> before one to match it literally.`
        + `<pre>${escapeHtml(message)}</pre>`
        + `<div class="suggestion-list">${suggestionButtonsHtml(suggestions)}</div>`
        + `<p class="suggestion-hint">${REGEX_HELP_LINK}</p></div>`;
    bindSuggestionButtons(resultsContainer, suggestions);
    bindRegexHelpLinks(resultsContainer);
}

/** Show (`open` true), hide (false) or toggle (undefined) the regex cheat-sheet. */
function toggleRegexHelp(open) {
    const panel = document.getElementById('regex-help');
    const btn = document.getElementById('regex-help-btn');
    if (!panel) return;
    const show = open === undefined ? !panel.classList.contains('open') : open;
    panel.classList.toggle('open', show);
    if (btn) btn.setAttribute('aria-expanded', String(show));
    if (show) panel.scrollIntoView({ block: 'nearest' });
}

document.getElementById('regex-help-btn')?.addEventListener('click', () => toggleRegexHelp());
document.getElementById('regex-help-close')?.addEventListener('click', () => toggleRegexHelp(false));
document.querySelectorAll('#regex-help [data-example]').forEach(btn => {
    btn.addEventListener('click', () => applySuggestion({ query: btn.dataset.example, regex: true }));
});

/**
 * Fetch more results: below the server's cap the whole list again at the
 * bigger size, replacing what is shown (offset pages can repeat or skip hits
 * when the search stops at its match budget); past the cap, the next page.
 */
async function loadMoreResults() {
    const search = _currentSearch;
    if (!search || search.loading || !search.last?.has_more) return;
    search.loading = true;

    const btn = document.getElementById('load-more-btn');
    if (btn) {
        btn.disabled = true;
        btn.textContent = 'Loading…';
    }

    if (_searchAbort) _searchAbort.abort();
    _searchAbort = new AbortController();
    const signal = _searchAbort.signal;

    try {
        const next = loadMoreRequest(search.results.length, search.maxResults);
        const params = new URLSearchParams(search.params);
        params.set('max', String(next.max));
        if (next.offset > 0) params.set('offset', String(next.offset));
        const data = await fetchSearchPage(params, signal, (attempt, max) => {
            const b = document.getElementById('load-more-btn');
            if (b) b.textContent = `Index updating, retrying… (${attempt}/${max})`;
        });
        if (signal.aborted || _currentSearch !== search) return;
        const firstNewGroup = groupResultsByFile(search.results).length;
        // Keep everything that was on screen (plus the first new chunk) in the
        // re-render, so the page does not shrink under the reader.
        const renderedBefore = getResultGroups().length;
        search.results = next.redraw ? data.results : search.results.concat(data.results);
        search.last = data;
        renderSearch(search, data.elapsed_ms, {
            preserveSelection: true,
            minRendered: renderedBefore + RENDER_CHUNK_GROUPS,
        });
        // Move focus to the first newly loaded group so keyboard users land
        // where the new content starts.
        ensureGroupsRendered(firstNewGroup + 1);
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

/** Tooltip for the LOAD MORE button. */
function loadMoreTitle(search) {
    const next = loadMoreRequest(search.results.length, search.maxResults);
    return next.redraw
        ? `Search again for the first ${next.max} results`
        : `Fetch the next ${next.max} results (offset ${next.offset})`;
}

/** Render the current search (all pages loaded so far) into the results area. */
function renderSearch(search, durationMs, opts = {}) {
    const data = search.last;
    const query = search.query;

    resultsHeader.style.display = 'flex';
    const label = resultsCountLabel(data, search.results.length);
    resultsCount.textContent = label.text;
    resultsCount.title = label.title;
    const meta = searchMetaLabel(data, durationMs, searchIgnoresCase(search));
    searchMetaEl.textContent = meta.text;
    searchMetaEl.title = meta.title;
    const more = moreFilesRequest(data);
    if (moreFilesBtn) {
        moreFilesBtn.style.display = more ? '' : 'none';
        moreFilesBtn.textContent = more ? more.label : '';
        moreFilesBtn.title = more
            ? `Open ${more.maxFiles.toLocaleString('en-US')} of the ${Number(data.total_candidates).toLocaleString('en-US')} files that could contain the query (slower)`
            : '';
        moreFilesBtn.onclick = more ? () => performSearch({ trigger: 'option', maxFiles: more.maxFiles }) : null;
    }
    const freshnessEl = document.getElementById('index-freshness');
    const freshness = indexFreshnessLabel(data.index, Date.now());
    if (freshnessEl) {
        freshnessEl.style.display = freshness ? '' : 'none';
        freshnessEl.textContent = freshness ? freshness.text : '';
        freshnessEl.title = freshness ? freshness.title : '';
    }

    renderFileMatches(data);

    if (search.results.length === 0) {
        resetRenderState();
        resultsContainer.removeAttribute('role');
        resultsContainer.removeAttribute('aria-label');
        const noHits = data.files && data.files.length
            ? `No lines match "${escapeHtml(query)}"; the files above match by path.`
            : `No results found for "${escapeHtml(query)}"`;
        resultsContainer.innerHTML = `<div class="empty-state no-results"><p>${noHits}</p>`
            + '<div id="search-suggestions" class="search-suggestions" aria-live="polite"></div></div>';
        if (!search.symbolsOnly && !search.isReferences) showNoResultSuggestions(search);
        return;
    }

    const groupedResults = groupResultsByFile(search.results);
    if (!opts.preserveSelection) _selectedGroupIndex = -1; // reset keyboard selection on each new search
    // Matcher for lines without server offsets (context lines, previews).
    _currentMatcher = buildQueryMatcher(query, {
        regex: search.isRegex,
        references: search.isReferences,
        caseSensitive: search.matchCase,
    });

    // Render on demand: the first RENDER_INITIAL_GROUPS groups now, the rest
    // in chunks of RENDER_CHUNK_GROUPS as the sentinel scrolls into view. A
    // "Load more" re-render keeps at least what was on screen before.
    resetRenderState();

    // The groups are a keyboard-selectable list (j/k, Enter); the listbox
    // role gives aria-selected on each group meaning.
    resultsContainer.setAttribute('role', 'listbox');
    resultsContainer.setAttribute('aria-label', `Search results for ${query}`);
    // Paging: LOAD MORE asks for a longer list (loadMoreRequest).
    resultsContainer.innerHTML =
        `<div class="render-sentinel" aria-hidden="true"></div>` +
        (data.has_more
            ? `<div id="load-more-row" class="load-more-row">
                <button id="load-more-btn" type="button" class="load-more-btn"
                    title="${loadMoreTitle(search)}">Load more</button>
               </div>`
            : '');

    const loadMoreBtn = document.getElementById('load-more-btn');
    if (loadMoreBtn) loadMoreBtn.addEventListener('click', loadMoreResults);

    _render = {
        groups: groupedResults,
        rendered: 0,
        sentinel: resultsContainer.querySelector('.render-sentinel'),
        observer: null,
    };
    const initial = Math.max(RENDER_INITIAL_GROUPS, opts.minRendered || 0, _selectedGroupIndex + 1);
    renderPendingGroups(initial);
    if (_render && _render.rendered < _render.groups.length) {
        if (typeof IntersectionObserver === 'function') {
            _render.observer = new IntersectionObserver((entries, observer) => {
                if (!entries.some(e => e.isIntersecting)) return;
                renderPendingGroups(RENDER_CHUNK_GROUPS);
                // Inserting a chunk moves the sentinel down but may leave it
                // intersecting, which is not a state change and would not be
                // reported; re-observing delivers a fresh entry either way.
                if (_render && _render.observer === observer) {
                    observer.unobserve(_render.sentinel);
                    observer.observe(_render.sentinel);
                }
            }, { rootMargin: '600px 0px' });
            _render.observer.observe(_render.sentinel);
        } else {
            renderPendingGroups(Infinity);
        }
    }

    // Roving tabindex: the selected group (or the first) is the one Tab reaches.
    highlightSelectedGroup(getResultGroups(), { focus: false });
}

// ============================================
// FILES WHOSE PATH MATCHES (above the content results)
// ============================================

/** Path matches asked of the server, and how many show before "Show all". */
const FILE_MATCHES_FETCH = 50;
const FILE_MATCHES_COLLAPSED = 5;

/** File type icon based on extension. */
function fileIconName(filePath) {
    const ext = (filePath.split('.').pop() || '').toLowerCase();
    if (ext === 'md') return 'description';
    return ['yaml', 'yml', 'toml', 'json'].includes(ext) ? 'data_object' : 'code';
}

function clearFileMatches() {
    if (!fileMatchesSection) return;
    fileMatchesSection.style.display = 'none';
    fileMatchesSection.innerHTML = '';
}

/**
 * One path, its directory muted and the matched parts marked (a match that
 * crosses into the file name stays one mark).
 */
function fileMatchPathHtml(path, ranges) {
    let html = '';
    let inMark = false;
    for (const run of pathMatchRuns(path, ranges)) {
        if (run.hit !== inMark) {
            html += run.hit ? '<mark class="highlight">' : '</mark>';
            inMark = run.hit;
        }
        const style = run.name ? 'color:var(--accent-ink);font-weight:700' : 'color:var(--text-3)';
        html += `<span style="${style}">${escapeHtml(run.text)}</span>`;
    }
    return inMark ? html + '</mark>' : html;
}

/** The "Files" group: the response's path matches, or nothing. */
function renderFileMatches(data) {
    if (!fileMatchesSection) return;
    const files = (data && data.files) || [];
    if (!files.length) {
        clearFileMatches();
        return;
    }
    const total = typeof data.total_files === 'number' ? data.total_files : files.length;
    const rows = files.map((f, i) => `
        <li${i >= FILE_MATCHES_COLLAPSED ? ' class="file-match-extra" hidden' : ''}>
          <div style="display:flex;align-items:center;gap:12px;padding-right:16px;border-top:1px solid var(--border-soft)">
            <button type="button" class="file-match-row" data-file-path="${escapeHtml(f.file_path)}"
                title="Open ${escapeHtml(f.file_path)}"
                style="display:flex;align-items:center;gap:8px;flex:1;min-width:0;text-align:left;padding:6px 0 6px 16px;background:none;border:none;cursor:pointer;font-family:var(--font-mono);font-size:12px">
                ${iconSvg(fileIconName(f.file_path), 16)}
                <span class="truncate">${fileMatchPathHtml(f.file_path, f.match_ranges)}</span>
            </button>
            <div class="flex items-center gap-2 flex-shrink-0">
                ${hasImportGraph(f.file_path) ? `<a class="explore-graph-link hover:text-primary transition-colors"
                    style="color:var(--text-3);display:flex"
                    href="${escapeHtml(graphExplorerUrl(f.file_path))}"
                    aria-label="Explore this file's imports"
                    title="Explore imports in the dependency explorer">${iconSvg('account_tree', 16)}</a>` : ''}
                <button type="button" class="copy-path-btn hover:text-primary transition-colors"
                    style="cursor:pointer;color:var(--text-3);background:none;border:none;padding:0;display:flex"
                    data-file-path="${escapeHtml(f.file_path)}"
                    aria-label="Copy file path"
                    title="Copy file path">${iconSvg('content_copy', 16)}</button>
                <button type="button" class="view-file-btn hover:text-primary transition-colors"
                    style="cursor:pointer;color:var(--text-3);background:none;border:none;padding:0;display:flex"
                    data-file-path="${escapeHtml(f.file_path)}"
                    data-line-number="1"
                    data-whole-file="true"
                    aria-label="View full file"
                    title="View full file">${iconSvg('open_in_new', 18)}</button>
            </div>
          </div>
        </li>`).join('');
    const hiddenCount = Math.max(0, files.length - FILE_MATCHES_COLLAPSED);
    const notFetched = total - files.length;
    const footer = hiddenCount > 0
        ? `<button type="button" class="file-matches-more load-more-btn" style="margin:8px 16px">Show ${hiddenCount} more</button>`
        : '';
    const narrow = notFetched > 0
        ? `<p class="file-matches-narrow" ${hiddenCount > 0 ? 'hidden' : ''} style="margin:8px 16px;font-size:12px;color:var(--text-3)">${notFetched.toLocaleString('en-US')} more not shown; add to the path to narrow it.</p>`
        : '';
    fileMatchesSection.innerHTML = `
        <div class="bg-white border border-black rounded overflow-hidden" style="box-shadow:var(--shadow-sm);margin-bottom:16px">
            <div class="border-b border-black px-4 py-2 flex items-center gap-2" style="background:var(--surface-head)">
                ${iconSvg('description', 16)}
                <span class="font-label label-case text-xs font-bold tracking-tight">Files</span>
                <span class="label-case" style="padding:2px 6px;background:var(--chip);border:1px solid var(--border);color:var(--text-2);font-size:10px;font-family:var(--font-label)">${fileMatchesLabel(total)}</span>
            </div>
            <ul style="list-style:none;margin:0;padding:0">${rows}</ul>
            ${footer}${narrow}
        </div>`;
    fileMatchesSection.style.display = '';
    fileMatchesSection.querySelectorAll('.file-match-row').forEach(btn => {
        btn.addEventListener('click', () => showFileModal(btn.dataset.filePath, 1));
    });
    // Preview-on-hover, view and copy buttons work as on a result group.
    wireResultGroups(fileMatchesSection);
    const more = fileMatchesSection.querySelector('.file-matches-more');
    if (more) {
        more.addEventListener('click', () => {
            fileMatchesSection.querySelectorAll('.file-match-extra').forEach(li => { li.hidden = false; });
            const note = fileMatchesSection.querySelector('.file-matches-narrow');
            if (note) note.hidden = false;
            more.remove();
        });
    }
}

// ============================================
// RESULT RENDERING (on demand)
// ============================================

const RENDER_INITIAL_GROUPS = 60;
const RENDER_CHUNK_GROUPS = 40;

/**
 * The result groups of the search on screen and how many are in the DOM.
 * null when nothing is rendered.
 * @type {{groups: Array, rendered: number, sentinel: Element, observer: IntersectionObserver|null}|null}
 */
let _render = null;

function resetRenderState() {
    if (_render && _render.observer) _render.observer.disconnect();
    _render = null;
}

/** Total groups of the current search, rendered or not. */
function totalResultGroups() {
    return _render ? _render.groups.length : getResultGroups().length;
}

/**
 * Append up to `count` not-yet-rendered groups before the sentinel, wiring
 * their handlers and highlighting on the detached fragment first. Removes
 * the sentinel once every group is in the DOM.
 */
function renderPendingGroups(count) {
    const st = _render;
    if (!st || st.rendered >= st.groups.length) return;
    const end = Math.min(st.groups.length, st.rendered + count);
    const holder = document.createElement('div');
    holder.innerHTML = st.groups.slice(st.rendered, end).map(renderGroupHtml).join('');
    wireResultGroups(holder);
    const nodes = Array.from(holder.children);
    // New groups are Tab-unreachable (-1) unless they hold the selection; the
    // first group's roving tabindex is set by highlightSelectedGroup.
    if (st.sentinel.parentNode) st.sentinel.before(...nodes);
    else resultsContainer.append(...nodes);
    st.rendered = end;
    if (st.rendered >= st.groups.length) {
        if (st.observer) st.observer.disconnect();
        st.observer = null;
        st.sentinel.remove();
    }
}

/** Make sure at least the first `n` groups are in the DOM (j/k, Load more). */
function ensureGroupsRendered(n) {
    while (_render && _render.rendered < Math.min(n, _render.groups.length)) {
        renderPendingGroups(RENDER_CHUNK_GROUPS);
    }
}

/** The HTML of one file group (its header and every hit). */
function renderGroupHtml(group) {
    const firstHit = group.hits[0];
    const depCount = Math.max(...group.hits.map(hit => hit.dependency_count || 0));
    const lang = hljsLangForPath(group.filePath);
    const ext = fileExtension(group.filePath);
    const langClass = langClassForPath(group.filePath);

    // Split path into directory + filename for display
    const pathParts = group.filePath.split('/');
    const fileName = pathParts.pop();
    const dirPath = pathParts.length ? pathParts.join('/') + '/' : '';

    const fileIcon = fileIconName(group.filePath);

    const depBadge = depCount > 0
        ? `<button type="button" class="deps-badge result-badge"
            aria-label="${depCount} dependents; show files that import this file"
            data-file-path="${escapeHtml(group.filePath)}" title="Files that import this one">imported by ${depCount}</button>`
        : '';
    // A muted outline badge with the language's colour as a dot, so it no
    // longer competes with the match highlight.
    const langBadge = ext
        ? `<span class="result-badge lang-badge"><span class="lang-dot" style="background:var(--lang-${langClass || 'default'}, var(--text-4))"></span>${escapeHtml(ext)}</span>`
        : '';

    // A filename hit has line_number 0: the viewer opens at line 1 for it.
    const groupViewLine = Math.max(1, firstHit.line_number || 0);

    // Each hit is its code: the line number in a gutter (the match line's
    // number opens the file there and previews it on hover), the hit's kind
    // as a small tag when it is a definition or reference. It used to be a
    // header row ("line 76 · Symbol · open") above the code.
    const hitsHtml = group.hits.map((result) => {
        const matchType = getMatchTypeLabel(result.match_type);
        const isFilenameHit = !result.line_number;
        const viewLine = isFilenameHit ? 1 : result.line_number;
        const tags = (matchType.isSymbol ? `<span class="hit-tag">${matchType.text}</span>` : '') +
            (result.content_truncated
                ? `<span class="hit-tag truncated-badge" title="The line is longer than the 500-byte content window; open the file to see all of it">… truncated</span>`
                : '');
        const title = `${isFilenameHit ? 'File name match' : `Line ${result.line_number}`} · ${matchType.text.toLowerCase()} · ranking score ${result.score.toFixed(2)}&#10;Click to open the file here`;
        // The preview icon at the end of the match line: hover it to preview
        // the file, click it to open the file at this line.
        const openBtn = `<button type="button" class="hit-open view-file-btn" data-file-path="${escapeHtml(result.file_path)}" data-line-number="${viewLine}"
            aria-label="View file at ${isFilenameHit ? 'the top' : 'line ' + result.line_number}" title="Preview the file (click to keep it open)">${iconSvg('open_in_new', 16)}</button>`;
        const matchNum = `<button type="button" class="hit-num hit-num-open" data-file-path="${escapeHtml(result.file_path)}" data-line-number="${viewLine}"
            aria-label="Open file at ${isFilenameHit ? 'the top' : 'line ' + result.line_number}" title="${title}">${isFilenameHit ? 'file' : result.line_number}</button>`;

        // With context, every line of the window; without, the match line
        // alone (its offsets are into `content` rather than the full line).
        const hasContext = result.context_lines && result.context_lines.length > 0;
        const rows = hasContext
            ? result.context_lines.map((text, i) => ({ text, n: (result.context_start_line || 1) + i }))
            : [{ text: result.content, n: result.line_number }];
        const codeRows = rows.map(({ text, n }) => {
            const isMatch = n === result.line_number;
            // The match line carries the server's byte offsets so the
            // highlight pass can mark the exact hit.
            const offsets = !isMatch ? ''
                : hasContext
                    ? ` data-lms="${Number(result.line_match_start) || 0}" data-lme="${Number(result.line_match_end) || 0}"`
                    : ` data-lms="${Number(result.match_start) || 0}" data-lme="${Number(result.match_end) || 0}"`;
            return `<div class="hit-line${isMatch ? ' hit-match' : ''}">` +
                (isMatch ? matchNum : `<span class="hit-num">${n}</span>`) +
                `<span class="ctx-line-content${isMatch ? ' match-line' : ''}"${offsets}>${escapeHtml(text)}</span>` +
                (isMatch && tags ? `<span class="hit-tags">${tags}</span>` : '') +
                (isMatch ? openBtn : '') +
                `</div>`;
        }).join('');

        return `<pre class="result-code result-code-ctx language-${lang}" data-has-context="true" data-lang="${lang}">${codeRows}</pre>`;
    }).join('');

    return `
        <div class="result-group bg-white border border-black rounded overflow-hidden" style="box-shadow:var(--shadow-sm)" role="option" tabindex="-1" aria-selected="false" aria-label="${escapeHtml(group.filePath)}, ${group.hits.length} hit${group.hits.length !== 1 ? 's' : ''}" data-file-path="${escapeHtml(group.filePath)}" data-line-number="${groupViewLine}">
            <div class="group-head">
                <button type="button" class="group-open" data-file-path="${escapeHtml(group.filePath)}" data-line-number="${groupViewLine}"
                    title="Open ${escapeHtml(group.filePath)}">
                    ${iconSvg(fileIcon, 16)}
                    <span class="group-path font-mono">${dirPath ? `<span class="group-dir">${escapeHtml(dirPath)}</span>` : ''}<span class="group-name">${escapeHtml(fileName)}</span></span>
                </button>
                <span class="result-badge group-count">${group.hits.length} hit${group.hits.length !== 1 ? 's' : ''}</span>
                <span class="group-badges">${langBadge}${depBadge}</span>
                <span class="group-actions">
                    ${hasImportGraph(group.filePath) ? `<a class="explore-graph-link"
                        href="${escapeHtml(graphExplorerUrl(group.filePath))}"
                        aria-label="Explore this file's imports"
                        title="Explore imports in the dependency explorer">${iconSvg('account_tree', 16)}</a>` : ''}
                    <button type="button" class="copy-path-btn"
                        data-file-path="${escapeHtml(group.filePath)}"
                        aria-label="Copy file path"
                        title="Copy file path">${iconSvg('content_copy', 16)}</button>
                </span>
            </div>
            ${hitsHtml}
        </div>
    `;
}

/**
 * Post-process freshly built group elements under `root`: syntax and match
 * highlighting, view/copy/dependency handlers. Runs on a detached holder
 * before the groups are inserted, so the page lays out only once.
 */
function wireResultGroups(root) {
    // Syntax-highlight each result, then mark the hit: the match line from
    // the server's byte offsets, every line from the query terms.
    root.querySelectorAll('pre.result-code').forEach(pre => {
        // One pass over the whole window: per-line highlighting coloured
        // the prose of multi-line comments and strings as code.
        const lang = pre.dataset.lang || 'plaintext';
        const spans = [...pre.querySelectorAll('.ctx-line-content')];
        const texts = spans.map(span => span.textContent);
        const htmlLines = highlightSnippetLines(texts, lang);
        spans.forEach((span, i) => {
            const text = texts[i];
            span.innerHTML = htmlLines[i];
            const extra = span.dataset.lms !== undefined
                ? [byteRangeToCharRange(text, Number(span.dataset.lms), Number(span.dataset.lme))]
                : [];
            highlightTermsIn(span, _currentMatcher, extra);
        });
    });

    // The file name opens the file at its first hit; a line number at that line.
    root.querySelectorAll('.group-open, .hit-num-open').forEach(btn => {
        btn.addEventListener('click', () => showFileModal(btn.dataset.filePath, parseInt(btn.dataset.lineNumber, 10) || 1));
    });

    // Attach View button click handler and context tooltip
    root.querySelectorAll('.view-file-btn').forEach(btn => {
        const filePath = btn.dataset.filePath;
        const lineNumber = parseInt(btn.dataset.lineNumber, 10) || 1;
        const wholeFile = btn.dataset.wholeFile === 'true';
        // Click pins the preview open (the document click handler has
        // already closed any other one); its Full view button opens the viewer.
        btn.addEventListener('click', () => {
            clearTimeout(_ctxHoverTimer);
            if (isContextTooltipVisible() && _ctxAnchor === btn) { pinContextTooltip(); return; }
            _ctxPinned = true;
            showContextTooltip(btn, filePath, lineNumber, wholeFile);
        });
        // Hover-intent delay: only fetch a preview if the cursor lingers ~200ms,
        // so sweeping down a results list doesn't fire a burst of requests.
        // A pinned preview is not replaced by hovering another icon.
        btn.addEventListener('mouseenter', () => {
            if (_ctxPinned && isContextTooltipVisible()) return;
            clearTimeout(_ctxHoverTimer);
            _ctxHoverTimer = setTimeout(
                () => showContextTooltip(btn, filePath, lineNumber, wholeFile),
                200
            );
        });
        btn.addEventListener('mouseleave', () => {
            clearTimeout(_ctxHoverTimer);
            hideContextTooltip();
        });
    });

    // Copy-path buttons: copy the file path to the clipboard with brief feedback.
    root.querySelectorAll('.copy-path-btn').forEach(btn => {
        btn.addEventListener('click', async () => {
            const ok = await copyTextToClipboard(btn.dataset.filePath || '');
            setCopyButtonState(btn, ok ? 'copied' : 'failed');
        });
    });

    // Attach dependency-badge handlers via dataset (no inline JS handlers, so
    // file paths containing quotes can never inject code).
    root.querySelectorAll('.deps-badge').forEach(badge => {
        const filePath = badge.dataset.filePath;
        badge.addEventListener('mouseenter', () => showDepsTooltip(badge, filePath));
        badge.addEventListener('mouseleave', hideDepsTooltip);
        badge.addEventListener('click', () => {
            hideDepsTooltipImmediately();
            showDependents(filePath);
        });
    });

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

/**
 * Reflect `_selectedGroupIndex` in the DOM: outline, aria-selected and a
 * roving tabindex (the selected group, or the first, is the one Tab reaches),
 * and move real focus to it so screen readers follow j/k.
 * @param {HTMLElement[]} groups
 * @param {{focus?: boolean}} [opts] - focus:false when the group already has focus
 */
function highlightSelectedGroup(groups, opts = {}) {
    groups.forEach((g, i) => {
        const selected = i === _selectedGroupIndex;
        g.setAttribute('aria-selected', selected ? 'true' : 'false');
        g.setAttribute('tabindex', selected || (_selectedGroupIndex < 0 && i === 0) ? '0' : '-1');
        if (selected) {
            g.style.outline = '3px solid var(--accent-ink)';
            g.style.outlineOffset = '2px';
            if (opts.focus !== false) g.focus({ preventScroll: true });
            g.scrollIntoView({ block: 'nearest' });
        } else {
            g.style.outline = '';
            g.style.outlineOffset = '';
        }
    });
}

function moveResultSelection(delta) {
    const total = totalResultGroups();
    if (!total) return;
    const next = _selectedGroupIndex < 0
        ? 0
        : Math.max(0, Math.min(total - 1, _selectedGroupIndex + delta));
    // A group past the rendered window is rendered before it is selected.
    ensureGroupsRendered(next + 1);
    _selectedGroupIndex = next;
    highlightSelectedGroup(getResultGroups());
}

function openSelectedGroup() {
    const g = getResultGroups()[_selectedGroupIndex];
    if (!g) return;
    showFileModal(g.dataset.filePath, parseInt(g.dataset.lineNumber, 10) || 1);
}

// A group that receives focus by mouse or Tab becomes the selection too, so
// j/k continue from it (no focus() call here: it already has focus).
resultsContainer.addEventListener('focusin', (e) => {
    const group = e.target.closest && e.target.closest('.result-group');
    if (!group || e.target !== group) return;
    const groups = getResultGroups();
    const idx = groups.indexOf(group);
    if (idx >= 0 && idx !== _selectedGroupIndex) {
        _selectedGroupIndex = idx;
        highlightSelectedGroup(groups, { focus: false });
    }
});

// Global shortcuts: '/' focuses search; j/k or arrows move the result selection;
// Enter opens the selected file. Ignored while typing in a field or with a modal open.
document.addEventListener('keydown', (e) => {
    const active = document.activeElement;
    const tag = (active && active.tagName) || '';
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
        // A focused button or link handles Enter itself.
        if (tag === 'BUTTON' || tag === 'A') return;
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
// CLIPBOARD
// ============================================

/**
 * Copy `text` to the clipboard. The async Clipboard API only exists on
 * secure origins (https or localhost); a server reached over plain http on
 * another host falls back to a temporary textarea and execCommand('copy').
 * @returns {Promise<boolean>} whether the copy succeeded
 */
async function copyTextToClipboard(text) {
    if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (e) {
            console.warn('Clipboard API failed, trying execCommand:', e);
        }
    }
    try {
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.setAttribute('readonly', '');
        ta.setAttribute('aria-hidden', 'true');
        ta.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;pointer-events:none';
        document.body.appendChild(ta);
        ta.focus({ preventScroll: true });
        ta.select();
        ta.setSelectionRange(0, ta.value.length);
        const ok = typeof document.execCommand === 'function' && document.execCommand('copy');
        ta.remove();
        return !!ok;
    } catch (e) {
        console.error('Copy failed:', e);
        return false;
    }
}

const COPY_STATE_MS = 1500;

/**
 * Show a brief "Copied" / "Copy failed" state on a copy button (icon,
 * colour, title and accessible name), then restore its idle look.
 * @param {HTMLButtonElement} btn
 * @param {'copied'|'failed'} state
 */
function setCopyButtonState(btn, state) {
    if (!btn.dataset.idleLabel) {
        btn.dataset.idleLabel = btn.getAttribute('aria-label') || 'Copy file path';
        btn.dataset.idleTitle = btn.title || btn.dataset.idleLabel;
        btn._idleIconHtml = btn.innerHTML;
    }
    clearTimeout(btn._copyStateTimer);
    const copied = state === 'copied';
    const label = copied ? 'Copied' : 'Copy failed';
    btn.classList.remove('copy-ok', 'copy-failed');
    btn.classList.add(copied ? 'copy-ok' : 'copy-failed');
    btn.setAttribute('aria-label', label);
    btn.title = label;
    btn.innerHTML = iconSvg(copied ? 'check' : 'error', 16);
    btn._copyStateTimer = setTimeout(() => {
        btn.classList.remove('copy-ok', 'copy-failed');
        btn.setAttribute('aria-label', btn.dataset.idleLabel);
        btn.title = btn.dataset.idleTitle;
        btn.innerHTML = btn._idleIconHtml;
    }, COPY_STATE_MS);
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
                `<ul>${items || '<li style="color:var(--text-3);font-style:italic">none</li>'}</ul>` +
                more +
                `</div>`;
        }

        const dependentsSection = buildSection(depData.files || [], 'Imported by', 'dependents');
        const importsSection = buildSection(imptData.files || [], 'Imports', 'dependencies');

        popover.innerHTML =
            `<div class="deps-popover-header">${escapeHtml(basename)}</div>` +
            dependentsSection +
            importsSection +
            `<a class="deps-popover-explore" href="${escapeHtml(graphExplorerUrl(filePath))}">Open in dependency explorer →</a>`;

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
    await showDependencyList('Dependents', filePath, 'dependents', 'Files that import this file:');
}

async function showDependencies(filePath) {
    hideDepsTooltipImmediately();
    await showDependencyList('Dependencies', filePath, 'dependencies', 'Files imported by this file:');
}

/**
 * Open the dependency dialog for `filePath`, fetching `/api/<endpoint>`.
 * A failed fetch is reported inside the dialog with the page's error
 * styling rather than a blocking alert().
 */
async function showDependencyList(title, filePath, endpoint, description) {
    try {
        const response = await fetch(`${API_BASE}/api/${endpoint}?file=${encodeURIComponent(filePath)}`);
        if (!response.ok) throw new Error(await readErrorBody(response));
        const data = await response.json();
        showDependencyModal(title, filePath, data.files || [], description);
    } catch (error) {
        console.error(`Error fetching ${endpoint}:`, error);
        showDependencyModal(title, filePath, [], description, {
            error: `Failed to load ${endpoint}: ${error.message}`,
        });
    }
}

// ============================================
// DIALOG HELPERS (shared by the dependency and file modals)
// ============================================

// Elements that had focus when each open dialog was opened, innermost last.
const _dialogFocusStack = [];

const FOCUSABLE_SELECTOR =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), ' +
    'textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Keep Tab / Shift+Tab cycling inside the dialog that received the event. */
function trapDialogTab(e) {
    if (e.key !== 'Tab') return;
    const overlay = e.currentTarget;
    const focusables = Array.from(overlay.querySelectorAll(FOCUSABLE_SELECTOR))
        .filter(el => el.offsetParent !== null);
    if (!focusables.length) { e.preventDefault(); return; }
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement;
    if (e.shiftKey) {
        if (active === first || !overlay.contains(active)) { e.preventDefault(); last.focus(); }
    } else if (active === last || !overlay.contains(active)) {
        e.preventDefault();
        first.focus();
    }
}

/**
 * Give an overlay/dialog pair dialog semantics: role, aria-modal, an
 * accessible name, a Tab focus trap, and initial focus. Focus is restored
 * to the opener by releaseDialog().
 */
function armDialog(overlay, dialog, label, initialFocus) {
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    dialog.setAttribute('aria-label', label);
    if (!dialog.hasAttribute('tabindex')) dialog.setAttribute('tabindex', '-1');
    overlay.addEventListener('keydown', trapDialogTab);
    _dialogFocusStack.push(document.activeElement);
    const target = initialFocus || dialog;
    // The overlay is appended synchronously before this runs; focus directly.
    if (typeof target.focus === 'function') target.focus({ preventScroll: true });
}

/** Undo armDialog: drop the trap and return focus to the element that opened it. */
function releaseDialog(overlay) {
    if (overlay) overlay.removeEventListener('keydown', trapDialogTab);
    const opener = _dialogFocusStack.pop();
    if (opener && typeof opener.focus === 'function' && document.contains(opener)) {
        opener.focus({ preventScroll: true });
    }
}

/**
 * @param {{error?: string}} [opts] - `error` replaces the list with an inline
 *   error message (the request for the list failed).
 */
function showDependencyModal(title, filePath, files, description, opts = {}) {
    const existingModal = document.getElementById('dep-modal');
    if (existingModal) closeModal();

    const fileList = opts.error
        ? `<li><div class="error-message" role="alert"><strong>Error:</strong> ${escapeHtml(opts.error)}</div></li>`
        : files.length > 0
            ? files.map(f => `<li style="padding:0.25rem 0;font-family:monospace;font-size:0.85rem;color:var(--text)">${escapeHtml(f)}</li>`).join('')
            : '<li style="color:var(--text-3);">No files found</li>';
    const countLabel = opts.error ? '' : ` (${files.length})`;

    const modal = document.createElement('div');
    modal.id = 'dep-modal';
    modal.style.cssText = 'position:fixed;inset:0;background:var(--overlay);display:flex;align-items:center;justify-content:center;z-index:1000;';
    modal.innerHTML = `
        <div class="dep-modal-dialog" style="background:var(--surface-2);border:1px solid var(--border);box-shadow:var(--shadow-lg);padding:1.5rem;max-width:600px;width:90%;max-height:80vh;overflow:auto;">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;">
                <h2 id="dep-modal-title" style="font-size:1.1rem;font-family:var(--font-mono);color:var(--text)">${escapeHtml(title)}${countLabel}</h2>
                <button type="button" class="dep-modal-close" aria-label="Close" title="Close (Esc)" style="background:none;border:1px solid var(--line-strong);width:1.75rem;height:1.75rem;font-size:1.1rem;cursor:pointer;color:var(--text);display:flex;align-items:center;justify-content:center;">&times;</button>
            </div>
            <p style="font-family:monospace;font-size:0.85rem;color:var(--link);margin-bottom:0.5rem;word-break:break-all">${escapeHtml(filePath)}</p>
            <p style="color:var(--text-2);font-size:0.85rem;margin-bottom:0.75rem;">${description}</p>
            <ul style="list-style:none;padding:0;">${fileList}</ul>
            <a class="deps-popover-explore" style="padding-left:0" href="${escapeHtml(graphExplorerUrl(filePath))}">Open in dependency explorer →</a>
        </div>
    `;

    modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
    const closeBtn = modal.querySelector('.dep-modal-close');
    if (closeBtn) closeBtn.addEventListener('click', closeModal);
    document.addEventListener('keydown', handleModalEscape);
    document.body.appendChild(modal);
    armDialog(modal, modal.querySelector('.dep-modal-dialog'), `${title} of ${filePath}`, closeBtn);
}

function handleModalEscape(e) {
    if (e.key === 'Escape') closeModal();
}

function closeModal() {
    const modal = document.getElementById('dep-modal');
    if (modal) {
        releaseDialog(modal);
        modal.remove();
    }
    document.removeEventListener('keydown', handleModalEscape);
}

// ============================================
// FILE VIEWER MODAL
// ============================================


// Stored so the overlay click listener can be removed on close
let _fileModalOverlayListener = null;

/**
 * Open the file viewer on `filePath` at `highlightLine`.
 * @param {{keepHistory?: boolean}} [opts] - keepHistory: this is a step of
 *   go-to-definition or Back inside the viewer, so keep the Back trail
 *   (a fresh open starts a new one).
 */
async function showFileModal(filePath, highlightLine, opts = {}) {
    // Ensure hover preview is not left visible behind the modal.
    hideContextTooltipImmediately();
    hideCodeMenu();
    if (!opts.keepHistory) _fileViewBack = [];
    _fileView = { file: filePath, line: highlightLine || 1 };

    // Clean up any existing file modal and its listeners
    if (document.getElementById('file-modal')) closeFileModal();

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
    closeBtn.type = 'button';
    closeBtn.className = 'file-modal-close';
    closeBtn.title = 'Close (Esc)';
    closeBtn.setAttribute('aria-label', 'Close');
    closeBtn.textContent = '×';
    closeBtn.addEventListener('click', closeFileModal);

    // Copies the page URL, which carries the search and (in its fragment)
    // this file and line, so the link reopens the viewer where it is now.
    const copyLink = document.createElement('button');
    copyLink.type = 'button';
    copyLink.className = 'file-modal-explore';
    copyLink.textContent = 'Copy link';
    copyLink.title = 'Copy a link that opens this file at the selected line';
    copyLink.addEventListener('click', async () => {
        const ok = await copyTextToClipboard(location.href);
        copyLink.textContent = ok ? 'Copied' : 'Copy failed';
        setTimeout(() => { copyLink.textContent = 'Copy link'; }, 1500);
    });

    if (_fileViewBack.length) {
        const back = document.createElement('button');
        back.type = 'button';
        back.className = 'file-modal-explore file-modal-back';
        const prev = _fileViewBack[_fileViewBack.length - 1];
        back.textContent = '← Back';
        back.title = `Back to ${prev.file}:${prev.line} (Alt+←)`;
        back.addEventListener('click', fileViewGoBack);
        header.appendChild(back);
    }
    header.appendChild(pathSpan);
    header.appendChild(copyLink);
    if (hasImportGraph(filePath)) {
        const explore = document.createElement('a');
        explore.className = 'file-modal-explore';
        explore.href = graphExplorerUrl(filePath);
        explore.textContent = 'Explore imports';
        explore.title = 'Open this file in the dependency explorer';
        header.appendChild(explore);
    }
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
    armDialog(modal, dialog, `File ${filePath}`, closeBtn);
    setFileViewHash(filePath, highlightLine || 1);

    // Clicking a line number selects that line, as on GitHub, and puts it in
    // the URL so Copy link (or the address bar) shares it.
    let view = null;
    body.addEventListener('click', (e) => {
        const num = e.target.closest('.file-line-num');
        if (!num || !view) return;
        const line = parseInt(num.textContent, 10);
        if (!(line > 0)) return;
        view.setMark(line);
        _fileView = { file: filePath, line };
        setFileViewHash(filePath, line);
    });

    try {
        view = await populateFileView(body, filePath, highlightLine, _currentMatcher || currentQueryMatcher());
        // The scroll area takes focus so the arrow keys scroll the file.
        if (view) view.scroller.setAttribute('aria-label', `Contents of ${filePath}`);
    } catch (error) {
        body.innerHTML = `<div class="error-message"><strong>Error:</strong> ${escapeHtml(error.message)}</div>`;
    }
}

function handleFileModalEscape(e) {
    if (e.key === 'Escape') closeFileModal();
    else if (e.key === 'ArrowLeft' && e.altKey && _fileViewBack.length) {
        e.preventDefault();
        fileViewGoBack();
    }
}

function closeFileModal() {
    const modal = document.getElementById('file-modal');
    if (modal) {
        if (_fileModalOverlayListener) {
            modal.removeEventListener('click', _fileModalOverlayListener);
            _fileModalOverlayListener = null;
        }
        releaseDialog(modal);
        modal.remove();
    }
    document.removeEventListener('keydown', handleFileModalEscape);
    if (parseFileViewHash(location.hash)) {
        history.replaceState(history.state, '', location.pathname + location.search);
    }
}

/** Record the open file and line in the URL fragment, without a history entry. */
function setFileViewHash(filePath, line) {
    history.replaceState(history.state, '', location.pathname + location.search + fileViewHash(filePath, line));
}

/** Open the file viewer named by the URL fragment (a shared link), if any. */
function openFileViewFromHash() {
    const view = parseFileViewHash(location.hash);
    if (view) showFileModal(view.file, view.line);
}

window.addEventListener('hashchange', openFileViewFromHash);

// ============================================
// GO TO DEFINITION
//
// In the file viewer and in result snippets, Ctrl+click (Cmd+click on a
// Mac) on a name opens its definition; holding the key underlines the name
// under the pointer. Double-clicking a name opens a small menu: Definition,
// References, Search. Definitions come from the symbol search (exact name,
// whole word, case-sensitive): one opens directly, several open a list, none
// falls back to a text search. Inside the viewer each jump leaves a Back
// step (button or Alt+←).
// ============================================

const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || '');
const DEFN_KEY_LABEL = IS_MAC ? '⌘' : 'Ctrl';
/** The file and line the viewer shows, and the trail Back walks. */
let _fileView = null;
let _fileViewBack = [];

function isDefnModifier(e) {
    return IS_MAC ? e.metaKey : e.ctrlKey;
}

/** The code line element (one line of text) that `el` sits in, or null. */
function codeLineElement(el) {
    if (!el || !el.closest) return null;
    return el.closest('.file-modal-body .file-line-content, .ctx-tooltip .file-line-content, .ctx-line-content');
}

/** A DOM Range over the [start, end) text offsets of `el`. */
function rangeForOffsets(el, start, end) {
    const range = document.createRange();
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    let pos = 0;
    let node;
    let started = false;
    while ((node = walker.nextNode())) {
        const len = node.nodeValue.length;
        if (!started && start <= pos + len) { range.setStart(node, start - pos); started = true; }
        if (started && end <= pos + len) { range.setEnd(node, end - pos); return range; }
        pos += len;
    }
    return null;
}

/** The name under the viewport point (x, y) in a code line, or null. */
function identifierAtPoint(x, y) {
    let node = null;
    let offset = 0;
    if (document.caretPositionFromPoint) {
        const p = document.caretPositionFromPoint(x, y);
        if (p) { node = p.offsetNode; offset = p.offset; }
    } else if (document.caretRangeFromPoint) {
        const r = document.caretRangeFromPoint(x, y);
        if (r) { node = r.startContainer; offset = r.startOffset; }
    }
    if (!node || node.nodeType !== Node.TEXT_NODE) return null;
    const lineEl = codeLineElement(node.parentElement);
    if (!lineEl) return null;

    const walker = document.createTreeWalker(lineEl, NodeFilter.SHOW_TEXT);
    let pos = 0;
    let n;
    while ((n = walker.nextNode()) && n !== node) pos += n.nodeValue.length;
    const id = identifierAt(lineEl.textContent, pos + offset);
    if (!id) return null;
    const range = rangeForOffsets(lineEl, id.start, id.end);
    // The caret snaps to the nearest character, so make sure the pointer is
    // really over the name (not in the blank space after a short line).
    const over = range && [...range.getClientRects()].some(r =>
        x >= r.left - 1 && x <= r.right + 1 && y >= r.top - 1 && y <= r.bottom + 1);
    return over ? { ...id, range, lineEl } : null;
}

/** The file a code line belongs to: the viewer's file or the result's. */
function fileOfCodeLine(lineEl) {
    if (lineEl.closest('#file-modal')) return _fileView ? _fileView.file : null;
    const preview = lineEl.closest('.ctx-tooltip');
    if (preview) return preview._file || null;
    const group = lineEl.closest('.result-group');
    return group ? group.dataset.filePath : null;
}

function setDefnHover(id) {
    document.body.classList.toggle('defn-hover', !!id);
    if (typeof CSS === 'undefined' || !CSS.highlights || typeof Highlight === 'undefined') return;
    if (id) CSS.highlights.set('fcs-defn', new Highlight(id.range));
    else CSS.highlights.delete('fcs-defn');
}

document.addEventListener('mousemove', (e) => {
    setDefnHover(isDefnModifier(e) ? identifierAtPoint(e.clientX, e.clientY) : null);
});
document.addEventListener('keyup', (e) => {
    if (e.key === 'Control' || e.key === 'Meta') setDefnHover(null);
});
window.addEventListener('blur', () => setDefnHover(null));

// Capture phase, so the click never reaches the result row or line number.
document.addEventListener('click', (e) => {
    if (!isDefnModifier(e) || e.button !== 0) return;
    const id = identifierAtPoint(e.clientX, e.clientY);
    if (!id) return;
    e.preventDefault();
    e.stopPropagation();
    setDefnHover(null);
    goToDefinition(id.word, fileOfCodeLine(id.lineEl), id.range.getBoundingClientRect());
}, true);

document.addEventListener('dblclick', (e) => {
    const id = identifierAtPoint(e.clientX, e.clientY);
    if (!id) return;
    showCodeMenu(id.range.getBoundingClientRect(), menuForName(id.word, fileOfCodeLine(id.lineEl)));
});

let _codeMenu = null;

/**
 * Show a small floating menu under `rect`. `content` is {title?, items:
 * [{label, detail?, action}]} where each action runs after the menu closes.
 */
function showCodeMenu(rect, content) {
    hideCodeMenu();
    const menu = document.createElement('div');
    menu.className = 'code-menu';
    menu.setAttribute('role', 'menu');
    if (content.title) {
        const title = document.createElement('div');
        title.className = 'code-menu-title';
        title.textContent = content.title;
        menu.appendChild(title);
    }
    content.items.forEach(item => {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.setAttribute('role', 'menuitem');
        const label = document.createElement('span');
        label.textContent = item.label;
        btn.appendChild(label);
        if (item.detail) {
            const detail = document.createElement('code');
            detail.textContent = item.detail;
            btn.appendChild(detail);
        }
        btn.addEventListener('click', () => { hideCodeMenu(); item.action(); });
        menu.appendChild(btn);
    });
    document.body.appendChild(menu);
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - w - 8));
    const below = rect.bottom + 4;
    const top = below + h > window.innerHeight - 8 ? Math.max(8, rect.top - h - 4) : below;
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;
    _codeMenu = menu;
    menu.querySelector('button')?.focus({ preventScroll: true });
}

function hideCodeMenu() {
    if (_codeMenu) { _codeMenu.remove(); _codeMenu = null; }
}

document.addEventListener('mousedown', (e) => {
    if (_codeMenu && !_codeMenu.contains(e.target)) hideCodeMenu();
}, true);
// Capture phase: Escape closes the menu before it can close the viewer.
document.addEventListener('keydown', (e) => {
    if (!_codeMenu) return;
    if (e.key === 'Escape') {
        e.preventDefault();
        e.stopImmediatePropagation();
        hideCodeMenu();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        e.stopImmediatePropagation();
        const items = [..._codeMenu.querySelectorAll('button')];
        const i = items.indexOf(document.activeElement);
        const next = e.key === 'ArrowDown' ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
        items[next].focus();
    }
}, true);
window.addEventListener('resize', hideCodeMenu);
document.addEventListener('scroll', hideCodeMenu, true);

function menuForName(name, fromFile) {
    return {
        title: name,
        items: [
            { label: 'Definition', detail: `${DEFN_KEY_LABEL}+click`, action: () => goToDefinition(name, fromFile, null) },
            { label: 'References', action: () => searchForName(name, 'references') },
            { label: 'Search', action: () => searchForName(name, 'text') },
        ],
    };
}

/** Run a search for `name` in the main page: its references, or as text. */
function searchForName(name, mode) {
    closeFileModal();
    hideContextTooltipImmediately();
    queryInput.value = mode === 'text' ? definitionQuery(name).replace('case:yes ', '') : name;
    modeCheckboxes().forEach(cb => { cb.checked = mode === 'references' && cb === referencesModeCheckbox; });
    syncToggleVisuals();
    submitSearch();
}

let _defnLookup = null;

/**
 * Find where `name` is defined and open it. `fromFile` ranks definitions in
 * the same file first; `rect` (optional) is where to anchor the list when
 * there are several.
 */
async function goToDefinition(name, fromFile, rect) {
    if (_defnLookup) _defnLookup.abort();
    const controller = new AbortController();
    _defnLookup = controller;
    document.body.classList.add('defn-busy');
    try {
        const params = new URLSearchParams({ q: definitionQuery(name), symbols: 'true', max: '50' });
        const resp = await fetch(`${API_BASE}/api/search?${params}`, { signal: controller.signal });
        if (!resp.ok) throw new Error(await readErrorBody(resp));
        const data = await resp.json();
        const defs = rankDefinitions(data.results, name, fromFile);
        if (defs.length === 0) {
            searchForName(name, 'text');
        } else if (defs.length === 1) {
            openDefinition(defs[0]);
        } else {
            const anchor = rect || (document.querySelector('#file-modal .file-modal-header') || document.body).getBoundingClientRect();
            showCodeMenu(anchor, {
                title: `${defs.length} definitions of ${name}`,
                items: defs.slice(0, 20).map(d => ({
                    label: `${d.file}:${d.line}`,
                    detail: d.content.length > 80 ? d.content.slice(0, 79) + '…' : d.content,
                    action: () => openDefinition(d),
                })),
            });
        }
    } catch (err) {
        if (err.name !== 'AbortError') searchForName(name, 'text');
    } finally {
        if (_defnLookup === controller) {
            _defnLookup = null;
            document.body.classList.remove('defn-busy');
        }
    }
}

/**
 * Open a definition: a Back step inside the viewer, or a fresh viewer. From
 * the preview, the viewer opens with Back leading to where the preview was.
 */
function openDefinition(def) {
    const preview = _ctxTooltip && _ctxTooltip.style.display !== 'none' ? _ctxTooltip : null;
    if (!document.getElementById('file-modal') && preview && preview._file) {
        const from = { file: preview._file, line: preview._line || 1 };
        _fileViewBack = [from];
        showFileModal(def.file, def.line, { keepHistory: true });
    } else if (document.getElementById('file-modal') && _fileView) {
        _fileViewBack.push(_fileView);
        showFileModal(def.file, def.line, { keepHistory: true });
    } else {
        showFileModal(def.file, def.line);
    }
}

function fileViewGoBack() {
    const prev = _fileViewBack.pop();
    if (prev) showFileModal(prev.file, prev.line, { keepHistory: true });
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

if (caseModeCheckbox) {
    caseModeCheckbox.addEventListener('change', () => {
        syncToggleVisuals();
        optionChanged();
    });
}

// The mode toggles are mutually exclusive: turning one on turns the others off.
modeCheckboxes().forEach(cb => {
    cb.addEventListener('change', () => {
        if (cb.checked) modeCheckboxes().forEach(other => { if (other !== cb) other.checked = false; });
        syncToggleVisuals();
        optionChanged();
    });
});
if (rankModeSelect) rankModeSelect.addEventListener('change', optionChanged);
if (contextLinesSelect) contextLinesSelect.addEventListener('change', () => { _contextChosen = true; optionChanged(); });

document.getElementById('active-filters')?.addEventListener('click', (e) => {
    const btn = e.target.closest('button[data-clear]');
    if (btn) clearPathFilters([document.getElementById(btn.dataset.clear)]);
});
document.getElementById('clear-filters-btn')?.addEventListener('click', () => {
    clearPathFilters([includeFilterInput, excludeFilterInput]);
});

if (includeFilterInput) {
    includeFilterInput.addEventListener('input', syncFilterIndicators);
    includeFilterInput.addEventListener('input', debouncedSearch);
    includeFilterInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') optionChanged(); });
}
if (excludeFilterInput) {
    excludeFilterInput.addEventListener('input', syncFilterIndicators);
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
    openFileViewFromHash();
});
