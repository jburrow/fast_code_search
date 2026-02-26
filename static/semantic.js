// ============================================
// SEMANTIC SEARCH - View-specific logic
// Uses: common.js
// ============================================

const API_BASE = '/api';

// DOM Elements
const queryInput = document.getElementById('query');
const searchBtn = document.getElementById('search-btn');
const maxResultsSelect = document.getElementById('max-results');
const resultsContainer = document.getElementById('results');

// Progress elements
const progressPanel = document.getElementById('progress-panel');
const progressBar = document.getElementById('progress-bar');
const progressStatus = document.getElementById('progress-status');
const progressMessage = document.getElementById('progress-message');
const progressPercent = document.getElementById('progress-percent');

// Search readiness manager (disables search until index is ready)
const searchReadiness = new SearchReadinessManager({
    searchInputId: 'query',
    searchButtonId: 'search-btn',
    resultsContainerId: 'results',
    additionalInputIds: ['max-results'],
    onReadyChange: (isReady, status) => {
        console.log(`Semantic search readiness changed: ${isReady ? 'READY' : 'NOT READY'}`, status?.status);
        if (isReady && queryInput.value.trim()) {
            // If user typed while waiting, trigger search now
            performSearch();
        }
    }
});

// ============================================
// PROGRESS WEBSOCKET
// ============================================

const progressWS = new ProgressWebSocket({
    onUpdate: updateProgressUI,
    onConnected: () => console.log('Semantic progress WebSocket connected'),
    onDisconnected: () => console.log('Semantic progress WebSocket disconnected'),
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

async function performSearch() {
    // Don't search if index isn't ready yet
    if (!searchReadiness.isReady) {
        console.log('Semantic search blocked: index not ready');
        return;
    }
    
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value);
    
    if (!query) {
        resultsContainer.innerHTML = '';
        
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
    
    resultsContainer.innerHTML = '<div class="flex justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-purple-500"></div></div>';
    const startTime = Date.now();
    
    try {
        const url = `${API_BASE}/search?q=${encodeURIComponent(query)}&max=${maxResults}`;
        const response = await fetch(url);
        
        if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
        
        const data = await response.json();
        const latency = Date.now() - startTime;
        
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
            <div class="text-center py-12 text-zinc-500">
                <div class="text-4xl mb-4">🔍</div>
                <h3 class="text-xl font-medium text-zinc-300 mb-2">No results found</h3>
                <p>Try a different query or check if the server has indexed your files.</p>
                <p class="mt-3 text-sm text-zinc-600">
                    Query: "${escapeHtml(query)}" • Latency: ${latency}ms
                </p>
            </div>
        `;
        return;
    }
    
    const resultsHtml = results.map(result => createResultCard(result)).join('');
    resultsContainer.innerHTML = `
        <div class="flex justify-between items-center mb-4 text-sm text-zinc-400">
            <span>Found ${results.length} results</span>
            <span class="font-mono">${latency}ms</span>
        </div>
    ` + resultsHtml;
}

function createResultCard(result) {
    const score = (result.similarity_score * 100).toFixed(1);
    const scorePercent = Math.min(100, score);
    
    // Determine chunk type badge
    let badgeText = 'Code';
    if (result.chunk_type === 'function') badgeText = '⚡ Function';
    else if (result.chunk_type === 'class') badgeText = '📦 Class';
    else if (result.chunk_type === 'module') badgeText = '📄 Module';
    
    const codeWithLines = formatCodeWithLineNumbers(result.content, result.start_line);
    
    return `
        <div class="bg-zinc-900/50 border border-zinc-800/80 rounded-xl overflow-hidden hover:border-zinc-700 transition-colors shadow-sm mb-4">
            <div class="flex justify-between items-center px-4 py-3 bg-zinc-900/80 border-b border-zinc-800/80">
                <div class="flex flex-col gap-1">
                    <div class="flex items-center gap-2 font-mono text-sm">
                        <span class="text-purple-400">📄 ${escapeHtml(result.file_path)}</span>
                    </div>
                    <div class="flex items-center gap-3 text-xs text-zinc-500">
                        <span>Lines ${result.start_line}-${result.end_line}</span>
                        ${result.symbol_name ? `<span>Symbol: <strong class="text-zinc-300">${escapeHtml(result.symbol_name)}</strong></span>` : ''}
                        <span class="px-2 py-0.5 rounded bg-purple-500/20 text-purple-300 border border-purple-500/30">${badgeText}</span>
                    </div>
                </div>
                <div class="flex flex-col items-end gap-1">
                    <div class="w-24 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                        <div class="h-full bg-purple-500" style="width: ${scorePercent}%"></div>
                    </div>
                    <span class="text-xs text-zinc-400 font-mono">${score}%</span>
                </div>
            </div>
            <div class="p-4 overflow-x-auto">
                <pre class="font-mono text-sm text-zinc-300 m-0 leading-relaxed"><code>${codeWithLines}</code></pre>
            </div>
        </div>
    `;
}

// ============================================
// EVENT LISTENERS
// ============================================

queryInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') performSearch();
});

searchBtn.addEventListener('click', performSearch);

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

// Store original placeholder before readiness manager may change it
searchReadiness.storeDefaultPlaceholder();

// Start with search disabled until we know the status
searchReadiness.update({ status: 'loading_index', message: 'Connecting to server...' });

loadStats();
progressWS.start();
