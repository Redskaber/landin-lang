//! Stage 6.9: Stdlib trait method signatures + query API.
//!
//! Architectural extraction from `stdlib.rs` (TD: stdlib split).
//! Contains the static method signature registry, forward/reverse queries,
//! field accessors, and semantic group queries for stdlib trait methods.
//!
//! Per §16: self-contained — uses StdlibTypeKind from the parent module
//! but does not reference mir/codegen/traits. No circular dependencies.

use crate::stdlib::StdlibTypeKind;

// ============================================================================
// Stage 5.36: Stdlib trait method signatures
//
// Provides a static registry of method signatures for each builtin stdlib
// trait, so that downstream stages (typeck trait-bound solving, dyn Trait
// MIR lowering, vtable codegen) can query "what methods does trait T
// declare, with what self-kind, parameter count, and return type?"
// without re-parsing trait declarations.
//
// Per API-naming-standard §3:
//   - `StdlibTraitMethod` follows `<Noun><Noun><Noun>` pattern.
//   - `StdlibSelfKind` follows `<Noun><Noun><Noun>` pattern.
//   - Query functions follow `<noun>_<noun>` / `<noun>_<noun>_<noun>` /
//     `find_<noun>_<noun>` / `is_<noun>_<noun>` patterns.
//
// Per §16: uses `StdlibTypeKind` (stdlib-internal), no `mir::ty` reference,
// so no circular dependency.
// ============================================================================

/// Stage 5.36: Receiver kind for a trait method.
///
/// Determines how `self` is passed in the vtable function signature.
///
/// Per API-naming-standard §3: `StdlibSelfKind` follows `<Noun><Noun><Noun>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibSelfKind {
    /// `fn(self, ...) -> ...` — by value.
    SelfByValue,
    /// `fn(&self, ...) -> ...` — shared reference.
    SelfByRef,
    /// `fn(&mut self, ...) -> ...` — mutable reference.
    SelfByMutRef,
    /// Associated function (no `self` parameter).
    NoSelf,
}

/// Stage 5.36: A single trait method signature in the stdlib registry.
///
/// Each entry maps a `(trait_name, method_name)` pair to its signature
/// metadata: receiver kind, parameter count (excluding `self`), return
/// type kind, parameter type kinds, and whether the method is `unsafe`.
///
/// Per API-naming-standard §3: `StdlibTraitMethod` follows
/// `<Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>` /
/// `is_<adj>` patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibTraitMethod {
    /// Method name (e.g. "clone", "fmt", "next").
    pub name: &'static str,
    /// How `self` is received.
    pub self_kind: StdlibSelfKind,
    /// Number of parameters excluding `self` (e.g. `fn eq(&self, other: &Self)`
    /// has `param_count == 1`).
    pub param_count: u32,
    /// Return type kind.
    pub return_kind: StdlibTypeKind,
    /// Stage 5.84: Parameter type kinds (excluding `self`). Length matches
    /// `param_count`. Used by codegen to emit precise LLVM arg types for
    /// dyn Trait method calls (TD-016 param refinement).
    pub param_kinds: &'static [StdlibTypeKind],
    /// Whether the method is `unsafe fn`.
    pub is_unsafe: bool,
}

impl StdlibTraitMethod {
    /// Convenience: returns true if the method takes `self` (any kind).
    pub fn has_self(&self) -> bool {
        !matches!(self.self_kind, StdlibSelfKind::NoSelf)
    }
}

// ---------------------------------------------------------------------------
// Static method registry — keyed by trait name.
//
// Marker traits (Copy/Send/Sync/Sized/Unpin) intentionally return
// `Some(&[])` (non-None but zero entries), so callers can distinguish
// "trait is in the stdlib registry but has no methods" from
// "trait is not a stdlib trait at all" (None).
// ---------------------------------------------------------------------------

/// Stage 5.84: Empty param_kinds slice for methods with no parameters.
/// Used as the `param_kinds` value for all `param_count: 0` methods.
const EMPTY_PARAM_KINDS: &[StdlibTypeKind] = &[];

/// Stage 5.86: Complete list of all stdlib trait names (marker + method).
///
/// Extracted from the duplicated local `ALL_REGISTERED_TRAITS` constants in
/// `stdlib_traits_with_method` and `stdlib_traits_with_vtable` to eliminate
/// repetition. Synchronized with the match arms in `stdlib_trait_methods`.
///
/// Per §16: kept in this module (not imported from `traits::builtin`) so
/// that `stdlib.rs` stays self-contained.
const STDLIB_TRAITS: &[&str] = &[
    // Markers (no methods)
    "Copy",
    "Send",
    "Sync",
    "Sized",
    "Unpin",
    "Eq",
    // Core traits
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
    // I/O traits
    "Read",
    "Write",
    // Unary ops
    "Neg",
    "Not",
    // Arithmetic binary ops
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
    // Arithmetic assign ops
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

/// Stage 5.36: Marker-trait method table — empty (no methods).
const MARKER_METHODS: &[StdlibTraitMethod] = &[];

/// Stage 5.36: Clone method table.
const CLONE_METHODS: &[StdlibTraitMethod] = &[
    StdlibTraitMethod {
        name: "clone",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 0,
        return_kind: StdlibTypeKind::AllocType, // Self (placeholder: AllocType for Adt-like)
        param_kinds: EMPTY_PARAM_KINDS,
        is_unsafe: false,
    },
    StdlibTraitMethod {
        name: "clone_from",
        self_kind: StdlibSelfKind::SelfByMutRef,
        param_count: 1, // source: &Self
        return_kind: StdlibTypeKind::Unit,
        param_kinds: &[StdlibTypeKind::AllocType],
        is_unsafe: false,
    },
];

/// Stage 5.36: Drop method table.
const DROP_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "drop",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::Unit,
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Default method table.
const DEFAULT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "default",
    self_kind: StdlibSelfKind::NoSelf,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // Self
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Display method table.
const DISPLAY_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "fmt",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                          // f: &mut Formatter
    return_kind: StdlibTypeKind::StdType,    // Result<(), Error> → StdType
    param_kinds: &[StdlibTypeKind::StdType], // Stage 5.92: Formatter is std type
    is_unsafe: false,
}];

/// Stage 5.36: Debug method table (same shape as Display).
const DEBUG_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "fmt",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,
    return_kind: StdlibTypeKind::StdType,
    param_kinds: &[StdlibTypeKind::StdType], // Stage 5.92: Formatter is std type
    is_unsafe: false,
}];

/// Stage 5.36: PartialEq method table.
const PARTIAL_EQ_METHODS: &[StdlibTraitMethod] = &[
    StdlibTraitMethod {
        name: "eq",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 1, // other: &Self
        return_kind: StdlibTypeKind::Bool,
        param_kinds: &[StdlibTypeKind::AllocType],
        is_unsafe: false,
    },
    StdlibTraitMethod {
        name: "ne",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 1,
        return_kind: StdlibTypeKind::Bool,
        param_kinds: &[StdlibTypeKind::AllocType],
        is_unsafe: false,
    },
];

/// Stage 5.36: PartialOrd method table.
const PARTIAL_ORD_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "partial_cmp",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                       // other: &Self
    return_kind: StdlibTypeKind::StdType, // Option<Ordering>
    param_kinds: &[StdlibTypeKind::AllocType],
    is_unsafe: false,
}];

/// Stage 5.36: Ord method table.
const ORD_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "cmp",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                       // other: &Self
    return_kind: StdlibTypeKind::StdType, // Ordering
    param_kinds: &[StdlibTypeKind::AllocType],
    is_unsafe: false,
}];

/// Stage 5.36: Hash method table.
const HASH_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "hash",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1, // state: &mut Hasher
    return_kind: StdlibTypeKind::Unit,
    param_kinds: &[StdlibTypeKind::StdType], // Stage 5.92: Hasher is std type
    is_unsafe: false,
}];
// ---------------------------------------------------------------------------
// Per-trait arithmetic tables (each StdlibTraitMethod.name is correct).
// ---------------------------------------------------------------------------

/// Stage 5.36: Method-name override for arithmetic binary traits.
///
/// Each of Add/Sub/Mul/Div/Rem shares the shape
/// `fn(self, rhs: Rhs) -> Self::Output` but differs in the method name.
/// This constant maps trait_name → method_name so that diagnostics and
/// reverse queries can answer "what method name does trait Add declare?".
pub const ARITH_OP_METHOD_NAMES: &[(&str, &str)] = &[
    ("Add", "add"),
    ("Sub", "sub"),
    ("Mul", "mul"),
    ("Div", "div"),
    ("Rem", "rem"),
    ("BitAnd", "bitand"),
    ("BitOr", "bitor"),
    ("BitXor", "bitxor"),
    ("Shl", "shl"),
    ("Shr", "shr"),
];

/// Stage 5.36: Method-name override for arithmetic assign traits.
pub const ARITH_ASSIGN_METHOD_NAMES: &[(&str, &str)] = &[
    ("AddAssign", "add_assign"),
    ("SubAssign", "sub_assign"),
    ("MulAssign", "mul_assign"),
    ("DivAssign", "div_assign"),
    ("RemAssign", "rem_assign"),
    ("BitAndAssign", "bitand_assign"),
    ("BitOrAssign", "bitor_assign"),
    ("BitXorAssign", "bitxor_assign"),
    ("ShlAssign", "shl_assign"),
    ("ShrAssign", "shr_assign"),
];

// Per-op static tables — built at compile time so each method's `name`
// field is correct (not "add" with a runtime override).
macro_rules! arith_binary_table {
    ($const_name:ident, $method_name:literal) => {
        const $const_name: &[StdlibTraitMethod] = &[StdlibTraitMethod {
            name: $method_name,
            self_kind: StdlibSelfKind::SelfByValue,
            param_count: 1,                         // rhs: Rhs
            return_kind: StdlibTypeKind::AllocType, // Self::Output (Adt-like)
            param_kinds: &[StdlibTypeKind::AllocType],
            is_unsafe: false,
        }];
    };
}

arith_binary_table!(ADD_METHODS, "add");
arith_binary_table!(SUB_METHODS, "sub");
arith_binary_table!(MUL_METHODS, "mul");
arith_binary_table!(DIV_METHODS, "div");
arith_binary_table!(REM_METHODS, "rem");
arith_binary_table!(BITAND_METHODS, "bitand");
arith_binary_table!(BITOR_METHODS, "bitor");
arith_binary_table!(BITXOR_METHODS, "bitxor");
arith_binary_table!(SHL_METHODS, "shl");
arith_binary_table!(SHR_METHODS, "shr");

macro_rules! arith_assign_table {
    ($const_name:ident, $method_name:literal) => {
        const $const_name: &[StdlibTraitMethod] = &[StdlibTraitMethod {
            name: $method_name,
            self_kind: StdlibSelfKind::SelfByMutRef,
            param_count: 1, // rhs: Rhs
            return_kind: StdlibTypeKind::Unit,
            param_kinds: &[StdlibTypeKind::AllocType],
            is_unsafe: false,
        }];
    };
}

arith_assign_table!(ADD_ASSIGN_METHODS, "add_assign");
arith_assign_table!(SUB_ASSIGN_METHODS, "sub_assign");
arith_assign_table!(MUL_ASSIGN_METHODS, "mul_assign");
arith_assign_table!(DIV_ASSIGN_METHODS, "div_assign");
arith_assign_table!(REM_ASSIGN_METHODS, "rem_assign");
arith_assign_table!(BITAND_ASSIGN_METHODS, "bitand_assign");
arith_assign_table!(BITOR_ASSIGN_METHODS, "bitor_assign");
arith_assign_table!(BITXOR_ASSIGN_METHODS, "bitxor_assign");
arith_assign_table!(SHL_ASSIGN_METHODS, "shl_assign");
arith_assign_table!(SHR_ASSIGN_METHODS, "shr_assign");

/// Stage 5.36: Neg (unary minus) method table.
const NEG_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "neg",
    self_kind: StdlibSelfKind::SelfByValue,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // Self
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Not (logical/bitwise NOT) method table.
const NOT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "not",
    self_kind: StdlibSelfKind::SelfByValue,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType,
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Deref method table.
const DEREF_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "deref",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // &Self::Target
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: DerefMut method table.
const DEREF_MUT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "deref_mut",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // &mut Self::Target
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: IntoIterator method table.
const INTO_ITERATOR_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "into_iter",
    self_kind: StdlibSelfKind::SelfByValue,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // Self::IntoIter
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Iterator method table.
const ITERATOR_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "next",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::StdType, // Option<Self::Item>
    param_kinds: EMPTY_PARAM_KINDS,
    is_unsafe: false,
}];

/// Stage 5.36: Read method table.
const READ_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "read",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 1,                       // buf: &mut [u8]
    return_kind: StdlibTypeKind::StdType, // Result<usize>
    param_kinds: &[StdlibTypeKind::AllocType],
    is_unsafe: false,
}];

/// Stage 5.36: Write method table.
const WRITE_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "write",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 1,                       // buf: &[u8]
    return_kind: StdlibTypeKind::StdType, // Result<usize>
    param_kinds: &[StdlibTypeKind::AllocType],
    is_unsafe: false,
}];

/// Stage 5.36: Lookup the method slice for a stdlib trait by name.
///
/// Returns:
/// - `Some(&[])` for marker traits (Copy/Send/Sync/Sized/Unpin/Eq) — they
///   are in the registry but have no methods.
/// - `Some(&[...])` for traits with known method signatures.
/// - `None` for traits that are not in the stdlib trait registry.
///
/// Per API-naming-standard §3: `stdlib_trait_methods` follows
/// `<noun>_<noun>_<noun>` pattern (stdlib-scoped free-function query).
pub fn stdlib_trait_methods(trait_name: &str) -> Option<&'static [StdlibTraitMethod]> {
    match trait_name {
        // Markers — in registry, no methods
        "Copy" | "Send" | "Sync" | "Sized" | "Unpin" | "Eq" => Some(MARKER_METHODS),
        // Core traits
        "Clone" => Some(CLONE_METHODS),
        "Drop" => Some(DROP_METHODS),
        "Default" => Some(DEFAULT_METHODS),
        "Display" => Some(DISPLAY_METHODS),
        "Debug" => Some(DEBUG_METHODS),
        "PartialEq" => Some(PARTIAL_EQ_METHODS),
        "PartialOrd" => Some(PARTIAL_ORD_METHODS),
        "Ord" => Some(ORD_METHODS),
        "Hash" => Some(HASH_METHODS),
        "Deref" => Some(DEREF_METHODS),
        "DerefMut" => Some(DEREF_MUT_METHODS),
        "IntoIterator" => Some(INTO_ITERATOR_METHODS),
        "Iterator" => Some(ITERATOR_METHODS),
        // I/O traits
        "Read" => Some(READ_METHODS),
        "Write" => Some(WRITE_METHODS),
        // Unary ops
        "Neg" => Some(NEG_METHODS),
        "Not" => Some(NOT_METHODS),
        // Arithmetic binary ops — each trait has its own per-op table
        "Add" => Some(ADD_METHODS),
        "Sub" => Some(SUB_METHODS),
        "Mul" => Some(MUL_METHODS),
        "Div" => Some(DIV_METHODS),
        "Rem" => Some(REM_METHODS),
        "BitAnd" => Some(BITAND_METHODS),
        "BitOr" => Some(BITOR_METHODS),
        "BitXor" => Some(BITXOR_METHODS),
        "Shl" => Some(SHL_METHODS),
        "Shr" => Some(SHR_METHODS),
        // Arithmetic assign ops
        "AddAssign" => Some(ADD_ASSIGN_METHODS),
        "SubAssign" => Some(SUB_ASSIGN_METHODS),
        "MulAssign" => Some(MUL_ASSIGN_METHODS),
        "DivAssign" => Some(DIV_ASSIGN_METHODS),
        "RemAssign" => Some(REM_ASSIGN_METHODS),
        "BitAndAssign" => Some(BITAND_ASSIGN_METHODS),
        "BitOrAssign" => Some(BITOR_ASSIGN_METHODS),
        "BitXorAssign" => Some(BITXOR_ASSIGN_METHODS),
        "ShlAssign" => Some(SHL_ASSIGN_METHODS),
        "ShrAssign" => Some(SHR_ASSIGN_METHODS),
        // Not registered (Fn/FnMut/FnOnce/From/Into/AsRef/...) → None
        _ => None,
    }
}

/// Stage 5.36: Get the method count for a stdlib trait.
///
/// Returns `Some(n)` if the trait is in the registry (n may be 0 for
/// marker traits), `None` if the trait is not a stdlib trait.
///
/// Per API-naming-standard §3: `stdlib_trait_method_count` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_trait_method_count(trait_name: &str) -> Option<usize> {
    stdlib_trait_methods(trait_name).map(|m| m.len())
}

/// Stage 5.36: Find a specific method by name in a stdlib trait.
///
/// Returns `Some(&StdlibTraitMethod)` if the trait is registered and
/// contains a method with the given name, `None` otherwise.
///
/// Per API-naming-standard §3: `find_stdlib_trait_method` follows
/// `find_<noun>_<noun>_<noun>` pattern.
pub fn find_stdlib_trait_method(
    trait_name: &str,
    method_name: &str,
) -> Option<&'static StdlibTraitMethod> {
    stdlib_trait_methods(trait_name)?
        .iter()
        .find(|m| m.name == method_name)
}

/// Stage 5.36: Check whether a (trait, method) pair exists in the stdlib registry.
///
/// Per API-naming-standard §3: `is_stdlib_trait_method` follows
/// `is_<noun>_<noun>_<noun>` pattern.
pub fn is_stdlib_trait_method(trait_name: &str, method_name: &str) -> bool {
    find_stdlib_trait_method(trait_name, method_name).is_some()
}

/// Stage 5.36: Reverse query — find all stdlib traits that declare a method
/// with the given name.
///
/// Useful for diagnostics (e.g. "method `clone` is declared by traits: Clone").
///
/// Iterates the complete set of trait names that are registered in
/// `stdlib_trait_methods()` (which is a superset of
/// `all_stdlib_trait_names()` — it also includes builtin marker traits
/// like Copy/Clone/Drop that live in `traits::builtin::BUILTIN_TRAIT_NAMES`
/// but are duplicated here so `stdlib.rs` stays self-contained per §16).
///
/// Per API-naming-standard §3: `stdlib_traits_with_method` follows
/// `<noun>_<noun>_with_<noun>` pattern.
pub fn stdlib_traits_with_method(method_name: &str) -> Vec<&'static str> {
    // Stage 5.86: uses the module-level STDLIB_TRAITS constant
    // (previously a local ALL_REGISTERED_TRAITS duplicate).
    let mut out: Vec<&'static str> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        if find_stdlib_trait_method(trait_name, method_name).is_some() {
            out.push(trait_name);
        }
    }
    out
}

// ============================================================================
// Stage 5.37: Stdlib vtable slot layout
//
// Assigns each stdlib trait's methods a deterministic 0-based vtable slot
// index, based on the order returned by `stdlib_trait_methods()`. This is
// the last static-prep step before dyn Trait MIR lowering — codegen will
// call these queries to determine:
//   - `@.vtable.<trait>.<type>` global's element count (= slot_count)
//   - the byte offset of a method call (= slot_index × pointer_size)
//
// Per API-naming-standard §3:
//   - `StdlibVtableSlot` follows `<Noun><Noun><Noun>` pattern.
//   - Query functions follow `<noun>_<noun>_<noun>` /
//     `<noun>_<noun>_<noun>_<noun>` / `is_<noun>_<adj>_<noun>` /
//     `<noun>_<noun>_with_<noun>` patterns.
//
// Per §16: uses `StdlibTraitMethod` (stdlib-internal), no `mir::ty` /
// `codegen::EmitType` reference, so no circular dependency.
// ============================================================================

/// Stage 5.37: A single vtable slot description — slot index + method ref.
///
/// Each entry in a trait's vtable layout maps a 0-based index to the
/// corresponding method signature. The index determines the method's
/// byte offset in the vtable global (`index × pointer_size`).
///
/// Per API-naming-standard §3: `StdlibVtableSlot` follows
/// `<Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>` /
/// `<noun>` patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibVtableSlot {
    /// 0-based vtable slot index.
    pub slot_index: u32,
    /// Reference to the method signature at this slot.
    pub method: &'static StdlibTraitMethod,
}

/// Stage 5.37: Look up the vtable slot index for a specific (trait, method) pair.
///
/// Returns:
/// - `Some(slot_index)` if the trait is registered and contains the method.
/// - `None` if the trait is not registered, or the trait is a marker (no
///   methods), or the method name doesn't match any method in the trait.
///
/// The slot index is the position of the method in
/// `stdlib_trait_methods(trait_name)` — deterministic for the lifetime of
/// the process (does not depend on HashMap iteration order).
///
/// Per API-naming-standard §3: `stdlib_trait_method_index` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_trait_method_index(trait_name: &str, method_name: &str) -> Option<u32> {
    let methods = stdlib_trait_methods(trait_name)?;
    methods
        .iter()
        .position(|m| m.name == method_name)
        .map(|idx| idx as u32)
}

/// Stage 5.93: Get the return type kind of a stdlib trait method.
///
/// Convenience accessor — equivalent to
/// `find_stdlib_trait_method(trait_name, method_name).map(|m| m.return_kind)`
/// but more readable at call sites.
///
/// Per API-naming-standard §3: `stdlib_trait_method_return_kind` follows
/// the `<noun>_<noun>_<noun>_<noun>_<noun>` pattern, mirroring
/// `stdlib_trait_method_count` / `stdlib_trait_method_index` from v1.6.
pub fn stdlib_trait_method_return_kind(
    trait_name: &str,
    method_name: &str,
) -> Option<StdlibTypeKind> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.return_kind)
}

/// Stage 5.93: Get the parameter type kinds of a stdlib trait method.
///
/// Convenience accessor — equivalent to
/// `find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_kinds)`
/// but more readable at call sites.
///
/// Per API-naming-standard §3: `stdlib_trait_method_param_kinds` follows
/// the `<noun>_<noun>_<noun>_<noun>_<noun>` pattern (plural), mirroring
/// `stdlib_trait_method_count` / `stdlib_trait_method_index` from v1.6.
pub fn stdlib_trait_method_param_kinds(
    trait_name: &str,
    method_name: &str,
) -> Option<&'static [StdlibTypeKind]> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_kinds)
}

/// Stage 5.94: Get the `self` receiver kind of a stdlib trait method.
///
/// Convenience accessor — equivalent to
/// `find_stdlib_trait_method(trait_name, method_name).map(|m| m.self_kind)`
/// but more readable at call sites.
///
/// Per API-naming-standard §3: `stdlib_trait_method_self_kind` follows
/// the `<noun>_<noun>_<noun>_<noun>_<noun>` pattern, mirroring
/// `stdlib_trait_method_return_kind` from v1.63.
pub fn stdlib_trait_method_self_kind(
    trait_name: &str,
    method_name: &str,
) -> Option<StdlibSelfKind> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.self_kind)
}

/// Stage 5.94: Get the parameter count of a stdlib trait method.
///
/// Convenience accessor — equivalent to
/// `find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_count)`
/// but more readable at call sites.
///
/// Per API-naming-standard §3: `stdlib_trait_method_param_count` follows
/// the `<noun>_<noun>_<noun>_<noun>_<noun>` pattern, mirroring
/// `stdlib_trait_method_return_kind` from v1.63.
pub fn stdlib_trait_method_param_count(trait_name: &str, method_name: &str) -> Option<u32> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.param_count)
}

/// Stage 5.94: Check whether a stdlib trait method is `unsafe`.
///
/// Convenience accessor — equivalent to
/// `find_stdlib_trait_method(trait_name, method_name).map(|m| m.is_unsafe)`
/// but more readable at call sites.
///
/// Per API-naming-standard §3 + §8.1: `stdlib_trait_method_is_unsafe`
/// follows the `<noun>_<noun>_<noun>_<noun>_<is_adj>` pattern
/// (`is_` prefix per §8.1 helper-verb convention).
pub fn stdlib_trait_method_is_unsafe(trait_name: &str, method_name: &str) -> Option<bool> {
    find_stdlib_trait_method(trait_name, method_name).map(|m| m.is_unsafe)
}

/// Stage 5.95: Find all stdlib trait methods with a given `self` receiver kind.
///
/// Returns a `Vec<(&'static str, &'static str)>` of `(trait_name, method_name)`
/// pairs for every stdlib trait method whose `self_kind` matches the given
/// `kind`. This is a **reverse query** — given a self_kind, find all matching
/// methods. Complements `stdlib_trait_method_self_kind` (Stage 5.94, forward
/// query for a single method's self_kind).
///
/// Useful for:
/// - Codegen: find all `SelfByValue` methods (need to copy receiver)
/// - Typeck: validate self kind consistency
/// - Documentation: list methods by receiver type
///
/// Per API-naming-standard §3 + §8.1: `stdlib_trait_methods_by_self_kind`
/// follows the `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern (plural),
/// mirroring `stdlib_traits_with_method` from v1.6. The `_by_self_kind` suffix
/// follows Rust API-guidelines field-filter convention (mirrors
/// `find_dyn_trait_method_call_in_plan_by_method` from v1.47).
pub fn stdlib_trait_methods_by_self_kind(
    kind: StdlibSelfKind,
) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for method in methods {
                if method.self_kind == kind {
                    out.push((trait_name, method.name));
                }
            }
        }
    }
    out
}

/// Stage 5.96: Find all stdlib trait methods with a given return type kind.
///
/// Returns a `Vec<(&'static str, &'static str)>` of `(trait_name, method_name)`
/// pairs for every stdlib trait method whose `return_kind` matches the given
/// `kind`. This is a **reverse query** — given a return_kind, find all matching
/// methods. Complements `stdlib_trait_method_return_kind` (Stage 5.93, forward
/// query for a single method's return_kind).
///
/// Useful for:
/// - Codegen: find all methods returning Bool (need i1 result type)
/// - Codegen: find all methods returning Unit (void call, no result register)
/// - Typeck: validate return type consistency
///
/// Per API-naming-standard §3: `stdlib_trait_methods_by_return_kind` follows
/// the `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern (plural), mirroring
/// `stdlib_trait_methods_by_self_kind` from v1.65.
pub fn stdlib_trait_methods_by_return_kind(
    kind: StdlibTypeKind,
) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for method in methods {
                if method.return_kind == kind {
                    out.push((trait_name, method.name));
                }
            }
        }
    }
    out
}

/// Stage 5.98: Find all stdlib trait methods matching a given `is_unsafe` flag.
///
/// Returns a `Vec<(&'static str, &'static str)>` of `(trait_name, method_name)`
/// pairs for every stdlib trait method whose `is_unsafe` field matches the
/// given `is_unsafe` parameter. This is a **reverse query** — given an unsafe
/// flag, find all matching methods. Complements `stdlib_trait_method_is_unsafe`
/// (Stage 5.94, forward query for a single method's is_unsafe).
///
/// Useful for:
/// - Codegen: find all unsafe methods (need unsafe context)
/// - Typeck: validate unsafe method calls
/// - Safety audit: list all unsafe trait methods
///
/// Per API-naming-standard §3 + §8.1: `stdlib_trait_methods_by_is_unsafe`
/// follows the `<noun>_<noun>_<noun>_<prep>_<is_adj>` pattern (plural),
/// mirroring `stdlib_trait_methods_by_self_kind` from v1.65.
pub fn stdlib_trait_methods_by_is_unsafe(is_unsafe: bool) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for method in methods {
                if method.is_unsafe == is_unsafe {
                    out.push((trait_name, method.name));
                }
            }
        }
    }
    out
}

/// Stage 5.99: Find all stdlib trait methods with a given parameter count.
///
/// Returns a `Vec<(&'static str, &'static str)>` of `(trait_name, method_name)`
/// pairs for every stdlib trait method whose `param_count` matches the given
/// `param_count`. This is a **reverse query** — given a param count, find all
/// matching methods. Complements `stdlib_trait_method_param_count` (Stage 5.94,
/// forward query for a single method's param_count).
///
/// This is the **fourth and final** reverse query dimension, completing the
/// reverse query series (self_kind/return_kind/is_unsafe/param_count).
///
/// Per API-naming-standard §3: `stdlib_trait_methods_by_param_count` follows
/// the `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern (plural), mirroring
/// `stdlib_trait_methods_by_self_kind` from v1.65.
pub fn stdlib_trait_methods_by_param_count(param_count: u32) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for method in methods {
                if method.param_count == param_count {
                    out.push((trait_name, method.name));
                }
            }
        }
    }
    out
}

/// Stage 5.37: Get the complete vtable slot layout for a stdlib trait.
///
/// Returns `Some(Vec<StdlibVtableSlot>)` for any registered trait (including
/// markers, which return an empty Vec), or `None` for unknown traits.
///
/// The returned Vec is ordered by `slot_index` (0, 1, 2, ...) and is
/// deterministic — repeated calls with the same trait name return the
/// same ordering.
///
/// Per API-naming-standard §3: `stdlib_vtable_layout` follows
/// `<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_layout(trait_name: &str) -> Option<Vec<StdlibVtableSlot>> {
    let methods = stdlib_trait_methods(trait_name)?;
    Some(
        methods
            .iter()
            .enumerate()
            .map(|(idx, m)| StdlibVtableSlot {
                slot_index: idx as u32,
                method: m,
            })
            .collect(),
    )
}

/// Stage 5.37: Get the total number of vtable slots for a stdlib trait.
///
/// Returns:
/// - `Some(0)` for marker traits (Copy/Send/Sync/Sized/Unpin/Eq).
/// - `Some(n)` for traits with `n` declared methods.
/// - `None` for traits that are not in the stdlib registry.
///
/// This is the value codegen uses to determine the element count of
/// `@.vtable.<trait>.<type>` global.
///
/// Per API-naming-standard §3: `stdlib_vtable_slot_count` follows
/// `<noun>_<noun>_<noun>_<noun>` pattern.
pub fn stdlib_vtable_slot_count(trait_name: &str) -> Option<u32> {
    stdlib_trait_methods(trait_name).map(|m| m.len() as u32)
}

/// Stage 5.37: Check whether a trait is a marker trait (declares no methods).
///
/// Returns `true` only for traits that are registered in the stdlib
/// registry AND have zero methods (Copy/Send/Sync/Sized/Unpin/Eq).
/// Returns `false` for:
/// - Traits with methods (Clone/Drop/Add/...)
/// - Traits not in the stdlib registry (BogusTrait/From/Into/...)
///
/// Per API-naming-standard §3: `is_stdlib_marker_trait` follows
/// `is_<noun>_<adj>_<noun>` pattern.
pub fn is_stdlib_marker_trait(trait_name: &str) -> bool {
    matches!(
        stdlib_trait_methods(trait_name),
        Some(methods) if methods.is_empty()
    )
}

/// Stage 5.85: Check if a trait name is a stdlib trait (marker or with methods).
///
/// Returns `true` for:
/// - Marker traits: Copy/Send/Sync/Sized/Unpin/Eq
/// - Traits with methods: Clone/Drop/Default/Display/Debug/PartialEq/
///   PartialOrd/Ord/Hash/Deref/DerefMut/IntoIterator/Iterator/Read/Write/
///   Neg/Not/Add/Sub/Mul/Div/Rem/BitAnd/BitOr/BitXor/Shl/Shr/
///   AddAssign/SubAssign/MulAssign/DivAssign/RemAssign/BitAndAssign/
///   BitOrAssign/BitXorAssign/ShlAssign/ShrAssign
///
/// Returns `false` for:
/// - User-defined trait names (Foo/Bar/MyTrait/...)
/// - Empty string
/// - Method names mistakenly passed as trait names ("clone" vs "Clone")
///
/// This is the trait-level membership query, complementing:
/// - `is_stdlib_marker_trait` (marker-only check)
/// - `is_stdlib_trait_method` (method-level check)
///
/// Per API-naming-standard §3 + §8.1: `is_stdlib_trait` follows the
/// `is_<noun>_<noun>` pattern (`is_` prefix per §8.1 helper-verb convention,
/// mirroring `is_stdlib_marker_trait` from v1.6).
pub fn is_stdlib_trait(trait_name: &str) -> bool {
    // A trait is a stdlib trait if it's either a marker trait OR a trait
    // with methods in the registry. Both are captured by stdlib_trait_methods
    // returning Some (marker traits return Some(&[]), method traits return
    // Some(&[...])).
    stdlib_trait_methods(trait_name).is_some()
}

/// Stage 5.37: Get all stdlib traits that have at least one vtable slot
/// (i.e. declare at least one method).
///
/// Marker traits (Copy/Send/Sync/Sized/Unpin/Eq) are excluded — they
/// have empty vtables and don't need a global emitted.
///
/// Useful for codegen: iterate this list to know which traits need
/// `@.vtable.<trait>.<type>` globals emitted for each impl.
///
/// Per API-naming-standard §3: `stdlib_traits_with_vtable` follows
/// `<noun>_<noun>_with_<noun>` pattern.
pub fn stdlib_traits_with_vtable() -> Vec<&'static str> {
    // Stage 5.86: uses the module-level STDLIB_TRAITS constant
    // (previously a local ALL_REGISTERED_TRAITS duplicate).
    let mut out: Vec<&'static str> = Vec::new();
    for &trait_name in STDLIB_TRAITS {
        // Include only traits with at least one method slot.
        if matches!(stdlib_vtable_slot_count(trait_name), Some(n) if n > 0) {
            out.push(trait_name);
        }
    }
    out
}

/// Stage 5.86: Return the total number of stdlib traits (marker + method).
///
/// Convenience function — equivalent to `stdlib_all_traits().len()` but
/// avoids allocating a Vec. Useful for sanity checks, capacity hints, and
/// diagnostic output.
///
/// Per API-naming-standard §3: `stdlib_trait_count` follows the
/// `<noun>_<noun>_<noun>` pattern, mirroring `stdlib_trait_method_count`
/// from v1.6.
pub fn stdlib_trait_count() -> usize {
    STDLIB_TRAITS.len()
}

/// Stage 5.86: Return all stdlib trait names (marker + method).
///
/// Returns a `Vec<&'static str>` containing every trait name registered in
/// the stdlib. This is the **unfiltered** list — includes both marker
/// traits (Copy/Send/Sync/Sized/Unpin/Eq) and traits with methods
/// (Clone/Drop/Display/Add/...).
///
/// Contrast with:
/// - `stdlib_traits_with_vtable` — filtered to traits with at least one method
/// - `stdlib_traits_with_method(name)` — filtered to traits having a specific method
///
/// Per API-naming-standard §3: `stdlib_all_traits` follows the
/// `<noun>_<adj>_<noun>` pattern (`all_` prefix per Rust API-guidelines
/// convention for "return everything" queries).
pub fn stdlib_all_traits() -> Vec<&'static str> {
    STDLIB_TRAITS.to_vec()
}

/// Stage 5.87: Return all stdlib marker trait names.
///
/// Returns a `Vec<&'static str>` containing the names of all stdlib marker
/// traits: Copy/Send/Sync/Sized/Unpin/Eq. Marker traits have no methods
/// (empty vtables) and are used for compile-time type constraints rather
/// than runtime dispatch.
///
/// This is the batch query complement to `is_stdlib_marker_trait` (which
/// checks a single trait). Symmetric with `stdlib_traits_with_vtable`
/// (which returns traits **with** methods).
///
/// Per API-naming-standard §3: `stdlib_marker_traits` follows the
/// `<noun>_<noun>_<noun>` pattern (plural noun), mirroring
/// `stdlib_traits_with_vtable` from v1.7.
pub fn stdlib_marker_traits() -> Vec<&'static str> {
    STDLIB_TRAITS
        .iter()
        .copied()
        .filter(|&name| is_stdlib_marker_trait(name))
        .collect()
}

/// Stage 5.88: Return all stdlib arithmetic operator trait names.
///
/// Returns a `Vec<&'static str>` containing the names of all stdlib
/// arithmetic operator traits — both binary ops and their assign variants:
///
/// - Binary: Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr
/// - Assign: AddAssign, SubAssign, MulAssign, DivAssign, RemAssign,
///   BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign
///
/// This is a **semantic group query** — returns traits that share a
/// semantic category (arithmetic operators). Useful for:
/// - Operator overloading detection
/// - Type inference assistance ("does this type support arithmetic?")
/// - Codegen decisions about operator call emission
///
/// Per API-naming-standard §3: `stdlib_arithmetic_traits` follows the
/// `<noun>_<adj>_<noun>` pattern (plural), mirroring `stdlib_marker_traits`
/// from v1.57.
pub fn stdlib_arithmetic_traits() -> Vec<&'static str> {
    const ARITHMETIC_TRAITS: &[&str] = &[
        // Binary arithmetic ops
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
        // Arithmetic assign ops
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
    ARITHMETIC_TRAITS.to_vec()
}

/// Stage 5.89: Return all stdlib core trait names.
///
/// Returns a `Vec<&'static str>` containing the names of all stdlib core
/// traits — the most commonly used traits for everyday programming:
///
/// - Lifecycle: Clone, Drop, Default
/// - Formatting: Display, Debug
/// - Comparison: PartialEq, PartialOrd, Ord, Hash
/// - Dereference: Deref, DerefMut
/// - Iteration: IntoIterator, Iterator
///
/// This is a **semantic group query** — returns traits that share a
/// semantic category (core programming traits). Useful for:
/// - Type checker: "which core operations does this type support?"
/// - Documentation generators: list core trait impls
/// - Codegen: decide whether to emit runtime support code
///
/// Per API-naming-standard §3: `stdlib_core_traits` follows the
/// `<noun>_<adj>_<noun>` pattern (plural), mirroring
/// `stdlib_arithmetic_traits` from v1.58.
pub fn stdlib_core_traits() -> Vec<&'static str> {
    const CORE_TRAITS: &[&str] = &[
        // Lifecycle
        "Clone",
        "Drop",
        "Default",
        // Formatting
        "Display",
        "Debug",
        // Comparison
        "PartialEq",
        "PartialOrd",
        "Ord",
        "Hash",
        // Dereference
        "Deref",
        "DerefMut",
        // Iteration
        "IntoIterator",
        "Iterator",
    ];
    CORE_TRAITS.to_vec()
}

/// Stage 5.90: Return all stdlib I/O trait names.
///
/// Returns a `Vec<&'static str>` containing the names of all stdlib I/O
/// traits: Read, Write. These traits provide byte-stream I/O operations
/// for types that support reading from / writing to byte streams.
///
/// This is a **semantic group query** — returns traits that share a
/// semantic category (I/O). Useful for:
/// - Detecting which types support I/O operations
/// - Codegen decisions about I/O method emission
///
/// Per API-naming-standard §3: `stdlib_io_traits` follows the
/// `<noun>_<adj>_<noun>` pattern (plural), mirroring
/// `stdlib_core_traits` from v1.59.
pub fn stdlib_io_traits() -> Vec<&'static str> {
    const IO_TRAITS: &[&str] = &["Read", "Write"];
    IO_TRAITS.to_vec()
}

/// Stage 5.90: Return all stdlib unary operator trait names.
///
/// Returns a `Vec<&'static str>` containing the names of all stdlib unary
/// operator traits: Neg, Not. These traits provide unary arithmetic
/// operations (`-x` for Neg, `!x` for Not).
///
/// This is a **semantic group query** — returns traits that share a
/// semantic category (unary operators). Useful for:
/// - Operator overloading detection for unary ops
/// - Type inference assistance ("does this type support negation?")
///
/// Per API-naming-standard §3: `stdlib_unary_traits` follows the
/// `<noun>_<adj>_<noun>` pattern (plural), mirroring
/// `stdlib_arithmetic_traits` from v1.58.
pub fn stdlib_unary_traits() -> Vec<&'static str> {
    const UNARY_TRAITS: &[&str] = &["Neg", "Not"];
    UNARY_TRAITS.to_vec()
}
