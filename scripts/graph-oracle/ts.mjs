// TypeScript's own answer to "what does each import resolve to?", for the
// dependency-graph oracle (examples/graph_oracle.rs).
//
//   node scripts/graph-oracle/ts.mjs ROOT > ts-edges.json
//
// For every JS/TS file under ROOT it lists the imports TypeScript sees
// (ts.preProcessFile: import/export from, require, import(), import x =
// require) and resolves each with ts.resolveModuleName under the nearest
// tsconfig.json or jsconfig.json (`extends`, `paths` and `baseUrl`
// included). With no config it uses Bundler resolution. Imports that land
// in node_modules or outside ROOT count as unresolved: fcs never indexes
// them either. Output: {"files": [...], "edges": [{from, line, spec, to}]}
// with absolute paths; "files" are the files analysed.

import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const root = fs.realpathSync(process.argv[2] ?? ".");
const SKIP = new Set(["node_modules", ".git", "dist", "build", "target", "out", "coverage"]);
const EXT = /\.(ts|tsx|js|jsx|mjs|cjs|mts|cts)$/;

function* walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) {
      if (!SKIP.has(e.name)) yield* walk(path.join(dir, e.name));
    } else if (e.isFile() && EXT.test(e.name) && !e.name.endsWith(".d.ts")) {
      yield path.join(dir, e.name);
    }
  }
}

const DEFAULTS = {
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  module: ts.ModuleKind.ESNext,
};
const configCache = new Map();

/** Compiler options from the nearest tsconfig.json / jsconfig.json. */
function optionsFor(file) {
  let dir = path.dirname(file);
  const seen = [];
  for (;;) {
    if (configCache.has(dir)) break;
    seen.push(dir);
    const cfg = ["tsconfig.json", "jsconfig.json"]
      .map((n) => path.join(dir, n))
      .find((p) => fs.existsSync(p));
    if (cfg) {
      const parsed = ts.getParsedCommandLineOfConfigFile(cfg, {}, {
        ...ts.sys,
        onUnRecoverableConfigFileDiagnostic: () => {},
      });
      configCache.set(dir, parsed ? parsed.options : DEFAULTS);
      break;
    }
    const up = path.dirname(dir);
    if (up === dir || !dir.startsWith(root)) {
      configCache.set(dir, DEFAULTS);
      break;
    }
    dir = up;
  }
  const found = configCache.get(dir);
  for (const d of seen) configCache.set(d, found);
  // fcs resolves JS and JSON targets too; let TypeScript see them.
  return { ...found, allowJs: true, resolveJsonModule: true };
}

const files = [];
const out = [];
for (const file of walk(root)) {
  files.push(file);
  const text = fs.readFileSync(file, "utf8");
  const info = ts.preProcessFile(text, true, true);
  const options = optionsFor(file);
  for (const imp of info.importedFiles) {
    const spec = imp.fileName;
    const line = text.slice(0, imp.pos).split("\n").length;
    const r = ts.resolveModuleName(spec, file, options, ts.sys).resolvedModule;
    let to = null;
    if (r && !r.isExternalLibraryImport) {
      const real = fs.realpathSync(r.resolvedFileName);
      if (real.startsWith(root + path.sep) && !real.includes(`${path.sep}node_modules${path.sep}`)) {
        to = real;
      }
    }
    out.push({ from: file, line, spec, to });
  }
}
process.stdout.write(JSON.stringify({ files, edges: out }));
