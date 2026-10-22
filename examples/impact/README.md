# Diff impact evidence

Scripts behind `/mnt/project-files/designs/diff-impact-analysis.md` (2026-10-22).
Evaluation only: nothing here is part of fcs.

```sh
# fcs side: changed symbols per commit, and the files each strategy lists
cargo run --release --example impact_probe -- --repo DJANGO --commits 300 --out django-cases.jsonl --level2
# Python oracle (Jedi 0.20.1): every use, with the definition it resolves to
python3 examples/impact/py_impact_oracle.py DJANGO django-oracle.jsonl
# TypeScript oracle: findReferences on each changed definition (npm i typescript@5.6.3)
node examples/impact/ts_refs.js SENTRY ts-defs.jsonl ts-oracle.jsonl
# scores
python3 examples/impact/final.py django-cases.jsonl django-oracle.jsonl
python3 examples/impact/timing.py django-cases.jsonl
# worst case: 400k files all importing one module
python3 examples/impact/synth_gen.py 400000   # in an empty git repo, then commit a change to core/util.py
```

`impact_probe` uses `SearchEngine::text_candidates`, made public on this branch for it.
