# Dependency graph support

What the import graph (the dependency explorer, "imported by N", `/api/graph`,
MCP's graph tools) understands, per language. An import that is not listed
here produces no edge. The graph has no oracle yet, so this page is the
contract: if fcs misses an edge listed as supported, that is a bug; if it
misses one listed as not handled, that is a known gap.

Edges exist for **Rust, Python and JavaScript/TypeScript** only. Go, Java,
C and C++ `#include`, C#, Ruby and every other language are searched and
have symbols, but get no import edges.

## How resolution works, for every language

- An import resolves only to a file that is **in the index**. Packages
  outside the indexed folders (`std`, site-packages, `node_modules`, which
  is excluded by default) never produce an edge.
- A bare or external name resolves to nothing. There is no "a file with
  that name somewhere in the repo" guess, so a missing edge is never
  replaced by a wrong one.
- Imports that do not resolve are kept. They are retried when a matching
  file is indexed later, and `GET /api/graph/imports?path=…` lists every
  import of a file with its line, and `target: null` for the unresolved
  ones.

## Rust (`.rs`)

| Form | Supported |
|---|---|
| `use crate::a::b;` | yes, from the nearest `Cargo.toml`'s `src/` |
| `use self::a;`, `use super::super::a;` | yes, relative to the module's directory (`foo/` for `foo.rs`; the file's directory for `mod.rs`, `lib.rs`, `main.rs`) |
| `use a::b;` where `a` is a sibling module | yes |
| `use my_crate::a;` inside `my_crate` | yes (`[lib] name`, else `[package] name` with `-` as `_`) |
| `use a::{b, c};`, `use a::*;`, `use a as b;` | yes, to the longest module path that exists (`a/b.rs`, then `a/b/mod.rs`) |
| `mod foo;` | yes |
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
| `from a.b import c` | yes, to `a/b.py` or `a/b/__init__.py` |
| `from .x import y`, `from ..x import y` | yes: one dot is the file's own package, each extra dot one level up |
| `from . import y` | **partly**: the edge goes to the package's `__init__.py`, not to `y.py` |
| `from pkg import submodule` | **partly**: the edge goes to `pkg/__init__.py` (or `pkg.py`), not to the submodule |
| Absolute imports | resolved from the file's directory and up to 7 parents, nearest first |
| `src/` layouts (`tests/` importing `src/pkg`) | **no**: no `pyproject.toml`, `setup.cfg` or `sys.path` is read |
| `.pyi` stubs as import targets | **no** |
| Standard library, installed packages | no edge, by design |

## JavaScript and TypeScript (`.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`)

| Form | Supported |
|---|---|
| `import … from './x'`, `export … from './x'`, `export * from './x'` | yes |
| `require('./x')`, `` require(`./x`) ``, `import('./x')` | yes (not a template string with `${…}`) |
| `import x = require('./x')` (TypeScript) | **no** |
| Relative specifiers (`./`, `../`, `.`, `..`) | yes |
| `@/x`, `~/x` | yes: from the nearest folder with a `package.json` or `tsconfig.json`, trying `src/x` then `x` |
| Extension probing | the exact file; `.js`/`.jsx`/`.mjs` written for a `.ts`/`.tsx`/`.mts` file; then `.ts .tsx .js .jsx .mjs .cjs .mts .cts .d.ts .json` appended; then `index.{ts,tsx,js,jsx,mjs,cjs}` |
| `.cjs` written for a `.cts` file; `index.mts`, `index.cts`, `index.d.ts` | **no** |
| tsconfig `paths` and `baseUrl` | **no**: tsconfig is not parsed |
| Workspace packages by name, `package.json` `exports`/`main` | **no** |
| Packages in `node_modules` | no edge, by design |

## How this is checked

Extraction and resolution have unit tests per form
(`src/symbols/extractor.rs`, `src/dependencies/mod.rs`). The next step in
the reliability plan is an oracle per language: TypeScript's own
`ts.resolveModuleName` and Python's `grimp`, run over pinned repositories,
with every edge fcs draws or misses reported like the [ripgrep
backtest](../benchmarks/correctness.md) does for search.
