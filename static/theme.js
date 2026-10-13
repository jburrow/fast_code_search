// ============================================
// THEMES: apply the saved theme before paint, the header menu, and the
// first-run picker
// ============================================
// Every page loads, in <head> and before any stylesheet:
//
//   <script src="/ui-config.js"></script>   (server: default theme, picker on/off)
//   <script src="/theme.js"></script>
//
// so <html data-theme> is set before the page paints and never flashes the
// wrong colours. The colours themselves are the token blocks in app.css.
//
// The person's choice is kept in this browser (localStorage). With no choice
// saved, the page follows the server's `default_theme`. The first visit shows
// a picker unless the server sets `theme_picker = false`; dismissing it keeps
// the server default and the picker is not shown again.
(function () {
    'use strict';

    const THEMES = [
        { id: 'bold', name: 'Bold', note: 'Yellow and black, square corners, capitals.' },
        { id: 'corporate', name: 'Corporate', note: 'White and grey, one blue accent, rounded.' },
        { id: 'corporate-dark', name: 'Corporate dark', note: 'Corporate on dark surfaces.' },
    ];
    const THEME_KEY = 'fcs-theme';
    const ASKED_KEY = 'fcs-theme-asked';
    const root = document.documentElement;
    const config = window.FCS_UI || {};
    const isTheme = (id) => THEMES.some((t) => t.id === id);
    const serverDefault = isTheme(config.default_theme) ? config.default_theme : 'bold';

    function read(key) {
        try { return localStorage.getItem(key); } catch (_) { return null; }
    }
    function write(key, value) {
        try { localStorage.setItem(key, value); } catch (_) { /* private mode: this page only */ }
    }

    function saved() {
        const id = read(THEME_KEY);
        return isTheme(id) ? id : null;
    }
    function current() {
        return root.dataset.theme || serverDefault;
    }
    function apply(id) {
        root.dataset.theme = isTheme(id) ? id : serverDefault;
    }
    function choose(id) {
        write(THEME_KEY, id);
        write(ASKED_KEY, '1');
        apply(id);
    }

    apply(saved() || serverDefault);

    // ---------- header menu ----------
    let menu = null;
    function closeMenu() {
        if (!menu) return;
        const btn = menu._anchor;
        menu.remove();
        menu = null;
        document.removeEventListener('mousedown', onOutside, true);
        document.removeEventListener('keydown', onMenuKey, true);
        if (btn) btn.setAttribute('aria-expanded', 'false');
    }
    function onOutside(e) {
        if (menu && !menu.contains(e.target) && e.target !== menu._anchor && !menu._anchor.contains(e.target)) closeMenu();
    }
    function onMenuKey(e) {
        if (!menu) return;
        const items = [...menu.querySelectorAll('button')];
        const i = items.indexOf(document.activeElement);
        if (e.key === 'Escape') { e.preventDefault(); const b = menu._anchor; closeMenu(); b.focus(); }
        else if (e.key === 'ArrowDown') { e.preventDefault(); items[(i + 1) % items.length].focus(); }
        else if (e.key === 'ArrowUp') { e.preventDefault(); items[(i - 1 + items.length) % items.length].focus(); }
    }
    function swatch(id) {
        return `<span class="theme-swatch" data-theme="${id}" style="background:var(--accent);box-shadow:inset 0 0 0 4px var(--page)"></span>`;
    }
    function openMenu(anchor) {
        if (menu) { closeMenu(); return; }
        menu = document.createElement('div');
        menu.className = 'theme-menu';
        menu.setAttribute('role', 'menu');
        menu.setAttribute('aria-label', 'Theme');
        menu._anchor = anchor;
        const active = current();
        menu.innerHTML = THEMES.map((t) =>
            `<button type="button" role="menuitemradio" aria-checked="${t.id === active}" data-theme-id="${t.id}">${swatch(t.id)}${t.name}</button>`
        ).join('');
        menu.addEventListener('click', (e) => {
            const b = e.target.closest('button[data-theme-id]');
            if (!b) return;
            choose(b.dataset.themeId);
            closeMenu();
            anchor.focus();
        });
        document.body.appendChild(menu);
        const r = anchor.getBoundingClientRect();
        menu.style.top = `${r.bottom + window.scrollY + 4}px`;
        menu.style.left = `${Math.max(8, r.right + window.scrollX - menu.offsetWidth)}px`;
        anchor.setAttribute('aria-expanded', 'true');
        document.addEventListener('mousedown', onOutside, true);
        document.addEventListener('keydown', onMenuKey, true);
        (menu.querySelector('[aria-checked="true"]') || menu.querySelector('button')).focus();
    }

    // ---------- first-run picker ----------
    function preview(id) {
        return `<div class="theme-preview" data-theme="${id}" aria-hidden="true">` +
            `<div class="pv-row"><span class="pv-input">render</span><span class="pv-btn">Search</span></div>` +
            `<div class="pv-card"><div class="pv-head">src/cli/format.rs</div>` +
            `<div class="pv-code"><span class="k">pub fn</span> <mark>render</mark>(</div></div>` +
            `</div>`;
    }
    function openPicker() {
        if (document.querySelector('.theme-picker')) return;
        const before = current();
        let picked = saved() ||
            (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'corporate-dark' : serverDefault);
        apply(picked);

        const overlay = document.createElement('div');
        overlay.className = 'theme-picker';
        overlay.innerHTML =
            `<div class="theme-picker-dialog" role="dialog" aria-modal="true" aria-labelledby="theme-picker-title">` +
                `<h2 id="theme-picker-title">Choose a theme</h2>` +
                `<p>Pick how Fast Code Search looks in this browser. You can change it later from the palette button in the header.</p>` +
                `<div class="theme-options">` +
                    THEMES.map((t) =>
                        `<button type="button" class="theme-option" data-theme-id="${t.id}" aria-pressed="${t.id === picked}">` +
                            preview(t.id) +
                            `<span class="theme-option-name">${t.name}</span>` +
                            `<span class="theme-option-note">${t.note}</span>` +
                        `</button>`
                    ).join('') +
                `</div>` +
                `<div class="theme-picker-actions">` +
                    `<button type="button" class="theme-option-skip btn">Not now</button>` +
                    `<button type="button" class="theme-option-use btn btn-primary">Use this theme</button>` +
                `</div>` +
            `</div>`;

        const options = () => [...overlay.querySelectorAll('.theme-option')];
        function mark(id) {
            picked = id;
            options().forEach((o) => o.setAttribute('aria-pressed', String(o.dataset.themeId === id)));
            apply(id);
        }
        function close(keep) {
            if (keep) choose(picked);
            else { write(ASKED_KEY, '1'); apply(saved() || before); }
            overlay.remove();
            document.removeEventListener('keydown', onKey, true);
        }
        function onKey(e) {
            if (e.key === 'Escape') { e.preventDefault(); close(false); }
            else if (e.key === 'Enter' && e.target.classList && e.target.classList.contains('theme-option')) {
                e.preventDefault(); mark(e.target.dataset.themeId); close(true);
            }
        }
        overlay.addEventListener('mouseover', (e) => {
            const o = e.target.closest('.theme-option');
            if (o) apply(o.dataset.themeId);
        });
        overlay.addEventListener('mouseleave', () => apply(picked));
        overlay.addEventListener('focusin', (e) => {
            const o = e.target.closest('.theme-option');
            if (o) apply(o.dataset.themeId);
        });
        overlay.addEventListener('click', (e) => {
            const o = e.target.closest('.theme-option');
            if (o) { mark(o.dataset.themeId); return; }
            if (e.target.closest('.theme-option-use')) close(true);
            else if (e.target.closest('.theme-option-skip') || e.target === overlay) close(false);
        });
        document.addEventListener('keydown', onKey, true);
        document.body.appendChild(overlay);
        const first = overlay.querySelector('.theme-option[aria-pressed="true"]');
        if (first) first.focus();
    }

    window.fcsTheme = { themes: THEMES, current, choose, openMenu, openPicker };

    if (config.theme_picker !== false && !read(ASKED_KEY) && !saved()) {
        if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', openPicker);
        else openPicker();
    }
})();
