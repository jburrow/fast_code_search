"""grimp's answer to "which modules does each Python module import?", for
the dependency-graph oracle (`fcs check`, examples/graph_oracle.rs).

    python3 src/check/oracle/py.py ROOT > py-edges.json

Every package directly under ROOT (or ROOT/src) is handed to grimp, which
parses the imports statically, the way Python would resolve them with ROOT
on sys.path. External packages are left out: fcs never indexes them either.
Output: {"files": [...], "edges": [{from, line, spec, to}]}, absolute
paths, one edge per import line; "files" are the modules analysed.

For layouts with more on sys.path (scripts/graph-oracle/layouts.py):
FCS_ORACLE_PY_PATHS adds folders under ROOT (os.pathsep-separated), and
FCS_ORACLE_PY_NAMESPACES names namespace packages (folders without
`__init__.py`, possibly split over several of those folders).
"""

import json
import os
import sys

import grimp


def packages(base, namespaces):
    return sorted(
        name
        for name in os.listdir(base)
        if name.isidentifier()
        and (
            os.path.isfile(os.path.join(base, name, "__init__.py"))
            or (name in namespaces and os.path.isdir(os.path.join(base, name)))
        )
    )


def module_file(bases, module):
    parts = module.split(".")
    for base in bases:
        as_package = os.path.join(base, *parts, "__init__.py")
        if os.path.isfile(as_package):
            return as_package
        as_module = os.path.join(base, *parts) + ".py"
        if os.path.isfile(as_module):
            return as_module
    return None


def main():
    root = os.path.realpath(
        sys.argv[1] if len(sys.argv) > 1 else os.environ.get("FCS_ORACLE_ROOT", ".")
    )
    extra = [p for p in os.environ.get("FCS_ORACLE_PY_PATHS", "").split(os.pathsep) if p]
    namespaces = set(
        n for n in os.environ.get("FCS_ORACLE_PY_NAMESPACES", "").split(",") if n
    )
    bases = []
    for b in [root, os.path.join(root, "src")] + [os.path.join(root, p) for p in extra]:
        b = os.path.realpath(b)
        if os.path.isdir(b) and b not in bases:
            bases.append(b)
    # The first folder holding a top-level name wins, as on sys.path;
    # a namespace package collects every folder's portion.
    pkgs = []
    for base in bases:
        for name in packages(base, namespaces):
            if name not in pkgs:
                pkgs.append(name)
    files, edges = [], []
    if pkgs:
        sys.path[:0] = bases
        graph = grimp.build_graph(*pkgs, include_external_packages=False, cache_dir=None)
        for importer in sorted(graph.modules):
            src = module_file(bases, importer)
            if src is None:
                continue
            files.append(src)
            for imported in sorted(graph.find_modules_directly_imported_by(importer)):
                dst = module_file(bases, imported)
                if dst is None:
                    continue
                for d in graph.get_import_details(importer=importer, imported=imported):
                    edges.append(
                        {
                            "from": src,
                            "line": d["line_number"],
                            "spec": d["line_contents"],
                            "to": dst,
                        }
                    )
    json.dump({"files": files, "edges": edges}, sys.stdout)


if __name__ == "__main__":
    main()
