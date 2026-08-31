//! Stage 15.9 — VtableEntry.fn_name interning + TraitError typed errors.
//!
//! These tests verify the two Phase 2 audit quick wins completed in
//! Stage 15.9:
//! 1. `VtableEntry.fn_name` is now `Spur` (was `String`) — interned.
//! 2. `CompileErrors.trait_errors` is now `Vec<TraitError>` (was `Vec<String>`)
//!    — preserves structured CoherenceError/IncompleteImpl data.
//!
//! Coverage:
//! 1. VtableEntry.fn_name resolves correctly via interner
//! 2. Multiple vtable entries resolve correctly
//! 3. TraitError::Coherence carries structured data
//! 4. TraitError::Incomplete carries structured data
//! 5. TraitError::format_with_interner produces correct messages
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify both
//! changes work correctly with real HIR produced by compile().

#![cfg(test)]
//!
//! Stage 30.22: migrated from deprecated `find_vtable` (Spur-based) to
//! `find_vtable_by_def_ids` (DefId-keyed, type-safe).
use landin_compiler::compile;
use landin_compiler::session::SourceMap;
use landin_compiler::TraitError;

/// Stage 15.9 test 1: VtableEntry.fn_name is interned as Spur.
///
/// Verifies that after compilation, the vtable's fn_name Spur resolves
/// to the expected LLVM symbol name via the interner.
#[test]
fn stage15_9_vtable_fn_name_interned() {
    let src = r#"
        trait Foo { fn bar(); }
        struct S;
        impl Foo for S { fn bar() {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(result.errors.is_empty());

    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let foo_def_id = result
        .trait_resolver
        .find_trait_def_id(foo_spur)
        .expect("Foo trait DefId");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S type DefId");
    let vtable = result
        .trait_resolver
        .find_vtable_by_def_ids(foo_def_id, s_def_id)
        .expect("vtable should exist");

    // Stage 15.9: fn_name is Spur, resolve via interner.
    let fn_name_str = result
        .interner
        .try_resolve(&vtable.entries[0].fn_name)
        .expect("fn_name should resolve");
    assert_eq!(
        fn_name_str, "landin_S_bar",
        "fn_name should resolve to landin_S_bar"
    );
}

/// Stage 15.9 test 2: Multiple vtable entries all resolve correctly.
#[test]
fn stage15_9_multiple_vtable_entries_interned() {
    let src = r#"
        trait Multi { fn alpha(); fn beta(); fn gamma(); }
        struct T;
        impl Multi for T { fn alpha() {} fn beta() {} fn gamma() {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(result.errors.is_empty());

    let multi_spur = result.interner.get("Multi").expect("Multi interned");
    let t_spur = result.interner.get("T").expect("T interned");
    let multi_def_id = result
        .trait_resolver
        .find_trait_def_id(multi_spur)
        .expect("Multi trait DefId");
    let t_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == t_spur)
        .map(|(&d, _)| d)
        .expect("T type DefId");
    let vtable = result
        .trait_resolver
        .find_vtable_by_def_ids(multi_def_id, t_def_id)
        .expect("vtable should exist");

    assert_eq!(vtable.entries.len(), 3, "should have 3 entries");

    // All fn_name Spurs should resolve to the expected symbols.
    let expected = ["landin_T_alpha", "landin_T_beta", "landin_T_gamma"];
    for (i, entry) in vtable.entries.iter().enumerate() {
        let fn_name_str = result
            .interner
            .try_resolve(&entry.fn_name)
            .expect("fn_name should resolve");
        assert_eq!(
            fn_name_str, expected[i],
            "entry {} fn_name should be {}",
            i, expected[i]
        );
    }
}

/// Stage 15.9 test 3: TraitError::Coherence carries structured data.
///
/// Verifies that coherence errors preserve the CoherenceError struct
/// (trait_name, self_ty_name, impl_def_ids) — not just a string.
#[test]
fn stage15_9_trait_error_coherence_structured() {
    let src = r#"
        trait Foo {}
        struct S;
        impl Foo for S {}
        impl Foo for S {}
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.errors.trait_errors.is_empty());

    // Stage 15.9: trait_errors[0] is TraitError::Coherence, carrying
    // the structured CoherenceError data.
    match &result.errors.trait_errors[0] {
        TraitError::Coherence(ce) => {
            let trait_str = result.interner.try_resolve(&ce.trait_name).unwrap_or("?");
            let type_str = result.interner.try_resolve(&ce.self_ty_name).unwrap_or("?");
            assert_eq!(trait_str, "Foo", "trait_name should be Foo");
            assert_eq!(type_str, "S", "self_ty_name should be S");
            assert!(
                ce.impl_def_ids.len() >= 2,
                "should have at least 2 impl_def_ids"
            );
        }
        TraitError::Incomplete(_) => panic!("expected Coherence, got Incomplete"),
        TraitError::InherentConflict(_) => panic!("expected Coherence, got InherentConflict"),
        TraitError::PrimitiveInherentImpl(_) => {
            panic!("expected Coherence, got PrimitiveInherentImpl")
        }
        TraitError::OrphanRule(_) => panic!("expected Coherence, got OrphanRule"),
    }
}

/// Stage 15.9 test 4: TraitError::Incomplete carries structured data.
#[test]
fn stage15_9_trait_error_incomplete_structured() {
    let src = r#"
        trait Foo { fn bar(); fn baz(); fn qux(); }
        struct S;
        impl Foo for S { fn bar() {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.errors.trait_errors.is_empty());

    // Stage 15.9: Find the Incomplete error (may be at index 0 or later).
    let incomplete = result
        .errors
        .trait_errors
        .iter()
        .find_map(|e| match e {
            TraitError::Incomplete(inc) => Some(inc),
            _ => None,
        })
        .expect("should have at least one Incomplete error");

    let trait_str = result
        .interner
        .try_resolve(&incomplete.trait_name)
        .unwrap_or("?");
    let type_str = result
        .interner
        .try_resolve(&incomplete.self_ty_name)
        .unwrap_or("?");
    assert_eq!(trait_str, "Foo", "trait_name should be Foo");
    assert_eq!(type_str, "S", "self_ty_name should be S");
    assert!(
        incomplete.missing_methods.len() >= 2,
        "should have at least 2 missing methods (baz, qux)"
    );

    // Verify the missing methods are interned correctly.
    let missing_names: Vec<&str> = incomplete
        .missing_methods
        .iter()
        .map(|s| result.interner.try_resolve(s).unwrap_or("?"))
        .collect();
    assert!(
        missing_names.contains(&"baz"),
        "missing_methods should contain baz, got: {:?}",
        missing_names
    );
    assert!(
        missing_names.contains(&"qux"),
        "missing_methods should contain qux, got: {:?}",
        missing_names
    );
}

/// Stage 15.9 test 5: TraitError::format_with_interner produces correct messages.
#[test]
fn stage15_9_trait_error_format_with_interner() {
    // Coherence error message.
    let src = "trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}";
    let result = compile(src);
    let msg = result.errors.trait_errors[0].format_with_interner(&result.interner);
    assert!(
        msg.contains("conflicting implementations"),
        "coherence message should mention 'conflicting implementations', got: {}",
        msg
    );
    assert!(
        msg.contains("Foo"),
        "coherence message should mention trait name 'Foo', got: {}",
        msg
    );
    assert!(
        msg.contains("S"),
        "coherence message should mention type name 'S', got: {}",
        msg
    );

    // Incomplete error message.
    let src2 =
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}";
    let result2 = compile(src2);
    let msg2 = result2.errors.trait_errors[0].format_with_interner(&result2.interner);
    assert!(
        msg2.contains("missing method"),
        "incomplete message should mention 'missing method', got: {}",
        msg2
    );
    assert!(
        msg2.contains("baz"),
        "incomplete message should mention missing method 'baz', got: {}",
        msg2
    );
}

/// Stage 15.9 test 6: format_via_diagnostics with interner displays trait errors.
#[test]
fn stage15_9_format_via_diagnostics_with_interner() {
    let src = "trait T { fn f(); } struct S; impl T for S {} fn main() { 0 }";
    let result = compile(src);
    // Stage 30.22: migrated from deprecated format_for_user to format_via_diagnostics.
    let _ = result.errors.format_via_diagnostics(
        src,
        "test",
        &SourceMap::new(src),
        Some(&result.interner),
    );
}
