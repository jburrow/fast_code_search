# Reference support

What the **References** mode (REFERENCES in the web UI, `fcs refs`,
`references=true`, MCP `mode: "references"`) finds, per language.

References are search-based, not compiler-based. While indexing, each file
is parsed with tree-sitter, and the grammar's tags query marks some names
as references: call sites, type mentions, `new` expressions, base classes.
A reference search returns the lines where the identifier appears in one of
those positions. Two consequences hold for every language:

- **Names, not symbols.** `fcs refs parse` returns every call of anything
  called `parse`, whichever class or module it belongs to. Nothing tells two
  functions with the same name apart.
- **Only the listed positions.** A use in any other position (an import
  line; outside Python, a function passed as a value) is not a reference,
  even though a text search finds it. When references look incomplete,
  search the plain text as well.

A definition is never its own reference, and references are exact and
case-sensitive on the identifier.

If fcs misses a form listed as found, that is a bug; if it misses one
listed as not found, that is a known gap.

## At a glance

| Language | Calls | Method calls | Type mentions | `new` / constructors | Base classes | Not found (main gaps) |
|---|---|---|---|---|---|---|
| Rust | yes | yes | yes | n/a | `impl Trait for T` | anything inside a macro's arguments, the `Type` in `Type::new()`, enum variants (`Item::A`), `use` lines, functions as values |
| Python | yes | yes | yes | yes (a call) | yes | imports, string annotations, a few value positions; also returns uses of other things with the same name (see below) |
| JavaScript | yes | yes | n/a | yes | **no** | `extends`, `instanceof`, functions as values, property reads, imports |
| TypeScript | yes | yes | partly | yes | **no** | `Foo[]`, `Foo<T>`, `A \| B`, `ns.Type`, type arguments, `type` aliases, `extends` and `implements`, `instanceof`, imports |
| Go | yes | yes | yes | composite literals (`Item{}`) | n/a | functions as values (`Run: helper`, `cb := helper`), field reads, imports |
| Java | yes | yes | **partly** | yes | yes | field, parameter, local and generic types, annotations, `Foo.class`, `Foo::bar`, the class in `Util.f()`, casts, imports |
| C# | **member calls only** | yes | partly | yes | yes | plain calls `Helper()`, generic calls `Make<T>()`, parameter and generic types, attributes, `typeof`, casts, the class in `Util.F()`, `using` |
| C | **no** | **no** | **no** | n/a | n/a | everything: no references are recorded |
| C++ | **no** | **no** | **no** | **no** | **no** | everything: no references are recorded |
| Ruby | yes | yes | constants | `Item.new` | yes | see below: also returns local variables |
| PHP | **no** (plain `f()`) | yes | **no** | **no** | `implements` only | plain function calls, `new Item()`, `extends`, parameter and return types, `use` |
| Bash and other languages | no | no | no | no | no | not parsed for references |

"Partly" is spelled out in the language's section below.

## Rust (`.rs`)

| Form | Found |
|---|---|
| `helper(x)`, `self.helper()`, `crate::a::helper()`, `Type::new()` (the `new`) | yes |
| `helper::<T>()`, `a::helper::<T>()` | yes |
| Types in fields, parameters, returns and generics (`Result<Item, Error>`), `impl Trait for S` | yes |
| The `Type` in `Type::new()` or `Item::default()` | **no** |
| Enum variants and paths in patterns (`Item::A`) | **no** |
| Anything inside a macro call (`println!("{}", helper(x))`, `assert!(…)`) | **no**: macro arguments are not parsed |
| `use crate::a::{Engine, helper};` | **no** |
| Functions as values (`let cb = helper;`, `.map(helper)`) | **no** |
| Field reads (`self.prop`) | **no** |
| `#[derive(Debug)]` | **no** |

The macro gap matters most in tests: calls inside `assert_eq!(…)` are not
references.

## Python (`.py`, `.pyi`, `.pyw`)

Python is checked against [Jedi](https://github.com/davidhalter/jedi),
which resolves each name to its definition (see the end of the page).

| Form | Found |
|---|---|
| `helper(x)`, `self.helper()`, `pkg.util.do_thing()`, `Foo()` | yes |
| Decorators: `@login_required`, `@app.route("/x")`, `@bp.before_app_request`, `@name.setter` | yes |
| Base classes: `class Foo(Base, Mixin)` | yes |
| Type annotations: `item: Item`, `-> Result`, `list[Item]`, `Item \| None`, `Callable[[A], B]`, `t.Optional[Item]` | yes |
| Functions and classes as values: `isinstance(x, Item)`, `cb = helper`, `return Result`, `register(self.on_done)`, `(a, b)`, `{"k": helper}`, `x == helper` | yes |
| Any attribute that is read: `self.prop`, `obj.CONSTANT`, `request.form` | yes |
| A class used through an attribute: `Item.from_dict()` (the `Item`, when it starts with a capital) | yes |
| `except Oops as e:`, `raise Failed` | yes |
| Assigning an attribute: `self.prop = 1` | no, on purpose: it sets `prop`, it does not use it |
| A parameter or local variable with the same name as a function (`def view(request, app): app.run()`) | no, on purpose: Python makes a name bound anywhere in a function local to it, unless it is declared `global` or `nonlocal` |
| `import`, `from … import` | **no** |
| Annotations written as strings: `x: "Item"` | **no** |
| Other value positions: lambda bodies, `yield x`, `*args` unpacking, f-string `{x}` | **no** |

Because references are names, `fcs refs get` also returns every `.get(…)`
on a dictionary, and `fcs refs status_code` returns reads of every
attribute called `status_code`, whichever class it belongs to. On the
repositories below, between one Python reference in ten and one in five
is such a use of something else with the same name.

## JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`)

| Form | Found |
|---|---|
| `helper(x)`, `this.helper()`, `z.string()` | yes |
| `new Foo()` | yes |
| `require("x")` | no, on purpose |
| `class Foo extends Base` | **no** |
| `x instanceof Item` | **no** |
| Functions as values (`const cb = helper`, `arr.map(helper)`) | **no** |
| Property reads (`this.prop`) and object keys | **no** |
| `import { helper } from "./x"` | **no** |

## TypeScript (`.ts`, `.mts`, `.cts`, `.tsx`)

Everything JavaScript finds, plus some type mentions:

| Form | Found |
|---|---|
| A plain type in an annotation: `item: Item`, `): Result` | yes |
| Arrays, generics and unions: `Item[]`, `Map<string, Item>`, `Item \| null` | **no** |
| Qualified types: `z.ZodType`, `ns.Item` | **no** |
| Type arguments: `generic<Item>(x)`, `Promise<Item>` | **no** |
| `type T = Item & Other`, `interface A extends B` | **no** |
| `class Foo extends Base implements Iface` | **no** |
| Decorators | **no** |

## Go (`.go`)

| Form | Found |
|---|---|
| `helper(x)`, `s.helper()`, `pkg.DoThing()` | yes |
| Types anywhere (`e *Engine`, `[]Item`, `map[string]Item`, `Item{}`) | yes, built-in types such as `string` and `error` too |
| Functions as values: `cb := helper`, `Command{Run: helper}`, `sort.Slice(x, less)` | **no** |
| Field reads (`s.prop`) | **no** |
| The package in `pkg.DoThing()`, `import` lines | **no** |

## Java (`.java`)

| Form | Found |
|---|---|
| `helper(x)`, `this.helper()`, `Util.doThing()` (the `doThing`) | yes |
| `new Item()` | yes |
| `extends Base`, `implements Iface` | yes |
| Field, parameter, return, local and generic types: `private Engine e;`, `run(Engine e, List<Item> xs)`, `Item x` | **no** |
| The class in `Util.doThing()` | **no** |
| `Item.class`, `S::helper`, casts `(Result) x` | **no** |
| Annotations (`@Annot`) | **no** |
| `import` lines | **no** |

## C# (`.cs`)

| Form | Found |
|---|---|
| `this.Helper()`, `Util.DoThing()` (the `DoThing`) | yes |
| `new Item()`, `Item x = …`, field types | yes |
| `class S : Base, IFace` | yes |
| Plain calls: `Helper(e)` | **no** |
| Generic calls and generic types: `Make<Item>()`, `List<Item>` | **no** |
| Parameter and return types | **no** |
| The class in `Util.DoThing()` | **no** |
| Attributes (`[Serializable]`), `typeof(Item)`, casts | **no** |
| `using` lines | **no** |

## C and C++

No references are recorded. The C and C++ grammars' tags queries define
symbols only, so `fcs refs` finds nothing in `.c`, `.h`, `.cpp`, `.cc`,
`.hpp` and the other C and C++ extensions. Use a plain or whole-word text
search instead.

## Ruby (`.rb`, `.rake`, `.gemspec`)

Calls, method calls, constants (`Item.new`, `Util.do_thing`), base classes
and `include Mixin` are found. Ruby's tags query is meant to skip local
variables, but fcs does not apply that filter, so **local variables and
parameters are returned as references too** (`x` in `x = Item.new`).

## PHP (`.php`, `.phtml`)

| Form | Found |
|---|---|
| `$this->helper()`, `Util::doThing()` (the `doThing`) | yes |
| `implements IFace` | yes |
| Plain calls: `helper($e)` | **no** |
| `new Item()` | **no** |
| `extends Base`, parameter and return types, `use` lines | **no** |

## How complete this is

### Python, against Jedi

`examples/ref_oracle.rs` compares the references fcs records in a
repository with the ones [Jedi](https://github.com/davidhalter/jedi) 0.20
finds (`src/check/oracle/py_refs.py`): every name Jedi resolves to a
function, class or property defined in the repository, other than the
definition itself and import lines. A reference Jedi lists and fcs does not
is **missing**. A reference fcs records, under the name of one of those
functions or classes, where Jedi resolves the name to something else, is
**extra**: almost always a use of something else with the same name.
Where Jedi cannot resolve a name at all (an untyped receiver), neither side
is counted.

| Repository | Found | Precision | Before October 2026 |
|---|---|---|---|
| pallets/flask | 98.1% | 87.3% | 57.4% found, 90.9% precision |
| psf/requests | 100% | 90.2% | 74.4%, 88.9% |
| pallets/click | 97.1% | 91.3% | 65.5%, 93.6% |
| encode/httpx | 99.5% | 80.5% | 59.0%, 86.6% |

CI runs it on Flask and Click (`.github/workflows/ref-oracle.yml`) and
fails if a change misses or adds more references than that.

```sh
pip install jedi==0.20.1
python3 src/check/oracle/py_refs.py bench-corpus/flask > flask.json
cargo run --release --example ref_oracle -- --oracle flask.json
```

The report groups misses and extras by the syntax around the name, which
says which row of the Python table each one is.

### Other languages

There is no oracle for the other languages yet. Each form above is checked
with a small file per language. To give a sense of scale, a one-off count
in October 2026 took every function, method and type defined in a public
repository and counted how many of its uses in code fall on a reference.
The count is rough: some of those uses are not references at all (a
local variable that shares a type's name, an import line).

| Language | Repository | Uses found as references |
|---|---|---|
| Go | spf13/cobra | 90% |
| Rust | fast_code_search `src/` | 53% |
| Java | google/gson | 52% |
| C# | JamesNK/Newtonsoft.Json | 50% |
| TypeScript | colinhacks/zod | 40% |
| TypeScript | sindresorhus/ky | 38% |
| C | DaveGamble/cJSON | 0% |
| C++ | fmtlib/fmt | 0% |

Closing these gaps is on the [roadmap](../project/roadmap.md).
