//! Stage 5.10: Builtin Clone/Drop activation + generic builtin trait check tests
//!
//! Tests that `is_clone_builtin()`, `is_drop_builtin()`, and the generic
//! `implements_builtin_trait()` methods correctly detect `impl Clone for S`,
//! `impl Drop for S`, and any builtin trait impl — without requiring the
//! user to define `trait Clone {}` or `trait Drop {}`.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// Helper: find the DefId of a type by name in the resolver.
fn find_type_def_id(
    result: &landin_compiler::CompileResult,
    name: &str,
) -> Option<landin_compiler::hir::DefId> {
    let name_spur = result.interner.get(name)?;
    result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == name_spur)
        .map(|(&d, _)| d)
}

/// `impl Clone for S` (without `trait Clone {}`) should make S be Clone.
#[test]
fn test_builtin_clone_works_without_trait_def() {
    let result = compile("struct S; impl Clone for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_clone_builtin(s_def_id, &result.interner),
        "S with `impl Clone for S` (no `trait Clone {{}}`) should be Clone"
    );
}

/// `impl Drop for S` (without `trait Drop {}`) should make S have Drop.
#[test]
fn test_builtin_drop_works_without_trait_def() {
    let result = compile("struct S; impl Drop for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_drop_builtin(s_def_id, &result.interner),
        "S with `impl Drop for S` (no `trait Drop {{}}`) should have Drop"
    );
}

/// Without `impl Clone for S`, S should NOT be Clone.
#[test]
fn test_no_clone_impl_means_not_clone() {
    let result = compile("struct S; fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        !result
            .trait_resolver
            .is_clone_builtin(s_def_id, &result.interner),
        "S without `impl Clone for S` should NOT be Clone"
    );
}

/// Generic `implements_builtin_trait()` should work for any builtin trait.
#[test]
fn test_generic_builtin_trait_check_copy() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .implements_builtin_trait(s_def_id, "Copy", &result.interner),
        "implements_builtin_trait(\"Copy\") should return true for `impl Copy for S`"
    );
}

/// Generic `implements_builtin_trait()` should work for Clone too.
#[test]
fn test_generic_builtin_trait_check_clone() {
    let result = compile("struct S; impl Clone for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .implements_builtin_trait(s_def_id, "Clone", &result.interner),
        "implements_builtin_trait(\"Clone\") should return true for `impl Clone for S`"
    );
}

/// Generic `implements_builtin_trait()` should return false for unimpl'd trait.
#[test]
fn test_generic_builtin_trait_check_false() {
    let result = compile("struct S; fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        !result
            .trait_resolver
            .implements_builtin_trait(s_def_id, "Drop", &result.interner),
        "implements_builtin_trait(\"Drop\") should return false for S without `impl Drop`"
    );
}

/// Multiple builtin trait impls on the same type should all be detected.
#[test]
fn test_multiple_builtin_traits_on_same_type() {
    let result =
        compile("struct S; impl Copy for S {} impl Clone for S {} impl Drop for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_copy_builtin(s_def_id, &result.interner),
        "S should be Copy"
    );
    assert!(
        result
            .trait_resolver
            .is_clone_builtin(s_def_id, &result.interner),
        "S should be Clone"
    );
    assert!(
        result
            .trait_resolver
            .is_drop_builtin(s_def_id, &result.interner),
        "S should have Drop"
    );
}
