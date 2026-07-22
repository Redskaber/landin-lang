//! Stage 5.8: Standard trait registry (stdlib MVP) tests
//!
//! Tests that the compiler recognizes builtin standard traits (Copy, Clone,
//! Drop, Sized, Send, Sync, etc.) automatically — without the user defining
//! `trait Copy {}`. This is the stdlib MVP foundation.
//!
//! Per §16: tests use the `compile()` public API + inspect
//! `CompileResult.trait_resolver.builtin_traits`.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;
use landin_compiler::BUILTIN_TRAIT_NAMES;

/// The compiler should recognize all 10 builtin standard traits after
/// compilation, even when the user does not define any of them.
#[test]
fn test_builtin_traits_registered() {
    let result = compile("fn main() {}");
    // All builtin trait names should be in the builtin_traits registry.
    for &name in BUILTIN_TRAIT_NAMES {
        let spur = result
            .interner
            .get(name)
            .unwrap_or_else(|| panic!("{} should be interned", name));
        assert!(
            result.trait_resolver.is_builtin_trait(spur),
            "{} should be a builtin trait",
            name
        );
    }
}

/// `find_builtin_trait()` should return a DefId in the reserved high range
/// (u32::MAX downward) for each builtin trait.
#[test]
fn test_builtin_trait_def_ids_in_reserved_range() {
    use landin_compiler::BUILTIN_DEF_ID_BASE;

    let result = compile("fn main() {}");
    for &name in BUILTIN_TRAIT_NAMES {
        let spur = result.interner.get(name).expect("builtin trait interned");
        let def_id = result
            .trait_resolver
            .find_builtin_trait(spur)
            .unwrap_or_else(|| panic!("{} should have a builtin DefId", name));
        // DefId should be in [u32::MAX - 9, u32::MAX] (10 builtin traits).
        assert!(
            def_id.0 > BUILTIN_DEF_ID_BASE - BUILTIN_TRAIT_NAMES.len() as u32,
            "{} DefId {} should be in reserved range [{}, {}]",
            name,
            def_id.0,
            BUILTIN_DEF_ID_BASE - BUILTIN_TRAIT_NAMES.len() as u32 + 1,
            BUILTIN_DEF_ID_BASE
        );
    }
}

/// User-defined traits should NOT be flagged as builtin.
#[test]
fn test_user_defined_trait_not_builtin() {
    let result = compile("trait Foo { fn bar(); } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo should be interned");
    assert!(
        !result.trait_resolver.is_builtin_trait(foo_spur),
        "user-defined trait Foo should NOT be a builtin trait"
    );
}

/// When the user defines `trait Copy {}`, it should still be recognized as
/// a builtin trait (the builtin registry is independent of user definitions).
#[test]
fn test_builtin_copy_recognized_even_with_user_definition() {
    let result = compile("trait Copy {} fn main() {}");
    let copy_spur = result
        .interner
        .get("Copy")
        .expect("Copy should be interned");
    // Copy is a builtin trait — recognized regardless of user definition.
    assert!(
        result.trait_resolver.is_builtin_trait(copy_spur),
        "Copy should be recognized as a builtin trait even with user definition"
    );
}

/// The builtin trait count should match BUILTIN_TRAIT_NAMES.len().
#[test]
fn test_builtin_trait_count() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.trait_resolver.builtin_traits.len(),
        BUILTIN_TRAIT_NAMES.len(),
        "builtin_traits registry should have {} entries",
        BUILTIN_TRAIT_NAMES.len()
    );
}
