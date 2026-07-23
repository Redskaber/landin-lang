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
