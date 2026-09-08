// Render ```mermaid fences. mdBook emits them as <pre><code class="language-mermaid">;
// mermaid wants <pre class="mermaid"> holding the raw source. The library is
// loaded only on pages that have a diagram.
(function () {
  const blocks = document.querySelectorAll('pre > code.language-mermaid');
  if (!blocks.length) return;
  blocks.forEach(code => {
    const pre = code.parentElement;
    const el = document.createElement('pre');
    el.className = 'mermaid';
    el.textContent = code.textContent;
    pre.replaceWith(el);
  });
  const dark = document.documentElement.classList.contains('ayu') ||
    document.documentElement.classList.contains('navy') ||
    document.documentElement.classList.contains('coal');
  const s = document.createElement('script');
  s.src = 'https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js';
  // Measure text only once the web fonts are in, or labels are laid out for
  // the fallback font and clipped when the real one arrives.
  const font = '"Open Sans", "Helvetica Neue", Arial, sans-serif';
  s.onload = () => {
    mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'neutral', securityLevel: 'loose',
      flowchart: { htmlLabels: true, padding: 8, nodeSpacing: 30, rankSpacing: 36 },
      themeVariables: { fontFamily: font, fontSize: '14px' } });
    (document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve())
      .then(() => mermaid.run({ querySelector: 'pre.mermaid' }));
  };
  document.head.appendChild(s);
})();
