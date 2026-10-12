// Records the feature demos in docs/images/demos/ from a running server.
//
//   scripts/docs/record-demos.sh            # builds, indexes this repo, records, converts
//   node scripts/docs/record-demos.mjs OUT  # just the recording, against FCS_URL
//
// Each demo is a Playwright video (WebM) of a scripted session. A drawn cursor
// stands in for the real one, which videos do not capture. Demos only ever
// index this repository, so they show no one else's code.

// require() rather than import so NODE_PATH can point at a global install.
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const { chromium } = createRequire(import.meta.url)('playwright');

const BASE = process.env.FCS_URL || 'http://127.0.0.1:8080';
const OUT = process.argv[2] || 'demos-raw';
const ONLY = process.argv[3];
const SIZE = { width: 1280, height: 760 };
// The terminal reads better smaller and wider than tall.
const SIZES = { cli: { width: 1100, height: 600 } };

const CURSOR = `
  (() => {
    if (document.getElementById('__demo_cursor')) return;
    const c = document.createElement('div');
    c.id = '__demo_cursor';
    c.style.cssText = 'position:fixed;z-index:2147483647;pointer-events:none;width:22px;height:22px;' +
      'left:0;top:0;transform:translate(-3px,-2px);transition:transform .08s';
    c.innerHTML = '<svg width="22" height="22" viewBox="0 0 22 22"><path d="M3 2l14 9-6.5 1.2L7 19z" ' +
      'fill="#111" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>';
    const add = () => document.body.appendChild(c);
    document.body ? add() : document.addEventListener('DOMContentLoaded', add);
    addEventListener('mousemove', e => { c.style.left = e.clientX + 'px'; c.style.top = e.clientY + 'px'; }, true);
    addEventListener('mousedown', () => { c.style.transform = 'translate(-3px,-2px) scale(.8)'; }, true);
    addEventListener('mouseup', () => { c.style.transform = 'translate(-3px,-2px)'; }, true);
  })();`;

const wait = ms => new Promise(r => setTimeout(r, ms));
let contextStart = 0;
let readyAt;
let mouse = { x: SIZE.width / 2, y: SIZE.height / 2 };

async function moveTo(page, target, opts = {}) {
  const loc = typeof target === 'string' ? page.locator(target).first() : target;
  await loc.scrollIntoViewIfNeeded();
  const box = await loc.boundingBox();
  if (!box) throw new Error(`no box for ${target}`);
  const x = box.x + (opts.dx ?? box.width / 2);
  const y = box.y + (opts.dy ?? box.height / 2);
  await page.mouse.move(x, y, { steps: opts.steps ?? 18 });
  mouse = { x, y };
}

async function click(page, target, opts = {}) {
  await moveTo(page, target, opts);
  await wait(250);
  await page.mouse.down();
  await wait(90);
  await page.mouse.up();
  await wait(opts.after ?? 600);
}

async function type(page, text, delay = 85) {
  for (const ch of text) {
    await page.keyboard.type(ch);
    await wait(delay);
  }
}

// Opens a page and waits for the "indexing complete" bar to go.
async function open(page, url) {
  await page.goto(`${BASE}${url}`);
  await page.waitForSelector('#progress-panel', { state: 'hidden', timeout: 8000 }).catch(() => {});
  await wait(400);
  // The video starts with the context; the GIF starts here.
  readyAt ??= (Date.now() - contextStart) / 1000;
}


// ── Terminal demo ────────────────────────────────────────────────────────
// A page styled as a terminal; each command's real output (run against the
// same server) is converted from ANSI colours to HTML and printed after the
// command is "typed".

const FCS = process.env.FCS_BIN || path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../target/release/fcs');

function runFcs(args, pipe) {
  const env = { ...process.env, FCS_SERVER: BASE };
  const r = spawnSync(FCS, ['--color', 'always', ...(pipe ? [] : ['--format', 'grouped']), ...args], { env, encoding: 'utf8' });
  let out = r.stdout;
  if (pipe) out = out.split('\n').slice(0, pipe).join('\n') + '\n';
  return out + (r.stderr ? `\x1b[2m${r.stderr.trim()}\x1b[0m\n` : '');
}

function ansiToHtml(text) {
  const esc = t => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  let html = '', open = 0;
  for (const part of text.split(/(\x1b\[[0-9;]*m)/)) {
    const m = part.match(/^\x1b\[([0-9;]*)m$/);
    if (!m) { html += esc(part); continue; }
    const codes = m[1].split(';').filter(Boolean);
    if (!codes.length || codes.includes('0')) { html += '</span>'.repeat(open); open = 0; }
    const cls = codes.filter(c => c !== '0').map(c => `a${c}`).join(' ');
    if (cls) { html += `<span class="${cls}">`; open++; }
  }
  return html + '</span>'.repeat(open);
}

const TERMINAL = `<!doctype html><meta charset="utf-8"><style>
  @import url('${BASE}/fonts.css');
  html,body{margin:0;height:100%;background:#fff900}
  .win{margin:28px;height:calc(100% - 56px);background:#1d1c0f;border:2px solid #000;box-shadow:6px 6px 0 #000;display:flex;flex-direction:column}
  .bar{height:30px;display:flex;align-items:center;padding:0 12px;gap:8px;border-bottom:1px solid #3a382a;color:#9a977c;font:13px 'JetBrains Mono',monospace}
  .dot{width:11px;height:11px;border-radius:50%;background:#5f5d48}
  .bar span:last-child{margin-left:auto;margin-right:auto}
  pre{margin:0;padding:16px 20px;flex:1;overflow:hidden;color:#e8e5c8;font:16px/1.45 'JetBrains Mono',monospace;white-space:pre-wrap}
  .p{color:#fff900}.cmd{color:#fff}.caret{background:#fff900;color:#1d1c0f}
  .a1{font-weight:700}.a2{color:#8f8c72}.a31{color:#1d1c0f;background:#fff900}.a35{color:#c9a7ff}.a32{color:#9be39b}.a33{color:#ffd76a}.a36{color:#7fd8e6}
</style><div class="win"><div class="bar"><i class="dot"></i><i class="dot"></i><i class="dot"></i><span>fcs</span></div><pre id="t"></pre></div>`;

async function terminal(page, steps) {
  await page.setContent(TERMINAL);
  await page.evaluate(() => document.fonts.ready);
  readyAt ??= (Date.now() - contextStart) / 1000;
  await wait(500);
  for (const step of steps) {
    await page.evaluate(() => {
      const t = document.getElementById('t');
      t.insertAdjacentHTML('beforeend', '<span class="p">$ </span><span class="cmd"></span><span class="caret"> </span>');
    });
    for (const ch of step.show) {
      await page.evaluate(c => { const cmds = document.querySelectorAll('#t .cmd'); cmds[cmds.length - 1].textContent += c; }, ch);
      await wait(55);
    }
    await wait(350);
    const html = ansiToHtml(runFcs(step.args, step.pipe));
    await page.evaluate(h => {
      const t = document.getElementById('t');
      t.querySelector('.caret').remove();
      t.insertAdjacentHTML('beforeend', '\n' + h + '\n');
      t.scrollTop = t.scrollHeight;
    }, html);
    await wait(step.hold ?? 2600);
    if (step.clear) await page.evaluate(() => { document.getElementById('t').innerHTML = ''; });
  }
}

const demos = {
  // The command-line client: phrase, definitions, references, regex, files only.
  async cli(page) {
    await terminal(page, [
      { show: `fcs '"fn main"' -n 3`, args: ['"fn main"', '-n', '3'] },
      { show: 'fcs symbols SearchEngine -n 2', args: ['symbols', 'SearchEngine', '-n', '2'], clear: true },
      { show: 'fcs refs parse_query -n 3', args: ['refs', 'parse_query', '-n', '3'] },
      { show: `fcs -e 'fn \\w+_trigrams?\\(' -n 3`, args: ['-e', 'fn \\w+_trigrams?\\(', '-n', '3'], clear: true },
      { show: `fcs -l -g '*.rs' TODO | head -3`, args: ['-l', '-g', '*.rs', 'TODO'], pipe: 3, hold: 3200 },
    ]);
  },

  // Search as you type, ranked: definitions first, then the file viewer.
  async search(page) {
    await open(page, '/');
    await click(page, '#query');
    await type(page, 'TrigramIndex');
    await wait(2600);
    await click(page, '#results .view-file-btn', { after: 2600 });
    await page.mouse.wheel(0, 300);
    await wait(1400);
    await page.keyboard.press('Escape');
    await wait(500);
    await click(page, '#query');
    await page.keyboard.press('ControlOrMeta+A');
    await type(page, '"fn main"');
    await wait(2800);
  },

  // Symbols and references modes for one identifier.
  async references(page) {
    await open(page, '/');
    await click(page, '#query');
    await type(page, 'SearchEngine');
    await wait(1800);
    await click(page, 'label:has(#symbols-mode)', { after: 2400 });
    await click(page, 'label:has(#references-mode)', { after: 2600 });
    await page.mouse.wheel(0, 320);
    await wait(1600);
  },

  // Regex: the cheat-sheet, a broken pattern and a search that finds nothing.
  async regex(page) {
    await open(page, '/');
    await click(page, 'label:has(#regex-mode)', { after: 500 });
    await click(page, '#regex-help-btn', { after: 1800 });
    await click(page, '#regex-help [data-example^="fn "]', { after: 2400 });
    await click(page, '#regex-help-close', { after: 500 });
    await page.locator('#query').focus();
    await page.keyboard.press('ControlOrMeta+A');
    await type(page, 'Some(config');
    await page.keyboard.press('Enter');
    await wait(1800);
    await moveTo(page, '#results .suggestion-btn, #results button:has-text("Escape")');
    await wait(1400);
    await page.locator('#query').focus();
    await page.keyboard.press('ControlOrMeta+A');
    // Split so this file does not match its own query once it is indexed.
    await type(page, 'pub struct ' + 'search' + 'engine');
    await page.keyboard.press('Enter');
    await wait(2200);
    await click(page, '.suggestion-btn', { after: 2600 });
  },

  // Dependency explorer: neighbourhood, a neighbour's source, impact.
  async graph(page) {
    await open(page, '/graph.html');
    await click(page, '#filter');
    await type(page, 'search/engine/mod');
    await wait(700);
    await click(page, '#files > *', { after: 2400 });
    await click(page, '.gnode[aria-label="src/search/ranking.rs"], .gnode[data-key*="ranking"]', { after: 2400 });
    await click(page, 'button[data-mode="impact"]', { after: 3200 });
  },
};

fs.mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch({
  executablePath: fs.existsSync('/opt/pw-browsers/chromium') ? '/opt/pw-browsers/chromium' : undefined,
});
for (const [name, run] of Object.entries(demos)) {
  if (ONLY && ONLY !== name) continue;
  const dir = path.join(OUT, `${name}.tmp`);
  fs.rmSync(dir, { recursive: true, force: true });
  const size = SIZES[name] || SIZE;
  const context = await browser.newContext({ viewport: size, recordVideo: { dir, size }, colorScheme: 'light' });
  contextStart = Date.now();
  readyAt = undefined;
  await context.addInitScript(CURSOR);
  const page = await context.newPage();
  await page.mouse.move(mouse.x, mouse.y);
  try {
    await run(page);
  } catch (e) {
    console.error(`${name}: ${e.message}`);
  }
  await page.screenshot({ path: path.join(OUT, `${name}.png`) });
  await context.close();
  const video = fs.readdirSync(dir).find(f => f.endsWith('.webm'));
  fs.renameSync(path.join(dir, video), path.join(OUT, `${name}.webm`));
  fs.rmSync(dir, { recursive: true, force: true });
  fs.writeFileSync(path.join(OUT, `${name}.start`), `${(readyAt ?? 0).toFixed(2)}\n`);
  console.log(`recorded ${name}`);
}

// The still screenshot at the top of the README and the Web UI page.
if (!ONLY || ONLY === 'stills') {
  const context = await browser.newContext({ viewport: { width: 1360, height: 860 }, deviceScaleFactor: 2 });
  const page = await context.newPage();
  await page.goto(`${BASE}/?q=trigram`);
  await page.waitForSelector('#results .result-group');
  await page.waitForSelector('#progress-panel', { state: 'hidden', timeout: 8000 }).catch(() => {});
  await page.mouse.move(0, 0);
  await wait(500);
  await page.screenshot({ path: path.join(OUT, 'web-ui.png') });
  await context.close();
  console.log('captured web-ui.png');
}
await browser.close();
