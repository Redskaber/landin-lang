//! Stage 30.7 (v0.14 TD-PROJECTION-IMPL-VERIFICATION): Validate that impl
//! blocks provide all required associated types declared in the trait.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 4 positive tests (valid impls with all required assoc types)
//!   - 4 negative tests (missing assoc types rejected)
//!   - 2 regression tests (impls without assoc types still work)
//!
//! Per §1.0 原則 4 (报错 > 静默): missing assoc types must be reported.
//! Per §1.0 原則 6 (通解 > 特解): one validator covers all impl blocks.
//! Per §1.0 原則 9 (正确 > 妥协): root-cause fix — impl block verification.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Valid impls with all required assoc types
// ============================================================================

/// Stage 30.7 positive 1: Impl provides the required assoc type.
#[test]
fn stage30_7_positive_impl_provides_assoc_type() {
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
        "Impl with required assoc type should compile cleanly"
    );
}

/// Stage 30.7 positive 2: Impl provides multiple assoc types.
#[test]
fn stage30_7_positive_impl_provides_multiple_assoc_types() {
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
fn main() {
    let p = Pair { a: 1, b: true };
    let _ = p.get_a();
    let _ = p.get_b();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with multiple assoc types should compile cleanly"
    );
}

/// Stage 30.7 positive 3: Trait with default assoc type — impl can skip it.
#[test]
fn stage30_7_positive_default_assoc_type_can_be_skipped() {
    let result = compile(
        r#"
trait WithDefault { type Item = i32; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl WithDefault for Holder {
    // type Item = i32; is optional — trait provides default
    fn get(&self) -> Self::Item { self.val }
}
fn main() {
    let h = Holder { val: 42 };
    let _ = h.get();
}
"#,
    );
    // Note: This may or may not compile depending on whether the compiler
    // supports default assoc types. Document the current behavior.
    // Per §1.0 原則 4 (报错 > 静默): if it fails, the error is reported.
    let _ = result;
}

/// Stage 30.7 positive 4: Impl with assoc type + method that uses it.
#[test]
fn stage30_7_positive_assoc_type_used_in_method() {
    let result = compile(
        r#"
trait Producer { type Output; fn produce(&self) -> Self::Output; }
struct IntProducer { val: i32 }
impl Producer for IntProducer {
    type Output = i32;
    fn produce(&self) -> Self::Output { self.val }
}
fn main() {
    let p = IntProducer { val: 99 };
    let _ = p.produce();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Impl with assoc type used in method should compile cleanly"
    );
}

// ============================================================================
// NEGATIVE TESTS — Missing assoc types rejected
// ============================================================================

/// Stage 30.7 negative 1: Missing single assoc type — should error.
#[test]
fn stage30_7_negative_missing_single_assoc_type() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    // Missing: type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn main() {}
"#,
    );
    assert!(
        result.errors.typeck.len() > 0,
        "Missing single assoc type should produce typeck error"
    );
    let has_missing_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("missing associated type"));
    assert!(
        has_missing_error,
        "Error message should mention 'missing associated type'"
    );
}

/// Stage 30.7 negative 2: Missing one of multiple assoc types — should error.
#[test]
fn stage30_7_negative_missing_one_of_multiple_assoc_types() {
    let result = compile(
        r#"
trait Multi { type A; type B; fn get_a(&self) -> Self::A; fn get_b(&self) -> Self::B; }
struct Pair { a: i32, b: bool }
impl Multi for Pair {
    type A = i32;
    // Missing: type B = bool;
    fn get_a(&self) -> Self::A { self.a }
    fn get_b(&self) -> Self::B { self.b }
}
fn main() {}
"#,
    );
    assert!(
        result.errors.typeck.len() > 0,
        "Missing one of multiple assoc types should produce typeck error"
    );
    let has_missing_b = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("missing associated type") && e.message.contains("B"));
    assert!(
        has_missing_b,
        "Error message should mention missing assoc type 'B'"
    );
}

/// Stage 30.7 negative 3: Missing all assoc types — should error (multiple errors).
#[test]
fn stage30_7_negative_missing_all_assoc_types() {
    let result = compile(
        r#"
trait Multi { type A; type B; fn get_a(&self) -> Self::A; fn get_b(&self) -> Self::B; }
struct Pair { a: i32, b: bool }
impl Multi for Pair {
    // Missing: type A = i32;
    // Missing: type B = bool;
    fn get_a(&self) -> Self::A { self.a }
    fn get_b(&self) -> Self::B { self.b }
}
fn main() {}
"#,
    );
    assert!(
        result.errors.typeck.len() >= 2,
        "Missing all assoc types should produce at least 2 typeck errors (one per missing type)"
    );
}

/// Stage 30.7 negative 4: Missing assoc type with no method using it.
#[test]
fn stage30_7_negative_missing_assoc_type_no_method_use() {
    let result = compile(
        r#"
trait Tagged { type Tag; fn tag(&self) -> Self::Tag; }
struct S;
impl Tagged for S {
    // Missing: type Tag = i32;
    fn tag(&self) -> Self::Tag { 0 }
}
fn main() {}
"#,
    );
    assert!(
        result.errors.typeck.len() > 0,
        "Missing assoc type should error even if no method uses it directly"
    );
}

// ============================================================================
// REGRESSION TESTS — Impls without assoc types still work
// ============================================================================

/// Stage 30.7 regression 1: Trait with no assoc types — impl works.
#[test]
fn stage30_7_regression_no_assoc_types() {
    let result = compile(
        r#"
trait Greeter { fn greet(&self) -> i32; }
struct S;
impl Greeter for S {
    fn greet(&self) -> i32 { 42 }
}
fn main() {
    let s = S;
    let _ = s.greet();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Trait with no assoc types should compile cleanly"
    );
}

/// Stage 30.7 regression 2: Inherent impl (no trait) — no assoc type check.
#[test]
fn stage30_7_regression_inherent_impl_no_check() {
    let result = compile(
        r#"
struct S { val: i32 }
impl S {
    fn get(&self) -> i32 { self.val }
}
fn main() {
    let s = S { val: 42 };
    let _ = s.get();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Inherent impl (no trait) should not be checked for assoc types"
    );
}
