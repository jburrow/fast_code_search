/* fast_code_search — GitHub Pages site script */

// ── Theme toggle ──────────────────────────────────────────────────────────
(function () {
  const html = document.documentElement;
  const btn = document.getElementById('themeToggle');
  const icon = btn && btn.querySelector('.theme-icon');
  const STORAGE_KEY = 'fcs-theme';

  function apply(theme) {
    html.setAttribute('data-theme', theme);
    if (icon) icon.textContent = theme === 'dark' ? '🌙' : '☀️';
    try { localStorage.setItem(STORAGE_KEY, theme); } catch (_) { /* private mode */ }
  }

  // Restore saved preference, or follow OS
  const saved = (() => { try { return localStorage.getItem(STORAGE_KEY); } catch (_) { return null; } })();
  const preferred = saved || (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');
  apply(preferred);

  if (btn) {
    btn.addEventListener('click', () => {
      apply(html.getAttribute('data-theme') === 'dark' ? 'light' : 'dark');
    });
  }
})();

// ── Version injection ─────────────────────────────────────────────────────
// The deploy workflow writes version.json next to index.html.
// Fall back gracefully if it doesn't exist (local preview).
(function () {
  fetch('version.json')
    .then(r => r.ok ? r.json() : null)
    .then(data => {
      if (!data) return;
      const els = document.querySelectorAll('#version, #footerVersion');
      els.forEach(el => { el.textContent = data.version || el.textContent; });
    })
    .catch(() => { /* offline / local preview */ });
})();

// ── Benchmark iframe fallback ─────────────────────────────────────────────
// If the benchmark page hasn't been generated yet (fresh gh-pages), show
// a friendly message instead of a broken iframe.
(function () {
  const iframe = document.getElementById('benchChart');
  const fallback = document.getElementById('benchFallback');
  if (!iframe || !fallback) return;

  // On error (404 for the src URL) show fallback
  iframe.addEventListener('error', () => {
    iframe.style.display = 'none';
    fallback.style.display = 'block';
  });

  // Also listen for load; if the page returned is empty/404 HTML, hide iframe
  iframe.addEventListener('load', () => {
    try {
      // Same-origin only — will throw for cross-origin files
      const doc = iframe.contentDocument || iframe.contentWindow.document;
      if (!doc || doc.title.includes('404') || doc.body.textContent.trim() === '') {
        iframe.style.display = 'none';
        fallback.style.display = 'block';
      }
    } catch (_) {
      // Cross-origin: assume it loaded fine
    }
  });
})();

// ── Dynamic changelog from changelog.json ────────────────────────────────
// The deploy workflow writes changelog.json from CHANGELOG.md.
// If present, replace the static fallback entries.
(function () {
  fetch('changelog.json')
    .then(r => r.ok ? r.json() : null)
    .then(data => {
      if (!data || !Array.isArray(data.releases) || !data.releases.length) return;
      const list = document.getElementById('changelogList');
      if (!list) return;

      list.innerHTML = data.releases.map(r => `
        <div class="release-entry">
          <div class="release-header">
            <span class="release-version">v${esc(r.version)}</span>
            <span class="release-date">${esc(r.date)}</span>
            <a class="release-link" href="${esc(r.url)}" target="_blank" rel="noopener">Release notes →</a>
          </div>
          <p>${esc(r.summary)}</p>
        </div>
      `).join('');
    })
    .catch(() => { /* offline / local preview — static entries remain */ });

  function esc(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }
})();

// ── Smooth active-nav highlighting ───────────────────────────────────────
(function () {
  const sections = Array.from(document.querySelectorAll('section[id], header[id]'));
  const links = Array.from(document.querySelectorAll('.nav-links a[href^="#"]'));
  if (!sections.length || !links.length) return;

  const observer = new IntersectionObserver(entries => {
    entries.forEach(entry => {
      if (entry.isIntersecting) {
        links.forEach(a => a.classList.remove('active'));
        const active = links.find(a => a.getAttribute('href') === '#' + entry.target.id);
        if (active) active.classList.add('active');
      }
    });
  }, { rootMargin: '-30% 0px -60% 0px' });

  sections.forEach(s => observer.observe(s));
})();
