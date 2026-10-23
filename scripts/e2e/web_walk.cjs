// Walk a search in the web UI the way a person does, and print what the
// page shows, for tests/contract_walk.rs.
//
//     node scripts/e2e/web_walk.cjs http://127.0.0.1:8080 'needle' [regex|symbols|references]
//
// Opens /?q=QUERY in headless Chromium, renders every file group (they are
// added as the page scrolls), presses LOAD MORE until it goes away, then
// presses "Show N more hits in this file" on every group until none is
// left. Prints one JSON object: the results-count label and, per group in
// page order, its path, its count badge and the line of every hit it shows
// (0 for a file-name hit).
//
// Needs the `playwright` package (npm i -D playwright, or a global install
// on NODE_PATH) and its Chromium (npx playwright install chromium).

const { chromium } = require('playwright');

async function main() {
    const [base, query, mode] = process.argv.slice(2);
    if (!base || query === undefined) {
        console.error('usage: web_walk.cjs URL QUERY [regex|symbols|references]');
        process.exit(2);
    }
    const params = new URLSearchParams({ q: query });
    if (mode === 'regex') params.set('regex', 'true');
    if (mode === 'symbols') params.set('symbols', 'true');
    if (mode === 'references') params.set('references', 'true');

    const browser = await chromium.launch();
    try {
        const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
        const errors = [];
        page.on('pageerror', e => errors.push(String(e)));
        // A returning visitor: the first-visit theme picker (a modal over
        // the page) was answered before.
        await page.addInitScript(() => {
            try { localStorage.setItem('fcs-theme-asked', '1'); } catch (_) { /* no storage */ }
        });
        await page.goto(`${base}/?${params}`);
        await page.waitForSelector('#results .result-group, #results-count:not(:empty)', { timeout: 30000 });

        // Render every group: they are added as the sentinel scrolls into view.
        const renderAll = async () => {
            for (let i = 0; i < 1000; i++) {
                const pending = await page.evaluate(() => {
                    window.scrollTo(0, document.body.scrollHeight);
                    return !!document.querySelector('#results .render-sentinel, #results [data-sentinel]');
                });
                if (!pending) break;
                await page.waitForTimeout(20);
            }
        };
        await renderAll();

        // LOAD MORE until it is gone.
        for (let i = 0; i < 10000; i++) {
            const btn = await page.$('#load-more-btn');
            if (!btn) break;
            const before = await page.$$eval('#results .hit-num-open', els => els.length);
            await btn.click();
            await page.waitForFunction(
                n => !document.querySelector('#load-more-btn:disabled') &&
                    document.querySelectorAll('#results .hit-num-open').length !== n,
                before, { timeout: 30000 },
            ).catch(() => {});
            await renderAll();
        }

        // "Show N more hits in this file" on every group.
        for (let i = 0; i < 10000; i++) {
            const btn = await page.$('#results .group-more-btn:not(:disabled)');
            if (!btn) break;
            const path = await btn.getAttribute('data-file-path');
            await btn.click();
            await page.waitForFunction(
                p => ![...document.querySelectorAll('#results .group-more-btn')]
                    .some(b => b.dataset.filePath === p && b.textContent.includes('Loading')),
                path, { timeout: 30000 },
            ).catch(() => {});
            await page.waitForTimeout(20);
            await renderAll();
        }

        const shown = await page.evaluate(() => ({
            count: (document.getElementById('results-count') || {}).textContent || '',
            groups: [...document.querySelectorAll('#results .result-group')].map(g => ({
                path: g.dataset.filePath,
                badge: (g.querySelector('.group-count') || {}).textContent || '',
                lines: [...g.querySelectorAll('.hit-num-open')].map(b =>
                    b.textContent.trim() === 'file' ? 0 : Number(b.dataset.lineNumber)),
                more: !!g.querySelector('.group-more-btn'),
            })),
        }));
        shown.errors = errors;
        process.stdout.write(JSON.stringify(shown));
    } finally {
        await browser.close();
    }
}

main().catch(e => {
    console.error(e);
    process.exit(1);
});
