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
const statDeps = document.getElementById('stat-deps');

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
        statDeps.textContent = formatNumber(stats.dependency_edges || 0);
    } catch (error) {
        console.error('Failed to fetch stats:', error);
        statFiles.textContent = '-';
        statSize.textContent = '-';
        statTrigrams.textContent = '-';
        statDeps.textContent = '-';
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
            const depCount = result.dependency_count || 0;
            const depBadge = depCount > 0 
                ? `<span class="dep-badge" title="${depCount} files depend on this" onclick="showDependents('${escapeHtml(result.file_path)}')">${depCount} deps</span>`
                : '';
            return `
                <div class="result-item">
                    <div class="result-header">
                        <div>
                            <span class="result-path">${escapeHtml(result.file_path)}</span>
                            <span class="result-line">:${result.line_number}</span>
                        </div>
                        <div class="result-meta">
                            ${depBadge}
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

// Show dependents modal
async function showDependents(filePath) {
    try {
        const params = new URLSearchParams({ file: filePath });
        const response = await fetch(`${API_BASE}/api/dependents?${params}`);
        
        if (!response.ok) {
            throw new Error(`Failed to fetch dependents: ${response.statusText}`);
        }

        const data = await response.json();
        showDependencyModal('Dependents', filePath, data.files, 'Files that import this file:');
    } catch (error) {
        console.error('Error fetching dependents:', error);
        alert('Failed to load dependents: ' + error.message);
    }
}

// Show dependencies modal
async function showDependencies(filePath) {
    try {
        const params = new URLSearchParams({ file: filePath });
        const response = await fetch(`${API_BASE}/api/dependencies?${params}`);
        
        if (!response.ok) {
            throw new Error(`Failed to fetch dependencies: ${response.statusText}`);
        }

        const data = await response.json();
        showDependencyModal('Dependencies', filePath, data.files, 'Files imported by this file:');
    } catch (error) {
        console.error('Error fetching dependencies:', error);
        alert('Failed to load dependencies: ' + error.message);
    }
}

// Display dependency modal
function showDependencyModal(title, filePath, files, description) {
    // Remove existing modal if any
    const existingModal = document.getElementById('dep-modal');
    if (existingModal) {
        existingModal.remove();
    }

    const fileList = files.length > 0
        ? files.map(f => `<li class="dep-file">${escapeHtml(f)}</li>`).join('')
        : '<li class="dep-none">No files found</li>';

    const modal = document.createElement('div');
    modal.id = 'dep-modal';
    modal.className = 'modal-overlay';
    modal.innerHTML = `
        <div class="modal-content">
            <div class="modal-header">
                <h2>${title} (${files.length})</h2>
                <button class="modal-close" onclick="closeModal()">&times;</button>
            </div>
            <div class="modal-body">
                <p class="modal-file">${escapeHtml(filePath)}</p>
                <p class="modal-desc">${description}</p>
                <ul class="dep-list">
                    ${fileList}
                </ul>
            </div>
        </div>
    `;

    document.body.appendChild(modal);

    // Close on overlay click
    modal.addEventListener('click', (e) => {
        if (e.target === modal) {
            closeModal();
        }
    });

    // Close on Escape key
    document.addEventListener('keydown', handleModalEscape);
}

function handleModalEscape(e) {
    if (e.key === 'Escape') {
        closeModal();
    }
}

function closeModal() {
    const modal = document.getElementById('dep-modal');
    if (modal) {
        modal.remove();
    }
    document.removeEventListener('keydown', handleModalEscape);
}
