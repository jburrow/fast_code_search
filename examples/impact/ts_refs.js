// TypeScript language service oracle: files that reference each definition.
// node refs.js ROOT defs.jsonl out.jsonl   (defs rows: {file, line, name}, 0-based line)
const ts = require('typescript');
const fs = require('fs');
const path = require('path');
const [root, defsPath, outPath] = process.argv.slice(2);
const cfgPath = path.join(root, 'tsconfig.json');
const cfg = ts.readConfigFile(cfgPath, ts.sys.readFile);
const parsed = ts.parseJsonConfigFileContent(cfg.config, ts.sys, root);
parsed.options.types = [];
const files = parsed.fileNames;
const versions = {};
const host = {
  getScriptFileNames: () => files,
  getScriptVersion: f => '1',
  getScriptSnapshot: f => fs.existsSync(f) ? ts.ScriptSnapshot.fromString(fs.readFileSync(f, 'utf8')) : undefined,
  getCurrentDirectory: () => root,
  getCompilationSettings: () => parsed.options,
  getDefaultLibFileName: o => ts.getDefaultLibFilePath(o),
  fileExists: ts.sys.fileExists, readFile: ts.sys.readFile, readDirectory: ts.sys.readDirectory,
  directoryExists: ts.sys.directoryExists, getDirectories: ts.sys.getDirectories,
};
const ls = ts.createLanguageService(host, ts.createDocumentRegistry());
const out = fs.createWriteStream(outPath);
const defs = fs.readFileSync(defsPath, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
console.error(`${files.length} files in program, ${defs.length} definitions`);
let i = 0;
for (const d of defs) {
  const abs = path.join(root, d.file);
  const sf = ls.getProgram().getSourceFile(abs);
  if (!sf) { out.write(JSON.stringify({...d, error: 'not in program'}) + '\n'); continue; }
  const lineText = sf.text.split('\n')[d.line] || '';
  const col = lineText.search(new RegExp(`\\b${d.name.replace(/[$]/g, '\\$')}\\b`));
  if (col < 0) { out.write(JSON.stringify({...d, error: 'name not on line'}) + '\n'); continue; }
  const pos = sf.getPositionOfLineAndCharacter(d.line, col);
  const t = Date.now();
  let users = new Set();
  try {
    const refs = ls.findReferences(abs, pos) || [];
    for (const r of refs) for (const e of r.references) {
      if (e.isDefinition) continue;
      users.add(path.relative(root, e.fileName));
    }
  } catch (e) { out.write(JSON.stringify({...d, error: String(e).slice(0, 200)}) + '\n'); continue; }
  out.write(JSON.stringify({...d, users: [...users], ms: Date.now() - t}) + '\n');
  if (++i % 20 === 0) console.error(`${i}/${defs.length}`);
}
out.end();
