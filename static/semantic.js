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
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value);
    
    if (!query) return;
    
    showLoading('results', `Searching for: "${query}"`);
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
    const scorePercent = Math.min(100, score);
    
    // Determine chunk type badge
    let badgeText = 'Code';
    if (result.chunk_type === 'function') badgeText = '⚡ Function';
    else if (result.chunk_type === 'class') badgeText = '📦 Class';
    else if (result.chunk_type === 'module') badgeText = '📄 Module';
    
    const codeWithLines = formatCodeWithLineNumbers(result.content, result.start_line);
    
    return `
        <div class="result-item">
            <div class="result-header">
                <div class="result-info">
                    <div class="result-file">📄 ${escapeHtml(result.file_path)}</div>
                    <div class="result-meta">
                        <span>Lines ${result.start_line}-${result.end_line}</span>
                        ${result.symbol_name ? `<span>Symbol: <strong>${escapeHtml(result.symbol_name)}</strong></span>` : ''}
                        <span class="result-badge">${badgeText}</span>
                    </div>
                </div>
                <div class="result-score">
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

loadStats();
setInterval(loadStats, 30000);
