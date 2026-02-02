// Semantic Code Search UI JavaScript

const API_BASE = '/api';
let currentQuery = '';
let queryStartTime = 0;

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    loadStats();
    setupEventListeners();
    
    // Refresh stats every 30 seconds
    setInterval(loadStats, 30000);
});

function setupEventListeners() {
    const queryInput = document.getElementById('query');
    const searchBtn = document.getElementById('search-btn');
    const exampleBtns = document.querySelectorAll('.example-btn');
    
    // Search on Enter key
    queryInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            performSearch();
        }
    });
    
    // Search on button click
    searchBtn.addEventListener('click', performSearch);
    
    // Example queries
    exampleBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            const query = btn.getAttribute('data-query');
            queryInput.value = query;
            performSearch();
        });
    });
}

async function loadStats() {
    try {
        const response = await fetch(`${API_BASE}/stats`);
        if (!response.ok) {
            throw new Error('Failed to load stats');
        }
        
        const stats = await response.json();
        
        document.getElementById('stat-files').textContent = stats.num_files.toLocaleString();
        document.getElementById('stat-chunks').textContent = stats.num_chunks.toLocaleString();
        document.getElementById('stat-cache').textContent = stats.cache_size.toLocaleString();
    } catch (error) {
        console.error('Error loading stats:', error);
        document.getElementById('stat-files').textContent = 'Error';
        document.getElementById('stat-chunks').textContent = 'Error';
        document.getElementById('stat-cache').textContent = 'Error';
    }
}

async function performSearch() {
    const query = document.getElementById('query').value.trim();
    const maxResults = parseInt(document.getElementById('max-results').value);
    
    if (!query) {
        return;
    }
    
    currentQuery = query;
    const resultsContainer = document.getElementById('results');
    
    // Show loading
    resultsContainer.innerHTML = `
        <div class="loading">
            <div class="loading-spinner"></div>
            <p style="margin-top: 1rem;">Searching for: "${escapeHtml(query)}"</p>
        </div>
    `;
    
    queryStartTime = Date.now();
    
    try {
        const url = `${API_BASE}/search?q=${encodeURIComponent(query)}&max=${maxResults}`;
        const response = await fetch(url);
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        const data = await response.json();
        const latency = Date.now() - queryStartTime;
        
        // Update latency stat
        document.getElementById('stat-latency').textContent = `${latency}ms`;
        
        displayResults(data.results, query, latency);
        
    } catch (error) {
        console.error('Search error:', error);
        resultsContainer.innerHTML = `
            <div class="error-message">
                <strong>Error:</strong> ${escapeHtml(error.message)}
            </div>
        `;
        document.getElementById('stat-latency').textContent = 'Error';
    }
}

function displayResults(results, query, latency) {
    const resultsContainer = document.getElementById('results');
    
    if (!results || results.length === 0) {
        resultsContainer.innerHTML = `
            <div class="no-results">
                <div class="no-results-icon">🔍</div>
                <h3>No results found</h3>
                <p>Try a different query or check if the server has indexed your files.</p>
                <p style="margin-top: 1rem; color: var(--text-secondary);">
                    Query: "${escapeHtml(query)}" • Latency: ${latency}ms
                </p>
            </div>
        `;
        return;
    }
    
    const resultsHtml = results.map((result, index) => {
        return createResultCard(result, index + 1);
    }).join('');
    
    resultsContainer.innerHTML = `
        <div style="margin-bottom: 1rem; color: var(--text-secondary); text-align: center;">
            Found <strong style="color: var(--primary-color);">${results.length}</strong> results 
            in <strong style="color: var(--success-color);">${latency}ms</strong>
        </div>
        ${resultsHtml}
    `;
}

function createResultCard(result, index) {
    const score = (result.similarity_score * 100).toFixed(1);
    const scorePercent = Math.min(100, score);
    
    // Determine chunk type badge
    let badgeText = 'Code';
    if (result.chunk_type === 'function') {
        badgeText = '⚡ Function';
    } else if (result.chunk_type === 'class') {
        badgeText = '📦 Class';
    } else if (result.chunk_type === 'module') {
        badgeText = '📄 Module';
    }
    
    // Format code with line numbers
    const lines = result.content.split('\n');
    const startLine = result.start_line;
    const codeWithLines = lines.map((line, i) => {
        const lineNum = startLine + i;
        return `<span class="line-number">${lineNum}</span>${escapeHtml(line)}`;
    }).join('\n');
    
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

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
