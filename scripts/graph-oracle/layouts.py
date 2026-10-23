"""Write Python repositories laid out the way real monorepos are, for the
graph oracle (examples/graph_oracle.rs, CI workflow "graph oracle").

    python scripts/graph-oracle/layouts.py LAYOUT OUT [--seed N]

Django, the deep tree and the generated codebases (tests/support/pygen.rs)
are each one package under one folder on sys.path. Real repositories are
not, and the graph has broken on their shapes (#170: a root folder with
`__init__.py`). Each layout here is one such shape, filled with seeded
modules that import each other by absolute and relative names:

- `monorepo`: services and libraries side by side, each its own project
  with a `src/` folder (`services/billing/src/billing/...`,
  `libs/lib_db/src/lib_db/...`), installed into one environment, so every
  `src/` is on sys.path. Services import the libraries by name.
- `root-init`: a repository whose root folder has an `__init__.py` and is
  run with the root on sys.path; top-level packages (`apps`, `platform`,
  `libs`) import each other as `apps.billing.models`.
- `namespace`: one namespace package (PEP 420, no `__init__.py`) split
  over two folders on sys.path (`src/acme/...` and `plugins/src/acme/...`).

It writes OUT/oracle.env with the settings the oracle (src/check/oracle/
py.py) needs for the layout: the extra sys.path folders
(FCS_ORACLE_PY_PATHS) and the namespace packages (FCS_ORACLE_PY_NAMESPACES).
OUT/.git is created so the folder looks like a repository's root.
"""

import os
import random
import sys

FORMS = ["import", "import_as", "from_module", "from_name", "relative"]


class Tree:
    def __init__(self, out, rng):
        self.out = out
        self.rng = rng
        # dotted name -> (path of the file, sys.path folder it hangs off)
        self.modules = {}
        self.order = []

    def package(self, base, dotted, init=True):
        path = os.path.join(self.out, base, *dotted.split("."))
        os.makedirs(path, exist_ok=True)
        if init:
            self.add(base, dotted, os.path.join(path, "__init__.py"))

    def add(self, base, dotted, path):
        self.modules[dotted] = path
        self.order.append(dotted)
        with open(path, "w") as f:
            f.write(f'"""{dotted}."""\n\nNAME_{dotted.replace(".", "_")} = 1\n')

    def module(self, base, dotted):
        self.add(base, dotted, os.path.join(self.out, base, *dotted.split(".")) + ".py")

    def imports(self, importer, targets):
        """Append imports of `targets` to `importer`'s file."""
        lines = []
        for t in targets:
            if t == importer:
                continue
            form = self.rng.choice(FORMS)
            parent, _, leaf = t.rpartition(".")
            name = f"NAME_{t.replace('.', '_')}"
            if form == "relative" and importer.split(".")[0] == t.split(".")[0]:
                rel = relative(importer, t, self.modules[importer].endswith("__init__.py"))
                if rel:
                    lines.append(rel)
                    continue
            if form == "import" or (form == "relative"):
                lines.append(f"import {t}")
            elif form == "import_as":
                lines.append(f"import {t} as m_{t.replace('.', '_')}")
            elif form == "from_module" and parent:
                lines.append(f"from {parent} import {leaf}")
            else:
                lines.append(f"from {t} import {name}")
        with open(self.modules[importer], "a") as f:
            f.write("\n" + "\n".join(lines) + "\n")

    def wire(self, groups, per_module=(1, 4)):
        """Each module imports a few from the groups it may see: `groups`
        maps a dotted prefix to the prefixes its modules may import."""
        for m in self.order:
            allowed = [
                t
                for prefix, sees in groups.items()
                if m == prefix or m.startswith(prefix + ".")
                for t in self.order
                if any(t == s or t.startswith(s + ".") for s in sees)
            ]
            if allowed:
                n = self.rng.randint(*per_module)
                self.imports(m, self.rng.sample(allowed, min(n, len(allowed))))


def relative(importer, target, importer_is_package):
    """`from ..x import y` for `target` seen from `importer`, if expressible."""
    here = importer.split(".")
    if not importer_is_package:
        here = here[:-1]  # the package the module is in
    there = target.split(".")
    common = 0
    while common < min(len(here), len(there)) and here[common] == there[common]:
        common += 1
    if common == 0:
        return None
    dots = "." * (len(here) - common + 1)
    rest = there[common:]
    if not rest:
        return None
    if len(rest) == 1:
        return f"from {dots} import {rest[0]}"
    return f"from {dots}{'.'.join(rest[:-1])} import {rest[-1]}"


def subtree(tree, base, top, rng, depth=3, width=3, modules=4):
    """A package `top` under `base`, nested `depth` deep."""
    tree.package(base, top)
    frontier = [top]
    for _ in range(depth):
        nxt = []
        for p in frontier:
            for i in range(rng.randint(1, width)):
                child = f"{p}.sub{i}"
                tree.package(base, child)
                nxt.append(child)
            for i in range(rng.randint(1, modules)):
                tree.module(base, f"{p}.mod{i}")
        frontier = nxt
    for p in frontier:
        for i in range(rng.randint(1, modules)):
            tree.module(base, f"{p}.mod{i}")


def monorepo(out, rng):
    tree = Tree(out, rng)
    services = ["billing", "search_svc", "ingest"]
    libs = ["lib_db", "lib_auth", "lib_util"]
    paths = []
    for kind, names, depth in [("services", services, 3), ("libs", libs, 2)]:
        for name in names:
            project = os.path.join(out, kind, name)
            base = os.path.join(kind, name, "src")
            subtree(tree, base, name, rng, depth=depth)
            paths.append(base)
            with open(os.path.join(project, "pyproject.toml"), "w") as f:
                f.write(f'[project]\nname = "{name}"\nversion = "1.0"\n')
    groups = {s: [s] + libs for s in services}
    groups.update({lib: [lib, "lib_util"] for lib in libs})
    tree.wire(groups)
    return {"FCS_ORACLE_PY_PATHS": os.pathsep.join(paths)}


def root_init(out, rng):
    tree = Tree(out, rng)
    with open(os.path.join(out, "__init__.py"), "w") as f:
        f.write("")
    tops = ["apps", "platform_core", "libs"]
    for top in tops:
        subtree(tree, "", top, rng)
    tree.wire({"apps": tops, "platform_core": ["platform_core", "libs"], "libs": ["libs"]})
    return {}


def namespace(out, rng):
    tree = Tree(out, rng)
    for base, part in [("src", "core"), ("src", "billing"), ("plugins/src", "plugin_x")]:
        os.makedirs(os.path.join(out, base, "acme"), exist_ok=True)
        subtree(tree, base, f"acme.{part}", rng, depth=2)
    tree.wire({"acme": ["acme"]})
    return {"FCS_ORACLE_PY_PATHS": "plugins/src", "FCS_ORACLE_PY_NAMESPACES": "acme"}


LAYOUTS = {"monorepo": monorepo, "root-init": root_init, "namespace": namespace}


def main():
    args = sys.argv[1:]
    seed = 1
    if "--seed" in args:
        i = args.index("--seed")
        seed = int(args[i + 1])
        del args[i : i + 2]
    layout, out = args
    os.makedirs(os.path.join(out, ".git"), exist_ok=True)
    env = LAYOUTS[layout](out, random.Random(seed))
    with open(os.path.join(out, "oracle.env"), "w") as f:
        for k, v in env.items():
            f.write(f"{k}={v}\n")


if __name__ == "__main__":
    main()
