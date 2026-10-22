# Synthetic Python monorepo: N files in packages of 1000; one hub module
# (core/util.py) imported by every other file; half of the importers call
# `helper`, the rest only use `other_*`. Then one commit changes `helper`.
import os, sys, random
N = int(sys.argv[1])
random.seed(0)
os.makedirs("core", exist_ok=True)
open("core/__init__.py", "w").write("")
util = "def helper(x):\n    return x + 1\n\n\n" + "".join(f"def other_{i}(x):\n    return x * {i}\n\n\n" for i in range(50))
open("core/util.py", "w").write(util)
for i in range(N):
    d = f"pkg{i // 1000:04d}"
    if i % 1000 == 0:
        os.makedirs(d, exist_ok=True)
        open(f"{d}/__init__.py", "w").write("")
    uses = i % 2 == 0
    name = "helper" if uses else f"other_{i % 50}"
    body = f"from core.util import {name}\nimport os\n\n\ndef task_{i}(ctx):\n    value = ctx.get('k{i}')\n    if value is None:\n        return {name}(0)\n    return value\n\n\ndef helper_unrelated_{i}():\n    return os.getcwd()\n"
    open(f"{d}/job_{i}.py", "w").write(body)
