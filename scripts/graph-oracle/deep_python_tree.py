"""Write a Python package nested DEPTH levels deep, for the graph oracle.

    python scripts/graph-oracle/deep_python_tree.py OUT [DEPTH]

Every level has an `__init__.py` and a module that imports, by absolute
name, the package root's module and its parent level's module. fcs must
find the package root however far above the importing file it sits; grimp
draws the reference edges.
"""

import os
import sys


def main():
    out = sys.argv[1]
    depth = int(sys.argv[2]) if len(sys.argv) > 2 else 14
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
