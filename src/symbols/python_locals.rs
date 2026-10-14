//! Python local variables are not references.
//!
//! References are names, so a parameter or local variable that happens to
//! share a function's name (`app`, `request`, `view`) would be listed as a
//! use of that function. Python decides scope syntactically: a name bound
//! anywhere in a function (a parameter, an assignment, a `for` or `with`
//! target) is local to the whole function unless declared `global` or
//! `nonlocal`. This drops references to such names, as Jedi would resolve
//! them to the local rather than the function.
//!
//! `def` and `class` names inside a function are not treated as locals:
//! a nested function is a function, and `return wrapped_view` is a use of
//! it. Imports inside a function are not either, since they bind the
//! imported function itself.

use super::extractor::SymbolRef;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// Scopes whose bindings are local: functions, lambdas and comprehensions.
fn is_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "lambda"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression"
    )
}

/// Identifiers a target binds: `a`, `a, b`, `(a, [b, *c])`; not `a.b` or
/// `a[0]`, which assign into an existing object.
fn target_names<'a>(node: Node<'a>, source: &'a str, out: &mut HashSet<&'a str>) {
    match node.kind() {
        "identifier" => {
            if let Some(t) = source.get(node.byte_range()) {
                out.insert(t);
            }
        }
        "pattern_list"
        | "tuple_pattern"
        | "list_pattern"
        | "list_splat_pattern"
        | "tuple"
        | "list"
        | "parenthesized_expression"
        | "as_pattern_target" => {
            let mut c = node.walk();
            for ch in node.named_children(&mut c) {
                target_names(ch, source, out);
            }
        }
        _ => {}
    }
}

/// Parameter names of a `parameters` or `lambda_parameters` node.
fn parameter_names<'a>(params: Node<'a>, source: &'a str, out: &mut HashSet<&'a str>) {
    let mut c = params.walk();
    for p in params.named_children(&mut c) {
        let name = match p.kind() {
            "identifier" => Some(p),
            "default_parameter" | "typed_default_parameter" => p.child_by_field_name("name"),
            "typed_parameter" | "list_splat_pattern" | "dictionary_splat_pattern" => {
                let mut c2 = p.walk();
                let first = p.named_children(&mut c2).find(|n| {
                    matches!(
                        n.kind(),
                        "identifier" | "list_splat_pattern" | "dictionary_splat_pattern"
                    )
                });
                first.map(|n| {
                    if n.kind() == "identifier" {
                        n
                    } else {
                        n.named_child(0).unwrap_or(n)
                    }
                })
            }
            _ => None,
        };
        if let Some(n) = name {
            target_names(n, source, out);
        }
    }
}

/// Names bound in `scope`'s own body, not in nested functions or classes.
fn bound_names<'a>(scope: Node<'a>, source: &'a str) -> HashSet<&'a str> {
    let mut bound = HashSet::new();
    let mut declared_outer = HashSet::new();
    if let Some(params) = scope.child_by_field_name("parameters") {
        parameter_names(params, source, &mut bound);
    }
    let mut stack: Vec<Node> = Vec::new();
    let mut c = scope.walk();
    for ch in scope.named_children(&mut c) {
        if ch.kind() != "parameters" && ch.kind() != "lambda_parameters" {
            stack.push(ch);
        }
    }
    while let Some(n) = stack.pop() {
        match n.kind() {
            // Their own scopes: a nested def's locals are not this one's.
            "function_definition" | "lambda" | "class_definition" => continue,
            "assignment" | "augmented_assignment" => {
                if let Some(left) = n.child_by_field_name("left") {
                    target_names(left, source, &mut bound);
                }
            }
            "for_statement" | "for_in_clause" => {
                if let Some(left) = n.child_by_field_name("left") {
                    target_names(left, source, &mut bound);
                }
            }
            "named_expression" => {
                if let Some(name) = n.child_by_field_name("name") {
                    target_names(name, source, &mut bound);
                }
            }
            "as_pattern" => {
                if let Some(alias) = n.child_by_field_name("alias") {
                    target_names(alias, source, &mut bound);
                }
            }
            "global_statement" | "nonlocal_statement" => {
                let mut c2 = n.walk();
                for id in n.named_children(&mut c2) {
                    if let Some(t) = source.get(id.byte_range()) {
                        declared_outer.insert(t);
                    }
                }
            }
            _ => {}
        }
        let mut c2 = n.walk();
        for ch in n.named_children(&mut c2) {
            stack.push(ch);
        }
    }
    bound.retain(|n| !declared_outer.contains(n));
    bound
}

/// `node` is (part of) what an assignment, `for` or `with` writes to.
fn is_assignment_target(node: Node) -> bool {
    let mut cur = node;
    while let Some(p) = cur.parent() {
        match p.kind() {
            "pattern_list" | "tuple_pattern" | "list_pattern" | "tuple" | "list" => cur = p,
            "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause" => {
                return p
                    .child_by_field_name("left")
                    .is_some_and(|l| l.id() == cur.id());
            }
            _ => return false,
        }
    }
    false
}

/// Drop the references that name a local variable of an enclosing
/// function, lambda or comprehension.
pub(crate) fn drop_locals(root: Node, source: &str, refs: &mut Vec<SymbolRef>) {
    let mut cache: HashMap<usize, HashSet<&str>> = HashMap::new();
    refs.retain(|r| {
        let point = tree_sitter::Point::new(r.line as usize, r.column as usize);
        let Some(node) = root.descendant_for_point_range(point, point) else {
            return true;
        };
        // `obj.name` looks `name` up on the object, never in the scope;
        // `obj.name = x` sets it, which is no use of it.
        if let Some(attr) = node.parent().filter(|p| {
            p.kind() == "attribute"
                && p.child_by_field_name("attribute")
                    .is_some_and(|a| a.id() == node.id())
        }) {
            return !is_assignment_target(attr);
        }
        let mut cur = node.parent();
        while let Some(n) = cur {
            if is_scope(n.kind()) {
                let bound = cache
                    .entry(n.id())
                    .or_insert_with(|| bound_names(n, source));
                if bound.contains(r.name.as_str()) {
                    return false;
                }
            }
            cur = n.parent();
        }
        true
    });
}
