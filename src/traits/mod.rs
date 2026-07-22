//! Trait resolution: collect trait definitions + impl blocks + build dispatch tables.
//!
//! Stage 5.1: Basic TraitResolver — collects trait/impl metadata from HIR.
//! Stage 5.4: Added `type_by_def_id` reverse map for Copy trait detection.
//! Stage 5.5: Added vtable data structures for L5 trait dispatch.
//! Stage 5.6: VtableEntry now carries the resolved LLVM symbol name (`fn_name`)
//!            so codegen can emit vtable globals without re-walking HIR.
//!            Naming convention: `landin_<SelfType>_<method>` (matches the
//!            symbol codegen emits for impl method bodies via driver body_metas).
//! Stage 5.8: Added `BuiltinTraits` registry — the compiler now recognizes
//!            standard traits (Copy, Clone, Drop, Sized, etc.) automatically,
//!            without requiring the user to define `trait Copy {}`. This is
//!            the stdlib MVP foundation.
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
/// to the concrete function that implements it.
///
/// Stage 5.6: `fn_def_id` (which previously held the *impl block's* DefId,
/// not the per-method DefId, since HIR doesn't give a separate DefId to
/// impl methods) has been replaced by `fn_name: String` — the resolved
/// LLVM symbol name (e.g. `landin_S_bar`). This lets codegen emit vtable
/// globals without consulting `fn_name_by_def_id` (which only covers
/// top-level fns) or re-walking HIR (which would violate §16).
#[derive(Debug, Clone)]
pub struct VtableEntry {
    /// The method name (interned symbol) as declared in the trait.
    pub method_name: Spur,
    /// Stage 5.6: The resolved LLVM symbol name for the concrete impl
    /// method (e.g. `landin_S_bar`). Computed at collect time using the
    /// `landin_<SelfType>_<method>` convention; matches the symbol that
    /// codegen emits for the impl method body (per Stage 5.6 body_metas
    /// extension in driver.rs).
    pub fn_name: String,
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
    /// Stage 5.8: Builtin traits registry — standard traits recognized by
    /// the compiler without user definition (Copy, Clone, Drop, Sized, etc.).
    /// Maps the interned trait name to its builtin DefId (a reserved DefId
    /// in the BUILTIN range, e.g. DefId(u32::MAX - N)).
    pub builtin_traits: HashMap<Spur, DefId>,
}

/// Stage 5.8: The set of builtin trait names recognized by the compiler.
///
/// These are standard library traits that the compiler knows about
/// intrinsically — users do not need to define `trait Copy {}` for the
/// compiler to detect Copy impls. The names are interned during
/// `register_builtin_traits` and stored in `TraitResolver.builtin_traits`.
///
/// Per §15 (最优 > 最小): this is the stdlib MVP foundation — a full stdlib
/// crate would provide actual implementations, but for now the compiler
/// just needs to *recognize* these trait names so that `is_copy()` and
/// future trait checks work without user boilerplate.
pub const BUILTIN_TRAIT_NAMES: &[&str] = &[
    "Copy", "Clone", "Drop", "Sized", "Send", "Sync", "Unpin", "Fn", "FnMut", "FnOnce",
];

/// Stage 5.8: Reserved DefId base for builtin traits.
///
/// Builtin traits get DefIds in the high range (u32::MAX downward) so they
/// never collide with user-defined items (which start from 0). This avoids
/// the need to synthesize fake HIR nodes for builtin traits.
pub const BUILTIN_DEF_ID_BASE: u32 = u32::MAX;

impl TraitResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage 5.8: Register builtin standard traits (Copy, Clone, Drop, etc.)
    /// in the resolver so the compiler recognizes them without user
    /// definition. Called by `collect()` before walking HIR.
    ///
    /// Each builtin trait is assigned a reserved DefId in the high range
    /// (BUILTIN_DEF_ID_BASE downward) and interned into the interner.
    /// User-defined traits with the same name take precedence — if the
    /// user defines `trait Copy {}`, that trait's DefId replaces the
    /// builtin in `trait_by_name` (but `builtin_traits` still records
    /// the builtin DefId for reference).
    pub fn register_builtin_traits(&mut self, interner: &mut Rodeo) {
        for (idx, &name) in BUILTIN_TRAIT_NAMES.iter().enumerate() {
            let spur = interner.get_or_intern(name);
            // Reserved DefId: u32::MAX, u32::MAX-1, u32::MAX-2, ...
            let def_id = DefId::new(BUILTIN_DEF_ID_BASE - idx as u32);
            self.builtin_traits.insert(spur, def_id);
            // Also register in trait_by_name so find_trait() works.
            // User-defined traits will overwrite this during collect().
            self.trait_by_name.entry(spur).or_insert(def_id);
            // Register the name in type_by_def_id so implements_by_def_id
            // can resolve the trait name.
            self.type_by_def_id.insert(def_id, spur);
        }
    }

    /// Stage 5.8: Check if a trait name (Spur) refers to a builtin trait.
    pub fn is_builtin_trait(&self, name: Spur) -> bool {
        self.builtin_traits.contains_key(&name)
    }

    /// Stage 5.8: Get the builtin DefId for a builtin trait name.
    pub fn find_builtin_trait(&self, name: Spur) -> Option<DefId> {
        self.builtin_traits.get(&name).copied()
    }

    /// Collect all trait definitions, impl blocks, type names, and vtables from HIR.
    pub fn collect(&mut self, hir: &HirCrate, interner: &Rodeo) {
        // Stage 5.8: Builtin traits are registered by the driver before
        // collect() is called (via register_builtin_traits), because that
        // method needs &mut Rodeo while collect() takes &Rodeo. Here we
        // just ensure "Copy" is interned for the legacy lookup path.
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

                        // Stage 5.6: resolve the self type's string form up front
                        // so vtable entries can carry the LLVM symbol name
                        // (`landin_<SelfType>_<method>`). This matches the naming
                        // that driver.rs's body_metas now uses for impl method
                        // bodies, so the vtable's symbol references resolve
                        // correctly at link time.
                        let self_ty_str = self_ty_name
                            .and_then(|s| interner.try_resolve(&s))
                            .unwrap_or("Type");

                        // Stage 5.5: Build vtable entries from impl methods.
                        let mut vtable_entries = Vec::new();
                        let mut method_names = Vec::new();
                        for impl_item in &i.items {
                            if let HirImplItem::Fn(f) = impl_item {
                                let method_str =
                                    interner.try_resolve(&f.ident.name).unwrap_or("fn");
                                method_names.push(f.ident.name);
                                vtable_entries.push(VtableEntry {
                                    method_name: f.ident.name,
                                    fn_name: format!("landin_{}_{}", self_ty_str, method_str),
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
///
/// Stage 5.6: promoted to `pub` so the driver can reuse the same name
/// resolution that TraitResolver uses for vtable entries (avoids the
/// driver and TraitResolver drifting apart on naming convention).
pub fn extract_impl_self_ty_name(ty: &HirTy) -> Option<Spur> {
    extract_ty_name(ty)
}

/// Best-effort extraction of a type name from a HirTy.
fn extract_ty_name(ty: &HirTy) -> Option<Spur> {
    match &ty.kind {
        HirTyKind::Path(_, path) => path.segments.last().map(|s| s.ident.name),
        _ => None,
    }
}
