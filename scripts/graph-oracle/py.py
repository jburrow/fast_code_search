"""grimp's answer to "which modules does each Python module import?", for
the dependency-graph oracle (examples/graph_oracle.rs).

    python3 scripts/graph-oracle/py.py ROOT > py-edges.json

Every package directly under ROOT (or ROOT/src) is handed to grimp, which
parses the imports statically, the way Python would resolve them with ROOT
on sys.path. External packages are left out: fcs never indexes them either.
Output: {"files": [...], "edges": [{from, line, spec, to}]}, absolute
paths, one edge per import line; "files" are the modules analysed.
"""

import json
import os
import sys

import grimp


def packages(base):
    return sorted(
        name
        for name in os.listdir(base)
        if name.isidentifier()
        and os.path.isfile(os.path.join(base, name, "__init__.py"))
    )


def module_file(base, module):
    parts = module.split(".")
    as_package = os.path.join(base, *parts, "__init__.py")
    if os.path.isfile(as_package):
        return as_package
    as_module = os.path.join(base, *parts) + ".py"
    return as_module if os.path.isfile(as_module) else None


def main():
    root = os.path.realpath(sys.argv[1] if len(sys.argv) > 1 else ".")
    files, edges = [], []
    for base in (root, os.path.join(root, "src")):
        if not os.path.isdir(base):
            continue
        pkgs = packages(base)
        if not pkgs:
            continue
        sys.path.insert(0, base)
        graph = grimp.build_graph(*pkgs, include_external_packages=False, cache_dir=None)
        for importer in sorted(graph.modules):
            src = module_file(base, importer)
            if src is None:
                continue
            files.append(src)
            for imported in sorted(graph.find_modules_directly_imported_by(importer)):
                dst = module_file(base, imported)
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
