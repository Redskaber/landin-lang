//! Stdlib facade: type system, trait method registry, vtable layout.
//!
//! Stage 6.9 architectural split: this module is the entry point for the
//! stdlib subsystem, organized into 3 sub-modules by responsibility:
//! - `trait_methods` — trait method signatures + query API (domain B)
//! - `vtable_layout` — vtable layout + symbols + emission (domain C)
//! - `prelude` — Stage 18.165: built-in type injection (Option/Result)
//! - (this file) — type system + prelude + registration (domain A)
//!
//! Per §16: stdlib is self-contained — no mir/codegen/traits references.
//!
//! Standard library MVP — core + alloc type definitions + prelude registration.
//!
//! Stage 5.25: Implements the `core` layer of Landin's three-layer stdlib
//! (core / alloc / std). This module provides:
//! - `StdlibPrelude` — prelude items auto-imported into every Landin module
//! - `register_stdlib()` — register all stdlib types/traits in the interner
//!
//! Stage 5.28: Added `alloc` layer types (Box/Vec/String/HashMap/Rc/Arc)
//! and `fmt` traits (Display/Debug/Write). Extended `register_stdlib()` and
//! `StdlibPrelude` to include alloc-layer items.
//!
//! Per §16: stdlib registration happens in the driver (pre-compilation),
//! using the interner. No HIR access needed.
//!
//! Per API-naming-standard §3: types use `<Noun><Noun>` pattern;
//! methods use `<verb>_<noun>` pattern.

use lasso::Rodeo;

mod trait_methods;
mod vtable_layout;
// Stage 18.165: Built-in prelude type injection (Option/Result).
pub mod prelude;

// Stage 14.4 §23 compliance: explicit re-export lists (no glob `pub use X::*;`).
// Each name below is a public symbol from a sub-module that callers may use
// via `crate::stdlib::<Name>` or `landin_compiler::stdlib::<Name>`.

// From `trait_methods` — trait method signatures + query API (domain B).
pub use trait_methods::{
    find_stdlib_trait_method, is_stdlib_marker_trait, is_stdlib_trait, is_stdlib_trait_method,
    stdlib_all_traits, stdlib_arithmetic_traits, stdlib_core_traits, stdlib_io_traits,
    stdlib_marker_traits, stdlib_trait_count, stdlib_trait_method_count, stdlib_trait_method_index,
    stdlib_trait_method_is_unsafe, stdlib_trait_method_param_count,
    stdlib_trait_method_param_kinds, stdlib_trait_method_return_kind,
    stdlib_trait_method_self_kind, stdlib_trait_methods, stdlib_trait_methods_by_is_unsafe,
    stdlib_trait_methods_by_param_count, stdlib_trait_methods_by_return_kind,
    stdlib_trait_methods_by_self_kind, stdlib_traits_with_method, stdlib_traits_with_vtable,
    stdlib_unary_traits, stdlib_vtable_layout, stdlib_vtable_slot_count, StdlibSelfKind,
    StdlibTraitMethod, StdlibVtableSlot, ARITH_ASSIGN_METHOD_NAMES, ARITH_OP_METHOD_NAMES,
};

// From `vtable_layout` — vtable layout + symbols + emission (domain C).
pub use vtable_layout::{
    stdlib_data_global_name, stdlib_dynptr_global_name, stdlib_impl_method_symbol,
    stdlib_pointer_width_bytes, stdlib_vtable_byte_size, stdlib_vtable_emission,
    stdlib_vtable_emission_summary, stdlib_vtable_emissions_for_traits, stdlib_vtable_global_name,
    stdlib_vtable_method_offset, stdlib_vtable_method_symbols, stdlib_vtable_plan,
    stdlib_vtable_plan_entry_count, stdlib_vtable_plan_is_complete,
    stdlib_vtable_plan_missing_methods, StdlibPointerWidth, StdlibVtableEmission,
    StdlibVtableEmissionSummary, StdlibVtablePlan, StdlibVtablePlanEntry,
};

/// Stage 5.25: Core primitive type names recognized by the compiler.
///
/// These are the types that Landin's `core` layer provides. The compiler
/// knows about them intrinsically — users don't need to define them.
pub const STDLIB_CORE_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool",
    "char", "str", "()", "Never",
];

/// Stage 5.28: Alloc-layer type names — heap-allocated collection types.
pub const STDLIB_ALLOC_TYPES: &[&str] = &[
    "Box",
    "Vec",
    "String",
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "Rc",
    "Arc",
    "Cell",
    "RefCell",
    "LinkedList",
    "VecDeque",
];

/// Stage 5.28: Alloc-layer trait names — formatting + smart-pointer traits.
pub const STDLIB_ALLOC_TRAITS: &[&str] = &[
    "Display",
    "Debug",
    "Write",
    "Formatter",
    "Deref",
    "DerefMut",
    "Default",
    "Hash",
];

/// Stage 5.30: Std-layer type names — OS-dependent I/O + threading types.
pub const STDLIB_STD_TYPES: &[&str] = &[
    "File",
    "Dir",
    "Path",
    "PathBuf",
    "OpenOptions",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "Thread",
    "JoinHandle",
    "Mutex",
    "Condvar",
    "Command",
    "ExitStatus",
    "OsStr",
    "OsString",
    "Stdin",
    "Stdout",
    "Stderr",
    "BufReader",
    "BufWriter",
    "Result",
    "Option",
    "Some",
    "None",
    "Ok",
    "Err",
];

/// Stage 5.30: Std-layer trait names — I/O + error handling traits.
pub const STDLIB_STD_TRAITS: &[&str] =
    &["Read", "Write", "Seek", "BufRead", "Error", "Termination"];

/// Stage 5.25: Core marker trait names (beyond BUILTIN_TRAIT_NAMES).
///
/// These are additional traits in `core::marker` that the compiler
/// recognizes. Note: Copy/Clone/Drop/Sized/Send/Sync/Unpin are already
/// in BUILTIN_TRAIT_NAMES (Stage 5.8). This list adds the remaining
/// marker traits.
pub const STDLIB_MARKER_TRAITS: &[&str] = &[
    "Sized", // already in BUILTIN_TRAIT_NAMES but listed for completeness
    "Unpin", // same
];

/// Stage 5.25: Core ops trait names (operator overloading).
///
/// These are the traits in `core::ops` that enable operator overloading.
/// The compiler recognizes them for future operator trait checking.
pub const STDLIB_OPS_TRAITS: &[&str] = &[
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
    "Neg",
    "Not",
    "PartialEq",
    "PartialOrd",
    "Eq",
    "Ord",
    "Index",
    "IndexMut",
    "Range",
    "RangeInclusive",
];

/// Stage 5.25: Core convert trait names.
pub const STDLIB_CONVERT_TRAITS: &[&str] =
    &["From", "Into", "TryFrom", "TryInto", "AsRef", "AsMut"];

/// Stage 5.25: Core iter trait names.
pub const STDLIB_ITER_TRAITS: &[&str] = &[
    "Iterator",
    "IntoIterator",
    "FromIterator",
    "DoubleEndedIterator",
    "ExactSizeIterator",
];

/// Stage 5.25: All stdlib trait names (marker + ops + convert + iter + alloc + std).
pub fn all_stdlib_trait_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    names.extend_from_slice(STDLIB_MARKER_TRAITS);
    names.extend_from_slice(STDLIB_OPS_TRAITS);
    names.extend_from_slice(STDLIB_CONVERT_TRAITS);
    names.extend_from_slice(STDLIB_ITER_TRAITS);
    names.extend_from_slice(STDLIB_ALLOC_TRAITS);
    names.extend_from_slice(STDLIB_STD_TRAITS);
    names.sort();
    names.dedup();
    names
}

/// Stage 5.25: All stdlib type names (core + alloc + std).
pub fn all_stdlib_type_names() -> Vec<&'static str> {
    let mut names = STDLIB_CORE_TYPES.to_vec();
    names.extend_from_slice(STDLIB_ALLOC_TYPES);
    names.extend_from_slice(STDLIB_STD_TYPES);
    names
}

/// Stage 5.25: Stdlib prelude — items auto-imported into every module.
///
/// Contains the list of type + trait names that the compiler
/// automatically makes available without `use` statements.
#[derive(Debug, Clone)]
pub struct StdlibPrelude {
    /// Type names in the prelude.
    pub types: Vec<&'static str>,
    /// Trait names in the prelude.
    pub traits: Vec<&'static str>,
}

impl Default for StdlibPrelude {
    fn default() -> Self {
        Self {
            types: all_stdlib_type_names(),
            traits: all_stdlib_trait_names(),
        }
    }
}

impl StdlibPrelude {
    /// Stage 5.25: Check if a name is in the prelude.
    ///
    /// Per API-naming-standard §3: `contains` follows standard Rust
    /// collection method naming.
    pub fn contains(&self, name: &str) -> bool {
        self.types.contains(&name) || self.traits.contains(&name)
    }

    /// Stage 5.25: Get total prelude item count.
    pub fn len(&self) -> usize {
        self.types.len() + self.traits.len()
    }

    /// Stage 5.25: Check if prelude is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Stage 5.29: Get the layer of a stdlib name.
    ///
    /// Returns the `StdlibLayer` for a given name — `Core` for primitives,
    /// `Alloc` for heap types, `None` for unknown names.
    ///
    /// Per API-naming-standard §3: `layer_for_name` follows
    /// `<noun>_for_<noun>` pattern for query methods.
    pub fn layer_for_name(&self, name: &str) -> StdlibLayer {
        if STDLIB_CORE_TYPES.contains(&name) {
            StdlibLayer::Core
        } else if STDLIB_ALLOC_TYPES.contains(&name) {
            StdlibLayer::Alloc
        } else if STDLIB_STD_TYPES.contains(&name) {
            StdlibLayer::Std
        } else {
            StdlibLayer::None
        }
    }

    /// Stage 5.29: Get all names in a specific layer.
    ///
    /// Per API-naming-standard §3: `names_for_layer` follows
    /// `<noun>_for_<noun>` pattern.
    pub fn names_for_layer(&self, layer: StdlibLayer) -> Vec<&'static str> {
        match layer {
            StdlibLayer::Core => STDLIB_CORE_TYPES.to_vec(),
            StdlibLayer::Alloc => STDLIB_ALLOC_TYPES.to_vec(),
            StdlibLayer::Std => STDLIB_STD_TYPES.to_vec(),
            StdlibLayer::None => Vec::new(),
        }
    }
}

/// Stage 5.29: Stdlib layer — distinguishes core/alloc/std layers.
///
/// Per API-naming-standard §3: `StdlibLayer` follows `<Noun><Noun>` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibLayer {
    /// Core layer — primitive types + marker/ops/convert/iter traits.
    Core,
    /// Alloc layer — heap types (Box/Vec/String/...) + fmt/Deref traits.
    Alloc,
    /// Stage 5.30: Std layer — OS-dependent types (File/Path/TcpStream/...) + I/O traits.
    Std,
    /// Not a stdlib item.
    None,
}

/// Stage 5.25: Register all stdlib types + traits in the interner.
///
/// Called by the driver before `collect()` to ensure all stdlib names
/// are interned. This allows the compiler to recognize `i32`, `Add`,
/// `From`, etc. without user definitions.
///
/// Per §16: driver-level pre-computation using &mut Rodeo.
/// Per API-naming-standard §3: `register_stdlib` follows
/// `register_<noun>` pattern consistent with `register_builtin_traits`.
pub fn register_stdlib(interner: &mut Rodeo) {
    // Register core types
    for &name in STDLIB_CORE_TYPES {
        interner.get_or_intern(name);
    }
    // Stage 5.28: Register alloc types
    for &name in STDLIB_ALLOC_TYPES {
        interner.get_or_intern(name);
    }
    // Stage 5.30: Register std types
    for &name in STDLIB_STD_TYPES {
        interner.get_or_intern(name);
    }
    // Register ops/convert/iter/alloc/std traits
    for name in all_stdlib_trait_names() {
        interner.get_or_intern(name);
    }
}

/// Stage 5.25: Get the default stdlib prelude.
///
/// Per API-naming-standard §3: `default_prelude` follows
/// `<adj>_<noun>` pattern for accessor functions returning defaults.
pub fn default_prelude() -> StdlibPrelude {
    StdlibPrelude::default()
}

/// Stage 5.31: Stdlib facade — aggregate statistics + layer queries.
///
/// Provides a unified interface for querying stdlib composition: how many
/// types/traits per layer, total counts, and whether a name is stdlib-provided.
///
/// Per API-naming-standard §3: `StdlibFacade` follows `<Noun><Noun>` pattern;
/// methods use `<noun>_count` / `<noun>_for_<noun>` patterns.
#[derive(Debug, Clone)]
pub struct StdlibFacade {
    /// The prelude backing this facade.
    pub prelude: StdlibPrelude,
}

impl Default for StdlibFacade {
    fn default() -> Self {
        Self {
            prelude: default_prelude(),
        }
    }
}

impl StdlibFacade {
    /// Stage 5.31: Create a new StdlibFacade from a prelude.
    pub fn from_prelude(prelude: StdlibPrelude) -> Self {
        Self { prelude }
    }

    /// Stage 5.31: Get the total type count across all layers.
    pub fn type_count(&self) -> usize {
        all_stdlib_type_names().len()
    }

    /// Stage 5.31: Get the total trait count across all layers.
    pub fn trait_count(&self) -> usize {
        all_stdlib_trait_names().len()
    }

    /// Stage 5.31: Get the type count for a specific layer.
    pub fn type_count_for_layer(&self, layer: StdlibLayer) -> usize {
        self.prelude.names_for_layer(layer).len()
    }

    /// Stage 5.31: Get the number of stdlib layers (always 3: Core, Alloc, Std).
    pub fn layer_count(&self) -> usize {
        3
    }

    /// Stage 5.31: Check if a name is stdlib-provided (any layer).
    pub fn is_stdlib_name(&self, name: &str) -> bool {
        self.prelude.layer_for_name(name) != StdlibLayer::None
            || all_stdlib_trait_names().contains(&name)
    }

    /// Stage 5.31: Get a summary string of the facade state.
    pub fn summary(&self) -> String {
        format!(
            "StdlibFacade:\n  layers: {}\n  total types: {}\n  total traits: {}\n  core types: {}\n  alloc types: {}\n  std types: {}\n",
            self.layer_count(),
            self.type_count(),
            self.trait_count(),
            self.type_count_for_layer(StdlibLayer::Core),
            self.type_count_for_layer(StdlibLayer::Alloc),
            self.type_count_for_layer(StdlibLayer::Std),
        )
    }
}

/// Stage 5.34: Stdlib type kind — simplified representation of stdlib types.
///
/// Maps stdlib type name strings (like "i32", "bool", "Vec") to a simple
/// enum that the compiler can use for type resolution without depending on
/// `mir::ty` (avoids circular dependency).
///
/// Per API-naming-standard §3: `StdlibTypeKind` follows `<Noun><Noun>` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibTypeKind {
    // Core integer types
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    // Core float types
    F32,
    F64,
    // Core other primitives
    Bool,
    Char,
    Str,
    Unit,
    Never,
    // Alloc types (represented as Adt with a name)
    AllocType,
    // Std types (represented as Adt with a name)
    StdType,
    // Not a stdlib type
    Unknown,
}

/// Stage 5.34: Resolve a stdlib type name to its `StdlibTypeKind`.
///
/// Given a string like "i32", "bool", "Vec", returns the corresponding
/// `StdlibTypeKind`. Returns `Unknown` for non-stdlib names.
///
/// Per API-naming-standard §3: `resolve_stdlib_type` follows
/// `resolve_<noun>_<noun>` pattern for resolution queries.
pub fn resolve_stdlib_type(name: &str) -> StdlibTypeKind {
    match name {
        // Core integers
        "i8" => StdlibTypeKind::I8,
        "i16" => StdlibTypeKind::I16,
        "i32" => StdlibTypeKind::I32,
        "i64" => StdlibTypeKind::I64,
        "i128" => StdlibTypeKind::I128,
        "u8" => StdlibTypeKind::U8,
        "u16" => StdlibTypeKind::U16,
        "u32" => StdlibTypeKind::U32,
        "u64" => StdlibTypeKind::U64,
        "u128" => StdlibTypeKind::U128,
        // Core floats
        "f32" => StdlibTypeKind::F32,
        "f64" => StdlibTypeKind::F64,
        // Core other
        "bool" => StdlibTypeKind::Bool,
        "char" => StdlibTypeKind::Char,
        "str" => StdlibTypeKind::Str,
        // Stage 18.180: "String" is no longer a Str alias — it's a real
        // prelude struct (owned heap type). Falls through to AllocType.
        "()" => StdlibTypeKind::Unit,
        "Never" => StdlibTypeKind::Never,
        // Alloc types
        // Stage 18.180: String moved here from the Str alias (it's now a
        // real heap-allocated struct in the prelude).
        "String" | "Box" | "Vec" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Rc"
        | "Arc" | "Cell" | "RefCell" | "LinkedList" | "VecDeque" => StdlibTypeKind::AllocType,
        // Std types
        "File" | "Dir" | "Path" | "PathBuf" | "OpenOptions" | "TcpStream" | "TcpListener"
        | "UdpSocket" | "Thread" | "JoinHandle" | "Mutex" | "Condvar" | "Command"
        | "ExitStatus" | "OsStr" | "OsString" | "Stdin" | "Stdout" | "Stderr" | "BufReader"
        | "BufWriter" | "Result" | "Option" | "Some" | "None" | "Ok" | "Err" => {
            StdlibTypeKind::StdType
        }
        // Unknown
        _ => StdlibTypeKind::Unknown,
    }
}

/// Stage 5.34: Check if a type name is a primitive stdlib type (Core layer).
///
/// Per API-naming-standard §3: `is_primitive_type` follows `is_<adj>_<noun>` pattern.
pub fn is_primitive_type(name: &str) -> bool {
    !matches!(
        resolve_stdlib_type(name),
        StdlibTypeKind::Unknown | StdlibTypeKind::AllocType | StdlibTypeKind::StdType
    )
}

/// Stage 5.34: Get the bit width of an integer type (if applicable).
///
/// Returns `Some(width)` for integer types (e.g. "i32" → 32),
/// `None` for non-integer types.
///
/// Per API-naming-standard §3: `integer_bit_width` follows `<noun>_<noun>` pattern.
pub fn integer_bit_width(name: &str) -> Option<u32> {
    match resolve_stdlib_type(name) {
        StdlibTypeKind::I8 | StdlibTypeKind::U8 => Some(8),
        StdlibTypeKind::I16 | StdlibTypeKind::U16 => Some(16),
        StdlibTypeKind::I32 | StdlibTypeKind::U32 => Some(32),
        StdlibTypeKind::I64 | StdlibTypeKind::U64 => Some(64),
        StdlibTypeKind::I128 | StdlibTypeKind::U128 => Some(128),
        _ => None,
    }
}

/// Stage 5.34: Check if a type name is a signed integer type.
pub fn is_signed_integer(name: &str) -> bool {
    matches!(
        resolve_stdlib_type(name),
        StdlibTypeKind::I8
            | StdlibTypeKind::I16
            | StdlibTypeKind::I32
            | StdlibTypeKind::I64
            | StdlibTypeKind::I128
    )
}

/// Stage 5.34: Check if a type name is an unsigned integer type.
pub fn is_unsigned_integer(name: &str) -> bool {
    matches!(
        resolve_stdlib_type(name),
        StdlibTypeKind::U8
            | StdlibTypeKind::U16
            | StdlibTypeKind::U32
            | StdlibTypeKind::U64
            | StdlibTypeKind::U128
    )
}

/// Stage 5.34: Check if a type name is a floating-point type.
pub fn is_float_type(name: &str) -> bool {
    matches!(
        resolve_stdlib_type(name),
        StdlibTypeKind::F32 | StdlibTypeKind::F64
    )
}

/// Stage 5.35: Get the size in bytes of a primitive stdlib type.
///
/// Returns `Some(bytes)` for primitive types with known sizes:
/// - i8/u8/bool → 1
/// - i16/u16 → 2
/// - i32/u32/f32/char → 4
/// - i64/u64/f64 → 8
/// - i128/u128 → 16
/// - () (unit) → 0
/// - Never → 0 (uninhabited)
///
/// Returns `None` for str (unsized), alloc types, std types, and unknowns.
///
/// Per API-naming-standard §3: `type_size_bytes` follows `<noun>_<noun>`
/// pattern for data-access queries.
pub fn type_size_bytes(name: &str) -> Option<u64> {
    match resolve_stdlib_type(name) {
        StdlibTypeKind::I8 | StdlibTypeKind::U8 | StdlibTypeKind::Bool => Some(1),
        StdlibTypeKind::I16 | StdlibTypeKind::U16 => Some(2),
        StdlibTypeKind::I32 | StdlibTypeKind::U32 | StdlibTypeKind::F32 | StdlibTypeKind::Char => {
            Some(4)
        }
        StdlibTypeKind::I64 | StdlibTypeKind::U64 | StdlibTypeKind::F64 => Some(8),
        StdlibTypeKind::I128 | StdlibTypeKind::U128 => Some(16),
        StdlibTypeKind::Unit | StdlibTypeKind::Never => Some(0),
        // Str is unsized, alloc/std types have variable size
        StdlibTypeKind::Str
        | StdlibTypeKind::AllocType
        | StdlibTypeKind::StdType
        | StdlibTypeKind::Unknown => None,
    }
}

/// Stage 5.35: Get the alignment in bytes of a primitive stdlib type.
///
/// Alignment matches size for primitives (natural alignment).
/// Returns `None` for unsized/unknown types.
///
/// Per API-naming-standard §3: `type_alignment_bytes` follows
/// `<noun>_<noun>` pattern.
pub fn type_alignment_bytes(name: &str) -> Option<u64> {
    type_size_bytes(name)
}

/// Stage 5.35: Check if a type is zero-sized (ZST).
///
/// Returns `true` for `()` and `Never` (size == 0).
///
/// Per API-naming-standard §3: `is_zero_sized_type` follows
/// `is_<adj>_<noun>` pattern.
pub fn is_zero_sized_type(name: &str) -> bool {
    matches!(
        resolve_stdlib_type(name),
        StdlibTypeKind::Unit | StdlibTypeKind::Never
    )
}

/// Stage 5.35: Get a human-readable description of a stdlib type.
///
/// Returns a string like "32-bit signed integer", "64-bit float",
/// "boolean", "UTF-8 string slice (unsized)", etc.
/// Returns `None` for unknown types.
///
/// Per API-naming-standard §3: `type_description` follows
/// `<noun>_<noun>` pattern for accessor functions returning descriptions.
pub fn type_description(name: &str) -> Option<&'static str> {
    match resolve_stdlib_type(name) {
        StdlibTypeKind::I8 => Some("8-bit signed integer"),
        StdlibTypeKind::I16 => Some("16-bit signed integer"),
        StdlibTypeKind::I32 => Some("32-bit signed integer"),
        StdlibTypeKind::I64 => Some("64-bit signed integer"),
        StdlibTypeKind::I128 => Some("128-bit signed integer"),
        StdlibTypeKind::U8 => Some("8-bit unsigned integer"),
        StdlibTypeKind::U16 => Some("16-bit unsigned integer"),
        StdlibTypeKind::U32 => Some("32-bit unsigned integer"),
        StdlibTypeKind::U64 => Some("64-bit unsigned integer"),
        StdlibTypeKind::U128 => Some("128-bit unsigned integer"),
        StdlibTypeKind::F32 => Some("32-bit floating point"),
        StdlibTypeKind::F64 => Some("64-bit floating point"),
        StdlibTypeKind::Bool => Some("boolean"),
        StdlibTypeKind::Char => Some("Unicode scalar value (4 bytes)"),
        StdlibTypeKind::Str => Some("UTF-8 string slice (unsized)"),
        StdlibTypeKind::Unit => Some("unit type (zero-sized)"),
        StdlibTypeKind::Never => Some("never type (uninhabited, zero-sized)"),
        StdlibTypeKind::AllocType => Some("alloc-layer heap type"),
        StdlibTypeKind::StdType => Some("std-layer OS-dependent type"),
        StdlibTypeKind::Unknown => None,
    }
}
