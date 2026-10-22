import collections, json, sys
cases_p, oracle_p = sys.argv[1:3]
langs = sys.argv[3].split(",") if len(sys.argv) > 3 else None
users = collections.defaultdict(set)
for l in open(oracle_p):
    r = json.loads(l)
    if "target_file" in r: users[(r["target_file"], r["target_line"])].add(r["file"])
    elif "users" in r: users[(r["file"], r["line"])] |= set(r["users"])
S = [("module_all", "Today: change_impact (all transitive importers)"),
     ("module1", "Today: direct importers (rdeps:FILE)"),
     ("refs_rdeps1", "Today by hand: fcs refs 'rdeps:FILE NAME'"),
     ("word_global", "Whole-word NAME anywhere (rg -w)"),
     ("refs_global", "References of NAME anywhere"),
     ("refs_rdeps_all", "References of NAME in rdeps:FILE:*"),
     ("guided_refs", "Proposed tier 1: references, name-guided walk"),
     ("guided_alias", "Proposed tiers 1+2: + every word mention, renames followed")]
def f(s, k):
    if k == "class_named":
        return set(s["strategies"]["refs_rdeps_all"]["files"]) & set(s["strategies"].get("owner_word_rdeps_all", {}).get("files", []))
    return set(s["strategies"].get(k, {}).get("files", [])) if k in s["strategies"] else None
for grp in ("other", "method"):
    tot = collections.Counter(); n = 0; zero_rep = collections.Counter(); nz = 0
    for l in open(cases_p):
        c = json.loads(l)
        if "symbols" not in c: continue
        ch = set(c["changed_files"])
        for s in c["symbols"]:
            if langs and not s["file"].endswith(tuple(langs)): continue
            if ("method" if s["kind"] == "method" else "other") != grp: continue
            if s["head_line"] is None or s["search_name"] != s["name"]: continue
            t = users[(s["file"], s["head_line"])] - ch
            if not t: continue
            n += 1
            for k, _ in S + ([("class_named", "")] if grp == "method" else []):
                g = f(s, k)
                if g is None: continue
                tot[(k, "n")] += 1; tot[(k, "tp")] += len(g & t); tot[(k, "t")] += len(t); tot[(k, "g")] += len(g)
    print(f"\n{grp}: {n} symbols with users")
    for k, label in S + ([("class_named", "Methods: refs in rdeps:* that also name the class")] if grp == "method" else []):
        if not tot[(k, "n")]: continue
        print(f"| {label} | {tot[(k,'tp')]/tot[(k,'t')]:.0%} | {tot[(k,'tp')]/max(tot[(k,'g')],1):.0%} | {tot[(k,'g')]/tot[(k,'n')]:.1f} | ({tot[(k,'n')]} syms)")
