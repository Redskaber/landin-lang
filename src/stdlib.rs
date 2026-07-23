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
/// type kind, and whether the method is `unsafe`.
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

/// Stage 5.36: Marker-trait method table — empty (no methods).
const MARKER_METHODS: &[StdlibTraitMethod] = &[];

/// Stage 5.36: Clone method table.
const CLONE_METHODS: &[StdlibTraitMethod] = &[
    StdlibTraitMethod {
        name: "clone",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 0,
        return_kind: StdlibTypeKind::AllocType, // Self (placeholder: AllocType for Adt-like)
        is_unsafe: false,
    },
    StdlibTraitMethod {
        name: "clone_from",
        self_kind: StdlibSelfKind::SelfByMutRef,
        param_count: 1, // source: &Self
        return_kind: StdlibTypeKind::Unit,
        is_unsafe: false,
    },
];

/// Stage 5.36: Drop method table.
const DROP_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "drop",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::Unit,
    is_unsafe: false,
}];

/// Stage 5.36: Default method table.
const DEFAULT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "default",
    self_kind: StdlibSelfKind::NoSelf,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // Self
    is_unsafe: false,
}];

/// Stage 5.36: Display method table.
const DISPLAY_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "fmt",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                       // f: &mut Formatter
    return_kind: StdlibTypeKind::StdType, // Result<(), Error> → StdType
    is_unsafe: false,
}];

/// Stage 5.36: Debug method table (same shape as Display).
const DEBUG_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "fmt",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,
    return_kind: StdlibTypeKind::StdType,
    is_unsafe: false,
}];

/// Stage 5.36: PartialEq method table.
const PARTIAL_EQ_METHODS: &[StdlibTraitMethod] = &[
    StdlibTraitMethod {
        name: "eq",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 1, // other: &Self
        return_kind: StdlibTypeKind::Bool,
        is_unsafe: false,
    },
    StdlibTraitMethod {
        name: "ne",
        self_kind: StdlibSelfKind::SelfByRef,
        param_count: 1,
        return_kind: StdlibTypeKind::Bool,
        is_unsafe: false,
    },
];

/// Stage 5.36: PartialOrd method table.
const PARTIAL_ORD_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "partial_cmp",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                       // other: &Self
    return_kind: StdlibTypeKind::StdType, // Option<Ordering>
    is_unsafe: false,
}];

/// Stage 5.36: Ord method table.
const ORD_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "cmp",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1,                       // other: &Self
    return_kind: StdlibTypeKind::StdType, // Ordering
    is_unsafe: false,
}];

/// Stage 5.36: Hash method table.
const HASH_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "hash",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 1, // state: &mut Hasher
    return_kind: StdlibTypeKind::Unit,
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
    is_unsafe: false,
}];

/// Stage 5.36: Not (logical/bitwise NOT) method table.
const NOT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "not",
    self_kind: StdlibSelfKind::SelfByValue,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType,
    is_unsafe: false,
}];

/// Stage 5.36: Deref method table.
const DEREF_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "deref",
    self_kind: StdlibSelfKind::SelfByRef,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // &Self::Target
    is_unsafe: false,
}];

/// Stage 5.36: DerefMut method table.
const DEREF_MUT_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "deref_mut",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // &mut Self::Target
    is_unsafe: false,
}];

/// Stage 5.36: IntoIterator method table.
const INTO_ITERATOR_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "into_iter",
    self_kind: StdlibSelfKind::SelfByValue,
    param_count: 0,
    return_kind: StdlibTypeKind::AllocType, // Self::IntoIter
    is_unsafe: false,
}];

/// Stage 5.36: Iterator method table.
const ITERATOR_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "next",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 0,
    return_kind: StdlibTypeKind::StdType, // Option<Self::Item>
    is_unsafe: false,
}];

/// Stage 5.36: Read method table.
const READ_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "read",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 1,                       // buf: &mut [u8]
    return_kind: StdlibTypeKind::StdType, // Result<usize>
    is_unsafe: false,
}];

/// Stage 5.36: Write method table.
const WRITE_METHODS: &[StdlibTraitMethod] = &[StdlibTraitMethod {
    name: "write",
    self_kind: StdlibSelfKind::SelfByMutRef,
    param_count: 1,                       // buf: &[u8]
    return_kind: StdlibTypeKind::StdType, // Result<usize>
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
    /// Complete list of trait names that have entries in
    /// `stdlib_trait_methods()`'s match table.
    ///
    /// Kept in this module (not imported from `traits::builtin`) so that
    /// `stdlib.rs` stays self-contained per §16 (no backwards dependency
    /// on the `traits` module). Synchronized with the match arms in
    /// `stdlib_trait_methods`.
    const ALL_REGISTERED_TRAITS: &[&str] = &[
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
    let mut out: Vec<&'static str> = Vec::new();
    for &trait_name in ALL_REGISTERED_TRAITS {
        if find_stdlib_trait_method(trait_name, method_name).is_some() {
            out.push(trait_name);
        }
    }
    out
}
