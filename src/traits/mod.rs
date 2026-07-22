//! Trait resolution: collect trait definitions + impl blocks + build dispatch tables.
//!
//! Stage 5.1: Basic TraitResolver — collects trait/impl metadata from HIR.
//! Stage 5.4: Added `type_by_def_id` reverse map for Copy trait detection.
//! Stage 5.5: Added vtable data structures for L5 trait dispatch.
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

/// Stage 5.5: A single entry in a vtable — maps a trait method name
/// to the concrete function DefId that implements it.
#[derive(Debug, Clone)]
pub struct VtableEntry {
    /// The method name (interned symbol) as declared in the trait.
    pub method_name: Spur,
    /// The DefId of the concrete function in the impl block.
    pub fn_def_id: DefId,
}

/// Stage 5.5: A vtable for a specific (trait, type) pair.
///
/// Contains the dispatch entries that map trait method names to
/// concrete function DefIds. This is the data structure that codegen
/// will use to generate LLVM vtable globals for `dyn Trait` support.
#[derive(Debug, Clone)]
pub struct Vtable {
    /// The trait name (interned symbol).
    pub trait_name: Spur,
    /// The self type name (interned symbol).
    pub self_ty_name: Spur,
    /// The DefId of the impl block this vtable corresponds to.
    pub impl_def_id: DefId,
    /// Method dispatch entries: method_name → concrete fn DefId.
    pub entries: Vec<VtableEntry>,
}

/// The TraitResolver. Collects trait definitions and impl blocks from HIR,
/// builds dispatch tables and vtables for method resolution.
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
    /// Stage 5.4: DefId → type name (for struct/enum/trait).
    pub type_by_def_id: HashMap<DefId, Spur>,
    /// Stage 5.5: Vtables keyed by (trait_name, self_ty_name).
    /// Each vtable maps trait method names to concrete fn DefIds.
    pub vtables: HashMap<(Spur, Spur), Vtable>,
}

impl TraitResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect all trait definitions, impl blocks, type names, and vtables from HIR.
    pub fn collect(&mut self, hir: &HirCrate, interner: &Rodeo) {
        let _ = interner.get("Copy");

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
                        self.type_by_def_id.insert(*def_id, t.ident.name);
                        self.traits.insert(*def_id, info);
                    }
                    HirItem::Struct(s) => {
                        self.type_by_def_id.insert(*def_id, s.ident.name);
                    }
                    HirItem::Enum(e) => {
                        self.type_by_def_id.insert(*def_id, e.ident.name);
                    }
                    HirItem::Impl(i) => {
                        let trait_name = i
                            .of_trait
                            .as_ref()
                            .and_then(|p| p.segments.last().map(|s| s.ident.name));
                        let self_ty_name = extract_ty_name(&i.self_ty);

                        // Stage 5.5: Build vtable entries from impl methods.
                        let mut vtable_entries = Vec::new();
                        let mut method_names = Vec::new();
                        for impl_item in &i.items {
                            if let HirImplItem::Fn(f) = impl_item {
                                method_names.push(f.ident.name);
                                vtable_entries.push(VtableEntry {
                                    method_name: f.ident.name,
                                    fn_def_id: *def_id,
                                });
                            }
                        }

                        let info = ImplInfo {
                            def_id: *def_id,
                            trait_name,
                            self_ty_name,
                            methods: method_names,
                            is_unsafe: i.is_unsafe,
                        };

                        // Stage 5.5: Build and store vtable if this is a trait impl.
                        if let (Some(tn), Some(stn)) = (trait_name, self_ty_name) {
                            self.impl_by_trait_and_type.insert((tn, stn), *def_id);

                            // Create vtable for this (trait, type) pair.
                            let vtable = Vtable {
                                trait_name: tn,
                                self_ty_name: stn,
                                impl_def_id: *def_id,
                                entries: vtable_entries,
                            };
                            self.vtables.insert((tn, stn), vtable);
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

    /// Stage 5.4: Check if a type (by DefId) implements a trait (by name).
    pub fn implements_by_def_id(&self, trait_name: Spur, def_id: DefId) -> bool {
        if let Some(type_name) = self.type_by_def_id.get(&def_id) {
            self.implements(trait_name, *type_name)
        } else {
            false
        }
    }

    /// Stage 5.4: Check if a type (by DefId) implements Copy.
    pub fn is_copy(&self, def_id: DefId, copy_name: Spur) -> bool {
        self.implements_by_def_id(copy_name, def_id)
    }

    /// Stage 5.5: Look up a vtable by (trait_name, self_ty_name).
    /// Returns the vtable containing method dispatch entries.
    pub fn find_vtable(&self, trait_name: Spur, self_ty_name: Spur) -> Option<&Vtable> {
        self.vtables.get(&(trait_name, self_ty_name))
    }

    /// Stage 5.5: Get the number of collected vtables.
    pub fn vtable_count(&self) -> usize {
        self.vtables.len()
    }

    /// Get the number of collected traits.
    pub fn trait_count(&self) -> usize {
        self.traits.len()
    }

    /// Get the number of collected impls.
    pub fn impl_count(&self) -> usize {
        self.impls.len()
    }

    /// Stage 5.4: Get the number of collected type names.
    pub fn type_count(&self) -> usize {
        self.type_by_def_id.len()
    }
}

/// Best-effort extraction of a type name from a HirTy.
fn extract_ty_name(ty: &HirTy) -> Option<Spur> {
    match &ty.kind {
        HirTyKind::Path(_, path) => path.segments.last().map(|s| s.ident.name),
        _ => None,
    }
}
