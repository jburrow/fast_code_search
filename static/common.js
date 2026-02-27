// ============================================
// COMMON UTILITIES - Shared across all search views
// ============================================

/**
 * Escape HTML to prevent XSS attacks
 * @param {string} text - Raw text to escape
 * @returns {string} HTML-safe string
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Format bytes to human-readable string
 * @param {number} bytes - Number of bytes
 * @returns {string} Formatted string (e.g., "1.5 MB")
 */
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

/**
 * Format large numbers with locale-specific separators
 * @param {number} num - Number to format
 * @returns {string} Formatted string (e.g., "1,234,567")
 */
function formatNumber(num) {
    return num.toLocaleString();
}

/**
 * Format elapsed time to human-readable string
 * @param {number} secs - Seconds elapsed
 * @returns {string} Formatted string (e.g., "2m 30s")
 */
function formatElapsed(secs) {
    if (!secs) return '';
    if (secs < 60) return `${secs.toFixed(1)}s`;
    const mins = Math.floor(secs / 60);
    const remainingSecs = secs % 60;
    return `${mins}m ${remainingSecs.toFixed(0)}s`;
}

/**
 * Debounce a function call
 * @param {Function} func - Function to debounce
 * @param {number} wait - Milliseconds to wait
 * @returns {Function} Debounced function
 */
function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

/**
 * Update a stat element's text content safely
 * @param {string} elementId - DOM element ID
 * @param {string|number} value - Value to display
 */
function updateStat(elementId, value) {
    const el = document.getElementById(elementId);
    if (el) {
        el.textContent = value;
    }
}

/**
 * Show/hide an element
 * @param {string} elementId - DOM element ID
 * @param {boolean} show - Whether to show the element
 * @param {string} display - Display value when shown (default: 'flex')
 */
function toggleElement(elementId, show, display = 'flex') {
    const el = document.getElementById(elementId);
    if (el) {
        el.style.display = show ? display : 'none';
    }
}

/**
 * Set loading state in a container
 * @param {string} containerId - DOM container ID
 * @param {string} message - Loading message
 */
function showLoading(containerId, message = 'Loading...') {
    const container = document.getElementById(containerId);
    if (container) {
        container.innerHTML = `
            <div class="loading">
                <div class="loading-spinner"></div>
                <p style="margin-top: 0.75rem;">${escapeHtml(message)}</p>
            </div>
        `;
    }
}

/**
 * Show error state in a container
 * @param {string} containerId - DOM container ID
 * @param {string} message - Error message
 */
function showError(containerId, message) {
    const container = document.getElementById(containerId);
    if (container) {
        container.innerHTML = `
            <div class="error-message">
                <strong>Error:</strong> ${escapeHtml(message)}
            </div>
        `;
    }
}

/**
 * Show empty/no results state
 * @param {string} containerId - DOM container ID
 * @param {string} message - Message to display
 * @param {string} icon - Emoji icon (default: 🔍)
 */
function showEmpty(containerId, message, icon = '🔍') {
    const container = document.getElementById(containerId);
    if (container) {
        container.innerHTML = `
            <div class="no-results">
                <div class="no-results-icon">${icon}</div>
                <p>${escapeHtml(message)}</p>
            </div>
        `;
    }
}

// ============================================
// SEARCH READINESS MANAGER
// ============================================

/**
 * Manages search UI readiness state based on indexing status.
 * Disables search inputs during indexing/loading and enables when ready.
 */
class SearchReadinessManager {
    constructor(options = {}) {
        // Elements to enable/disable
        this.searchInputId = options.searchInputId || 'query';
        this.searchButtonId = options.searchButtonId || 'search-btn';
        this.resultsContainerId = options.resultsContainerId || 'results';
        
        // Additional elements to manage (array of IDs)
        this.additionalInputIds = options.additionalInputIds || [];
        
        // Callback when readiness changes
        this.onReadyChange = options.onReadyChange || (() => {});
        
        // Current state
        this.isReady = false;
        this.lastStatus = null;
    }
    
    /**
     * Check if a status indicates the index is ready for searching
     * @param {string} status - Indexing status string
     * @returns {boolean} True if ready to search
     */
    isStatusReady(status) {
        // Ready states: idle (fully ready) or completed
        // Also ready during reconciling since we have a loaded index
        // Also allow searching during indexing/discovering — the write lock is
        // only briefly held per batch, so searches will succeed between batches.
        // loading_index is excluded as it holds the write lock for the full duration.
        const readyStates = ['idle', 'completed', 'reconciling', 'resolving_imports', 'indexing', 'discovering'];
        return readyStates.includes(status);
    }
    
    /**
     * Get user-friendly message for current status
     * @param {object} statusObj - Status object from WebSocket
     * @returns {string} User-friendly message
     */
    getStatusMessage(statusObj) {
        const status = statusObj.status;
        const message = statusObj.message;
        
        switch (status) {
            case 'loading_index':
                return message || 'Loading search index...';
            case 'discovering':
                return message || 'Discovering files...';
            case 'indexing':
                const pct = statusObj.progress_percent || 0;
                return message || `Indexing files (${pct}%)...`;
            case 'reconciling':
                return 'Updating index...';
            case 'resolving_imports':
                return 'Resolving imports...';
            default:
                return '';
        }
    }
    
    /**
     * Update readiness state based on status
     * @param {object} statusObj - Status object from WebSocket/API
     */
    update(statusObj) {
        const status = statusObj.status || 'idle';
        const wasReady = this.isReady;
        this.isReady = this.isStatusReady(status);
        this.lastStatus = statusObj;
        
        // Update UI elements
        this.updateUI();
        
        // Notify if readiness changed
        if (wasReady !== this.isReady) {
            this.onReadyChange(this.isReady, statusObj);
        }
    }
    
    /**
     * Update UI elements based on current readiness
     */
    updateUI() {
        const searchInput = document.getElementById(this.searchInputId);
        const searchButton = document.getElementById(this.searchButtonId);
        const resultsContainer = document.getElementById(this.resultsContainerId);
        
        if (searchInput) {
            searchInput.disabled = !this.isReady;
            searchInput.classList.toggle('search-disabled', !this.isReady);
            
            if (!this.isReady && this.lastStatus) {
                searchInput.placeholder = this.getStatusMessage(this.lastStatus);
            } else {
                // Restore default placeholder
                searchInput.placeholder = searchInput.dataset.defaultPlaceholder || 'Search code...';
            }
        }
        
        if (searchButton) {
            searchButton.disabled = !this.isReady;
            searchButton.classList.toggle('search-disabled', !this.isReady);
        }
        
        // Update additional inputs
        this.additionalInputIds.forEach(id => {
            const el = document.getElementById(id);
            if (el) {
                el.disabled = !this.isReady;
                el.classList.toggle('search-disabled', !this.isReady);
            }
        });
        
        // Show loading message in results if not ready and results is empty
        if (!this.isReady && resultsContainer && this.lastStatus) {
            const hasContent = resultsContainer.querySelector('.result-item, .no-results');
            if (!hasContent) {
                const msg = this.getStatusMessage(this.lastStatus);
                const pct = this.lastStatus.progress_percent || 0;
                resultsContainer.innerHTML = `
                    <div class="loading-index-state">
                        <div class="loading-spinner"></div>
                        <p class="loading-message">${escapeHtml(msg)}</p>
                        <div class="loading-progress">
                            <div class="loading-progress-bar" style="width: ${pct}%"></div>
                        </div>
                    </div>
                `;
            }
        }
        
        // Clear loading state when ready - restore empty state prompt
        if (this.isReady && resultsContainer) {
            const loadingState = resultsContainer.querySelector('.loading-index-state');
            if (loadingState) {
                resultsContainer.innerHTML = `
                    <div class="empty-state">
                        <p>Enter a search query to find code</p>
                    </div>
                `;
            }
        }
    }
    
    /**
     * Store default placeholder for restoration
     */
    storeDefaultPlaceholder() {
        const searchInput = document.getElementById(this.searchInputId);
        if (searchInput && !searchInput.dataset.defaultPlaceholder) {
            searchInput.dataset.defaultPlaceholder = searchInput.placeholder;
        }
    }
}

/**
 * Create a result summary header
 * @param {number} count - Number of results
 * @param {number} latencyMs - Search latency in ms
 * @returns {string} HTML string
 */
function createResultsSummary(count, latencyMs) {
    return `
        <div style="margin-bottom: 0.5rem; color: var(--text-secondary); text-align: center; font-size: 0.85rem;">
            Found <strong style="color: var(--accent);">${count}</strong> results 
            in <strong style="color: var(--success);">${latencyMs}ms</strong>
        </div>
    `;
}

/**
 * Format code with line numbers
 * @param {string} content - Code content
 * @param {number} startLine - Starting line number
 * @returns {string} HTML string with line numbers
 */
function formatCodeWithLineNumbers(content, startLine = 1) {
    const lines = content.split('\n');
    return lines.map((line, i) => {
        const lineNum = startLine + i;
        return `<span class="line-number">${lineNum}</span>${escapeHtml(line)}`;
    }).join('\n');
}

/**
 * Highlight matching text in content
 * @param {string} content - Content to search in
 * @param {string} query - Query to highlight
 * @returns {string} HTML with highlighted matches
 */
function highlightMatches(content, query) {
    if (!query) return escapeHtml(content);
    
    const escaped = escapeHtml(content);
    const queryEscaped = escapeHtml(query);
    const regex = new RegExp(`(${queryEscaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
    return escaped.replace(regex, '<span class="highlight">$1</span>');
}

// ============================================
// PROGRESS WEBSOCKET (for real-time indexing progress)
// ============================================

class ProgressWebSocket {
    constructor(options = {}) {
        this.wsUrl = options.wsUrl || `ws://${location.host}/ws/progress`;
        this.onUpdate = options.onUpdate || (() => {});
        this.onError = options.onError || console.error;
        this.onConnected = options.onConnected || (() => {});
        this.onDisconnected = options.onDisconnected || (() => {});
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        this.shouldReconnect = true;
    }

    connect() {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            return; // Already connected
        }

        try {
            this.ws = new WebSocket(this.wsUrl);

            this.ws.onopen = () => {
                this.reconnectAttempts = 0;
                this.onConnected();
            };

            this.ws.onmessage = (event) => {
                try {
                    const status = JSON.parse(event.data);
                    this.onUpdate(status);
                } catch (e) {
                    this.onError(new Error('Failed to parse progress message'));
                }
            };

            this.ws.onclose = (event) => {
                this.onDisconnected();
                if (this.shouldReconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
                    this.scheduleReconnect();
                }
            };

            this.ws.onerror = (error) => {
                this.onError(error);
            };

        } catch (error) {
            this.onError(error);
            if (this.shouldReconnect) {
                this.scheduleReconnect();
            }
        }
    }

    scheduleReconnect() {
        this.reconnectAttempts++;
        const delay = Math.min(this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1), 30000);
        setTimeout(() => this.connect(), delay);
    }

    start() {
        this.shouldReconnect = true;
        this.connect();
    }

    stop() {
        this.shouldReconnect = false;
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
}

// Legacy StatusPoller for fallback (if WebSocket not available)
class StatusPoller {
    constructor(options = {}) {
        this.apiBase = options.apiBase || '';
        this.fastIntervalMs = options.fastIntervalMs || 1000;
        this.slowIntervalMs = options.slowIntervalMs || 30000;
        this.onUpdate = options.onUpdate || (() => {});
        this.onError = options.onError || console.error;
        this.intervalId = null;
        this.currentInterval = null;
    }

    async fetch() {
        try {
            const response = await fetch(`${this.apiBase}/api/status`);
            if (!response.ok) throw new Error('Failed to fetch status');
            const status = await response.json();
            this.onUpdate(status);
            this.adjustPolling(status.is_indexing);
        } catch (error) {
            this.onError(error);
        }
    }

    adjustPolling(isIndexing) {
        const targetInterval = isIndexing ? this.fastIntervalMs : this.slowIntervalMs;
        if (this.currentInterval === targetInterval) return;

        if (this.intervalId) {
            clearInterval(this.intervalId);
        }

        this.intervalId = setInterval(() => this.fetch(), targetInterval);
        this.currentInterval = targetInterval;
    }

    start() {
        this.fetch();
    }

    stop() {
        if (this.intervalId) {
            clearInterval(this.intervalId);
            this.intervalId = null;
            this.currentInterval = null;
        }
    }
}

// Export for module usage (if needed in future)
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        escapeHtml,
        formatBytes,
        formatNumber,
        formatElapsed,
        debounce,
        updateStat,
        toggleElement,
        showLoading,
        showError,
        showEmpty,
        createResultsSummary,
        formatCodeWithLineNumbers,
        highlightMatches,
        ProgressWebSocket,
        StatusPoller,
        SearchReadinessManager
    };
}
