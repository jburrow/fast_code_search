; fast_code_search supplement to the Python grammar's tags.scm, which only
; records calls. Checked against Jedi with examples/ref_oracle.rs; see
; docs/book/src/reference/reference-support.md. Local variables that share
; a function's name, and attributes being assigned, are dropped afterwards
; (src/symbols/python_locals.rs).

; Base classes: class Foo(Base, pkg.Mixin)
(class_definition
  superclasses: (argument_list (identifier) @name)) @reference.class

; Decorators without a call: @login_required, @bp.before_app_request
(decorator (identifier) @name) @reference.call
(decorator (attribute attribute: (identifier) @name)) @reference.call

; Every attribute read: obj.method passed as a callback, a property,
; pkg.Class in an annotation (kind:attribute; a call of it stays kind:call)
(attribute attribute: (identifier) @name) @reference.attribute

; The object of a decorator attribute: @permanent.setter
(decorator (attribute object: (identifier) @name)) @reference.call

; A class used through an attribute: Item.from_dict(...)
((attribute object: (identifier) @name) @reference.class
  (#match? @name "^[A-Z]"))

; Type annotations: x: Item, -> Result, list[Item]
(type (identifier) @name) @reference.type
(generic_type (identifier) @name) @reference.type
(type_parameter (type (identifier) @name)) @reference.type

; Functions and classes used as values: passed, compared, assigned,
; returned, collected
(argument_list (identifier) @name) @reference.value
(keyword_argument value: (identifier) @name) @reference.value
(comparison_operator (identifier) @name) @reference.value
(assignment right: (identifier) @name) @reference.value
(return_statement (identifier) @name) @reference.value
(tuple (identifier) @name) @reference.value
(list (identifier) @name) @reference.value
(set (identifier) @name) @reference.value
(pair value: (identifier) @name) @reference.value
(boolean_operator (identifier) @name) @reference.value
(conditional_expression (identifier) @name) @reference.value

; Unions and subscripts, in annotations and type aliases: A | B, Callable[[A], B]
(binary_operator (identifier) @name) @reference.type
(subscript value: (identifier) @name) @reference.type
(subscript subscript: (identifier) @name) @reference.type

; Exceptions: except E as e:, raise E
(except_clause (identifier) @name) @reference.class
(except_clause (as_pattern . (identifier) @name)) @reference.class
(raise_statement (identifier) @name) @reference.class
