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
const resultsContainer = document.getElementById('results');
const resultsHeader = document.getElementById('results-header');
const resultsCount = document.getElementById('results-count');
const rankingInfoEl = document.getElementById('ranking-info');
const searchTimeEl = document.getElementById('search-time');

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

    // Auto-expand Options if any non-default values were loaded (regex/symbols are now always visible)
    const hasAdvanced = params.has('include') || params.has('exclude') ||
        params.has('rank') || params.has('max');
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

    const qs = params.toString();
    history.replaceState(null, '', qs ? `?${qs}` : location.pathname);
}

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    resultsContainerId: 'results',
    additionalInputIds: ['include-filter', 'exclude-filter', 'max-results', 'regex-mode', 'symbols-mode', 'rank-mode'],
    onReadyChange: (isReady, status) => {
        if (isReady && queryInput.value.trim()) {
            // If user typed while waiting, trigger search now
            performSearch();
        }
    }
});

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
    onConnected: () => {},
    onDisconnected: () => {},
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
    const gap = 10;
    const rect = anchor.getBoundingClientRect(); // viewport-relative (works with position:fixed)
    const viewW = window.innerWidth;
    const viewH = window.innerHeight;

    const spaceLeft = rect.left - gap * 2;
    const spaceRight = viewW - rect.right - gap * 2;
    // Max width that can ever fit on screen regardless of side
    const maxFit = viewW - gap * 2;

    let width, left;
    if (spaceLeft >= spaceRight || spaceLeft >= 220) {
        // Place LEFT of the button
        width = Math.min(Math.max(220, spaceLeft), 960, maxFit);
        left = rect.left - width - gap;
    } else {
        // More space on the right — place RIGHT of the button
        width = Math.min(Math.max(220, spaceRight), 960, maxFit);
        left = rect.right + gap;
    }

    // Clamp so the right edge never escapes the viewport
    left = Math.max(gap, Math.min(left, viewW - width - gap));

    // Override CSS min-width so it doesn't re-expand beyond what fits
    tooltip.style.minWidth = '0';
    tooltip.style.width = `${width}px`;
    tooltip.style.left = `${left}px`;

    // Vertically centre on the button, clamped to viewport
    const ttH = tooltip.offsetHeight || 520;
    let top = rect.top + (rect.height / 2) - (ttH / 2);
    if (top + ttH > viewH - gap) top = viewH - ttH - gap;
    if (top < gap) top = gap;
    tooltip.style.top = `${top}px`;
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

    // Keep URL in sync so searches can be shared as links
    syncUrlFromState();

    if (!query) {
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="empty-state"><p>Enter a search query to find code</p></div>';
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
        
        const response = await fetch(`${API_BASE}/api/search?${params}`);
        if (!response.ok) throw new Error(`Search failed: ${response.statusText}`);

        const data = await response.json();
        const duration = data.elapsed_ms !== undefined ? data.elapsed_ms : (performance.now() - startTime);

        resultsHeader.style.display = 'flex';
        resultsCount.textContent = `${data.results.length} result${data.results.length !== 1 ? 's' : ''}`;
        searchTimeEl.textContent = `${duration.toFixed(1)}ms`;

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
            const depBadge = depCount > 0 
                ? `<span class="result-badge" title="${depCount} files depend on this" style="cursor:pointer" onclick="showDependents('${escapeHtml(result.file_path)}')">${depCount} deps</span>`
                : '';
            const lang = hljsLangForPath(result.file_path);
            return `
                <div class="result-item" data-file-path="${escapeHtml(result.file_path)}" data-line-number="${result.line_number}">
                    <div class="result-header">
                        <div class="result-info">
                            <span class="result-path" title="${escapeHtml(result.file_path)}">${escapeHtml(result.file_path)}</span>
                            <span class="result-line">:${result.line_number}</span>
                        </div>
                        <div class="result-meta">
                            ${depBadge}
                            <span class="result-score">Score: ${result.score.toFixed(2)}</span>
                            <span class="result-type ${matchType.isSymbol ? 'symbol' : ''}">${matchType.text}</span>
                            <button class="view-file-btn"
                                data-file-path="${escapeHtml(result.file_path)}"
                                data-line-number="${result.line_number}">📄 View</button>
                        </div>
                    </div>
                    <div class="result-content">
                        <pre class="result-code language-${lang}" data-query="${escapeHtml(query)}">${escapeHtml(result.content)}</pre>
                    </div>
                </div>
            `;
        }).join('');

        // Apply syntax highlighting then re-apply query-term highlight on each result
        resultsContainer.querySelectorAll('pre.result-code').forEach(pre => {
            if (typeof hljs !== 'undefined') {
                hljs.highlightElement(pre);
            }
            const q = pre.dataset.query;
            if (q) applyQueryHighlight(pre, q);
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

// ============================================
// EVENT LISTENERS
// ============================================

queryInput.addEventListener('input', debouncedSearch);
queryInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') performSearch();
});

maxResultsSelect.addEventListener('change', performSearch);

if (regexModeCheckbox) regexModeCheckbox.addEventListener('change', performSearch);
if (symbolsModeCheckbox) symbolsModeCheckbox.addEventListener('change', performSearch);
if (rankModeSelect) rankModeSelect.addEventListener('change', performSearch);

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

// Store original placeholder before readiness manager may change it
searchReadiness.storeDefaultPlaceholder();

// Restore any state encoded in the URL (shared link or browser back navigation)
loadStateFromUrl();

// Start with search disabled until we know the status
searchReadiness.update({ status: 'loading_index', message: 'Connecting to server...' });

// Restore state from URL on page load; auto-expand Advanced Options for non-default values
loadStateFromUrl(URL_FIELDS, () => {
    const advancedDetails = document.querySelector('.advanced-options');
    if (advancedDetails) advancedDetails.open = true;
});

fetchStats();
progressWS.start();
