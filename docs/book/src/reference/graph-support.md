# Dependency graph support

What the import graph (the dependency explorer, "imported by N", `/api/graph`,
MCP's graph tools) understands, per language. An import that is not listed
here produces no edge. If fcs misses an edge listed as supported, that is a
bug; if it misses one listed as not handled, that is a known gap. Python
and TypeScript are checked against their own resolvers (see the end of the
page).
[How the import graph is built](../how-it-works/dependency-graph.md)
explains the method, including how an import is judged internal or
external.

Edges exist for **Rust, Python and JavaScript/TypeScript** only. Go, Java,
C and C++ `#include`, C#, Ruby and every other language are searched and
have symbols, but get no import edges.

## How resolution works, for every language

- An import resolves only to a file that is **in the index**. Packages
  outside the indexed folders (`std`, site-packages, `node_modules`, which
  is excluded by default) never produce an edge.
- A bare or external name resolves to nothing, unless the language's own
  configuration maps it to an indexed file (for JavaScript and TypeScript:
  tsconfig `paths`/`baseUrl`, a workspace `package.json` `name`, or the
  repository root when there is no tsconfig `baseUrl`). There is no "a
  file with that name somewhere in the repo" guess, so a missing edge is
  never replaced by a wrong one.
- Imports that do not resolve are kept. They are retried when a matching
  file is indexed later, and `GET /api/graph/imports?path=…` lists every
  import of a file with its line, and `target: null` for the unresolved
  ones.
- The unresolved ones are counted. Most are packages, but one written as a
  path into the project (`./x`, `.x`, `crate::`, `@/`) is a broken import
  or a form listed below as not handled. The dependency explorer's folder
  map gives that count, `GET /api/graph/modules` returns both
  (`unresolved_imports`, `unresolved_local`), MCP's `dependency_overview`
  reports them, and `file_dependencies` lists a file's unresolved imports.

## Rust (`.rs`)

| Form | Supported |
|---|---|
| `use crate::a::b;` | yes, from the nearest `Cargo.toml`'s `src/` |
| `use self::a;`, `use super::super::a;` | yes, relative to the module's directory (`foo/` for `foo.rs`; the file's directory for `mod.rs`, `lib.rs`, `main.rs`) |
| `use a::b;` where `a` is a sibling module | yes |
| `use my_crate::a;` inside `my_crate` | yes (`[lib] name`, else `[package] name` with `-` as `_`) |
| `use a::{b, c};`, `use a::*;`, `use a as b;` | yes, to the longest module path that exists (`a/b.rs`, then `a/b/mod.rs`) |
| `mod foo;` | yes |
| `crate::a::f()`, `super::x::Y` used inline, without a `use` | yes, once per file, like `use crate::a::f` |
| `use other_workspace_crate::x;` | **no**: only the importing crate's own name is recognised |
| `use crate::{a, b};` | **no**: reduces to `crate::`, which does not reach `lib.rs` |
| `extern crate x;`, `#[path = "…"] mod x;` | **no** |
| External crates and `std` | no edge, by design |

Inline `mod foo { … }` is treated like `mod foo;`, so it can draw an edge to
a file `foo.rs` that is not the module. Config read: `Cargo.toml`.

## Python (`.py`, `.pyi`, `.pyw`)

| Form | Supported |
|---|---|
| `import a.b`, `import a, b as c` | yes, one edge per module |
| `from a.b import c` | yes: to `a/b/c.py` (or `a/b/c/__init__.py`) when `c` is a module, else to `a/b.py` or `a/b/__init__.py`, as Python does |
| `from .x import y`, `from ..x import y` | yes: one dot is the file's own package, each extra dot one level up; when `x` does not exist the import is unresolved, as Python fails it |
| `from . import y` | yes: to `y.py` when it is a module, else to the package's `__init__.py` |
| Absolute imports | yes, as Python 3 does: from the ancestor folders that are not packages themselves (no `__init__.py`), so `import json` inside a package is the standard library, not a `json.py` next to it; where several could hold the module, the one that holds most of the dotted name wins |
| `src/` layouts (`tests/` importing `src/pkg`) | yes: each of those folders' `src/` is tried too |
| Absolute imports from a file nested any number of folders deep | yes: every ancestor folder is tried |
| A repository whose root folder has an `__init__.py`, run with that folder on `sys.path` (`import app.models`) | yes: when nothing else resolves the import, the outermost package folder is tried as a root if it is the repository root (`.git`) or no folder above it holds an indexed file |
| Other `sys.path` entries (`pyproject.toml`, `setup.cfg`, `PYTHONPATH`) | **no**: no configuration is read, so a package in a sibling folder (`lib/mypkg/` imported from `apps/x/main.py`, with `lib` put on `PYTHONPATH` by your setup) is not found and the import counts as external |
| `.pyi` stubs as import targets | **no** |
| Standard library, installed packages | no edge, by design |

## JavaScript and TypeScript (`.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`)

| Form | Supported |
|---|---|
| `import … from './x'`, `export … from './x'`, `export * from './x'` | yes |
| `require('./x')`, `` require(`./x`) ``, `import('./x')` | yes (not a template string with `${…}`), also `import(/* comment */ './x')` |
| `import x from './x.json' with { type: 'json' }`, and the older `assert { … }` | yes |
| `import x = require('./x')` (TypeScript) | yes |
| Relative specifiers (`./`, `../`, `.`, `..`) | yes |
| `@/x`, `~/x` | yes: tsconfig `paths` when it maps them, else from the nearest folder with a `package.json` or `tsconfig.json` (else the repository root), trying `src/x` then `x` |
| Extension probing | the exact file; `.js`/`.jsx` written for a `.ts`/`.tsx` file, `.mjs` for `.mts`, `.cjs` for `.cts`; then `.ts .tsx .js .jsx .mjs .cjs .mts .cts .d.ts .json` appended; then `index` with each of those extensions |
| Imports of stylesheets, `.vue`/`.svelte` components and other assets | yes, when the file is indexed (a bundler resolves them; TypeScript does not) |
| A folder's own `package.json` `main` (`import './lib'` where `lib/package.json` names the entry) | **no** |
| `package.json` `imports` (`#name`) and a package importing itself by name | **no** |
| tsconfig / jsconfig `paths` and `baseUrl`, with relative `extends` | yes, as `tsc` resolves them; an `extends` naming an npm package is skipped |
| Workspace packages by name (`@acme/shared`, `@acme/shared/x`) | yes: to an indexed `package.json` with that `name`: its `types`/`source`/`module`/`main`/`exports["."]` entry, else `src/index`; a subpath under `src/` or the package root |
| No tsconfig `baseUrl`/`paths` (`import 'components/Button'`) | yes: tried from the repository root (the folder holding `.git`) |
| `exports` subpath maps and nested conditions | **no** |
| Packages in `node_modules` | no edge, by design |

## How this is checked

Extraction and resolution have unit tests per form
(`src/symbols/extractor.rs`, `src/dependencies/mod.rs`). On top of those,
`examples/graph_oracle.rs` compares the whole graph with an independent
resolver over a pinned repository and lists every edge fcs misses or draws
wrongly:

| Language | Oracle | Repository | Result |
|---|---|---|---|
| Python | [grimp](https://github.com/python-grimp/grimp) 3.17 (`src/check/oracle/py.py`) | Django 5.2 | 3,041 of 3,041 edges, none extra |
| TypeScript, JavaScript | TypeScript 5.9 `ts.resolveModuleName` under each file's tsconfig (`src/check/oracle/ts.mjs`) | Vite 6.3.5 | 99.6% of edges, 1 extra; 4 of the 7 misses are `package.json` `imports` (`#name`); 221 workspace-package edges the oracle cannot see without an install are listed apart |

CI runs both, and fails if a change misses or adds more edges than that.

```sh
python3 src/check/oracle/py.py bench-corpus/django > py.json
cargo run --release --example graph_oracle -- bench-corpus/django --oracle py.json --lang py
(cd scripts/graph-oracle && npm ci)   # pins TypeScript
(cd scripts/graph-oracle && node ../../src/check/oracle/ts.mjs ../../bench-corpus/vite) > ts.json
cargo run --release --example graph_oracle -- bench-corpus/vite --oracle ts.json --lang ts
```

To run the same comparison on your own code, use `fcs check` (see
[Correctness against ripgrep](../benchmarks/correctness.md#on-your-own-code-fcs-check)).
It runs whichever oracles are installed: Python's needs `pip install
grimp`, TypeScript's needs `typescript` in the repository's
`node_modules`.
