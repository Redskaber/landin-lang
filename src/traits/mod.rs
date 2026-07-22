//! Trait resolution: collect trait definitions + impl blocks + build dispatch tables.
//!
//! Stage 5.1: Basic TraitResolver — collects trait/impl metadata from HIR.
//!
//! Per §16 (阶段间接口隔离): TraitResolver reads HIR during the driver's
//! pre-computation phase, then provides data to typeck/borrowck/codegen.

use crate::hir::*;
use lasso::Rodeo;
use lasso::Spur;
use std::collections::HashMap;

/// A trait definition collected by TraitResolver.
#[derive(Debug, Clone)]
pub struct TraitInfo {
    /// DefId of the trait item.
    pub def_id: DefId,
    /// Trait name (interned symbol).
    pub name: Spur,
    /// Method names defined in the trait (interned symbols).
    pub methods: Vec<Spur>,
    /// Whether this is an unsafe trait.
    pub is_unsafe: bool,
}

/// An impl block collected by TraitResolver.
#[derive(Debug, Clone)]
pub struct ImplInfo {
    /// DefId of the impl block.
    pub def_id: DefId,
    /// The trait being implemented (None for inherent impl).
    pub trait_name: Option<Spur>,
    /// The self type name (best-effort — from HirTy path).
    pub self_ty_name: Option<Spur>,
    /// Method names implemented in this impl block.
    pub methods: Vec<Spur>,
    /// Whether this is an unsafe impl.
    pub is_unsafe: bool,
}

/// The TraitResolver. Collects trait definitions and impl blocks from HIR,
/// builds dispatch tables for method resolution.
///
/// Per §16: This is built by the driver during pre-computation, then passed
/// as data to typeck/borrowck/codegen (no HIR access needed downstream).
#[derive(Debug, Default)]
pub struct TraitResolver {
    /// All trait definitions: DefId → TraitInfo.
    pub traits: HashMap<DefId, TraitInfo>,
    /// All impl blocks: DefId → ImplInfo.
    pub impls: HashMap<DefId, ImplInfo>,
    /// Trait name → DefId (for looking up traits by name).
    pub trait_by_name: HashMap<Spur, DefId>,
    /// (trait_name, self_ty_name) → impl DefId (for impl lookup).
    pub impl_by_trait_and_type: HashMap<(Spur, Spur), DefId>,
}

impl TraitResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect all trait definitions and impl blocks from HIR.
    pub fn collect(&mut self, hir: &HirCrate, _interner: &Rodeo) {
        for (def_id, node) in &hir.owners {
            if let OwnerNode::Item(item) = node {
                match item {
                    HirItem::Trait(t) => {
                        let mut methods = Vec::new();
                        for trait_item in &t.items {
                            if let HirTraitItem::Fn(f) = trait_item {
                                methods.push(f.ident.name);
                            }
                        }
                        let info = TraitInfo {
                            def_id: *def_id,
                            name: t.ident.name,
                            methods,
                            is_unsafe: t.is_unsafe,
                        };
                        self.trait_by_name.insert(t.ident.name, *def_id);
                        self.traits.insert(*def_id, info);
                    }
                    HirItem::Impl(i) => {
                        let trait_name =
                            i.of_trait
                                .as_ref()
                                .and_then(|p| p.segments.last().map(|s| s.ident.name));
                        let self_ty_name = extract_ty_name(&i.self_ty);
                        let mut methods = Vec::new();
                        for impl_item in &i.items {
                            if let HirImplItem::Fn(f) = impl_item {
                                methods.push(f.ident.name);
                            }
                        }
                        let info = ImplInfo {
                            def_id: *def_id,
                            trait_name,
                            self_ty_name,
                            methods,
                            is_unsafe: i.is_unsafe,
                        };
                        if let (Some(tn), Some(stn)) = (trait_name, self_ty_name) {
                            self.impl_by_trait_and_type
                                .insert((tn, stn), *def_id);
                        }
                        self.impls.insert(*def_id, info);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Look up a trait by name.
    pub fn find_trait(&self, name: Spur) -> Option<&TraitInfo> {
        self.trait_by_name
            .get(&name)
            .and_then(|id| self.traits.get(id))
    }

    /// Look up an impl block by (trait_name, self_ty_name).
    pub fn find_impl(&self, trait_name: Spur, self_ty_name: Spur) -> Option<&ImplInfo> {
        self.impl_by_trait_and_type
            .get(&(trait_name, self_ty_name))
            .and_then(|id| self.impls.get(id))
    }

    /// Check if a type implements a trait (by name).
    pub fn implements(&self, trait_name: Spur, self_ty_name: Spur) -> bool {
        self.find_impl(trait_name, self_ty_name).is_some()
    }

    /// Get the number of collected traits.
    pub fn trait_count(&self) -> usize {
        self.traits.len()
    }

    /// Get the number of collected impls.
    pub fn impl_count(&self) -> usize {
        self.impls.len()
    }
}

/// Best-effort extraction of a type name from a HirTy.
fn extract_ty_name(ty: &HirTy) -> Option<Spur> {
    match &ty.kind {
        HirTyKind::Path(_, path) => path.segments.last().map(|s| s.ident.name),
        _ => None,
    }
}
