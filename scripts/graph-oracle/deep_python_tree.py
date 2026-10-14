"""Write a Python package nested DEPTH levels deep, for the graph oracle.

    python scripts/graph-oracle/deep_python_tree.py OUT [DEPTH] [--root-init]

Every level has an `__init__.py` and a module that imports, by absolute
name, the package root's module and its parent level's module. fcs must
find the package root however far above the importing file it sits; grimp
draws the reference edges. `--root-init` also puts an `__init__.py` in OUT
itself, as in a repository whose root folder is a package but which is run
with that folder on sys.path.
"""

import os
import sys


def main():
    args = [a for a in sys.argv[1:] if a != "--root-init"]
    out = args[0]
    depth = int(args[1]) if len(args) > 1 else 14
    if "--root-init" in sys.argv:
        os.makedirs(out, exist_ok=True)
        open(os.path.join(out, "__init__.py"), "w").close()
    names = ["deep"] + [f"l{i}" for i in range(1, depth)]
    for n in range(1, len(names) + 1):
        pkg = os.path.join(out, *names[:n])
        os.makedirs(pkg, exist_ok=True)
        dotted = ".".join(names[:n])
        with open(os.path.join(pkg, "__init__.py"), "w") as f:
            f.write("")
        with open(os.path.join(pkg, "mod.py"), "w") as f:
            f.write("import deep.mod\n")
            if n > 1:
                parent = ".".join(names[: n - 1])
                f.write(f"from {parent} import mod as parent_mod\n")
                f.write(f"import {parent}.mod\n")
            f.write(f"VALUE = {dotted!r}\n")


if __name__ == "__main__":
    main()
