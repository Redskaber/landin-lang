//! TraitResolver — collect trait/impl metadata + build dispatch tables.
//!
//! Stage 5.23: extracted from traits/mod.rs per deep review r70 (TD-NEW-1).
//! mod.rs now re-exports from this module + vtable.rs + builtin.rs.

use crate::hir::*;
use crate::traits::builtin::{BUILTIN_DEF_ID_BASE, BUILTIN_TRAIT_NAMES};
use crate::traits::vtable::{Vtable, VtableEntry};
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
    /// Stage 5.15: Supertrait names (interned symbols) — traits that this
    /// trait requires `Self` to also implement (e.g. `trait Foo: Bar` →
    /// supertraits = [Bar_spur]). Extracted from `HirTrait.supertraits`.
    pub supertraits: Vec<Spur>,
    /// Stage 14.97 (Bug Y1 fix): Method names that have default bodies
    /// (body: Some(BodyId) in the trait declaration). These don't need
    /// to be overridden in impl blocks.
    pub default_methods: Vec<Spur>,
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
    /// Stage 15.89: Source span of the impl block (from HirImpl.span).
    /// Used to attach accurate spans to trait coherence/incomplete errors.
    pub span: crate::session::Span,
}

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
    /// Stage 16.07 (Task 3 step 1): (trait_def_id, self_type_def_id) →
    /// impl DefId. DefId-keyed lookup — type-safe, no interner needed,
    /// and prepares for generic SubstsRef support (Task 3 step 2).
    ///
    /// This map is populated alongside `impl_by_trait_and_type` during
    /// `collect()`. For non-generic types, both maps give the same result.
    /// For generic types (future), this map will be extended to
    /// `(DefId, SubstsRef)` keys.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one DefId-keyed lookup replaces
    /// the Spur-based lookup for new callers.
    /// Per §23: `impls_by_def_ids` follows `<noun>_<prep>_<noun>` pattern.
    pub impls_by_def_ids: HashMap<(DefId, DefId), DefId>,
    /// Stage 5.4: DefId → type name (for struct/enum/trait).
    pub type_by_def_id: HashMap<DefId, Spur>,
    /// Stage 5.5: Vtables keyed by (trait_name, self_ty_name).
    /// Each vtable maps trait method names to concrete fn DefIds.
    pub vtables: HashMap<(Spur, Spur), Vtable>,
    /// Stage 16.10 (Task 3 Step 3 continuation): Vtables keyed by
    /// (trait_def_id, self_type_def_id). DefId-keyed lookup — type-safe,
    /// no interner needed, parallel to `impls_by_def_ids`.
    ///
    /// This map is populated alongside `vtables` during `collect()`.
    /// For non-generic types, both maps give the same result.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one DefId-keyed lookup replaces
    /// the Spur-based lookup for new callers.
    /// Per §23: `vtables_by_def_ids` follows `<noun>_<prep>_<noun>` pattern.
    pub vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>,
    /// Stage 5.8: Builtin traits registry — standard traits recognized by
    /// the compiler without user definition (Copy, Clone, Drop, Sized, etc.).
    /// Maps the interned trait name to its builtin DefId (a reserved DefId
    /// in the BUILTIN range, e.g. DefId(u32::MAX - N)).
    pub builtin_traits: HashMap<Spur, DefId>,
    /// Stage 16.06: Types that are derived Copy (no `impl Drop`, all fields
    /// are Copy). Populated by `collect()` via a fixpoint iteration that
    /// mirrors Rust's `#[derive(Copy, Clone)]` semantics.
    ///
    /// This closes the sound Copy migration gap: types like
    /// `struct Point { x: i32, y: i32 }` are intuitively Copy (all fields
    /// are primitives), and the user shouldn't have to write
    /// `impl Copy for Point {}` for the common case.
    ///
    /// Per §1.0 原則 9 "正确 > 妥协": this is the sound approach — no
    /// unsound `Adt => true` fallback. The derivation is conservative:
    /// only structs with ALL Copy fields AND no `impl Drop` are derived.
    /// Per §1.0 原則 3 "显式 > 隐式": the derivation rule is explicit and
    /// documented (matches Rust's `#[derive(Copy)]`).
    /// Per §16: TraitResolver reads HIR during `collect()` (allowed —
    /// data flows downstream). BorrowChecker queries via `is_copy_builtin`
    /// without needing HIR access.
    pub derived_copy_types: std::collections::HashSet<DefId>,
}

/// Stage 5.18: A trait coherence error — detected when multiple `impl`
/// blocks exist for the same `(trait, type)` pair. In Rust this is a
/// hard error ("conflicting implementations"). Landin Stage 5.18 detects
/// it post-collection; the driver can report it as a compilation error.
///
/// Per API-naming-standard §3: `CoherenceError` follows the `<Noun>Error`
/// pattern consistent with `TypeError`, `BorrowError`, etc.
#[derive(Debug, Clone)]
pub struct CoherenceError {
    /// The trait name (interned symbol) with conflicting impls.
    pub trait_name: Spur,
    /// The self type name (interned symbol) with conflicting impls.
    pub self_ty_name: Spur,
    /// The DefIds of all impl blocks for this (trait, type) pair.
    pub impl_def_ids: Vec<DefId>,
    /// Stage 15.89: Source span of the first conflicting impl block.
    /// Used to attach accurate spans to coherence error messages
    /// (was: Span::DUMMY, producing "1:1").
    pub span: crate::session::Span,
}

/// Stage 5.20: An incomplete impl — a `impl Trait for Type` block that
/// is missing one or more methods declared by the trait.
///
/// Per API-naming-standard §3: `IncompleteImpl` follows the `<Adj><Noun>`
/// pattern consistent with `CoherenceError`.
#[derive(Debug, Clone)]
pub struct IncompleteImpl {
    /// The trait name (interned symbol).
    pub trait_name: Spur,
    /// The self type name (interned symbol).
    pub self_ty_name: Spur,
    /// Method names (interned symbols) declared in the trait but not
    /// implemented in the impl block.
    pub missing_methods: Vec<Spur>,
    /// Stage 15.89: Source span of the incomplete impl block.
    /// Used to attach accurate spans to incomplete impl error messages
    /// (was: Span::DUMMY, producing "1:1").
    pub span: crate::session::Span,
}

/// Stage 5.20: A comprehensive validation report for all trait impls.
///
/// Aggregates coherence errors (Stage 5.18) and incomplete impls (Stage
/// 5.19) into a single report. The driver can call `validate_impls()`
/// once after `collect()` to get all validation issues.
///
/// Per API-naming-standard §3: `ImplValidationReport` follows the
/// `<Noun>ValidationReport` pattern.
#[derive(Debug, Clone)]
pub struct ImplValidationReport {
    /// Coherence errors — conflicting impls for the same (trait, type).
    pub coherence_errors: Vec<CoherenceError>,
    /// Incomplete impls — impls missing one or more trait methods.
    pub incomplete_impls: Vec<IncompleteImpl>,
    /// Overall validity: true if no coherence errors AND no incomplete impls.
    pub is_valid: bool,
}

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
    ///
    /// Stage 15.9: Changed `interner` from `&Rodeo` to `&mut Rodeo` so we can
    /// intern the resolved vtable symbol names (VtableEntry.fn_name is now Spur).
    /// All call sites already pass `&mut Rodeo` or can be adjusted trivially.
    pub fn collect(&mut self, hir: &HirCrate, interner: &mut Rodeo) {
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
                        // Stage 14.97 (Bug Y1 fix): Collect default method bodies.
                        // Trait methods with `body: Some(BodyId)` have default
                        // implementations. When an impl doesn't override them,
                        // the default body should be used instead of reporting
                        // "missing method".
                        //
                        // We store the set of method names that have default
                        // bodies so that `impl_covers_trait` and
                        // `missing_impl_methods` can skip them.
                        let mut default_methods: Vec<Spur> = Vec::new();
                        for trait_item in &t.items {
                            if let HirTraitItem::Fn(f) = trait_item {
                                if f.body.is_some() {
                                    default_methods.push(f.ident.name);
                                }
                            }
                        }
                        // Stage 5.15: Collect supertrait names from
                        // HirTrait.supertraits (Vec<HirTypeBound>).
                        // Each HirTypeBound::Trait(HirTraitBound) has a
                        // HirPath; extract the last segment's name Spur.
                        let supertraits: Vec<Spur> = t
                            .supertraits
                            .iter()
                            .filter_map(|bound| {
                                if let HirTypeBound::Trait(tb) = bound {
                                    tb.path.segments.last().map(|s| s.ident.name)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        let info = TraitInfo {
                            def_id: *def_id,
                            name: t.ident.name,
                            methods,
                            is_unsafe: t.is_unsafe,
                            supertraits,
                            default_methods,
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
                        //
                        // Stage 15.9: Owned String (was &str) so we can mutably
                        // borrow the interner later to intern the symbol name.
                        let self_ty_str = self_ty_name
                            .and_then(|s| interner.try_resolve(&s))
                            .unwrap_or("Type")
                            .to_string();

                        // Stage 5.5: Build vtable entries from impl methods.
                        let mut vtable_entries = Vec::new();
                        let mut method_names = Vec::new();
                        for impl_item in &i.items {
                            if let HirImplItem::Fn(f) = impl_item {
                                let method_str =
                                    interner.try_resolve(&f.ident.name).unwrap_or("fn");
                                method_names.push(f.ident.name);
                                // Stage 15.9: Intern the resolved symbol name
                                // instead of allocating a String. Closes HP-B16.
                                let fn_name_str = format!("landin_{}_{}", self_ty_str, method_str);
                                let fn_name_spur = interner.get_or_intern(fn_name_str);
                                vtable_entries.push(VtableEntry {
                                    method_name: f.ident.name,
                                    fn_name: fn_name_spur,
                                });
                            }
                        }

                        let info = ImplInfo {
                            def_id: *def_id,
                            trait_name,
                            self_ty_name,
                            methods: method_names,
                            is_unsafe: i.is_unsafe,
                            // Stage 15.89: store the impl block's source span
                            // for accurate trait error reporting.
                            span: i.span,
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

                            // Stage 16.10: DefId-keyed maps (impls_by_def_ids,
                            // vtables_by_def_ids) are now populated in a post-pass
                            // after the main collect() loop, to handle HIR
                            // iteration ordering (user-defined traits may not be
                            // in trait_by_name yet when their impls are processed).
                            // See populate_def_id_keyed_maps().
                        }
                        self.impls.insert(*def_id, info);
                    }
                    _ => {}
                }
            }
        }

        // Stage 16.10: Post-pass to populate DefId-keyed maps.
        // The main loop above populates impls_by_def_ids and vtables_by_def_ids
        // inline, but this fails for user-defined traits when the impl block
        // is processed BEFORE the trait definition (due to HashMap iteration
        // order). This post-pass runs after ALL traits, types, and impls have
        // been collected, so all lookups will succeed.
        self.populate_def_id_keyed_maps();

        // Stage 16.06: Derive Copy for structs whose fields are all Copy.
        // This mirrors Rust's `#[derive(Copy, Clone)]` semantics and closes
        // the sound Copy migration gap (Stages 15.99/16.02/16.03).
        self.derive_copy_types(hir, interner);
    }

    /// Stage 16.10: Populate DefId-keyed maps (impls_by_def_ids, vtables_by_def_ids).
    ///
    /// This post-pass runs after the main `collect()` loop, ensuring all
    /// traits and types are registered before resolving DefId keys.
    /// The inline population during the main loop may miss impls for
    /// user-defined traits that appear later in the HIR iteration order.
    ///
    /// Per §23: `populate_def_id_keyed_maps` follows `<verb>_<noun>_<noun>`
    /// pattern.
    fn populate_def_id_keyed_maps(&mut self) {
        // Clear any partial data from inline population (some entries may
        // have been added during the main loop; we rebuild from scratch
        // to ensure completeness and consistency).
        self.impls_by_def_ids.clear();
        self.vtables_by_def_ids.clear();

        for (impl_def_id, info) in &self.impls {
            // Skip impls without trait or self type info.
            let (Some(trait_name), Some(self_ty_name)) = (info.trait_name, info.self_ty_name)
            else {
                continue;
            };

            // Resolve trait_name Spur → trait DefId via trait_by_name.
            let Some(trait_def_id) = self.trait_by_name.get(&trait_name).copied() else {
                continue;
            };

            // Resolve self_ty_name Spur → self type DefId via reverse lookup.
            let self_def_id = self
                .type_by_def_id
                .iter()
                .find(|(_, &name)| name == self_ty_name)
                .map(|(&d, _)| d);
            let Some(self_def_id) = self_def_id else {
                continue;
            };

            // Populate impls_by_def_ids.
            self.impls_by_def_ids
                .insert((trait_def_id, self_def_id), *impl_def_id);

            // Populate vtables_by_def_ids (clone from Spur-keyed map).
            if let Some(vtable) = self.vtables.get(&(trait_name, self_ty_name)) {
                self.vtables_by_def_ids
                    .insert((trait_def_id, self_def_id), vtable.clone());
            }
        }
    }

    /// Stage 16.06: Derive Copy for structs whose fields are all Copy.
    ///
    /// Performs a fixpoint iteration: repeatedly scan all structs, and for
    /// each struct that has no `impl Drop` and no explicit `impl Copy`,
    /// check if ALL its fields are Copy (primitives, refs, arrays of Copy,
    /// tuples of Copy, or other derived-Copy structs). If so, mark it as
    /// derived Copy. Repeat until no new types are derived.
    ///
    /// This handles recursive/nested structs: `struct A { b: B }` where
    /// `struct B { x: i32 }` — B is derived Copy first, then A.
    ///
    /// Per §1.0 原則 9 "正确 > 妥协": sound derivation, no `Adt => true`.
    /// Per §1.0 原則 6 "通用 > 特例": one rule handles all Copy-derivable
    /// structs, not just primitives.
    /// Per §23: `derive_copy_types` follows `<verb>_<noun>_<noun>` pattern.
    fn derive_copy_types(&mut self, hir: &HirCrate, interner: &Rodeo) {
        use crate::hir::{HirItem, OwnerNode};

        // Collect all struct DefIds and their fields.
        // Also collect the set of DefIds that have `impl Drop` (Copy+Drop conflict).
        let mut drop_def_ids: std::collections::HashSet<DefId> = std::collections::HashSet::new();
        if let Some(drop_name) = interner.get("Drop") {
            for impl_info in self.impls.values() {
                if impl_info.trait_name == Some(drop_name) {
                    if let Some(self_ty_name) = impl_info.self_ty_name {
                        // Find the DefId whose type_by_def_id matches self_ty_name.
                        for (did, name) in &self.type_by_def_id {
                            if *name == self_ty_name {
                                drop_def_ids.insert(*did);
                            }
                        }
                    }
                }
            }
        }

        // Also collect explicit `impl Copy` DefIds (already Copy, no need to derive).
        let mut explicit_copy_def_ids: std::collections::HashSet<DefId> =
            std::collections::HashSet::new();
        if let Some(copy_name) = interner.get("Copy") {
            for impl_info in self.impls.values() {
                if impl_info.trait_name == Some(copy_name) {
                    if let Some(self_ty_name) = impl_info.self_ty_name {
                        for (did, name) in &self.type_by_def_id {
                            if *name == self_ty_name {
                                explicit_copy_def_ids.insert(*did);
                            }
                        }
                    }
                }
            }
        }

        // Fixpoint iteration: repeat until no new types are derived.
        loop {
            let mut changed = false;
            for (def_id, node) in &hir.owners {
                // Stage 16.06: Derive Copy for structs AND enums.
                // - Structs: all fields must be Copy.
                // - Enums: all variant fields must be Copy (unit variants
                //   have no fields, so they're always Copy-derivable).
                let (is_struct, is_enum, all_fields_copy) = match node {
                    OwnerNode::Item(HirItem::Struct(s)) => {
                        let all_copy = s.fields.iter().all(|f| {
                            hir_ty_is_copy_candidate(
                                &f.ty.kind,
                                &self.derived_copy_types,
                                &explicit_copy_def_ids,
                            )
                        });
                        (true, false, all_copy)
                    }
                    OwnerNode::Item(HirItem::Enum(e)) => {
                        // All variant fields must be Copy.
                        let all_copy = e.variants.iter().all(|v| {
                            let fields = match &v.data {
                                crate::hir::HirVariantData::Unit(_) => &[],
                                crate::hir::HirVariantData::Tuple(fs, _) => fs.as_slice(),
                                crate::hir::HirVariantData::Struct(fs, _) => fs.as_slice(),
                            };
                            fields.iter().all(|f| {
                                hir_ty_is_copy_candidate(
                                    &f.ty.kind,
                                    &self.derived_copy_types,
                                    &explicit_copy_def_ids,
                                )
                            })
                        });
                        (false, true, all_copy)
                    }
                    _ => (false, false, false),
                };
                if !is_struct && !is_enum {
                    continue;
                }
                // Skip if already explicit Copy.
                if explicit_copy_def_ids.contains(def_id) {
                    continue;
                }
                // Skip if already derived Copy.
                if self.derived_copy_types.contains(def_id) {
                    continue;
                }
                // Skip if has `impl Drop` (Copy+Drop conflict).
                if drop_def_ids.contains(def_id) {
                    continue;
                }
                if all_fields_copy {
                    self.derived_copy_types.insert(*def_id);
                    changed = true;
                }
            }
            if !changed {
                break;
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
    ///
    /// Stage 16.11 (Task 3 Step 4): DEPRECATED. Use `find_impl_by_def_ids`
    /// instead — it's type-safe and doesn't require an interner.
    #[deprecated(
        note = "Use find_impl_by_def_ids (DefId-keyed, type-safe, no interner needed) instead. (Stage 16.11)"
    )]
    pub fn find_impl(&self, trait_name: Spur, self_ty_name: Spur) -> Option<&ImplInfo> {
        self.impl_by_trait_and_type
            .get(&(trait_name, self_ty_name))
            .and_then(|id| self.impls.get(id))
    }

    /// Stage 16.07 (Task 3 step 1): Look up an impl block by DefIds.
    ///
    /// This is the **preferred lookup method** for new code — it uses
    /// `DefId`s instead of `Spur`s, providing:
    /// 1. **Type safety**: DefId is a unique identifier, not a string hash.
    /// 2. **No interner needed**: callers don't need `&Rodeo` to look up.
    /// 3. **Prepares for generics**: Task 3 step 2 will extend the key to
    ///    `(DefId, SubstsRef)` for generic type support.
    ///
    /// For non-generic types (v0.1), this gives the same result as
    /// `find_impl(trait_name_spur, self_ty_name_spur)`.
    ///
    /// Per §23: `find_impl_by_def_ids` follows `<verb>_<noun>_<prep>_<noun>`
    /// pattern. The `_by_def_ids` suffix distinguishes from the Spur-based
    /// `find_impl`.
    /// Per §1.0 原則 6 "通用 > 特例": one DefId-keyed lookup for all callers.
    pub fn find_impl_by_def_ids(
        &self,
        trait_def_id: DefId,
        self_type_def_id: DefId,
    ) -> Option<&ImplInfo> {
        self.impls_by_def_ids
            .get(&(trait_def_id, self_type_def_id))
            .and_then(|id| self.impls.get(id))
    }

    /// Stage 16.07 (Task 3 step 1): Check if a type implements a trait,
    /// keyed by DefIds.
    ///
    /// This is the DefId-based equivalent of `implements(trait_name_spur,
    /// self_ty_name_spur)`. Preferred for new code.
    ///
    /// Per §23: `implements_by_def_ids` follows `<verb>_<prep>_<noun>`
    /// pattern.
    pub fn implements_by_def_ids(&self, trait_def_id: DefId, self_type_def_id: DefId) -> bool {
        self.find_impl_by_def_ids(trait_def_id, self_type_def_id)
            .is_some()
    }

    /// Stage 16.07 (Task 3 step 1): Look up a trait DefId by name.
    ///
    /// Convenience method to convert a trait name Spur to DefId, then
    /// use with `find_impl_by_def_ids`. Returns `None` if the trait
    /// name is not registered.
    ///
    /// Per §23: `find_trait_def_id` follows `<verb>_<noun>_<noun>` pattern.
    pub fn find_trait_def_id(&self, trait_name: Spur) -> Option<DefId> {
        self.trait_by_name.get(&trait_name).copied()
    }

    /// Stage 5.14: Get the method names declared in a trait (by Spur).
    /// Returns `None` if the trait is not found.
    ///
    /// Per API-naming-standard §3: `trait_methods` follows `<noun>_<noun>`
    /// pattern for query methods returning collections.
    pub fn trait_methods(&self, trait_name: Spur) -> Option<&Vec<Spur>> {
        self.find_trait(trait_name).map(|t| &t.methods)
    }

    /// Stage 5.14: Get the method names implemented in an impl block
    /// (by trait_name + self_ty_name). Returns `None` if no impl found.
    ///
    /// Per API-naming-standard §3: `impl_methods` follows `<noun>_<noun>`
    /// pattern; parallels `trait_methods`.
    ///
    /// Stage 16.11 (Task 3 Step 4): DEPRECATED. Use `impl_methods_by_def_ids`
    /// instead — it's type-safe and doesn't require an interner.
    #[deprecated(
        note = "Use impl_methods_by_def_ids (DefId-keyed, type-safe) instead. (Stage 16.11)"
    )]
    pub fn impl_methods(&self, trait_name: Spur, self_ty_name: Spur) -> Option<&Vec<Spur>> {
        #[allow(deprecated)]
        self.find_impl(trait_name, self_ty_name).map(|i| &i.methods)
    }

    /// Stage 16.11 (Task 3 Step 4): Get the method names implemented in an
    /// impl block, keyed by DefIds. DefId-keyed equivalent of `impl_methods`.
    ///
    /// Per §23: `impl_methods_by_def_ids` follows `<noun>_<noun>_<prep>_<noun>`
    /// pattern.
    pub fn impl_methods_by_def_ids(
        &self,
        trait_def_id: DefId,
        self_type_def_id: DefId,
    ) -> Option<&Vec<Spur>> {
        self.find_impl_by_def_ids(trait_def_id, self_type_def_id)
            .map(|i| &i.methods)
    }

    /// Stage 5.14: Check if a trait declares a method (by name).
    /// Returns `false` if the trait is not found or doesn't declare the method.
    ///
    /// Per API-naming-standard §3: `trait_has_method` follows
    /// `<noun>_<verb>_<noun>` pattern for boolean queries.
    pub fn trait_has_method(&self, trait_name: Spur, method_name: Spur) -> bool {
        if let Some(methods) = self.trait_methods(trait_name) {
            methods.contains(&method_name)
        } else {
            false
        }
    }

    /// Stage 5.14: Find all traits that declare a method (by name).
    /// Returns a Vec of trait name Spurs. Useful for method resolution
    /// when the method name is known but the trait is not.
    ///
    /// Per API-naming-standard §3: `traits_with_method` follows
    /// `<noun>_with_<noun>` pattern for collection-returning queries.
    pub fn traits_with_method(&self, method_name: Spur) -> Vec<Spur> {
        self.traits
            .values()
            .filter_map(|t| {
                if t.methods.contains(&method_name) {
                    Some(t.name)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Stage 5.14: Get the method count for a trait (by Spur).
    /// Returns 0 if the trait is not found.
    ///
    /// Per API-naming-standard §3: `method_count_for_trait` follows
    /// `<noun>_count_for_<noun>` pattern, consistent with
    /// `impl_count_for_trait` (Stage 5.13).
    pub fn method_count_for_trait(&self, trait_name: Spur) -> usize {
        self.trait_methods(trait_name).map(|m| m.len()).unwrap_or(0)
    }

    /// Stage 5.15: Get the supertrait names of a trait (by Spur).
    /// Returns `None` if the trait is not found.
    ///
    /// Per API-naming-standard §3: `trait_supertraits` follows
    /// `<noun>_<noun>` pattern, consistent with `trait_methods`.
    pub fn trait_supertraits(&self, trait_name: Spur) -> Option<&Vec<Spur>> {
        self.find_trait(trait_name).map(|t| &t.supertraits)
    }

    /// Stage 5.15: Check if a trait has a specific supertrait.
    /// Returns `false` if the trait is not found or doesn't have the supertrait.
    ///
    /// Per API-naming-standard §3: `trait_has_supertrait` follows
    /// `<noun>_<verb>_<noun>` pattern, consistent with `trait_has_method`.
    pub fn trait_has_supertrait(&self, trait_name: Spur, supertrait_name: Spur) -> bool {
        if let Some(supertraits) = self.trait_supertraits(trait_name) {
            supertraits.contains(&supertrait_name)
        } else {
            false
        }
    }

    /// Stage 5.15: Get the supertrait count for a trait (by Spur).
    /// Returns 0 if the trait is not found.
    ///
    /// Per API-naming-standard §3: `supertrait_count_for_trait` follows
    /// `<noun>_count_for_<noun>` pattern, consistent with
    /// `method_count_for_trait`.
    pub fn supertrait_count_for_trait(&self, trait_name: Spur) -> usize {
        self.trait_supertraits(trait_name)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Stage 5.17: Resolve a vtable method to its concrete LLVM symbol name.
    ///
    /// Stage 15.9: Find a vtable entry by method name (without resolving
    /// the fn_name to a string). Used by `resolve_vtable_method` (which
    /// needs the string) and `vtable_has_method` (which only needs to
    /// know if the entry exists).
    ///
    /// Per §1.0 原则 3 "显式 > 隐式": the entry lookup is explicit, separate
    /// from the string resolution.
    fn find_vtable_method_entry(
        &self,
        trait_name: Spur,
        self_ty_name: Spur,
        method_name: Spur,
    ) -> Option<&VtableEntry> {
        #[allow(deprecated)]
        let vtable = self.find_vtable(trait_name, self_ty_name)?;
        vtable.entries.iter().find(|e| e.method_name == method_name)
    }

    /// Given `(trait_spur, type_spur, method_spur)`, looks up the vtable
    /// for `(trait, type)` and finds the entry whose `method_name` matches
    /// `method_spur`. Returns the resolved `fn_name` (e.g. `landin_S_bar`).
    ///
    /// Returns `None` if:
    /// - No vtable exists for `(trait, type)` (no `impl Trait for Type`)
    /// - The trait doesn't declare the method
    /// - The method isn't in the vtable entries
    ///
    /// This is the single entry point for vtable method resolution — it
    /// combines `find_vtable` + entry lookup in one call.
    ///
    /// Stage 15.9: Added `interner` parameter because `VtableEntry.fn_name`
    /// is now an interned `Spur` (was `String`). The interner resolves it
    /// to `&str` for the caller.
    ///
    /// Per API-naming-standard §3: `resolve_vtable_method` follows
    /// `resolve_<noun>_<noun>` pattern for resolution queries returning
    /// the resolved value.
    pub fn resolve_vtable_method<'a>(
        &'a self,
        interner: &'a lasso::Rodeo,
        trait_name: Spur,
        self_ty_name: Spur,
        method_name: Spur,
    ) -> Option<&'a str> {
        let entry = self.find_vtable_method_entry(trait_name, self_ty_name, method_name)?;
        // Stage 15.9: The returned &str borrows from `interner`, not `self`.
        // Both params share lifetime `'a` so the returned &str is valid as
        // long as both self and interner are alive.
        interner.try_resolve(&entry.fn_name)
    }

    /// Stage 5.17: Get all method symbol names from a vtable
    /// (by trait + type). Returns an empty Vec if no vtable exists.
    ///
    /// Stage 15.9: Added `interner` parameter because `VtableEntry.fn_name`
    /// is now an interned `Spur` (was `String`).
    ///
    /// Per API-naming-standard §3: `vtable_method_names` follows
    /// `<noun>_<noun>_<noun>` pattern for collection-returning queries.
    pub fn vtable_method_names<'a>(
        &'a self,
        interner: &'a lasso::Rodeo,
        trait_name: Spur,
        self_ty_name: Spur,
    ) -> Vec<&'a str> {
        #[allow(deprecated)]
        if let Some(vtable) = self.find_vtable(trait_name, self_ty_name) {
            vtable
                .entries
                .iter()
                .filter_map(|e| interner.try_resolve(&e.fn_name))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Stage 5.17: Check if a vtable has a method entry
    /// (by trait + type + method name).
    ///
    /// Stage 15.9: No longer calls `resolve_vtable_method` (which now
    /// requires an interner). Uses `find_vtable_method_entry` directly —
    /// the existence check doesn't need to resolve the fn_name string.
    ///
    /// Per API-naming-standard §3: `vtable_has_method` follows
    /// `<noun>_<verb>_<noun>` pattern for boolean queries, consistent
    /// with `trait_has_method`.
    pub fn vtable_has_method(
        &self,
        trait_name: Spur,
        self_ty_name: Spur,
        method_name: Spur,
    ) -> bool {
        self.find_vtable_method_entry(trait_name, self_ty_name, method_name)
            .is_some()
    }

    /// Check if a type implements a trait (by name).
    ///
    /// Stage 16.11 (Task 3 Step 4): DEPRECATED. Use `implements_by_def_ids`
    /// instead — it's type-safe and doesn't require an interner.
    #[deprecated(
        note = "Use implements_by_def_ids (DefId-keyed, type-safe, no interner needed) instead. (Stage 16.11)"
    )]
    pub fn implements(&self, trait_name: Spur, self_ty_name: Spur) -> bool {
        #[allow(deprecated)]
        self.find_impl(trait_name, self_ty_name).is_some()
    }

    /// Stage 5.4: Check if a type (by DefId) implements a trait (by name).
    ///
    /// Stage 16.11 (Task 3 Step 4): DEPRECATED. Use `implements_by_def_ids`
    /// instead — it takes both DefIds and is fully type-safe.
    #[deprecated(
        note = "Use implements_by_def_ids (both args are DefIds, type-safe) instead. (Stage 16.11)"
    )]
    pub fn implements_by_def_id(&self, trait_name: Spur, def_id: DefId) -> bool {
        if let Some(type_name) = self.type_by_def_id.get(&def_id) {
            #[allow(deprecated)]
            self.implements(trait_name, *type_name)
        } else {
            false
        }
    }

    /// Stage 5.4: Check if a type (by DefId) implements Copy.
    pub fn is_copy(&self, def_id: DefId, copy_name: Spur) -> bool {
        #[allow(deprecated)]
        self.implements_by_def_id(copy_name, def_id)
    }

    /// Stage 5.9: Check if a type (by DefId) implements the builtin Copy
    /// trait. Unlike `is_copy()`, this does NOT require the caller to pass
    /// the Copy Spur — it looks up the builtin Copy trait from
    /// `builtin_traits` automatically.
    ///
    /// This is the preferred Copy-detection entry point for downstream
    /// stages (borrowck, typeck) because it works regardless of whether
    /// the user defined `trait Copy {}` — the builtin registration (Stage
    /// 5.8) ensures "Copy" is always interned and recognized.
    ///
    /// Stage 16.06: Also checks `derived_copy_types` — structs whose
    /// fields are all Copy (and no `impl Drop`) are derived Copy via
    /// `derive_copy_types()` during `collect()`. This mirrors Rust's
    /// `#[derive(Copy, Clone)]` semantics.
    ///
    /// Returns `false` if:
    /// - The builtin Copy trait is not registered (shouldn't happen after
    ///   Stage 5.8, but defensive).
    /// - The type's DefId is not in `type_by_def_id`.
    /// - The type does not have an `impl Copy for <Type>` block AND is
    ///   not in `derived_copy_types`.
    pub fn is_copy_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
        // Stage 16.06: Check derived Copy first (no interner lookup needed).
        if self.derived_copy_types.contains(&def_id) {
            return true;
        }
        // Stage 16.08 (Task 3 Step 3): Use DefId-keyed lookup instead of
        // Spur-based `is_copy`. Resolve "Copy" Spur → trait DefId via
        // `find_trait_def_id` (which uses `trait_by_name`, so user-defined
        // `trait Copy {}` takes precedence over the builtin).
        // Then call `implements_by_def_ids` (DefId-keyed, no interner needed
        // for the actual lookup).
        //
        // The `interner` parameter is retained for backward compatibility
        // and to resolve the "Copy" string to a Spur. Future Step 4 can
        // remove it once all callers pre-resolve the trait DefId.
        if let Some(copy_name) = interner.get("Copy") {
            if let Some(trait_def_id) = self.find_trait_def_id(copy_name) {
                self.implements_by_def_ids(trait_def_id, def_id)
            } else {
                false
            }
        } else {
            // Defensive: if "Copy" is not interned (e.g. register_builtin_traits
            // wasn't called), fall back to false. This is safer than the old
            // fallback of true (which was unsound — it treated all Adt as Copy).
            false
        }
    }

    /// Stage 5.10: Check if a type (by DefId) implements the builtin Clone
    /// trait. Follows the same pattern as `is_copy_builtin()` — looks up
    /// "Clone" from the interner automatically (no caller-supplied Spur).
    ///
    /// Returns `false` if "Clone" is not interned or the type has no
    /// `impl Clone for <Type>` block.
    pub fn is_clone_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
        // Stage 16.08 (Task 3 Step 3): Use DefId-keyed lookup.
        if let Some(clone_name) = interner.get("Clone") {
            if let Some(trait_def_id) = self.find_trait_def_id(clone_name) {
                self.implements_by_def_ids(trait_def_id, def_id)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Stage 5.10: Check if a type (by DefId) implements the builtin Drop
    /// trait. Follows the same pattern as `is_copy_builtin()`.
    ///
    /// Returns `false` if "Drop" is not interned or the type has no
    /// `impl Drop for <Type>` block.
    pub fn is_drop_builtin(&self, def_id: DefId, interner: &Rodeo) -> bool {
        // Stage 16.08 (Task 3 Step 3): Use DefId-keyed lookup.
        if let Some(drop_name) = interner.get("Drop") {
            if let Some(trait_def_id) = self.find_trait_def_id(drop_name) {
                self.implements_by_def_ids(trait_def_id, def_id)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Stage 5.10: Generic builtin trait check — checks if a type implements
    /// any builtin trait by name. This is the generic form of
    /// `is_copy_builtin` / `is_clone_builtin` / `is_drop_builtin`.
    ///
    /// `trait_name` is the string name of the builtin trait (e.g. "Send",
    /// "Sync", "Sized"). The trait must be in `BUILTIN_TRAIT_NAMES` and
    /// registered via `register_builtin_traits()`.
    ///
    /// Returns `false` if:
    /// - The trait name is not interned.
    /// - The type's DefId is not in `type_by_def_id`.
    /// - The type does not have an `impl <Trait> for <Type>` block.
    pub fn implements_builtin_trait(
        &self,
        def_id: DefId,
        trait_name: &str,
        interner: &Rodeo,
    ) -> bool {
        // Stage 16.08 (Task 3 Step 3): Use DefId-keyed lookup.
        if let Some(trait_spur) = interner.get(trait_name) {
            if let Some(trait_def_id) = self.find_trait_def_id(trait_spur) {
                self.implements_by_def_ids(trait_def_id, def_id)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Stage 5.5: Look up a vtable by (trait_name, self_ty_name).
    /// Returns the vtable containing method dispatch entries.
    ///
    /// Stage 16.11 (Task 3 Step 4): DEPRECATED for query-only callers.
    /// Use `find_vtable_by_def_ids` instead — it's type-safe and doesn't
    /// require an interner. This method is retained for callers that
    /// need string-keyed access (e.g., `build_dyn_trait_fat_ptrs_from_resolver`
    /// which iterates `vtables.keys()` for string names).
    #[deprecated(
        note = "Use find_vtable_by_def_ids (DefId-keyed, type-safe) instead. Retained for string-keyed iteration callers. (Stage 16.11)"
    )]
    pub fn find_vtable(&self, trait_name: Spur, self_ty_name: Spur) -> Option<&Vtable> {
        self.vtables.get(&(trait_name, self_ty_name))
    }

    /// Stage 16.10 (Task 3 Step 3 continuation): Look up a vtable by DefIds.
    ///
    /// This is the **preferred vtable lookup method** for new code — it uses
    /// `DefId`s instead of `Spur`s, providing:
    /// 1. **Type safety**: DefId is a unique identifier, not a string hash.
    /// 2. **No interner needed**: callers don't need `&Rodeo` to look up.
    /// 3. **Consistency**: parallels `find_impl_by_def_ids` (Stage 16.07).
    ///
    /// For non-generic types (v0.1), this gives the same result as
    /// `find_vtable(trait_name_spur, self_ty_name_spur)`.
    ///
    /// Per §23: `find_vtable_by_def_ids` follows `<verb>_<noun>_<prep>_<noun>`
    /// pattern. The `_by_def_ids` suffix distinguishes from the Spur-based
    /// `find_vtable`.
    /// Per §1.0 原則 6 "通用 > 特例": one DefId-keyed lookup for all callers.
    pub fn find_vtable_by_def_ids(
        &self,
        trait_def_id: DefId,
        self_type_def_id: DefId,
    ) -> Option<&Vtable> {
        self.vtables_by_def_ids
            .get(&(trait_def_id, self_type_def_id))
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

    /// Stage 5.13: Get the number of trait impls for a specific type
    /// (by DefId). Counts how many `impl Trait for <Type>` blocks exist
    /// for the given type.
    ///
    /// Useful for diagnostics ("type S implements N traits") and for
    /// typeck trait-bound solving.
    ///
    /// Per API-naming-standard §3: `impl_count_` prefix consistent with
    /// `impl_count()`; `_for_type` suffix specifies the dimension.
    pub fn impl_count_for_type(&self, def_id: DefId) -> usize {
        // Look up the type name, then count impls where self_ty_name matches.
        if let Some(&type_name) = self.type_by_def_id.get(&def_id) {
            self.impls
                .values()
                .filter(|impl_info| impl_info.self_ty_name == Some(type_name))
                .count()
        } else {
            0
        }
    }

    /// Stage 5.13: Get the number of impls for a specific trait (by Spur).
    /// Counts how many `impl <Trait> for Type` blocks exist for the given
    /// trait.
    ///
    /// Useful for diagnostics ("trait Foo has N implementations") and for
    /// coherence checking.
    ///
    /// Per API-naming-standard §3: `impl_count_` prefix; `_for_trait` suffix.
    pub fn impl_count_for_trait(&self, trait_spur: Spur) -> usize {
        self.impls
            .values()
            .filter(|impl_info| impl_info.trait_name == Some(trait_spur))
            .count()
    }

    /// Stage 5.13: Get the number of builtin traits registered.
    /// Equivalent to `builtin_traits.len()`.
    pub fn builtin_trait_count(&self) -> usize {
        self.builtin_traits.len()
    }

    /// Stage 5.13: Get all trait names (Spurs) that a type implements.
    /// Returns a Vec of trait name Spurs for which `impl <Trait> for <Type>`
    /// exists.
    ///
    /// Per API-naming-standard §3: `traits_for_type` follows the
    /// `<noun>_for_<noun>` pattern for query methods returning collections.
    pub fn traits_for_type(&self, def_id: DefId) -> Vec<Spur> {
        if let Some(&type_name) = self.type_by_def_id.get(&def_id) {
            self.impls
                .values()
                .filter_map(|impl_info| {
                    if impl_info.self_ty_name == Some(type_name) {
                        impl_info.trait_name
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Stage 5.16: Generate a human-readable summary of the TraitResolver
    /// state. Useful for diagnostics, debugging, and error messages.
    ///
    /// The summary includes:
    /// - Trait count + impl count + type count + vtable count + builtin count
    /// - Per-trait: name, method count, supertrait count
    /// - Per-type: name, impl count, implemented trait names
    ///
    /// Per API-naming-standard §3: `summary` is a noun naming the output
    /// (the summary string); consistent with Rust convention for
    /// human-readable output methods (e.g. `to_string`).
    pub fn summary(&self, interner: &Rodeo) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "TraitResolver summary:\n  traits: {}\n  impls: {}\n  types: {}\n  vtables: {}\n  builtin_traits: {}\n",
            self.trait_count(),
            self.impl_count(),
            self.type_count(),
            self.vtable_count(),
            self.builtin_trait_count()
        ));

        // Per-trait details
        if !self.traits.is_empty() {
            out.push_str("\n  Traits:\n");
            for trait_info in self.traits.values() {
                let name = interner.try_resolve(&trait_info.name).unwrap_or("?");
                let method_count = trait_info.methods.len();
                let supertrait_count = trait_info.supertraits.len();
                out.push_str(&format!(
                    "    {}: {} methods, {} supertraits",
                    name, method_count, supertrait_count
                ));
                if !trait_info.supertraits.is_empty() {
                    let supers: Vec<&str> = trait_info
                        .supertraits
                        .iter()
                        .map(|s| interner.try_resolve(s).unwrap_or("?"))
                        .collect();
                    out.push_str(&format!(" ({})", supers.join(", ")));
                }
                out.push('\n');
            }
        }

        // Per-type impl details
        if !self.type_by_def_id.is_empty() {
            out.push_str("\n  Types:\n");
            for (&def_id, &name_spur) in &self.type_by_def_id {
                // Skip builtin trait DefIds (they're in the reserved range)
                if def_id.0 > BUILTIN_DEF_ID_BASE - BUILTIN_TRAIT_NAMES.len() as u32 {
                    continue;
                }
                let name = interner.try_resolve(&name_spur).unwrap_or("?");
                let impl_count = self.impl_count_for_type(def_id);
                out.push_str(&format!("    {}: {} impls", name, impl_count));
                if impl_count > 0 {
                    let traits: Vec<String> = self
                        .traits_for_type(def_id)
                        .iter()
                        .map(|s| interner.try_resolve(s).unwrap_or("?").to_string())
                        .collect();
                    out.push_str(&format!(" ({})", traits.join(", ")));
                }
                out.push('\n');
            }
        }

        out
    }

    /// Stage 5.18: Check trait coherence — detect conflicting impls
    /// (multiple `impl Trait for Type` for the same `(trait, type)` pair).
    ///
    /// Stage 17.09 (v0.5 P2): Enhanced to also detect duplicate impl blocks
    /// with identical DefIds (same impl block registered twice due to a
    /// driver bug). Previously, two impls with the same DefId would be
    /// reported as a coherence error even though they're the same block.
    /// Now, DefId-level dedup is performed before group counting.
    ///
    /// In Rust, this is a hard error ("conflicting implementations of
    /// trait"). Landin Stage 5.18 detects it post-collection by scanning
    /// all impls and grouping by `(trait_name, self_ty_name)`. Any group
    /// with >1 *distinct* impl is a coherence error.
    ///
    /// Returns a Vec of `CoherenceError` — one per conflicting pair.
    /// Empty Vec means no coherence violations.
    ///
    /// Per API-naming-standard §3: `check_coherence` follows
    /// `check_<noun>` pattern consistent with `check_visibility`.
    pub fn check_coherence(&self) -> Vec<CoherenceError> {
        use std::collections::HashMap as StdHashMap;

        // Group impl DefIds by (trait_name, self_ty_name)
        // Stage 15.89: also track the first impl's span for error reporting.
        let mut groups: StdHashMap<(Spur, Spur), (Vec<DefId>, crate::session::Span)> =
            StdHashMap::new();
        for impl_info in self.impls.values() {
            if let (Some(trait_name), Some(self_ty_name)) =
                (impl_info.trait_name, impl_info.self_ty_name)
            {
                let entry = groups
                    .entry((trait_name, self_ty_name))
                    .or_insert_with(|| (Vec::new(), impl_info.span));
                // Stage 17.09: Dedup by DefId — don't count the same impl
                // block twice (can happen if collect() registers it twice).
                if !entry.0.contains(&impl_info.def_id) {
                    entry.0.push(impl_info.def_id);
                }
            }
        }

        // Any group with >1 *distinct* impl is a coherence error
        groups
            .into_iter()
            .filter(|(_, (def_ids, _))| def_ids.len() > 1)
            .map(
                |((trait_name, self_ty_name), (impl_def_ids, span))| CoherenceError {
                    trait_name,
                    self_ty_name,
                    impl_def_ids,
                    span,
                },
            )
            .collect()
    }

    /// Stage 5.18: Check if a specific (trait, type) pair has conflicting
    /// impls. Returns `true` if >1 impl exists for this pair.
    ///
    /// Per API-naming-standard §3: `has_coherence_error` follows
    /// `has_<noun>` pattern for boolean queries.
    pub fn has_coherence_error(&self, trait_name: Spur, self_ty_name: Spur) -> bool {
        let count = self
            .impls
            .values()
            .filter(|i| i.trait_name == Some(trait_name) && i.self_ty_name == Some(self_ty_name))
            .count();
        count > 1
    }

    /// Stage 5.18: Get the coherence error count (number of (trait, type)
    /// pairs with conflicting impls).
    ///
    /// Per API-naming-standard §3: `coherence_error_count` follows
    /// `<noun>_count` pattern consistent with `trait_count` / `impl_count`.
    pub fn coherence_error_count(&self) -> usize {
        self.check_coherence().len()
    }

    /// Stage 5.19: Check if an impl covers all methods declared by the trait.
    ///
    /// Given `(trait_spur, type_spur)`, compares the methods implemented
    /// in the impl block against the methods declared in the trait. Returns
    /// `true` if all trait methods are implemented.
    ///
    /// Returns `false` if:
    /// - No impl exists for `(trait, type)`
    /// - The impl is missing one or more trait methods
    ///
    /// Per API-naming-standard §3: `impl_covers_trait` follows
    /// `<noun>_<verb>_<noun>` pattern for boolean queries.
    pub fn impl_covers_trait(&self, trait_name: Spur, self_ty_name: Spur) -> bool {
        let trait_info = match self.find_trait(trait_name) {
            Some(info) => info,
            None => return false,
        };
        let trait_methods = &trait_info.methods;
        let default_methods = &trait_info.default_methods;
        #[allow(deprecated)]
        let impl_methods = match self.impl_methods(trait_name, self_ty_name) {
            Some(m) => m,
            None => return false,
        };
        // Every trait method must be in the impl methods OR have a default body.
        // Stage 14.97 (Bug Y1 fix): Methods with default bodies don't need
        // to be overridden in impl blocks.
        trait_methods
            .iter()
            .all(|tm| impl_methods.contains(tm) || default_methods.contains(tm))
    }

    /// Stage 5.19: Get the trait methods missing from an impl.
    ///
    /// Returns a Vec of method name Spurs that are declared in the trait
    /// but not implemented in the impl block. Empty Vec means the impl
    /// is complete (or no trait/impl exists).
    ///
    /// Per API-naming-standard §3: `missing_impl_methods` follows
    /// `<adjective>_<noun>_<noun>` pattern for collection-returning queries.
    pub fn missing_impl_methods(&self, trait_name: Spur, self_ty_name: Spur) -> Vec<Spur> {
        let trait_info = match self.find_trait(trait_name) {
            Some(info) => info,
            None => return Vec::new(),
        };
        let trait_methods = &trait_info.methods;
        let default_methods = &trait_info.default_methods;
        #[allow(deprecated)]
        let impl_methods = match self.impl_methods(trait_name, self_ty_name) {
            Some(m) => m,
            None => return Vec::new(),
        };
        // Stage 14.97 (Bug Y1 fix): Skip methods that have default bodies.
        trait_methods
            .iter()
            .filter(|tm| !impl_methods.contains(tm) && !default_methods.contains(tm))
            .copied()
            .collect()
    }

    /// Stage 5.19: Get the count of missing methods in an impl.
    ///
    /// Per API-naming-standard §3: `missing_method_count` follows
    /// `<noun>_count` pattern consistent with `method_count_for_trait`.
    pub fn missing_method_count(&self, trait_name: Spur, self_ty_name: Spur) -> usize {
        self.missing_impl_methods(trait_name, self_ty_name).len()
    }

    /// Stage 5.20: Validate all trait impls — runs coherence check (Stage
    /// 5.18) + completeness check (Stage 5.19) across all impls and
    /// returns a single `ImplValidationReport`.
    ///
    /// This is the single entry point for "are all impls OK?" — the driver
    /// can call this once after `collect()` to get a comprehensive report.
    ///
    /// Per API-naming-standard §3: `validate_impls` follows `validate_<noun>`
    /// pattern consistent with `check_coherence` (verb-first for action methods).
    pub fn validate_impls(&self) -> ImplValidationReport {
        let coherence_errors = self.check_coherence();

        // Check completeness for every (trait, type) pair that has an impl
        let mut incomplete_impls: Vec<IncompleteImpl> = Vec::new();
        for impl_info in self.impls.values() {
            if let (Some(trait_name), Some(self_ty_name)) =
                (impl_info.trait_name, impl_info.self_ty_name)
            {
                let missing = self.missing_impl_methods(trait_name, self_ty_name);
                if !missing.is_empty() {
                    incomplete_impls.push(IncompleteImpl {
                        trait_name,
                        self_ty_name,
                        missing_methods: missing,
                        // Stage 15.89: store the impl block's source span
                        // for accurate error reporting.
                        span: impl_info.span,
                    });
                }
            }
        }

        let is_valid = coherence_errors.is_empty() && incomplete_impls.is_empty();

        ImplValidationReport {
            coherence_errors,
            incomplete_impls,
            is_valid,
        }
    }

    /// Stage 5.20: Quick boolean check — are all impls valid (no coherence
    /// errors + no incomplete impls)?
    ///
    /// Per API-naming-standard §3: `impls_are_valid` follows
    /// `<noun>_are_<adj>` pattern for boolean aggregate queries.
    pub fn impls_are_valid(&self) -> bool {
        self.coherence_error_count() == 0 && self.all_impls_complete()
    }

    /// Stage 5.20: Check if all impls are complete (no missing methods).
    /// Returns `false` if any impl is missing trait methods.
    ///
    /// Per API-naming-standard §3: `all_impls_complete` follows
    /// `all_<noun>_<adj>` pattern for boolean aggregate queries.
    pub fn all_impls_complete(&self) -> bool {
        for impl_info in self.impls.values() {
            if let (Some(trait_name), Some(self_ty_name)) =
                (impl_info.trait_name, impl_info.self_ty_name)
            {
                if !self.impl_covers_trait(trait_name, self_ty_name) {
                    return false;
                }
            }
        }
        true
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

/// Stage 16.06: Check if a HIR type kind is a Copy candidate.
///
/// Used by `TraitResolver::derive_copy_types` to determine if a struct
/// field's type is Copy. This is a conservative check — it returns `true`
/// only for types that are definitely Copy:
/// - Primitives: bool, char, int, uint, float, never
/// - References (shared refs are Copy)
/// - Raw pointers
/// - Function pointers
/// - Tuples of Copy candidates
/// - Arrays of Copy candidates
/// - Structs that are already in `derived_copy_types` or
///   `explicit_copy_def_ids`
///
/// Returns `false` for:
/// - Slices (unsized)
/// - Trait objects (dyn Trait)
/// - Closures
/// - Infer (unknown — conservative false)
/// - Paths that don't resolve to a known DefId (conservative false)
///
/// Per §1.0 原則 9 "正确 > 妥协": conservative false is sound — a false
/// negative just means a struct won't be derived Copy (user can add
/// explicit `impl Copy`), while a false positive would be unsound.
/// Per §23: `hir_ty_is_copy_candidate` follows `<noun>_<verb>_<noun>`
/// pattern for predicate functions.
fn hir_ty_is_copy_candidate(
    kind: &crate::hir::HirTyKind,
    derived_copy_types: &std::collections::HashSet<DefId>,
    explicit_copy_def_ids: &std::collections::HashSet<DefId>,
) -> bool {
    use crate::hir::HirTyKind;
    match kind {
        HirTyKind::Bool
        | HirTyKind::Char
        | HirTyKind::Int(_)
        | HirTyKind::Uint(_)
        | HirTyKind::Float(_)
        | HirTyKind::Never => true,
        HirTyKind::Ref(_, _, _) => true,
        HirTyKind::Ptr(_, _) => true,
        HirTyKind::FnPtr { .. } => true,
        HirTyKind::Tuple(tys) => tys
            .iter()
            .all(|t| hir_ty_is_copy_candidate(&t.kind, derived_copy_types, explicit_copy_def_ids)),
        HirTyKind::Array(inner, _) => {
            hir_ty_is_copy_candidate(&inner.kind, derived_copy_types, explicit_copy_def_ids)
        }
        // Path: check if it resolves to a DefId that's derived or explicit Copy.
        HirTyKind::Path(_, path) => {
            use crate::hir::Res;
            match path.res {
                Res::Def(def_id, _) => {
                    derived_copy_types.contains(&def_id) || explicit_copy_def_ids.contains(&def_id)
                }
                // Unresolved or non-def paths: conservative false.
                _ => false,
            }
        }
        // Conservative false for unsized/unknown types.
        HirTyKind::Slice(_)
        | HirTyKind::TraitObject { .. }
        | HirTyKind::ImplTrait(_)
        | HirTyKind::Infer => false,
    }
}
