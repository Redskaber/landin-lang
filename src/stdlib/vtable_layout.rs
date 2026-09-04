//! Stage 6.9: Stdlib vtable layout planning + symbol generation + emission.
//!
//! Architectural extraction from `stdlib.rs` (TD: stdlib split).
//! Contains vtable slot layout, byte size computation, vtable construction
//! planning, LLVM symbol name generation, and emission aggregation.
//!
//! Per §16: self-contained — uses StdlibTypeKind/StdlibTraitMethod from
//! parent modules but does not reference mir/codegen/traits.

use crate::stdlib::{stdlib_trait_method_index, stdlib_trait_methods, stdlib_vtable_slot_count};

// ============================================================================
// Stage 5.38: Stdlib vtable byte size + pointer-width-aware layout helpers
//
// Translates vtable slot indices into byte offsets — the form codegen
// actually needs for LLVM IR emission:
//   - `alloca [n × i8]` size = `stdlib_vtable_byte_size(trait, width)`
//   - `getelementptr i8, ptr @vtable, i64 offset` offset
//     = `stdlib_vtable_method_offset(trait, method, width)`
//
// Per API-naming-standard §3:
//   - `StdlibPointerWidth` follows `<Noun><Noun><Noun>` pattern.
//   - Variants `Pointer32` / `Pointer64` follow `<Noun><Digits>` pattern.
//   - Query functions follow `<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: uses only `StdlibPointerWidth` (stdlib-internal) + already-existing
// `stdlib_vtable_slot_count` / `stdlib_trait_method_index`. No `mir::ty` /
// `codegen::EmitType` reference, so no circular dependency.
// ============================================================================

/// Stage 5.38: Target pointer width — determines vtable slot byte size.
///
/// Each vtable slot is one function pointer; its byte size equals the
/// target's pointer width.
///
/// Per API-naming-standard §3: `StdlibPointerWidth` follows
/// `<Noun><Noun><Noun>` pattern. Variants follow `<Noun><Digits>` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibPointerWidth {
    /// 32-bit target — 4 bytes per pointer slot.
    Pointer32,
    /// 64-bit target — 8 bytes per pointer slot.
    Pointer64,
}

impl StdlibPointerWidth {
    /// Stage 5.38: Get the byte size of a single pointer slot for this width.
    ///
    /// Returns 4 for `Pointer32`, 8 for `Pointer64`.
    ///
    /// Per API-naming-standard §3: `byte_size` method follows
    /// `<noun>_<noun>` pattern.
    pub const fn byte_size(self) -> u32 {
        match self {
            StdlibPointerWidth::Pointer32 => 4,
            StdlibPointerWidth::Pointer64 => 8,
        }
    }
}

/// Stage 5.38: Free-function form of `StdlibPointerWidth::byte_size`.
///
/// Returns the byte size of a single pointer slot for the given width.
/// Convenience for callers that don't hold a `StdlibPointerWidth` value.
///
/// Per API-naming-standard §3: `stdlib_pointer_width_bytes` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_pointer_width_bytes(width: StdlibPointerWidth) -> u32 {
    width.byte_size()
}

/// Stage 5.38: Get the total byte size of a trait's vtable.
///
/// Returns `slot_count × pointer_width_bytes`. Specifically:
/// - `Some(0)` for marker traits (Copy/Send/Sync/Sized/Unpin/Eq).
/// - `Some(n × width)` for traits with `n` declared methods.
/// - `None` for traits not in the stdlib registry.
///
/// Codegen uses this to size `alloca` / `getelementptr` calculations
/// involving the vtable global.
///
/// Per API-naming-standard §3: `stdlib_vtable_byte_size` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_byte_size(trait_name: &str, width: StdlibPointerWidth) -> Option<u64> {
    let slot_count = stdlib_vtable_slot_count(trait_name)?;
    Some(slot_count as u64 * width.byte_size() as u64)
}

/// Stage 5.38: Get the byte offset of a method within a trait's vtable.
///
/// Returns `slot_index × pointer_width_bytes` if the (trait, method) pair
/// is registered. Returns `None` if:
/// - the trait is not registered
/// - the trait is a marker (no slots at all)
/// - the method name doesn't match any method in the trait
///
/// Codegen uses this to emit
/// `getelementptr i8, ptr @vtable, i64 <offset>` for dyn Trait method calls.
///
/// Per API-naming-standard §3: `stdlib_vtable_method_offset` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_method_offset(
    trait_name: &str,
    method_name: &str,
    width: StdlibPointerWidth,
) -> Option<u64> {
    let slot_index = stdlib_trait_method_index(trait_name, method_name)?;
    Some(slot_index as u64 * width.byte_size() as u64)
}

// ============================================================================
// Stage 5.39: Stdlib vtable construction planner
//
// Combines trait method signatures (Stage 5.36) + slot indexing (Stage 5.37)
// + an impl's provided method names into a single ordered "vtable plan" that
// codegen can consume in one pass:
//   - For each plan entry: if `provided`, emit `@landin_<Type>_<method>` symbol
//   - If `!provided`, emit `null` or a panic stub
//
// This avoids codegen re-deriving slot order / provided-checking logic —
// the planner does it once, purely, with no side effects.
//
// Per API-naming-standard §3:
//   - `StdlibVtablePlan` / `StdlibVtablePlanEntry` follow
//     `<Noun><Noun><Noun>` / `<Noun><Noun><Noun><Noun>` patterns.
//   - Query functions follow `<noun>_<noun>_<noun>` /
//     `<noun>_<noun>_<noun>_<noun>_<noun>` / `<noun>_<noun>_<noun>_<adj>` /
//     `<noun>_<noun>_<noun>_<adj>_<noun>` patterns.
//
// Per §16: uses only `&'static str` + `Vec<>` + scalars — no `mir::ty` /
// `codegen::EmitType` / `traits::TraitResolver` reference, no circular dep.
// ============================================================================

/// Stage 5.39: A single entry in a vtable construction plan.
///
/// Combines a vtable slot index (from Stage 5.37) with the trait-declared
/// method name and a flag indicating whether the impl provides that method.
///
/// Codegen consumes this directly: `provided=true` → fill slot with the
/// impl method's LLVM symbol; `provided=false` → fill with `null` or a
/// panic stub.
///
/// Per API-naming-standard §3: `StdlibVtablePlanEntry` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>` /
/// `<adj>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtablePlanEntry {
    /// 0-based vtable slot index (from `stdlib_trait_method_index`).
    pub slot_index: u32,
    /// Trait-declared method name at this slot.
    pub method_name: &'static str,
    /// Whether the impl provides a method with this name.
    pub provided: bool,
}

/// Stage 5.39: A complete vtable construction plan for a (trait, impl) pair.
///
/// Contains the trait name + an ordered list of `StdlibVtablePlanEntry`.
/// The list is ordered by `slot_index` (0, 1, 2, ...) and is deterministic.
///
/// Per API-naming-standard §3: `StdlibVtablePlan` follows
/// `<Noun><Noun><Noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtablePlan {
    /// The trait this vtable is for.
    pub trait_name: &'static str,
    /// Ordered vtable slot entries.
    pub entries: Vec<StdlibVtablePlanEntry>,
}

impl StdlibVtablePlan {
    /// Stage 5.39: Check if the plan is complete (all slots provided).
    ///
    /// Returns `true` if every entry has `provided == true`. Markers
    /// (empty plan) are vacuously complete.
    ///
    /// Per API-naming-standard §3: `stdlib_vtable_plan_is_complete` (free
    /// fn form) follows `<noun>_<noun>_<noun>_<adj>` pattern. This method
    /// form follows `<noun>_<adj>` pattern.
    pub fn is_complete(&self) -> bool {
        self.entries.iter().all(|e| e.provided)
    }

    /// Stage 5.39: Get the list of method names not provided by the impl.
    ///
    /// Returns trait-declared method names where `provided == false`,
    /// in slot-index order. Empty if the plan is complete.
    ///
    /// Per API-naming-standard §3: `stdlib_vtable_plan_missing_methods`
    /// (free fn form) follows `<noun>_<noun>_<noun>_<adj>_<noun>` pattern.
    /// This method form follows `<adj>_<noun>` pattern (missing_methods).
    pub fn missing_methods(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|e| !e.provided)
            .map(|e| e.method_name)
            .collect()
    }
}

/// Stage 5.39: Build a vtable construction plan for a (trait, impl) pair.
///
/// Given a trait name and a slice of method names the impl provides,
/// returns `Some(StdlibVtablePlan)` with one entry per trait-declared
/// method (in slot-index order). Each entry's `provided` flag is set
/// based on whether `provided_method_names` contains the method name.
///
/// Returns:
/// - `Some(plan)` with `entries: vec![]` for marker traits (no methods).
/// - `Some(plan)` with `entries: [...]` for traits with methods.
/// - `None` for traits not in the stdlib registry.
///
/// Extra names in `provided_method_names` that don't match any
/// trait-declared method are silently ignored (they don't affect the plan).
///
/// Per API-naming-standard §3: `stdlib_vtable_plan` follows
/// `<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_plan(
    trait_name: &str,
    provided_method_names: &[&str],
) -> Option<StdlibVtablePlan> {
    // Look up the static trait name. For marker traits and known traits,
    // we need to return a `&'static str` for `StdlibVtablePlan.trait_name`.
    // Use the `ALL_REGISTERED_TRAITS` list to validate.
    let static_trait_name: &'static str = {
        /// Local copy of the registered-traits list (same as in
        /// `stdlib_traits_with_method` / `stdlib_traits_with_vtable` —
        /// duplicated per §16 to keep stdlib.rs self-contained).
        const ALL_REGISTERED_TRAITS: &[&str] = &[
            "Copy",
            "Send",
            "Sync",
            "Sized",
            "Unpin",
            "Eq",
            "Clone",
            "Drop",
            "Default",
            "Display",
            "Debug",
            "PartialEq",
            "PartialOrd",
            "Ord",
            "Hash",
            "Deref",
            "DerefMut",
            "IntoIterator",
            "Iterator",
            "Read",
            "Write",
            "Neg",
            "Not",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "BitAnd",
            "BitOr",
            "BitXor",
            "Shl",
            "Shr",
            "AddAssign",
            "SubAssign",
            "MulAssign",
            "DivAssign",
            "RemAssign",
            "BitAndAssign",
            "BitOrAssign",
            "BitXorAssign",
            "ShlAssign",
            "ShrAssign",
        ];
        ALL_REGISTERED_TRAITS
            .iter()
            .copied()
            .find(|&n| n == trait_name)?
    };

    let methods = stdlib_trait_methods(static_trait_name)?;
    let entries = methods
        .iter()
        .enumerate()
        .map(|(idx, m)| StdlibVtablePlanEntry {
            slot_index: idx as u32,
            method_name: m.name,
            provided: provided_method_names.contains(&m.name),
        })
        .collect();
    Some(StdlibVtablePlan {
        trait_name: static_trait_name,
        entries,
    })
}

/// Stage 5.39: Get the total entry count for a trait's vtable plan.
///
/// Returns `Some(n)` where `n` equals `stdlib_vtable_slot_count(trait_name)`
/// (one entry per slot). Returns `None` for unknown traits.
///
/// This is a convenience wrapper — `stdlib_vtable_plan(trait, &[])?.entries.len()`
/// gives the same answer, but this fn avoids allocating the entries Vec.
///
/// Per API-naming-standard §3: `stdlib_vtable_plan_entry_count` follows
/// `<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_plan_entry_count(trait_name: &str) -> Option<u32> {
    stdlib_vtable_slot_count(trait_name)
}

/// Stage 5.39: Free-function form of `StdlibVtablePlan::is_complete`.
///
/// Returns `true` if all entries in the plan have `provided == true`.
/// Markers (empty plan) are vacuously complete.
///
/// Per API-naming-standard §3: `stdlib_vtable_plan_is_complete` follows
/// `<noun>_<noun>_<noun>_<adj>` pattern.
pub fn stdlib_vtable_plan_is_complete(plan: &StdlibVtablePlan) -> bool {
    plan.is_complete()
}

/// Stage 5.39: Free-function form of `StdlibVtablePlan::missing_methods`.
///
/// Returns trait-declared method names where `provided == false`, in
/// slot-index order. Empty if the plan is complete.
///
/// Per API-naming-standard §3: `stdlib_vtable_plan_missing_methods` follows
/// `<noun>_<noun>_<noun>_<adj>_<noun>` pattern.
pub fn stdlib_vtable_plan_missing_methods(plan: &StdlibVtablePlan) -> Vec<&'static str> {
    plan.missing_methods()
}

// ============================================================================
// Stage 5.40: Stdlib vtable symbol name planner
//
// Extracts the LLVM symbol-name formatting logic that codegen currently
// inlines via `format!()` calls into pure stdlib functions. Stage 5.41+
// will replace codegen's `format!` calls with these planner functions —
// behavior-equivalent, but string logic centralized for future naming
// convention changes (e.g. adding module-path prefixes).
//
// Existing codegen conventions (must match byte-for-byte):
//   - impl method symbol: `format!("landin_{}_{}", type, method)` → `landin_S_bar`
//   - vtable global name: `format!(".vtable.{}.{}", trait, type)` → `.vtable.Foo.S`
//   - dynptr global name: `format!(".dynptr.{}.{}", trait, type)` → `.dynptr.Foo.S`
//   - data global name:   `format!(".data.{}", type)`              → `.data.S`
//
// Per API-naming-standard §3:
//   - All 5 new functions follow `<noun>_<noun>_<adj>_<noun>` or
//     `<noun>_<noun>_<noun>_<noun>` patterns.
//
// Per §16: pure functions, input &str, output String / Vec<String> — no
// mir::ty / codegen::EmitType / traits::TraitResolver reference, no
// circular dependency.
// ============================================================================

/// Stage 5.40: Build the LLVM global name for a trait's vtable.
///
/// Returns `format!(".vtable.{}.{}", trait_name, type_name)` — e.g.
/// `.vtable.Foo.S`. This matches the existing codegen convention in
/// `src/codegen/mod.rs:145` byte-for-byte.
///
/// Per API-naming-standard §3: `stdlib_vtable_global_name` follows
/// `<noun>_<noun>_<adj>_<noun>` pattern.
pub fn stdlib_vtable_global_name(trait_name: &str, type_name: &str) -> String {
    format!(".vtable.{trait_name}.{type_name}")
}

/// Stage 5.40: Build the LLVM global name for a (trait, type) dyn pointer.
///
/// Returns `format!(".dynptr.{}.{}", trait_name, type_name)` — e.g.
/// `.dynptr.Foo.S`. Matches `src/codegen/mod.rs:184` byte-for-byte.
///
/// Per API-naming-standard §3: `stdlib_dynptr_global_name` follows
/// `<noun>_<noun>_<adj>_<noun>` pattern.
pub fn stdlib_dynptr_global_name(trait_name: &str, type_name: &str) -> String {
    format!(".dynptr.{trait_name}.{type_name}")
}

/// Stage 5.40: Build the LLVM global name for a type's data global.
///
/// Returns `format!(".data.{}", type_name)` — e.g. `.data.S`. Matches
/// the codegen convention referenced in `src/codegen/mod.rs:162` and
/// `src/codegen/text_emitter.rs:565`.
///
/// Per API-naming-standard §3: `stdlib_data_global_name` follows
/// `<noun>_<noun>_<adj>_<noun>` pattern.
pub fn stdlib_data_global_name(type_name: &str) -> String {
    format!(".data.{type_name}")
}

/// Stage 5.40: Build the LLVM symbol name for an impl method.
///
/// Returns `format!("landin_{}_{}", type_name, method_name)` — e.g.
/// `landin_S_bar`. Matches `src/traits/resolver.rs:235` byte-for-byte.
///
/// Per API-naming-standard §3: `stdlib_impl_method_symbol` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_impl_method_symbol(type_name: &str, method_name: &str) -> String {
    // Stage 98 (v0.9): Include trait name in mangled name to avoid collisions.
    // Old: landin_{type}_{method}  (e.g., landin_i32_fmt)
    // New: landin_{trait}_{type}_{method}  (e.g., landin_Display_i32_fmt)
    // Note: This function doesn't have access to trait_name — it's called
    // from stdlib_vtable_method_symbols which does. The trait name is
    // passed through there.
    // For backward compat: this function still produces old-style mangling.
    // The caller (stdlib_vtable_method_symbols) now overrides with trait name.
    format!("landin_{type_name}_{method_name}")
}

/// Stage 5.40: Build the ordered list of LLVM symbol strings for a vtable.
///
/// Given a (trait, type, provided_methods) triple, returns `Some(Vec<String>)`
/// where each entry corresponds to a vtable slot in slot-index order:
/// - `provided=true`  → `stdlib_impl_method_symbol(type, method)` (e.g. `landin_S_clone`)
/// - `provided=false` → `"null"` (codegen emits this literally as the null pointer)
///
/// Returns `None` if the trait is not in the stdlib registry. Markers
/// (Copy/Send/Sync/Sized/Unpin/Eq) return `Some(vec![])` (empty vtable).
///
/// Codegen consumes this list directly to emit
/// `@.vtable.<trait>.<type> = private unnamed_addr constant [n x ptr] [...]`.
///
/// Per API-naming-standard §3: `stdlib_vtable_method_symbols` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_method_symbols(
    trait_name: &str,
    type_name: &str,
    provided_method_names: &[&str],
) -> Option<Vec<String>> {
    let plan = stdlib_vtable_plan(trait_name, provided_method_names)?;
    Some(
        plan.entries
            .iter()
            .map(|entry| {
                if entry.provided {
                    // Stage 98 (v0.9): Include trait name in mangled symbol
                    // to match the new driver_codegen_prep.rs + resolver.rs
                    // mangling scheme (landin_{trait}_{type}_{method}).
                    format!("landin_{}_{}_{}", trait_name, type_name, entry.method_name)
                } else {
                    "null".to_string()
                }
            })
            .collect(),
    )
}

// ============================================================================
// Stage 5.41: Stdlib vtable emission plan (aggregate structure)
//
// Single-call aggregate that returns everything codegen needs to emit
// `@.vtable.<trait>.<type>` global:
//   - global_name (".vtable.<trait>.<type>")
//   - method_symbols (Vec<String> — "landin_T_m" or "null" per slot)
//   - slot_count (u32)
//   - byte_size_32 / byte_size_64 (u64 — for 32/64-bit targets)
//   - is_marker (true if slot_count == 0)
//   - is_complete (true if all slots provided)
//
// Stage 5.42+ will replace codegen's inline format! calls + separate stdlib
// queries with a single `stdlib_vtable_emission()` call that returns this
// struct. Codegen becomes simpler: one function call, one struct, direct
// field access.
//
// Per API-naming-standard §3:
//   - `StdlibVtableEmission` follows `<Noun><Noun><Noun>` pattern.
//   - Query functions follow `<noun>_<noun>_<noun>` /
//     `<noun>_<noun>_<noun>_<prep>_<noun>` patterns.
//
// Per §16: uses only String + Vec<String> + scalars — no `mir::ty` /
// `codegen::EmitType` / `traits::TraitResolver` reference, no circular dep.
// ============================================================================

/// Stage 5.41: Everything codegen needs to emit one `@.vtable.<trait>.<type>` global.
///
/// Returned by `stdlib_vtable_emission()`. Codegen consumes this struct
/// directly to emit the vtable global — no need to call 5 separate stdlib
/// functions.
///
/// Per API-naming-standard §3: `StdlibVtableEmission` follows
/// `<Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>` /
/// `<noun>_<noun>_<digits>` / `is_<adj>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtableEmission {
    /// The trait this vtable is for (static string from the stdlib registry).
    pub trait_name: &'static str,
    /// The implementing type name (caller-provided).
    pub type_name: String,
    /// LLVM global name: `format!(".vtable.{trait}.{type}")`.
    pub global_name: String,
    /// Ordered method symbol list — each entry is either
    /// `format!("landin_{type}_{method}")` (provided) or `"null"` (missing).
    /// Codegen emits this directly as the `[n x ptr] [...]` initializer.
    pub method_symbols: Vec<String>,
    /// Number of vtable slots (= `method_symbols.len()`).
    pub slot_count: u32,
    /// Vtable byte size on a 32-bit target (= `slot_count × 4`).
    pub byte_size_32: u64,
    /// Vtable byte size on a 64-bit target (= `slot_count × 8`).
    pub byte_size_64: u64,
    /// `true` if the trait is a marker (0 slots — empty vtable).
    pub is_marker: bool,
    /// `true` if all slots are provided (no "null" entries).
    pub is_complete: bool,
}

/// Stage 5.41: Build a complete vtable emission plan for a (trait, type) pair.
///
/// Given a trait name, type name, and the method names the impl provides,
/// returns `Some(StdlibVtableEmission)` containing everything codegen needs
/// to emit the `@.vtable.<trait>.<type>` global in one pass:
/// - `global_name` (from `stdlib_vtable_global_name`)
/// - `method_symbols` (from `stdlib_vtable_method_symbols`)
/// - `slot_count` (= `method_symbols.len()`)
/// - `byte_size_32` / `byte_size_64` (= `slot_count × 4` / `× 8`)
/// - `is_marker` (`true` if `slot_count == 0`)
/// - `is_complete` (`true` if no "null" entries)
///
/// Returns `None` if the trait is not in the stdlib registry.
///
/// Per API-naming-standard §3: `stdlib_vtable_emission` follows
/// `<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_emission(
    trait_name: &str,
    type_name: &str,
    provided_method_names: &[&str],
) -> Option<StdlibVtableEmission> {
    // Resolve the static trait name (mirrors stdlib_vtable_plan logic).
    /// Local copy of the registered-traits list (same as in
    /// `stdlib_vtable_plan` — duplicated per §16 to keep stdlib.rs
    /// self-contained).
    const ALL_REGISTERED_TRAITS: &[&str] = &[
        "Copy",
        "Send",
        "Sync",
        "Sized",
        "Unpin",
        "Eq",
        "Clone",
        "Drop",
        "Default",
        "Display",
        "Debug",
        "PartialEq",
        "PartialOrd",
        "Ord",
        "Hash",
        "Deref",
        "DerefMut",
        "IntoIterator",
        "Iterator",
        "Read",
        "Write",
        "Neg",
        "Not",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "BitAnd",
        "BitOr",
        "BitXor",
        "Shl",
        "Shr",
        "AddAssign",
        "SubAssign",
        "MulAssign",
        "DivAssign",
        "RemAssign",
        "BitAndAssign",
        "BitOrAssign",
        "BitXorAssign",
        "ShlAssign",
        "ShrAssign",
    ];
    let static_trait_name: &'static str = ALL_REGISTERED_TRAITS
        .iter()
        .copied()
        .find(|&n| n == trait_name)?;

    let global_name = stdlib_vtable_global_name(static_trait_name, type_name);
    let method_symbols =
        stdlib_vtable_method_symbols(static_trait_name, type_name, provided_method_names)?;
    let slot_count = method_symbols.len() as u32;
    let byte_size_32 = slot_count as u64 * 4;
    let byte_size_64 = slot_count as u64 * 8;
    let is_marker = slot_count == 0;
    let is_complete = !method_symbols.iter().any(|s| s == "null");

    Some(StdlibVtableEmission {
        trait_name: static_trait_name,
        type_name: type_name.to_string(),
        global_name,
        method_symbols,
        slot_count,
        byte_size_32,
        byte_size_64,
        is_marker,
        is_complete,
    })
}

/// Stage 5.41: Build vtable emission plans for multiple traits on one type.
///
/// Given a slice of trait names, a type name, and the impl's provided method
/// names, returns a `Vec<StdlibVtableEmission>` — one per trait that is
/// registered in the stdlib registry. Unknown traits are silently skipped
/// (no `None` entries in the Vec).
///
/// Useful for codegen when a single type implements multiple stdlib traits
/// (e.g. `struct S` impls `Clone + Drop + Display`).
///
/// Per API-naming-standard §3: `stdlib_vtable_emissions_for_traits` follows
/// `<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn stdlib_vtable_emissions_for_traits(
    trait_names: &[&str],
    type_name: &str,
    provided_method_names: &[&str],
) -> Vec<StdlibVtableEmission> {
    let mut out: Vec<StdlibVtableEmission> = Vec::new();
    for &trait_name in trait_names {
        if let Some(emission) = stdlib_vtable_emission(trait_name, type_name, provided_method_names)
        {
            out.push(emission);
        }
        // Unknown traits are silently skipped — caller may pass a list
        // containing non-stdlib traits (e.g. user-defined traits).
    }
    out
}

// ============================================================================
// Stage 5.42: Stdlib vtable emission summary (project-level aggregate stats)
//
// Aggregates a list of `StdlibVtableEmission` into project-level statistics:
// total emissions, marker count, complete/incomplete counts, total slots,
// total byte sizes (32/64-bit), and deduplicated trait names.
//
// This is the last static-analysis step before codegen modification
// (Stage 5.43+). Codegen will call this after collecting all emissions to
// emit a diagnostic line like "emit N vtables, M bytes total" — useful for
// debugging vtable bloat.
//
// Per API-naming-standard §3:
//   - `StdlibVtableEmissionSummary` follows `<Noun><Noun><Noun><Noun>` pattern.
//   - `stdlib_vtable_emission_summary` follows `<noun>_<noun>_<noun>_<noun>`
//     pattern.
//
// Per §16: uses only &'static str + Vec + scalars — no `mir::ty` /
// `codegen::EmitType` / `traits::TraitResolver` reference, no circular dep.
// ============================================================================

/// Stage 5.42: Project-level vtable emission statistics.
///
/// Aggregates a list of `StdlibVtableEmission` into summary counts and
/// totals. Useful for:
/// - Codegen diagnostics ("emit N vtables, M bytes total")
/// - Detecting vtable bloat (large `total_byte_size_64`)
/// - Finding incomplete impls (`incomplete_count > 0`)
/// - Identifying marker-heavy code (`marker_count` high relative to
///   `total_emissions`)
///
/// Per API-naming-standard §3: `StdlibVtableEmissionSummary` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Field names follow
/// `<adj>_<noun>` / `<noun>_<noun>` / `<noun>_<noun>_<digits>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtableEmissionSummary {
    /// Total number of emissions in the input list.
    pub total_emissions: u32,
    /// Number of emissions that are markers (slot_count == 0).
    pub marker_count: u32,
    /// Number of emissions where all slots are provided (is_complete == true).
    pub complete_count: u32,
    /// Number of emissions with at least one missing slot (is_complete == false).
    pub incomplete_count: u32,
    /// Sum of `slot_count` across all emissions.
    pub total_slots: u32,
    /// Sum of `byte_size_32` across all emissions.
    pub total_byte_size_32: u64,
    /// Sum of `byte_size_64` across all emissions.
    pub total_byte_size_64: u64,
    /// Deduplicated list of trait names involved in the emissions.
    pub trait_names: Vec<&'static str>,
}

/// Stage 5.42: Build a project-level summary from a list of vtable emissions.
///
/// Given a slice of `StdlibVtableEmission`, returns a
/// `StdlibVtableEmissionSummary` aggregating total counts, slot totals,
/// byte-size totals (32/64-bit), and deduplicated trait names.
///
/// Empty input returns a summary with all-zero counts and empty `trait_names`.
///
/// Per API-naming-standard §3: `stdlib_vtable_emission_summary` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_emission_summary(
    emissions: &[StdlibVtableEmission],
) -> StdlibVtableEmissionSummary {
    let total_emissions = emissions.len() as u32;
    let marker_count = emissions.iter().filter(|e| e.is_marker).count() as u32;
    let complete_count = emissions.iter().filter(|e| e.is_complete).count() as u32;
    let incomplete_count = total_emissions - complete_count;
    let total_slots: u32 = emissions.iter().map(|e| e.slot_count).sum();
    let total_byte_size_32: u64 = emissions.iter().map(|e| e.byte_size_32).sum();
    let total_byte_size_64: u64 = emissions.iter().map(|e| e.byte_size_64).sum();

    // Deduplicate trait names while preserving first-seen order.
    let mut trait_names: Vec<&'static str> = Vec::new();
    for e in emissions {
        if !trait_names.contains(&e.trait_name) {
            trait_names.push(e.trait_name);
        }
    }

    StdlibVtableEmissionSummary {
        total_emissions,
        marker_count,
        complete_count,
        incomplete_count,
        total_slots,
        total_byte_size_32,
        total_byte_size_64,
        trait_names,
    }
}
