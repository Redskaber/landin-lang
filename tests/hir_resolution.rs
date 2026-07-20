//! Name resolution tests (Stage 1.3).
//!
//! Verify that HirPath.res fields are correctly populated after resolution.

use landin_compiler::ast;
use landin_compiler::hir::lower::lower_crate;
use landin_compiler::hir::*;
use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use landin_compiler::resolve::resolve_crate;
use lasso::Rodeo;

fn parse_lower_resolve(src: &str) -> HirCrate {
    let mut interner = Rodeo::new();
    // Pre-intern keyword strings that the parser looks up via interner.get()
    // but never interns itself (parser only has &Rodeo). Without this, paths
    // like `Self` get Spur::default() and can't be resolved.
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");

    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    assert!(parser.into_errors().is_empty(), "parse errors");
    let mut hir = lower_crate(&krate, &interner);
    let _ = resolve_crate(&mut hir, &mut interner);
    hir
}

// === Primitive types are directly parsed (not via HirPath) ===

#[test]
fn prim_i32_direct() {
    let hir = parse_lower_resolve("fn f(x: i32) {}");
    let f = hir.owners.first().unwrap();
    let f = match &f.1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!("expected Fn"),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    assert!(matches!(&ty.kind, HirTyKind::Int(ast::IntTy::I32)));
}

#[test]
fn prim_bool_direct() {
    let hir = parse_lower_resolve("fn f(x: bool) {}");
    let f = hir.owners.first().unwrap();
    let f = match &f.1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!("expected Fn"),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    assert!(matches!(&ty.kind, HirTyKind::Bool));
}

#[test]
fn prim_all_16_types() {
    let cases = [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char",
    ];
    for name in cases {
        let hir = parse_lower_resolve(&format!("fn f(x: {}) {{}}", name));
        let f = hir.owners.first().unwrap();
        if let OwnerNode::Item(HirItem::Fn(f)) = &f.1 {
            let ty = f.sig.inputs[0].ty.as_ref().unwrap();
            assert!(
                !matches!(&ty.kind, HirTyKind::Path(_, _)),
                "{} should not be a Path",
                name
            );
        }
    }
}

// === User-defined type resolution ===

#[test]
fn struct_name_resolves_to_def() {
    let hir = parse_lower_resolve("struct Point { x: i32 } fn f(p: Point) {}");
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let f = match &fn_owner.unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    if let HirTyKind::Path(_, p) = &ty.kind {
        assert!(
            matches!(p.res, Res::Def(_, _)),
            "expected Def, got {:?}",
            p.res
        );
    } else {
        panic!("expected Path ty");
    }
}

#[test]
fn enum_name_resolves_to_def() {
    let hir = parse_lower_resolve("enum E { A, B } fn f(e: E) {}");
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let f = match &fn_owner.unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    if let HirTyKind::Path(_, p) = &ty.kind {
        assert!(matches!(p.res, Res::Def(_, _)));
    }
}

#[test]
fn trait_name_resolves_to_def() {
    let hir = parse_lower_resolve("trait Clone {} fn f<T: Clone>(x: T) {}");
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let f = match &fn_owner.unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    // Check that the bound `Clone` resolved
    let tp = match &f.generics.params[0] {
        HirGenericParam::Type(tp) => tp,
        _ => panic!("expected Type param"),
    };
    let bound = &tp.bounds[0];
    if let HirTypeBound::Trait(tb) = bound {
        assert!(
            matches!(tb.path.res, Res::Def(_, _)),
            "expected Clone to resolve"
        );
    }
}

#[test]
fn type_alias_resolves_to_def() {
    let hir = parse_lower_resolve("type Score = i32; fn f(x: Score) {}");
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let f = match &fn_owner.unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    if let HirTyKind::Path(_, p) = &ty.kind {
        assert!(matches!(p.res, Res::Def(_, _)));
    }
}

// === Function call resolution in body ===

#[test]
fn fn_call_path_resolves() {
    let hir = parse_lower_resolve("fn foo() {} fn main() { foo(); }");
    let found = hir.bodies.iter().any(|(_, body)| {
        if let HirExprKind::Block(block) = &body.value.kind {
            block.stmts.iter().any(|stmt| {
                if let HirStmt::Expr(expr, _) = stmt {
                    if let HirExprKind::Call { func, .. } = &expr.kind {
                        if let HirExprKind::Path(p) = &func.kind {
                            return matches!(p.res, Res::Def(_, _));
                        }
                    }
                }
                false
            })
        } else {
            false
        }
    });
    assert!(found, "foo() call should resolve to Res::Def");
}

#[test]
fn fn_ref_path_resolves() {
    let hir = parse_lower_resolve("fn foo() {} fn main() { let _ = foo; }");
    let found = hir.bodies.iter().any(|(_, body)| {
        if let HirExprKind::Block(block) = &body.value.kind {
            block.stmts.iter().any(|stmt| {
                if let HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        if let HirExprKind::Path(p) = &init.kind {
                            return matches!(p.res, Res::Def(_, _));
                        }
                    }
                }
                false
            })
        } else {
            false
        }
    });
    assert!(found, "foo reference should resolve");
}

// === Self type resolution ===

#[test]
fn self_type_resolves() {
    let hir = parse_lower_resolve("impl Foo { fn bar(x: Self) {} }");
    let found = hir.owners.iter().any(|(_, n)| {
        if let OwnerNode::Item(HirItem::Fn(f)) = n {
            for p in &f.sig.inputs {
                if let Some(ty) = &p.ty {
                    if let HirTyKind::Path(_, path) = &ty.kind {
                        if path.segments.len() == 1 {
                            return path.res == Res::SelfTy;
                        }
                    }
                }
            }
        }
        false
    });
    assert!(found, "Self should resolve to Res::SelfTy");
}

// === Unknown name → Res::Err ===

#[test]
fn unknown_type_is_err() {
    let hir = parse_lower_resolve("fn f(x: Undefined) {}");
    let f = match &hir.owners.first().unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    let ty = f.sig.inputs[0].ty.as_ref().unwrap();
    if let HirTyKind::Path(_, p) = &ty.kind {
        assert_eq!(p.res, Res::Err);
    }
}

// === Struct literal path resolution ===

#[test]
fn struct_literal_path_resolves() {
    let hir = parse_lower_resolve("struct P { x: i32 } fn f() { let _ = P { x: 1 }; }");
    let found = hir.bodies.iter().any(|(_, body)| {
        if let HirExprKind::Block(block) = &body.value.kind {
            block.stmts.iter().any(|stmt| {
                if let HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        if let HirExprKind::Struct { path, .. } = &init.kind {
                            return matches!(path.res, Res::Def(_, _));
                        }
                    }
                }
                false
            })
        } else {
            false
        }
    });
    assert!(found, "struct literal path should resolve");
}

// === Where clause bound resolution ===

#[test]
fn where_clause_bound_resolves() {
    let hir = parse_lower_resolve("trait C {} fn f<T>(x: T) where T: C { x }");
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let f = match &fn_owner.unwrap().1 {
        OwnerNode::Item(HirItem::Fn(f)) => f,
        _ => panic!(),
    };
    assert_eq!(f.generics.where_clause.len(), 1);
    let bound = &f.generics.where_clause[0].bounds[0];
    if let HirTypeBound::Trait(tb) = bound {
        assert!(matches!(tb.path.res, Res::Def(_, _)));
    }
}

// === No Res::Unknown remaining ===

#[test]
fn no_unknown_after_resolution() {
    let hir = parse_lower_resolve("fn foo() {} fn main() { foo(); }");
    let mut found = false;
    for (_, body) in &hir.bodies {
        check_expr(&body.value, &mut found);
    }
    assert!(!found, "found Res::Unknown after resolution");
}

fn check_expr(expr: &HirExpr, found: &mut bool) {
    match &expr.kind {
        HirExprKind::Path(p) => {
            if p.res == Res::Unknown {
                *found = true;
            }
        }
        HirExprKind::Block(b) => {
            for s in &b.stmts {
                if let HirStmt::Local(l) = s {
                    if let Some(i) = &l.init {
                        check_expr(i, found);
                    }
                }
                if let HirStmt::Expr(e, _) = s {
                    check_expr(e, found);
                }
            }
            if let Some(e) = &b.expr {
                check_expr(e, found);
            }
        }
        HirExprKind::Call { func, args } => {
            check_expr(func, found);
            for a in args {
                check_expr(a, found);
            }
        }
        _ => {}
    }
}

// === Integration: existing programs still work ===

#[test]
fn integration_complex_program() {
    let hir = parse_lower_resolve(
        r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn new(x: i32, y: i32) -> Point { Point { x, y } }
        }
        fn main() { let p = Point::new(1, 2); }
    "#,
    );
    assert!(hir.owner_count() >= 3); // Point + impl + new + main
}

#[test]
fn integration_generics_and_where() {
    let hir = parse_lower_resolve("fn f<T: Clone + Send>(x: T) -> T where T: 'static { x }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_unsafe_and_extern() {
    let hir = parse_lower_resolve("unsafe fn foo() {} extern \"C\" { fn bar(); }");
    assert!(hir.owner_count() >= 2);
}
