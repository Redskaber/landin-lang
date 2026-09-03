//! Stage 16.55 (Task 11 Phase 4a): Per-mono codegen — specialized naming.
//!
//! Provides `mangle_ty`, `mono_item_name`, and `build_mono_item_names` for
//! generating specialized LLVM symbol names for monomorphized items.
//!
//! Per §23: all functions follow `<verb>_<noun>` or `<verb>_<noun>_<noun>` patterns.
//! Per §16: reads Ty + optional name maps (no HIR access).

use super::item::MonoItem;
use crate::hir::DefId;
use crate::mir::ty::{Ty, TyKind};

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
        // Stage 87 (v0.8 — TD-DYN-TRAIT-COMPLETION): `dyn Trait` mangled
        // as `dyn_<def_id>` — used in mono item names. The trait DefId
        // distinguishes `dyn Greeter` from `dyn Display`.
        TyKind::Dyn(def_id) => format!("dyn_{}", def_id.as_u32()),
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
            let base_name = format!("Adt_{}", def_id.as_u32());
            if substs.is_empty() {
                base_name
            } else {
                let substs_str: Vec<String> = substs.iter().map(mangle_ty).collect();
                format!("{}_{}", base_name, substs_str.join("_"))
            }
        }
        TyKind::Projection(def_id, substs) => {
            let base_name = format!("Proj_{}", def_id.as_u32());
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

// =================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::hir::DefId;
    use crate::mir::ty::{ConstVal, Mutability, Region, Ty, TyKind};
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
}
