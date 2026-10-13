// One code viewer for the search page's file viewer and hover preview and
// the dependency explorer's source pane: line numbers, syntax and query
// highlighting, a window of lines with "Show more" at either edge, and a
// ruler beside the scrollbar marking every hit in the whole file (click a
// mark to jump there). Uses escapeHtml (common.js) and the pure helpers in
// lib/keyword-helpers.js (splitHighlightedHtml, rulerMarks, nearestLine,
// windowFor), which have Node tests.

// Syntax-highlighting very large files is slow; past this they show plain.
const HLJS_MAX_CHARS = 2_000_000;

/**
 * Highlight `text` once for `lang` and return an escaped HTML string per
 * line. Falls back to plain escaped lines when hljs is missing, the language
 * is unknown, the file is very large, or highlighting throws.
 * @returns {string[]}
 */
function highlightLines(text, lang) {
    const plain = () => text.split('\n').map(escapeHtml);
    if (typeof hljs === 'undefined' || text.length > HLJS_MAX_CHARS) return plain();
    const language = lang && lang !== 'plaintext' && hljs.getLanguage(lang) ? lang : null;
    if (!language) return plain();
    try {
        const html = hljs.highlight(text, { language, ignoreIllegals: true }).value;
        const lines = splitHighlightedHtml(html);
        // Sanity: hljs never adds or removes newlines, so the counts agree.
        return lines.length === text.split('\n').length ? lines : plain();
    } catch (_) {
        return plain();
    }
}

class CodeView {
    /**
     * Render a file into `host` (which the view fills).
     * @param {HTMLElement} host
     * @param {object} o
     * @param {string[]} o.lines - escaped/highlighted HTML per line
     * @param {number} [o.target] - line to open at (1-based)
     * @param {number} [o.mark] - line to highlight as selected (0 = none)
     * @param {number[]} [o.hits] - ascending lines to mark on the ruler
     * @param {string} [o.hitLabel] - what a ruler mark stands for ("match")
     * @param {(span: HTMLElement) => void} [o.paintLine] - e.g. mark query terms
     * @param {(n: number) => ({cls?: string, extra?: string} | null)} [o.decorate]
     * @param {number} [o.window] - lines laid out each side of the target
     * @param {string} [o.lang] - for the `data-lang` of the code block
     */
    constructor(host, o) {
        this.host = host;
        this.lines = o.lines;
        this.total = Math.max(1, o.lines.length);
        this.mark = o.mark || 0;
        this.hits = o.hits || [];
        this.hitLabel = o.hitLabel || 'match';
        this.paintLine = o.paintLine || null;
        this.decorate = o.decorate || null;
        this.size = o.window || 1000;
        this.step = this.size;
        const target = Math.min(Math.max(1, o.target || 1), this.total);
        this.from = Math.max(1, target - this.size);
        this.to = Math.min(this.total, target + this.size);

        host.classList.add('code-view');
        host.innerHTML =
            `<div class="cv-scroll" tabindex="0">` +
                `<div class="file-code" data-lang="${escapeHtml(o.lang || '')}">` +
                    `<div class="file-more" data-side="above"></div>` +
                    `<div class="file-lines"></div>` +
                    `<div class="file-more" data-side="below"></div>` +
                `</div>` +
            `</div>` +
            `<div class="cv-ruler" aria-hidden="true"></div>`;
        this.scroller = host.querySelector('.cv-scroll');
        this.linesEl = host.querySelector('.file-lines');
        this.aboveEl = host.querySelector('.file-more[data-side="above"]');
        this.belowEl = host.querySelector('.file-more[data-side="below"]');
        this.ruler = host.querySelector('.cv-ruler');

        this.linesEl.append(...this.renderRange(this.from, this.to));
        this.updateMore();
        this.drawRuler();
        this.ruler.addEventListener('click', (e) => this.onRulerClick(e));
    }

    /** Row elements for lines a..b. */
    renderRange(a, b) {
        if (b < a) return [];
        const holder = document.createElement('div');
        const rows = [];
        for (let n = a; n <= b; n++) {
            const d = this.decorate ? this.decorate(n) : null;
            const cls = 'file-line' + (n === this.mark ? ' file-line-highlight' : '') + (d && d.cls ? ' ' + d.cls : '');
            rows.push(`<div class="${cls}" id="file-line-${n}" data-line="${n}">` +
                `<span class="file-line-num">${n}</span>` +
                `<span class="file-line-content">${this.lines[n - 1] || ' '}${d && d.extra ? d.extra : ''}</span>` +
                `</div>`);
        }
        holder.innerHTML = rows.join('');
        if (this.paintLine) holder.querySelectorAll('.file-line-content').forEach(this.paintLine);
        return [...holder.childNodes];
    }

    updateMore() {
        const hiddenAbove = this.from - 1;
        const hiddenBelow = this.total - this.to;
        this.aboveEl.innerHTML = hiddenAbove > 0
            ? `<button type="button" class="file-more-btn">Show ${Math.min(this.step, hiddenAbove).toLocaleString()} more lines above (${hiddenAbove.toLocaleString()} hidden)</button>`
            : '';
        this.belowEl.innerHTML = hiddenBelow > 0
            ? `<button type="button" class="file-more-btn">Show ${Math.min(this.step, hiddenBelow).toLocaleString()} more lines below (${hiddenBelow.toLocaleString()} hidden)</button>`
            : '';
        const up = this.aboveEl.querySelector('button');
        if (up) up.addEventListener('click', () => this.extendTo(Math.max(1, this.from - this.step), this.to));
        const down = this.belowEl.querySelector('button');
        if (down) down.addEventListener('click', () => this.extendTo(this.from, Math.min(this.total, this.to + this.step)));
    }

    /** Grow the rendered window to [from, to], keeping what is on screen still. */
    extendTo(from, to) {
        if (from < this.from) {
            const prevTop = this.scroller.scrollTop;
            const prevHeight = this.scroller.scrollHeight;
            this.linesEl.prepend(...this.renderRange(from, this.from - 1));
            this.from = from;
            this.scroller.scrollTop = prevTop + (this.scroller.scrollHeight - prevHeight);
        }
        if (to > this.to) {
            this.linesEl.append(...this.renderRange(this.to + 1, to));
            this.to = to;
        }
        this.updateMore();
    }

    /** The row element of line `n`, if it is rendered. */
    row(n) {
        return this.linesEl.querySelector(`#file-line-${n}`);
    }

    /**
     * Bring line `n` into view (laying it out first when it is outside the
     * window) at `where` of the pane: 'center' or 'third'.
     */
    goTo(n, { where = 'center', flash = false } = {}) {
        n = Math.min(Math.max(1, n), this.total);
        const w = windowFor(n, this.from, this.to, this.total, this.size, this.step);
        if (w.fresh) {
            this.from = w.from;
            this.to = w.to;
            this.linesEl.replaceChildren(...this.renderRange(this.from, this.to));
            this.updateMore();
        } else if (w.from !== this.from || w.to !== this.to) {
            this.extendTo(w.from, w.to);
        }
        const row = this.row(n);
        if (!row) return null;
        const s = this.scroller.getBoundingClientRect();
        const r = row.getBoundingClientRect();
        const offset = where === 'third' ? this.scroller.clientHeight / 3 : this.scroller.clientHeight / 2 - r.height / 2;
        this.scroller.scrollTop += r.top - s.top - offset;
        if (flash) {
            row.classList.remove('flash');
            void row.offsetWidth; // restart the animation
            row.classList.add('flash');
        }
        return row;
    }

    /** Select line `n` (the highlighted line), as a click on its number does. */
    setMark(n) {
        this.linesEl.querySelectorAll('.file-line-highlight').forEach(el => el.classList.remove('file-line-highlight'));
        this.mark = n;
        const row = this.row(n);
        if (row) row.classList.add('file-line-highlight');
        this.drawRuler();
    }

    drawRuler() {
        const marks = rulerMarks(this.hits, this.total);
        const label = this.hitLabel;
        const parts = marks.map(m => `<span class="cv-mark" style="top:${m.pct}%" title="${label} at line ${m.line}"></span>`);
        if (this.mark > 0) {
            const pct = this.total > 1 ? ((this.mark - 1) / (this.total - 1)) * 100 : 0;
            parts.push(`<span class="cv-mark cv-mark-current" style="top:${Math.round(pct * 100) / 100}%" title="Line ${this.mark}"></span>`);
        }
        this.ruler.innerHTML = parts.join('');
        this.ruler.hidden = !this.hits.length;
        this.ruler.title = this.hits.length
            ? `${this.hits.length.toLocaleString()} line${this.hits.length === 1 ? '' : 's'} with a ${label}; click to jump`
            : '';
    }

    /** A click on the ruler jumps to the marked line nearest the pointer. */
    onRulerClick(e) {
        if (!this.hits.length) return;
        const rect = this.ruler.getBoundingClientRect();
        const frac = Math.min(1, Math.max(0, (e.clientY - rect.top) / Math.max(1, rect.height)));
        const line = nearestLine(this.hits, 1 + Math.round(frac * (this.total - 1)));
        if (line == null) return;
        this.goTo(line, { flash: true });
        if (this.onJump) this.onJump(line);
    }
}
