//! Stage 5.92: param_kinds data accuracy tests
//!
//! Tests that the `param_kinds` field on `StdlibTraitMethod` entries is
//! accurate after the Stage 5.92 data refinement. Specifically verifies
//! that Display::fmt, Debug::fmt, and Hash::hash use `StdType` for their
//! Formatter/Hasher parameters (not `AllocType` as in the Stage 5.84 default).
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::StdlibTypeKind;
use landin_compiler::stdlib_trait_methods;

/// Display::fmt param_kinds is [StdType] (Formatter is std type).
#[test]
fn test_display_fmt_param_kinds_std_type() {
    let methods = stdlib_trait_methods("Display").expect("Display methods");
    let fmt = methods
        .iter()
        .find(|m| m.name == "fmt")
        .expect("fmt method");
    assert_eq!(fmt.param_kinds.len(), 1);
    assert_eq!(
        fmt.param_kinds[0],
        StdlibTypeKind::StdType,
        "Display::fmt param should be StdType (Formatter), got {:?}",
        fmt.param_kinds[0]
    );
}

/// Debug::fmt param_kinds is [StdType] (Formatter is std type).
#[test]
fn test_debug_fmt_param_kinds_std_type() {
    let methods = stdlib_trait_methods("Debug").expect("Debug methods");
    let fmt = methods
        .iter()
        .find(|m| m.name == "fmt")
        .expect("fmt method");
    assert_eq!(fmt.param_kinds.len(), 1);
    assert_eq!(
        fmt.param_kinds[0],
        StdlibTypeKind::StdType,
        "Debug::fmt param should be StdType (Formatter), got {:?}",
        fmt.param_kinds[0]
    );
}

/// Hash::hash param_kinds is [StdType] (Hasher is std type).
#[test]
fn test_hash_hash_param_kinds_std_type() {
    let methods = stdlib_trait_methods("Hash").expect("Hash methods");
    let hash = methods
        .iter()
        .find(|m| m.name == "hash")
        .expect("hash method");
    assert_eq!(hash.param_kinds.len(), 1);
    assert_eq!(
        hash.param_kinds[0],
        StdlibTypeKind::StdType,
        "Hash::hash param should be StdType (Hasher), got {:?}",
        hash.param_kinds[0]
    );
}

/// Clone::clone_from param_kinds is [AllocType] (source: &Self, unchanged).
#[test]
fn test_clone_clone_from_param_kinds_alloc_type() {
    let methods = stdlib_trait_methods("Clone").expect("Clone methods");
    let clone_from = methods
        .iter()
        .find(|m| m.name == "clone_from")
        .expect("clone_from method");
    assert_eq!(clone_from.param_kinds.len(), 1);
    assert_eq!(
        clone_from.param_kinds[0],
        StdlibTypeKind::AllocType,
        "Clone::clone_from param should be AllocType (&Self), got {:?}",
        clone_from.param_kinds[0]
    );
}

/// PartialEq::eq param_kinds is [AllocType] (other: &Self, unchanged).
#[test]
fn test_partial_eq_eq_param_kinds_alloc_type() {
    let methods = stdlib_trait_methods("PartialEq").expect("PartialEq methods");
    let eq = methods.iter().find(|m| m.name == "eq").expect("eq method");
    assert_eq!(eq.param_kinds.len(), 1);
    assert_eq!(
        eq.param_kinds[0],
        StdlibTypeKind::AllocType,
        "PartialEq::eq param should be AllocType (&Self), got {:?}",
        eq.param_kinds[0]
    );
}

/// PartialOrd::partial_cmp param_kinds is [AllocType] (other: &Self, unchanged).
#[test]
fn test_partial_ord_partial_cmp_param_kinds_alloc_type() {
    let methods = stdlib_trait_methods("PartialOrd").expect("PartialOrd methods");
    let partial_cmp = methods
        .iter()
        .find(|m| m.name == "partial_cmp")
        .expect("partial_cmp method");
    assert_eq!(partial_cmp.param_kinds.len(), 1);
    assert_eq!(
        partial_cmp.param_kinds[0],
        StdlibTypeKind::AllocType,
        "PartialOrd::partial_cmp param should be AllocType (&Self), got {:?}",
        partial_cmp.param_kinds[0]
    );
}

/// Ord::cmp param_kinds is [AllocType] (other: &Self, unchanged).
#[test]
fn test_ord_cmp_param_kinds_alloc_type() {
    let methods = stdlib_trait_methods("Ord").expect("Ord methods");
    let cmp = methods
        .iter()
        .find(|m| m.name == "cmp")
        .expect("cmp method");
    assert_eq!(cmp.param_kinds.len(), 1);
    assert_eq!(
        cmp.param_kinds[0],
        StdlibTypeKind::AllocType,
        "Ord::cmp param should be AllocType (&Self), got {:?}",
        cmp.param_kinds[0]
    );
}

/// All stdlib methods have param_count matching param_kinds.len().
#[test]
fn test_all_methods_param_count_matches_param_kinds_length() {
    let all_traits = [
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
    for trait_name in &all_traits {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for m in methods {
                assert_eq!(
                    m.param_count as usize,
                    m.param_kinds.len(),
                    "trait {} method {}: param_count {} != param_kinds.len() {}",
                    trait_name,
                    m.name,
                    m.param_count,
                    m.param_kinds.len()
                );
            }
        }
    }
}
