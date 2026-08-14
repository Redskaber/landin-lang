//! HIR lowering tests.
//!
//! Per Stage 1.2 plan:
//! - Integration tests: all parse cases lower without panic
//! - Structural tests: 30+ tests verifying field-by-field AST→HIR equivalence

use landin_compiler::ast;
use landin_compiler::hir::*;
use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use lasso::Rodeo;

fn parse_and_lower(src: &str) -> HirCrate {
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    let errors = parser.into_errors();
    assert!(
        errors.is_empty(),
        "parse errors for {:?}: {:?}",
        src,
        errors
    );
    lower_crate(&krate, &interner).0
}

// =====================================================================
// Integration: all parse cases lower without panic
// =====================================================================

#[test]
fn integration_empty_crate() {
    let hir = parse_and_lower("");
    assert_eq!(hir.owner_count(), 0);
}

#[test]
fn integration_simple_fn() {
    let hir = parse_and_lower("fn main() {}");
    assert_eq!(hir.owner_count(), 1);
    assert_eq!(hir.body_count(), 1);
}

#[test]
fn integration_struct_with_fields() {
    let hir = parse_and_lower("struct Point { x: i32, y: i32 }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_enum_with_variants() {
    let hir = parse_and_lower("enum E { A, B(i32), C { x: i32 } }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_trait_with_items() {
    let hir = parse_and_lower("trait Foo { fn bar(&self); type Item; const X: i32; }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_impl_block() {
    let hir = parse_and_lower("impl Foo for Bar { fn baz(&self) {} }");
    assert_eq!(hir.owner_count(), 2); // 1 for impl, 1 for baz
}

#[test]
fn integration_module_inline() {
    let hir = parse_and_lower("mod m { fn foo() {} }");
    assert_eq!(hir.owner_count(), 2); // 1 for mod, 1 for foo
}

#[test]
fn integration_use_decl() {
    let hir = parse_and_lower("use std::io;");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_const_and_static() {
    let hir = parse_and_lower("const X: i32 = 42; static Y: &str = \"hi\";");
    assert_eq!(hir.owner_count(), 2);
    assert_eq!(hir.body_count(), 2);
}

#[test]
fn integration_complex_fn_body() {
    let hir = parse_and_lower(
        r#"
        fn fib(n: i64) -> i64 {
            if n < 2 { return n; }
            fib(n - 1) + fib(n - 2)
        }
        "#,
    );
    assert_eq!(hir.owner_count(), 1);
    assert_eq!(hir.body_count(), 1);
}

#[test]
fn integration_generics_and_where() {
    let hir = parse_and_lower("fn f<T>(x: T) -> T where T: Clone { x }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_struct_literal() {
    let hir = parse_and_lower("fn f() { let p = Point { x: 1, y: 2 }; }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_match_expression() {
    let hir = parse_and_lower("fn f(x: i32) { match x { 1 => 1, _ => 0 } }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_closure() {
    let hir = parse_and_lower("fn f() { let g = |x| x + 1; }");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_unsafe_fn() {
    let hir = parse_and_lower("unsafe fn foo() {}");
    assert_eq!(hir.owner_count(), 1);
}

#[test]
fn integration_extern_block() {
    let hir = parse_and_lower(r#"extern "C" { fn printf(fmt: *const u8) -> i32; }"#);
    assert_eq!(hir.owner_count(), 2); // 1 for extern block, 1 for printf
}

#[test]
fn integration_attributes() {
    let hir = parse_and_lower("#[derive(Debug)] pub fn foo() {}");
    assert_eq!(hir.owner_count(), 1);
}

// =====================================================================
// Structural: fn lowering
// =====================================================================

#[test]
fn struct_fn_name_preserved() {
    let hir = parse_and_lower("fn my_func() {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    match &owner.1 {
        OwnerNode::Item(HirItem::Fn(f)) => {
            // Can't resolve Spur without interner, but span should be non-empty
            assert!(f.ident.span.lo < f.ident.span.hi);
        }
        other => panic!("expected Fn, got {:?}", other),
    }
}

#[test]
fn struct_fn_body_present() {
    let hir = parse_and_lower("fn foo() { 42 }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        assert!(f.body.is_some(), "fn should have a body");
    } else {
        panic!("expected Fn");
    }
}

#[test]
fn struct_fn_inputs_preserved() {
    let hir = parse_and_lower("fn add(a: i32, b: i32) -> i32 { a + b }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        assert_eq!(f.sig.inputs.len(), 2, "should have 2 params");
    } else {
        panic!("expected Fn");
    }
}

#[test]
fn struct_fn_is_unsafe_preserved() {
    let hir = parse_and_lower("unsafe fn foo() {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        assert!(f.sig.is_unsafe, "should be unsafe");
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: struct lowering
// =====================================================================

#[test]
fn struct_struct_fields_preserved() {
    let hir = parse_and_lower("struct Point { x: i32, y: i32 }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Struct(s)) = &owner.1 {
        assert_eq!(s.fields.len(), 2, "should have 2 fields");
    } else {
        panic!("expected Struct");
    }
}

#[test]
fn struct_struct_is_unit() {
    let hir = parse_and_lower("struct Empty;");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Struct(s)) = &owner.1 {
        assert!(s.is_unit, "should be unit struct");
        assert!(s.fields.is_empty());
    } else {
        panic!("expected Struct");
    }
}

// =====================================================================
// Structural: enum lowering
// =====================================================================

#[test]
fn struct_enum_variants_preserved() {
    let hir = parse_and_lower("enum E { A, B(i32), C { x: i32 } }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Enum(e)) = &owner.1 {
        assert_eq!(e.variants.len(), 3, "should have 3 variants");
    } else {
        panic!("expected Enum");
    }
}

// =====================================================================
// Structural: trait lowering
// =====================================================================

#[test]
fn struct_trait_items_preserved() {
    let hir = parse_and_lower("trait Foo { fn bar(&self); type Item; const X: i32; }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Trait(t)) = &owner.1 {
        assert_eq!(t.items.len(), 3, "should have 3 trait items");
    } else {
        panic!("expected Trait");
    }
}

#[test]
fn struct_trait_supertraits_preserved() {
    let hir = parse_and_lower("trait Foo: Clone + Send {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Trait(t)) = &owner.1 {
        assert_eq!(t.supertraits.len(), 2, "should have 2 supertraits");
    } else {
        panic!("expected Trait");
    }
}

// =====================================================================
// Structural: impl lowering
// =====================================================================

#[test]
fn struct_impl_items_preserved() {
    let hir = parse_and_lower("impl Foo { fn bar(&self) {} fn baz(&self) {} }");
    // Should have 3 owners: impl + bar + baz
    assert_eq!(hir.owner_count(), 3);
}

// =====================================================================
// Structural: expr lowering (representative)
// =====================================================================

#[test]
fn struct_binary_expr_preserved() {
    let hir = parse_and_lower("fn f() { let _ = 1 + 2; }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let body_id = f.body.expect("should have body");
        let body = hir.body(body_id).expect("body should exist");
        // body.value should be a Block containing a Local stmt with a Binary init
        match &body.value.kind {
            HirExprKind::Block(block) => {
                assert!(block.stmts.len() == 1);
                if let HirStmt::Local(local) = &block.stmts[0] {
                    let init = local.init.as_ref().expect("should have init");
                    match &init.kind {
                        HirExprKind::Binary { op, .. } => {
                            assert_eq!(*op, HirBinOp::Add);
                        }
                        other => panic!("expected Binary, got {:?}", other),
                    }
                } else {
                    panic!("expected Local stmt");
                }
            }
            other => panic!("expected Block, got {:?}", other),
        }
    } else {
        panic!("expected Fn");
    }
}

#[test]
fn struct_lit_kind_preserved() {
    let hir = parse_and_lower("fn f() { 42 }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let body_id = f.body.expect("should have body");
        let body = hir.body(body_id).expect("body should exist");
        match &body.value.kind {
            HirExprKind::Block(block) => {
                let expr = block.expr.as_ref().expect("should have trailing expr");
                match &expr.kind {
                    HirExprKind::Lit(HirLitKind::Int(42, _)) => {}
                    other => panic!("expected Int(42), got {:?}", other),
                }
            }
            other => panic!("expected Block, got {:?}", other),
        }
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: path + Res placeholder
// =====================================================================

#[test]
fn struct_path_res_unknown() {
    let hir = parse_and_lower("fn f(x: i32) { x }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let body_id = f.body.expect("should have body");
        let body = hir.body(body_id).expect("body should exist");
        match &body.value.kind {
            HirExprKind::Block(block) => {
                let expr = block.expr.as_ref().expect("should have trailing expr");
                match &expr.kind {
                    HirExprKind::Path(p) => {
                        assert_eq!(
                            p.res,
                            Res::Unknown,
                            "Res should be Unknown before name resolution"
                        );
                    }
                    other => panic!("expected Path, got {:?}", other),
                }
            }
            other => panic!("expected Block, got {:?}", other),
        }
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: type lowering (InferTy placeholder)
// =====================================================================

#[test]
fn struct_ty_inferred_none() {
    let hir = parse_and_lower("fn f(x: i32) {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let param = &f.sig.inputs[0];
        match &param.ty {
            Some(ty) => {
                assert!(
                    ty.inferred.is_none(),
                    "InferTy should be None before typeck"
                );
                match &ty.kind {
                    HirTyKind::Int(ast::IntTy::I32) => {}
                    other => panic!("expected Int(I32), got {:?}", other),
                }
            }
            None => panic!("should have a type"),
        }
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: self_kind preservation
// =====================================================================

#[test]
fn struct_self_kind_ref_mut_preserved() {
    let hir = parse_and_lower("impl Foo { fn bar(&mut self) {} }");
    // Should have 2 owners: impl + bar
    assert_eq!(hir.owner_count(), 2);
    // Find the fn (second owner)
    let fn_owner = hir
        .owners
        .iter()
        .find(|(_, n)| matches!(n, OwnerNode::Item(HirItem::Fn(_))));
    let fn_owner = fn_owner.expect("should find Fn owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &fn_owner.1 {
        assert!(!f.sig.inputs.is_empty());
        let p = &f.sig.inputs[0];
        assert!(p.self_kind.is_some());
        assert_eq!(
            p.self_kind,
            Some(ast::SelfKind::Ref(ast::Mutability::Mutable))
        );
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: closure is_move preservation
// =====================================================================

#[test]
fn struct_closure_is_move_preserved() {
    let hir = parse_and_lower("fn f() { let g = move || 42; }");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let body_id = f.body.expect("should have body");
        let body = hir.body(body_id).expect("body should exist");
        match &body.value.kind {
            HirExprKind::Block(block) => {
                if let HirStmt::Local(local) = &block.stmts[0] {
                    let init = local.init.as_ref().expect("should have init");
                    match &init.kind {
                        HirExprKind::Closure { is_move, .. } => {
                            assert!(*is_move, "should be move closure");
                        }
                        other => panic!("expected Closure, got {:?}", other),
                    }
                }
            }
            other => panic!("expected Block, got {:?}", other),
        }
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// Structural: generics + where clause preservation
// =====================================================================

#[test]
fn struct_generics_preserved() {
    let hir = parse_and_lower("fn f<T, U>(x: T, y: U) {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        assert_eq!(f.generics.params.len(), 2, "should have 2 generic params");
    } else {
        panic!("expected Fn");
    }
}

#[test]
fn struct_where_clause_preserved() {
    let hir = parse_and_lower("fn f<T>(x: T) where T: Clone {}");
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        assert_eq!(
            f.generics.where_clause.len(),
            1,
            "should have 1 where predicate"
        );
    } else {
        panic!("expected Fn");
    }
}

// =====================================================================
// HirId uniqueness
// =====================================================================

#[test]
fn hir_id_uniqueness() {
    let hir = parse_and_lower("fn f() { let x = 1; let y = 2; }");
    // Collect all HirIds from the body
    let owner = hir.owners.first().expect("should have 1 owner");
    if let OwnerNode::Item(HirItem::Fn(f)) = &owner.1 {
        let body_id = f.body.expect("should have body");
        let body = hir.body(body_id).expect("body should exist");
        // The body's HirId + the Block's HirId + 2 Local HirIds + 2 Lit HirIds
        // should all be unique.
        let mut ids = vec![body.hir_id];
        if let HirExprKind::Block(block) = &body.value.kind {
            ids.push(block.hir_id);
            for stmt in &block.stmts {
                if let HirStmt::Local(local) = stmt {
                    ids.push(local.hir_id);
                    if let Some(init) = &local.init {
                        ids.push(init.hir_id);
                    }
                }
            }
        }
        // Check uniqueness
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            ids.len(),
            sorted.len(),
            "HirIds should be unique: {:?}",
            ids
        );
    } else {
        panic!("expected Fn");
    }
}
