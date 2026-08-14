//! Scope-based name resolution tests (Stage 1.4).

use landin_compiler::hir::lower::lower_crate;
use landin_compiler::hir::*;
use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use landin_compiler::resolve::resolve_crate;
use lasso::Rodeo;

fn parse_lower_resolve(src: &str) -> (HirCrate, Rodeo) {
    let mut interner = Rodeo::new();
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    assert!(parser.into_errors().is_empty());
    let mut hir = lower_crate(&krate, &interner).0;
    let _ = resolve_crate(&mut hir, &mut interner);
    (hir, interner)
}

fn find_res(hir: &HirCrate, interner: &Rodeo, name: &str) -> Option<Res> {
    let target = interner.get(name)?;
    for (_, body) in &hir.bodies {
        if let Some(r) = walk(body.value.kind_ref(), target) {
            return Some(r);
        }
    }
    None
}

/// Get a reference to the HirExprKind for convenience.
trait KindRef {
    fn kind_ref(&self) -> &HirExprKind;
}
impl KindRef for HirExpr {
    fn kind_ref(&self) -> &HirExprKind {
        &self.kind
    }
}

fn walk(kind: &HirExprKind, target: lasso::Spur) -> Option<Res> {
    match kind {
        HirExprKind::Path(p) if p.segments.len() == 1 && p.segments[0].ident.name == target => {
            Some(p.res)
        }
        HirExprKind::Block(b) => {
            for s in &b.stmts {
                match s {
                    HirStmt::Local(l) => {
                        if let Some(i) = &l.init {
                            if let Some(r) = walk(&i.kind, target) {
                                return Some(r);
                            }
                        }
                    }
                    HirStmt::Expr(e, _) => {
                        if let Some(r) = walk(&e.kind, target) {
                            return Some(r);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(e) = &b.expr {
                return walk(&e.kind, target);
            }
            None
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            walk(&lhs.kind, target).or_else(|| walk(&rhs.kind, target))
        }
        HirExprKind::Call { func, args } => {
            walk(&func.kind, target).or_else(|| args.iter().find_map(|a| walk(&a.kind, target)))
        }
        HirExprKind::Closure { body, .. } => walk(&body.kind, target),
        HirExprKind::Match { arms, .. } => {
            for arm in arms {
                if let Some(r) = walk(&arm.body.kind, target) {
                    return Some(r);
                }
            }
            None
        }
        HirExprKind::For { body, .. } => walk(&HirExprKind::Block(body.clone()), target),
        HirExprKind::If { cond, then, else_ } => walk(&cond.kind, target)
            .or_else(|| walk(&HirExprKind::Block(then.clone()), target))
            .or_else(|| else_.as_ref().and_then(|e| walk(&e.kind, target))),
        _ => None,
    }
}

#[test]
fn let_binding_resolves_to_local() {
    let (hir, int) = parse_lower_resolve("fn f() { let x = 42; x }");
    assert!(matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))));
}

#[test]
fn let_binding_in_init() {
    let (hir, int) = parse_lower_resolve("fn f() { let x = 1; let y = x; }");
    assert!(matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))));
}

#[test]
fn multiple_let_bindings() {
    let (hir, int) = parse_lower_resolve("fn f() { let a = 1; let b = 2; let c = a + b; }");
    assert!(matches!(find_res(&hir, &int, "a"), Some(Res::Local(_))));
    assert!(matches!(find_res(&hir, &int, "b"), Some(Res::Local(_))));
}

#[test]
fn fn_param_resolves_to_local() {
    let (hir, int) = parse_lower_resolve("fn f(x: i32) { x }");
    assert!(matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))));
}

#[test]
fn fn_params_in_binary() {
    let (hir, int) = parse_lower_resolve("fn add(a: i32, b: i32) -> i32 { a + b }");
    assert!(matches!(find_res(&hir, &int, "a"), Some(Res::Local(_))));
    assert!(matches!(find_res(&hir, &int, "b"), Some(Res::Local(_))));
}

#[test]
fn shadowing_inner_shadows_outer() {
    let (hir, int) = parse_lower_resolve("fn f() { let x = 1; { let x = 2; x } }");
    assert!(matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))));
}

#[test]
fn re_binding_in_same_scope() {
    let (hir, int) = parse_lower_resolve("fn f() { let x = 1; let x = 2; x }");
    assert!(matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))));
}

#[test]
fn forward_reference_is_not_local() {
    let (hir, int) = parse_lower_resolve("fn f() { let x = x; }");
    // The `x` in the init should NOT be Local (forward ref)
    let res = find_res(&hir, &int, "x");
    assert!(
        !matches!(res, Some(Res::Local(_))),
        "forward ref should not be Local, got {:?}",
        res
    );
}

#[test]
fn closure_param_resolves() {
    let (hir, int) = parse_lower_resolve("fn f() { let g = |x| x; }");
    assert!(
        matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))),
        "closure param should be Local"
    );
}

#[test]
fn closure_accesses_outer() {
    let (hir, int) = parse_lower_resolve("fn f() { let outer = 42; let g = |x| outer; }");
    assert!(matches!(find_res(&hir, &int, "outer"), Some(Res::Local(_))));
}

#[test]
fn match_arm_binds_variable() {
    let (hir, int) = parse_lower_resolve("fn f(x: i32) { match x { n => n } }");
    assert!(
        matches!(find_res(&hir, &int, "n"), Some(Res::Local(_))),
        "match arm binding should be Local"
    );
}

#[test]
fn for_loop_binds_variable() {
    let (hir, int) = parse_lower_resolve("fn f() { for x in 0..10 { x } }");
    assert!(
        matches!(find_res(&hir, &int, "x"), Some(Res::Local(_))),
        "for-loop binding should be Local"
    );
}

#[test]
fn local_shadows_fn() {
    let (hir, int) = parse_lower_resolve("fn foo() {} fn main() { let foo = 42; foo }");
    assert!(
        matches!(find_res(&hir, &int, "foo"), Some(Res::Local(_))),
        "local should shadow fn"
    );
}

#[test]
fn fn_call_when_no_local() {
    let (hir, int) = parse_lower_resolve("fn foo() {} fn main() { foo() }");
    assert!(
        matches!(find_res(&hir, &int, "foo"), Some(Res::Def(_, _))),
        "should resolve to Def when no local"
    );
}

#[test]
fn block_scope_does_not_leak() {
    let (hir, int) = parse_lower_resolve("fn f() { { let x = 1; } x }");
    let res = find_res(&hir, &int, "x");
    assert!(
        !matches!(res, Some(Res::Local(_))),
        "block-scoped binding should not leak, got {:?}",
        res
    );
}

#[test]
fn integration_fibonacci() {
    let (hir, int) = parse_lower_resolve(
        "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) }",
    );
    assert!(matches!(find_res(&hir, &int, "n"), Some(Res::Local(_))));
}

#[test]
fn no_unknown_after_scope_resolution() {
    let (hir, _) = parse_lower_resolve("fn f(x: i32) { let y = x; y + x }");
    let body = hir.bodies.first().map(|(_, b)| b).unwrap();
    let mut found = false;
    check_unknown(&body.value, &mut found);
    assert!(!found, "found Res::Unknown");
}

fn check_unknown(expr: &HirExpr, found: &mut bool) {
    match &expr.kind {
        HirExprKind::Path(p) if p.res == Res::Unknown => *found = true,
        HirExprKind::Block(b) => {
            for s in &b.stmts {
                if let HirStmt::Local(l) = s {
                    if let Some(i) = &l.init {
                        check_unknown(i, found);
                    }
                }
                if let HirStmt::Expr(e, _) = s {
                    check_unknown(e, found);
                }
            }
            if let Some(e) = &b.expr {
                check_unknown(e, found);
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            check_unknown(lhs, found);
            check_unknown(rhs, found);
        }
        _ => {}
    }
}
