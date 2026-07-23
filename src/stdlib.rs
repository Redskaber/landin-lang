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

/// Stage 5.25: All stdlib trait names (marker + ops + convert + iter + alloc).
pub fn all_stdlib_trait_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    names.extend_from_slice(STDLIB_MARKER_TRAITS);
    names.extend_from_slice(STDLIB_OPS_TRAITS);
    names.extend_from_slice(STDLIB_CONVERT_TRAITS);
    names.extend_from_slice(STDLIB_ITER_TRAITS);
    names.extend_from_slice(STDLIB_ALLOC_TRAITS);
    names.sort();
    names.dedup();
    names
}

/// Stage 5.25: All stdlib type names (core primitives + alloc types).
pub fn all_stdlib_type_names() -> Vec<&'static str> {
    let mut names = STDLIB_CORE_TYPES.to_vec();
    names.extend_from_slice(STDLIB_ALLOC_TYPES);
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
    // Register ops/convert/iter/alloc traits
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
