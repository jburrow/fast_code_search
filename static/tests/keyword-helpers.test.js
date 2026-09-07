'use strict';
// Node tests for the pure keyword-search helpers (`npm test`, built-in runner).
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const H = require(path.join(__dirname, '..', 'lib', 'keyword-helpers.js'));
// escapeHtml / formatBytes stay in common.js (semantic.html loads only that
// file); they are tested here from there.
const C = require(path.join(__dirname, '..', 'common.js'));

// ---------- byte offsets ----------

test('byteRangeToCharRange: ASCII is the identity', () => {
    assert.deepEqual(H.byteRangeToCharRange('hello world', 6, 11), [6, 11]);
    assert.deepEqual(H.byteRangeToCharRange('abc', 0, 0), [0, 0]);
});

test('byteRangeToCharRange: multibyte characters before the match shift it', () => {
    // 'é' is 2 bytes, 1 code unit; 'ü' likewise.
    const s = 'café needle';
    assert.deepEqual(H.byteRangeToCharRange(s, 6, 12), [5, 11]);
    assert.equal(s.slice(...H.byteRangeToCharRange(s, 6, 12)), 'needle');
    // '€' is 3 bytes.
    const e = '€€x';
    assert.deepEqual(H.byteRangeToCharRange(e, 6, 7), [2, 3]);
    // An emoji is 4 bytes and 2 UTF-16 code units.
    const emoji = '😀ab';
    assert.deepEqual(H.byteRangeToCharRange(emoji, 4, 6), [2, 4]);
    assert.deepEqual(H.byteRangeToCharRange(emoji, 0, 4), [0, 2]);
});

test('byteRangeToCharRange: out-of-range offsets clamp to the string', () => {
    assert.deepEqual(H.byteRangeToCharRange('abc', 1, 99), [1, 3]);
    assert.deepEqual(H.byteRangeToCharRange('abc', 99, 200), [3, 3]);
    // end before start never yields a negative range (it collapses to empty)
    const [s, e] = H.byteRangeToCharRange('abcdef', 4, 2);
    assert.equal(e, s);
    // an offset inside a multibyte sequence lands on the next character boundary
    assert.deepEqual(H.byteRangeToCharRange('é!', 1, 3), [1, 2]);
});

test('mergeRanges: sorts, drops empty ranges and merges overlaps', () => {
    assert.deepEqual(H.mergeRanges([[5, 8], [1, 3], [2, 4], [8, 8], [7, 10]]), [[1, 4], [5, 10]]);
    assert.deepEqual(H.mergeRanges([]), []);
    assert.deepEqual(H.mergeRanges([null, [3, 1]]), []);
});

// ---------- query tokenizer (parity with src/search/query_syntax.rs) ----------

test('parseQueryTerms: plain words and quoted phrases', () => {
    assert.deepEqual(H.parseQueryTerms('hello world').terms, ['hello', 'world']);
    assert.deepEqual(H.parseQueryTerms('"hello world"').terms, ['hello world']);
    assert.deepEqual(H.parseQueryTerms('  fn\tmain\n').terms, ['fn', 'main']);
    assert.deepEqual(H.parseQueryTerms(''), { terms: [], excludeTerms: [], caseSensitive: false, wholeWord: false });
    assert.deepEqual(H.parseQueryTerms(null).terms, []);
});

test('parseQueryTerms: operators are consumed, -term excludes', () => {
    const q = H.parseQueryTerms('needle lang:rust file:src/ -file:test case:yes word:y -haystack -lang:py');
    assert.deepEqual(q.terms, ['needle']);
    assert.deepEqual(q.excludeTerms, ['haystack']);
    assert.equal(q.caseSensitive, true);
    assert.equal(q.wholeWord, true);
    assert.deepEqual(H.parseQueryTerms('file:*.toml x').terms, ['x']);
    // an unknown but alphanumeric language name is still a filter
    assert.deepEqual(H.parseQueryTerms('lang:xyz x').terms, ['x']);
    assert.equal(H.parseQueryTerms('a case:no').caseSensitive, false);
    assert.equal(H.parseQueryTerms('a case:TRUE').caseSensitive, true);
    assert.equal(H.parseQueryTerms('a word:1').wholeWord, true);
});

test('parseQueryTerms: - negates only before a letter, _ or a quote', () => {
    assert.deepEqual(H.parseQueryTerms('-1 x').terms, ['-1', 'x']);
    assert.deepEqual(H.parseQueryTerms('-> fn').terms, ['->', 'fn']);
    assert.deepEqual(H.parseQueryTerms('-> fn').excludeTerms, []);
    const q = H.parseQueryTerms('-1.5 - -_x -Wall');
    assert.deepEqual(q.terms, ['-1.5', '-']);
    assert.deepEqual(q.excludeTerms, ['_x', 'Wall']);
});

test('parseQueryTerms: quoting makes operator-like text literal', () => {
    assert.deepEqual(H.parseQueryTerms('"file:"').terms, ['file:']);
    assert.deepEqual(H.parseQueryTerms('"-foo"').terms, ['-foo']);
    assert.deepEqual(H.parseQueryTerms('x -"dog here"').excludeTerms, ['dog here']);
    assert.deepEqual(H.parseQueryTerms('x -"dog here"').terms, ['x']);
    // a quote after the colon still belongs to the operator
    assert.deepEqual(H.parseQueryTerms('file:"my dir" x').terms, ['x']);
    assert.deepEqual(H.parseQueryTerms('-"file:x"').excludeTerms, ['file:x']);
});

test('parseQueryTerms: an operator without an argument or with a bad language is a term', () => {
    const q = H.parseQueryTerms('file: lang: case: word:');
    assert.deepEqual(q.terms, ['file:', 'lang:', 'case:', 'word:']);
    assert.equal(q.caseSensitive, false);
    assert.equal(q.wholeWord, false);
    assert.deepEqual(H.parseQueryTerms('-file:').excludeTerms, ['file:']);
    assert.deepEqual(H.parseQueryTerms('lang:??? x').terms, ['lang:???', 'x']);
});

test('tokenizeQuery records where the first quote stood', () => {
    assert.deepEqual(H.tokenizeQuery('a "b c" -"d"'), [
        { text: 'a', quoteAt: null },
        { text: 'b c', quoteAt: 0 },
        { text: '-d', quoteAt: 1 },
    ]);
    // an unclosed quote swallows the rest of the query as one token
    assert.deepEqual(H.tokenizeQuery('"open phrase').map(t => t.text), ['open phrase']);
});

// ---------- regex translation ----------

test('compileRustRegex: leading inline flags and named groups', () => {
    const re = H.compileRustRegex('(?i)foo');
    assert.ok(re instanceof RegExp);
    assert.equal(re.flags.includes('i'), true);
    assert.equal(re.flags.includes('g'), true);
    assert.equal(re.test('FOO'), true);
    const named = H.compileRustRegex('(?P<name>a+)b');
    assert.equal(named.source, '(?<name>a+)b');
    assert.equal('aab'.match(named).groups === undefined, true); // /g match has no groups
    named.lastIndex = 0;
    assert.equal(named.exec('aab').groups.name, 'aa');
    const multi = H.compileRustRegex('(?is)a.b');
    assert.equal(multi.flags.includes('s'), true);
    assert.equal(multi.test('A\nB'), true);
});

test('compileRustRegex: invalid patterns give null, non-unicode fallback works', () => {
    assert.equal(H.compileRustRegex('('), null);
    assert.equal(H.compileRustRegex('[unclosed'), null);
    // `\-` outside a class is an error under the u flag; falls back to the non-u RegExp
    const re = H.compileRustRegex('a\\-b');
    assert.ok(re instanceof RegExp);
    assert.equal(re.test('a-b'), true);
});

// ---------- term matching ----------

function ranges(text, query, opts) {
    return H.matcherRanges(text, H.buildQueryMatcher(query, opts));
}

test('buildQueryMatcher: every term is matched case-insensitively', () => {
    assert.deepEqual(ranges('fn main() { Main }', 'fn main'), [[0, 2], [3, 7], [12, 16]]);
    assert.deepEqual(ranges('let x = 1;', 'lang:rust let'), [[0, 3]]);
    assert.equal(H.buildQueryMatcher('', {}), null);
    assert.equal(H.buildQueryMatcher('lang:rust'), null);
    assert.equal(H.buildQueryMatcher('-x'), null);
});

test('buildQueryMatcher: longest term wins, case:yes and word:yes are honoured', () => {
    assert.deepEqual(ranges('foobar foo', 'foo foobar'), [[0, 6], [7, 10]]);
    assert.deepEqual(ranges('Foo foo', 'case:yes Foo'), [[0, 3]]);
    assert.deepEqual(ranges('foo foobar _foo', 'word:yes foo'), [[0, 3]]);
    assert.deepEqual(ranges('a.b axb', '"a.b"'), [[0, 3]]); // regex metacharacters are literal
});

test('buildQueryMatcher: regex and references modes', () => {
    assert.deepEqual(ranges('fn  foo() fn bar', 'fn\\s+\\w+', { regex: true }), [[0, 7], [10, 16]]);
    assert.equal(H.buildQueryMatcher('(', { regex: true }), null);
    // references: the identifier, whole word and case-sensitive
    assert.deepEqual(ranges('Foo foo Foobar (Foo)', 'Foo', { references: true }), [[0, 3], [16, 19]]);
});

test('matcherRanges: zero-length matches do not loop and output is capped', () => {
    assert.deepEqual(ranges('abc', 'x*', { regex: true }), []);
    const many = H.matcherRanges('a'.repeat(2000), H.buildQueryMatcher('a'));
    assert.equal(many.length, 501);
    assert.deepEqual(H.matcherRanges('', H.buildQueryMatcher('a')), []);
    assert.deepEqual(H.matcherRanges('abc', null), []);
});

// ---------- highlight.js line splitting ----------

test('splitHighlightedHtml: spans crossing a newline are closed and reopened', () => {
    const html = '<span class="hljs-comment">/* a\nb */</span> x';
    assert.deepEqual(H.splitHighlightedHtml(html), [
        '<span class="hljs-comment">/* a</span>',
        '<span class="hljs-comment">b */</span> x',
    ]);
});

test('splitHighlightedHtml: nested spans, empty lines and a trailing newline', () => {
    const html = '<span class="a">x<span class="b">y\n\nz</span>w</span>\n';
    assert.deepEqual(H.splitHighlightedHtml(html), [
        '<span class="a">x<span class="b">y</span></span>',
        '<span class="a"><span class="b"></span></span>',
        '<span class="a"><span class="b">z</span>w</span>',
        '',
    ]);
    assert.deepEqual(H.splitHighlightedHtml('plain'), ['plain']);
    assert.deepEqual(H.splitHighlightedHtml(''), ['']);
    // a stray '<' that is not a tag is kept as text
    assert.deepEqual(H.splitHighlightedHtml('a &lt; b <'), ['a &lt; b <']);
});

test('splitHighlightedHtml never changes the line count', () => {
    const src = 'a\n<span>b\nc</span>\n\nd';
    assert.equal(H.splitHighlightedHtml(src).length, src.split('\n').length);
});

// ---------- escaping and formatting (common.js) ----------

test('escapeHtml escapes markup and both quote styles', () => {
    assert.equal(C.escapeHtml('<a href="x" title=\'y\'>&</a>'),
        '&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;&amp;&lt;/a&gt;');
    assert.equal(C.escapeHtml(null), '');
    assert.equal(C.escapeHtml(undefined), '');
    assert.equal(C.escapeHtml(42), '42');
});

test('formatBytes picks the unit and one decimal', () => {
    assert.equal(C.formatBytes(0), '0 B');
    assert.equal(C.formatBytes(1023), '1023 B');
    assert.equal(C.formatBytes(1024), '1 KB');
    assert.equal(C.formatBytes(1536), '1.5 KB');
    assert.equal(C.formatBytes(3 * 1024 * 1024), '3 MB');
    assert.equal(C.formatBytes(1219949), '1.2 MB');
});

// ---------- URL / form state validation ----------

test('clampMaxResults keeps 1..1000 and rejects non-numbers', () => {
    assert.equal(H.clampMaxResults('50'), 50);
    assert.equal(H.clampMaxResults('1000'), 1000);
    assert.equal(H.clampMaxResults('5000'), 1000);
    assert.equal(H.clampMaxResults('0'), 1);
    assert.equal(H.clampMaxResults('-7'), 1);
    assert.equal(H.clampMaxResults('abc'), null);
    assert.equal(H.clampMaxResults(undefined), null);
    assert.equal(H.clampMaxResults('25px'), 25); // parseInt semantics, as the select values are
});

test('pickMaxResultsOption chooses the smallest option covering the request', () => {
    const opts = ['25', '50', '100', '250', '500', '1000'];
    assert.equal(H.pickMaxResultsOption(50, opts), 50);
    assert.equal(H.pickMaxResultsOption(51, opts), 100);
    assert.equal(H.pickMaxResultsOption(1, opts), 25);
    assert.equal(H.pickMaxResultsOption(1000, opts), 1000);
    assert.equal(H.pickMaxResultsOption(1000, ['25', '50', '250']), 250);
    assert.equal(H.pickMaxResultsOption(10, []), undefined);
    assert.equal(H.pickMaxResultsOption(10, ['x']), undefined);
});

test('clampContextLines keeps 0..10', () => {
    assert.equal(H.clampContextLines('3'), 3);
    assert.equal(H.clampContextLines('0'), 0);
    assert.equal(H.clampContextLines('42'), 10);
    assert.equal(H.clampContextLines('-1'), 0);
    assert.equal(H.clampContextLines('nope'), 0);
    assert.equal(H.clampContextLines(''), 0);
});

test('parseBoolParam accepts the usual truthy spellings only', () => {
    for (const v of ['true', 'TRUE', '1', 'yes', 'on']) assert.equal(H.parseBoolParam(v), true, v);
    for (const v of ['false', '0', 'no', 'off', '', 'maybe', undefined]) assert.equal(H.parseBoolParam(v), false, String(v));
});

// ---------- misc ----------

test('hljsLangForPath maps extensions case-insensitively and defaults to plaintext', () => {
    assert.equal(H.hljsLangForPath('src/main.rs'), 'rust');
    assert.equal(H.hljsLangForPath('a/b/C.PY'), 'python');
    assert.equal(H.hljsLangForPath('index.html'), 'xml');
    assert.equal(H.hljsLangForPath('Makefile'), 'makefile'); // the whole name is the "extension"
    assert.equal(H.hljsLangForPath('README'), 'plaintext');
    assert.equal(H.hljsLangForPath(''), 'plaintext');
});

test('iconSvg references the sprite symbol with dashes', () => {
    assert.equal(H.iconSvg('open_in_new', 18),
        '<svg class="icon" width="18" height="18" aria-hidden="true"><use href="#i-open-in-new"/></svg>');
});

test('colour helpers: parsing and WCAG contrast', () => {
    assert.deepEqual(H.parseColorToRgb('#fff'), { r: 255, g: 255, b: 255 });
    assert.deepEqual(H.parseColorToRgb('#d44e38'), { r: 212, g: 78, b: 56 });
    assert.deepEqual(H.parseColorToRgb('rgb(1, 2, 3)'), { r: 1, g: 2, b: 3 });
    assert.deepEqual(H.parseColorToRgb('rgba(1 2 3 / 50%)'), { r: 1, g: 2, b: 3 });
    assert.equal(H.parseColorToRgb('#12345'), null);
    assert.equal(H.parseColorToRgb('red'), null);
    assert.equal(H.parseColorToRgb(''), null);
    const white = H.relativeLuminance({ r: 255, g: 255, b: 255 });
    const black = H.relativeLuminance({ r: 0, g: 0, b: 0 });
    assert.equal(Math.round(H.contrastRatio(white, black)), 21);
    assert.equal(H.contrastRatio(white, white), 1);
});
