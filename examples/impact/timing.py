import json,sys
rows=[json.loads(l) for l in open(sys.argv[1])]
cases=[c for c in rows if 'symbols' in c]
syms=[s for c in cases for s in c['symbols']]
def pct(v,p): v=sorted(v); return v[min(len(v)-1,int(len(v)*p))]
print(len(cases),'commits',len(syms),'symbols')
for k in ['module1','refs_rdeps1','refs_rdeps_all','refs_global','word_rdeps1','word_rdeps_all','word_global','guided_refs','guided_word','guided_alias','guided_lazy','module_all','level2']:
    ms=[s['strategies'][k]['ms'] for s in syms if k in s['strategies']]
    if not ms: continue
    nf=[len(s['strategies'][k].get('files',[])) for s in syms if k in s['strategies']]
    print(f"| {k} | {pct(ms,.5):.1f} | {pct(ms,.95):.1f} | {max(ms):.0f} | {pct(nf,.5)} | {pct(nf,.95)} | {max(nf)} |")
w=[s['strategies']['guided_word']['walk_ms'] for s in syms]
print('guided walk only p50 %.3f p95 %.3f max %.1f'%(pct(w,.5),pct(w,.95),max(w)))
for k in ['guided_refs','guided_word','guided_alias','guided_lazy','refs_rdeps_all','module1','module_all']:
    u=[len(set().union(*[set(s['strategies'][k].get('files',[])) for s in c['symbols']])) for c in cases]
    t=[sum(s['strategies'][k]['ms'] for s in c['symbols']) for c in cases]
    print(k,'per-commit files p50',pct(u,.5),'p90',pct(u,.9),'max',max(u),'| per-commit ms p50 %.0f p95 %.0f max %.0f'%(pct(t,.5),pct(t,.95),max(t)))
