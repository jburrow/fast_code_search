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
use tree_sitter::{Node, Point};

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

/// `path[i]` is (part of) what an assignment, `for` or `with` writes to;
/// `path` holds its ancestors from the root down.
fn is_assignment_target(path: &[Node], i: usize) -> bool {
    let mut cur = path[i];
    for &p in path[..i].iter().rev() {
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

/// A function, lambda or comprehension, and the nearest one around it.
struct Scope<'a> {
    node: Node<'a>,
    start: Point,
    end: Point,
    parent: Option<usize>,
}

/// Drop the references that name a local variable of an enclosing
/// function, lambda or comprehension.
///
/// One walk over the tree collects the scopes and the attribute names;
/// each reference is then placed by position. Looking each reference up
/// from the root and climbing with `Node::parent` (which itself re-walks
/// from the root) cost a fifth of Django's whole build.
pub(crate) fn drop_locals(root: Node, source: &str, refs: &mut Vec<SymbolRef>) {
    if refs.is_empty() {
        return;
    }
    // Scopes in document order; `obj.name` positions -> keep the reference.
    let mut scopes: Vec<Scope> = Vec::new();
    let mut attributes: HashMap<Point, bool> = HashMap::new();
    let mut path: Vec<Node> = Vec::new();
    let mut open_scopes: Vec<(usize, usize)> = Vec::new(); // (scope, depth)
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        let depth = path.len();
        while open_scopes.last().is_some_and(|&(_, d)| d >= depth) {
            open_scopes.pop();
        }
        path.push(node);
        if is_scope(node.kind()) {
            scopes.push(Scope {
                node,
                start: node.start_position(),
                end: node.end_position(),
                parent: open_scopes.last().map(|&(i, _)| i),
            });
            open_scopes.push((scopes.len() - 1, depth));
        }
        // `obj.name` looks `name` up on the object, never in the scope;
        // `obj.name = x` sets it, which is no use of it.
        if node.kind() == "attribute" {
            if let Some(name) = node.child_by_field_name("attribute") {
                attributes.insert(
                    name.start_position(),
                    !is_assignment_target(&path, path.len() - 1),
                );
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        path.pop();
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                break;
            }
            path.pop();
        }
        if path.is_empty() {
            break;
        }
    }

    let mut bound: HashMap<usize, HashSet<&str>> = HashMap::new();
    refs.retain(|r| {
        let point = Point::new(r.line as usize, r.column as usize);
        if let Some(&keep) = attributes.get(&point) {
            return keep;
        }
        // The innermost scope around the point is the last one opened
        // before it, or one of that scope's enclosing scopes.
        let opened = scopes.partition_point(|s| s.start <= point);
        let mut at = opened.checked_sub(1);
        while let Some(i) = at {
            let scope = &scopes[i];
            if scope.start <= point && point < scope.end {
                let names = bound
                    .entry(i)
                    .or_insert_with(|| bound_names(scope.node, source));
                if names.contains(r.name.as_str()) {
                    return false;
                }
            }
            at = scope.parent;
        }
        true
    });
}
