"""Classify guided_word files the oracle does not confirm: is every mention
in a comment or a string (heuristic, line-based)?"""
import collections, json, re, sys
cases_p, oracle_p, root, strat = sys.argv[1:5]
users = collections.defaultdict(set)
for l in open(oracle_p):
    r = json.loads(l)
    if "target_file" in r: users[(r["target_file"], r["target_line"])].add(r["file"])
    elif "users" in r: users[(r["file"], r["line"])] |= set(r["users"])
def where(line, name):
    s = line.strip()
    if s.startswith(("#", "//", "*", "/*")): return "comment"
    m = re.search(r"\b%s\b" % re.escape(name), line)
    before = line[:m.start()]
    if before.count('"') % 2 or before.count("'") % 2 or before.count("`") % 2: return "string"
    return "code"
c = collections.Counter(); tot = collections.Counter()
for l in open(cases_p):
    case = json.loads(l)
    if "symbols" not in case: continue
    ch = set(case["changed_files"])
    for s in case["symbols"]:
        if s["head_line"] is None or s["search_name"] != s["name"]: continue
        t = users[(s["file"], s["head_line"])] - ch
        if not t: continue
        grp = "method" if s["kind"] == "method" else "other"
        names = s["strategies"][strat].get("names", [s["name"]])
        for f in set(s["strategies"][strat]["files"]):
            tot[(grp, f in t)] += 1
            if f in t: continue
            try: lines = open(f"{root}/{f}", encoding="utf-8", errors="replace").read().splitlines()
            except OSError: continue
            kinds = {where(x, n) for x in lines for n in names if re.search(r"\b%s\b" % re.escape(n), x)}
            c[(grp, "code" if "code" in kinds else ("string" if "string" in kinds else "comment"))] += 1
print(tot); print(c)
