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
        "()" => StdlibTypeKind::Unit,
        "Never" => StdlibTypeKind::Never,
        // Alloc types
        "Box" | "Vec" | "String" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Rc"
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
                    stdlib_impl_method_symbol(type_name, entry.method_name)
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
