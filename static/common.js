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
        this.isReady = true; // optimistic: start enabled, disable only on confirmed failure
        this.lastStatus = null;
        this.isOffline = false;

        // Optional: ID of a containing section to hide entirely when offline
        // (cleaner than just greying out inputs)
        this.searchSectionId = options.searchSectionId || null;

        this.storeDefaultPlaceholder();
    }

    /**
     * Mark the search engine server as offline or online.
     * When offline, all inputs are disabled and a prominent banner is shown.
     * When coming back online, the banner is removed and normal readiness flow resumes.
     * @param {boolean} isOffline
     */
    setOffline(isOffline) {
        if (this.isOffline === isOffline) return;
        this.isOffline = isOffline;
        if (isOffline) {
            this.isReady = false;
        }
        this.updateUI();
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
            case 'reconciling': {
                const pct = statusObj.progress_percent || 0;
                return message || `Updating index (${pct}%)...`;
            }
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

        // --- Offline state: server is not reachable ---
        if (this.isOffline) {
            // Hide the search section entirely (cleaner than greyed-out inputs)
            if (this.searchSectionId) {
                const section = document.getElementById(this.searchSectionId);
                if (section) section.style.display = 'none';
            }
            if (searchInput) {
                searchInput.disabled = true;
                searchInput.classList.add('search-disabled');
                searchInput.placeholder = 'Server not running';
            }
            if (searchButton) {
                searchButton.disabled = true;
                searchButton.classList.add('search-disabled');
            }
            this.additionalInputIds.forEach(id => {
                const el = document.getElementById(id);
                if (el) { el.disabled = true; el.classList.add('search-disabled'); }
            });
            if (resultsContainer && !resultsContainer.querySelector('.server-offline-state')) {
                resultsContainer.innerHTML = `
                    <div class="server-offline-state" style="
                        padding: 2.5rem 1.5rem;
                        text-align: center;
                        font-family: 'JetBrains Mono', monospace;
                        background: #fff;
                        border: 1px solid #000;
                        box-shadow: 2px 2px 0 #000;
                    ">
                        <div style="font-size: 2rem; margin-bottom: 0.75rem; opacity: 0.45; line-height: 1;">&#9888;</div>
                        <div style="font-weight: 700; font-size: 0.9rem; color: #1d1c0f; margin-bottom: 0.5rem; text-transform: uppercase; letter-spacing: 0.07em;">Server Not Running</div>
                        <div style="color: #7a785f; font-size: 0.76rem; max-width: 340px; margin: 0 auto; line-height: 1.65;">
                            The search server could not be reached.<br>
                            Start the server and this page will reconnect automatically.
                        </div>
                    </div>
                `;
            }
            return;
        }

        // Coming back online — restore section visibility and clear offline banner
        if (this.searchSectionId) {
            const section = document.getElementById(this.searchSectionId);
            if (section) section.style.display = '';
        }
        if (resultsContainer) {
            const offlineBanner = resultsContainer.querySelector('.server-offline-state');
            if (offlineBanner) resultsContainer.innerHTML = '';
        }

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
        // Called when consecutive reconnect failures exceed offlineThreshold
        this.onServerOffline = options.onServerOffline || (() => {});
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        this.shouldReconnect = true;
        // Number of failed reconnect attempts before calling onServerOffline
        this.offlineThreshold = options.offlineThreshold ?? 3;
        this._reportedOffline = false;
    }

    connect() {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            return; // Already connected
        }

        try {
            this.ws = new WebSocket(this.wsUrl);

            this.ws.onopen = () => {
                this.reconnectAttempts = 0;
                // If we previously reported the server as offline, clear that state
                if (this._reportedOffline) {
                    this._reportedOffline = false;
                }
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
        // After enough consecutive failures, notify that the server appears offline
        if (!this._reportedOffline && this.reconnectAttempts >= this.offlineThreshold) {
            this._reportedOffline = true;
            this.onServerOffline();
        }
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

// ============================================
// URL STATE HELPERS
// ============================================

/**
 * Load search state from URL query parameters into form fields.
 *
 * @param {Array<{param: string, setter: Function, defaultValue?: *}>} fields
 *   Each field descriptor:
 *     - param: URL parameter name
 *     - setter: function(value: string) that applies the value to the UI
 *     - defaultValue: (optional) default value; used to determine whether a field is
 *       non-default for auto-expand logic
 * @param {Function} [onNonDefault] - called once if any non-default value was loaded
 *   (e.g., to expand an "Advanced Options" panel)
 * @returns {boolean} true if at least one non-default value was loaded from the URL
 */
function loadStateFromUrl(fields, onNonDefault) {
    const params = new URLSearchParams(window.location.search);
    let hasNonDefault = false;

    fields.forEach(({ param, setter, defaultValue }) => {
        if (params.has(param)) {
            const value = params.get(param);
            setter(value);
            // A value is "non-default" when no defaultValue was specified, or when the
            // URL value differs from the default (e.g. '50' vs '10').  An explicit empty
            // string in the URL (?q=) still matches defaultValue='' and is therefore
            // treated as default, so it never triggers the onNonDefault callback.
            if (defaultValue === undefined || String(value) !== String(defaultValue)) {
                hasNonDefault = true;
            }
        }
    });

    if (hasNonDefault && typeof onNonDefault === 'function') {
        onNonDefault();
    }

    return hasNonDefault;
}

/**
 * Sync current form state to the URL query string via history.replaceState.
 * Parameters whose current value equals their default are omitted to keep URLs short.
 *
 * @param {Array<{param: string, getter: Function, defaultValue?: *}>} fields
 *   Each field descriptor:
 *     - param: URL parameter name
 *     - getter: function() that returns the current value as a string
 *     - defaultValue: (optional) default value; omitted from URL when the current value matches
 */
function syncUrlFromState(fields) {
    const params = new URLSearchParams();

    fields.forEach(({ param, getter, defaultValue }) => {
        const value = getter();
        // Skip empty values unconditionally — we never write empty strings to the URL.
        // Also skip when the value equals the declared default to keep URLs short.
        if (value !== undefined && value !== null && value !== '' &&
                (defaultValue === undefined || String(value) !== String(defaultValue))) {
            params.set(param, String(value));
        }
    });

    const search = params.toString();
    const url = search ? `${window.location.pathname}?${search}` : window.location.pathname;
    history.replaceState(null, '', url);
}

/**
 * Map a file path's extension to a CSS class suffix for language badge colouring.
 * Used by both keyword and semantic search result rendering.
 * Returns null for unknown/generic extensions.
 */
function langClassForPath(filePath) {
    const ext = (filePath.split('.').pop() || '').toLowerCase();
    const MAP = {
        rs: 'rs', py: 'py', js: 'js', mjs: 'js', cjs: 'js', jsx: 'js',
        ts: 'ts', tsx: 'ts',
        go: 'go', rb: 'rb', java: 'java', cs: 'cs', cpp: 'cpp', cc: 'cpp',
        cxx: 'cpp', hpp: 'cpp', c: 'c', h: 'c', sh: 'sh', bash: 'sh',
        zsh: 'sh', toml: 'toml', yaml: 'yaml', yml: 'yaml', json: 'json',
        css: 'css', scss: 'css', sql: 'sql', md: 'md', proto: 'proto',
    };
    return MAP[ext] || null;
}

// ============================================
// SHARED SEARCH HISTORY UTILITIES
// ============================================

/**
 * Load search history from a specific storage key.
 * @param {string} storageKey - localStorage key (e.g., 'fcs_history')
 * @returns {Array<string>} History array, most-recent first
 */
function loadSearchHistory(storageKey) {
    try {
        const raw = localStorage.getItem(storageKey);
        return raw ? JSON.parse(raw) : [];
    } catch (_) { return []; }
}

/**
 * Save a query to search history.
 * @param {string} storageKey - localStorage key
 * @param {string} query - Query to save
 * @param {number} maxSize - Max history size (default: 50)
 */
function saveSearchHistory(storageKey, query, maxSize = 50) {
    if (!query || query.length < 2) return;
    try {
        let history = loadSearchHistory(storageKey).filter(q => q !== query);
        history.unshift(query);
        if (history.length > maxSize) history = history.slice(0, maxSize);
        localStorage.setItem(storageKey, JSON.stringify(history));
    } catch (_) { /* storage unavailable */ }
}

/**
 * Clear all history for a storage key.
 * @param {string} storageKey - localStorage key
 */
function clearSearchHistory(storageKey) {
    try { localStorage.removeItem(storageKey); } catch (_) { /* ignore */ }
}

/**
 * Render and show history dropdown with callbacks.
 * @param {HTMLElement} dropdownEl - Dropdown to populate
 * @param {HTMLInputElement} queryInput - Query input field
 * @param {string} storageKey - localStorage key
 * @param {Function} onSelectQuery - Callback: (query) => void
 * @param {Function} onDelete - Callback: (query) => void (optional)
 */
function showSearchHistoryDropdown(dropdownEl, queryInput, storageKey, onSelectQuery, onDelete = null) {
    if (!dropdownEl) return;
    const filter = queryInput?.value?.trim() || '';
    const all = loadSearchHistory(storageKey);
    const matches = filter ? all.filter(q => q.toLowerCase().includes(filter.toLowerCase())) : all;
    if (matches.length === 0) { dropdownEl.style.display = 'none'; return; }

    dropdownEl.innerHTML = matches.slice(0, 10).map((q, i) =>
        `<div class="history-item flex items-center gap-2 px-4 py-2 cursor-pointer hover:bg-primary-container font-label text-xs text-on-surface" data-idx="${i}" data-query="${escapeHtml(q)}">
            <span class="material-symbols-outlined" style="font-size:14px;color:#7a785f;flex-shrink:0">history</span>
            <span class="flex-1 truncate">${escapeHtml(q)}</span>
            <button class="history-delete material-symbols-outlined ml-auto flex-shrink-0" style="font-size:14px;color:#7a785f;background:none;border:none;cursor:pointer;padding:0" data-query="${escapeHtml(q)}" title="Remove">close</button>
        </div>`
    ).join('') + `<div class="flex items-center justify-end px-4 py-1.5 border-t border-outline-variant">
        <button id="clear-history-btn" class="font-label text-[10px] text-outline hover:text-black transition-colors">CLEAR ALL HISTORY</button>
    </div>`;
    dropdownEl.style.display = 'block';

    dropdownEl.querySelectorAll('.history-item').forEach(item => {
        item.addEventListener('mousedown', (e) => {
            if (e.target.classList.contains('history-delete')) return;
            e.preventDefault();
            dropdownEl.style.display = 'none';
            if (onSelectQuery) onSelectQuery(item.dataset.query);
        });
    });

    dropdownEl.querySelectorAll('.history-delete').forEach(btn => {
        btn.addEventListener('mousedown', (e) => {
            e.preventDefault();
            e.stopPropagation();
            const q = btn.dataset.query;
            try { let h = loadSearchHistory(storageKey).filter(x => x !== q); localStorage.setItem(storageKey, JSON.stringify(h)); } catch (_) {}
            if (onDelete) onDelete(q);
            showSearchHistoryDropdown(dropdownEl, queryInput, storageKey, onSelectQuery, onDelete);
        });
    });

    const clearBtn = document.getElementById('clear-history-btn');
    if (clearBtn) clearBtn.addEventListener('mousedown', (e) => { e.preventDefault(); clearSearchHistory(storageKey); dropdownEl.style.display = 'none'; });
}

/**
 * Hide history dropdown.
 * @param {HTMLElement} dropdownEl - Dropdown to hide
 */
function hideSearchHistoryDropdown(dropdownEl) {
    if (dropdownEl) dropdownEl.style.display = 'none';
}

/**
 * Navigate history dropdown with arrow keys.
 * @param {HTMLElement} dropdownEl - Dropdown element
 * @param {HTMLInputElement} queryInput - Query input to update
 * @param {number} direction - +1 for down, -1 for up
 * @returns {boolean} True if handled
 */
function navigateSearchHistoryDropdown(dropdownEl, queryInput, direction) {
    if (!dropdownEl || dropdownEl.style.display === 'none') return false;
    const items = Array.from(dropdownEl.querySelectorAll('.history-item'));
    if (items.length === 0) return false;
    let focusedIdx = parseInt(dropdownEl.dataset.focusedIdx || '-1', 10);
    focusedIdx = Math.max(-1, Math.min(items.length - 1, focusedIdx + direction));
    dropdownEl.dataset.focusedIdx = focusedIdx;
    items.forEach((el, i) => el.classList.toggle('bg-primary-container', i === focusedIdx));
    if (focusedIdx >= 0 && queryInput) queryInput.value = items[focusedIdx].dataset.query;
    return true;
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
        SearchReadinessManager,
        loadStateFromUrl,
        syncUrlFromState,
        langClassForPath,
    };
}
