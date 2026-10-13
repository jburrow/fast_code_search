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
const SKIP = new Set(['app.css', 'tailwind.css', 'fonts.css', 'highlight-light.min.css']);
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
