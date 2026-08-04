//! Stage 16.54 (Task 11 Phase 3): Monomorphization collection — walk MIR
//! bodies and collect `MonoItem { def_id, substs }` pairs for codegen.
//!
//! Stage 16.55 (Task 11 Phase 4a): Specialized naming — `mangle_ty`,
//! `mono_item_name`, `build_mono_item_names`.
//!
//! Stage 16.57 (Task 11 Phase 4b): Per-mono layouts — `MonoLayoutKey`,
//! `MonoLayoutMap`, `build_mono_layouts`.
//!
//! This module provides the `collect_mono_items` function, which walks all
//! MIR bodies in a crate and collects the set of generic instantiations
//! that need specialized codegen. Each `MonoItem` represents one
//! specialization: e.g., `Vec<i32>` and `Vec<bool>` are two distinct
//! MonoItems.
//!
//! ## Algorithm
//!
//! `collect_mono_items` walks each `MirBody` and inspects:
//! - `local_decls[i].ty` — local variable types
//! - `Rvalue::Aggregate(AggregateKind::Adt(def_id, substs, ...), _)` — struct/enum construction
//! - `Rvalue::Cast(_, _, ty)` — cast target types
//! - `ProjectionElem::Field(_, ty)` — field projection types
//! - `TerminatorKind::Call { func, .. }` — function call operand types
//! - `AggregateKind::Array(ty)` — array element types
//!
//! For each type encountered, it extracts `MonoItem`s from:
//! - `TyKind::Adt(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Type { def_id, substs }`
//! - `TyKind::FnDef(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Fn { def_id, substs }`
//! - `TyKind::Closure(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Closure { def_id, substs }`
//! - Recursively walks inner substs (e.g., `Vec<Vec<i32>>` → outer Vec<i32> + inner Vec<i32>)
//!
//! ## Deduplication
//!
//! MonoItems are deduplicated by `(def_id, substs)` using a `HashSet`.
//! `Vec<i32>` used in 100 places produces 1 MonoItem.
//!
//! ## What This Does NOT Do (Phase 4)
//!
//! `collect_mono_items` only collects — it doesn't generate specialized
//! code. Phase 4 (per-mono codegen) will use the collected MonoItems to
//! emit specialized LLVM types/functions. For now, the collected items
//! are available for inspection and future codegen integration.
//!
//! Per §23: `collect_mono_items` follows `<verb>_<noun>_<noun>` pattern.
//! Per §16: reads MIR only (no HIR access during collection).
//! Per §1.0 原則 6 "通用 > 特例": one collection function for all type kinds.

use crate::hir::DefId;
use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{AggregateKind, Operand, Place, PlaceKind, ProjectionElem, Rvalue};
use crate::mir::ty::{SubstsRef, Ty, TyKind};
use std::collections::HashSet;

/// A monomorphization item: one specialization of a generic definition.
///
/// Each MonoItem represents one concrete instantiation of a generic type
/// or function. For example, `Vec<i32>` and `Vec<bool>` are two distinct
/// MonoItems with the same `def_id` (Vec) but different `substs`.
///
/// Per §23: `MonoItem` follows `<Noun>_<Noun>` pattern (data type).
/// Per §16: pure data — no behavior, just a key for codegen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonoItem {
    /// A monomorphized type: `Vec<i32>`, `Pair<i32, bool>`, etc.
    Type { def_id: DefId, substs: SubstsRef },
    /// A monomorphized function: `fn id<T>(x: T) -> T` called with `i32`.
    Fn { def_id: DefId, substs: SubstsRef },
    /// A monomorphized closure: `Closure<i32>` (closure with i32 captures).
    Closure { def_id: DefId, substs: SubstsRef },
}

impl MonoItem {
    /// Get the DefId of this MonoItem.
    pub fn def_id(&self) -> DefId {
        match self {
            MonoItem::Type { def_id, .. }
            | MonoItem::Fn { def_id, .. }
            | MonoItem::Closure { def_id, .. } => *def_id,
        }
    }

    /// Get the substs of this MonoItem.
    pub fn substs(&self) -> &SubstsRef {
        match self {
            MonoItem::Type { substs, .. }
            | MonoItem::Fn { substs, .. }
            | MonoItem::Closure { substs, .. } => substs,
        }
    }

    /// Format this MonoItem as a human-readable string (for debugging).
    pub fn debug_string(&self) -> String {
        format!(
            "MonoItem::{}({:?}, {:?})",
            self.kind_str(),
            self.def_id(),
            self.substs()
        )
    }

    fn kind_str(&self) -> &'static str {
        match self {
            MonoItem::Type { .. } => "Type",
            MonoItem::Fn { .. } => "Fn",
            MonoItem::Closure { .. } => "Closure",
        }
    }
}

/// Collect all MonoItems from a slice of MIR bodies.
///
/// Walks each MIR body and collects `MonoItem`s from:
/// - Local declarations (local_decls[i].ty)
/// - Statements (Rvalue::Aggregate, Rvalue::Cast)
/// - Terminators (TerminatorKind::Call)
/// - Projection elements (Field types)
///
/// Returns a deduplicated `Vec<MonoItem>`. The order is unspecified
/// (HashSet iteration order).
///
/// Per §23: `collect_mono_items` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads MIR only (no HIR access).
/// Per §1.0 原則 6 "通用 > 特例": one function for all MIR body kinds.
pub fn collect_mono_items(mirs: &[MirBody]) -> Vec<MonoItem> {
    let mut collected: HashSet<MonoItem> = HashSet::new();
    for mir in mirs {
        collect_from_mir_body(mir, &mut collected);
    }
    collected.into_iter().collect()
}

/// Collect MonoItems from a single MIR body.
fn collect_from_mir_body(mir: &MirBody, collected: &mut HashSet<MonoItem>) {
    // 1. Collect from local declarations.
    for local_decl in &mir.local_decls {
        collect_from_ty(&local_decl.ty, collected);
    }

    // 2. Collect from basic blocks (statements + terminators).
    for block in &mir.basic_blocks {
        for stmt in &block.statements {
            collect_from_statement(&stmt.kind, collected);
        }
        collect_from_terminator(&block.terminator.kind, collected);
    }
}

/// Collect MonoItems from a statement.
fn collect_from_statement(stmt: &StatementKind, collected: &mut HashSet<MonoItem>) {
    match stmt {
        StatementKind::Assign(boxed) => {
            let (_, rvalue) = &**boxed;
            collect_from_rvalue(rvalue, collected);
        }
        StatementKind::Println { args, .. } => {
            // Println args are operands — collect from their types.
            for arg in args {
                collect_from_operand(arg, collected);
            }
        }
        // Nop, StorageLive, StorageDead, Deinit — no types to collect.
        _ => {}
    }
}

/// Collect MonoItems from a rvalue.
fn collect_from_rvalue(rvalue: &Rvalue, collected: &mut HashSet<MonoItem>) {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::UnaryOp(_, operand) => {
            collect_from_operand(operand, collected);
        }
        Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
            collect_from_operand(a, collected);
            collect_from_operand(b, collected);
        }
        Rvalue::Ref(_, _, place) => {
            collect_from_place(place, collected);
        }
        Rvalue::Cast(_, operand, ty) => {
            collect_from_operand(operand, collected);
            collect_from_ty(ty, collected);
        }
        Rvalue::Aggregate(kind, operands) => {
            collect_from_aggregate_kind(kind, collected);
            for operand in operands {
                collect_from_operand(operand, collected);
            }
        }
    }
}

/// Collect MonoItems from an aggregate kind.
fn collect_from_aggregate_kind(kind: &AggregateKind, collected: &mut HashSet<MonoItem>) {
    match kind {
        AggregateKind::Array(ty) => collect_from_ty(ty, collected),
        AggregateKind::Adt(def_id, _, substs, field_tys) => {
            // Collect the Adt itself if it has non-empty substs.
            if !substs.is_empty() {
                collected.insert(MonoItem::Type {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            // Collect from inner substs (e.g., Vec<Vec<i32>> → inner Vec<i32>).
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
            // Collect from field types.
            for field_ty in field_tys {
                collect_from_ty(field_ty, collected);
            }
        }
        AggregateKind::Closure(def_id, substs) => {
            if !substs.is_empty() {
                collected.insert(MonoItem::Closure {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        AggregateKind::Tuple => {}
    }
}

/// Collect MonoItems from an operand.
fn collect_from_operand(operand: &Operand, collected: &mut HashSet<MonoItem>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            collect_from_place(place, collected);
        }
        Operand::Constant(const_val) => {
            collect_from_ty(&const_val.ty, collected);
        }
    }
}

/// Collect MonoItems from a place.
fn collect_from_place(place: &Place, collected: &mut HashSet<MonoItem>) {
    match &place.kind {
        PlaceKind::Local(_) | PlaceKind::Static(_) => {}
        PlaceKind::Projection(base, elem) => {
            collect_from_place(base, collected);
            collect_from_projection_elem(elem, collected);
        }
    }
}

/// Collect MonoItems from a projection element.
fn collect_from_projection_elem(elem: &ProjectionElem, collected: &mut HashSet<MonoItem>) {
    match elem {
        ProjectionElem::Field(_, ty) => collect_from_ty(ty, collected),
        ProjectionElem::Deref
        | ProjectionElem::Index(_)
        | ProjectionElem::ConstantIndex { .. }
        | ProjectionElem::Subslice { .. } => {}
    }
}

/// Collect MonoItems from a terminator.
fn collect_from_terminator(term: &TerminatorKind, collected: &mut HashSet<MonoItem>) {
    match term {
        TerminatorKind::Call { func, args, .. } => {
            collect_from_operand(func, collected);
            for arg in args {
                collect_from_operand(arg, collected);
            }
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            collect_from_operand(discr, collected);
        }
        TerminatorKind::Drop { place, .. } => {
            collect_from_place(place, collected);
        }
        TerminatorKind::Assert { cond, .. } => {
            collect_from_operand(cond, collected);
        }
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

/// Collect MonoItems from a type.
///
/// This is the core type-walking function. It extracts MonoItems from:
/// - `TyKind::Adt(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Type`
/// - `TyKind::FnDef(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Fn`
/// - `TyKind::Closure(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Closure`
///
/// It also recursively walks inner types (substs, Ref inner, Tuple elements,
/// Array/Slice element, etc.) to find nested MonoItems.
///
/// Per §23: `collect_from_ty` follows `<verb>_<prep>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all type kinds.
pub fn collect_from_ty(ty: &Ty, collected: &mut HashSet<MonoItem>) {
    match &ty.kind {
        // Generic-capable types — collect if substs are non-empty.
        TyKind::Adt(def_id, substs) => {
            if !substs.is_empty() {
                collected.insert(MonoItem::Type {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            // Recursively collect from inner substs (e.g., Vec<Vec<i32>>).
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::FnDef(def_id, substs) => {
            if !substs.is_empty() {
                collected.insert(MonoItem::Fn {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::Closure(def_id, substs) => {
            if !substs.is_empty() {
                collected.insert(MonoItem::Closure {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }

        // Recursive types — walk inner.
        TyKind::Ref(_, _, inner) => collect_from_ty(inner, collected),
        TyKind::RawPtr(_, inner) => collect_from_ty(inner, collected),
        TyKind::Array(inner, _) => collect_from_ty(inner, collected),
        TyKind::Slice(inner) => collect_from_ty(inner, collected),
        TyKind::Tuple(tys) => {
            for inner_ty in tys {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::FnPtr(sig) => {
            for input_ty in &sig.inputs {
                collect_from_ty(input_ty, collected);
            }
            collect_from_ty(&sig.output, collected);
        }

        // Leaf types — no MonoItems to collect.
        TyKind::Bool
        | TyKind::Char
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Str
        | TyKind::Never
        | TyKind::Foreign
        | TyKind::Error
        | TyKind::Param(_)
        | TyKind::Infer(_) => {}
    }
}

// =====================================================================
// Stage 16.55 (Task 11 Phase 4): Per-mono codegen — specialized naming
// =====================================================================

/// Mangle a `Ty` to a compact string suitable for use in specialized
/// function/type names.
///
/// This is the "no interner" variant — Adt types use DefId fallback
/// (e.g., `Adt_5_i32`). Use `mangle_ty_with_interner` for readable type
/// names.
///
/// Examples:
/// - `i32` → `"i32"`
/// - `bool` → `"bool"`
/// - `Adt(Box, [i32])` → `"Adt_5_i32"` (DefId fallback)
/// - `Ref(_, _, i32)` → `"ref_i32"`
/// - `Tuple([i32, bool])` → `"tuple_i32_bool"`
/// - `Array(i32, 10)` → `"array_i32_10"`
/// - `Slice(i32)` → `"slice_i32"`
///
/// Per §23: `mangle_ty` follows `<verb>_<noun>` pattern.
/// Per §16: reads Ty only (no HIR, no interner access).
pub fn mangle_ty(ty: &Ty) -> String {
    match &ty.kind {
        TyKind::Bool => "bool".to_string(),
        TyKind::Char => "char".to_string(),
        TyKind::Int(int_ty) => format!("{:?}", int_ty).to_lowercase(),
        TyKind::Uint(uint_ty) => format!("{:?}", uint_ty).to_lowercase(),
        TyKind::Float(float_ty) => format!("{:?}", float_ty).to_lowercase(),
        TyKind::Str => "str".to_string(),
        TyKind::Never => "never".to_string(),
        TyKind::Foreign => "foreign".to_string(),
        TyKind::Error => "error".to_string(),
        TyKind::Param(param_ty) => {
            // For Param, use the name if available, else the index.
            format!("param_{}", param_ty.index)
        }
        TyKind::Infer(_) => "infer".to_string(),

        TyKind::Ref(_, mutability, inner) => {
            let prefix = match mutability {
                crate::mir::ty::Mutability::Mutable => "refmut",
                crate::mir::ty::Mutability::Immutable => "ref",
            };
            format!("{}_{}", prefix, mangle_ty(inner))
        }
        TyKind::RawPtr(mutability, inner) => {
            let prefix = match mutability {
                crate::mir::ty::Mutability::Mutable => "ptrmut",
                crate::mir::ty::Mutability::Immutable => "ptr",
            };
            format!("{}_{}", prefix, mangle_ty(inner))
        }
        TyKind::Array(inner, len) => {
            let len_str = match &len.val {
                crate::mir::ty::ConstVal::Uint(n) => n.to_string(),
                crate::mir::ty::ConstVal::Int(n) => n.to_string(),
                _ => "unknown".to_string(),
            };
            format!("array_{}_{}", mangle_ty(inner), len_str)
        }
        TyKind::Slice(inner) => {
            format!("slice_{}", mangle_ty(inner))
        }
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                "unit".to_string()
            } else {
                let parts: Vec<String> = tys.iter().map(mangle_ty).collect();
                format!("tuple_{}", parts.join("_"))
            }
        }
        TyKind::Adt(def_id, substs) => {
            // Without an interner, we can't resolve Symbol to a string.
            // Use DefId fallback. Use mangle_ty_with_interner for readable names.
            let base_name = format!("Adt_{}", def_id.as_u32());
            if substs.is_empty() {
                base_name
            } else {
                let substs_str: Vec<String> = substs.iter().map(mangle_ty).collect();
                format!("{}_{}", base_name, substs_str.join("_"))
            }
        }
        TyKind::FnDef(def_id, substs) => {
            let base = format!("fn_{}", def_id.as_u32());
            if substs.is_empty() {
                base
            } else {
                let substs_str: Vec<String> = substs.iter().map(mangle_ty).collect();
                format!("{}_{}", base, substs_str.join("_"))
            }
        }
        TyKind::Closure(def_id, substs) => {
            let base = format!("closure_{}", def_id.as_u32());
            if substs.is_empty() {
                base
            } else {
                let substs_str: Vec<String> = substs.iter().map(mangle_ty).collect();
                format!("{}_{}", base, substs_str.join("_"))
            }
        }
        TyKind::FnPtr(sig) => {
            let inputs: Vec<String> = sig.inputs.iter().map(mangle_ty).collect();
            let output = mangle_ty(&sig.output);
            format!("fnptr_{}__{}", inputs.join("_"), output)
        }
    }
}

/// Mangle a `Ty` to a compact string using resolved type names.
///
/// This is a convenience wrapper around `mangle_ty` that resolves
/// `Symbol` values to strings using the provided interner.
///
/// Per §23: `mangle_ty_with_interner` follows `<verb>_<noun>_<prep>_<noun>`
/// pattern.
pub fn mangle_ty_with_interner(
    ty: &Ty,
    type_name_by_def_id: &std::collections::HashMap<DefId, crate::lexer::Symbol>,
    interner: &lasso::Rodeo,
) -> String {
    match &ty.kind {
        TyKind::Adt(def_id, substs) => {
            let base_name = type_name_by_def_id
                .get(def_id)
                .and_then(|s| interner.try_resolve(s))
                .map(String::from)
                .unwrap_or_else(|| format!("Adt_{}", def_id.as_u32()));
            if substs.is_empty() {
                base_name
            } else {
                let substs_str: Vec<String> = substs
                    .iter()
                    .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
                    .collect();
                format!("{}_{}", base_name, substs_str.join("_"))
            }
        }
        TyKind::Ref(_, mutability, inner) => {
            let prefix = match mutability {
                crate::mir::ty::Mutability::Mutable => "refmut",
                crate::mir::ty::Mutability::Immutable => "ref",
            };
            format!(
                "{}_{}",
                prefix,
                mangle_ty_with_interner(inner, type_name_by_def_id, interner)
            )
        }
        TyKind::RawPtr(mutability, inner) => {
            let prefix = match mutability {
                crate::mir::ty::Mutability::Mutable => "ptrmut",
                crate::mir::ty::Mutability::Immutable => "ptr",
            };
            format!(
                "{}_{}",
                prefix,
                mangle_ty_with_interner(inner, type_name_by_def_id, interner)
            )
        }
        TyKind::Array(inner, len) => {
            let len_str = match &len.val {
                crate::mir::ty::ConstVal::Uint(n) => n.to_string(),
                crate::mir::ty::ConstVal::Int(n) => n.to_string(),
                _ => "unknown".to_string(),
            };
            format!(
                "array_{}_{}",
                mangle_ty_with_interner(inner, type_name_by_def_id, interner),
                len_str
            )
        }
        TyKind::Slice(inner) => format!(
            "slice_{}",
            mangle_ty_with_interner(inner, type_name_by_def_id, interner)
        ),
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                "unit".to_string()
            } else {
                let parts: Vec<String> = tys
                    .iter()
                    .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
                    .collect();
                format!("tuple_{}", parts.join("_"))
            }
        }
        TyKind::FnDef(def_id, substs) => {
            let base = format!("fn_{}", def_id.as_u32());
            if substs.is_empty() {
                base
            } else {
                let substs_str: Vec<String> = substs
                    .iter()
                    .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
                    .collect();
                format!("{}_{}", base, substs_str.join("_"))
            }
        }
        TyKind::Closure(def_id, substs) => {
            let base = format!("closure_{}", def_id.as_u32());
            if substs.is_empty() {
                base
            } else {
                let substs_str: Vec<String> = substs
                    .iter()
                    .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
                    .collect();
                format!("{}_{}", base, substs_str.join("_"))
            }
        }
        TyKind::FnPtr(sig) => {
            let inputs: Vec<String> = sig
                .inputs
                .iter()
                .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
                .collect();
            let output = mangle_ty_with_interner(&sig.output, type_name_by_def_id, interner);
            format!("fnptr_{}__{}", inputs.join("_"), output)
        }
        // Leaf types — delegate to mangle_ty (no name resolution needed).
        _ => mangle_ty(ty),
    }
}

/// Generate a specialized name for a `MonoItem`.
///
/// Examples:
/// - `Type { def_id: Box, substs: [i32] }` + base "Box" → `"Box_i32"`
/// - `Fn { def_id: id, substs: [i32] }` + base "id" → `"id_i32"`
/// - `Closure { def_id, substs: [i32] }` + base "call" → `"call_i32"`
///
/// The `base_name` is the unspecialized name (e.g., "Box", "id", "call").
/// For types, this comes from `type_name_by_def_id`. For functions, this
/// comes from `fn_name_by_def_id` (stripped of the `landin_` prefix).
///
/// Per §23: `mono_item_name` follows `<noun>_<noun>_<noun>` pattern.
pub fn mono_item_name(
    item: &MonoItem,
    base_name: &str,
    type_name_by_def_id: &std::collections::HashMap<DefId, crate::lexer::Symbol>,
    interner: &lasso::Rodeo,
) -> String {
    let substs = item.substs();
    if substs.is_empty() {
        return base_name.to_string();
    }
    let substs_str: Vec<String> = substs
        .iter()
        .map(|t| mangle_ty_with_interner(t, type_name_by_def_id, interner))
        .collect();
    format!("{}_{}", base_name, substs_str.join("_"))
}

/// Build a map from `MonoItem` to specialized name.
///
/// For each MonoItem, looks up the base name:
/// - `Type { def_id, .. }` → from `type_name_by_def_id` (resolved via interner)
/// - `Fn { def_id, .. }` → from `fn_name_by_def_id` (stripped of `landin_` prefix)
/// - `Closure { def_id, .. }` → `closure_<def_id>` (no base name map)
///
/// Then applies `mono_item_name` to generate the specialized name.
///
/// Per §23: `build_mono_item_names` follows `<verb>_<noun>_<noun>_<noun>`
/// pattern.
pub fn build_mono_item_names(
    items: &[MonoItem],
    fn_name_by_def_id: &std::collections::HashMap<DefId, String>,
    type_name_by_def_id: &std::collections::HashMap<DefId, crate::lexer::Symbol>,
    interner: &lasso::Rodeo,
) -> std::collections::HashMap<MonoItem, String> {
    let mut map = std::collections::HashMap::new();
    for item in items {
        let base_name = match item {
            MonoItem::Type { def_id, .. } => type_name_by_def_id
                .get(def_id)
                .and_then(|s| interner.try_resolve(s))
                .map(String::from)
                .unwrap_or_else(|| format!("Adt_{}", def_id.as_u32())),
            MonoItem::Fn { def_id, .. } => fn_name_by_def_id
                .get(def_id)
                .map(|name| name.strip_prefix("landin_").unwrap_or(name).to_string())
                .unwrap_or_else(|| format!("fn_{}", def_id.as_u32())),
            MonoItem::Closure { def_id, .. } => {
                format!("closure_{}", def_id.as_u32())
            }
        };
        let specialized = mono_item_name(item, &base_name, type_name_by_def_id, interner);
        map.insert(item.clone(), specialized);
    }
    map
}

// =====================================================================
// Stage 16.57 (Task 11 Phase 4b): Per-mono layouts
// =====================================================================

/// A hashable key for per-mono layouts.
///
/// Wraps `(DefId, Vec<TyKind>)` — the DefId of the generic type plus the
/// TyKind values of its substs. Uses TyKind (not Ty) because Ty doesn't
/// implement Hash/Eq (it's interned), while TyKind derives them.
///
/// Two MonoLayoutKeys are equal iff they have the same DefId and the same
/// substs (element-wise TyKind comparison). This ensures `Box<i32>` and
/// `Box<i32>` map to the same layout, while `Box<i32>` and `Box<bool>` map
/// to different layouts.
///
/// Per §23: `MonoLayoutKey` follows `<Noun>_<Noun>_<Noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoLayoutKey {
    pub def_id: DefId,
    pub substs: Vec<TyKind>,
}

impl MonoLayoutKey {
    /// Create a MonoLayoutKey from a DefId and substs slice.
    ///
    /// Extracts the TyKind from each Ty in the substs.
    pub fn new(def_id: DefId, substs: &crate::mir::ty::SubstsRef) -> Self {
        let substs = substs.iter().map(|t| t.kind.clone()).collect();
        MonoLayoutKey { def_id, substs }
    }

    /// Create a MonoLayoutKey from a MonoItem.
    pub fn from_mono_item(item: &MonoItem) -> Self {
        match item {
            MonoItem::Type { def_id, substs }
            | MonoItem::Fn { def_id, substs }
            | MonoItem::Closure { def_id, substs } => Self::new(*def_id, substs),
        }
    }
}

/// A map from MonoLayoutKey to AdtLayout.
///
/// Each entry represents one specialized layout for a generic type
/// instantiation. For example, `Box<i32>` and `Box<bool>` have distinct
/// entries because their field types differ (i32 vs bool).
///
/// Built by `build_mono_layouts` from collected MonoItems. The layouts use
/// substituted field types — e.g., for `struct Box<T> { val: T }` with
/// substs `[i32]`, the field type is `i32` (not `Param(T)` or `Error`).
///
/// Per §23: `MonoLayoutMap` follows `<Noun>_<Noun>_<Noun>` pattern.
pub type MonoLayoutMap = std::collections::HashMap<MonoLayoutKey, crate::mir::body::AdtLayout>;

/// Build per-mono layouts for all Type MonoItems.
///
/// For each `MonoItem::Type { def_id, substs }`:
/// 1. Get the generic params via `generics_of(def_id, hir)`
/// 2. Lower each field type with `lower_hir_ty_to_mir_ty_with_generics`
///    (resolves type params to `Param`)
/// 3. Apply `substitute(field_ty, substs)` to replace `Param` with actual types
/// 4. Build an `AdtLayout` with the substituted field types
/// 5. Insert into the map keyed by `MonoLayoutKey`
///
/// Non-generic types (empty substs) are skipped — they use the existing
/// `AdtLayouts` map (keyed by DefId only). Only generic instantiations
/// get per-mono layouts.
///
/// Per §23: `build_mono_layouts` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads HIR + MIR (allowed during layout building).
/// Per §1.0 原則 6 "通用 > 特例": one function for struct + enum layouts.
pub fn build_mono_layouts(items: &[MonoItem], hir: &crate::hir::HirCrate) -> MonoLayoutMap {
    use crate::hir::{HirItem, OwnerNode};
    use crate::mir::body::AdtLayout;
    use crate::mir::ty::TyKind as Tk;
    use crate::session::Span;

    let mut map = MonoLayoutMap::new();

    for item in items {
        // Only build layouts for Type MonoItems with non-empty substs.
        let (def_id, substs) = match item {
            MonoItem::Type { def_id, substs } if !substs.is_empty() => (*def_id, substs.clone()),
            _ => continue,
        };

        // Skip if already built (dedup by key).
        let key = MonoLayoutKey::new(def_id, &substs);
        if map.contains_key(&key) {
            continue;
        }

        // Get the HIR owner for this DefId.
        let owner = match hir.owner(def_id) {
            Some(o) => o,
            None => continue,
        };

        // Get generic params for this type.
        let generic_params = crate::hir::generics::generics_of(def_id, hir);

        match owner {
            OwnerNode::Item(HirItem::Struct(s)) => {
                // Lower each field type with generics, then substitute.
                let field_tys: Vec<crate::mir::ty::Ty> = s
                    .fields
                    .iter()
                    .map(|f| {
                        let field_ty = crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                            &f.ty,
                            &generic_params,
                        );
                        crate::mir::substitute::substitute(&field_ty, &substs)
                    })
                    .collect();
                map.insert(key, AdtLayout::Struct { field_tys });
            }
            OwnerNode::Item(HirItem::Enum(e)) => {
                let discriminant_ty =
                    crate::mir::ty::Ty::new(Tk::Int(crate::ast::IntTy::I32), Span::DUMMY);
                let variant_payloads: Vec<Vec<crate::mir::ty::Ty>> = e
                    .variants
                    .iter()
                    .map(|variant| match &variant.data {
                        crate::hir::HirVariantData::Unit(_) => Vec::new(),
                        crate::hir::HirVariantData::Tuple(fields, _) => fields
                            .iter()
                            .map(|f| {
                                let field_ty =
                                    crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                                        &f.ty,
                                        &generic_params,
                                    );
                                crate::mir::substitute::substitute(&field_ty, &substs)
                            })
                            .collect(),
                        crate::hir::HirVariantData::Struct(fields, _) => fields
                            .iter()
                            .map(|f| {
                                let field_ty =
                                    crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                                        &f.ty,
                                        &generic_params,
                                    );
                                crate::mir::substitute::substitute(&field_ty, &substs)
                            })
                            .collect(),
                    })
                    .collect();
                map.insert(
                    key,
                    AdtLayout::Enum {
                        discriminant_ty,
                        variant_payloads,
                    },
                );
            }
            _ => {}
        }
    }

    map
}

/// Stage 16.58 (Task 11 Phase 4c): Look up a per-mono layout for a Ty.
///
/// Given a `TyKind::Adt(def_id, substs)` and an optional `MonoLayoutMap`,
/// returns the specialized `AdtLayout` if one exists for this instantiation.
/// Returns `None` if:
/// - `mono_layouts` is `None` (not built)
/// - `substs` is empty (non-generic — use the existing AdtLayouts map)
/// - No layout was built for this (def_id, substs) pair
///
/// This is the codegen integration point — codegen calls this first for
/// Adt types, falling back to `AdtLayouts` when it returns `None`.
///
/// Per §23: `lookup_mono_layout` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads MonoLayoutMap (built from MIR + HIR, no HIR at lookup time).
pub fn lookup_mono_layout<'a>(
    def_id: DefId,
    substs: &crate::mir::ty::SubstsRef,
    mono_layouts: Option<&'a MonoLayoutMap>,
) -> Option<&'a crate::mir::body::AdtLayout> {
    let map = mono_layouts?;
    if substs.is_empty() {
        return None;
    }
    let key = MonoLayoutKey::new(def_id, substs);
    map.get(&key)
}

// =====================================================================
// Unit Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::ast::UintTy;
    use crate::compile;
    use crate::mir::body::{MirBody, Statement, Terminator};
    use crate::mir::place::{LocalId, Place};
    use crate::mir::ty::{ConstVal, Mutability, Region};
    use crate::mir::LocalDecl;
    use crate::session::Span;

    /// Helper: create a Ty of the given kind.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    /// Helper: create an i32 Ty.
    fn i32_ty() -> Ty {
        ty(TyKind::Int(IntTy::I32))
    }

    /// Helper: create a bool Ty.
    fn bool_ty() -> Ty {
        ty(TyKind::Bool)
    }

    /// Helper: create an Adt Ty with substs.
    fn adt_ty(def_id: u32, substs: Vec<Ty>) -> Ty {
        ty(TyKind::Adt(DefId::new(def_id), substs.into()))
    }

    /// Helper: create an empty MirBody.
    fn empty_mir() -> MirBody {
        MirBody::new(Span::DUMMY)
    }

    // =================================================================
    // §1. MonoItem struct tests
    // =================================================================

    #[test]
    fn stage16_54_mono_item_type_def_id() {
        let item = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(1));
    }

    #[test]
    fn stage16_54_mono_item_fn_def_id() {
        let item = MonoItem::Fn {
            def_id: DefId::new(2),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(2));
    }

    #[test]
    fn stage16_54_mono_item_closure_def_id() {
        let item = MonoItem::Closure {
            def_id: DefId::new(3),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(3));
    }

    #[test]
    fn stage16_54_mono_item_equality() {
        let item1 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let item2 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item1, item2);
    }

    #[test]
    fn stage16_54_mono_item_inequality_different_substs() {
        let item1 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let item2 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![bool_ty()].into(),
        };
        assert_ne!(item1, item2);
    }

    // =================================================================
    // §2. collect_from_ty tests
    // =================================================================

    #[test]
    fn stage16_54_collect_from_adt_with_substs() {
        let mut collected = HashSet::new();
        let t = adt_ty(1, vec![i32_ty()]);
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected.iter().any(|m| matches!(
            m,
            MonoItem::Type { def_id, substs } if *def_id == DefId::new(1) && substs.len() == 1
        )));
    }

    #[test]
    fn stage16_54_collect_from_adt_empty_substs() {
        let mut collected = HashSet::new();
        let t = adt_ty(1, vec![]);
        collect_from_ty(&t, &mut collected);
        assert!(collected.is_empty());
    }

    #[test]
    fn stage16_54_collect_from_nested_adt() {
        let mut collected = HashSet::new();
        // Vec<Vec<i32>> — outer Adt(1, [Adt(1, [i32])])
        let inner = adt_ty(1, vec![i32_ty()]);
        let outer = adt_ty(1, vec![inner]);
        collect_from_ty(&outer, &mut collected);
        // Should collect both outer and inner
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn stage16_54_collect_from_ref_adt() {
        let mut collected = HashSet::new();
        let inner = adt_ty(1, vec![i32_ty()]);
        let ref_ty = ty(TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(inner),
        ));
        collect_from_ty(&ref_ty, &mut collected);
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn stage16_54_collect_from_tuple_of_adts() {
        let mut collected = HashSet::new();
        let adt1 = adt_ty(1, vec![i32_ty()]);
        let adt2 = adt_ty(2, vec![bool_ty()]);
        let tuple = ty(TyKind::Tuple(vec![adt1, adt2]));
        collect_from_ty(&tuple, &mut collected);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn stage16_54_collect_from_fn_def() {
        let mut collected = HashSet::new();
        let t = ty(TyKind::FnDef(DefId::new(5), vec![i32_ty()].into()));
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected.iter().any(|m| matches!(m, MonoItem::Fn { .. })));
    }

    #[test]
    fn stage16_54_collect_from_closure() {
        let mut collected = HashSet::new();
        let t = ty(TyKind::Closure(DefId::new(7), vec![i32_ty()].into()));
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected
            .iter()
            .any(|m| matches!(m, MonoItem::Closure { .. })));
    }

    #[test]
    fn stage16_54_collect_from_leaf_types() {
        let mut collected = HashSet::new();
        collect_from_ty(&i32_ty(), &mut collected);
        collect_from_ty(&bool_ty(), &mut collected);
        collect_from_ty(&ty(TyKind::Str), &mut collected);
        collect_from_ty(&ty(TyKind::Never), &mut collected);
        collect_from_ty(&ty(TyKind::Error), &mut collected);
        assert!(collected.is_empty());
    }

    // =================================================================
    // §3. collect_mono_items tests
    // =================================================================

    #[test]
    fn stage16_54_collect_empty_mirs() {
        let items = collect_mono_items(&[]);
        assert!(items.is_empty());
    }

    #[test]
    fn stage16_54_collect_from_local_decls() {
        let mut mir = empty_mir();
        // Add two locals with Adt types.
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(2, vec![bool_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn stage16_54_collect_dedup() {
        let mut mir = empty_mir();
        // Add two locals with the SAME Adt type — should dedup to 1.
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn stage16_54_collect_multiple_mirs() {
        let mut mir1 = empty_mir();
        mir1.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        let mut mir2 = empty_mir();
        mir2.local_decls.push(LocalDecl {
            ty: adt_ty(2, vec![bool_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(&[mir1, mir2]);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn stage16_54_collect_across_mirs_dedup() {
        let mut mir1 = empty_mir();
        mir1.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        let mut mir2 = empty_mir();
        mir2.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(&[mir1, mir2]);
        assert_eq!(items.len(), 1);
    }

    // =================================================================
    // §4. collect_from_statement / rvalue / terminator tests
    // =================================================================

    #[test]
    fn stage16_54_collect_from_aggregate_statement() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a statement: Assign(local_0, Aggregate(Adt(1, [i32]), []))
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(
                        AggregateKind::Adt(DefId::new(1), 0, vec![i32_ty()].into(), vec![]),
                        vec![],
                    ),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
        assert!(items.iter().any(|m| matches!(
            m,
            MonoItem::Type { def_id, .. } if *def_id == DefId::new(1)
        )));
    }

    #[test]
    fn stage16_54_collect_from_cast_statement() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        let const_val = crate::mir::ty::Const {
            ty: i32_ty(),
            val: ConstVal::Int(42),
        };
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Cast(
                        crate::mir::place::CastKind::Numeric,
                        Operand::Constant(const_val),
                        adt_ty(1, vec![i32_ty()]),
                    ),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        // Should collect from both the operand's type (i32 — leaf, no item)
        // and the cast target type (Adt(1, [i32])).
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn stage16_54_collect_from_call_terminator() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a Call terminator with a FnDef func.
        let func_ty = ty(TyKind::FnDef(DefId::new(5), vec![i32_ty()].into()));
        let func_operand = Operand::Constant(crate::mir::ty::Const {
            ty: func_ty,
            val: ConstVal::Uint(5),
        });
        mir.basic_blocks[block_id.0 as usize].terminator = Terminator {
            kind: TerminatorKind::Call {
                func: func_operand,
                args: vec![],
                destination: Place::local(LocalId(0), Span::DUMMY),
                target: None,
                dyn_trait_call: None,
            },
            span: Span::DUMMY,
        };

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
        assert!(items.iter().any(|m| matches!(m, MonoItem::Fn { .. })));
    }

    #[test]
    fn stage16_54_collect_from_array_aggregate() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a statement: Assign(local_0, Aggregate(Array(Adt(1, [i32])), []))
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(AggregateKind::Array(adt_ty(1, vec![i32_ty()])), vec![]),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
    }

    // =================================================================
    // §5. Complex scenarios
    // =================================================================

    #[test]
    fn stage16_54_collect_mixed_types() {
        let mut mir = empty_mir();
        // Local 1: Adt(1, [i32])
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        // Local 2: Adt(2, [bool, Adt(1, [u64])])  — nested
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(
                2,
                vec![bool_ty(), adt_ty(1, vec![ty(TyKind::Uint(UintTy::U64))])],
            ),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        // Should collect: Adt(1, [i32]), Adt(2, [bool, Adt(1, [u64])]), Adt(1, [u64])
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn stage16_54_debug_string() {
        let item = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let s = item.debug_string();
        assert!(s.contains("Type"));
        assert!(s.contains("DefId"));
    }

    // =================================================================
    // §6. mangle_ty tests (Stage 16.55, Phase 4)
    // =================================================================

    #[test]
    fn stage16_55_mangle_ty_bool() {
        assert_eq!(mangle_ty(&bool_ty()), "bool");
    }

    #[test]
    fn stage16_55_mangle_ty_i32() {
        assert_eq!(mangle_ty(&i32_ty()), "i32");
    }

    #[test]
    fn stage16_55_mangle_ty_adt_with_substs() {
        let t = adt_ty(5, vec![i32_ty()]);
        // Without interner, Adt uses DefId fallback
        assert_eq!(mangle_ty(&t), "Adt_5_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_adt_empty_substs() {
        let t = adt_ty(5, vec![]);
        assert_eq!(mangle_ty(&t), "Adt_5");
    }

    #[test]
    fn stage16_55_mangle_ty_ref() {
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(i32_ty()),
        ));
        assert_eq!(mangle_ty(&t), "ref_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_ref_mut() {
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Mutable,
            Box::new(i32_ty()),
        ));
        assert_eq!(mangle_ty(&t), "refmut_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_tuple() {
        let t = ty(TyKind::Tuple(vec![i32_ty(), bool_ty()]));
        assert_eq!(mangle_ty(&t), "tuple_i32_bool");
    }

    #[test]
    fn stage16_55_mangle_ty_empty_tuple() {
        let t = ty(TyKind::Tuple(vec![]));
        assert_eq!(mangle_ty(&t), "unit");
    }

    #[test]
    fn stage16_55_mangle_ty_array() {
        let len = crate::mir::ty::Const {
            ty: i32_ty(),
            val: ConstVal::Uint(10),
        };
        let t = ty(TyKind::Array(Box::new(i32_ty()), Box::new(len)));
        assert_eq!(mangle_ty(&t), "array_i32_10");
    }

    #[test]
    fn stage16_55_mangle_ty_slice() {
        let t = ty(TyKind::Slice(Box::new(i32_ty())));
        assert_eq!(mangle_ty(&t), "slice_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_nested_adt() {
        let inner = adt_ty(1, vec![i32_ty()]);
        let outer = adt_ty(2, vec![inner]);
        assert_eq!(mangle_ty(&outer), "Adt_2_Adt_1_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_fn_def() {
        let t = ty(TyKind::FnDef(DefId::new(7), vec![i32_ty()].into()));
        assert_eq!(mangle_ty(&t), "fn_7_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_closure() {
        let t = ty(TyKind::Closure(DefId::new(3), vec![i32_ty()].into()));
        assert_eq!(mangle_ty(&t), "closure_3_i32");
    }

    #[test]
    fn stage16_55_mangle_ty_param() {
        use crate::mir::ty::ParamTy;
        let t = ty(TyKind::Param(ParamTy {
            index: 0,
            name: crate::lexer::Symbol::default(),
        }));
        assert_eq!(mangle_ty(&t), "param_0");
    }

    #[test]
    fn stage16_55_mangle_ty_str() {
        let t = ty(TyKind::Str);
        assert_eq!(mangle_ty(&t), "str");
    }

    #[test]
    fn stage16_55_mangle_ty_never() {
        let t = ty(TyKind::Never);
        assert_eq!(mangle_ty(&t), "never");
    }

    // =================================================================
    // §7. mono_item_name tests (Stage 16.55, Phase 4)
    // =================================================================

    #[test]
    fn stage16_55_mono_item_name_type_with_substs() {
        let map: std::collections::HashMap<DefId, crate::lexer::Symbol> =
            std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();
        let item = MonoItem::Type {
            def_id: DefId::new(5),
            substs: vec![i32_ty()].into(),
        };
        let name = mono_item_name(&item, "Box", &map, &interner);
        assert_eq!(name, "Box_i32");
    }

    #[test]
    fn stage16_55_mono_item_name_fn_with_substs() {
        let map: std::collections::HashMap<DefId, crate::lexer::Symbol> =
            std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();
        let item = MonoItem::Fn {
            def_id: DefId::new(7),
            substs: vec![i32_ty()].into(),
        };
        let name = mono_item_name(&item, "id", &map, &interner);
        assert_eq!(name, "id_i32");
    }

    #[test]
    fn stage16_55_mono_item_name_empty_substs() {
        let map: std::collections::HashMap<DefId, crate::lexer::Symbol> =
            std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();
        let item = MonoItem::Type {
            def_id: DefId::new(5),
            substs: vec![].into(),
        };
        let name = mono_item_name(&item, "Box", &map, &interner);
        assert_eq!(name, "Box");
    }

    #[test]
    fn stage16_55_mono_item_name_multiple_substs() {
        let map: std::collections::HashMap<DefId, crate::lexer::Symbol> =
            std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();
        let item = MonoItem::Type {
            def_id: DefId::new(5),
            substs: vec![i32_ty(), bool_ty()].into(),
        };
        let name = mono_item_name(&item, "Pair", &map, &interner);
        assert_eq!(name, "Pair_i32_bool");
    }

    #[test]
    fn stage16_55_mono_item_name_nested_substs() {
        let map: std::collections::HashMap<DefId, crate::lexer::Symbol> =
            std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();
        let inner = adt_ty(1, vec![i32_ty()]);
        let item = MonoItem::Type {
            def_id: DefId::new(2),
            substs: vec![inner].into(),
        };
        let name = mono_item_name(&item, "Outer", &map, &interner);
        // Inner Adt uses DefId fallback (no interner resolution for type names)
        assert_eq!(name, "Outer_Adt_1_i32");
    }

    // =================================================================
    // §8. build_mono_item_names tests (Stage 16.55, Phase 4)
    // =================================================================

    #[test]
    fn stage16_55_build_mono_item_names_basic() {
        let mut fn_map = std::collections::HashMap::new();
        fn_map.insert(DefId::new(7), "landin_id".to_string());
        let type_map = std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();

        let items = vec![
            MonoItem::Fn {
                def_id: DefId::new(7),
                substs: vec![i32_ty()].into(),
            },
            MonoItem::Fn {
                def_id: DefId::new(7),
                substs: vec![bool_ty()].into(),
            },
        ];

        let names = build_mono_item_names(&items, &fn_map, &type_map, &interner);
        assert_eq!(names.len(), 2);
        assert_eq!(names.get(&items[0]), Some(&"id_i32".to_string()));
        assert_eq!(names.get(&items[1]), Some(&"id_bool".to_string()));
    }

    #[test]
    fn stage16_55_build_mono_item_names_empty() {
        let fn_map = std::collections::HashMap::new();
        let type_map = std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();

        let names = build_mono_item_names(&[], &fn_map, &type_map, &interner);
        assert!(names.is_empty());
    }

    #[test]
    fn stage16_55_build_mono_item_names_mixed() {
        let mut fn_map = std::collections::HashMap::new();
        fn_map.insert(DefId::new(7), "landin_id".to_string());
        let type_map = std::collections::HashMap::new();
        let interner = lasso::Rodeo::new();

        let items = vec![
            MonoItem::Fn {
                def_id: DefId::new(7),
                substs: vec![i32_ty()].into(),
            },
            MonoItem::Type {
                def_id: DefId::new(5),
                substs: vec![i32_ty()].into(),
            },
            MonoItem::Closure {
                def_id: DefId::new(3),
                substs: vec![i32_ty()].into(),
            },
        ];

        let names = build_mono_item_names(&items, &fn_map, &type_map, &interner);
        assert_eq!(names.len(), 3);
        // Fn: id_i32
        assert_eq!(names.get(&items[0]), Some(&"id_i32".to_string()));
        // Type: Adt_5_i32 (no type name in map, fallback to DefId)
        assert_eq!(names.get(&items[1]), Some(&"Adt_5_i32".to_string()));
        // Closure: closure_3_i32
        assert_eq!(names.get(&items[2]), Some(&"closure_3_i32".to_string()));
    }

    // =================================================================
    // §9. MonoLayoutKey tests (Stage 16.57, Phase 4b)
    // =================================================================

    #[test]
    fn stage16_57_mono_layout_key_new() {
        let substs: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key = MonoLayoutKey::new(DefId::new(1), &substs);
        assert_eq!(key.def_id, DefId::new(1));
        assert_eq!(key.substs.len(), 1);
        assert_eq!(key.substs[0], TyKind::Int(IntTy::I32));
    }

    #[test]
    fn stage16_57_mono_layout_key_empty_substs() {
        let substs: crate::mir::ty::SubstsRef = vec![].into();
        let key = MonoLayoutKey::new(DefId::new(2), &substs);
        assert_eq!(key.def_id, DefId::new(2));
        assert!(key.substs.is_empty());
    }

    #[test]
    fn stage16_57_mono_layout_key_equality() {
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        assert_eq!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_inequality_different_def_id() {
        let substs: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs);
        let key2 = MonoLayoutKey::new(DefId::new(2), &substs);
        assert_ne!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_inequality_different_substs() {
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![bool_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_from_mono_item_type() {
        let item = MonoItem::Type {
            def_id: DefId::new(5),
            substs: vec![i32_ty()].into(),
        };
        let key = MonoLayoutKey::from_mono_item(&item);
        assert_eq!(key.def_id, DefId::new(5));
        assert_eq!(key.substs.len(), 1);
    }

    #[test]
    fn stage16_57_mono_layout_key_from_mono_item_fn() {
        let item = MonoItem::Fn {
            def_id: DefId::new(7),
            substs: vec![bool_ty()].into(),
        };
        let key = MonoLayoutKey::from_mono_item(&item);
        assert_eq!(key.def_id, DefId::new(7));
        assert_eq!(key.substs.len(), 1);
        assert_eq!(key.substs[0], TyKind::Bool);
    }

    #[test]
    fn stage16_57_mono_layout_key_hashable() {
        use std::collections::HashSet;
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        let mut set = HashSet::new();
        set.insert(key1);
        set.insert(key2);
        assert_eq!(set.len(), 1); // dedup
    }

    // =================================================================
    // §10. build_mono_layouts tests (Stage 16.57, Phase 4b)
    // =================================================================

    #[test]
    fn stage16_57_build_mono_layouts_empty_items() {
        let result = compile("fn main() { 0 }");
        let hir = result.hir.as_ref().expect("HIR should be available");
        let layouts = build_mono_layouts(&[], hir);
        assert!(layouts.is_empty());
    }

    #[test]
    fn stage16_57_build_mono_layouts_non_generic_skipped() {
        // Non-generic MonoItems (empty substs) are skipped.
        let result = compile("fn main() { 0 }");
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = vec![MonoItem::Type {
            def_id: DefId::new(0),
            substs: vec![].into(),
        }];
        let layouts = build_mono_layouts(&items, hir);
        assert!(layouts.is_empty());
    }

    #[test]
    fn stage16_57_build_mono_layouts_generic_struct() {
        let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have at least 1 layout (Box<i32>)
        assert!(
            !layouts.is_empty(),
            "Expected at least 1 mono layout, got: {:?}",
            layouts
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_two_instantiations() {
        let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<bool> = Box { val: true }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 2 layouts (Box<i32> and Box<bool>)
        assert_eq!(
            layouts.len(),
            2,
            "Expected exactly 2 mono layouts (Box<i32> + Box<bool>), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_dedup() {
        let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<i32> = Box { val: 43 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 1 layout (Box<i32> deduped)
        assert_eq!(
            layouts.len(),
            1,
            "Expected exactly 1 mono layout (Box<i32> deduped), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_nested_generic() {
        let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 2 layouts (Box<Box<i32>> and Box<i32>)
        assert!(
            layouts.len() >= 2,
            "Expected at least 2 mono layouts (nested Box), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_correct_field_type() {
        use crate::mir::body::AdtLayout;
        let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Find the Box<i32> layout and verify its field type is i32 (not Param or Error)
        let has_i32_field = layouts.values().any(|layout| match layout {
            AdtLayout::Struct { field_tys } => {
                field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Int(IntTy::I32))
            }
            _ => false,
        });
        assert!(
            has_i32_field,
            "Expected a struct layout with i32 field type (substituted), got: {:?}",
            layouts
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_generic_enum() {
        use crate::mir::body::AdtLayout;
        let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::Some(42); }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have at least 1 layout (Opt<i32>)
        let has_enum_layout = layouts
            .values()
            .any(|layout| matches!(layout, AdtLayout::Enum { .. }));
        assert!(
            has_enum_layout,
            "Expected at least 1 enum layout (Opt<i32>), got: {:?}",
            layouts
        );
    }
}
