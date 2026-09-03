//! Stage 68 (v0.8 — TD-IMPL-TRAIT-MONO-RESOLUTION): Trait method resolution map.
//!
//! Pre-computes a map from (trait_method_def_id, type_name) → concrete impl
//! method DefId. This map is built by the driver (after trait resolution,
//! before codegen) and passed as data to `codegen_mono_functions`.
//!
//! Per §16 (codegen is HIR-free): the map is pre-computed in the driver
//! and passed as data — codegen never touches HIR or TraitResolver.
//! Per §12 (最优 > 最小): root-cause fix — re-resolve trait methods during
//! monomorphization using pre-computed data.
//! Per §1.0 原則 6 (通解 > 特解): one map for all trait method resolutions.

use crate::hir::{DefId, HirCrate, HirItem, OwnerNode, Res};
use crate::lexer::Symbol;
use crate::traits::TraitResolver;
use lasso::Rodeo;
use std::collections::HashMap;

/// Maps a (trait_method_def_id, type_name) pair to the concrete impl
/// method's DefId. Built by the driver after trait resolution, before
/// codegen.
///
/// Example: (Clone::clone DefId, "i32") → i32::clone impl method DefId
///
/// Per §1.0 原則 10 (唯一可信数据源): single source of truth for trait
/// method resolution during monomorphization.
#[derive(Debug, Clone, Default)]
pub struct TraitMethodResolutionMap {
    /// Key: (trait_method_def_id, type_name_string)
    /// Value: concrete impl method DefId
    map: HashMap<(DefId, String), DefId>,
}

impl TraitMethodResolutionMap {
    /// Look up the concrete impl method DefId for a given trait method
    /// and concrete type name.
    ///
    /// Returns `None` if no impl exists for this (trait, type) pair.
    pub fn lookup(&self, trait_method_def_id: DefId, type_name: &str) -> Option<DefId> {
        self.map
            .get(&(trait_method_def_id, type_name.to_string()))
            .copied()
    }

    /// Insert a mapping.
    pub fn insert(
        &mut self,
        trait_method_def_id: DefId,
        type_name: String,
        impl_method_def_id: DefId,
    ) {
        self.map
            .insert((trait_method_def_id, type_name), impl_method_def_id);
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Is the map empty?
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Build the `TraitMethodResolutionMap` by scanning all trait declarations
/// and their impls in the HIR.
///
/// For each trait `trait Foo { fn method(&self); }` and each
/// `impl Foo for Type { fn method(&self) { ... } }`, map:
///   (trait_method_def_id, type_name) → impl_method_def_id
///
/// Per §12 (最优 > 最小): pre-compute in driver, pass as data to codegen.
/// Per §1.0 原則 6 (通解 > 特解): one pass for all traits + all impls.
pub fn build_trait_method_resolution_map(
    hir: &HirCrate,
    interner: &Rodeo,
    _trait_resolver: &TraitResolver,
) -> TraitMethodResolutionMap {
    let mut map = TraitMethodResolutionMap::default();

    // Step 1: Collect all trait declarations and their method DefIds.
    // Key: trait_name_spur → Vec<(method_name_spur, method_def_id)>
    let mut trait_methods: HashMap<Symbol, Vec<(Symbol, DefId)>> = HashMap::new();
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Trait(t)) = owner {
            let trait_name = t.ident.name;
            let mut methods = Vec::new();
            for item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = item {
                    methods.push((f.ident.name, f.hir_id.owner));
                }
            }
            trait_methods.insert(trait_name, methods);
        }
    }

    // Step 2: For each impl block `impl Trait for Type { ... }`, map each
    // method to (trait_method_def_id, type_name) → impl_method_def_id.
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Impl(impl_block)) = owner {
            // Only process trait impls (not inherent impls).
            if let Some(trait_path) = &impl_block.of_trait {
                // Get the trait name (last segment of the path).
                if let Some(trait_seg) = trait_path.segments.last() {
                    let trait_name = trait_seg.ident.name;

                    // Get the self type name.
                    let type_name = get_type_name(&impl_block.self_ty, interner);

                    // Look up the trait's method DefIds.
                    if let Some(methods) = trait_methods.get(&trait_name) {
                        for (method_name, trait_method_def_id) in methods {
                            // Find the corresponding impl method.
                            for impl_item in &impl_block.items {
                                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                                    if f.ident.name == *method_name {
                                        map.insert(
                                            *trait_method_def_id,
                                            type_name.clone(),
                                            f.hir_id.owner,
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

    map
}

/// Get the source-language name of a HIR type as a string.
///
/// For ADT types: looks up the struct/enum name.
/// For primitive types: returns the primitive name ("i32", "bool", etc.).
/// For other types: returns empty string (no trait method resolution).
///
/// Per §1.0 原則 6 (通解 > 特解): one function for all type kinds.
fn get_type_name(ty: &crate::hir::HirTy, interner: &Rodeo) -> String {
    use crate::hir::HirTyKind;
    match &ty.kind {
        HirTyKind::Path(_, path) => {
            // Single-segment path: resolve via Res.
            if let Some(seg) = path.segments.last() {
                match &path.res {
                    Res::Def(_def_id, _) => {
                        // Look up the struct/enum name from HIR.
                        // For now, use the segment's ident name.
                        interner.resolve(&seg.ident.name).to_string()
                    }
                    Res::PrimTy(prim) => format!("{:?}", prim).to_lowercase(),
                    _ => interner.resolve(&seg.ident.name).to_string(),
                }
            } else {
                String::new()
            }
        }
        HirTyKind::Int(int_ty) => {
            use crate::ast::IntTy;
            match int_ty {
                IntTy::I8 => "i8",
                IntTy::I16 => "i16",
                IntTy::I32 => "i32",
                IntTy::I64 => "i64",
                IntTy::I128 => "i128",
                IntTy::Isize => "isize",
            }
            .to_string()
        }
        HirTyKind::Uint(uint_ty) => {
            use crate::ast::UintTy;
            match uint_ty {
                UintTy::U8 => "u8",
                UintTy::U16 => "u16",
                UintTy::U32 => "u32",
                UintTy::U64 => "u64",
                UintTy::U128 => "u128",
                UintTy::Usize => "usize",
            }
            .to_string()
        }
        HirTyKind::Bool => "bool".to_string(),
        HirTyKind::Char => "char".to_string(),
        HirTyKind::Float(float_ty) => {
            use crate::ast::FloatTy;
            match float_ty {
                FloatTy::F32 => "f32",
                FloatTy::F64 => "f64",
            }
            .to_string()
        }
        _ => {
            // For Path types (including str, which parses as Path),
            // fall through to the Path arm above (already handled).
            // For other types (Tuple, Array, Ref, etc.), return empty.
            String::new()
        }
    }
}
