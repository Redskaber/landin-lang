//! Stage 16.25 — Deep Review Round 5: v0.3 milestone verification.
//!
//! These tests verify the v0.3 milestones are achieved and stable:
//! 1. Sound Copy detection works end-to-end
//! 2. Task 3 DefId-keyed lookup is consistent
//! 3. Task 10 no-capture closures use synthesized `call` function
//! 4. All core features compile correctly
//!
//! Per §29.1.3: milestone verification.
//! Per §1.0 原則 9: sound foundation for v0.3.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.25 test 1: No-capture closure uses synthesized call function.
#[test]
fn stage16_25_nocapture_closure_synthesized() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(
        !result.has_errors(),
        "Closure should compile: {:?}",
        result.errors.borrowck
    );
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
    let mir = &result.synthesized_closure_mir_bodies[0];
    assert!(mir.def_id.is_some(), "MirBody should have def_id");
}

/// Stage 16.25 test 2: Sound Copy — derived Copy works.
#[test]
fn stage16_25_sound_copy_derived() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let p2 = p; p.x + p2.y }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.25 test 3: Sound Copy — non-Copy with Drop rejects double-move.
#[test]
fn stage16_25_sound_copy_non_copy() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let r = R; let r2 = r; let r3 = r; 0 }";
    let result = compile(src);
    assert!(!result.errors.borrowck.is_empty());
}

/// Stage 16.25 test 4: Task 3 — DefId-keyed lookup for user traits.
#[test]
fn stage16_25_def_id_lookup() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").unwrap();
    let foo_def_id = result.trait_resolver.find_trait_def_id(foo_spur).unwrap();
    let s_spur = result.interner.get("S").unwrap();
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .unwrap();
    assert!(result
        .trait_resolver
        .implements_by_def_ids(foo_def_id, s_def_id));
}

/// Stage 16.25 test 5: Complete program with all features compiles.
#[test]
fn stage16_25_complete_program() {
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

/// Stage 16.25 test 6: Chained no-capture closure calls.
#[test]
fn stage16_25_chained_closure_calls() {
    let src = "fn main() -> i32 { let f = |x: i32| x + 1; f(f(f(0))) }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Chained calls should compile: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.25 test 7: Multiple closures in same function.
#[test]
fn stage16_25_multiple_closures() {
    let src = "fn main() -> i32 { let f = |x| x + 1; let g = |y| y * 2; f(5) + g(3) }";
    let result = compile(src);
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.25 test 8: MirBody.def_id is set for synthesized functions.
#[test]
fn stage16_25_mirbody_def_id_set() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    let mir = &result.synthesized_closure_mir_bodies[0];
    assert!(mir.def_id.is_some(), "def_id should be set");
}
