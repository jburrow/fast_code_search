// Write a TypeScript workspace monorepo laid out the way real ones are, for
// the graph oracle (examples/graph_oracle.rs, CI workflow "graph oracle").
//
//   node scripts/graph-oracle/ts_layouts.mjs OUT [--seed N]
//
// Vite is one such repository, pinned; this one is generated, so every
// shape below is there whatever Vite does next:
//
// - npm workspaces (`packages/*`, `apps/*`), each package with a
//   `package.json` name (`@acme/ui`) and installed into `node_modules` as
//   a symlink, as `npm install` leaves it;
// - a root `tsconfig.base.json` with `baseUrl` and `paths` (`@acme/x`,
//   `@acme/x/*`), which each project's `tsconfig.json` `extends`, and an
//   app adding its own `paths` alias (`~/x`);
// - barrels (`export * from './a'`, `export { b } from './b'`), `import
//   type`, dynamic `import()`, `.mts`/`.cts` files imported as `.mjs`/
//   `.cjs`, `require` in CommonJS, JSON imports, folder `index` files;
// - forms fcs does not handle, on purpose, so they stay listed gaps:
//   `package.json` `imports` (`#internal/x`), `exports` subpaths
//   (`@acme/ui/theme`), and a folder whose own `package.json` names its
//   entry (`import './legacy'`).
//
// Modules import each other at random (seeded), within a package by
// relative path and across packages by name.

import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
let seed = 1;
const si = args.indexOf("--seed");
if (si >= 0) {
  seed = Number(args[si + 1]);
  args.splice(si, 2);
}
const out = path.resolve(args[0]);

// Mulberry32: small and the same on every platform.
let state = seed >>> 0;
function rand() {
  state = (state + 0x6d2b79f5) >>> 0;
  let t = state;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const below = (n) => Math.floor(rand() * n);
const pick = (xs) => xs[below(xs.length)];

function write(file, text) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
}
const json = (v) => JSON.stringify(v, null, 2) + "\n";

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(path.join(out, ".git"), { recursive: true });

const packages = ["ui", "data", "utils", "auth", "billing"];
const apps = ["web", "api"];
write(
  path.join(out, "package.json"),
  json({ name: "acme", private: true, workspaces: ["packages/*", "apps/*"] }),
);
const paths = {};
for (const p of packages) {
  paths[`@acme/${p}`] = [`packages/${p}/src`];
  paths[`@acme/${p}/*`] = [`packages/${p}/src/*`];
}
write(
  path.join(out, "tsconfig.base.json"),
  json({
    compilerOptions: {
      target: "es2022",
      module: "esnext",
      moduleResolution: "bundler",
      allowJs: true,
      resolveJsonModule: true,
      baseUrl: ".",
      paths,
    },
  }),
);

// Modules per project: { project: [{ file, spec(fromFile) }] }
const modules = {};
function addModule(project, rel, body = "") {
  const file = path.join(out, project, rel);
  (modules[project] ??= []).push(file);
  write(file, `// ${project}/${rel}\nexport const v_${modules[project].length} = 1;\n${body}`);
  return file;
}

function relSpec(from, to) {
  let r = path.relative(path.dirname(from), to).split(path.sep).join("/");
  if (!r.startsWith(".")) r = "./" + r;
  // Written the way TypeScript code imports them.
  r = r.replace(/\.mts$/, ".mjs").replace(/\.cts$/, ".cjs");
  if (rand() < 0.5) r = r.replace(/\.(ts|tsx)$/, "");
  else r = r.replace(/\.(ts|tsx)$/, ".js");
  return r.replace(/\/index(\.js)?$/, "");
}

for (const p of packages) {
  const dir = `packages/${p}`;
  write(
    path.join(out, dir, "package.json"),
    json({
      name: `@acme/${p}`,
      version: "1.0.0",
      main: "src/index.ts",
      types: "src/index.ts",
      ...(p === "ui"
        ? { exports: { ".": "./src/index.ts", "./theme": "./src/theme/main.ts" } }
        : {}),
      ...(p === "data" ? { imports: { "#internal/*": "./src/internal/*.ts" } } : {}),
    }),
  );
  write(path.join(out, dir, "tsconfig.json"), json({ extends: "../../tsconfig.base.json" }));
  for (let i = 0; i < 20 + below(20); i++) {
    const sub = pick(["", "lib/", "components/", "lib/deep/"]);
    const ext = pick([".ts", ".ts", ".tsx", ".mts", ".cts", ".js"]);
    addModule(dir, `src/${sub}m${i}${ext}`);
  }
  addModule(dir, `src/lib/index.ts`);
  if (p === "ui") addModule(dir, "src/theme/main.ts");
  if (p === "data") addModule(dir, "src/internal/cache.ts");
  write(path.join(out, dir, "src/config.json"), json({ name: p }));
}
for (const a of apps) {
  const dir = `apps/${a}`;
  write(path.join(out, dir, "package.json"), json({ name: `@acme/app-${a}`, private: true }));
  write(
    path.join(out, dir, "tsconfig.json"),
    json({
      extends: "../../tsconfig.base.json",
      compilerOptions: { paths: { ...paths, "~/*": [`apps/${a}/src/*`] } },
    }),
  );
  for (let i = 0; i < 30 + below(20); i++) {
    const sub = pick(["", "pages/", "routes/", "lib/"]);
    const ext = a === "web" ? pick([".tsx", ".ts"]) : pick([".ts", ".mts", ".cts"]);
    addModule(dir, `src/${sub}m${i}${ext}`);
  }
  // A folder whose own package.json names its entry.
  write(path.join(out, dir, "src/legacy/package.json"), json({ main: "./entry.js" }));
  addModule(dir, "src/legacy/entry.js");
}

// Installed workspaces, as npm leaves them.
for (const p of packages) {
  const link = path.join(out, "node_modules/@acme", p);
  fs.mkdirSync(path.dirname(link), { recursive: true });
  fs.symlinkSync(path.join("..", "..", "packages", p), link, "dir");
}

// Barrels: each package's index re-exports some of its modules.
for (const p of packages) {
  const dir = `packages/${p}`;
  const index = path.join(out, dir, "src/index.ts");
  const own = modules[dir].filter((f) => !/\.cts$/.test(f));
  const lines = [];
  for (const f of own.slice(0, 4)) {
    const spec = relSpec(index, f);
    lines.push(rand() < 0.5 ? `export * from '${spec}';` : `export { v_1 as x${lines.length} } from '${spec}';`);
  }
  write(index, lines.join("\n") + "\n");
}

function importLine(from, spec, n) {
  const cjs = /\.cts$/.test(from);
  if (cjs) return `const r${n} = require('${spec}');`;
  const forms = [
    `import { v_1 as i${n} } from '${spec}';`,
    `import * as i${n} from '${spec}';`,
    `const i${n} = await import('${spec}');`,
    `export * from '${spec}';`,
  ];
  if (/\.(ts|tsx|mts)$/.test(from)) forms.push(`import type { T${n} } from '${spec}';`);
  return pick(forms);
}

let n = 0;
for (const [project, files] of Object.entries(modules)) {
  const isApp = project.startsWith("apps/");
  for (const from of files) {
    const lines = [];
    for (let k = 0; k < 1 + below(4); k++) {
      const choice = below(10);
      if (choice < 5) {
        const to = pick(files);
        if (to !== from) lines.push(importLine(from, relSpec(from, to), n++));
      } else if (choice < 8) {
        const p = pick(packages);
        const spec = pick([`@acme/${p}`, `@acme/${p}/lib`, `@acme/${p}/config.json`]);
        lines.push(importLine(from, spec, n++));
      } else if (isApp && choice === 8) {
        const to = pick(files);
        const rel = path.relative(path.join(out, project, "src"), to).split(path.sep).join("/");
        lines.push(importLine(from, "~/" + rel.replace(/\.(ts|tsx)$/, ""), n++));
      } else if (choice === 9) {
        // The forms fcs does not handle, now and then.
        const odd = pick([
          isApp ? "./legacy" : null,
          project === "packages/data" ? "#internal/cache" : null,
          "@acme/ui/theme",
        ]);
        if (odd && !(odd === "./legacy" && path.dirname(from) !== path.join(out, project, "src"))) {
          lines.push(importLine(from, odd, n++));
        }
      }
    }
    fs.appendFileSync(from, lines.join("\n") + "\n");
  }
}
