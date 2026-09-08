#!/usr/bin/env python3
"""Copy the canonical docs into the book with links rewritten.

Canonical files live at the repository level so the README, release
archives and error messages can point at them. The book gets a copy under
docs/book/src/imported/; links between imported files are rewritten to
their new names, links to repository files become GitHub URLs, and the
first heading is kept as the page title.
"""
from __future__ import annotations
import re, shutil, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "docs/book/src/imported"
REPO = "https://github.com/jburrow/fast_code_search/blob/main/"

# source path (from repo root) -> book file name
IMPORTS = {
    "docs/CLI.md": "cli.md",
    "docs/RUN-AT-STARTUP.md": "run-at-startup.md",
    "docs/API.md": "api.md",
    "docs/DEPLOYMENT.md": "deployment.md",
    "docs/DEVELOPMENT.md": "development.md",
    "docs/design/PRIOR_ART.md": "prior-art.md",
    "docs/GLOSSARY.md": "glossary.md",
    "docs/semantic/SEMANTIC_SEARCH_README.md": "semantic.md",
    "CHANGELOG.md": "changelog.md",
    "CONTRIBUTING.md": "contributing.md",
    "SECURITY.md": "security.md",
    "CODE_OF_CONDUCT.md": "code-of-conduct.md",
    "docs/internal/plans/2026-09-07-premium-repository-plan.md": "plan-repository.md",
    "docs/internal/plans/2026-09-06-keyword-engine-and-ui-review.md": "review-2026-09.md",
    "docs/internal/plans/2026-09-04-keyword-engine-roadmap.md": "roadmap-2026-09.md",
}

def rewrite(src: pathlib.Path, text: str) -> str:
    base = src.parent
    def repl(m):
        target, anchor = m.group(1), m.group(2) or ""
        if re.match(r"^(https?:|mailto:|#)", target):
            return m.group(0)
        resolved = (ROOT / target.lstrip("/")).resolve() if target.startswith("/") else (base / target).resolve()
        try:
            rel = resolved.relative_to(ROOT).as_posix()
        except ValueError:
            return m.group(0)
        if rel in IMPORTS:
            return f"]({IMPORTS[rel]}{anchor})"
        if resolved.exists():
            return f"]({REPO}{rel}{anchor})"
        return m.group(0)
    return re.sub(r"\]\(([^)#\s]+)(#[^)]*)?\)", repl, text)

def main() -> None:
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    for rel, name in IMPORTS.items():
        src = ROOT / rel
        text = src.read_text(encoding="utf-8")
        (OUT / name).write_text(rewrite(src, text), encoding="utf-8")
    print(f"imported {len(IMPORTS)} documents into {OUT.relative_to(ROOT)}")

if __name__ == "__main__":
    main()
