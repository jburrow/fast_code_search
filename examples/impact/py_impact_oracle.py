"""Jedi oracle for impact analysis: every use of a function, class or
property defined in the tree, with the definition it resolves to.

    python3 py_impact_oracle.py ROOT OUT.jsonl [--procs N]

Same rules as src/check/oracle/py_refs.py (goto with follow_imports, import
lines skipped, tree + stdlib sys_path), but each row also names the target:
{"file", "line", "name", "target_file", "target_line"} (0-based lines,
paths relative to ROOT). Rows for every target of an ambiguous goto.
"""

import json
import os
import sys
import sysconfig
from multiprocessing import Pool

import jedi

SKIP_DIRS = {".git", ".venv", "venv", "node_modules", "__pycache__", ".tox", "build", "dist"}
ROOT = None
PROJECT = None


def py_files(root):
    for base, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS and not d.startswith("."))
        for f in sorted(files):
            if f.endswith(".py"):
                yield os.path.join(base, f)


def init(root):
    global ROOT, PROJECT
    ROOT = root
    # parso drops in-memory cache entries older than 10 minutes, and jedi
    # then fails with KeyError in parser_cache: keep them, and recycle
    # workers (maxtasksperchild) so memory stays bounded.
    import parso.cache

    parso.cache._CACHED_FILE_MAXIMUM_SURVIVAL = 10**9
    parso.cache._CACHED_SIZE_TRIGGER = 10**9
    # One cache per process: a shared parso cache races (KeyError in
    # parser_cache) and whole files are lost.
    jedi.settings.cache_directory = os.path.join(
        os.path.expanduser("~/.cache/jedi-impact"), str(os.getpid())
    )
    stdlib = {sysconfig.get_paths()[k] for k in ("stdlib", "platstdlib")}
    PROJECT = jedi.Project(root, sys_path=[root, os.path.join(root, "src"), *sorted(stdlib)])


def one(path):
    rows = []
    try:
        with open(path, encoding="utf-8") as fh:
            source = fh.read()
        script = jedi.Script(source, path=path, project=PROJECT)
        names = script.get_names(all_scopes=True, definitions=False, references=True)
    except Exception as e:
        return [{"failed": path, "error": repr(e)[:200]}]
    rel = os.path.relpath(path, ROOT)
    failed = 0
    for name in names:
        try:
            rows.extend(uses(name, rel))
        except Exception:
            failed += 1
    if failed:
        rows.append({"failed_names": failed, "file": rel})
    return rows


def uses(name, rel):
    rows = []
    if True:
        if name.is_definition():
            return rows
        node = name._name.tree_name
        if node is not None and node.search_ancestor("import_from", "import_name") is not None:
            return rows
        targets = name.goto(follow_imports=True, follow_builtin_imports=False)
        for t in targets:
            if t.type not in ("function", "class", "property") or t.module_path is None:
                continue
            tp = os.path.realpath(str(t.module_path))
            if not tp.startswith(ROOT + os.sep) or t.line is None:
                continue
            rows.append(
                {
                    "file": rel,
                    "line": name.line - 1,
                    "name": name.name,
                    "target_file": os.path.relpath(tp, ROOT),
                    "target_line": t.line - 1,
                }
            )
    return rows


def main():
    root = os.path.realpath(sys.argv[1])
    out = sys.argv[2]
    procs = int(sys.argv[4]) if len(sys.argv) > 4 else os.cpu_count()
    files = list(py_files(root))
    n = 0
    with Pool(procs, initializer=init, initargs=(root,), maxtasksperchild=50) as pool, open(out, "w") as fh:
        for i, rows in enumerate(pool.imap_unordered(one, files, chunksize=4)):
            for r in rows:
                fh.write(json.dumps(r) + "\n")
                n += "target_file" in r
            if i % 500 == 0:
                print(f"{i}/{len(files)} files, {n} uses", file=sys.stderr, flush=True)
    print(f"done: {len(files)} files, {n} uses", file=sys.stderr)


if __name__ == "__main__":
    main()
