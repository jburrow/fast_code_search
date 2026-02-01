// Fast Code Search - Web UI

const API_BASE = '';

// DOM Elements
const queryInput = document.getElementById('query');
const maxResultsSelect = document.getElementById('max-results');
const includeFilterInput = document.getElementById('include-filter');
const excludeFilterInput = document.getElementById('exclude-filter');
const regexModeCheckbox = document.getElementById('regex-mode');
const resultsContainer = document.getElementById('results');
const resultsHeader = document.getElementById('results-header');
const resultsCount = document.getElementById('results-count');
const searchTime = document.getElementById('search-time');

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
const progressDetails = document.getElementById('progress-details');

// Status polling state
let statusInterval = null;
const STATUS_POLL_FAST_MS = 1000;  // During indexing
const STATUS_POLL_SLOW_MS = 30000; // When idle/complete

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

// Format elapsed time to human readable
function formatElapsed(secs) {
    if (!secs) return '';
    if (secs < 60) return `${secs.toFixed(1)}s`;
    const mins = Math.floor(secs / 60);
    const remainingSecs = secs % 60;
    return `${mins}m ${remainingSecs.toFixed(0)}s`;
}

// Fetch indexing status
async function fetchStatus() {
    try {
        const response = await fetch(`${API_BASE}/api/status`);
        if (!response.ok) throw new Error('Failed to fetch status');
        
        const status = await response.json();
        updateProgressUI(status);
        
        // Adjust polling frequency based on indexing state
        adjustStatusPolling(status.is_indexing);
        
        // Also refresh stats when status changes
        if (status.is_indexing || status.status === 'completed') {
            fetchStats();
        }
    } catch (error) {
        console.error('Failed to fetch status:', error);
        // Hide progress panel on error
        if (progressPanel) {
            progressPanel.style.display = 'none';
        }
    }
}

// Update progress UI elements
function updateProgressUI(status) {
    if (!progressPanel) return;
    
    const isActive = status.is_indexing;
    const isCompleted = status.status === 'completed';
    const isIdle = status.status === 'idle';
    
    // Show/hide progress panel
    progressPanel.style.display = isIdle ? 'none' : 'block';
    
    // Update progress bar
    if (progressBar) {
        progressBar.style.width = `${status.progress_percent}%`;
        progressBar.className = `progress-fill ${isCompleted ? 'completed' : ''}`;
    }
    
    // Update percentage
    if (progressPercent) {
        progressPercent.textContent = `${status.progress_percent}%`;
    }
    
    // Update status badge
    if (progressStatus) {
        const statusLabels = {
            'idle': 'Ready',
            'discovering': 'Discovering',
            'indexing': 'Indexing',
            'resolving_imports': 'Resolving',
            'completed': 'Complete'
        };
        progressStatus.textContent = statusLabels[status.status] || status.status;
        progressStatus.className = `status-badge status-${status.status}`;
    }
    
    // Update message
    if (progressMessage) {
        progressMessage.textContent = status.message;
    }
    
    // Update details
    if (progressDetails) {
        let details = [];
        
        if (status.files_discovered > 0) {
            details.push(`${formatNumber(status.files_discovered)} files discovered`);
        }
        if (status.files_indexed > 0) {
            details.push(`${formatNumber(status.files_indexed)} indexed`);
        }
        if (status.total_batches > 0 && status.status === 'indexing') {
            details.push(`batch ${status.current_batch}/${status.total_batches}`);
        }
        if (status.elapsed_secs) {
            details.push(formatElapsed(status.elapsed_secs));
        }
        if (status.errors > 0) {
            details.push(`${status.errors} errors`);
        }
        
        progressDetails.textContent = details.join(' • ');
    }
}

// Adjust polling frequency based on indexing state
function adjustStatusPolling(isIndexing) {
    const targetInterval = isIndexing ? STATUS_POLL_FAST_MS : STATUS_POLL_SLOW_MS;
    
    // Only change if needed
    if (statusInterval && statusInterval.interval === targetInterval) {
        return;
    }
    
    // Clear existing interval
    if (statusInterval) {
        clearInterval(statusInterval.id);
    }
    
    // Set new interval
    const id = setInterval(fetchStatus, targetInterval);
    statusInterval = { id, interval: targetInterval };
}

// Perform search
async function performSearch() {
    const query = queryInput.value.trim();
    const maxResults = parseInt(maxResultsSelect.value, 10);
    const includeFilter = includeFilterInput?.value.trim() || '';
    const excludeFilter = excludeFilterInput?.value.trim() || '';
    const isRegex = regexModeCheckbox?.checked || false;

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
        if (includeFilter) {
            params.set('include', includeFilter);
        }
        if (excludeFilter) {
            params.set('exclude', excludeFilter);
        }
        if (isRegex) {
            params.set('regex', 'true');
        }
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

// Regex mode checkbox listener
if (regexModeCheckbox) {
    regexModeCheckbox.addEventListener('change', performSearch);
}

// Filter input event listeners
if (includeFilterInput) {
    includeFilterInput.addEventListener('input', handleSearchInput);
    includeFilterInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            if (searchTimeout) clearTimeout(searchTimeout);
            performSearch();
        }
    });
}
if (excludeFilterInput) {
    excludeFilterInput.addEventListener('input', handleSearchInput);
    excludeFilterInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            if (searchTimeout) clearTimeout(searchTimeout);
            performSearch();
        }
    });
}

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

// Initial status load and start polling
fetchStatus();

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
