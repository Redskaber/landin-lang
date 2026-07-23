//! Builtin trait registry — standard traits recognized by the compiler.
//!
//! Stage 5.8: BUILTIN_TRAIT_NAMES + BUILTIN_DEF_ID_BASE + register_builtin_traits.
//! Stage 5.11: BUILTIN_PRIMITIVE_COPY_KINDS + is_primitive_copy_kind.

/// Stage 5.8: The set of builtin trait names recognized by the compiler.
pub const BUILTIN_TRAIT_NAMES: &[&str] = &[
    "Copy", "Clone", "Drop", "Sized", "Send", "Sync", "Unpin", "Fn", "FnMut", "FnOnce",
];

/// Stage 5.8: Reserved DefId base for builtin traits.
pub const BUILTIN_DEF_ID_BASE: u32 = u32::MAX;

/// Stage 5.11: Primitive types that are always `Copy` (and `Clone`).
pub const BUILTIN_PRIMITIVE_COPY_KINDS: &[&str] = &[
    "Bool", "Char", "Int", "Uint", "Float", "Never", "Ref", "RawPtr", "FnDef", "FnPtr",
];

/// Stage 5.11: Check if a MIR `TyKind` variant name is always Copy.
pub fn is_primitive_copy_kind(kind_name: &str) -> bool {
    let base = kind_name.split('(').next().unwrap_or(kind_name);
    BUILTIN_PRIMITIVE_COPY_KINDS.contains(&base)
}
