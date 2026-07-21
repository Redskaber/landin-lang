//! HIR data structure tests.
//!
//! Per Stage 1.1 plan task D1: 30+ tests covering HIR node construction,
//! Debug round-trip, HirId propagation, and structural assertions.
//!
//! These tests construct HIR nodes directly (no AST→HIR lowering yet —
//! that is Stage 1.2). They verify the data structures are usable and
//! correctly typed.

use landin_compiler::ast::{Ident, IntTy, Mutability, PathLeading, UintTy};
use landin_compiler::hir::*;
use landin_compiler::lexer::Symbol;
use landin_compiler::session::Span;
use lasso::Rodeo;

// Helper: get an interned ident for "foo". Takes a mutable Rodeo because
// `get_or_intern` requires it.
fn intern_ident(interner: &mut Rodeo, name: &str) -> Ident {
    let sym = interner.get_or_intern(name);
    Ident::new(sym, Span::DUMMY)
}

// =====================================================================
// HirItem variant construction + Debug round-trip (5 tests)
// =====================================================================

#[test]
fn hir_fn_construction() {
    let mut interner = Rodeo::new();
    let ident = intern_ident(&mut interner, "foo");
    let hir_id = HirId::new(DefId(0), ItemLocalId(0));
    let fn_decl = HirFn {
        hir_id,
        ident,
        generics: HirGenerics::default(),
        sig: HirFnSig {
            inputs: vec![],
            output: HirFnRetTy::Default(Span::DUMMY),
            abi: landin_compiler::ast::Abi::Landin,
            is_unsafe: false,
            span: Span::DUMMY,
        },
        body: None,
        vis: landin_compiler::ast::Visibility::Private,
        attrs: vec![],
        span: Span::DUMMY,
    };
    let item = HirItem::Fn(fn_decl);
    let debug = format!("{:?}", item);
    assert!(debug.contains("HirFn"));
    // Ident Debug shows Spur(N), not the string — just check the variant name
}

#[test]
fn hir_struct_construction() {
    let mut interner = Rodeo::new();
    let ident = intern_ident(&mut interner, "Point");
    let hir_id = HirId::new(DefId(1), ItemLocalId(0));
    let struct_decl = HirStruct {
        hir_id,
        ident,
        generics: HirGenerics::default(),
        fields: vec![],
        is_unit: true,
        is_tuple: false,
        vis: landin_compiler::ast::Visibility::Public,
        attrs: vec![],
        span: Span::DUMMY,
    };
    let item = HirItem::Struct(struct_decl);
    let debug = format!("{:?}", item);
    assert!(debug.contains("HirStruct"));
    let _ = &debug;
    assert!(debug.contains("Public"));
}

#[test]
fn hir_enum_construction() {
    let mut interner = Rodeo::new();
    let ident = intern_ident(&mut interner, "Color");
    let hir_id = HirId::new(DefId(2), ItemLocalId(0));
    let enum_decl = HirEnum {
        hir_id,
        ident,
        generics: HirGenerics::default(),
        variants: vec![],
        vis: landin_compiler::ast::Visibility::Private,
        attrs: vec![],
        span: Span::DUMMY,
    };
    let item = HirItem::Enum(enum_decl);
    let debug = format!("{:?}", item);
    assert!(debug.contains("HirEnum"));
    let _ = &debug;
}

#[test]
fn hir_trait_construction() {
    let mut interner = Rodeo::new();
    let ident = intern_ident(&mut interner, "Display");
    let hir_id = HirId::new(DefId(3), ItemLocalId(0));
    let trait_decl = HirTrait {
        hir_id,
        ident,
        generics: HirGenerics::default(),
        supertraits: vec![],
        items: vec![],
        vis: landin_compiler::ast::Visibility::Public,
        attrs: vec![],
        is_unsafe: false,
        span: Span::DUMMY,
    };
    let item = HirItem::Trait(trait_decl);
    let debug = format!("{:?}", item);
    assert!(debug.contains("HirTrait"));
    let _ = &debug;
}

#[test]
fn hir_impl_construction() {
    let hir_id = HirId::new(DefId(4), ItemLocalId(0));
    let impl_decl = HirImpl {
        hir_id,
        generics: HirGenerics::default(),
        of_trait: None,
        self_ty: HirTy {
            hir_id: HirId::new(DefId(4), ItemLocalId(1)),
            kind: HirTyKind::Bool,
            inferred: None,
            span: Span::DUMMY,
        },
        items: vec![],
        attrs: vec![],
        is_unsafe: false,
        span: Span::DUMMY,
    };
    let item = HirItem::Impl(impl_decl);
    let debug = format!("{:?}", item);
    assert!(debug.contains("HirImpl"));
}

// =====================================================================
// Body construction (5 tests)
// =====================================================================

#[test]
fn hir_body_fn_empty() {
    // An empty fn body: `fn foo() {}`
    let hir_id = HirId::new(DefId(0), ItemLocalId(0));
    let body = Body {
        hir_id,
        params: vec![],
        value: HirExpr {
            hir_id: HirId::new(DefId(0), ItemLocalId(1)),
            kind: HirExprKind::Block(HirBlock {
                hir_id: HirId::new(DefId(0), ItemLocalId(2)),
                stmts: vec![],
                expr: None,
                span: Span::DUMMY,
            }),
            span: Span::DUMMY,
        },
        span: Span::DUMMY,
    };
    let debug = format!("{:?}", body);
    assert!(debug.contains("Body"));
    assert!(debug.contains("Block"));
}

#[test]
fn hir_body_const_initializer() {
    // A const body: `const X: i32 = 42;`
    let body = Body {
        hir_id: HirId::new(DefId(1), ItemLocalId(0)),
        params: vec![],
        value: HirExpr {
            hir_id: HirId::new(DefId(1), ItemLocalId(1)),
            kind: HirExprKind::Lit(HirLitKind::Int(42, None)),
            span: Span::DUMMY,
        },
        span: Span::DUMMY,
    };
    match body.value.kind {
        HirExprKind::Lit(HirLitKind::Int(n, _)) => assert_eq!(n, 42),
        ref other => panic!("expected Int lit, got {:?}", other),
    }
}

#[test]
fn hir_body_static_initializer() {
    let body = Body {
        hir_id: HirId::new(DefId(2), ItemLocalId(0)),
        params: vec![],
        value: HirExpr {
            hir_id: HirId::new(DefId(2), ItemLocalId(1)),
            kind: HirExprKind::Lit(HirLitKind::Bool(true)),
            span: Span::DUMMY,
        },
        span: Span::DUMMY,
    };
    match body.value.kind {
        HirExprKind::Lit(HirLitKind::Bool(b)) => assert!(b),
        ref other => panic!("expected Bool lit, got {:?}", other),
    }
}

#[test]
fn hir_body_with_params() {
    // A fn body with params: `fn add(a: i32, b: i32) -> i32 { a + b }`
    let mut interner = Rodeo::new();
    let a_ident = intern_ident(&mut interner, "a");
    let b_ident = intern_ident(&mut interner, "b");
    let body = Body {
        hir_id: HirId::new(DefId(3), ItemLocalId(0)),
        params: vec![
            HirParam {
                hir_id: HirId::new(DefId(3), ItemLocalId(1)),
                pat: HirPat {
                    hir_id: HirId::new(DefId(3), ItemLocalId(2)),
                    kind: HirPatKind::Ident(
                        landin_compiler::ast::BindingMode::ByValue(Mutability::Immutable),
                        a_ident,
                        None,
                    ),
                    span: Span::DUMMY,
                },
                ty: Some(HirTy {
                    hir_id: HirId::new(DefId(3), ItemLocalId(3)),
                    kind: HirTyKind::Int(IntTy::I32),
                    inferred: None,
                    span: Span::DUMMY,
                }),
                self_kind: None,
                span: Span::DUMMY,
            },
            HirParam {
                hir_id: HirId::new(DefId(3), ItemLocalId(4)),
                pat: HirPat {
                    hir_id: HirId::new(DefId(3), ItemLocalId(5)),
                    kind: HirPatKind::Ident(
                        landin_compiler::ast::BindingMode::ByValue(Mutability::Immutable),
                        b_ident,
                        None,
                    ),
                    span: Span::DUMMY,
                },
                ty: Some(HirTy {
                    hir_id: HirId::new(DefId(3), ItemLocalId(6)),
                    kind: HirTyKind::Int(IntTy::I32),
                    inferred: None,
                    span: Span::DUMMY,
                }),
                self_kind: None,
                span: Span::DUMMY,
            },
        ],
        value: HirExpr {
            hir_id: HirId::new(DefId(3), ItemLocalId(7)),
            kind: HirExprKind::Block(HirBlock {
                hir_id: HirId::new(DefId(3), ItemLocalId(8)),
                stmts: vec![],
                expr: None,
                span: Span::DUMMY,
            }),
            span: Span::DUMMY,
        },
        span: Span::DUMMY,
    };
    assert_eq!(body.params.len(), 2);
    assert!(body.params[0].self_kind.is_none());
}

#[test]
fn hir_body_closure_body() {
    // Closure body is inlined in HirExprKind::Closure (not a separate BodyId
    // in Stage 1.1 — see committee note about future refactor).
    let closure_expr = HirExpr {
        hir_id: HirId::new(DefId(5), ItemLocalId(0)),
        kind: HirExprKind::Closure {
            is_move: false,
            params: vec![],
            body: Box::new(HirExpr {
                hir_id: HirId::new(DefId(5), ItemLocalId(1)),
                kind: HirExprKind::Lit(HirLitKind::Int(42, None)),
                span: Span::DUMMY,
            }),
        },
        span: Span::DUMMY,
    };
    match closure_expr.kind {
        HirExprKind::Closure { is_move, body, .. } => {
            assert!(!is_move);
            match body.kind {
                HirExprKind::Lit(HirLitKind::Int(n, _)) => assert_eq!(n, 42),
                ref other => panic!("expected Int lit, got {:?}", other),
            }
        }
        ref other => panic!("expected Closure, got {:?}", other),
    }
}

// =====================================================================
// HirExpr representative variants (3 tests)
// =====================================================================

#[test]
fn hir_expr_binary_construction() {
    let lhs = Box::new(HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirExprKind::Lit(HirLitKind::Int(1, None)),
        span: Span::DUMMY,
    });
    let rhs = Box::new(HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(2)),
        kind: HirExprKind::Lit(HirLitKind::Int(2, None)),
        span: Span::DUMMY,
    });
    let expr = HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(3)),
        kind: HirExprKind::Binary {
            op: HirBinOp::Add,
            lhs,
            rhs,
        },
        span: Span::DUMMY,
    };
    match expr.kind {
        HirExprKind::Binary { op, lhs, rhs } => {
            assert_eq!(op, HirBinOp::Add);
            assert_eq!(lhs.hir_id.local_id, ItemLocalId(1));
            assert_eq!(rhs.hir_id.local_id, ItemLocalId(2));
        }
        _ => panic!("expected Binary"),
    }
}

#[test]
fn hir_expr_if_construction() {
    let cond = Box::new(HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirExprKind::Lit(HirLitKind::Bool(true)),
        span: Span::DUMMY,
    });
    let then_block = HirBlock {
        hir_id: HirId::new(DefId(0), ItemLocalId(2)),
        stmts: vec![],
        expr: None,
        span: Span::DUMMY,
    };
    let expr = HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(3)),
        kind: HirExprKind::If {
            cond,
            then: then_block,
            else_: None,
        },
        span: Span::DUMMY,
    };
    match expr.kind {
        HirExprKind::If { cond, then, else_ } => {
            assert!(else_.is_none());
            assert_eq!(cond.hir_id.local_id, ItemLocalId(1));
            assert_eq!(then.hir_id.local_id, ItemLocalId(2));
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn hir_expr_match_construction() {
    let arm = HirArm {
        hir_id: HirId::new(DefId(0), ItemLocalId(2)),
        pat: HirPat {
            hir_id: HirId::new(DefId(0), ItemLocalId(3)),
            kind: HirPatKind::Wild,
            span: Span::DUMMY,
        },
        guard: None,
        body: Box::new(HirExpr {
            hir_id: HirId::new(DefId(0), ItemLocalId(4)),
            kind: HirExprKind::Lit(HirLitKind::Int(0, None)),
            span: Span::DUMMY,
        }),
        span: Span::DUMMY,
    };
    let expr = HirExpr {
        hir_id: HirId::new(DefId(0), ItemLocalId(5)),
        kind: HirExprKind::Match {
            expr: Box::new(HirExpr {
                hir_id: HirId::new(DefId(0), ItemLocalId(1)),
                kind: HirExprKind::Lit(HirLitKind::Int(42, None)),
                span: Span::DUMMY,
            }),
            arms: vec![arm],
        },
        span: Span::DUMMY,
    };
    match expr.kind {
        HirExprKind::Match { arms, .. } => assert_eq!(arms.len(), 1),
        _ => panic!("expected Match"),
    }
}

// =====================================================================
// HirPat representative variants (3 tests)
// =====================================================================

#[test]
fn hir_pat_ident_with_binding_mode() {
    let mut interner = Rodeo::new();
    let ident = intern_ident(&mut interner, "x");
    let pat = HirPat {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirPatKind::Ident(
            landin_compiler::ast::BindingMode::ByValue(Mutability::Mutable),
            ident,
            None,
        ),
        span: Span::DUMMY,
    };
    match pat.kind {
        HirPatKind::Ident(mode, _, _) => {
            assert_eq!(
                mode,
                landin_compiler::ast::BindingMode::ByValue(Mutability::Mutable)
            );
        }
        _ => panic!("expected Ident pat"),
    }
}

#[test]
fn hir_pat_tuple_struct() {
    let mut interner = Rodeo::new();
    let sym = interner.get_or_intern("Some");
    let hir_path = HirPath {
        hir_id: HirId::new(DefId(0), ItemLocalId(50)),
        segments: vec![HirPathSegment {
            ident: Ident::new(sym, Span::DUMMY),
            args: None,
        }],
        leading: PathLeading::None,
        res: Res::Unknown,
        span: Span::DUMMY,
    };
    let inner_pat = HirPat {
        hir_id: HirId::new(DefId(0), ItemLocalId(2)),
        kind: HirPatKind::Wild,
        span: Span::DUMMY,
    };
    let pat = HirPat {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirPatKind::TupleStruct(hir_path, vec![inner_pat]),
        span: Span::DUMMY,
    };
    match pat.kind {
        HirPatKind::TupleStruct(_, pats) => assert_eq!(pats.len(), 1),
        _ => panic!("expected TupleStruct pat"),
    }
}

#[test]
fn hir_pat_or() {
    let lit = |id: u32, n: u128| HirPat {
        hir_id: HirId::new(DefId(0), ItemLocalId(id)),
        kind: HirPatKind::Lit(Box::new(HirExpr {
            hir_id: HirId::new(DefId(0), ItemLocalId(id + 10)),
            kind: HirExprKind::Lit(HirLitKind::Int(n, None)),
            span: Span::DUMMY,
        })),
        span: Span::DUMMY,
    };
    let pat = HirPat {
        hir_id: HirId::new(DefId(0), ItemLocalId(99)),
        kind: HirPatKind::Or(vec![lit(1, 1), lit(2, 2), lit(3, 3)]),
        span: Span::DUMMY,
    };
    match pat.kind {
        HirPatKind::Or(pats) => assert_eq!(pats.len(), 3),
        _ => panic!("expected Or pat"),
    }
}

// =====================================================================
// HirTy representative variants (2 tests)
// =====================================================================

#[test]
fn hir_ty_ref_with_lifetime_and_mutability() {
    let inner = Box::new(HirTy {
        hir_id: HirId::new(DefId(0), ItemLocalId(2)),
        kind: HirTyKind::Int(IntTy::I32),
        inferred: None,
        span: Span::DUMMY,
    });
    let lifetime = landin_compiler::ast::Lifetime {
        ident: Ident::new(Symbol::default(), Span::DUMMY),
        span: Span::DUMMY,
    };
    let ty = HirTy {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirTyKind::Ref(Some(lifetime), Mutability::Mutable, inner),
        inferred: None,
        span: Span::DUMMY,
    };
    match ty.kind {
        HirTyKind::Ref(Some(_), m, _) => assert_eq!(m, Mutability::Mutable),
        _ => panic!("expected Ref ty"),
    }
}

#[test]
fn hir_ty_uint_with_suffix() {
    let ty = HirTy {
        hir_id: HirId::new(DefId(0), ItemLocalId(1)),
        kind: HirTyKind::Uint(UintTy::U64),
        inferred: None,
        span: Span::DUMMY,
    };
    match ty.kind {
        HirTyKind::Uint(UintTy::U64) => {}
        _ => panic!("expected Uint U64"),
    }
}

// =====================================================================
// Res + InferTy additional coverage (2 tests)
// =====================================================================

#[test]
fn res_variants_distinct() {
    let local = Res::Local(HirId::new(DefId(0), ItemLocalId(1)));
    let def = Res::Def(DefId(5), DefKind::Fn);
    let prim = Res::PrimTy(PrimTy::I32);
    let self_ty = Res::SelfTy(landin_compiler::hir::HirSelfKind::Impl);
    let unknown = Res::Unknown;
    let err = Res::Err;
    assert_ne!(local, def);
    assert_ne!(def, prim);
    assert_ne!(prim, self_ty);
    assert_ne!(self_ty, unknown);
    assert_ne!(unknown, err);
}

#[test]
fn infer_ty_counter_monotonic() {
    let mut c = InferTyCounter::new();
    let mut prev: Option<InferTy> = None;
    for _ in 0..100 {
        let n = c.fresh();
        if let Some(p) = prev {
            assert!(n.0 > p.0, "InferTy should be monotonically increasing");
        }
        prev = Some(n);
    }
    assert_eq!(c.count(), 100);
}
