// ============================================
// APP SHELL: icon sprite, header and phone tab bar
// ============================================
// Every page includes this synchronously where its header goes:
//
//   <template id="app-header-extra">…page-specific header content…</template>
//   <script src="/shell.js" data-page="search"></script>
//
// It writes the shared header (wordmark, nav, help link) and the phone tab
// bar in place before the page paints, so all pages share one copy and
// cannot drift apart. `data-page` marks the current nav entry; the optional
// template's content goes between the nav and the help link (the search
// page's index stats and compact search box, the semantic page's stats).
(function () {
    'use strict';

    // Material Design icon paths (Apache-2.0); used as
    // <svg class="icon"><use href="#i-name"/></svg>.
    const SYMBOLS = [
    '<symbol id="i-search" viewBox="0 0 24 24"><path d="M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/></symbol>',
    '<symbol id="i-code" viewBox="0 0 24 24"><path d="M9.4 16.6 4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0 4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"/></symbol>',
    '<symbol id="i-function" viewBox="0 0 24 24"><path d="M18 4H6v2l6.5 6L6 18v2h12v-3h-7l5-5-5-5h7z"/></symbol>',
    '<symbol id="i-link" viewBox="0 0 24 24"><path d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z"/></symbol>',
    '<symbol id="i-tune" viewBox="0 0 24 24"><path d="M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z"/></symbol>',
    '<symbol id="i-content-copy" viewBox="0 0 24 24"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></symbol>',
    '<symbol id="i-open-in-new" viewBox="0 0 24 24"><path d="M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z"/></symbol>',
    '<symbol id="i-help-outline" viewBox="0 0 24 24"><path d="M11 18h2v-2h-2v2zm1-16C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm0-14c-2.21 0-4 1.79-4 4h2c0-1.1.9-2 2-2s2 .9 2 2c0 2-3 1.75-3 5h2c0-2.25 3-2.5 3-5 0-2.21-1.79-4-4-4z"/></symbol>',
    '<symbol id="i-analytics" viewBox="0 0 24 24"><path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14zM7 10h2v7H7zm4-3h2v10h-2zm4 6h2v4h-2z"/></symbol>',
    '<symbol id="i-account-tree" viewBox="0 0 24 24"><path d="M22 11V3h-7v3H9V3H2v8h7V8h2v10h4v3h7v-8h-7v3h-2V8h2v3z"/></symbol>',
    '<symbol id="i-description" viewBox="0 0 24 24"><path d="M8 16h8v2H8zm0-4h8v2H8zm6-10H6c-1.1 0-2 .9-2 2v16c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"/></symbol>',
    '<symbol id="i-auto-awesome" viewBox="0 0 24 24"><path d="m19 9 1.25-2.75L23 5l-2.75-1.25L19 1l-1.25 2.75L15 5l2.75 1.25L19 9zm-7.5.5L9 4 6.5 9.5 1 12l5.5 2.5L9 20l2.5-5.5L17 12l-5.5-2.5zM19 15l-1.25 2.75L15 19l2.75 1.25L19 23l1.25-2.75L23 19l-2.75-1.25L19 15z"/></symbol>',
    '<symbol id="i-warning" viewBox="0 0 24 24"><path d="M12 5.99 19.53 19H4.47L12 5.99M12 2 1 21h22L12 2zm1 14h-2v2h2v-2zm0-6h-2v4h2v-4z"/></symbol>',
    '<symbol id="i-check" viewBox="0 0 24 24"><path d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></symbol>',
    '<symbol id="i-error" viewBox="0 0 24 24"><path d="M11 15h2v2h-2v-2zm0-8h2v6h-2V7zm.99-5C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z"/></symbol>',
    '<symbol id="i-data-object" viewBox="0 0 24 24"><path d="M4 7v2c0 .55-.45 1-1 1H2v4h1c.55 0 1 .45 1 1v2c0 1.65 1.35 3 3 3h3v-2H7c-.55 0-1-.45-1-1v-2c0-1.3-.84-2.42-2-2.83v-.34C5.16 11.42 6 10.3 6 9V7c0-.55.45-1 1-1h3V4H7C5.35 4 4 5.35 4 7zm17 3c-.55 0-1-.45-1-1V7c0-1.65-1.35-3-3-3h-3v2h3c.55 0 1 .45 1 1v2c0 1.3.84 2.42 2 2.83v.34c-1.16.41-2 1.52-2 2.83v2c0 .55-.45 1-1 1h-3v2h3c1.65 0 3-1.35 3-3v-2c0-.55.45-1 1-1h1v-4h-1z"/></symbol>'
    ];

    const PAGES = [
        { id: 'search', href: '/', label: 'Search', icon: 'i-search' },
        { id: 'semantic', href: '/semantic.html', label: 'Semantic', icon: 'i-auto-awesome' },
        { id: 'graph', href: '/graph.html', label: 'Graph', icon: 'i-account-tree' },
        { id: 'index', href: '/diagnostics.html', label: 'Index', icon: 'i-analytics' },
        { id: 'docs', href: '/docs.html', label: 'Docs', icon: 'i-description' },
    ];

    const script = document.currentScript;
    const current = (script && script.dataset.page) || '';
    const icon = (id, size) =>
        `<svg class="icon" width="${size}" height="${size}" aria-hidden="true"><use href="#${id}"/></svg>`;
    const here = (p) => (p.id === current ? ' aria-current="page"' : '');

    const extra = document.getElementById('app-header-extra');
    const extraHtml = extra ? extra.innerHTML : '';

    const html =
        `<svg xmlns="http://www.w3.org/2000/svg" style="display:none" aria-hidden="true">${SYMBOLS.join('')}</svg>` +
        `<header class="app-header">` +
            `<div class="app-header-left">` +
                `<div class="app-wordmark">FAST_CODE_SEARCH</div>` +
                `<nav class="app-nav" aria-label="Main">` +
                    PAGES.map(p => `<a href="${p.href}"${here(p)}>${p.label}</a>`).join('') +
                `</nav>` +
            `</div>` +
            extraHtml +
            `<a href="/docs.html" class="app-help" title="Documentation" aria-label="Documentation">${icon('i-help-outline', 20)}</a>` +
        `</header>` +
        `<nav class="app-tabbar" aria-label="Main">` +
            PAGES.map(p => `<a href="${p.href}"${here(p)}>${icon(p.icon, 20)}<span>${p.label}</span></a>`).join('') +
        `</nav>`;

    if (script) script.insertAdjacentHTML('beforebegin', html);
    if (extra) extra.remove();
})();
