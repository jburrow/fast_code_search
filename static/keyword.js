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
let searchTimeout = null;
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
        };
        localStorage.setItem(LS_SETTINGS_KEY, JSON.stringify(settings));
    } catch (_) { /* storage unavailable */ }
}

/**
 * Load settings from localStorage and apply to form fields.
 * URL params take precedence over stored settings.
 */
function loadSettingsFromStorage() {
    try {
        const raw = localStorage.getItem(LS_SETTINGS_KEY);
        if (!raw) return;
        const s = JSON.parse(raw);
        if (maxResultsSelect && s.max) maxResultsSelect.value = s.max;
        if (rankModeSelect && s.rank) rankModeSelect.value = s.rank;
        if (contextLinesSelect && s.context != null) contextLinesSelect.value = String(s.context);
        if (includeFilterInput && s.include) includeFilterInput.value = s.include;
        if (excludeFilterInput && s.exclude) excludeFilterInput.value = s.exclude;
        if (regexModeCheckbox && s.regex) regexModeCheckbox.checked = true;
        if (symbolsModeCheckbox && s.symbols) symbolsModeCheckbox.checked = true;
    } catch (_) { /* ignore parse errors */ }
}

// ============================================
// SEARCH HISTORY
// ============================================

/**
 * Load search history array from localStorage.
 * Returns an array of unique query strings (most-recent first).
 */
function loadHistory() {
    try {
        const raw = localStorage.getItem(LS_HISTORY_KEY);
        return raw ? JSON.parse(raw) : [];
    } catch (_) {
        return [];
    }
}

/**
 * Persist a successful query to search history.
 */
function saveToHistory(query) {
    if (!query || query.length < 2) return;
    try {
        let history = loadHistory();
        // Remove duplicate, then prepend
        history = history.filter(q => q !== query);
        history.unshift(query);
        if (history.length > MAX_HISTORY) history = history.slice(0, MAX_HISTORY);
        localStorage.setItem(LS_HISTORY_KEY, JSON.stringify(history));
    } catch (_) { /* storage unavailable */ }
}

/**
 * Clear all saved search history.
 */
function clearHistory() {
    try { localStorage.removeItem(LS_HISTORY_KEY); } catch (_) { /* ignore */ }
    hideHistoryDropdown();
}

let _historyDropdownFocusedIdx = -1;

function showHistoryDropdown(filter) {
    if (!searchHistoryDropdown) return;
    const all = loadHistory();
    const matches = filter
        ? all.filter(q => q.toLowerCase().includes(filter.toLowerCase()))
        : all;

    if (matches.length === 0) {
        hideHistoryDropdown();
        return;
    }

    _historyDropdownFocusedIdx = -1;
    searchHistoryDropdown.innerHTML = matches.slice(0, 10).map((q, i) =>
        `<div class="history-item flex items-center gap-2 px-4 py-2 cursor-pointer hover:bg-primary-container font-label text-xs text-on-surface" data-idx="${i}" data-query="${escapeHtml(q)}">
            <span class="material-symbols-outlined" style="font-size:14px;color:#7a785f;flex-shrink:0">history</span>
            <span class="flex-1 truncate">${escapeHtml(q)}</span>
            <button class="history-delete material-symbols-outlined ml-auto flex-shrink-0" style="font-size:14px;color:#7a785f;background:none;border:none;cursor:pointer;padding:0" data-query="${escapeHtml(q)}" title="Remove">close</button>
        </div>`
    ).join('') +
    `<div class="flex items-center justify-end px-4 py-1.5 border-t border-outline-variant">
        <button id="clear-history-btn" class="font-label text-[10px] text-outline hover:text-black transition-colors">CLEAR ALL HISTORY</button>
    </div>`;

    searchHistoryDropdown.style.display = 'block';

    // Attach click handlers
    searchHistoryDropdown.querySelectorAll('.history-item').forEach(item => {
        item.addEventListener('mousedown', (e) => {
            // Ignore clicks on the delete button itself
            if (e.target.classList.contains('history-delete')) return;
            e.preventDefault();
            queryInput.value = item.dataset.query;
            hideHistoryDropdown();
            performSearch();
        });
    });

    searchHistoryDropdown.querySelectorAll('.history-delete').forEach(btn => {
        btn.addEventListener('mousedown', (e) => {
            e.preventDefault();
            e.stopPropagation();
            const q = btn.dataset.query;
            try {
                let h = loadHistory().filter(x => x !== q);
                localStorage.setItem(LS_HISTORY_KEY, JSON.stringify(h));
            } catch (_) { /* ignore */ }
            showHistoryDropdown(queryInput.value.trim());
        });
    });

    const clearBtn = document.getElementById('clear-history-btn');
    if (clearBtn) clearBtn.addEventListener('mousedown', (e) => { e.preventDefault(); clearHistory(); });
}

function hideHistoryDropdown() {
    if (searchHistoryDropdown) searchHistoryDropdown.style.display = 'none';
    _historyDropdownFocusedIdx = -1;
}

function navigateHistoryDropdown(dir) {
    if (!searchHistoryDropdown || searchHistoryDropdown.style.display === 'none') return false;
    const items = Array.from(searchHistoryDropdown.querySelectorAll('.history-item'));
    if (items.length === 0) return false;
    _historyDropdownFocusedIdx = Math.max(-1, Math.min(items.length - 1, _historyDropdownFocusedIdx + dir));
    items.forEach((el, i) => el.classList.toggle('bg-primary-container', i === _historyDropdownFocusedIdx));
    if (_historyDropdownFocusedIdx >= 0) {
        queryInput.value = items[_historyDropdownFocusedIdx].dataset.query;
    }
    return true;
}

// ============================================
// URL STATE
// ============================================

/**
 * Populate form fields from URL query parameters.
 * Auto-opens Advanced Options if any non-default option is present.
 */
function loadStateFromUrl() {
    const params = new URLSearchParams(location.search);

    if (params.has('q')) queryInput.value = params.get('q');
    if (params.has('max') && maxResultsSelect) maxResultsSelect.value = params.get('max');
    if (includeFilterInput && params.has('include')) includeFilterInput.value = params.get('include');
    if (excludeFilterInput && params.has('exclude')) excludeFilterInput.value = params.get('exclude');
    if (regexModeCheckbox && params.get('regex') === 'true') regexModeCheckbox.checked = true;
    if (symbolsModeCheckbox && params.get('symbols') === 'true') symbolsModeCheckbox.checked = true;
    if (rankModeSelect && params.has('rank')) rankModeSelect.value = params.get('rank');
    if (contextLinesSelect && params.has('context')) contextLinesSelect.value = params.get('context');

    // Auto-expand Options if any non-default values were loaded (regex/symbols are now always visible)
    const hasAdvanced = params.has('include') || params.has('exclude') ||
        params.has('rank') || params.has('max') || params.has('context');
    if (hasAdvanced) {
        const details = document.querySelector('.advanced-options');
        if (details) details.open = true;
    }
}

/**
 * Write current form state into the URL (replaces history entry, no navigation).
 * Omits default values to keep URLs short.
 */
function syncUrlFromState() {
    const query = queryInput.value.trim();
    const params = new URLSearchParams();

    if (query) params.set('q', query);

    const max = maxResultsSelect?.value;
    if (max && max !== '50') params.set('max', max);

    const include = includeFilterInput?.value.trim() || '';
    if (include) params.set('include', include);

    const exclude = excludeFilterInput?.value.trim() || '';
    if (exclude) params.set('exclude', exclude);

    if (regexModeCheckbox?.checked) params.set('regex', 'true');
    if (symbolsModeCheckbox?.checked) params.set('symbols', 'true');

    const rank = rankModeSelect?.value || 'auto';
    if (rank !== 'auto') params.set('rank', rank);

    const context = contextLinesSelect?.value || '0';
    if (context !== '0') params.set('context', context);

    const qs = params.toString();
    history.replaceState(null, '', qs ? `?${qs}` : location.pathname);
}

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    resultsContainerId: 'results',
    searchSectionId: 'search-section',
    additionalInputIds: ['include-filter', 'exclude-filter', 'max-results', 'regex-mode', 'symbols-mode', 'rank-mode', 'context-lines'],
    onReadyChange: (isReady, status) => {
        if (isReady && queryInput.value.trim()) {
            // If user typed while waiting, trigger search now
            performSearch();
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

    // Check semantic backend for the badge (non-blocking side-info)
    let semanticUp = false;
    try {
        const resp = await fetch(`http://${hostname}:8081/api/health`, { signal: AbortSignal.timeout(2000) });
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
        searchReadiness.setOffline(false);
    },
    onDisconnected: () => {},
    onServerOffline: () => {
        // Consecutive WS failures — the keyword search server is not running
        searchReadiness.setOffline(true);
    },
    onError: (err) => {
        console.error('Progress WebSocket error:', err);
        // Progress will continue via reconnection
    }
});

function updateProgressUI(status) {
    const isIdle = status.status === 'idle';
    const isCompleted = status.status === 'completed';
    
    // Update search readiness based on status
    searchReadiness.update(status);
    
    toggleElement('progress-panel', !isIdle, 'flex');
    
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

/**
 * Walk the DOM inside `el` and wrap occurrences of `query` text in
 * <mark class="highlight"> without breaking existing HTML structure.
 * Operates on text nodes only so it is safe after hljs has run.
 */
function applyQueryHighlight(el, query) {
    if (!query) return;
    const flags = 'gi';
    let re;
    try {
        re = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags);
    } catch (_) { return; }

    const walk = (node) => {
        if (node.nodeType === Node.TEXT_NODE) {
            const text = node.textContent;
            if (!re.test(text)) return;
            re.lastIndex = 0;
            const frag = document.createDocumentFragment();
            let last = 0, m;
            while ((m = re.exec(text)) !== null) {
                if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
                const mark = document.createElement('mark');
                mark.className = 'highlight';
                mark.textContent = m[0];
                frag.appendChild(mark);
                last = m.index + m[0].length;
            }
            if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
            node.parentNode.replaceChild(frag, node);
        } else if (node.nodeType === Node.ELEMENT_NODE && node.nodeName !== 'MARK') {
            // Clone children list as it may be mutated during traversal
            Array.from(node.childNodes).forEach(walk);
        }
    };
    walk(el);
    re.lastIndex = 0;
}

// ============================================
// FILE VIEW HELPER (shared by modal and tooltip)
// ============================================

/**
 * Fetch a file and render it into `container` with syntax highlighting,
 * query-term highlighting, and the matched line scrolled into view within
 * the container (works for both the full-screen modal and the fixed tooltip).
 */
async function populateFileView(container, filePath, highlightLine, query, signal) {
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

    if (query) {
        container.querySelectorAll('.file-line-content').forEach(span => applyQueryHighlight(span, query));
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

async function showContextTooltip(resultItem, filePath, lineNumber) {
    clearTimeout(_ctxHideTimer);
    if (_ctxFetchController) _ctxFetchController.abort();
    _ctxFetchController = new AbortController();

    const tooltip = getOrCreateTooltip();
    tooltip.innerHTML =
        `<div class="ctx-header">${escapeHtml(filePath)} : ${lineNumber}</div>` +
        `<div class="ctx-file-body"><div class="ctx-loading">Loading…</div></div>`;
    positionTooltip(tooltip, resultItem);
    tooltip.style.display = 'flex';

    const fileBody = tooltip.querySelector('.ctx-file-body');
    try {
        await populateFileView(fileBody, filePath, lineNumber, queryInput.value.trim(), _ctxFetchController.signal);
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

// URL state field descriptors for keyword search
const URL_FIELDS = [
    { param: 'q',       getter: () => queryInput.value.trim(),                      setter: (v) => { queryInput.value = v; },                                         defaultValue: '' },
    { param: 'max',     getter: () => maxResultsSelect.value,                        setter: (v) => { maxResultsSelect.value = v; },                                   defaultValue: '50' },
    { param: 'include', getter: () => includeFilterInput?.value.trim() || '',        setter: (v) => { if (includeFilterInput) includeFilterInput.value = v; },         defaultValue: '' },
    { param: 'exclude', getter: () => excludeFilterInput?.value.trim() || '',        setter: (v) => { if (excludeFilterInput) excludeFilterInput.value = v; },         defaultValue: '' },
    // Boolean fields use 'true' / '' (empty string) convention: empty string is the
    // "off" default and is never written to the URL; 'true' appears as ?regex=true.
    // An unrecognised value such as ?regex=false leaves the checkbox unchecked, which is
    // intentionally correct behaviour.
    { param: 'regex',   getter: () => regexModeCheckbox?.checked ? 'true' : '',      setter: (v) => { if (regexModeCheckbox) regexModeCheckbox.checked = v === 'true'; }, defaultValue: '' },
    { param: 'symbols', getter: () => symbolsModeCheckbox?.checked ? 'true' : '',    setter: (v) => { if (symbolsModeCheckbox) symbolsModeCheckbox.checked = v === 'true'; }, defaultValue: '' },
    { param: 'rank',    getter: () => rankModeSelect?.value || 'auto',               setter: (v) => { if (rankModeSelect) rankModeSelect.value = v; },                 defaultValue: 'auto' },
    { param: 'context', getter: () => contextLinesSelect?.value || '0',             setter: (v) => { if (contextLinesSelect) contextLinesSelect.value = v; },         defaultValue: '0' },
];

async function performSearch() {
    // Don't search if index isn't ready yet
    if (!searchReadiness.isReady) {
        return;
    }
    
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value, 10);
    const includeFilter = includeFilterInput?.value.trim() || '';
    const excludeFilter = excludeFilterInput?.value.trim() || '';
    const isRegex = regexModeCheckbox?.checked || false;
    const symbolsOnly = symbolsModeCheckbox?.checked || false;
    const rankMode = rankModeSelect?.value || 'auto';
    const contextLines = parseInt(contextLinesSelect?.value || '0', 10);

    // Keep URL in sync so searches can be shared as links
    syncUrlFromState();
    // Persist settings to localStorage
    saveSettingsToStorage();

    if (!query) {
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter a search query to find code</p></div>';
        return;
    }

    if (query.length < 3) {
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter at least 3 characters to search</p></div>';
        return;
    }

    resultsContainer.innerHTML = '<div class="loading">Searching...</div>';
    resultsHeader.style.display = 'none';

    const startTime = performance.now();

    try {
        const params = new URLSearchParams({ q: query, max: maxResults });
        if (includeFilter) params.set('include', includeFilter);
        if (excludeFilter) params.set('exclude', excludeFilter);
        if (isRegex) params.set('regex', 'true');
        if (symbolsOnly) params.set('symbols', 'true');
        if (rankMode !== 'auto') params.set('rank', rankMode);
        if (contextLines > 0) params.set('context', String(contextLines));
        
        const response = await fetch(`${API_BASE}/api/search?${params}`);
        if (!response.ok) throw new Error(`Search failed: ${response.statusText}`);

        const data = await response.json();
        const duration = data.elapsed_ms !== undefined ? data.elapsed_ms : (performance.now() - startTime);

        // Save successful query to history
        saveToHistory(query);

        resultsHeader.style.display = 'flex';
        resultsCount.textContent = `${data.results.length} RESULT${data.results.length !== 1 ? 'S' : ''} FOUND`;
        searchTimeEl.textContent = `LATENCY: ${duration.toFixed(1)}ms`;

        // Show ranking info if available
        if (data.rank_mode && data.total_candidates !== undefined) {
            const modeLabel = data.rank_mode === 'fast' ? '⚡ Fast' : (data.rank_mode === 'full' ? '📊 Full' : '🔄 Auto');
            const candidateInfo = data.candidates_searched !== data.total_candidates 
                ? `${data.candidates_searched.toLocaleString()}/${data.total_candidates.toLocaleString()} files`
                : `${data.total_candidates.toLocaleString()} files`;
            rankingInfoEl.textContent = `${modeLabel} (${candidateInfo})`;
            rankingInfoEl.title = `Ranking mode: ${data.rank_mode}\nTotal candidates: ${data.total_candidates}\nSearched: ${data.candidates_searched}`;
        } else {
            rankingInfoEl.textContent = '';
            rankingInfoEl.title = '';
        }

        if (data.results.length === 0) {
            resultsContainer.innerHTML = `<div class="empty-state"><p>No results found for "${escapeHtml(query)}"</p></div>`;
            return;
        }

        resultsContainer.innerHTML = data.results.map(result => {
            const matchType = getMatchTypeLabel(result.match_type);
            const depCount = result.dependency_count || 0;
            const lang = hljsLangForPath(result.file_path);
            const ext = (result.file_path.split('.').pop() || '').toLowerCase();
            const langClass = langClassForPath(result.file_path);

            // Split path into directory + filename for display
            const pathParts = result.file_path.split('/');
            const fileName = pathParts.pop();
            const dirPath = pathParts.length ? pathParts.join('/') + '/' : '';

            // File type icon based on extension
            const fileIcon = ext === 'md' ? 'description' : (ext === 'yaml' || ext === 'yml' || ext === 'toml' || ext === 'json' ? 'settings_suggest' : 'code');

            // Language badge style
            const langBadgeStyle = getLangBadgeStyle(langClass);

            // Dependency badge
            const depBadge = depCount > 0
                ? `<span style="cursor:pointer;padding:2px 6px;background:#ebe77f;color:#000;font-size:10px;font-family:'JetBrains Mono',monospace;border:1px solid rgba(0,0,0,0.2)"
                    title="${depCount} files depend on this" onclick="showDependents('${escapeHtml(result.file_path)}')">${depCount} deps</span>`
                : '';

            // Match type badge
            const typeBadgeStyle = matchType.isSymbol
                ? 'background:#a9efed;color:#00201f;border:1px solid #1e6868'
                : 'background:#e7e3ce;color:#494831;border:1px solid #cbc8aa';

            // Build code content — with context lines if available, otherwise just the match line
            let codeContent;
            if (result.context_lines && result.context_lines.length > 0) {
                const startLine = result.context_start_line || 1;
                codeContent = result.context_lines.map((line, i) => {
                    const lineNum = startLine + i;
                    const isMatch = lineNum === result.line_number;
                    const lineStyle = isMatch
                        ? 'display:flex;background:#fffde7;border-left:3px solid #646100'
                        : 'display:flex;border-left:3px solid transparent';
                    return `<div style="${lineStyle}">` +
                        `<span style="flex-shrink:0;width:3.5em;text-align:right;padding-right:0.75em;color:#9e9c80;font-size:0.75em;user-select:none;line-height:1.5em">${lineNum}</span>` +
                        `<span class="ctx-line-content${isMatch ? ' match-line' : ''}" style="flex:1;white-space:pre;overflow-x:auto">${escapeHtml(line)}</span>` +
                        `</div>`;
                }).join('');
            } else {
                codeContent = escapeHtml(result.content);
            }

            const preClass = result.context_lines
                ? `result-code result-code-ctx language-${lang}`
                : `result-code language-${lang}`;

            return `
                <div class="bg-white border border-black overflow-hidden" style="box-shadow:2px 2px 0 #000" data-file-path="${escapeHtml(result.file_path)}" data-line-number="${result.line_number}">
                    <!-- Card header -->
                    <div class="border-b border-black px-4 py-2 flex justify-between items-center" style="background:#dedac6">
                        <div class="flex items-center gap-2 min-w-0">
                            <span class="material-symbols-outlined" style="font-size:16px;flex-shrink:0">${fileIcon}</span>
                            <span class="font-label text-xs font-bold tracking-tight truncate" title="${escapeHtml(result.file_path)}">
                                ${dirPath ? `<span style="color:#7a785f;font-weight:400">${escapeHtml(dirPath)}</span>` : ''}<span style="color:#646100;font-weight:700">${escapeHtml(fileName)}</span>
                            </span>
                            <span class="font-label text-xs" style="color:#7a785f;flex-shrink:0">:${result.line_number}</span>
                        </div>
                        <div class="flex items-center gap-3 flex-shrink-0">
                            <span style="cursor:help;font-family:'JetBrains Mono',monospace;font-size:10px;color:#7a785f;text-transform:uppercase"
                                title="Score = base × multipliers&#10;&#10;• Exact case match: 2×&#10;• Symbol definition: 3×&#10;• In /src/ or /lib/: 1.5×&#10;• Match at start of line: 1.5×&#10;• Shorter lines preferred (log scale, min 0.3×)&#10;• Dependency boost: 1 + log10(import count)&#10;&#10;Higher scores rank first.">
                                ${result.score.toFixed(2)}
                            </span>
                            <button class="view-file-btn material-symbols-outlined hover:text-primary transition-colors"
                                style="font-size:18px;cursor:pointer;color:#7a785f;background:none;border:none;padding:0"
                                data-file-path="${escapeHtml(result.file_path)}"
                                data-line-number="${result.line_number}"
                                title="View full file">open_in_new</button>
                        </div>
                    </div>
                    <!-- Code content -->
                    <div class="overflow-x-auto" style="background:#fff">
                        <pre class="${preClass}" data-query="${escapeHtml(query)}" data-has-context="${result.context_lines ? 'true' : 'false'}" data-lang="${lang}">${codeContent}</pre>
                    </div>
                    <!-- Footer badges -->
                    <div class="px-4 py-1.5 flex gap-2 flex-wrap items-center" style="background:#f8f4df;border-top:1px solid #cbc8aa">
                        ${ext ? `<span style="${langBadgeStyle};padding:2px 6px;font-size:10px;font-family:'JetBrains Mono',monospace;text-transform:uppercase">${escapeHtml(ext)}</span>` : ''}
                        <span style="${typeBadgeStyle};padding:2px 6px;font-size:10px;font-family:'JetBrains Mono',monospace">${matchType.text}</span>
                        ${depBadge}
                    </div>
                </div>
            `;
        }).join('');

        // Apply syntax highlighting then re-apply query-term highlight on each result
        resultsContainer.querySelectorAll('pre.result-code').forEach(pre => {
            const hasContext = pre.dataset.hasContext === 'true';
            if (typeof hljs !== 'undefined') {
                if (hasContext) {
                    // Highlight individual line content spans for context view
                    pre.querySelectorAll('.ctx-line-content').forEach(span => {
                        const code = document.createElement('code');
                        code.className = `language-${pre.dataset.lang || 'plaintext'}`;
                        code.textContent = span.textContent;
                        hljs.highlightElement(code);
                        span.innerHTML = code.innerHTML;
                    });
                } else {
                    hljs.highlightElement(pre);
                }
            }
            const q = pre.dataset.query;
            if (q) {
                if (hasContext) {
                    pre.querySelectorAll('.ctx-line-content').forEach(span => applyQueryHighlight(span, q));
                } else {
                    applyQueryHighlight(pre, q);
                }
            }
        });

        // Attach View button click handler and context tooltip
        resultsContainer.querySelectorAll('.view-file-btn').forEach(btn => {
            const filePath = btn.dataset.filePath;
            const lineNumber = parseInt(btn.dataset.lineNumber, 10);
            btn.addEventListener('click', () => showFileModal(filePath, lineNumber));
            btn.addEventListener('mouseenter', () => showContextTooltip(btn, filePath, lineNumber));
            btn.addEventListener('mouseleave', hideContextTooltip);
        });

    } catch (error) {
        console.error('Search error:', error);
        showError('results', error.message);
    }
}

const debouncedSearch = debounce(performSearch, DEBOUNCE_MS);

// ============================================
// DEPENDENCY MODAL
// ============================================

async function showDependents(filePath) {
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

function showDependencyModal(title, filePath, files, description) {
    const existingModal = document.getElementById('dep-modal');
    if (existingModal) existingModal.remove();

    const fileList = files.length > 0
        ? files.map(f => `<li style="padding:0.25rem 0;font-family:monospace;font-size:0.85rem;">${escapeHtml(f)}</li>`).join('')
        : '<li style="color:var(--text-muted);">No files found</li>';

    const modal = document.createElement('div');
    modal.id = 'dep-modal';
    modal.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.6);display:flex;align-items:center;justify-content:center;z-index:1000;';
    modal.innerHTML = `
        <div style="background:var(--bg-secondary);border-radius:8px;padding:1.5rem;max-width:600px;width:90%;max-height:80vh;overflow:auto;border:1px solid var(--border);">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;">
                <h2 style="font-size:1.1rem;">${title} (${files.length})</h2>
                <button onclick="closeModal()" style="background:none;border:none;font-size:1.5rem;cursor:pointer;color:var(--text-secondary);">&times;</button>
            </div>
            <p style="font-family:monospace;font-size:0.85rem;color:var(--accent);margin-bottom:0.5rem;">${escapeHtml(filePath)}</p>
            <p style="color:var(--text-secondary);font-size:0.85rem;margin-bottom:0.75rem;">${description}</p>
            <ul style="list-style:none;padding:0;">${fileList}</ul>
        </div>
    `;

    modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
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
        await populateFileView(body, filePath, highlightLine, queryInput.value.trim());
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
    performSearch();
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
        performSearch();
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

maxResultsSelect.addEventListener('change', performSearch);

if (regexModeCheckbox) regexModeCheckbox.addEventListener('change', performSearch);
if (symbolsModeCheckbox) symbolsModeCheckbox.addEventListener('change', performSearch);
if (rankModeSelect) rankModeSelect.addEventListener('change', performSearch);
if (contextLinesSelect) contextLinesSelect.addEventListener('change', performSearch);

if (includeFilterInput) {
    includeFilterInput.addEventListener('input', debouncedSearch);
    includeFilterInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') performSearch(); });
}
if (excludeFilterInput) {
    excludeFilterInput.addEventListener('input', debouncedSearch);
    excludeFilterInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') performSearch(); });
}

// ============================================
// INITIALIZATION
// ============================================

// Store original placeholder for restoration when readiness state changes.
searchReadiness.storeDefaultPlaceholder();

// Load persisted settings from localStorage first (URL params will override below)
loadSettingsFromStorage();

// Restore state from URL on page load; URL params take precedence over localStorage.
// Auto-expand filter panel for non-default values.
loadStateFromUrl(URL_FIELDS, () => {
    const filterPanel = document.getElementById('filter-panel');
    if (filterPanel) filterPanel.classList.add('open');
    const advancedDetails = document.querySelector('.advanced-options');
    if (advancedDetails) advancedDetails.open = true;
});

// Probe the backend; start the WebSocket only after confirmation.
// Inputs start enabled (optimistic) — health check disables them only on failure.
checkBackendHealth().then(() => {
    progressWS.start();
    fetchStats();
    runInitialSearchFromUrl();
});
