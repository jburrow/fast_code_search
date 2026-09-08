# Ranking quality

Speed is easy to measure and easy to game: an engine that returns the
first fifty lines containing the query is fast and useless. This page
tracks the thing that matters — **does the first result answer the
query?** — with a suite of labelled queries that runs on every push and
fails the build when it regresses.

## What is measured

Each query in [`benches/ranking_quality.toml`](https://github.com/jburrow/fast_code_search/blob/main/benches/ranking_quality.toml)
names the file (and, for text queries, the line) a reader would want first,
with a one-line reason. The corpora are the pinned
[tokio 1.45.0](https://github.com/tokio-rs/tokio/tree/tokio-1.45.0) and
[Django 5.2](https://github.com/django/django/tree/5.2) trees used by the
latency benchmark, so both measurements describe the same index.

- **precision@1** — the share of queries whose first result is the expected one.
- **precision@5** — the share whose expected result is in the top five.

The CI gate is precision@1 ≥ 0.90 and precision@5 ≥ 0.95. The suite
deliberately favours the cases a scan tool gets wrong: a definition that
has many mentions in docs and tests, a bare identifier whose definition
should beat its uses, a short name (`Field`) with several unrelated
definitions where the widely imported one should win, and a reference
query where the definition itself must not appear.

## Current score

tokio 1.45.0 + Django 5.2: **precision@1 1.00, precision@5 1.00** over 20 queries.

| Query | Mode | Expected first | Why | Rank found |
|---|---|---|---|---|
| `struct Runtime` | text | `tokio/src/runtime/runtime.rs` | the definition, not the many mentions in docs and tests | 1 |
| `pub struct Mutex` | text | `tokio/src/sync/mutex.rs` | tokio's Mutex definition | 1 |
| `fn sleep` | text | `tokio/src/time/sleep.rs` | the free function, above test helpers named sleep | 1 |
| `fn block_on` | text | `tokio/src/runtime/runtime.rs or tokio/src/runtime/handle.rs` | the public Runtime::block_on / Handle::block_on, not the scheduler's internal fn | 1 |
| `struct Notify` | text | `tokio/src/sync/notify.rs` | the definition | 1 |
| `struct Semaphore` | text | `tokio/src/sync/semaphore.rs` | the definition | 1 |
| `class HttpResponse` | text | `django/http/response.py` | the class itself, above HttpResponseRedirect and friends | 1 |
| `def get_object_or_404` | text | `django/shortcuts.py` | the definition | 1 |
| `class QuerySet` | text | `django/db/models/query.py` | the definition | 1 |
| `class ModelForm` | text | `django/forms/models.py` | the definition | 1 |
| `class Client` | text | `django/test/client.py` | the test client definition | 1 |
| `HttpResponseBase` | text | `django/http/response.py` | a bare identifier: its definition should beat its uses | 1 |
| `Runtime` | symbols | `tokio/src/runtime/runtime.rs` | exact name match, the struct | 1 |
| `get_object_or_404` | symbols | `django/shortcuts.py` | exact name match | 1 |
| `JoinHandle` | symbols | `tokio/src/runtime/task/join.rs` | the struct, above the many impl blocks | 1 |
| `HttpResponse` | symbols | `django/http/response.py` | exact name match over HttpResponseRedirect etc. | 1 |
| `Field` | symbols | `django/db/models/fields/__init__.py` | the widely imported model Field, not the GDAL one | 1 |
| `pub fn sleep\(` | regex | `tokio/src/time/sleep.rs` | literal-accelerated regex | 1 |
| `^class Form\(` | regex | `django/forms/forms.py` | anchored regex | 1 |
| `get_object_or_404` | references | `django/contrib/flatpages/views.py` | the one call site in shipped (non-test) code; the definition itself must not be listed as a reference | 1 |

## What it caught

The first run of the suite scored 0.30 at precision@1. Every miss was a
ranking defect, not a missing feature, and each fix is in the changelog:

- Items declared inside `cfg_if!` and similar macro bodies were invisible
  to the symbol extractor, so `struct Runtime` ranked a doc comment above
  the definition. The extractor now re-parses macro bodies.
- Lines that matched only some of a multi-term query could outrank the
  line with the whole phrase. Phrase matches are now a strict tier above
  all-terms and single-term lines.
- Test, example, mock and fixture paths carried the same weight as shipped
  code. They are now scaled by 0.6, and public definitions by 1.6.
- Symbol search compared the query against names with visibility and
  attribute prefixes still attached, so `pub struct Mutex` lost to
  `pub(crate) struct MutexGuard`. Modifiers are stripped before scoring.
- Symbol search over a large candidate set spent its per-file budget on
  the first files scanned in parallel, which made results depend on thread
  scheduling. Candidates are now scored from the symbol cache first and
  only the best files are scanned for matches.
- Match budgets were consumed by whichever files finished first, so the
  same query could return different results on different runs. Files
  that contain the whole phrase are now scanned in a separate pass before
  the rest.

## Running it

```bash
git clone --depth 1 --branch tokio-1.45.0 https://github.com/tokio-rs/tokio bench-corpus/tokio
git clone --depth 1 --branch 5.2 https://github.com/django/django bench-corpus/django
cargo run --release --example ranking_quality -- bench-corpus/tokio bench-corpus/django
```

Add `--explain 'struct Runtime'` (with `--explain-mode symbols|regex|references`
for other modes) to print the top results with their scores, tiers and the
factors that produced them. `--min-p1` and `--min-p5` turn the run into a
gate, as CI does. Adding a query is a five-line TOML entry; the point of
the suite is that every ranking change comes with the example that
motivated it.
