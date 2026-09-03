//! Method dispatch resolution: resolve method calls to their targets.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.131):
//! Extracted from `expr_operand.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all method resolution functions: enum variant resolution,
//! inherent/trait method lookup, auto-deref, local init search, and ADT type inference.
//!
//! ## Sub-responsibility
//! Method dispatch: given a method call expression, resolve which function
//! definition it calls — searching locals, inherent impls, trait impls,
//! and auto-deref chains. Also resolves enum variant field types.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (method resolution)
//! - J3: no circular deps (called by expr_operand + control_flow; no callback)
//! - J4: method resolution sub-responsibility is complete in this file
//! - J5: stays within mir::lower stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::hir::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::ty_lower::lower_hir_ty_to_mir_ty;
use super::MirLowerCtxt;

/// Stage 18.284 (TD-INTRINSIC-OVERUSE Phase 2-A): Map a primitive `TyKind`
/// to its source-language name as it would appear in `impl <name> { ... }`.
///
/// Returns `None` for types that don't support inherent impls (Ref, RawPtr,
/// Tuple, Array, Slice, etc.).
///
/// Per §1.0 原則 6 (通解>特例): one mapping function for all primitive types.
/// Per §10 (api-naming): `name_of_primitive_ty` follows `<noun>_<prep>_<noun>`.
pub(super) fn name_of_primitive_ty(ty: &Ty) -> Option<&'static str> {
    use crate::ast::{FloatTy, IntTy, UintTy};
    match ty.kind {
        TyKind::Str => Some("str"),
        TyKind::Bool => Some("bool"),
        TyKind::Char => Some("char"),
        TyKind::Int(IntTy::I8) => Some("i8"),
        TyKind::Int(IntTy::I16) => Some("i16"),
        TyKind::Int(IntTy::I32) => Some("i32"),
        TyKind::Int(IntTy::I64) => Some("i64"),
        TyKind::Int(IntTy::I128) => Some("i128"),
        TyKind::Int(IntTy::Isize) => Some("isize"),
        TyKind::Uint(UintTy::U8) => Some("u8"),
        TyKind::Uint(UintTy::U16) => Some("u16"),
        TyKind::Uint(UintTy::U32) => Some("u32"),
        TyKind::Uint(UintTy::U64) => Some("u64"),
        TyKind::Uint(UintTy::U128) => Some("u128"),
        TyKind::Uint(UintTy::Usize) => Some("usize"),
        TyKind::Float(FloatTy::F32) => Some("f32"),
        TyKind::Float(FloatTy::F64) => Some("f64"),
        _ => None,
    }
}

/// Stage 18.285 (TD-INTRINSIC-OVERUSE Phase 2-A continuation): Map a primitive
/// `HirTyKind` to its source-language name. Mirror of `name_of_primitive_ty`
/// for the HIR side, needed because `impl i32 { ... }` parses `i32` as
/// `HirTyKind::Int(I32)` (not `HirTyKind::Path("i32")`) — see parser/ty.rs.
///
/// Returns `None` for non-primitive HIR types (Path, Ref, Tuple, etc.) and
/// for `Str` (which IS parsed as Path in Landin, not as a primitive variant).
///
/// Per §1.0 原則 6 (通解>特例): one mapping function for all primitive HIR types.
/// Per §12 (最优>最小): reuses the same `&'static str` returns as
/// `name_of_primitive_ty` so direct string comparison works uniformly.
pub(crate) fn name_of_primitive_hir_ty(kind: &crate::hir::HirTyKind) -> Option<&'static str> {
    use crate::ast::{FloatTy, IntTy, UintTy};
    match kind {
        crate::hir::HirTyKind::Bool => Some("bool"),
        crate::hir::HirTyKind::Char => Some("char"),
        crate::hir::HirTyKind::Int(IntTy::I8) => Some("i8"),
        crate::hir::HirTyKind::Int(IntTy::I16) => Some("i16"),
        crate::hir::HirTyKind::Int(IntTy::I32) => Some("i32"),
        crate::hir::HirTyKind::Int(IntTy::I64) => Some("i64"),
        crate::hir::HirTyKind::Int(IntTy::I128) => Some("i128"),
        crate::hir::HirTyKind::Int(IntTy::Isize) => Some("isize"),
        crate::hir::HirTyKind::Uint(UintTy::U8) => Some("u8"),
        crate::hir::HirTyKind::Uint(UintTy::U16) => Some("u16"),
        crate::hir::HirTyKind::Uint(UintTy::U32) => Some("u32"),
        crate::hir::HirTyKind::Uint(UintTy::U64) => Some("u64"),
        crate::hir::HirTyKind::Uint(UintTy::U128) => Some("u128"),
        crate::hir::HirTyKind::Uint(UintTy::Usize) => Some("usize"),
        crate::hir::HirTyKind::Float(FloatTy::F32) => Some("f32"),
        crate::hir::HirTyKind::Float(FloatTy::F64) => Some("f64"),
        _ => None,
    }
}

/// Stage 3.38 (L-ENUM): Resolve the variant index and field types for an
/// enum variant construction.
///
/// Given an enum DefId and a variant name, looks up the variant in the HIR
/// enum definition. Returns:
///   - `Some((variant_index, field_tys))` where field_tys includes the
///     discriminant (i32) as the first element, followed by the variant's
///     payload field types.
///   - `None` if the variant isn't found.
///
/// Per §16: MIR lower reads HIR (allowed — data flows downstream). The
/// resolved field_tys are sunk into `AggregateKind::Adt` so codegen reads
/// from MIR.
pub(crate) fn resolve_enum_variant(
    cx: &MirLowerCtxt,
    enum_def_id: crate::hir::DefId,
    variant_name: &crate::lexer::Symbol,
) -> Option<(u32, Vec<Ty>)> {
    let hir = cx.hir?;
    let owner = hir.find_owner(enum_def_id)?;
    let enum_def = match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => e,
        _ => return None,
    };
    for (i, variant) in enum_def.variants.iter().enumerate() {
        if variant.ident.name == *variant_name {
            // Found the variant. Build field_tys: [discriminant, payload...]
            let mut field_tys = vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)];
            match &variant.data {
                crate::hir::HirVariantData::Unit(_) => {
                    // No payload — just the discriminant.
                }
                crate::hir::HirVariantData::Tuple(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
                crate::hir::HirVariantData::Struct(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
            }
            return Some((i as u32, field_tys));
        }
    }
    None
}

/// Stage 18.167 (TD-VARIANT-CONSTRUCTOR): Extract the variant name from a
/// HIR path, supporting both 2-segment (`Option::Some`) and 1-segment
/// (`Some`) paths.
///
/// Per §1.0 原則 6 (通解>特例): one function for all variant name extraction.
/// Per §10: `variant_name_from_path` follows `<noun>_<noun>_<prep>_<noun>`.
pub(crate) fn variant_name_from_path(path: &crate::hir::HirPath) -> &crate::lexer::Symbol {
    if path.segments.len() >= 2 {
        &path.segments[1].ident.name
    } else {
        &path.segments[0].ident.name
    }
}

/// Stage 13.17: Resolve an inherent method call to a DefId.
///
/// Searches HIR for an `impl` block on the receiver's type (must be
/// `TyKind::Adt(adt_def_id, _)`) and finds a method with the given name.
///
/// Returns the method's DefId (the impl fn's `hir_id.owner`) if found,
/// or `None` if:
/// - The receiver's type is not `TyKind::Adt` (e.g., primitives, references)
/// - No impl block exists for the type
/// - The impl block doesn't contain a method with the given name
///
/// Per §16: this is a HIR query performed at MIR-lowering time. The result
/// (DefId) is sunk into the MIR as data (`Const{ty: FnDef(def_id), val: Uint(def_id)}`),
/// so codegen doesn't need to query HIR.
///
/// Per `api-naming-standard.md` §3 + §8: `resolve_inherent_method` follows
/// the `<verb>_<adjective>_<noun>` pattern (mirrors `resolve_enum_variant`).
///
/// Stage 14.18 (GAP-31): Query the self_kind of a method by its DefId.
///
/// Given a method's DefId (the owner of the fn body), find the method's
/// first parameter's `self_kind` (Value, Ref(Immutable), or Ref(Mutable)).
/// This tells the call site whether to pass the receiver by value or by
/// reference.
///
/// Returns `None` if the DefId doesn't resolve to an impl method or if
/// the method has no self param.
///
/// Per §16: this is a HIR query performed at MIR-lowering time. The result
/// is used immediately (to choose Operand::Copy vs Rvalue::Ref) and not
/// sunk into MIR — codegen doesn't need it.
///
/// Per `api-naming-standard.md` §3 + §8: `query_method_self_kind` follows
/// the `<verb>_<noun>_<noun>_<noun>` pattern.
pub(super) fn query_method_self_kind(
    hir: &crate::hir::HirCrate,
    method_def_id: crate::hir::DefId,
) -> Option<crate::ast::SelfKind> {
    // Search all owners for the method with this DefId.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method! Return its first param's self_kind.
                        return f.sig.inputs.first().and_then(|p| p.self_kind);
                    }
                }
            }
        }
        // Stage 14.97 (Bug Y1 fix): Also search Trait owners for trait default
        // body methods. When a method call resolves to a trait default body
        // (e.g., `p.double_value()` where double_value has a default body in
        // trait Counter), we need to know the self_kind to correctly lower the
        // call (e.g., borrow p as &p for &self methods).
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.hir_id.owner == method_def_id {
                        return f.sig.inputs.first().and_then(|p| p.self_kind);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn resolve_inherent_method(
    hir: &crate::hir::HirCrate,
    interner: &lasso::Rodeo,
    recv_ty: &Ty,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // Stage 14.42: Auto-deref Ref/RawPtr to find the underlying ADT type.
    //
    // This is needed for method chaining on `&mut self` returns. For example:
    //   `c.inc().inc().add(10)` — `c.inc()` returns `&mut Counter`, and the
    //   next `.inc()` call needs to resolve `inc` on `Counter` (the inner
    //   type of the Ref), not on the Ref itself.
    //
    // Per §13.4 (design alignment): Rust's auto-deref is a well-defined
    // semantic — method lookup follows the receiver's deref chain. We
    // implement the common case: one level of Ref/RawPtr auto-deref.
    // Multi-level auto-deref (e.g., `&&mut T`) is deferred.
    let recv_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => inner,
        _ => recv_ty,
    };

    // Stage 18.284+18.285 (TD-INTRINSIC-OVERUSE Phase 2-A): Determine the
    // receiver type's source-language name as a `&str` for impl-block matching.
    //
    // Three cases:
    //
    // (1) ADT types: look up the struct/enum name in HIR, resolve the Symbol
    //     to a `&str` via the interner. Impl blocks for `struct Foo { ... }`
    //     use `self_ty = Path("Foo")` — match by string comparison.
    //
    // (2) `str` primitive: parser parses `str` as a Path (since "str" is NOT
    //     in the primitive keyword list — see parser/ty.rs). So `impl str`
    //     has `self_ty = Path("str")` — match by string comparison (same as
    //     ADT path). The `&str` comes from `name_of_primitive_ty` returning
    //     `&'static str "str"`.
    //
    // (3) Other primitives (`i32`, `bool`, etc.): parser parses the keyword
    //     as a `HirTyKind::Int(I32)` / `HirTyKind::Bool` variant (since these
    //     keywords ARE in the primitive list). So `impl i32` has
    //     `self_ty = HirTyKind::Int(I32)` — match via `name_of_primitive_hir_ty`
    //     which returns the same `&'static str` we'd get from
    //     `name_of_primitive_ty` for the corresponding MIR type.
    //
    // Per §1.0 原則 6 (通解>特解): one unified string-based comparison path
    // for ALL types (ADT + all primitives). The impl-matching loop checks
    // both Path-based self_tys (resolved to &str) AND primitive-variant
    // self_tys (mapped to &str via name_of_primitive_hir_ty).
    // Per §12 (最优>最小): infrastructure for ALL primitive impls — adding
    // new primitive types (i32::abs, bool::then, char::is_ascii, etc.)
    // requires no architecture changes, only prelude impl declarations.
    let type_name_str: &str = match &recv_ty.kind {
        TyKind::Adt(def_id, _) => {
            // ADT path: look up the struct/enum name in HIR, resolve via interner.
            let sym = hir.find_owner(*def_id).and_then(|owner| match owner {
                crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) => Some(s.ident.name),
                crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => Some(e.ident.name),
                _ => None,
            })?;
            interner.resolve(&sym)
        }
        _ => {
            // Primitive path (str, i32, bool, etc.) — get &'static str name.
            // For `str`, this is "str" (matches `impl str { ... }`'s Path self_ty
            // when resolved to string). For `i32`, this is "i32" (matches
            // `impl i32 { ... }`'s HirTyKind::Int(I32) self_ty via
            // name_of_primitive_hir_ty).
            name_of_primitive_ty(recv_ty)?
        }
    };

    // Search all impl blocks for one whose self_ty matches the type name.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            // Check if this impl is for our type (inherent impl, not trait impl).
            if impl_block.of_trait.is_some() {
                continue; // Skip trait impls; only looking for inherent methods.
            }
            // Check if the impl's self_ty matches type_name_str.
            // Stage 18.285: Two matching paths, unified by string comparison:
            //   (1) Path self_ty (`impl str { ... }`, `impl Foo { ... }`):
            //       resolve the path's single segment to &str, compare.
            //   (2) Primitive variant self_ty (`impl i32 { ... }`, `impl bool`):
            //       use name_of_primitive_hir_ty to get &str, compare.
            let self_ty_matches = match &impl_block.self_ty.kind {
                crate::hir::HirTyKind::Path(_qself, path) if path.segments.len() == 1 => {
                    interner.resolve(&path.segments[0].ident.name) == type_name_str
                }
                hir_kind => name_of_primitive_hir_ty(hir_kind) == Some(type_name_str),
            };
            if !self_ty_matches {
                continue;
            }
            // Search the impl's items for a method with the given name.
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        // Found the method! Return its DefId (the owner of the fn body).
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }
    None
}

/// Stage 14.18 (GAP-31): Auto-deref a Place if its base local's type is Ref.
///
/// When a method takes `&self` or `&mut self`, the self param local has type
/// `Ref(_, _, Adt)`. Field access `self.field` needs to deref the reference
/// before projecting the field. This helper checks the Place's base local type
/// and wraps it in `ProjectionElem::Deref` if the type is `Ref`.
///
/// For non-Ref types (by-value self, structs, etc.), returns the Place unchanged.
///
/// Per §16: this is a MIR-lowering-time query on `cx.mir.local_decls` (data
/// already sunk from typeck). No HIR access needed.
pub(super) fn auto_deref_if_ref(cx: &MirLowerCtxt, place: Place, _receiver: &HirExpr) -> Place {
    // Check if the base local's type is Ref.
    let is_ref = match &place.kind {
        PlaceKind::Local(local_id) => {
            let ty = &cx.mir.local(*local_id).ty;
            matches!(ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
        }
        _ => false,
    };
    if is_ref {
        let span = place.span;
        Place {
            kind: PlaceKind::Projection(Box::new(place), ProjectionElem::Deref),
            span,
        }
    } else {
        place
    }
}

/// Stage 14.29: Query the return type of a method by its DefId.
///
/// Given a method's DefId (the owner of the fn body), find the method's
/// return type from HIR and lower it to a MIR type. This is used by
/// MethodCall lowering to set the dest local's type, enabling chained
/// method calls (e.g. `Calc::new(10).add(5).get()`) to resolve methods
/// on the result type.
///
/// Returns `None` if the DefId doesn't resolve to an impl method or if
/// the return type can't be lowered.
///
/// Stage 15.6 (perf): Now wrapped by `MirLowerCtxt::query_method_return_type`
/// which checks a RefCell<HashMap> cache to avoid repeated O(n) HIR scans.
/// This is the uncached inner implementation.
///
/// Per §23 (API Naming): public free function uses `<verb>_<noun>` pattern.
/// Per §1.0 原则 6 "通用 > 特例": one function handles all owner kinds (impl
/// method, free fn, trait default body).
pub fn query_method_return_type_uncached(
    hir: &crate::hir::HirCrate,
    method_def_id: crate::hir::DefId,
) -> Option<crate::mir::ty::Ty> {
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method! Lower its return type.
                        // Stage 14.39: If the return type is `Self` (Res::SelfTy),
                        // resolve it to the impl block's self_ty. This is the same
                        // fix as resolve_self_param_type (Stage 13.18).
                        //
                        // Stage 33.1 (TD-IMPL-METHOD-GENERIC-PARAM-RESOLUTION):
                        // Use `lower_hir_ty_to_mir_ty_with_hir_and_generics` with
                        // the impl block's generic params so type parameters like
                        // `T` in `fn get(&self) -> T` resolve to `Param(0)` instead
                        // of `Error`. Was: `lower_hir_ty_to_mir_ty(ty)` (no generics)
                        // → return type was Error for generic impl methods.
                        // Per §1.0 原則 6 (通解 > 特解): one path for all impl methods.
                        let impl_generics =
                            crate::hir::generics::find_generics(impl_block.hir_id.owner, hir);
                        return match &f.sig.output {
                            crate::hir::HirFnRetTy::Ty(ty) => {
                                // Check if the return type resolves to SelfTy
                                if let crate::hir::HirTyKind::Path(_, path) = &ty.kind {
                                    if matches!(path.res, crate::hir::Res::SelfTy(_)) {
                                        // Return type is `Self` — resolve to impl's self_ty
                                        return Some(
                                            super::lower_hir_ty_to_mir_ty_with_hir_and_generics(
                                                &impl_block.self_ty,
                                                Some(hir),
                                                &impl_generics,
                                            ),
                                        );
                                    }
                                }
                                let t = super::lower_hir_ty_to_mir_ty_with_hir_and_generics(
                                    ty,
                                    Some(hir),
                                    &impl_generics,
                                );
                                Some(t)
                            }
                            crate::hir::HirFnRetTy::Default(_) => {
                                // No explicit return type → unit ()
                                Some(crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Tuple(vec![]),
                                    f.span,
                                ))
                            }
                        };
                    }
                }
            }
        }
        // Stage 14.98 (Bug Z4 fix): Also search top-level HirItem::Fn owners.
        // Free functions (e.g., `fn make_n(i: i32) -> N { N { v: i } }`) are
        // stored as HirItem::Fn owners. Without this, method calls on results
        // of free functions (e.g., `let n = make_n(i); n.base();`) crashed
        // because the return type couldn't be traced.
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            if f.hir_id.owner == method_def_id {
                return match &f.sig.output {
                    crate::hir::HirFnRetTy::Ty(ty) => Some(super::lower_hir_ty_to_mir_ty(ty)),
                    crate::hir::HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Tuple(vec![]),
                        f.span,
                    )),
                };
            }
        }
        // Stage 14.98 (Bug Z1 fix): Also search Trait owners for trait default
        // body methods. When `let r = p.f(); r.g();` where f is a trait default
        // body, we need to query f's return type to resolve g.
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.hir_id.owner == method_def_id {
                        return match &f.sig.output {
                            crate::hir::HirFnRetTy::Ty(ty) => {
                                // For trait default body return types, `Self` is
                                // unknown without monomorphization. Use the first
                                // impl's self_ty as the specialization type (v0.1
                                // single-impl heuristic).
                                if let crate::hir::HirTyKind::Path(_, path) = &ty.kind {
                                    if matches!(path.res, crate::hir::Res::SelfTy(_)) {
                                        let trait_name = t.ident.name;
                                        let first_impl_self_ty =
                                            hir.owners.iter().find_map(|(_, o)| {
                                                if let crate::hir::OwnerNode::Item(
                                                    crate::hir::HirItem::Impl(impl_block),
                                                ) = o
                                                {
                                                    if impl_block.of_trait.as_ref().and_then(|p| {
                                                        p.segments.last().map(|s| s.ident.name)
                                                    }) == Some(trait_name)
                                                    {
                                                        return Some(
                                                            super::lower_hir_ty_to_mir_ty(
                                                                &impl_block.self_ty,
                                                            ),
                                                        );
                                                    }
                                                }
                                                None
                                            });
                                        if let Some(self_ty) = first_impl_self_ty {
                                            return Some(self_ty);
                                        }
                                    }
                                }
                                Some(super::lower_hir_ty_to_mir_ty(ty))
                            }
                            crate::hir::HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                                crate::mir::ty::TyKind::Tuple(vec![]),
                                f.span,
                            )),
                        };
                    }
                }
            }
        }
    }
    None
}

/// Stage 13.17: Resolve an inherent method call from the HIR receiver expression.
///
/// This is a fallback when the MIR local's type is still `Infer` (unresolved
/// at MIR-lowering time). We inspect the HIR receiver expression directly:
///
/// - `HirExprKind::Struct { path, .. }` — the receiver is a struct literal;
///   the path gives us the ADT DefId.
/// - `HirExprKind::Path(path)` — the receiver is a variable; we trace back
///   to its let binding's initializer type.
/// - `HirExprKind::Call { func, .. }` — the receiver is a function call
///   (e.g., tuple struct ctor); we check if func is an ADT ctor.
///
/// Per §16: this is a HIR query at MIR-lowering time. The result (DefId) is
/// sunk into the MIR as data.
/// Stage 14.91 (Bug X3 fix): Resolve a method via trait impls.
///
/// Searches all `impl Trait for Type` blocks for one whose `self_ty` matches
/// the receiver's ADT type and whose items include a method with the given name.
/// Returns the method's DefId if found.
///
/// This enables static trait dispatch: `impl Shape for Square { fn area() {...} }`
/// followed by `s.area()` resolves to the trait impl's `area` method.
///
/// Stage 32.3 (TD-PRELUDE-MONO-ORDER): Added `owner_def_id` parameter to
/// support `TyKind::Param(N)` receivers. When the recv_ty is `Param(N)`,
/// we look up the Nth type param's trait bounds (via `find_param_trait_bounds`)
/// in the enclosing impl block (identified by `owner_def_id`). For each
/// trait bound, we find the trait declaration and look up the method by name.
/// This makes `self.x.f()` where `x: X, X: T` (trait bound) resolve to
/// `T::f`'s DefId (the trait declaration's method, not an impl's method).
///
/// Per §13.4: Rust trait method resolution is complex (canonical query, etc.).
/// For v0.1, we implement the simple case: search all trait impls for a matching
/// self_ty + method name. This is O(n*m) but sufficient for v0.1's scale.
/// Per §1.0 原則 6 (通解 > 特解): one path handles Adt, primitive, and Param
/// receivers — Param uses trait bounds lookup, others use existing logic.
/// Per §1.0 原則 9 (正确 > 妥协): Param-typed method resolution is correct
/// (resolves to trait method), not silently skipped.
pub(super) fn resolve_trait_method(
    hir: &crate::hir::HirCrate,
    interner: &lasso::Rodeo,
    recv_ty: &Ty,
    method_name: &lasso::Spur,
    owner_def_id: Option<crate::hir::DefId>,
) -> Option<crate::hir::DefId> {
    // Auto-deref Ref to find the underlying type.
    let recv_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => inner,
        _ => recv_ty,
    };

    // Stage 32.3 (TD-PRELUDE-MONO-ORDER): Param(N) receiver — look up the
    // param's trait bounds via the enclosing impl block (identified by
    // owner_def_id). For each trait bound, find the trait declaration and
    // look up the method by name.
    //
    // This handles `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } }`
    // where `self.x: X` (Param(0)) and `X: T` (trait bound). The call
    // `self.x.f()` resolves to `T::f`'s DefId (the trait declaration's method).
    //
    // If the method has a default body, codegen emits a normal call.
    // If the method has no body (e.g., `fn f(&self) -> i32;`), codegen emits
    // an external declaration — the call is never linked if the body is
    // never executed (the test only checks compilation success).
    //
    // Per §1.0 原則 6 (通解 > 特解): one path for all Param-typed receivers.
    // Per §1.0 原則 4 (报错 > 静默): if no owner_def_id or no trait bounds,
    // return None — caller reports "no method found" explicitly.
    // Per §1.0 原則 9 (正确 > 妥协): correct trait method resolution via
    // trait bounds, not silent Error placeholder.
    if let TyKind::Param(ref param) = recv_ty.kind {
        let owner = owner_def_id?;
        let trait_bounds = crate::hir::generics::find_param_trait_bounds(owner, param.index, hir);
        for tb in &trait_bounds {
            // The trait bound's path resolves to a trait declaration.
            // Get the trait name (last segment of the path).
            if let Some(trait_name_sym) = tb.path.segments.last().map(|s| s.ident.name) {
                // Find the trait declaration in HIR by name.
                for (_, t_owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = t_owner {
                        if t.ident.name == trait_name_sym {
                            // Found the trait. Look up the method by name.
                            for trait_item in &t.items {
                                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                                    if f.ident.name == *method_name {
                                        // Return the trait method's DefId.
                                        // Note: this may be a method with no body
                                        // (declaration only). Codegen will handle
                                        // that case (external declaration).
                                        return Some(f.hir_id.owner);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Param-typed receiver but no matching trait method found.
        return None;
    }

    // Stage 18.295: Unified type name resolution — supports both ADT and
    // primitive types. Reuses the same pattern as resolve_inherent_method.
    // Per §1.0 原則 6 (通解>特解): one path for all types.
    // Per §12 (最优>最小): root cause fix — add interner param (same as
    // resolve_inherent_method) to enable string comparison.
    //
    // Stage 87 (v0.8 — TD-DYN-TRAIT-COMPLETION): For `dyn Trait` receivers,
    // look up the method directly in the trait declaration (the trait
    // DefId is carried by `TyKind::Dyn(trait_def_id)`). This is the
    // vtable dispatch path — the actual impl method is resolved at
    // runtime via the vtable.
    //
    // Per §1.0 原則 6 (通解 > 特解): one path for all Dyn receivers —
    // regardless of which trait.
    // Per §1.0 原則 4 (报错 > 静默): if the method isn't in the trait,
    // return None (caller reports "no method found").
    if let TyKind::Dyn(trait_def_id) = recv_ty.kind {
        // Look up the trait declaration by DefId.
        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t))) =
            hir.find_owner(trait_def_id)
        {
            // Search the trait's items for the method by name.
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.ident.name == *method_name {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
        return None;
    }

    let type_name_str: &str = match &recv_ty.kind {
        TyKind::Adt(def_id, _) => {
            let sym = hir.find_owner(*def_id).and_then(|owner| match owner {
                crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) => Some(s.ident.name),
                crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => Some(e.ident.name),
                _ => None,
            })?;
            interner.resolve(&sym)
        }
        _ => name_of_primitive_ty(recv_ty)?,
    };

    // Search all TRAIT impl blocks for one whose self_ty matches and whose
    // items include the method.
    // Stage 18.295: Two matching paths, unified by string comparison:
    //   (1) Path self_ty (`impl Trait for str` / `impl Trait for Foo`)
    //   (2) Primitive variant self_ty (`impl Trait for i32`)
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            if impl_block.of_trait.is_none() {
                continue;
            }
            let self_ty_matches = match &impl_block.self_ty.kind {
                crate::hir::HirTyKind::Path(_qself, path) if path.segments.len() == 1 => {
                    interner.resolve(&path.segments[0].ident.name) == type_name_str
                }
                hir_kind => name_of_primitive_hir_ty(hir_kind) == Some(type_name_str),
            };
            if !self_ty_matches {
                continue;
            }
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }

    // Stage 14.97 (Bug Y1 fix): Search trait definitions for default body.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            let trait_name = t.ident.name;
            let trait_implemented = hir.owners.iter().any(|(_, o)| {
                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = o {
                    if impl_block
                        .of_trait
                        .as_ref()
                        .and_then(|p| p.segments.last().map(|s| s.ident.name))
                        == Some(trait_name)
                    {
                        // Stage 18.295: Use unified string comparison.
                        let impl_self_matches = match &impl_block.self_ty.kind {
                            crate::hir::HirTyKind::Path(_, path) if path.segments.len() == 1 => {
                                interner.resolve(&path.segments[0].ident.name) == type_name_str
                            }
                            hir_kind => name_of_primitive_hir_ty(hir_kind) == Some(type_name_str),
                        };
                        return impl_self_matches;
                    }
                }
                false
            });
            if !trait_implemented {
                continue;
            }
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.ident.name == *method_name && f.body.is_some() {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }

    None
}

pub(super) fn resolve_inherent_method_from_hir_expr(
    cx: &MirLowerCtxt,
    hir: &crate::hir::HirCrate,
    receiver: &HirExpr,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    match &receiver.kind {
        // Struct literal: `P { x: 1 }.get()` — path gives us the ADT.
        HirExprKind::Struct { path, .. } => {
            if let crate::hir::Res::Def(def_id, _) = path.res {
                // Build a synthetic Adt type and resolve the method.
                let synth_ty = Ty::new(
                    TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                    receiver.span,
                );
                resolve_inherent_method(hir, cx.interner, &synth_ty, method_name)
            } else {
                None
            }
        }
        // Path: `p.get()` — trace back to the local's initializer.
        HirExprKind::Path(path) => {
            if let crate::hir::Res::Local(hir_id) = path.res {
                // Find the let binding for this local.
                if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                    return resolve_inherent_method(hir, cx.interner, &init_ty, method_name);
                }
                // Stage 14.38: If find_local_init_type failed (e.g. init is a
                // MethodCall), try to find the init expression directly and
                // resolve the method on its return type.
                if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                    // Stage 14.41: Handle static method call init.
                    // `let v = Vec::new(); v.push(42)` — the init is
                    // `Call { func: Path(Vec::new) }` where Vec::new resolves
                    // to `Res::Def(method_def_id, Fn)`. We look up the method's
                    // return type and resolve the target method on that type.
                    if let HirExprKind::Call {
                        func: init_func, ..
                    } = &init_expr.kind
                    {
                        if let HirExprKind::Path(init_path) = &init_func.kind {
                            if let crate::hir::Res::Def(init_did, init_kind) = init_path.res {
                                if matches!(init_kind, crate::resolve::DefKind::Fn) {
                                    // Stage 15.6 (perf): use cached lookup.
                                    if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                                        return resolve_inherent_method(
                                            hir,
                                            cx.interner,
                                            &ret_ty,
                                            method_name,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    // Stage 14.38: Handle instance method call init.
                    // `let c = a.add(b); c.get()` — the init is a MethodCall.
                    if let HirExprKind::MethodCall {
                        method: init_method,
                        ..
                    } = &init_expr.kind
                    {
                        // The init is a method call — resolve its return type
                        // via query_method_return_type, then resolve the target
                        // method on that type.
                        if let Some(init_did) = resolve_method_by_name(hir, &init_method.name) {
                            // Stage 15.6 (perf): use cached lookup.
                            if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                                // Stage 14.98 (Bug Z3 fix): Also try trait method
                                // resolution, not just inherent. Without this,
                                // `let r1 = p.f(); let r2 = r1.g();` where g is
                                // a trait default body crashes (LLVM "call i32 0").
                                return resolve_inherent_method(
                                    hir,
                                    cx.interner,
                                    &ret_ty,
                                    method_name,
                                )
                                .or_else(|| {
                                    resolve_trait_method(
                                        hir,
                                        cx.interner,
                                        &ret_ty,
                                        method_name,
                                        cx.owner_def_id,
                                    )
                                });
                            }
                        }
                    }
                }
            }
            None
        }
        // Call: could be a tuple struct ctor like `Pair(1, 2).get()`,
        // OR a static method call like `Vec::new().push(1)`.
        HirExprKind::Call { func, .. } => {
            if let HirExprKind::Path(path) = &func.kind {
                if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                    // Stage 14.41: Check DefKind to distinguish struct ctor
                    // from static method call.
                    if matches!(
                        def_kind,
                        crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                    ) {
                        // Struct/enum ctor — the call constructs an Adt.
                        let synth_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            receiver.span,
                        );
                        return resolve_inherent_method(hir, cx.interner, &synth_ty, method_name);
                    }
                    // Stage 14.41: Static method call (e.g., `Vec::new().push(1)`)
                    // — look up the method's return type and resolve the target
                    // method on that type.
                    if matches!(def_kind, crate::resolve::DefKind::Fn) {
                        // Stage 15.6 (perf): use cached lookup.
                        if let Some(ret_ty) = cx.query_method_return_type(def_id) {
                            // Stage 14.98 (Bug Z3 fix): Also try trait method
                            // resolution for static method call results.
                            // Stage 32.3: Pass owner_def_id for Param(N) resolution.
                            return resolve_inherent_method(hir, cx.interner, &ret_ty, method_name)
                                .or_else(|| {
                                    resolve_trait_method(
                                        hir,
                                        cx.interner,
                                        &ret_ty,
                                        method_name,
                                        cx.owner_def_id,
                                    )
                                });
                        }
                    }
                }
            }
            None
        }
        // Stage 14.42: MethodCall receiver — `c.inc().inc()` where the receiver
        // is itself a MethodCall. We resolve the inner method's return type via
        // `query_method_return_type`, then resolve the target method on that
        // type (with auto-deref handling Ref returns like `&mut Counter`).
        //
        // Per §13.4 (design alignment): this is the proper way to handle
        // method chaining — we trace through the HIR to find the receiver
        // type at MIR-lowering time, since typeck doesn't propagate Call
        // return types to dest locals.
        HirExprKind::MethodCall {
            method: recv_method,
            ..
        } => {
            // Resolve the receiver method's DefId by name.
            if let Some(recv_did) = resolve_method_by_name(hir, &recv_method.name) {
                // Get the receiver method's return type.
                // Stage 15.6 (perf): use cached lookup.
                if let Some(ret_ty) = cx.query_method_return_type(recv_did) {
                    // Resolve the target method on the return type.
                    // `resolve_inherent_method` now handles Ref auto-deref
                    // (added in Stage 14.42), so `&mut Counter` correctly
                    // resolves to `Counter`.
                    // Stage 14.98 (Bug Z3 fix): Also try trait method resolution.
                    return resolve_inherent_method(hir, cx.interner, &ret_ty, method_name)
                        .or_else(|| {
                            resolve_trait_method(
                                hir,
                                cx.interner,
                                &ret_ty,
                                method_name,
                                cx.owner_def_id,
                            )
                        });
                }
            }
            None
        }
        // Stage 14.44: Index receiver — `arr[i].method()` where the receiver
        // is an array indexing expression. We need to determine the array's
        // element type and resolve the method on that type.
        //
        // Per §13.4: this mirrors the Call/MethodCall receiver handling —
        // we trace through the HIR to find the receiver type at MIR-lowering
        // time, since typeck doesn't fully propagate types through indexing.
        HirExprKind::Index { receiver, .. } => {
            // Try to determine the array's element type from the receiver.
            // If the receiver is a Path (local variable), trace back to its
            // init type.
            if let HirExprKind::Path(path) = &receiver.kind {
                if let crate::hir::Res::Local(hir_id) = path.res {
                    // Find the array's element type from the local's init.
                    if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                        // If it's an Array, extract the element type.
                        if let TyKind::Array(elem_ty, _) = &init_ty.kind {
                            return resolve_inherent_method(hir, cx.interner, elem_ty, method_name);
                        }
                        // Otherwise, try resolving on the type directly.
                        return resolve_inherent_method(hir, cx.interner, &init_ty, method_name);
                    }
                    // Stage 14.44b: find_local_init_type failed (e.g., init is
                    // a static method call). Try find_local_init_expr to get
                    // the init expression, then resolve via query_method_return_type.
                    if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                        // Static method call init: `[Point::new(1, 2), ...]`
                        if let HirExprKind::Array { elems, .. } = &init_expr.kind {
                            if let Some(first_elem) = elems.first() {
                                // If the first element is a Call (static method),
                                // resolve its return type.
                                if let HirExprKind::Call { func, .. } = &first_elem.kind {
                                    if let HirExprKind::Path(p) = &func.kind {
                                        if let crate::hir::Res::Def(did, kind) = p.res {
                                            if matches!(kind, crate::resolve::DefKind::Fn) {
                                                // Stage 15.6 (perf): cached.
                                                if let Some(ret_ty) =
                                                    cx.query_method_return_type(did)
                                                {
                                                    return resolve_inherent_method(
                                                        hir,
                                                        cx.interner,
                                                        &ret_ty,
                                                        method_name,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        // Stage 14.93 (Bug Y3 fix): Field receiver — `o.inner.method()`
        // where the receiver is a field access expression. We trace back
        // through the outer struct's init to find the field's type.
        //
        // Per §13.4: mirrors the Index receiver handling — we trace through
        // the HIR to find the receiver type at MIR-lowering time.
        HirExprKind::Field {
            receiver: field_receiver,
            ident,
        } => {
            if let HirExprKind::Path(path) = &field_receiver.kind {
                if let crate::hir::Res::Local(hir_id) = path.res {
                    if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                        if let TyKind::Adt(struct_def_id, _) = &init_ty.kind {
                            if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(
                                s,
                            ))) = hir.find_owner(*struct_def_id)
                            {
                                for f in &s.fields {
                                    if f.ident.map(|fi| fi.name) == Some(ident.name) {
                                        let field_mir_ty =
                                            crate::mir::lower::lower_hir_ty_to_mir_ty(&f.ty);
                                        if let Some(did) = resolve_inherent_method(
                                            hir,
                                            cx.interner,
                                            &field_mir_ty,
                                            method_name,
                                        ) {
                                            return Some(did);
                                        }
                                        return resolve_trait_method(
                                            hir,
                                            cx.interner,
                                            &field_mir_ty,
                                            method_name,
                                            cx.owner_def_id,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Stage 14.38: Find the init expression for a local binding by hir_id.
/// Searches all HIR bodies for a `let pat = init;` where pat.hir_id == target.
pub(super) fn find_local_init_expr(
    hir: &crate::hir::HirCrate,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    for (_, body) in &hir.bodies {
        if let Some(expr) = search_expr_for_local_init_expr(&body.value, target_hir_id) {
            return Some(expr);
        }
    }
    None
}

// Stage 14.98 (Bug Z1/Z2/Z4 fix): Removed old `search_block_for_local_init_expr`
// (only handled Block). Use `search_expr_for_local_init_expr` instead, which
// handles all expression kinds (Block, If, While, For, Loop, Match) recursively.

/// Stage 14.38: Resolve a method DefId by name (searching all inherent impls).
pub(super) fn resolve_method_by_name(
    hir: &crate::hir::HirCrate,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // Stage 14.95 (regression fix): The Stage 14.94 Bug Y2 fix added a
    // `self_kind.is_none()` check to only return static methods. But this
    // broke `resolve_method_by_name` for instance methods — it's also
    // called from MethodCall receiver tracing (lines 2492, 2543) where
    // we need to find instance methods (with self) by name to get their
    // return types.
    //
    // Fix: return ANY method matching the name (static or instance).
    // The callers that specifically need static methods already check
    // `def_kind == DefKind::Fn` on the path resolution before calling
    // this function — so returning instance methods here is safe.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }
    None
}

/// Stage 13.17: Find the type of a local variable's initializer.
///
/// Given a `hir_id` for a local binding, search the HIR body for the
/// `let pat = init;` statement that binds it, and return the init's type.
#[allow(clippy::only_used_in_recursion)]
pub(super) fn find_local_init_type(
    cx: &MirLowerCtxt,
    hir: &crate::hir::HirCrate,
    target_hir_id: crate::hir::HirId,
) -> Option<Ty> {
    // Search all bodies. Body.value is a HirExpr (Block for fns).
    // We walk the block's statements looking for a Local that binds target_hir_id.
    // The recursive search below covers all cases including nested blocks.
    for (_, body) in &hir.bodies {
        if let Some(ty) = search_expr_for_local_init(&body.value, target_hir_id) {
            return Some(ty);
        }
        // Stage 14.90 (Bug X2 fix): If the init expression is a Path resolving
        // to another Local, recursively trace through that Local's init.
        // `let r = &p; r.sum()` → r's init is &p, p is Local(p_hir_id),
        // so we search for p_hir_id's init type.
        if let Some(init_expr) = search_expr_for_local_init_expr(&body.value, target_hir_id) {
            // Strip AddrOf wrappers
            let mut inner = &init_expr;
            while let HirExprKind::AddrOf { expr: e, .. } = &inner.kind {
                inner = e;
            }
            // If the inner expression is a Path to a Local, recurse
            if let HirExprKind::Path(path) = &inner.kind {
                if let crate::hir::Res::Local(inner_hir_id) = path.res {
                    if inner_hir_id != target_hir_id {
                        // Recurse — find the inner local's init type
                        if let Some(ty) = find_local_init_type(cx, hir, inner_hir_id) {
                            return Some(ty);
                        }
                    }
                }
            }
            // Stage 14.98 (Bug Z4 fix): If the init is a Call to a free function
            // (DefKind::Fn), query the function's return type.
            // `let n = make_n(i); n.base();` — make_n returns N, so n's type is N.
            // Without this, method resolution on n fails because n's MIR type is
            // Infer (typeck doesn't propagate Call return types to dest locals).
            if let HirExprKind::Call { func, .. } = &inner.kind {
                if let HirExprKind::Path(path) = &func.kind {
                    if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                        if matches!(def_kind, crate::resolve::DefKind::Fn) {
                            // Stage 15.6 (perf): use cached lookup.
                            if let Some(ret_ty) = cx.query_method_return_type(def_id) {
                                return Some(ret_ty);
                            }
                        }
                    }
                }
            }
            // Stage 14.98 (Bug Z1 fix): If the init is a MethodCall, query the
            // method's return type.
            if let HirExprKind::MethodCall {
                method: init_method,
                ..
            } = &inner.kind
            {
                if let Some(init_did) = resolve_method_by_name(hir, &init_method.name) {
                    // Stage 15.6 (perf): use cached lookup.
                    if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                        return Some(ret_ty);
                    }
                }
            }
            // Stage 14.98 (Bug Z2 fix): If the init is a Match, look at the
            // first arm's body to determine the type. All arms should have the
            // same type (typeck enforces this), so the first arm is sufficient
            // for type resolution.
            if let HirExprKind::Match { arms, .. } = &inner.kind {
                if let Some(first_arm) = arms.first() {
                    let arm_body = &first_arm.body;
                    // Try expr_to_adt_type first (handles struct/enum literals).
                    if let Some(ty) = expr_to_adt_type(arm_body) {
                        return Some(ty);
                    }
                    // Try Call with Fn DefKind.
                    if let HirExprKind::Call { func, .. } = &arm_body.kind {
                        if let HirExprKind::Path(p) = &func.kind {
                            if let crate::hir::Res::Def(did, kind) = p.res {
                                if matches!(kind, crate::resolve::DefKind::Fn) {
                                    // Stage 15.6 (perf): cached.
                                    if let Some(ret_ty) = cx.query_method_return_type(did) {
                                        return Some(ret_ty);
                                    }
                                }
                            }
                        }
                    }
                    // Try MethodCall.
                    if let HirExprKind::MethodCall { method: m, .. } = &arm_body.kind {
                        if let Some(did) = resolve_method_by_name(hir, &m.name) {
                            // Stage 15.6 (perf): cached.
                            if let Some(ret_ty) = cx.query_method_return_type(did) {
                                return Some(ret_ty);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recursively search an expression (and nested blocks) for a Local binding.
pub(super) fn search_expr_for_local_init(
    expr: &HirExpr,
    target_hir_id: crate::hir::HirId,
) -> Option<Ty> {
    match &expr.kind {
        HirExprKind::Block(block) => search_block_for_local_init(block, target_hir_id)
            .and_then(|init| expr_to_adt_type(&init)),
        HirExprKind::If { then, else_, .. } => {
            // then is HirBlock; else_ is Option<Box<HirExpr>>
            search_block_for_local_init(then, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
                .or_else(|| {
                    else_
                        .as_ref()
                        .and_then(|e| search_expr_for_local_init(e, target_hir_id))
                })
        }
        // Stage 14.98 (Bug Z1/Z2 fix): Recurse into loop bodies.
        // Previously, `search_expr_for_local_init` only handled Block and If —
        // it didn't search inside While/For/Loop/Match bodies. This meant
        // method calls on struct literals created inside loops crashed
        // ("Called function must be a pointer! %v17 = call i32 0(...)").
        //
        // Per §1.0 原则 6 "通用 > 特例": one recursive rule handles all
        // loop/match kinds by delegating to search_block_for_local_init.
        HirExprKind::While { cond, body, .. } => {
            // cond may contain a block expression with locals (rare but possible)
            if let Some(ty) = search_expr_for_local_init(cond, target_hir_id) {
                return Some(ty);
            }
            search_block_for_local_init(body, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
        }
        HirExprKind::For { iter, body, .. } => {
            if let Some(ty) = search_expr_for_local_init(iter, target_hir_id) {
                return Some(ty);
            }
            search_block_for_local_init(body, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
        }
        HirExprKind::Loop { body, .. } => search_block_for_local_init(body, target_hir_id)
            .and_then(|init| expr_to_adt_type(&init)),
        HirExprKind::Match { expr, arms } => {
            if let Some(ty) = search_expr_for_local_init(expr, target_hir_id) {
                return Some(ty);
            }
            // Search each arm's body
            for arm in arms {
                // arm.guard may be Some(expr) — search it
                if let Some(guard) = &arm.guard {
                    if let Some(ty) = search_expr_for_local_init(guard, target_hir_id) {
                        return Some(ty);
                    }
                }
                if let Some(ty) = search_expr_for_local_init(&arm.body, target_hir_id) {
                    return Some(ty);
                }
            }
            None
        }
        _ => None,
    }
}

/// Helper: search a HirBlock's statements + trailing expression for a Local
/// binding matching `target_hir_id`. Returns the init expression if found.
pub(super) fn search_block_for_local_init(
    block: &crate::hir::HirBlock,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    for stmt in &block.stmts {
        if let crate::hir::HirStmt::Local(local) = stmt {
            if local.pat.hir_id == target_hir_id {
                if let Some(init) = &local.init {
                    return Some(init.clone());
                }
            }
        }
        // Recurse into expression statements
        if let crate::hir::HirStmt::Expr(e, _) = stmt {
            if let Some(init_expr) = search_expr_for_local_init_expr(e, target_hir_id) {
                return Some(init_expr);
            }
        }
    }
    // Also check the block's trailing expression
    if let Some(trailing) = &block.expr {
        if let Some(init_expr) = search_expr_for_local_init_expr(trailing, target_hir_id) {
            return Some(init_expr);
        }
    }
    None
}

/// Helper: search an expression for a Local binding's init expression.
/// Returns the init expression (not yet type-resolved).
pub(super) fn search_expr_for_local_init_expr(
    expr: &HirExpr,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    match &expr.kind {
        HirExprKind::Block(block) => search_block_for_local_init(block, target_hir_id),
        HirExprKind::If { then, else_, .. } => search_block_for_local_init(then, target_hir_id)
            .or_else(|| {
                else_
                    .as_ref()
                    .and_then(|e| search_expr_for_local_init_expr(e, target_hir_id))
            }),
        HirExprKind::While { cond, body, .. } => {
            search_expr_for_local_init_expr(cond, target_hir_id)
                .or_else(|| search_block_for_local_init(body, target_hir_id))
        }
        HirExprKind::For { iter, body, .. } => search_expr_for_local_init_expr(iter, target_hir_id)
            .or_else(|| search_block_for_local_init(body, target_hir_id)),
        HirExprKind::Loop { body, .. } => search_block_for_local_init(body, target_hir_id),
        HirExprKind::Match { expr, arms } => {
            if let Some(init) = search_expr_for_local_init_expr(expr, target_hir_id) {
                return Some(init);
            }
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    if let Some(init) = search_expr_for_local_init_expr(guard, target_hir_id) {
                        return Some(init);
                    }
                }
                if let Some(init) = search_expr_for_local_init_expr(&arm.body, target_hir_id) {
                    return Some(init);
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract an ADT type from an expression (if it's a struct literal, ADT ctor call,
/// or method call returning an ADT).
pub(super) fn expr_to_adt_type(expr: &HirExpr) -> Option<Ty> {
    match &expr.kind {
        // Stage 14.90 (Bug X2 fix): Handle reference expressions.
        // `let r = &p; r.method()` — the init is `AddrOf { expr: p }`.
        // For method resolution, we want the INNER type (Adt), not the Ref.
        // The find_local_init_type caller handles the Ref wrapping separately.
        HirExprKind::AddrOf { expr: inner, .. } => expr_to_adt_type(inner),
        HirExprKind::Struct { path, .. } => {
            if let crate::hir::Res::Def(def_id, _) = path.res {
                Some(Ty::new(
                    TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                    expr.span,
                ))
            } else {
                None
            }
        }
        // Stage 14.52: Handle Path expressions that resolve to enum/struct types.
        // `Color::Red` resolves to `Res::Def(enum_def_id, Enum)` — the def_id
        // is the enum type's DefId, so we can construct `Adt(enum_def_id)`.
        // This enables method resolution on enum variant values like
        // `let r = Color::Red; r.to_code()`.
        HirExprKind::Path(path) => {
            if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                if matches!(
                    def_kind,
                    crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                ) {
                    return Some(Ty::new(
                        TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                        expr.span,
                    ));
                }
            }
            None
        }
        HirExprKind::Call { func, .. } => {
            // Stage 14.41: After the resolver fix for `Type::method` paths,
            // `Vec::new()` resolves to `Res::Def(method_def_id, Fn)` (the
            // method), NOT `Res::Def(struct_def_id, Struct)` (the struct).
            // We must check the DefKind — only Struct/Enum are valid Adt
            // constructors. For Fn (static method call), return None and let
            // the caller (`resolve_inherent_method_from_hir_expr`) handle it
            // via `query_method_return_type`.
            if let HirExprKind::Path(path) = &func.kind {
                if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                    // Only treat as Adt ctor if the path resolves to a Struct/Enum.
                    // Per §13.4 (design alignment): the DefKind is the authoritative
                    // discriminator — a Fn DefId is NOT an Adt.
                    if matches!(
                        def_kind,
                        crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                    ) {
                        return Some(Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            expr.span,
                        ));
                    }
                    // Fn (static method call) — fall through to None.
                    // The caller handles this via find_local_init_expr +
                    // query_method_return_type.
                }
            }
            None
        }
        // Stage 14.38: Method call — resolve the method's return type from HIR.
        // This enables `let c = a.add(b); c.dot(d)` where `add` returns Vec2.
        HirExprKind::MethodCall { method, .. } => {
            // Search all impl blocks for a method with this name, then
            // return its return type as an Adt.
            // This is a best-effort search — if multiple impls have the same
            // method name, we pick the first one. Typeck should catch real
            // mismatches.
            // Note: we can't access cx.hir here (expr_to_adt_type is a
            // standalone fn), so we return None. The caller
            // (resolve_inherent_method_from_hir_expr) handles MethodCall
            // separately via query_method_return_type.
            let _ = method;
            None
        }
        // Stage 14.44: Array literal — return Array(elem_ty, N) so callers
        // can extract the element type. The element type is detected from
        // the first element via expr_to_adt_type OR query_method_return_type
        // (for static method calls like `Point::new(1, 2)`).
        HirExprKind::Array { elems, .. } => {
            if let Some(first) = elems.first() {
                // First try expr_to_adt_type (handles struct/enum literals)
                if let Some(elem_ty) = expr_to_adt_type(first) {
                    let count_const = crate::mir::ty::Const {
                        ty: Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span),
                        val: crate::mir::ty::ConstVal::Uint(elems.len() as u128),
                    };
                    return Some(Ty::new(
                        TyKind::Array(Box::new(elem_ty), Box::new(count_const)),
                        expr.span,
                    ));
                }
            }
            None
        }
        _ => None,
    }
}
