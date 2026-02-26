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

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    resultsContainerId: 'results',
    additionalInputIds: ['include-filter', 'exclude-filter', 'max-results', 'regex-mode', 'symbols-mode', 'rank-mode'],
    onReadyChange: (isReady, status) => {
        console.log(`Search readiness changed: ${isReady ? 'READY' : 'NOT READY'}`, status?.status);
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
    onConnected: () => {
        console.log('Progress WebSocket connected');
    },
    onDisconnected: () => {
        console.log('Progress WebSocket disconnected');
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

async function performSearch() {
    // Don't search if index isn't ready yet
    if (!searchReadiness.isReady) {
        console.log('Search blocked: index not ready');
        return;
    }
    
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value, 10);
    const includeFilter = includeFilterInput?.value.trim() || '';
    const excludeFilter = excludeFilterInput?.value.trim() || '';
    const isRegex = regexModeCheckbox?.checked || false;
    const symbolsOnly = symbolsModeCheckbox?.checked || false;
    const rankMode = rankModeSelect?.value || 'auto';

    if (!query) {
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = '<div class="text-center py-12 text-zinc-500"><p>Enter a search query to find code</p></div>';
        
        // Reset layout to center
        const mainContainer = document.getElementById('main-container');
        if (mainContainer) {
            mainContainer.classList.add('justify-center', 'min-h-screen');
            mainContainer.classList.remove('pt-12', 'pb-6');
        }
        return;
    }

    // Transition layout to top
    const mainContainer = document.getElementById('main-container');
    if (mainContainer) {
        mainContainer.classList.remove('justify-center', 'min-h-screen');
        mainContainer.classList.add('pt-12', 'pb-6');
    }

    resultsContainer.innerHTML = '<div class="flex justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500"></div></div>';
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
            resultsContainer.innerHTML = `<div class="text-center py-12 text-zinc-500"><p>No results found for "${escapeHtml(query)}"</p></div>`;
            return;
        }

        resultsContainer.innerHTML = data.results.map(result => {
            const matchType = getMatchTypeLabel(result.match_type);
            const depCount = result.dependency_count || 0;
            const depBadge = depCount > 0 
                ? `<span class="px-2 py-0.5 bg-zinc-800 text-zinc-300 rounded text-xs cursor-pointer hover:bg-zinc-700 transition-colors" title="${depCount} files depend on this" onclick="showDependents('${escapeHtml(result.file_path)}')">${depCount} deps</span>`
                : '';
            return `
                <div class="bg-zinc-900/50 border border-zinc-800/80 rounded-xl overflow-hidden hover:border-zinc-700 transition-colors shadow-sm">
                    <div class="flex justify-between items-center px-4 py-3 bg-zinc-900/80 border-b border-zinc-800/80">
                        <div class="flex items-center gap-2 font-mono text-sm">
                            <span class="text-indigo-400">${escapeHtml(result.file_path)}</span>
                            <span class="text-zinc-500">:${result.line_number}</span>
                        </div>
                        <div class="flex items-center gap-3 text-xs">
                            ${depBadge}
                            <span class="text-zinc-500">Score: ${result.score.toFixed(2)}</span>
                            <span class="px-2 py-0.5 rounded ${matchType.isSymbol ? 'bg-indigo-500/20 text-indigo-300 border border-indigo-500/30' : 'bg-zinc-800 text-zinc-400'}">${matchType.text}</span>
                        </div>
                    </div>
                    <div class="p-4 overflow-x-auto">
                        <pre class="font-mono text-sm text-zinc-300 m-0 leading-relaxed">${highlightMatches(result.content, query)}</pre>
                    </div>
                </div>
            `;
        }).join('');

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

// Start with search disabled until we know the status
searchReadiness.update({ status: 'loading_index', message: 'Connecting to server...' });

fetchStats();
progressWS.start();
