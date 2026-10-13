'use strict';
// Every colour lives in static/app.css as a design token, so a theme only has
// to redefine that block. This test fails when a page or script hard-codes a
// colour instead of using var(--token).
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const STATIC = path.join(__dirname, '..');
// Generated or vendored files, and the token file itself.
const SKIP = new Set(['app.css', 'tailwind.css', 'fonts.css']);
const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const RGB = /\brgba?\(\s*\d/g;

function sources() {
    const files = [];
    for (const dir of ['', 'lib']) {
        for (const name of fs.readdirSync(path.join(STATIC, dir))) {
            if (!/\.(html|js|css)$/.test(name) || /\.min\./.test(name) || SKIP.has(name)) continue;
            files.push(path.join(dir, name));
        }
    }
    return files;
}

test('no hard-coded colours outside app.css', () => {
    const found = [];
    for (const file of sources()) {
        const lines = fs.readFileSync(path.join(STATIC, file), 'utf8').split('\n');
        lines.forEach((line, i) => {
            // `&#123;` entities and `href="#id"` anchors are not colours.
            const text = line.replace(/&#x?[0-9a-fA-F]+;/g, '').replace(/(href|use href)="#[^"]*"/g, '');
            for (const m of [...(text.match(HEX) || []), ...(text.match(RGB) || [])]) {
                found.push(`${file}:${i + 1}: ${m}`);
            }
        });
    }
    assert.deepEqual(found, [], 'use a var(--token) from static/app.css instead');
});

test('labels get their case and font from the theme', () => {
    const found = [];
    for (const file of sources()) {
        const lines = fs.readFileSync(path.join(STATIC, file), 'utf8').split('\n');
        lines.forEach((line, i) => {
            if (/text-transform:\s*uppercase/.test(line) || /class="[^"]*\buppercase\b/.test(line)) {
                found.push(`${file}:${i + 1}: use .label-case or text-transform: var(--label-case)`);
            }
            if (/'JetBrains Mono'|"Space Grotesk"|'Space Grotesk'/.test(line)) {
                found.push(`${file}:${i + 1}: use var(--font-mono), var(--font-label) or var(--font-display)`);
            }
        });
    }
    assert.deepEqual(found, []);
});

// A theme block must set every colour itself: a preview drawn in one theme
// inside a page in another would otherwise inherit the page's colours.
test('every theme sets every colour token', () => {
    const css = fs.readFileSync(path.join(STATIC, 'app.css'), 'utf8');
    const block = (selector) => {
        const start = css.indexOf(selector + ' {');
        assert.ok(start >= 0, `no ${selector} block in app.css`);
        return css.slice(start, css.indexOf('\n}', start));
    };
    const names = (text) => new Set([...text.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((m) => m[1]));
    // Shared by every theme: fonts, language badge colours.
    const shared = (n) => /^--(font-(ui|mono)|lang-)/.test(n);
    const bold = [...names(block(':root, [data-theme="bold"]'))].filter((n) => !shared(n));
    const themeIds = [...css.matchAll(/^\[data-theme="([a-z-]+)"\] \{/gm)].map((m) => m[1]);
    assert.deepEqual(themeIds, ['corporate', 'corporate-dark']);
    for (const id of themeIds) {
        const own = names(block(`[data-theme="${id}"]`));
        const missing = bold.filter((n) => !own.has(n));
        assert.deepEqual(missing, [], id);
    }
});

test('theme.js, app.css and the server agree on the theme names', () => {
    const js = fs.readFileSync(path.join(STATIC, 'theme.js'), 'utf8');
    const ids = [...js.matchAll(/\{ id: '([a-z-]+)'/g)].map((m) => m[1]);
    assert.deepEqual(ids, ['bold', 'corporate', 'corporate-dark']);
    const rust = fs.readFileSync(path.join(STATIC, '..', 'src', 'config.rs'), 'utf8');
    const variants = rust.slice(rust.indexOf('pub enum UiTheme'), rust.indexOf('}', rust.indexOf('pub enum UiTheme')));
    const kebab = [...variants.matchAll(/^\s+([A-Z][A-Za-z]+),/gm)].map((m) => m[1].replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase());
    assert.deepEqual(kebab, ids);
});

test('every var(--token) a page uses is defined', () => {
    const defined = new Set();
    for (const file of ['app.css', ...sources()]) {
        const css = fs.readFileSync(path.join(STATIC, file), 'utf8');
        for (const m of css.matchAll(/(--[a-z0-9-]+)\s*:/g)) defined.add(m[1]);
    }
    const missing = [];
    for (const file of sources()) {
        const text = fs.readFileSync(path.join(STATIC, file), 'utf8');
        // `var(--lang-${ext})` is built at run time; the --lang-* family is in app.css.
        for (const m of text.matchAll(/var\((--[a-z0-9-]+)(?![a-z0-9-]*\$)/g)) {
            if (!defined.has(m[1]) && !m[1].endsWith('-')) missing.push(`${file}: ${m[1]}`);
        }
    }
    assert.deepEqual([...new Set(missing)], []);
});
