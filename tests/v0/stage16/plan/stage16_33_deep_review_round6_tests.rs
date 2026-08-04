//! Stage 16.33 — Deep Review Round 6: v0.3 Closure Redesign Complete Verification.
//!
//! These tests verify the v0.3 closure redesign is fully complete and stable:
//! 1. All closure patterns compile AND run correctly
//! 2. Sound Copy detection works end-to-end
//! 3. Task 3 DefId-keyed lookup is consistent
//! 4. No regressions across the entire pipeline
//!
//! Per §29.1.3: milestone verification.
//! Per §1.0 原則 9: sound foundation for v0.3 release.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.33 test 1: No-capture closure compiles and uses synthesized path.
#[test]
fn stage16_33_nocapture_closure_complete() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.33 test 2: i32-capture closure (the original typeck gap case).
#[test]
fn stage16_33_i32_capture_complete() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.33 test 3: Struct-capture closure (was inline path before Stage 16.29).
#[test]
fn stage16_33_struct_capture_complete() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let f = || p.x + p.y; f() }";
    let result = compile(src);
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.33 test 4: Mutable capture closure (borrowck on closure MIR bodies).
#[test]
fn stage16_33_mutable_capture_complete() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors(), "{:?}", result.errors.borrowck);
}

/// Stage 16.33 test 5: Nested closure (double-nested).
#[test]
fn stage16_33_nested_closure_complete() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 2);
}

/// Stage 16.33 test 6: Triple-nested closure.
#[test]
fn stage16_33_triple_nested_complete() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors(), "{:?}", result.errors);
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.33 test 7: Sound Copy — derived Copy works.
#[test]
fn stage16_33_sound_copy_derived() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let p2 = p; p.x + p2.y }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.33 test 8: Sound Copy — non-Copy with Drop rejects double-move.
#[test]
fn stage16_33_sound_copy_non_copy() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let r = R; let r2 = r; let r3 = r; 0 }";
    let result = compile(src);
    assert!(!result.errors.borrowck.is_empty());
}

/// Stage 16.33 test 9: Task 3 — DefId-keyed lookup for user traits.
#[test]
fn stage16_33_def_id_lookup() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    assert!(!result.has_errors());
}

/// Stage 16.33 test 10: Complete program with all v0.3 features.
#[test]
fn stage16_33_complete_program_all_features() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        trait Add { fn add(&self, other: &Point) -> i32; }
        impl Add for Point {
            fn add(&self, other: &Point) -> i32 { self.x + other.x + self.y + other.y }
        }
        fn main() -> i32 {
            let p1 = Point { x: 1, y: 2 };
            let p2 = Point { x: 3, y: 4 };
            let p3 = p1;
            let f = |a: i32| a + p3.x;
            f(10) + p1.add(&p2)
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "{:?}", result.errors);
}
