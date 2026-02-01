// Fast Code Search - Web UI

const API_BASE = '';

// DOM Elements
const queryInput = document.getElementById('query');
const maxResultsSelect = document.getElementById('max-results');
const resultsContainer = document.getElementById('results');
const resultsHeader = document.getElementById('results-header');
const resultsCount = document.getElementById('results-count');
const searchTime = document.getElementById('search-time');

// Stats elements
const statFiles = document.getElementById('stat-files');
const statSize = document.getElementById('stat-size');
const statTrigrams = document.getElementById('stat-trigrams');

// Debounce timer
let searchTimeout = null;
const DEBOUNCE_MS = 300;

// Format bytes to human readable
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

// Format large numbers with commas
function formatNumber(num) {
    return num.toLocaleString();
}

// Escape HTML to prevent XSS
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Highlight matching text in content
function highlightMatches(content, query) {
    if (!query) return escapeHtml(content);
    
    const escaped = escapeHtml(content);
    const queryEscaped = escapeHtml(query);
    const regex = new RegExp(`(${queryEscaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
    return escaped.replace(regex, '<span class="highlight">$1</span>');
}

// Get match type label
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

// Fetch index stats
async function fetchStats() {
    try {
        const response = await fetch(`${API_BASE}/api/stats`);
        if (!response.ok) throw new Error('Failed to fetch stats');
        
        const stats = await response.json();
        statFiles.textContent = formatNumber(stats.num_files);
        statSize.textContent = formatBytes(stats.total_size);
        statTrigrams.textContent = formatNumber(stats.num_trigrams);
    } catch (error) {
        console.error('Failed to fetch stats:', error);
        statFiles.textContent = '-';
        statSize.textContent = '-';
        statTrigrams.textContent = '-';
    }
}

// Perform search
async function performSearch() {
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value, 10);

    if (!query) {
        resultsHeader.style.display = 'none';
        resultsContainer.innerHTML = `
            <div class="empty-state">
                <p>Enter a search query to find code</p>
            </div>
        `;
        return;
    }

    // Show loading state
    resultsContainer.innerHTML = '<div class="loading">Searching...</div>';
    resultsHeader.style.display = 'none';

    const startTime = performance.now();

    try {
        const params = new URLSearchParams({ q: query, max: maxResults });
        const response = await fetch(`${API_BASE}/api/search?${params}`);
        
        if (!response.ok) {
            throw new Error(`Search failed: ${response.statusText}`);
        }

        const data = await response.json();
        const endTime = performance.now();
        const duration = endTime - startTime;

        // Update results header
        resultsHeader.style.display = 'flex';
        resultsCount.textContent = `${data.results.length} result${data.results.length !== 1 ? 's' : ''}`;
        searchTime.textContent = `${duration.toFixed(0)}ms`;

        if (data.results.length === 0) {
            resultsContainer.innerHTML = `
                <div class="empty-state">
                    <p>No results found for "${escapeHtml(query)}"</p>
                </div>
            `;
            return;
        }

        // Render results
        resultsContainer.innerHTML = data.results.map(result => {
            const matchType = getMatchTypeLabel(result.match_type);
            return `
                <div class="result-item">
                    <div class="result-header">
                        <div>
                            <span class="result-path">${escapeHtml(result.file_path)}</span>
                            <span class="result-line">:${result.line_number}</span>
                        </div>
                        <div class="result-meta">
                            <span class="result-score">Score: ${result.score.toFixed(2)}</span>
                            <span class="result-type ${matchType.isSymbol ? 'symbol' : ''}">${matchType.text}</span>
                        </div>
                    </div>
                    <div class="result-content">
                        <pre>${highlightMatches(result.content, query)}</pre>
                    </div>
                </div>
            `;
        }).join('');

    } catch (error) {
        console.error('Search error:', error);
        resultsContainer.innerHTML = `
            <div class="error">
                <p>Search failed: ${escapeHtml(error.message)}</p>
            </div>
        `;
    }
}

// Debounced search handler
function handleSearchInput() {
    if (searchTimeout) {
        clearTimeout(searchTimeout);
    }
    searchTimeout = setTimeout(performSearch, DEBOUNCE_MS);
}

// Event listeners
queryInput.addEventListener('input', handleSearchInput);
maxResultsSelect.addEventListener('change', performSearch);

// Handle Enter key for immediate search
queryInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
        if (searchTimeout) {
            clearTimeout(searchTimeout);
        }
        performSearch();
    }
});

// Initial stats load
fetchStats();

// Refresh stats periodically (every 30 seconds)
setInterval(fetchStats, 30000);
