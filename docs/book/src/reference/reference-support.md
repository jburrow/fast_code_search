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
- **Only the listed positions.** A use in any other position (a function
  passed as a value, an import line, a property read) is not a reference,
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
| Python | yes | yes | **no** | yes (a call) | **no** | type annotations, base classes, bare `@decorator`, functions as values, attribute reads, imports |
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

Python records **calls only**.

| Form | Found |
|---|---|
| `helper(x)`, `self.helper()`, `pkg.util.do_thing()`, `Foo()` | yes |
| `@app.route("/x")` (the `route`) | yes, it is a call |
| Bare `@decorator` | **no** |
| Base classes: `class Foo(Base, Mixin)` | **no** |
| Type annotations: `item: Item`, `-> Result`, `list[Item]` | **no** |
| Classes and functions passed as values: `isinstance(x, Item)`, `cb = helper`, `return Result` | **no** |
| Attribute reads: `self.prop`, `obj.CONSTANT` | **no** |
| `import`, `from … import` | **no** |

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

There is no reference oracle yet (the import graph has one, see
[Dependency graph support](graph-support.md)). Each form above is checked
with a small file per language. To give a sense of scale, a one-off count
in October 2026 took every function, method and type defined in a public
repository and counted how many of its uses in code fall on a reference.
The count is rough: some of those uses are not references at all (a
local variable that shares a type's name, an import line).

| Language | Repository | Uses found as references |
|---|---|---|
| Go | spf13/cobra | 90% |
| Rust | fast_code_search `src/` | 53% |
| Python | psf/requests | 53% |
| Java | google/gson | 52% |
| C# | JamesNK/Newtonsoft.Json | 50% |
| Python | pallets/flask | 44% |
| TypeScript | colinhacks/zod | 40% |
| TypeScript | sindresorhus/ky | 38% |
| C | DaveGamble/cJSON | 0% |
| C++ | fmtlib/fmt | 0% |

Closing these gaps is on the [roadmap](../project/roadmap.md).
