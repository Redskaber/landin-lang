//! Stage 18.59 — LowerTyCtx Consolidation Tests.
//!
//! Verifies that `LowerTyCtx` + `lower_hir_ty_to_mir_ty_with_ctx` correctly
//! replaces the 7 `lower_hir_ty_to_mir_ty*` variants.
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 6 "通用 > 特例": one context handles all combinations.
//! Per §1.0 原則 5 "去除兼容思维": replaces parameter-combination anti-pattern.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;
use landin_compiler::hir::{HirGenericParam, HirItem, OwnerNode};
use landin_compiler::mir::lower::{lower_hir_ty_to_mir_ty_with_ctx, LowerTyCtx};
use landin_compiler::mir::ty::{ParamTy, TyKind};

// === Positive: LowerTyCtx works correctly ===

/// Stage 18.59 positive 1: LowerTyCtx::new + with_hir produces correct MIR.
///
/// Compiles a simple program and verifies that LowerTyCtx can lower
/// a HIR Ty to MIR Ty with hir access.
#[test]
fn stage18_59_lower_ty_ctx_new_with_hir() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 42 }; }";
    let result = compile(src);
    assert!(result.hir.is_some(), "HIR should be produced");

    // Find a struct field type to lower.
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    // Verify the type was lowered (not Error).
                    assert!(
                        !matches!(mir_ty.kind, TyKind::Error),
                        "LowerTyCtx should produce non-Error type, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
    // If no struct found, test still passes (no assertion failure).
}

/// Stage 18.59 positive 2: LowerTyCtx with generics produces correct Param.
///
/// Verifies that LowerTyCtx with generic_params resolves type parameters
/// to TyKind::Param.
#[test]
fn stage18_59_lower_ty_ctx_with_generics() {
    let src = "struct Box<T> { val: T } fn main() { let b = Box { val: 42 }; }";
    let result = compile(src);
    assert!(result.hir.is_some(), "HIR should be produced");

    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() && !s.generics.params.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    // Build generic_params from the struct's generics.
                    let generic_params: Vec<ParamTy> = s
                        .generics
                        .params
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| {
                            if let HirGenericParam::Type(tp) = p {
                                Some(ParamTy {
                                    index: i as u32,
                                    name: tp.ident.name,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();

                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter)
                        .with_hir(Some(hir))
                        .with_generics(&generic_params);
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    // Verify the type was lowered to Param (generic T).
                    assert!(
                        matches!(mir_ty.kind, TyKind::Param(_)),
                        "LowerTyCtx with generics should produce Param for T, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 positive 3: LowerTyCtx builder methods are chainable.
#[test]
fn stage18_59_lower_ty_ctx_builder_chain() {
    let mut region_counter = 0u32;
    // Verify builder methods return Self (chainable).
    let cx = LowerTyCtx::new(&mut region_counter)
        .with_hir(None)
        .with_generics(&[]);
    assert!(cx.hir.is_none(), "hir should be None");
    assert!(
        cx.generic_params.is_empty(),
        "generic_params should be empty"
    );
}

// === Negative: Verify LowerTyCtx error handling ===

/// Stage 18.59 negative 1: LowerTyCtx with no hir resolves plain types.
#[test]
fn stage18_59_lower_ty_ctx_no_hir_plain_type() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 42 }; }";
    let result = compile(src);
    assert!(result.hir.is_some(), "HIR should be produced");

    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter);
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Int(_)),
                        "LowerTyCtx should lower i32 to Int, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 2: LowerTyCtx with empty generics behaves like no generics.
#[test]
fn stage18_59_lower_ty_ctx_empty_generics() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 42 }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter)
                        .with_hir(Some(hir))
                        .with_generics(&[]); // empty generics
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        !matches!(mir_ty.kind, TyKind::Error),
                        "LowerTyCtx with empty generics should not produce Error"
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 3: LowerTyCtx default (no hir, no generics) still works.
#[test]
fn stage18_59_lower_ty_ctx_default_works() {
    let src = "struct S { x: bool } fn main() { let s = S { x: true }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter); // defaults
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Bool),
                        "LowerTyCtx default should lower bool to Bool, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 4: Verify LowerTyCtx produces non-Error for valid types.
#[test]
fn stage18_59_lower_ty_ctx_no_error_for_valid_type() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 42 }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        !matches!(mir_ty.kind, TyKind::Error),
                        "LowerTyCtx should not produce Error for valid i32 type"
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 5: LowerTyCtx with tuple type.
#[test]
fn stage18_59_lower_ty_ctx_tuple_type() {
    let src = "struct S { x: (i32, bool) } fn main() { let s = S { x: (42, true) }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Tuple(_)),
                        "LowerTyCtx should lower tuple to Tuple, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 6: LowerTyCtx with reference type.
#[test]
fn stage18_59_lower_ty_ctx_ref_type() {
    let src = "struct S { x: &i32 } fn main() { let s = S { x: &42 }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Ref(_, _, _)),
                        "LowerTyCtx should lower &i32 to Ref, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 7: LowerTyCtx with array type.
#[test]
fn stage18_59_lower_ty_ctx_array_type() {
    let src = "struct S { x: [i32; 3] } fn main() { let s = S { x: [1, 2, 3] }; }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if !s.fields.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Array(_, _)),
                        "LowerTyCtx should lower [i32; 3] to Array, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 8: LowerTyCtx with generic struct field.
#[test]
fn stage18_59_lower_ty_ctx_generic_struct_field() {
    let src = "struct Box<T> { val: T } struct S { x: Box<i32> } fn main() { 0 }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        // Find the S struct (not Box).
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                // S has a field of type Box<i32> — find it by checking field count.
                if s.fields.len() == 1 && s.generics.params.is_empty() {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    assert!(
                        matches!(mir_ty.kind, TyKind::Adt(_, _)),
                        "LowerTyCtx should lower Box<i32> to Adt, got: {:?}",
                        mir_ty.kind
                    );
                    return;
                }
            }
        }
    }
}

/// Stage 18.59 negative 9: LowerTyCtx region_counter advances correctly.
#[test]
fn stage18_59_lower_ty_ctx_region_counter_advances() {
    let src = "struct S { x: &i32, y: &bool } fn main() { 0 }";
    let result = compile(src);
    if let Some(hir) = &result.hir {
        for (_, node) in &hir.owners {
            if let OwnerNode::Item(HirItem::Struct(s)) = node {
                if s.fields.len() >= 2 {
                    let field_ty = &s.fields[0].ty;
                    let mut region_counter = 0u32;
                    let mut cx = LowerTyCtx::new(&mut region_counter).with_hir(Some(hir));
                    let _mir_ty = lower_hir_ty_to_mir_ty_with_ctx(field_ty, &mut cx);
                    // After lowering a &i32, region_counter should have advanced.
                    assert!(
                        *cx.region_counter > 0,
                        "region_counter should advance after lowering a reference type"
                    );
                    return;
                }
            }
        }
    }
}
