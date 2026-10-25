'use strict';
// Node tests for the pure keyword-search helpers (`npm test`, built-in runner).
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const H = require(path.join(__dirname, '..', 'lib', 'keyword-helpers.js'));
// escapeHtml / formatBytes stay in common.js (semantic.html loads only that
// file); they are tested here from there.
const C = require(path.join(__dirname, '..', 'common.js'));

// ---------- path matches ----------

test('pathMatchRuns: marks the hit and splits off the file name', () => {
    const runs = H.pathMatchRuns('src/web/api.rs', [[4, 14]]);
    assert.deepEqual(runs, [
        { text: 'src/', hit: false, name: false },
        { text: 'web/', hit: true, name: false },
        { text: 'api.rs', hit: true, name: true },
    ]);
    assert.equal(runs.map(r => r.text).join(''), 'src/web/api.rs');
});

test('pathMatchRuns: byte ranges past multibyte characters', () => {
    // 'é' is 2 bytes: "café/" is 6 bytes, so "api" is bytes 6..9.
    const runs = H.pathMatchRuns('café/api.rs', [[6, 9]]);
    assert.deepEqual(runs.filter(r => r.hit).map(r => r.text), ['api']);
    assert.deepEqual(H.pathMatchRuns('a.rs', []), [{ text: 'a.rs', hit: false, name: true }]);
});

test('fileMatchesLabel: singular and plural', () => {
    assert.equal(H.fileMatchesLabel(1), '1 path matches');
    assert.equal(H.fileMatchesLabel(1200), '1,200 paths match');
});

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

test('parseQueryTerms: several words are one literal phrase', () => {
    assert.deepEqual(H.parseQueryTerms('hello world').terms, ['hello world']);
    assert.deepEqual(H.parseQueryTerms('  fn  main\t( \n').terms, ['fn  main\t(']);
    assert.deepEqual(H.parseQueryTerms(''), { text: '', terms: [], caseSensitive: false, wholeWord: false });
    assert.deepEqual(H.parseQueryTerms(null).terms, []);
});

test('parseQueryTerms: operators are consumed', () => {
    const q = H.parseQueryTerms('needle lang:rust file:src/ -file:test case:yes word:y -lang:py');
    assert.deepEqual(q.terms, ['needle']);
    assert.equal(q.caseSensitive, true);
    assert.equal(q.wholeWord, true);
    assert.deepEqual(H.parseQueryTerms('file:*.toml x').terms, ['x']);
    // an unknown but alphanumeric language name is still a filter
    assert.deepEqual(H.parseQueryTerms('lang:xyz x').terms, ['x']);
    assert.equal(H.parseQueryTerms('a case:no').caseSensitive, false);
    assert.equal(H.parseQueryTerms('a case:TRUE').caseSensitive, true);
    assert.equal(H.parseQueryTerms('a word:1').wholeWord, true);
    // operators between pieces of text join them with one space
    assert.deepEqual(H.parseQueryTerms('fn  file:src/  main').terms, ['fn main']);
});

test('parseQueryTerms: quotes and a leading - are text', () => {
    assert.deepEqual(H.parseQueryTerms('-Wall gcc').terms, ['-Wall gcc']);
    assert.deepEqual(H.parseQueryTerms('-> fn').terms, ['-> fn']);
    assert.deepEqual(H.parseQueryTerms('"fn main"').terms, ['"fn main"']);
    assert.deepEqual(H.parseQueryTerms('{ "success": True').terms, ['{ "success": True']);
    assert.deepEqual(H.parseQueryTerms('"file:x"').terms, ['"file:x"']);
    assert.deepEqual(H.parseQueryTerms('x = ""').terms, ['x = ""']);
});

test('parseQueryTerms: an operator without an argument or with a bad language is text', () => {
    const q = H.parseQueryTerms('file: lang: case: word:');
    assert.deepEqual(q.terms, ['file: lang: case: word:']);
    assert.equal(q.caseSensitive, false);
    assert.equal(q.wholeWord, false);
    assert.deepEqual(H.parseQueryTerms('-file:').terms, ['-file:']);
    assert.deepEqual(H.parseQueryTerms('lang:??? x').terms, ['lang:??? x']);
    // a quoted argument holds spaces; an unclosed one is literal
    assert.deepEqual(H.parseQueryTerms('x file:"my dir"').terms, ['x']);
    assert.deepEqual(H.parseQueryTerms('file:"my dir').terms, ['dir']);
    assert.deepEqual(H.parseQueryTerms('url=file:x').terms, ['url=file:x']);
});

test('tokenizeQuery records spans, operators and arguments', () => {
    assert.deepEqual(H.tokenizeQuery('a file:"b c" case:'), [
        { start: 0, end: 1, op: '', arg: null },
        { start: 2, end: 12, op: 'file:', arg: 'b c' },
        { start: 13, end: 18, op: 'case:', arg: null },
    ]);
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

test('buildQueryMatcher: the phrase is matched case-insensitively', () => {
    assert.deepEqual(ranges('fn main() { FN MAIN } fn  main', 'fn main'), [[0, 7], [12, 19]]);
    assert.deepEqual(ranges('let x = 1;', 'lang:rust let'), [[0, 3]]);
    assert.equal(H.buildQueryMatcher('', {}), null);
    assert.equal(H.buildQueryMatcher('lang:rust'), null);
    assert.deepEqual(ranges('gcc -x', '-x'), [[4, 6]]);
});

test('buildQueryMatcher: case:yes and word:yes are honoured', () => {
    assert.deepEqual(ranges('Foo foo', 'case:yes Foo'), [[0, 3]]);
    assert.deepEqual(ranges('foo foobar _foo', 'word:yes foo'), [[0, 3]]);
    assert.deepEqual(ranges('a.b axb "a.b"', 'a.b'), [[0, 3], [9, 12]]); // regex metacharacters are literal
    assert.deepEqual(ranges('a.b "a.b"', '"a.b"'), [[4, 9]]); // so are quotes
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
    assert.equal(H.hljsLangForPath('Makefile'), 'makefile');
    assert.equal(H.hljsLangForPath('README'), 'plaintext');
    assert.equal(H.hljsLangForPath('src/lib.dart'), 'dart');
    assert.equal(H.hljsLangForPath('proto/search.proto'), 'protobuf');
});

test('hljsLangForPath: files known by name, in any directory', () => {
    // Used to take "deploy/dockerfile" as the extension and fall back to plain text.
    assert.equal(H.hljsLangForPath('deploy/Dockerfile'), 'dockerfile');
    assert.equal(H.hljsLangForPath('Dockerfile.dev'), 'dockerfile');
    assert.equal(H.hljsLangForPath('tools/Makefile'), 'makefile');
    assert.equal(H.hljsLangForPath('v1.2/NOTES'), 'plaintext');
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

// ---------- regex search help ----------

test('escapeRegex escapes every metacharacter', () => {
    assert.equal(H.escapeRegex('{ "a": f(x) }'), '\\{ "a": f\\(x\\) \\}');
    assert.equal(H.escapeRegex('a.b*c+d?e|f^g$h[i]\\'), 'a\\.b\\*c\\+d\\?e\\|f\\^g\\$h\\[i\\]\\\\');
    assert.ok(new RegExp(H.escapeRegex('x.(y)[z]{1}')).test('x.(y)[z]{1}'));
});

test('regexToLiteral unescapes punctuation and rejects regex features', () => {
    assert.equal(H.regexToLiteral('\\{ "success": True'), '{ "success": True');
    assert.equal(H.regexToLiteral('foo\\.bar'), 'foo.bar');
    assert.equal(H.regexToLiteral('plain'), 'plain');
    assert.equal(H.regexToLiteral('fn \\w+'), null);
    assert.equal(H.regexToLiteral('a.b'), null);
    assert.equal(H.regexToLiteral('(a)'), null);
    assert.equal(H.regexToLiteral('trailing\\'), null);
});

test('relaxRegexSpacing turns literal spaces into \\s*', () => {
    assert.equal(H.relaxRegexSpacing('\\{ "success": True'), '\\{\\s*"success":\\s*True');
    assert.equal(H.relaxRegexSpacing('a   b'), 'a\\s*b');
    assert.equal(H.relaxRegexSpacing('[ ]x y'), '[ ]x\\s*y');
    assert.equal(H.relaxRegexSpacing('a\\ b'), null);
    assert.equal(H.relaxRegexSpacing('nospace'), null);
    const re = new RegExp(H.relaxRegexSpacing('\\{ "success": True'));
    assert.ok(re.test('{"success":True'));
    assert.ok(re.test('{  "success":   True'));
});

test('looksLikeRegex spots regex syntax in a plain query', () => {
    for (const q of ['fn \\w+', 'foo.*bar', '\\{ "a"', 'TODO|FIXME', '[A-Z]+_ID', '^use ', 'end$']) {
        assert.ok(H.looksLikeRegex(q), q);
    }
    for (const q of ['fn main', '{ "success": True', 'a.b', 'x = 1', '->', 'path/to/file.rs']) {
        assert.ok(!H.looksLikeRegex(q), q);
    }
});

test('noResultSuggestions for a regex offers spacing, case and plain text', () => {
    const s = H.noResultSuggestions('\\{ "success": True', true);
    assert.deepEqual(s.map(x => [x.label, x.query, x.regex]), [
        ['Flexible spacing', '\\{\\s*"success":\\s*True', true],
        ['Ignore case', '(?i)\\{ "success": True', true],
        ['Flexible spacing, ignore case', '(?i)\\{\\s*"success":\\s*True', true],
        ['Search as plain text', '{ "success": True', false],
    ]);
    // Already case-insensitive, no spaces, not a literal: nothing to offer.
    assert.deepEqual(H.noResultSuggestions('(?i)fn\\w+', true), []);
    assert.deepEqual(H.noResultSuggestions('', true), []);
});

test('noResultSuggestions for plain text offers regex only when it looks like one', () => {
    assert.deepEqual(H.noResultSuggestions('fn \\w+', false), [
        { label: 'Search as regex', query: 'fn \\w+', regex: true },
    ]);
    assert.deepEqual(H.noResultSuggestions('fn main', false), []);
});

test('regexErrorSuggestions offers plain text and an escaped pattern', () => {
    assert.deepEqual(H.regexErrorSuggestions('{ "success": True'), [
        { label: 'Search as plain text', query: '{ "success": True', regex: false },
        { label: 'Escape special characters', query: '\\{ "success": True', regex: true },
    ]);
    assert.deepEqual(H.regexErrorSuggestions('abc').length, 1);
});

// ---------- dependency explorer links ----------

test('hasImportGraph: only languages whose imports are resolved', () => {
    for (const p of ['r/src/a.rs', 'r/pkg/x.py', 'r/web/a.tsx', 'r/web/b.MJS']) assert.ok(H.hasImportGraph(p), p);
    for (const p of ['r/README.md', 'r/Cargo.toml', 'r/main.go', '']) assert.ok(!H.hasImportGraph(p), p);
});

test('graphExplorerUrl encodes the path', () => {
    assert.equal(H.graphExplorerUrl('r/a b/#x.rs'), '/graph.html?file=r%2Fa%20b%2F%23x.rs');
});

// ---------- results header ----------

test('loadMoreRequest: below the cap the whole list is fetched again', () => {
    assert.deepEqual(H.loadMoreRequest(50, 50), { offset: 0, max: 100, redraw: true });
    assert.deepEqual(H.loadMoreRequest(950, 100), { offset: 0, max: 1000, redraw: true });
    assert.deepEqual(H.loadMoreRequest(999, 50), { offset: 0, max: 1000, redraw: true });
});

test('loadMoreRequest: at the cap it falls back to offset paging', () => {
    assert.deepEqual(H.loadMoreRequest(1000, 50), { offset: 1000, max: 50, redraw: false });
    assert.deepEqual(H.loadMoreRequest(1050, 50), { offset: 1050, max: 50, redraw: false });
});

test('resultsCountLabel: complete scan shows the plain total', () => {
    assert.equal(H.resultsCountLabel({ total_matches: 1234 }, 1234).text, '1,234 results');
    assert.equal(H.resultsCountLabel({ total_matches: 1 }, 1).text, '1 result');
    assert.equal(H.resultsCountLabel({ total_matches: 1234 }, 100).text, '100 of 1,234 results');
});

test('resultsCountLabel: unknown total shows N+', () => {
    const budget = H.resultsCountLabel({ truncated_by_budget: true, has_more: true }, 1000);
    assert.equal(budget.text, '1,000+ results');
    assert.match(budget.title, /total is unknown/);
    assert.equal(H.resultsCountLabel({ has_more: true }, 50).text, '50+ results');
    assert.equal(H.resultsCountLabel({ has_more: false }, 3).text, '3 results');
});

test('resultsCountLabel: the tooltip names the limit the server hit', () => {
    const data = {
        truncated_by_budget: true,
        has_more: true,
        limits_hit: [
            { limit: 'max_files', detail: 'searched the best 2 of 9 candidate files' },
            { limit: 'timeout', detail: 'stopped at the time limit' },
        ],
    };
    const label = H.resultsCountLabel(data, 10);
    assert.match(label.title, /stopped at the time limit/);
    // A file limit leaves files unread, so it is named too.
    assert.match(label.title, /candidate files/);
    assert.equal(H.limitsHitDetail({}, ['timeout']), '');
});

test('searchMetaLabel: a time limit shows beside the time', () => {
    const meta = H.searchMetaLabel({ limits_hit: [{ limit: 'timeout', detail: 'x' }] }, 80);
    assert.equal(meta.text, '80 ms · stopped at time limit');
});

test('formatDurationMs: one decimal only under 10 ms', () => {
    assert.equal(H.formatDurationMs(42.34), '42 ms');
    assert.equal(H.formatDurationMs(4.25), '4.3 ms');
    assert.equal(H.formatDurationMs(undefined), '0.0 ms');
    assert.equal(H.formatDurationMs(1234.5), '1,235 ms');
});

test('searchMetaLabel: normally just the time, file count in the tooltip', () => {
    const full = H.searchMetaLabel({ rank_mode: 'full', total_candidates: 5678, candidates_searched: 5678 }, 42.3);
    assert.deepEqual(full, { text: '42 ms', title: '5,678 files could contain the query' });
    assert.equal(H.searchMetaLabel({ total_candidates: 1, candidates_searched: 1 }, 2).title, '1 file could contain the query');
});

test('searchMetaLabel: fast ranking that skipped files says so', () => {
    const fast = H.searchMetaLabel({ rank_mode: 'fast', total_candidates: 12000, candidates_searched: 2000 }, 80);
    assert.equal(fast.text, 'Searched 2,000 of 12,000 files · 80 ms');
    assert.match(fast.title, /Search more files/);
});

test('indexFreshnessLabel: silent while watching or before the first sync', () => {
    assert.equal(H.indexFreshnessLabel(undefined, 0), null);
    assert.equal(H.indexFreshnessLabel({ watching: true, synced_at_ms: 0 }, 1e9), null);
    assert.equal(H.indexFreshnessLabel({ watching: false }, 1e9), null);
});

test('indexFreshnessLabel: names the changes the watcher has not applied yet', () => {
    const label = H.indexFreshnessLabel({ watching: true, synced_at_ms: 0, pending_changes: 12500 }, 1e9);
    assert.equal(label.text, 'Index updating · 12,500 file changes to apply');
    assert.match(label.title, /may not include/);
    assert.equal(H.indexFreshnessLabel({ watching: true, pending_changes: 1 }, 0).text,
        'Index updating · 1 file change to apply');
    assert.equal(H.indexFreshnessLabel({ watching: true, pending_changes: 0 }, 0), null);
    // Files that could not be read are reported, alone or after the backlog.
    assert.equal(H.indexFreshnessLabel({ watching: true, unreadable_files: 1 }, 0).text,
        '1 file unreadable');
    const both = H.indexFreshnessLabel({ watching: true, pending_changes: 2, unreadable_files: 3 }, 0);
    assert.equal(both.text, 'Index updating · 2 file changes to apply · 3 files unreadable');
    assert.match(both.title, /3 files could not be read and are left out of the index/);
    // Folders that could not be read or watched, too.
    const folders = H.indexFreshnessLabel(
        { watching: true, unreadable_folders: 1, unwatched_folders: 2 }, 0);
    assert.equal(folders.text, '1 folder unreadable · 2 folders not watched');
    assert.match(folders.title, /missing from the index/);
    assert.equal(H.indexFreshnessLabel({ watching: false, unwatched_folders: 2 }, 0), null);
});

test('indexFreshnessLabel: says how old the index is when watching is off', () => {
    const at = 1_760_000_000_000;
    const label = H.indexFreshnessLabel({ watching: false, synced_at_ms: at }, at + 6 * 60_000);
    assert.equal(label.text, 'Index synced 6 min ago · watching off');
    assert.match(label.title, /watch = true/);
    assert.equal(H.formatAge(30_000), 'just now');
    assert.equal(H.formatAge(2 * 3600_000 + 5 * 60_000), '2 h 5 min ago');
    assert.equal(H.formatAge(3 * 86400_000), '3 days ago');
});

test('searchMetaLabel: no ranking info shows only the time', () => {
    assert.deepEqual(H.searchMetaLabel({}, 12), { text: '12 ms', title: '' });
});

// ---------- snippet highlighting ----------

test('snippetHighlightPrefix: a window starting inside a doc comment', () => {
    const lines = [' * survive (and nest inside) spans.', ' * @param {Element} el', ' */', 'function f() {'];
    assert.equal(H.snippetHighlightPrefix(lines, 'javascript'), '/*\n');
    // A closing delimiter before any opening one, even without ` * ` lines.
    assert.equal(H.snippetHighlightPrefix(['    still prose', '*/', 'fn f() {}'], 'rust'), '/*\n');
    assert.equal(H.snippetHighlightPrefix(['', ' *', 'x'], 'c'), '/*\n');
});

test('snippetHighlightPrefix: ordinary code needs no prefix', () => {
    assert.equal(H.snippetHighlightPrefix(['const a = 1;', '/** doc */', 'f();'], 'javascript'), '');
    assert.equal(H.snippetHighlightPrefix(['/*', ' * doc', ' */'], 'java'), '');
    assert.equal(H.snippetHighlightPrefix(['*ptr = 0;', '*/'], 'c'), '/*\n');
    assert.equal(H.snippetHighlightPrefix(['*ptr = 0;'], 'c'), '');
    assert.equal(H.snippetHighlightPrefix(['**kwargs'], 'c'), '');
    // Languages without /* */ comments are left alone.
    assert.equal(H.snippetHighlightPrefix([' * not a comment', '*/'], 'python'), '');
    assert.equal(H.snippetHighlightPrefix([], 'rust'), '');
});

test('hljsLangForPath: every language it names is loaded in the browser', () => {
    // The UI loads highlight.min.js then highlight-languages.min.js; a name
    // missing from both renders as plain text.
    const fs = require('node:fs');
    const vm = require('node:vm');
    const ctx = { window: {} };
    vm.createContext(ctx);
    for (const f of ['highlight.min.js', 'highlight-languages.min.js']) {
        vm.runInContext(fs.readFileSync(path.join(__dirname, '..', f), 'utf8'), ctx);
    }
    vm.runInContext('globalThis.h = hljs;', ctx);
    const src = fs.readFileSync(path.join(__dirname, '..', 'lib', 'keyword-helpers.js'), 'utf8');
    const body = src.slice(src.indexOf('function hljsLangForPath'), src.indexOf('function hasImportGraph'));
    const names = new Set([...body.matchAll(/(?:return |: )'([a-z]+)'/g)].map(m => m[1]));
    names.delete('plaintext');
    assert.ok(names.size > 30);
    for (const name of names) assert.ok(ctx.h.getLanguage(name), `no grammar for ${name}`);
});

test('fileViewHash / parseFileViewHash round-trip paths with special characters', () => {
    assert.equal(H.fileViewHash('src/main.rs', 42), '#file=src%2Fmain.rs&line=42');
    assert.equal(H.fileViewHash('README.md', 1), '#file=README.md');
    for (const file of ['a b/c&d=e#f.rs', 'src/日本.py', '100%.txt']) {
        assert.deepEqual(H.parseFileViewHash(H.fileViewHash(file, 7)), { file, line: 7 });
    }
    assert.deepEqual(H.parseFileViewHash('#file=x.rs'), { file: 'x.rs', line: 1 });
    assert.deepEqual(H.parseFileViewHash('#file=x.rs&line=-3'), { file: 'x.rs', line: 1 });
    assert.deepEqual(H.parseFileViewHash('file=x.rs&line=junk'), { file: 'x.rs', line: 1 });
    assert.equal(H.parseFileViewHash(''), null);
    assert.equal(H.parseFileViewHash('#top'), null);
});

// ---------- go to definition ----------

test('identifierAt: the name under or just before the caret', () => {
    const t = 'let req = SearchRequest::from_args(&args);';
    assert.deepEqual(H.identifierAt(t, 12), { start: 10, end: 23, word: 'SearchRequest' });
    assert.deepEqual(H.identifierAt(t, 10), { start: 10, end: 23, word: 'SearchRequest' });
    // caret right after the word (clicked on its last half-character)
    assert.equal(H.identifierAt(t, 23).word, 'SearchRequest');
    assert.equal(H.identifierAt(t, 26).word, 'from_args');
    assert.equal(H.identifierAt('a = $el.x', 5).word, '$el');
    assert.equal(H.identifierAt('a + b', 2), null); // on the operator
    assert.equal(H.identifierAt('x = 42;', 5), null); // numbers are not names
    assert.equal(H.identifierAt('', 0), null);
    assert.equal(H.identifierAt('  ', 1), null);
});

test('identifierAt: non-ASCII names and offsets after astral characters', () => {
    assert.equal(H.identifierAt('größe = 1', 2).word, 'größe');
    const t = '"😀" + café';
    const r = H.identifierAt(t, t.indexOf('café') + 1);
    assert.deepEqual(r, { start: t.indexOf('café'), end: t.length, word: 'café' });
});

test('rankDefinitions: exact name only, same file first, deduplicated', () => {
    const results = [
        { file_path: 'b.rs', line_number: 3, content: 'pub struct SearchEngine {', match_start: 11, match_end: 23, score: 40 },
        { file_path: 'c.rs', line_number: 9, content: 'pub struct SemanticSearchEngine {', match_start: 19, match_end: 31, score: 50 },
        { file_path: 'a.rs', line_number: 7, content: 'impl SearchEngine {', match_start: 5, match_end: 17, score: 10 },
        { file_path: 'b.rs', line_number: 3, content: 'pub struct SearchEngine {', match_start: 11, match_end: 23, score: 40 },
        { file_path: 'd.rs', line_number: 0, content: 'SearchEngine', match_start: 0, match_end: 12, score: 99 },
    ];
    assert.deepEqual(H.rankDefinitions(results, 'SearchEngine', 'a.rs'), [
        { file: 'a.rs', line: 7, content: 'impl SearchEngine {' },
        { file: 'b.rs', line: 3, content: 'pub struct SearchEngine {' },
    ]);
    assert.deepEqual(H.rankDefinitions([], 'x', 'a.rs'), []);
    assert.equal(H.definitionQuery('foo'), 'word:yes case:yes foo');
});

// ---------- case and more files ----------

test('searchCaseParam: MATCH CASE means the same in every mode', () => {
    assert.equal(H.searchCaseParam({ matchCase: true }), 'true');
    assert.equal(H.searchCaseParam({ matchCase: true, isRegex: true }), 'true');
    assert.equal(H.searchCaseParam({ matchCase: false, isRegex: true }), 'false');
    // plain text leaves case:yes in the query in charge
    assert.equal(H.searchCaseParam({ matchCase: false }), null);
    assert.equal(H.searchCaseParam({ matchCase: false, isReferences: true }), null);
});

test('searchIgnoresCase and the results header say which way case went', () => {
    assert.equal(H.searchIgnoresCase({ query: 'foo' }), true);
    assert.equal(H.searchIgnoresCase({ query: 'foo case:yes' }), false);
    assert.equal(H.searchIgnoresCase({ query: 'foo', matchCase: true }), false);
    assert.equal(H.searchIgnoresCase({ query: 'fo+', isRegex: true }), true);
    assert.equal(H.searchIgnoresCase({ query: 'Foo', isReferences: true }), false);
    const data = { total_candidates: 10 };
    assert.equal(H.searchMetaLabel(data, 36, true).text, '36 ms · ignoring case');
    assert.equal(H.searchMetaLabel(data, 36, false).text, '36 ms · matching case');
    assert.equal(H.searchMetaLabel(data, 36).text, '36 ms');
});

test('buildQueryMatcher follows MATCH CASE', () => {
    const r = (text, q, opts) => H.matcherRanges(text, H.buildQueryMatcher(q, opts));
    assert.deepEqual(r('Foo foo', 'foo', { caseSensitive: true }), [[4, 7]]);
    assert.deepEqual(r('Foo foo', 'foo', {}), [[0, 3], [4, 7]]);
    assert.deepEqual(r('Foo foo', 'foo', { regex: true, caseSensitive: false }), [[0, 3], [4, 7]]);
    assert.deepEqual(r('Foo foo', 'foo', { regex: true, caseSensitive: true }), [[4, 7]]);
});

test('moreFilesRequest: every candidate, or as many as the server allows', () => {
    assert.equal(H.moreFilesRequest({ total_candidates: 100, candidates_searched: 100 }), null);
    assert.equal(H.moreFilesRequest({}), null);
    assert.deepEqual(H.moreFilesRequest({ total_candidates: 12000, candidates_searched: 2000 }),
        { maxFiles: 12000, label: 'Search all files' });
    assert.deepEqual(
        H.moreFilesRequest({ total_candidates: 12000, candidates_searched: 2000, max_files_allowed: 5000 }),
        { maxFiles: 5000, label: 'Search 5,000 files' });
    assert.equal(
        H.moreFilesRequest({ total_candidates: 12000, candidates_searched: 5000, max_files_allowed: 5000 }),
        null);
});

test('noResultSuggestions: no "ignore case" when the regex already ignores it', () => {
    const labels = (q, mc) => H.noResultSuggestions(q, true, mc).map(s => s.label);
    assert.ok(labels('Foo', true).includes('Ignore case'));
    assert.ok(!labels('Foo', false).includes('Ignore case'));
});

// ---------- path filter boxes ----------

test('pathFilterParam: commas and semicolons both separate patterns', () => {
    assert.equal(H.pathFilterParam('src/**, lib/**'), 'src/**;lib/**');
    assert.equal(H.pathFilterParam('src/**;lib/** ; '), 'src/**;lib/**');
    assert.equal(H.pathFilterParam('  '), '');
});

test('pathFilterParam: a comma inside braces is glob alternation', () => {
    assert.equal(H.pathFilterParam('**/*.{rs,toml}, docs/**'), '**/*.{rs,toml};docs/**');
});

// ---------- file names are never re-cased ----------

test('fileExtension: the extension exactly as written', () => {
    assert.equal(C.fileExtension('src/web/api.rs'), 'rs');
    assert.equal(C.fileExtension('docs/README.MD'), 'MD');
    assert.equal(C.fileExtension('.github/workflows/ci.yml'), 'yml');
    assert.equal(C.fileExtension('Makefile'), '');
    assert.equal(C.fileExtension('src.d/Dockerfile'), '');
    assert.equal(C.fileExtension('.gitignore'), '');
});

test('resultsCountLabel: path matches are counted next to content results', () => {
    const data = { total_matches: 1, files: [{ file_path: 'a' }, { file_path: 'b' }], total_files: 2 };
    assert.equal(H.resultsCountLabel(data, 1).text, '2 files · 1 result');
    assert.equal(H.resultsCountLabel({ total_matches: 3, files: [] }, 3).text, '3 results');
});

// ---------- code viewer ----------

test('matchLineNumbers: lines the query matches, from the text', () => {
    const m = H.buildQueryMatcher('foo');
    assert.deepEqual(H.matchLineNumbers('foo\nbar\nx FOO y\n\nfoofoo', m), [1, 3, 5]);
    assert.deepEqual(H.matchLineNumbers('foo\nfoo\nfoo', m, 2), [1, 2]);
    assert.deepEqual(H.matchLineNumbers('abc', null), []);
    // A regex that can match empty only counts real matches.
    assert.deepEqual(H.matchLineNumbers('a\n\nb', H.buildQueryMatcher('b*', { regex: true })), [3]);
});

test('rulerMarks: one mark per bucket, positioned by line', () => {
    assert.deepEqual(H.rulerMarks([1, 51, 101], 101), [
        { line: 1, pct: 0 }, { line: 51, pct: 50 }, { line: 101, pct: 100 },
    ]);
    // 10,000 hits in a long file collapse to at most `buckets` marks.
    const lines = Array.from({ length: 10000 }, (_, i) => i + 1);
    assert.equal(H.rulerMarks(lines, 10000, 400).length, 400);
    assert.deepEqual(H.rulerMarks([1], 1), [{ line: 1, pct: 0 }]);
});

test('nearestLine: closest entry, ties go up the file', () => {
    assert.equal(H.nearestLine([10, 20, 40], 1), 10);
    assert.equal(H.nearestLine([10, 20, 40], 15), 10);
    assert.equal(H.nearestLine([10, 20, 40], 31), 40);
    assert.equal(H.nearestLine([10, 20, 40], 99), 40);
    assert.equal(H.nearestLine([], 5), null);
});

test('windowFor: grow the window nearby, start a new one far away', () => {
    assert.deepEqual(H.windowFor(50, 1, 100, 5000, 100, 100), { from: 1, to: 100, fresh: false });
    assert.deepEqual(H.windowFor(150, 1, 100, 5000, 100, 100), { from: 1, to: 200, fresh: false });
    assert.deepEqual(H.windowFor(350, 400, 600, 5000, 100, 100), { from: 300, to: 600, fresh: false });
    assert.deepEqual(H.windowFor(4000, 1, 100, 5000, 100, 100), { from: 3900, to: 4100, fresh: true });
    assert.deepEqual(H.windowFor(4990, 1, 100, 5000, 100, 100), { from: 4890, to: 5000, fresh: true });
});

// ---------- per-file hit counts ----------

test('groupHitsLabel: a page showing some of a file\'s hits says how many it has', () => {
    assert.deepEqual(H.groupHitsLabel(1, 7, false).text, '1 of 7 hits');
    assert.equal(H.groupHitsLabel(1, 7, false).more, 6);
    assert.equal(H.groupHitsLabel(2, 2, false).text, '2 hits');
    assert.equal(H.groupHitsLabel(1, 1, false).text, '1 hit');
    assert.equal(H.groupHitsLabel(1, 1, false).more, 0);
    // Older servers send no count: the hits on the page are all we know.
    assert.equal(H.groupHitsLabel(3, undefined, false).text, '3 hits');
    // A file cut by the per-file cap may hold more.
    assert.equal(H.groupHitsLabel(5, 5, true).text, '5 of 5+ hits');
    assert.equal(H.groupHitsLabel(1, 1200, false).text, '1 of 1,200 hits');
});

test('exactPathGlob: escapes glob metacharacters one by one', () => {
    assert.equal(H.exactPathGlob('src/a.ts'), 'src/a.ts');
    assert.equal(H.exactPathGlob('app/[id]/{a,b}*?.ts'), 'app/[[]id[]]/[{]a,b[}][*][?].ts');
});

// ---------- graph-scoped search ----------

test('parseQueryTerms: deps: and rdeps: are filters, not text', () => {
    assert.equal(H.parseQueryTerms('Weight rdeps:src/a.rs:2 -deps:b.py').text, 'Weight');
    assert.equal(H.parseQueryTerms('deps:').text, 'deps:');
    assert.equal(H.parseQueryTerms('url=deps:x').text, 'url=deps:x');
});

test('scopeLabel: file count, plain-words tooltip and the incomplete warning', () => {
    assert.equal(H.scopeLabel({}), null);
    const one = H.scopeLabel({ scope: { filters: ['rdeps:src/a.rs:1'], files: 23, unresolved_local: 0 } });
    assert.equal(one.text, 'In 23 files by imports');
    assert.match(one.title, /src\/a\.rs and the files that import it directly/);
    assert.equal(one.incomplete, false);
    const gap = H.scopeLabel({ scope: { filters: ['deps:m.py:*', '-rdeps:t.py:2'], files: 1, unresolved_local: 2 } });
    assert.equal(gap.text, 'In 1 file by imports (may be incomplete)');
    assert.match(gap.title, /files imported by it at any distance; and not t\.py and the files that import it within 2 hops/);
    assert.match(gap.title, /2 imports written as project paths/);
    assert.equal(H.scopeLabel({ scope: { filters: ['-deps:x.py:1'] } }).text, 'Scoped by imports');
});

test('scopedSearchUrl: keeps the query and names the direction', () => {
    assert.equal(H.scopedSearchUrl('rdeps', 'src/a b.rs', 'Weight'), '/?q=Weight&rdeps=src%2Fa+b.rs');
    assert.equal(H.scopedSearchUrl('deps', 'x.py', ''), '/?deps=x.py');
});

test('historyWith: newest first, no duplicates, capped', () => {
    assert.deepStrictEqual(H.historyWith(['b', 'aa'], 'aa', null, 50), ['aa', 'b']);
    assert.deepStrictEqual(H.historyWith(['bb', 'cc'], 'dd', null, 2), ['dd', 'bb']);
    assert.deepStrictEqual(H.historyWith(['bb'], 'x', null, 50), ['bb'], 'one-character queries are not kept');
});

test('historyWith: a settled query that keeps being edited is replaced, not stacked', () => {
    // Paused on "Trigr", then finished the word.
    assert.deepStrictEqual(H.historyWith(['Trigr', 'fn main'], 'TrigramIndex', 'Trigr', 50), ['TrigramIndex', 'fn main']);
    // Paused, then cut it back.
    assert.deepStrictEqual(H.historyWith(['TrigramIndex', 'fn main'], 'Trigram', 'TrigramIndex', 50), ['Trigram', 'fn main']);
    // A different query keeps the settled one.
    assert.deepStrictEqual(H.historyWith(['Trigr', 'fn main'], 'parse', 'Trigr', 50), ['parse', 'Trigr', 'fn main']);
    // An Enter-recorded query (replace = null) is never replaced.
    assert.deepStrictEqual(H.historyWith(['Trigr'], 'TrigramIndex', null, 50), ['TrigramIndex', 'Trigr']);
});
