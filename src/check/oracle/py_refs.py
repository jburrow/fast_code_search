"""Jedi's answer to "where is each function and class used?", for the
reference oracle (examples/ref_oracle.rs).

    python3 src/check/oracle/py_refs.py ROOT > py-refs.json

For every name in every .py file under ROOT, Jedi resolves what it refers
to (`goto`, following imports). A use counts when it resolves to a
function or class defined in a file under ROOT, and it is neither that
definition itself nor a name in an import statement. Output:
{"files": [...], "refs": [{"file", "line", "column", "name", "kind"}],
"unresolved": [[file, line, column]]}, absolute paths, 0-based line and
column (bytes, like tree-sitter). "unresolved" are the uses Jedi could not
resolve to anything (an untyped receiver, dynamic attributes): fcs is
neither right nor wrong to list those.
"""

import json
import os
import sys
import sysconfig

import jedi

SKIP_DIRS = {".git", ".venv", "venv", "node_modules", "__pycache__", ".tox", "build", "dist"}


def py_files(root):
    for base, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS and not d.startswith("."))
        for f in sorted(files):
            if f.endswith(".py"):
                yield os.path.join(base, f)


def byte_column(line_text, char_column):
    return len(line_text[:char_column].encode("utf-8"))


def main():
    root = os.path.realpath(sys.argv[1] if len(sys.argv) > 1 else ".")
    # The tree and the standard library only: an installed copy of the same
    # package would otherwise win, and its definitions are not in the tree.
    stdlib = {sysconfig.get_paths()[k] for k in ("stdlib", "platstdlib")}
    sys_path = [root, os.path.join(root, "src"), *sorted(stdlib)]
    project = jedi.Project(root, sys_path=sys_path)
    files, refs, unresolved = [], [], []
    for path in py_files(root):
        try:
            with open(path, encoding="utf-8") as fh:
                source = fh.read()
        except (OSError, UnicodeDecodeError):
            continue
        files.append(path)
        lines = source.splitlines()
        script = jedi.Script(source, path=path, project=project)
        try:
            names = script.get_names(all_scopes=True, definitions=False, references=True)
        except Exception:
            continue
        for name in names:
            if name.is_definition():
                continue
            node = name._name.tree_name
            if node is not None and node.search_ancestor("import_from", "import_name") is not None:
                continue
            try:
                targets = name.goto(follow_imports=True, follow_builtin_imports=False)
            except Exception:
                continue
            kinds = {
                t.type
                for t in targets
                if t.type in ("function", "class", "property")
                and t.module_path is not None
                and os.path.realpath(str(t.module_path)).startswith(root + os.sep)
            }
            text = lines[name.line - 1] if name.line - 1 < len(lines) else ""
            if not targets:
                unresolved.append([path, name.line - 1, byte_column(text, name.column)])
                continue
            if not kinds:
                continue
            text = lines[name.line - 1] if name.line - 1 < len(lines) else ""
            refs.append(
                {
                    "file": path,
                    "line": name.line - 1,
                    "column": byte_column(text, name.column),
                    "name": name.name,
                    "kind": sorted(kinds)[0],
                }
            )
    json.dump({"files": files, "refs": refs, "unresolved": unresolved}, sys.stdout)


if __name__ == "__main__":
    main()
