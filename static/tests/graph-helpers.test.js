'use strict';
// Node tests for the dependency explorer's pure helpers (`npm test`).
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const G = require(path.join(__dirname, '..', 'lib', 'graph-helpers.js'));

test('graphBaseName / graphFolderOf split display paths', () => {
    assert.equal(G.graphBaseName('repo/src/a.rs'), 'a.rs');
    assert.equal(G.graphFolderOf('repo/src/a.rs'), 'repo/src');
    assert.equal(G.graphBaseName('a.rs'), 'a.rs');
    assert.equal(G.graphFolderOf('a.rs'), '');
});

test('searchIncludeFor escapes glob characters and drops unrepresentable paths', () => {
    assert.equal(G.searchIncludeFor(['r/a.rs', 'r/b.rs', 'r/a.rs']), 'r/a.rs;r/b.rs');
    assert.equal(G.searchIncludeFor(['r/[id]/page.tsx']), 'r/[[]id[]]/page.tsx');
    assert.equal(G.searchIncludeFor(['r/a*.py', 'r/{x}.js']), 'r/a[*].py;r/[{]x[}].js');
    assert.equal(G.searchIncludeFor(['r/semi;colon.rs', 'r/ok.rs']), 'r/ok.rs');
});

test('layerFolders puts each folder right of everything it imports', () => {
    const layers = G.layerFolders(['app', 'lib', 'util', 'alone'], [
        { from: 'app', to: 'lib' },
        { from: 'lib', to: 'util' },
        { from: 'app', to: 'util' },
    ]);
    assert.deepEqual(Object.fromEntries(layers), { app: 2, lib: 1, util: 0, alone: 0 });
});

test('layerFolders gives an import cycle one shared layer', () => {
    const layers = G.layerFolders(['a', 'b', 'c', 'base'], [
        { from: 'a', to: 'b', cycle: true },
        { from: 'b', to: 'a', cycle: true },
        { from: 'b', to: 'base' },
        { from: 'c', to: 'a' },
    ]);
    assert.equal(layers.get('a'), layers.get('b'));
    assert.equal(layers.get('a'), 1);
    assert.equal(layers.get('c'), 2);
    assert.equal(layers.get('base'), 0);
});

test('layerFolders handles a long chain without recursion', () => {
    const n = 50000;
    const folders = Array.from({ length: n }, (_, i) => 'f' + i);
    const edges = folders.slice(1).map((f, i) => ({ from: f, to: folders[i] }));
    assert.equal(G.layerFolders(folders, edges).get('f' + (n - 1)), n - 1);
});

test('capModuleMap keeps the most connected folders and their edges', () => {
    const map = {
        folders: [
            { folder: 'a', imports: 5, imported_by: 0 },
            { folder: 'b', imports: 0, imported_by: 5 },
            { folder: 'c', imports: 1, imported_by: 0 },
        ],
        edges: [{ from: 'a', to: 'b' }, { from: 'c', to: 'b' }],
    };
    const capped = G.capModuleMap(map, 2);
    assert.deepEqual(capped.folders.map(f => f.folder), ['a', 'b']);
    assert.deepEqual(capped.edges, [{ from: 'a', to: 'b' }]);
    assert.equal(capped.hidden, 1);
    assert.equal(G.capModuleMap(map, 5).hidden, 0);
});
