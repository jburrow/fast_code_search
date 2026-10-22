"""Score impact strategies (impact_probe output) against a Jedi oracle.

    python3 score.py cases.jsonl oracle.jsonl
"""
import collections
import json
import sys

cases = [json.loads(l) for l in open(sys.argv[1])]
cases = [c for c in cases if "symbols" in c]
users = collections.defaultdict(set)  # (target_file, target_line) -> files
for l in open(sys.argv[2]):
    r = json.loads(l)
    if "target_file" in r:
        users[(r["target_file"], r["target_line"])].add(r["file"])
    elif "users" in r:  # TypeScript language service oracle
        users[(r["file"], r["line"])] |= set(r["users"])

BASE = ["module1", "refs_rdeps1", "refs_rdeps_all", "refs_global", "word_rdeps1", "word_rdeps_all", "word_global", "guided_refs", "guided_word", "guided_alias", "guided_lazy"]


def files(s, k):
    return set(s["strategies"].get(k, {}).get("files", []))


def combos(s):
    out = {k: files(s, k) for k in BASE}
    meth = s["kind"] == "method"
    out["method: refs_rdeps_all + class named"] = (
        files(s, "refs_rdeps_all") & files(s, "owner_word_rdeps_all") if meth else set()
    )
    return out


def table(title, pred):
    tot = collections.Counter()
    n = 0
    sym_recall = collections.defaultdict(list)
    for c in cases:
        changed = set(c["changed_files"])
        for s in c["symbols"]:
            if s["head_line"] is None or s["search_name"] != s["name"] or not pred(s):
                continue
            truth = users[(s["file"], s["head_line"])] - changed
            if not truth:
                continue
            n += 1
            for k, got in combos(s).items():
                tot[(k, "tp")] += len(got & truth)
                tot[(k, "fp")] += len(got - truth)
                tot[(k, "truth")] += len(truth)
                tot[(k, "got")] += len(got)
                sym_recall[k].append(len(got & truth) / len(truth))
    print(f"\n### {title}: {n} changed symbols with at least one other-file user (oracle)\n")
    print("| Strategy | Users found (all) | Symbols fully covered | Reported files the oracle confirms | Files reported |")
    print("|---|---|---|---|---|")
    for k in list(combos({"strategies": {}, "kind": ""}).keys()):
        tp, truth, got = tot[(k, "tp")], tot[(k, "truth")], tot[(k, "got")]
        full = sum(1 for r in sym_recall[k] if r == 1.0)
        print(
            f"| {k} | {tp}/{truth} = {tp/max(truth,1):.0%} | {full}/{n} = {full/max(n,1):.0%} "
            f"| {tp/max(got,1):.0%} | {got} |"
        )


syms = [s for c in cases for s in c["symbols"]]
print("cases", len(cases), "symbols", len(syms), collections.Counter(s["kind"] for s in syms))
table("All kinds", lambda s: True)
table("Methods", lambda s: s["kind"] == "method")
table("Functions, classes, constants", lambda s: s["kind"] != "method")
