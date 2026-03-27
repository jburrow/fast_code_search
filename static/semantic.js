// ============================================
// SEMANTIC SEARCH - View-specific logic
// Uses: common.js
//
// NOTE: This semantic UI is simplified compared to keyword search (keyword.js).
// No tooltips, no full-file modal, no advanced filtering/ranking options.
// If adding these features in the future, ensure tooltip cleanup behavior
// matches keyword.js (call hideContextTooltipImmediately() before opening modals).
// ============================================

const API_BASE = '/api';

// DOM Elements
const queryInput = document.getElementById('query');
const searchBtn = document.getElementById('search-btn');
const maxResultsSelect = document.getElementById('max-results');
const resultsContainer = document.getElementById('results');
const searchHistoryDropdown = document.getElementById('search-history-dropdown');

// Progress elements
const progressPanel = document.getElementById('progress-panel');
const progressBar = document.getElementById('progress-bar');
const progressStatus = document.getElementById('progress-status');
const progressMessage = document.getElementById('progress-message');
const progressPercent = document.getElementById('progress-percent');

// ============================================
// LOCAL STORAGE PERSISTENCE
// ============================================

const LS_SEM_SETTINGS_KEY = 'fcs_sem_settings';
const LS_SEM_HISTORY_KEY = 'fcs_sem_history';
const MAX_SEM_HISTORY = 50;

function saveSettingsToStorage() {
    try {
        const settings = { max: maxResultsSelect?.value || '10' };
        localStorage.setItem(LS_SEM_SETTINGS_KEY, JSON.stringify(settings));
    } catch (_) { /* storage unavailable */ }
}

function loadSettingsFromStorage() {
    try {
        const raw = localStorage.getItem(LS_SEM_SETTINGS_KEY);
        if (!raw) return;
        const s = JSON.parse(raw);
        if (maxResultsSelect && s.max) maxResultsSelect.value = s.max;
    } catch (_) { /* ignore parse errors */ }
}

// ============================================
// SEARCH HISTORY
// ============================================

// ============================================
// SEARCH HISTORY (uses shared utilities from common.js)
// ============================================

function loadHistory() {
    return loadSearchHistory(LS_SEM_HISTORY_KEY);
}

function saveToHistory(query) {
    saveSearchHistory(LS_SEM_HISTORY_KEY, query, MAX_SEM_HISTORY);
}

function clearHistory() {
    clearSearchHistory(LS_SEM_HISTORY_KEY);
    hideHistoryDropdown();
}

function showHistoryDropdown(filter) {
    showSearchHistoryDropdown(searchHistoryDropdown, queryInput, LS_SEM_HISTORY_KEY,
        (selectedQuery) => {
            queryInput.value = selectedQuery;
            performSearch();
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

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    searchButtonId: 'search-btn',
    resultsContainerId: 'results',
    searchSectionId: 'search-section',
    additionalInputIds: ['max-results'],
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

// Tracks whether the semantic backend is confirmed available for this page.
// false = health check hasn't run yet or determined semantic is unavailable.
let semanticAvailable = false;

/**
 * Check both keyword (8080) and semantic (8081) backends.
 * Updates the backend status banner and sets semanticAvailable.
 */
async function checkBackendHealth() {
    const hostname = window.location.hostname;

    // Probe the server serving this page.
    let currentServerType = null;
    let healthOk = false;
    try {
        const resp = await fetch('/api/health', { signal: AbortSignal.timeout(2000) });
        if (resp.ok) {
            healthOk = true;
            const data = await resp.json();
            currentServerType = data.server_type ?? null;
        }
    } catch (e) { /* current server unreachable — WS offline detection handles it */ }

    // Available if health OK and not explicitly identified as the keyword server.
    // Older binaries omit server_type — treat as semantic when health is ok.
    semanticAvailable = healthOk && currentServerType !== 'keyword';

    if (!semanticAvailable) {
        searchReadiness.setOffline(true);
    }

    // Check keyword backend at its default port for the status badge
    let keywordUp = false;
    try {
        const resp = await fetch(`http://${hostname}:8080/api/health`, { signal: AbortSignal.timeout(2000) });
        keywordUp = resp.ok;
    } catch (e) { /* offline */ }

    const onSemanticServer = currentServerType === 'semantic' || currentServerType === null;
    renderBackendStatus(keywordUp, healthOk, onSemanticServer);
}

/**
 * Update the backend status banner based on health check results.
 */
function renderBackendStatus(keywordUp, semanticServerUp, onSemanticServer) {
    const banner = document.getElementById('backend-banner');
    if (!banner) return;

    updateBackendBadge('keyword-status-badge', keywordUp, 'KEYWORD');
    updateBackendBadge('semantic-status-badge', semanticServerUp && onSemanticServer, 'SEMANTIC');

    const msgEl = document.getElementById('backend-banner-msg');

    // Build the problem description
    const parts = [];
    if (!semanticServerUp) {
        parts.push('Semantic backend not running \u2014 start fast_code_search_semantic on port 8081');
    } else if (!onSemanticServer) {
        const semanticUrl = `http://${window.location.hostname}:8081`;
        parts.push(`Semantic search requires the semantic backend \u2014 visit ${semanticUrl}`);
    }
    if (!keywordUp) {
        parts.push('Keyword backend not running');
    }

    if (parts.length === 0) {
        banner.style.display = 'none';
        return;
    }

    // Warning colour: red for hard errors, amber when semantic is up but wrong server
    const isWrongServer = semanticServerUp && !onSemanticServer;
    banner.style.background = isWrongServer && keywordUp ? '#fff3cd' : '#ffdad6';
    banner.style.color = isWrongServer && keywordUp ? '#5a4000' : '#93000a';
    banner.style.display = 'flex';
    if (msgEl) msgEl.textContent = parts.join(' \u00b7 ');
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
// PROGRESS WEBSOCKET
// ============================================

const progressWS = new ProgressWebSocket({
    onUpdate: updateProgressUI,
    onConnected: () => {
        // WS connected means this server is reachable — clear any offline state
        if (semanticAvailable) {
            searchReadiness.setOffline(false);
        }
    },
    onDisconnected: () => {},
    onServerOffline: () => {
        // Consecutive WS failures — the semantic search server is not running
        searchReadiness.setOffline(true);
    },
    onError: (err) => console.error('Semantic progress WebSocket error:', err)
});

function updateProgressUI(status) {
    const isIdle = status.status === 'idle' || !status.status;
    const isCompleted = status.status === 'completed';
    
    // Update search readiness based on status
    searchReadiness.update(status);
    
    if (progressPanel) {
        toggleElement('progress-panel', status.is_indexing || (!isIdle && !isCompleted), 'flex');
    }
    
    if (progressBar) {
        progressBar.style.width = `${status.progress_percent || 0}%`;
        progressBar.className = `progress-fill ${isCompleted ? 'completed' : ''}`;
    }
    
    if (progressPercent) {
        updateStat('progress-percent', `${status.progress_percent || 0}%`);
    }
    
    if (progressStatus) {
        const labels = {
            'idle': 'Ready',
            'indexing': 'Indexing',
            'completed': 'Complete'
        };
        progressStatus.textContent = labels[status.status] || status.status || 'Ready';
        progressStatus.className = `status-badge status-${status.status || 'idle'}`;
    }
    
    if (progressMessage) {
        progressMessage.textContent = status.message || '';
    }
    
    // Update stats from WebSocket message (no separate HTTP request needed)
    if (status.num_files !== undefined) {
        updateStat('stat-files', formatNumber(status.num_files));
        updateStat('stat-chunks', formatNumber(status.num_chunks || 0));
        updateStat('stat-cache', formatNumber(status.cache_size || 0));
    }
}

// ============================================
// STATS
// ============================================

async function loadStats() {
    try {
        const response = await fetch(`${API_BASE}/stats`);
        if (!response.ok) throw new Error(`Failed to load stats: ${response.status}`);
        
        const stats = await response.json();
        updateStat('stat-files', formatNumber(stats.num_files));
        updateStat('stat-chunks', formatNumber(stats.num_chunks));
        updateStat('stat-cache', formatNumber(stats.cache_size));
    } catch (error) {
        console.error('Error loading stats:', error);
        ['stat-files', 'stat-chunks', 'stat-cache'].forEach(id => updateStat(id, 'Error'));
    }
}

// ============================================
// SEARCH
// ============================================

// URL state field descriptors for semantic search
const URL_FIELDS = [
    { param: 'q',   getter: () => queryInput.value.trim(),   setter: (v) => { queryInput.value = v; },   defaultValue: '' },
    { param: 'max', getter: () => maxResultsSelect.value,    setter: (v) => { maxResultsSelect.value = v; }, defaultValue: '10' },
];

async function performSearch() {
    // Don't search if index isn't ready yet
    if (!searchReadiness.isReady) {
        return;
    }
    
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value);
    
    if (!query) return;

    syncUrlFromState(URL_FIELDS);
    // Persist settings to localStorage
    saveSettingsToStorage();
    
    showLoading('results', `Searching for: "${query}"`);
    const startTime = Date.now();
    
    try {
        const url = `${API_BASE}/search?q=${encodeURIComponent(query)}&max=${maxResults}`;
        const response = await fetch(url);
        
        if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
        
        const data = await response.json();
        const latency = Date.now() - startTime;

        // Save successful query to history
        saveToHistory(query);
        
        updateStat('stat-latency', `${latency}ms`);
        displayResults(data.results, query, latency);
        
    } catch (error) {
        console.error('Search error:', error);
        showError('results', error.message);
        updateStat('stat-latency', 'Error');
    }
}

function displayResults(results, query, latency) {
    if (!results || results.length === 0) {
        resultsContainer.innerHTML = `
            <div class="no-results">
                <div class="no-results-icon">🔍</div>
                <h3>No results found</h3>
                <p>Try a different query or check if the server has indexed your files.</p>
                <p style="margin-top: 0.75rem; color: var(--text-secondary); font-size: 0.85rem;">
                    Query: "${escapeHtml(query)}" • Latency: ${latency}ms
                </p>
            </div>
        `;
        return;
    }
    
    const resultsHtml = results.map(result => createResultCard(result)).join('');
    resultsContainer.innerHTML = createResultsSummary(results.length, latency) + resultsHtml;
}

function createResultCard(result) {
    const score = (result.similarity_score * 100).toFixed(1);
    const scorePercent = Math.min(100, parseFloat(score));

    // Chunk type badge
    let chunkLabel = 'CODE';
    let chunkClass = 'result-badge';
    if (result.chunk_type === 'function') { chunkLabel = 'FUNCTION'; chunkClass = 'result-badge chunk-function'; }
    else if (result.chunk_type === 'class')    { chunkLabel = 'CLASS';    chunkClass = 'result-badge chunk-class'; }
    else if (result.chunk_type === 'module')   { chunkLabel = 'MODULE';   chunkClass = 'result-badge chunk-module'; }

    // Language badge (uses shared langClassForPath from common.js)
    const ext = (result.file_path.split('.').pop() || '').toLowerCase();
    const langClass = langClassForPath(result.file_path);
    const langStyle = langClass ? `background:var(--lang-${langClass},#e2e0d5);` : '';
    const langBadge = langClass ? `<span class="lang-badge" style="${langStyle}">${ext}</span>` : '';

    // Split path into dir + filename
    const lastSlash = result.file_path.lastIndexOf('/');
    const fileDir  = lastSlash >= 0 ? result.file_path.slice(0, lastSlash + 1) : '';
    const fileName = lastSlash >= 0 ? result.file_path.slice(lastSlash + 1) : result.file_path;

    const codeWithLines = formatCodeWithLineNumbers(result.content, result.start_line);

    return `
        <div class="result-item">
            <div class="result-header">
                <div class="result-info">
                    <div class="result-file">
                        <span class="material-symbols-outlined" style="font-size:14px;vertical-align:middle;margin-right:4px;color:#7a785f">description</span>
                        <span style="color:#7a785f;font-weight:400">${escapeHtml(fileDir)}</span><span style="font-weight:700">${escapeHtml(fileName)}</span>
                    </div>
                    <div class="result-meta">
                        ${langBadge}
                        <span class="${chunkClass}">${chunkLabel}</span>
                        ${result.symbol_name ? `<span class="result-badge" style="background:#ebe77f;border-color:#646100;color:#1e1d00">${escapeHtml(result.symbol_name)}</span>` : ''}
                        <span style="font-size:0.65rem;color:#7a785f;font-family:'JetBrains Mono',monospace">L${result.start_line}–${result.end_line}</span>
                    </div>
                </div>
                <div class="result-score" title="Similarity score: ${score}%">
                    <div class="score-bar">
                        <div class="score-fill" style="width: ${scorePercent}%"></div>
                    </div>
                    <span class="score-value">${score}%</span>
                </div>
            </div>
            <div class="result-content">
                <pre><code>${codeWithLines}</code></pre>
            </div>
        </div>
    `;
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
queryInput.addEventListener('input', (e) => {
    showHistoryDropdown(e.target.value.trim());
});
document.addEventListener('click', (e) => {
    if (e.target !== queryInput && !searchHistoryDropdown?.contains(e.target)) {
        hideHistoryDropdown();
    }
});

document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        queryInput?.focus();
    }
});

searchBtn.addEventListener('click', () => {
    hideHistoryDropdown();
    performSearch();
});

// Example query buttons
document.querySelectorAll('.example-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        queryInput.value = btn.getAttribute('data-query');
        performSearch();
    });
});

// ============================================
// INITIALIZATION
// ============================================

// Store original placeholder for restoration when readiness state changes.
searchReadiness.storeDefaultPlaceholder();

// Load persisted settings from localStorage first (URL params will override below)
loadSettingsFromStorage();

// Restore state from URL on page load; URL params take precedence over localStorage.
// Auto-expand Advanced Options for non-default values.
loadStateFromUrl(URL_FIELDS, () => {
    const advancedDetails = document.querySelector('.advanced-options');
    if (advancedDetails) advancedDetails.open = true;
});

// Probe the backend; start the WebSocket only after confirmation.
// Inputs start enabled (optimistic) — health check disables them only on failure.
checkBackendHealth().then(() => {
    progressWS.start();
    loadStats();
    runInitialSearchFromUrl();
});
