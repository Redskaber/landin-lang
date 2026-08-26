//! TraitResolver queries + diagnostics — extracted from resolver.rs.
//!
//! Stage 18.308 (P3 LOC refactor): per §13.4 J1-J6, split out the
//! pure-query / counting / coherence / validation methods into a
//! separate `impl TraitResolver` block. All methods are read-only
//! queries on `self` (no mutation). TraitResolver's fields are `pub`,
//! so cross-module access works without visibility changes.

// Mirror the imports from resolver.rs (the parent module's `use` statements
// are NOT re-exported by `use super::*;`, so we must list them explicitly).
use crate::hir::*;
use lasso::{Rodeo, Spur};

// Bring in the structs defined in resolver.rs (CoherenceError, ImplValidationReport,
// InherentImplConflict, TraitResolver, etc.) plus any pub items it re-exports.
use super::*;

impl TraitResolver {
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

    /// Stage 18.292 (类 Rust 架构修正): Check for duplicate inherent impl
    /// method definitions — two `impl Type { fn same_method {} }` blocks
    /// with the same method name on the same type.
    ///
    /// 类 Rust 设计: 用户不能覆盖 prelude 定义的原始类型方法。
    /// Rust 报 "duplicate definitions with name `X`" for this case。
    /// Landin 之前静默接受第一个定义, 是 soundness bug。
    ///
    /// **不跳过 marker impl** — prelude 的 `impl str { fn len { loop {} } }`
    /// 与用户的 `impl str { fn len { 42 } }` 冲突 → 报错。
    /// 这是类 Rust 设计: prelude 是权威实现, 用户不能覆盖。
    ///
    /// Per §2 原則 4 (报错>静默): conflicts must be reported。
    /// Per §1.0 原則 6 (通解>特解): one check for all inherent impl conflicts。
    /// Per §12 (最优>最小): 类 Rust — 不允许覆盖, 冲突即报错。
    pub fn check_inherent_impl_conflicts(&self) -> Vec<InherentImplConflict> {
        use std::collections::HashMap as StdHashMap;
        // Group impl DefIds by (self_ty_name, method_name) for inherent impls.
        // Inherent impls have trait_name == None.
        // Stage 18.292: 不跳过 marker impl — 类 Rust 设计, prelude 是权威实现。
        let mut groups: StdHashMap<(Spur, Spur), (Vec<DefId>, crate::session::Span)> =
            StdHashMap::new();
        for impl_info in self.impls.values() {
            // Only check inherent impls (trait_name is None).
            if impl_info.trait_name.is_some() {
                continue;
            }
            // Only check impls with a known self_ty_name.
            let self_ty_name = match impl_info.self_ty_name {
                Some(name) => name,
                None => continue,
            };
            // Check each method in this impl block.
            for &method_name in &impl_info.methods {
                let entry = groups
                    .entry((self_ty_name, method_name))
                    .or_insert_with(|| (Vec::new(), impl_info.span));
                if !entry.0.contains(&impl_info.def_id) {
                    entry.0.push(impl_info.def_id);
                }
            }
        }
        // Any group with >1 distinct impl is a conflict.
        groups
            .into_iter()
            .filter(|(_, (def_ids, _))| def_ids.len() > 1)
            .map(
                |((self_ty_name, method_name), (impl_def_ids, span))| InherentImplConflict {
                    self_ty_name,
                    method_name,
                    impl_def_ids,
                    span,
                },
            )
            .collect()
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
        // Stage 18.64: Inline deprecated impl_methods to remove #[allow(deprecated)].
        let impl_methods = match self
            .impl_by_trait_and_type
            .get(&(trait_name, self_ty_name))
            .and_then(|id| self.impls.get(id))
        {
            Some(i) => &i.methods,
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
        // Stage 18.64: Inline deprecated impl_methods.
        let impl_methods = match self
            .impl_by_trait_and_type
            .get(&(trait_name, self_ty_name))
            .and_then(|id| self.impls.get(id))
        {
            Some(i) => &i.methods,
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
                // Stage 18.73 P1-H: Check for missing associated consts.
                let missing_consts = self.missing_impl_associated_consts(trait_name, self_ty_name);
                if !missing.is_empty() || !missing_consts.is_empty() {
                    incomplete_impls.push(IncompleteImpl {
                        trait_name,
                        self_ty_name,
                        missing_methods: missing,
                        // Stage 15.89: store the impl block's source span
                        // for accurate error reporting.
                        span: impl_info.span,
                        missing_associated_consts: missing_consts,
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

    /// Stage 18.73 P1-H: Get the associated consts missing from an impl.
    ///
    /// Returns a Vec of const name Spurs that are declared in the trait
    /// but not implemented in the impl block.
    ///
    /// Per §1.0 原則 4 "报错 > 静默": missing associated consts must be reported.
    /// Per §10 naming: `missing_impl_associated_consts` follows
    ///   `<adjective>_<noun>_<noun>_<noun>` pattern.
    pub fn missing_impl_associated_consts(
        &self,
        trait_name: Spur,
        self_ty_name: Spur,
    ) -> Vec<Spur> {
        let trait_info = match self.find_trait(trait_name) {
            Some(info) => info,
            None => return Vec::new(),
        };
        let trait_consts = &trait_info.associated_consts;
        // Stage 18.64: Inline deprecated impl_methods pattern.
        let impl_consts = match self
            .impl_by_trait_and_type
            .get(&(trait_name, self_ty_name))
            .and_then(|id| self.impls.get(id))
        {
            Some(i) => &i.associated_consts,
            None => return Vec::new(),
        };
        trait_consts
            .iter()
            .filter(|c| !impl_consts.contains(c))
            .copied()
            .collect()
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
