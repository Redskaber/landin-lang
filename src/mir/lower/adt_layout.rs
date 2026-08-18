//! Stage 6.1: ADT layout extraction from mir/lower/mod.rs (TD-011 split).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (3346 → ~3200).
//! Contains functions for sinking ADT storage layouts from HIR into MIR's
//! `adt_layouts` side-table, so codegen can resolve `TyKind::Adt(def_id, _)`
//! without reading HIR (per §16 — L-PIPE-1 closure from Stage 3.47).
//!
//! Stage 15.8 (v0.2): Added `build_crate_adt_layouts` — builds ALL ADT
//! layouts from HIR upfront (crate-level), eliminating the per-body
//! `populate_adt_layouts` re-scans. The crate-level map is shared across
//! all MirBodies via `Arc<AdtLayouts>`.
//!
//! Stage 18.203 (TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE integrated fix):
//! Added `compute_type_size` — single source of truth for type-size queries
//! needed by runtime intrinsics (Box::new, Vec::push, Vec::get). Eliminates
//! the 3× duplicated size tables that were hardcoded in expr_variants.rs.

use crate::hir::{DefId, HirCrate, HirItem, OwnerNode};
use crate::mir::body::{AdtLayout, AdtLayouts, MirBody, StatementKind};
use crate::mir::place::{AggregateKind, Rvalue};
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;

// Re-export lower_hir_ty_to_mir_ty from the parent module.
use super::lower_hir_ty_to_mir_ty;

/// Stage 15.8 (v0.2): Build ALL ADT layouts from HIR, crate-level.
///
/// Scans every HIR owner for `HirItem::Struct` and `HirItem::Enum`,
/// builds an `AdtLayout` for each, and recursively registers nested ADTs.
/// The resulting map is complete — every ADT defined in the crate has its
/// layout registered, regardless of whether it appears in any body's
/// local_decls.
///
/// This is the root-cause fix for the "re-populate after writeback" hack
/// from Stages 14.41 and 14.84. Previously, `populate_adt_layouts` only
/// registered ADTs that appeared in `mir.local_decls` — but writeback
/// could change a local's type from `Infer` to `Adt(def_id, [])`, exposing
/// new DefIds that weren't registered. The fix was to re-run
/// `populate_adt_layouts` after writeback. With `build_crate_adt_layouts`,
/// all layouts are registered upfront — no re-runs needed.
///
/// Per §15 "最优 > 最小": this is the root-cause fix, not a workaround.
/// Per §1.0 原则 6 "通用 > 特例": one function handles all HIR owner kinds.
/// Per §16: reads HIR (allowed in MIR lower), produces MIR data.
pub fn build_crate_adt_layouts(hir: &HirCrate) -> AdtLayouts {
    let mut layouts: AdtLayouts = AdtLayouts::new();
    for (def_id, _owner) in &hir.owners {
        // Try to build a layout for this DefId. If it's a struct/enum,
        // build_adt_layout returns Some and we register it (plus nested).
        // For non-ADT owners (fns, impls, traits), build_adt_layout returns None.
        if build_adt_layout(*def_id, hir).is_some() {
            register_adt_layout_recursive(&mut layouts, *def_id, hir);
        }
    }
    layouts
}

/// Stage 3.47 (L-PIPE-1 closure): sink ADT layouts from HIR into MIR's
/// `adt_layouts` side-table.
///
/// Walks every local's type and every `AggregateKind::Adt` field type,
/// collecting all `TyKind::Adt(def_id, _)` DefIds. For each unique DefId,
/// builds an `AdtLayout` from HIR and inserts it into `mir.adt_layouts`.
/// Also registers one level of nested Adts.
///
/// Stage 15.8 (v0.2): This function is now DEPRECATED for driver use.
/// The driver should call `build_crate_adt_layouts(hir)` once and share
/// the result via `Arc<AdtLayouts>`. This per-body function is retained
/// for the `lower_hir_body_to_mir` internal call (which runs before the
/// driver has a chance to build the crate-level map), but its result is
/// overwritten by the driver's crate-level map after all bodies are
/// processed.
///
/// Per §15 "最优 > 最小": `build_crate_adt_layouts` is the root-cause fix;
/// this function is kept for backward compatibility during the migration.
pub(crate) fn populate_adt_layouts(mir: &mut MirBody, hir: &HirCrate) {
    // Stage 15.8: MirBody.adt_layouts is now Arc<AdtLayouts> (immutable
    // when shared). To populate, we need to extract the Arc, mutate the
    // inner HashMap, and re-wrap. This is safe because the Arc is not yet
    // shared (we're still in lower_hir_body_to_mir, before the driver
    // shares it across bodies).
    //
    // Arc::make_mut gives us a mutable ref if the Arc has refcount 1
    // (which is the case here — the Arc was just created in MirBody::new),
    // or clones the inner data if shared (which won't happen here).
    let layouts = std::sync::Arc::make_mut(&mut mir.adt_layouts);

    // Collect all DefIds referenced by any local's type (top-level scan).
    let mut def_ids_to_register: Vec<DefId> = Vec::new();
    for ld in &mir.local_decls {
        collect_adt_def_ids(&ld.ty, &mut def_ids_to_register);
    }

    // Also walk AggregateKind::Adt field_tys AND AggregateKind::Closure
    // substs in every Assign statement.
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign(boxed) = &stmt.kind {
                let (_, rvalue) = &**boxed;
                match rvalue {
                    Rvalue::Aggregate(AggregateKind::Adt(_, _, _, field_tys), _) => {
                        for ft in field_tys {
                            collect_adt_def_ids(ft, &mut def_ids_to_register);
                        }
                    }
                    // Stage 14.82 (GAP-7 partial fix): walk closure capture
                    // substs so captured Adts get their layouts registered.
                    Rvalue::Aggregate(AggregateKind::Closure(_, substs), _) => {
                        for st in substs.iter() {
                            collect_adt_def_ids(st, &mut def_ids_to_register);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Register each unique DefId using the Entry API.
    for def_id in def_ids_to_register {
        register_adt_layout_recursive(layouts, def_id, hir);
    }
}

/// Stage 14.43: Recursively register an ADT layout and all of its nested ADTs.
///
/// Previously, `populate_adt_layouts` only registered one level of nesting
/// (e.g., for L1→L2→L3, it registered L1 and L2 but not L3). This caused
/// `mir_type_to_emit_type_with_layouts` to return wrong types for deeply
/// nested structs — L1 would render as `{{i32}}` (2 levels) instead of
/// `{{{i32}}}` (3 levels), causing LLVM type mismatches.
///
/// Per §13.4 (design alignment): the layout registry should be complete —
/// all reachable ADTs should have their layouts registered. This function
/// walks the nesting chain recursively until no new ADTs are found.
fn register_adt_layout_recursive(
    layouts: &mut std::collections::HashMap<DefId, AdtLayout>,
    def_id: DefId,
    hir: &HirCrate,
) {
    use std::collections::hash_map::Entry;
    if let Entry::Vacant(e) = layouts.entry(def_id) {
        if let Some(layout) = build_adt_layout(def_id, hir) {
            let nested: Vec<DefId> = layout.field_def_ids();
            e.insert(layout);
            // Recursively register all nested ADTs (any depth).
            for nested_id in nested {
                register_adt_layout_recursive(layouts, nested_id, hir);
            }
        }
    }
}

/// Walk a `Ty` and collect every `TyKind::Adt(def_id, _)` DefId into `out`.
/// Recurses into Tuple, Array, Ref, RawPtr, Slice.
fn collect_adt_def_ids(ty: &Ty, out: &mut Vec<DefId>) {
    match &ty.kind {
        TyKind::Adt(def_id, _) => out.push(*def_id),
        TyKind::Tuple(tys) => {
            for t in tys {
                collect_adt_def_ids(t, out);
            }
        }
        TyKind::Array(elem, _) => collect_adt_def_ids(elem, out),
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => collect_adt_def_ids(inner, out),
        TyKind::Slice(elem) => collect_adt_def_ids(elem, out),
        // Stage 14.82 (GAP-7 partial fix): recurse into Closure substs so
        // captured Adts get their layouts registered. Without this, a
        // closure capturing a struct would have the struct's layout missing
        // from `mir.adt_layouts`, causing `mir_type_to_emit_type_with_layouts`
        // to fall back to `EmitType::I32` for the captured struct type —
        // producing wrong LLVM types and "Invalid InsertValueInst operands!"
        // errors.
        TyKind::Closure(_, substs) => {
            for t in substs.iter() {
                collect_adt_def_ids(t, out);
            }
        }
        _ => {}
    }
}

/// Build an `AdtLayout` for the given DefId by reading HIR.
/// Returns `None` if the DefId doesn't resolve to a struct or enum.
fn build_adt_layout(def_id: DefId, hir: &HirCrate) -> Option<AdtLayout> {
    let owner = hir.find_owner(def_id)?;
    match owner {
        OwnerNode::Item(HirItem::Struct(s)) => {
            let field_tys = s
                .fields
                .iter()
                .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                .collect();
            Some(AdtLayout::Struct { field_tys })
        }
        OwnerNode::Item(HirItem::Enum(e)) => {
            let discriminant_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
            let variant_payloads: Vec<Vec<Ty>> = e
                .variants
                .iter()
                .map(|variant| match &variant.data {
                    crate::hir::HirVariantData::Unit(_) => Vec::new(),
                    crate::hir::HirVariantData::Tuple(fields, _) => fields
                        .iter()
                        .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                        .collect(),
                    crate::hir::HirVariantData::Struct(fields, _) => fields
                        .iter()
                        .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                        .collect(),
                })
                .collect();
            Some(AdtLayout::Enum {
                discriminant_ty,
                variant_payloads,
            })
        }
        _ => None,
    }
}

/// Extension method on AdtLayout to extract nested Adt DefIds (for recursion).
trait AdtLayoutExt {
    fn field_def_ids(&self) -> Vec<DefId>;
}

impl AdtLayoutExt for AdtLayout {
    fn field_def_ids(&self) -> Vec<DefId> {
        let mut out = Vec::new();
        match self {
            AdtLayout::Struct { field_tys } => {
                for t in field_tys {
                    collect_adt_def_ids(t, &mut out);
                }
            }
            AdtLayout::Enum {
                variant_payloads, ..
            } => {
                for payload in variant_payloads {
                    for t in payload {
                        collect_adt_def_ids(t, &mut out);
                    }
                }
            }
        }
        out
    }
}

/// Stage 18.203 (TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE integrated fix):
/// Compute the byte size of a MIR type for runtime operations
/// (Box::new allocation, Vec::push/get elem_size).
///
/// This is the **single source of truth** for type-size queries needed by
/// runtime intrinsics — eliminates the 3× duplicated size tables that were
/// previously hardcoded in:
///   - `lower_box_new_intrinsic` (Box::new — TD-BOX-SIZE-OF)
///   - `lower_vec_push_intrinsic` (Vec::push — TD-VEC-ELEM-SIZE-INFERENCE)
///   - `lower_vec_get_intrinsic`  (Vec::get  — TD-VEC-ELEM-SIZE-INFERENCE)
///
/// Per §10 (DRY): one definition, consumed by all 3 intrinsics.
/// Per §12 (最优 > 最小): walks Adt HIR for proper struct/enum size, not
///   hardcoded "default 8".
/// Per §1.0 原则 6 (通解>特例): one function handles all `TyKind` variants.
///
/// # Size rules
///
/// | TyKind variant | Size (bytes) | Notes |
/// |----------------|-------------|-------|
/// | Bool           | 1           | Fixed ABI |
/// | Char           | 4           | Fixed ABI (Rust char = 4 bytes) |
/// | Int/Uint       | 1/2/4/8/16  | Per bit-width |
/// | Float          | 4 (f32) / 8 (f64) | Per precision |
/// | Never          | 0           | Uninhabited ZST |
/// | Tuple          | Σ field sizes | No padding (Landin MVP ≈ `repr(Rust)` natural alignment) |
/// | Array          | elem_size × count | When count is literal const |
/// | Adt (struct)   | Σ field sizes (recursive) | Walks HIR via `build_adt_layout` |
/// | Adt (enum)     | disc(4) + max(payload)    | Walks HIR via `build_adt_layout` |
/// | Ref/RawPtr/FnDef/FnPtr | 8   | Pointer-sized (64-bit target) |
/// | Str/Slice      | 0           | Unsized — caller should reject |
/// | Foreign/Closure/Projection | 8 | Opaque / fallback |
/// | Param/Infer/Error | **caller-supplied fallback** | Use `compute_type_size_with_fallback` to specify. Default 8 (`compute_type_size`). Vec ops pass 4 (canonical Vec<i32>). |
///
/// # Arguments
///
/// * `ty` — the MIR type whose size to compute
/// * `hir` — optional HIR crate reference, needed to walk struct/enum
///   definitions. `None` in test contexts that build MIR without HIR.
///
/// # Returns
///
/// The size in bytes as `i64` (signed to match LLVM `i64` size operands).
/// Returns `0` for unsized types (Str, Slice) and `8` (caller-supplied
/// fallback) for opaque/unknown types.
pub fn compute_type_size(ty: &Ty, hir: Option<&HirCrate>) -> i64 {
    compute_type_size_with_fallback(ty, hir, 8)
}

/// Stage 18.203: Variant of `compute_type_size` with a caller-supplied
/// fallback for `Infer`/`Param`/`Error` types.
///
/// **Why a fallback parameter?** At MIR-lower time, generic types (`Param`)
/// and inference variables (`Infer`) may not yet be resolved to concrete
/// types — typeck writeback runs *after* MIR lower. Different intrinsics
/// need different fallback behavior:
///
/// | Caller | Fallback | Rationale |
/// |--------|----------|-----------|
/// | `Box::new` | 8 | Safe over-allocation (Box just stores + Deref-loads; extra bytes unused) |
/// | `Vec::push` / `Vec::get` | 4 | Canonical `Vec<i32>` case; **must match** between push and get or Vec offsets corrupt |
///
/// Per §1.0 原则 6 (通解>特例): one function, parametric on fallback —
/// callers specify their domain-specific default rather than each caller
/// re-implementing the size table.
/// Per §10 (DRY): the primitive/Adt/Tuple/Array rules are defined once.
///
/// # Arguments
///
/// * `ty` — the MIR type whose size to compute
/// * `hir` — optional HIR crate reference (needed for Adt walks)
/// * `fallback` — size returned for `Param`/`Infer`/`Error` (caller-specific)
///
/// # Returns
///
/// Size in bytes; `fallback` for unresolved generic/inference types.
pub fn compute_type_size_with_fallback(ty: &Ty, hir: Option<&HirCrate>, fallback: i64) -> i64 {
    match &ty.kind {
        // Primitives — fixed ABI sizes.
        TyKind::Bool => 1,
        TyKind::Char => 4,
        TyKind::Int(int_ty) => match int_ty {
            crate::ast::IntTy::I8 => 1,
            crate::ast::IntTy::I16 => 2,
            crate::ast::IntTy::I32 => 4,
            crate::ast::IntTy::I64 => 8,
            crate::ast::IntTy::I128 => 16,
            crate::ast::IntTy::Isize => 8,
        },
        TyKind::Uint(uint_ty) => match uint_ty {
            crate::ast::UintTy::U8 => 1,
            crate::ast::UintTy::U16 => 2,
            crate::ast::UintTy::U32 => 4,
            crate::ast::UintTy::U64 => 8,
            crate::ast::UintTy::U128 => 16,
            crate::ast::UintTy::Usize => 8,
        },
        TyKind::Float(float_ty) => match float_ty {
            crate::ast::FloatTy::F32 => 4,
            crate::ast::FloatTy::F64 => 8,
        },
        // Unit-like types.
        TyKind::Never => 0,
        TyKind::Tuple(tys) => {
            // Sum of field sizes. Landin MVP uses natural alignment without
            // explicit padding (matches `repr(Rust)`); the sum is an
            // approximation that's correct when fields are naturally aligned.
            // TODO: proper layout with alignment (TD-LAYOUT-ALIGNMENT, v0.3+).
            tys.iter()
                .map(|t| compute_type_size_with_fallback(t, hir, fallback))
                .sum()
        }
        TyKind::Array(elem, count) => {
            // count × elem_size when count is a literal const.
            // TODO: const evaluation for non-literal counts (v0.2+).
            let elem_size = compute_type_size_with_fallback(elem, hir, fallback);
            let count_val: i64 = match &count.val {
                crate::mir::ty::ConstVal::Int(n) => *n as i64,
                crate::mir::ty::ConstVal::Uint(n) => *n as i64,
                _ => 0, // Unevaluated const → 0 (caller should handle)
            };
            elem_size * count_val
        }
        TyKind::Adt(def_id, _) => {
            // Walk HIR to compute struct/enum size. This is the proper
            // root-cause fix for TD-BOX-SIZE-OF (Box::new of structs)
            // and TD-VEC-ELEM-SIZE-INFERENCE (Vec<MyStruct>).
            if let Some(hir_ref) = hir {
                if let Some(layout) = build_adt_layout(*def_id, hir_ref) {
                    return adt_layout_size(&layout, Some(hir_ref), fallback);
                }
            }
            // HIR unavailable or DefId not a struct/enum (e.g., type alias).
            // Fallback: caller-supplied (Box=8, Vec=4) — matches MVP behavior.
            fallback
        }
        // Refs/Ptrs: pointer-sized (8 bytes on 64-bit).
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => 8,
        TyKind::FnDef(_, _) | TyKind::FnPtr(_) => 8,
        // Str is unsized; for sized contexts (Box<str> is unusual), return 0.
        // The caller should usually reject this.
        TyKind::Str => 0,
        // Slices are unsized; same handling as Str.
        TyKind::Slice(_) => 0,
        // Foreign types: opaque, fallback.
        TyKind::Foreign => fallback,
        // Closure = { captures }; MVP approximation: pointer-sized
        // (the closure struct's actual size needs capture analysis, v0.2+).
        TyKind::Closure(_, _) => fallback,
        // Unresolved associated type projection; fallback.
        TyKind::Projection(_, _) => fallback,
        // Generic param / Infer / Error: cannot compute statically.
        // Returns caller-supplied fallback — proper fix requires typeck
        // generic instantiation (TD-TYPECK-GENERIC-INST, v0.2 P2+).
        TyKind::Param(_) | TyKind::Infer(_) | TyKind::Error => fallback,
    }
}

/// Compute the byte size of an `AdtLayout` (struct or enum).
///
/// For struct: sum of field sizes (recursive, no padding — Landin MVP
/// approximation matching `repr(Rust)` natural alignment).
/// For enum: discriminant_size + max(variant_payload_size).
///
/// Per §1.0 原则 6 (通解>特例): one function handles both AdtLayout variants.
fn adt_layout_size(layout: &AdtLayout, hir: Option<&HirCrate>, fallback: i64) -> i64 {
    match layout {
        AdtLayout::Struct { field_tys } => field_tys
            .iter()
            .map(|t| compute_type_size_with_fallback(t, hir, fallback))
            .sum(),
        AdtLayout::Enum {
            discriminant_ty,
            variant_payloads,
        } => {
            let disc_size = compute_type_size_with_fallback(discriminant_ty, hir, fallback);
            let max_payload: i64 = variant_payloads
                .iter()
                .map(|payload| {
                    payload
                        .iter()
                        .map(|t| compute_type_size_with_fallback(t, hir, fallback))
                        .sum::<i64>()
                })
                .max()
                .unwrap_or(0);
            disc_size + max_payload
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::ty::{Const, ConstVal, InferVar, Ty, TyKind};

    #[test]
    fn stage18_203_primitive_sizes() {
        let hir = None;
        assert_eq!(
            compute_type_size(&Ty::new(TyKind::Bool, Span::DUMMY), hir),
            1
        );
        assert_eq!(
            compute_type_size(&Ty::new(TyKind::Char, Span::DUMMY), hir),
            4
        );
        assert_eq!(
            compute_type_size(
                &Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                hir
            ),
            4
        );
        assert_eq!(
            compute_type_size(
                &Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY),
                hir
            ),
            8
        );
        assert_eq!(
            compute_type_size(
                &Ty::new(TyKind::Int(crate::ast::IntTy::I128), Span::DUMMY),
                hir
            ),
            16
        );
        assert_eq!(
            compute_type_size(
                &Ty::new(TyKind::Uint(crate::ast::UintTy::U8), Span::DUMMY),
                hir
            ),
            1
        );
        assert_eq!(
            compute_type_size(
                &Ty::new(TyKind::Float(crate::ast::FloatTy::F64), Span::DUMMY),
                hir
            ),
            8
        );
    }

    #[test]
    fn stage18_203_tuple_size_is_sum_of_fields() {
        let hir = None;
        let tuple_ty = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY),
                Ty::new(TyKind::Bool, Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        // 4 + 8 + 1 = 13
        assert_eq!(compute_type_size(&tuple_ty, hir), 13);
    }

    #[test]
    fn stage18_203_array_size_is_elem_times_count() {
        let hir = None;
        let array_ty = Ty::new(
            TyKind::Array(
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
                Box::new(Const {
                    ty: Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY),
                    val: ConstVal::Int(10),
                }),
            ),
            Span::DUMMY,
        );
        // 4 × 10 = 40
        assert_eq!(compute_type_size(&array_ty, hir), 40);
    }

    #[test]
    fn stage18_203_pointer_size_is_8() {
        let hir = None;
        let ref_ty = Ty::new(
            TyKind::Ref(
                crate::mir::ty::Region::Erased,
                crate::mir::ty::Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        assert_eq!(compute_type_size(&ref_ty, hir), 8);
    }

    #[test]
    fn stage18_203_infer_param_fallback_is_8() {
        let hir = None;
        let infer_ty = Ty::new(
            TyKind::Infer(InferVar::TyVar(crate::mir::ty::TyVid(0))),
            Span::DUMMY,
        );
        assert_eq!(compute_type_size(&infer_ty, hir), 8);
        // Param<T> uses the same fallback branch as Infer — verified via
        // direct inspection of `compute_type_size`'s match arms (Param|Infer|Error => 8).
        // ParamTy construction requires a Symbol (lasso::Spur) which is non-trivial
        // in unit tests without an interner; the Infer test exercises the same code path.
    }

    #[test]
    fn stage18_203_unit_tuple_is_zero() {
        let hir = None;
        let unit_ty = Ty::new(TyKind::Tuple(vec![]), Span::DUMMY);
        assert_eq!(compute_type_size(&unit_ty, hir), 0);
    }
}
