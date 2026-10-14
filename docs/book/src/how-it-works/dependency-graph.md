# How the import graph is built

This page explains how fcs turns Python and JavaScript/TypeScript imports
into edges between files, and how it decides that an import is **internal**
(an edge to a file in your project) or **external** (a package, which gets
no edge). [Dependency graph support](../reference/graph-support.md) is the
short reference: a table of which import forms are handled. This page is
the method behind that table.

## The rule that decides internal vs external

fcs has no list of package names, and it does not read `requirements.txt`,
`pyproject.toml`, `package.json` dependencies or a lockfile. (For
JavaScript and TypeScript it reads `tsconfig.json` and each indexed
`package.json`'s own `name` and entry points, described below.) The decision
is made by one rule, the same for every language:

> An import is **internal** when the language's own lookup rules, applied
> to the files fcs has indexed, find a file. Otherwise it is **unresolved**,
> and it gets no edge.

So "internal" means "resolves to an indexed file", and everything else
falls into one of two kinds of unresolved import:

| Kind | How fcs tells | Examples | What it means |
|---|---|---|---|
| **External** | Written as a package name (not as a path into the project), and no indexed file matches | `import os`, `import numpy`, `from django.db import models` when Django is not indexed, `import React from 'react'`, `lodash/merge`, `@scope/pkg` | The standard library or an installed package. This is expected and is not a problem. |
| **Local but unresolved** | Written as a path into the project (`.x`, `./x`, `../x`, `@/x`, `~/x`) and no indexed file matches | `from .missing import x`, `import './old-name'` | A broken import, a file that is excluded from the index, or a form fcs does not handle yet (see the support table) |

The test for "written as a path into the project" is purely about the text:
it starts with `.`, `@/`, `~/`, or Rust's `crate::`, `self::`, `super::`.
`import pkg.missing` written as an absolute Python name cannot be told
apart from a package, so it counts as external.

Two consequences follow from the rule.

- **What is indexed decides what is internal.** Third-party code is not
  indexed by default: `node_modules`, `venv`, `.venv`, `build` and `dist`
  are excluded. So installed packages are external. If you index a
  dependency's source tree on purpose (for example, Django next to your
  project), imports of it become internal edges, as long as the lookup
  rules below reach it.
- **fcs never guesses by name.** If `import utils` cannot be found by the
  lookup rules, fcs does not settle for "some file called `utils.py`
  somewhere in the repository". A missing edge is better than a wrong one,
  because a wrong edge silently corrupts "imported by N", ranking and the
  dependency explorer.

## The pipeline

Both languages go through the same three steps.

1. **Extract.** Each file is parsed with its tree-sitter grammar, in the
   same pass that extracts symbols. Every import statement becomes an
   *import string* (the module text as written) and the line it is on.
   Nothing is executed and no interpreter or compiler is run.
2. **Resolve.** The import string is turned into candidate file paths
   using the language's rules (below). A candidate counts only if it is an
   indexed file. The check is a lookup in the index's path table; the disk
   is not consulted. The first candidate that is indexed becomes the edge.
3. **Keep the rest.** An import that resolves to nothing is kept,
   together with its file and line. When a file is indexed later (a new
   file, a rename, a watcher event), the imports that mention its name are
   tried again. So an edge appears as soon as its target exists, and order
   of indexing does not matter.

## Python

### What is extracted

| Source | Import string(s) |
|---|---|
| `import a.b` | `a.b` |
| `import a, b as c` | `a`, `b` (one per module; the alias is ignored) |
| `from a.b import c, d` | `a.b.c`, `a.b.d` (one per name) |
| `from a import *` | `a` |
| `from . import x` | `.x` |
| `from ..pkg import x` | `..pkg.x` |

`from a.b import c` gives `a.b.c` because `c` may be a submodule
(`a/b/c.py`) or just a name defined in `a/b.py`. Resolution tries the
submodule first and falls back to the module, which is what Python does.
Imports inside functions, `if TYPE_CHECKING:` blocks and `try:` blocks are
extracted like any other. `importlib.import_module("x")` and `__import__`
are not, because their argument is a runtime value.

### Relative imports (`from .x import y`)

These are always internal or local-but-unresolved, never external.

1. The dots choose the base folder. One dot is the importing file's own
   folder (its package), and each extra dot goes one folder up.
2. Under that base, fcs looks for the dotted name as a module or package,
   longest first. For `.a.b` it tries `a/b.py`, then `a/b/__init__.py`,
   then `a.py`, then `a/__init__.py`.
3. A match must cover the whole name, or all of it but the last part
   (the last part may be a name defined in the module). Anything shorter is
   not accepted, because Python would fail that import too.
4. `from . import name`, where `name` is not a module, resolves to the
   package's own `__init__.py`.

```text
pkg/__init__.py
pkg/api.py           from .models import User   -> pkg/models.py
pkg/models.py        from . import api          -> pkg/api.py
pkg/sub/views.py     from ..models import User  -> pkg/models.py
                     from .missing import x     -> unresolved (local)
```

### Absolute imports (`import a.b`, `from a import b`)

Python 3 looks for an absolute import only on `sys.path`, never next to the
importing file. fcs does not know your `sys.path`, so it works out the
likely entries from the folder layout.

1. **Find the roots.** Walk up from the importing file's folder, all the
   way up, however deep the file sits. A folder that contains `__init__.py` is part of a package, so it
   is *not* a root. Every other folder is a candidate root, and so is its
   `src/` subfolder (the "src layout").
2. **Look under each root** for the dotted name, longest first, exactly as
   for relative imports: `a/b.py`, `a/b/__init__.py`, `a.py`,
   `a/__init__.py`. The same "whole name, or all but the last part" rule
   applies.
3. **Pick the best root.** A root where the whole name exists wins
   immediately. Otherwise the root that matches the most of the name wins,
   and the nearest root wins a tie.
4. **Nothing found means external.** `import os`, `import requests` and
   `from django.db import models` find nothing in the index (the standard
   library and site-packages are not under any of these roots), so they
   resolve to nothing.

```text
myproject/                  (no __init__.py: a root)
  src/shop/__init__.py
  src/shop/cart.py          from shop.models import Item  -> src/shop/models.py
  src/shop/models.py        import json                   -> external (stdlib)
  src/shop/json.py
  tests/test_cart.py        from shop.cart import Cart    -> src/shop/cart.py
```

`import json` in `src/shop/models.py` does not resolve to the
`src/shop/json.py` next to it. `src/shop/` is a package, so it is not on
`sys.path`, and Python would load the standard library's `json`. A
`tests/test_cart.py` that imports `shop` finds it through the
`myproject/src/` root.

### Python limits

- Only the folder layout is used. `sys.path` entries from `pyproject.toml`,
  `setup.cfg`, `PYTHONPATH`, `.pth` files or editable installs are not
  read, so a package that lives somewhere these rules do not reach is
  external to fcs.
- Namespace packages (folders without `__init__.py`) work when they sit
  under a root, because each segment is a folder name.
- `.pyi` stub files are indexed and their imports are extracted, but they
  are never the target of an import.
- A name that exists both in your project and in the standard library
  (say a top-level `logging.py` in a root) resolves to your file, as it
  would for Python with that root first on `sys.path`.

## JavaScript and TypeScript

### What is extracted

| Source | Import string |
|---|---|
| `import x from './a'`, `import './a'`, `import type { T } from './a'` | `./a` |
| `export { x } from './a'`, `export * from './a'` | `./a` |
| `require('./a')`, `` require(`./a`) `` | `./a` |
| `import('./a')`, `import(/* chunk */ './a')` | `./a` |
| `import data from './a.json' with { type: 'json' }` (or `assert`) | `./a.json` |

A template string with `${…}`, or any argument that is not a string
literal, is a runtime value and is skipped. TypeScript's older
`import x = require('./a')` is extracted as `./a`.

### How a specifier is classified

The specifier's first characters decide which lookups apply, in this
order:

| Specifier | Lookup | If nothing is found |
|---|---|---|
| `./x`, `../x`, `.`, `..` | Relative to the importing file's folder | Local but unresolved |
| anything else | 1. The nearest `tsconfig.json` (or `jsconfig.json`): its `paths` (an exact pattern, else the wildcard pattern with the longest prefix, each target in order), then its `baseUrl`, with relative `extends` applied, as `tsc` does | (next step) |
| | 2. A workspace package: an indexed `package.json` whose `name` is the package part (`@acme/shared` of `@acme/shared/money`). The bare name goes to its entry point (`types`, `typings`, `source`, `module`, `main`, then `exports["."]`), then `src/index`, then `index`; a subpath goes to `src/money`, then `money` | (next step) |
| | 3. `@/x`, `~/x`: from the nearest parent folder with a `package.json` or `tsconfig.json` (else the repository root): `src/x` first, then `x` | Local but unresolved |
| | 4. When no tsconfig sets `baseUrl` or `paths`: the repository root (the folder holding `.git`) as `baseUrl`, so `components/Button` finds `<repo>/components/Button.tsx` | External |

Each step only counts when it finds an indexed file, so `react` and
`lodash/merge` stay external: no tsconfig alias, no indexed `package.json`
and no file under the repository root carries that name. `package.json`
`imports` (`#name`) are not read yet and show up as missing edges.

tsconfig, `package.json` and `.git` are read from disk (once per folder,
and again when a `tsconfig*.json` or `package.json` is re-indexed), so a
tsconfig excluded from the index still applies. The graph is built when
files are indexed: after changing `paths` or a package's `name`, re-index.

### Finding the file

Given the folder path a specifier points at, fcs tries, in order:

1. **The exact file**, when the specifier has an extension.
2. **The TypeScript extension swap.** In TypeScript ESM code you write the
   extension of the compiled output, so `./a.js` can mean `a.ts` or
   `a.tsx`, `./a.jsx` can mean `a.tsx` or `a.ts`, `./a.mjs` means `a.mts`
   and `./a.cjs` means `a.cts`.
3. **Appended extensions**: `.ts .tsx .js .jsx .mjs .cjs .mts .cts .d.ts
   .json`, in that order.
4. **A folder's index file**: `index` with each of the extensions in 3.

```text
web/
  package.json
  src/app.tsx         import { api } from './api'      -> src/api.ts
                      import Button from '@/ui/Button'  -> src/ui/Button/index.tsx
                      import { money } from '@acme/shared' -> packages/shared/src/index.ts
                      import './styles.css'             -> src/styles.css (if indexed)
                      import React from 'react'         -> external
  src/api.ts          export * from './types.js'        -> src/types.ts
```

Imports of stylesheets, `.vue` or `.svelte` components and other assets
resolve when that file is indexed, because a bundler resolves them and
they are real dependencies. TypeScript itself does not resolve them.

### JavaScript and TypeScript limits

- `package.json` `imports` (`#name`), `exports` subpath maps and nested
  `exports` conditions are not read; `rootDirs` is not applied.
- A tsconfig `extends` that names an npm package (`@tsconfig/node20`) is
  skipped; relative `extends` (a string or an array) are followed.
- Aliases defined only in a bundler config (webpack, Vite, Jest) are not
  read. `@/` and `~/` follow the common convention; other bundler-only
  aliases show up as missing edges.

## Seeing what did not resolve

- The dependency explorer's folder map says how many imports written as
  project paths resolve to no file.
- `GET /api/graph/modules` returns `unresolved_imports` (all of them,
  packages included) and `unresolved_local` (the ones written as project
  paths).
- `GET /api/graph/imports?path=…` lists every import of a file with its
  line; unresolved ones have `target: null`.
- MCP's `dependency_overview` gives both counts, and `file_dependencies`
  lists a file's unresolved imports.

A large `unresolved_local` count usually means a form from the limits
above (a bundler-only alias, for example) or files excluded from the index.

## How the method is checked

Rules like these are easy to get subtly wrong, so fcs compares its graph
with each language's own resolver, an **oracle** that knows nothing about
fcs.

| Language | Oracle | How it runs |
|---|---|---|
| Python | [grimp](https://github.com/python-grimp/grimp), the import-graph library behind import-linter | Every package directly under the folder (or its `src/`) is given to grimp with that folder on `sys.path`. grimp parses imports statically and resolves them as Python would. External packages are left out. |
| JavaScript, TypeScript | TypeScript's own `ts.resolveModuleName` | Every JS/TS file outside `node_modules`, `dist` and similar folders is pre-processed with `ts.preProcessFile`, and each import is resolved under the nearest `tsconfig.json` or `jsconfig.json`, with `extends`, `paths` and `baseUrl` applied (Bundler resolution when there is no config). A target in `node_modules` or outside the folder counts as external. |

The comparison (`src/check/graph.rs`) works on edges between files:

- Only files the oracle analysed are compared.
- An edge the oracle draws and fcs does not is **missing**. An edge fcs
  draws and the oracle does not is **extra**, which means a wrong edge.
- Missing edges are grouped by the shape of the import (relative,
  `from pkg import name`, `@/` alias, bare name…), so each group points at
  one row of the support table.
- Edges from TypeScript files to stylesheets, components and assets are
  counted separately and not judged, since TypeScript cannot resolve them.
- Package-style imports that fcs links and the oracle leaves unresolved
  are listed separately and not counted as extra. The oracle runs without
  an install, so it cannot follow a workspace package's `node_modules`
  link; fcs links it to the package's source. The repository-root fallback
  lands here too.

CI runs both oracles on pinned public repositories and fails if a change
adds a missing or extra edge:

| Language | Repository | Result |
|---|---|---|
| Python | Django 5.2 (grimp 3.17) | 3,041 of 3,041 edges, none extra |
| TypeScript | Vite 6.3.5 (TypeScript 5.9) | 7 missing (4 `package.json` `imports`, 3 relative imports TypeScript resolves through a folder's `package.json` or, where `a.js` and `a.ts` both exist, to `a.ts`), 1 extra; 221 workspace-package edges listed apart |

To run the same comparison on your own code, run `fcs check` (see
[Correctness against ripgrep](../benchmarks/correctness.md#on-your-own-code-fcs-check)).
It uses grimp if `pip install grimp` has been done, and TypeScript if it is
in the repository's `node_modules`. It reports missing and extra edges
with examples, but does not fail on them: they are either a known gap from
the limits above or a bug worth reporting.
