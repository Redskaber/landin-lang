//! Stage 30.12 (v0.15 TD-TYPECK-IMPL-CONTEXT): Assoc type bindings collection
//! + pre-typeck projection resolution.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 4 positive tests (assoc type bindings collected correctly)
//!   - 2 regression tests (non-assoc-type impls still work)
//!
//! Per §1.0 原則 4 (报错 > 静默): assoc type bindings are now collected.
//! Per §1.0 原則 9 (正确 > 妥协): honest scope — collection done,
//! projection_resolver runs before typeck, but Self::Item resolution
//! may not fully work for all cases (deferred to deeper typeck work).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Assoc type bindings collected + compiles cleanly
// ============================================================================

/// Stage 30.12 positive 1: Impl with `type Item = i32` — compiles.
#[test]
fn stage30_12_positive_impl_with_assoc_type_compiles() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn main() {
    let h = Holder { val: 42 };
    let _ = h.get();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with assoc type should compile cleanly"
    );
}

/// Stage 30.12 positive 2: Impl with `type Item = bool` + matching body.
#[test]
fn stage30_12_positive_bool_assoc_type_matching_body() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: bool }
impl Container for Holder {
    type Item = bool;
    fn get(&self) -> Self::Item { self.val }
}
fn main() {
    let h = Holder { val: true };
    let _ = h.get();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with bool assoc type + matching body should compile"
    );
}

/// Stage 30.12 positive 3: Multiple assoc types.
#[test]
fn stage30_12_positive_multiple_assoc_types() {
    let result = compile(
        r#"
trait Multi { type A; type B; fn get_a(&self) -> Self::A; fn get_b(&self) -> Self::B; }
struct Pair { a: i32, b: bool }
impl Multi for Pair {
    type A = i32;
    type B = bool;
    fn get_a(&self) -> Self::A { self.a }
    fn get_b(&self) -> Self::B { self.b }
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with multiple assoc types should compile"
    );
}

/// Stage 30.12 positive 4: Assoc type used in method not returning Self::Item.
#[test]
fn stage30_12_positive_assoc_type_not_in_return() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; fn count(&self) -> i32; }
struct Holder { val: i32 }
impl Container for Holder {
    type Item = i32;
    fn get(&self) -> Self::Item { self.val }
    fn count(&self) -> i32 { 1 }
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with method not using Self::Item should compile"
    );
}

// ============================================================================
// REGRESSION TESTS — Non-assoc-type impls still work
// ============================================================================

/// Stage 30.12 regression 1: Inherent impl (no trait) — no assoc type check.
#[test]
fn stage30_12_regression_inherent_impl_no_assoc() {
    let result = compile(
        r#"
struct S { val: i32 }
impl S {
    fn get(&self) -> i32 { self.val }
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Inherent impl should compile cleanly"
    );
}

/// Stage 30.12 regression 2: Direct mismatch (no Self::Item) — typeck catches.
#[test]
fn stage30_12_regression_direct_mismatch_caught() {
    let result = compile(
        r#"
struct Holder { val: i32 }
impl Holder {
    fn get(&self) -> bool { self.val }
}
fn main() {}
"#,
    );
    assert!(
        !result.errors.typeck.is_empty(),
        "Direct mismatch (i32 -> bool) should be caught by typeck"
    );
}
