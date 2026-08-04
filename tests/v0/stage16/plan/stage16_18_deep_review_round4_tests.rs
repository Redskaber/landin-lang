//! Stage 16.18 — Deep Review Round 4: v0.3 release readiness verification.
//!
//! This test verifies that the v0.3 release scope is complete and stable:
//! 1. Sound Copy detection works end-to-end
//! 2. Task 3 DefId-keyed lookup is consistent
//! 3. Task 10 infrastructure is in place (no regressions)
//! 4. All core features compile correctly
//!
//! Per §29.1.3 (Design-Impl-Test coverage): release readiness verification.
//! Per §1.0 原則 9 "正确 > 妥协": sound foundation for release.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.18 test 1: Sound Copy — derived Copy struct works end-to-end.
#[test]
fn stage16_18_sound_copy_derived_works() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let p2 = p; p.x + p2.y }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Derived Copy should work: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.18 test 2: Sound Copy — non-Copy with Drop rejects double-move.
#[test]
fn stage16_18_sound_copy_non_copy_rejects_double_move() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let r = R; let r2 = r; let r3 = r; 0 }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "Non-Copy double-move should be rejected"
    );
}

/// Stage 16.18 test 3: Task 3 — DefId-keyed lookup works for user traits.
#[test]
fn stage16_18_def_id_lookup_user_traits() {
    let src =
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "User trait should compile: {:?}",
        result.errors
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let foo_def_id = result
        .trait_resolver
        .find_trait_def_id(foo_spur)
        .expect("Foo DefId");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    assert!(
        result
            .trait_resolver
            .implements_by_def_ids(foo_def_id, s_def_id),
        "DefId lookup should find impl"
    );
}

/// Stage 16.18 test 4: Task 10 — closure infrastructure is in place.
#[test]
fn stage16_18_closure_infrastructure_present() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    assert!(
        !result.has_errors(),
        "Closure should compile: {:?}",
        result.errors.borrowck
    );
    assert_eq!(
        result.synthesized_closure_mir_bodies.len(),
        1,
        "should have 1 synthesized MIR body"
    );
    let mir = &result.synthesized_closure_mir_bodies[0];
    assert!(mir.def_id.is_some(), "MirBody should have def_id set");
    assert!(!mir.basic_blocks.is_empty(), "should have basic blocks");
}

/// Stage 16.18 test 5: Complete program with traits + closures + Copy compiles.
#[test]
fn stage16_18_complete_program_compiles() {
    let src = r#"
        trait Drawable { fn draw(&self) -> i32; }
        struct Circle { radius: i32 }
        impl Drawable for Circle { fn draw(&self) -> i32 { self.radius } }
        impl Copy for Circle {}
        fn main() -> i32 {
            let c = Circle { radius: 5 };
            let c2 = c;
            let f = |x: i32| x + 1;
            c.draw() + f(10)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Complete program should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.18 test 6: Enum with Copy variants works.
#[test]
fn stage16_18_enum_copy_variants_work() {
    let src =
        "enum Color { Red, Green, Blue } fn main() -> i32 { let c = Color::Red; let c2 = c; 0 }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with Copy variants should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.18 test 7: Multiple traits + impls compile.
#[test]
fn stage16_18_multiple_traits_compile() {
    let src = r#"
        trait A { fn a(&self) -> i32; }
        trait B { fn b(&self) -> i32; }
        struct S;
        impl A for S { fn a(&self) -> i32 { 1 } }
        impl B for S { fn b(&self) -> i32 { 2 } }
        fn main() -> i32 { let s = S; s.a() + s.b() }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple traits should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.18 test 8: Drop elaboration works end-to-end.
#[test]
fn stage16_18_drop_elaboration_works() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter { fn drop(&mut self) {} }
        fn main() -> i32 { let c = Counter { value: 42 }; c.value }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Drop elaboration should work: {:?}",
        result.errors.borrowck
    );
}
