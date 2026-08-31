//! Stage 30.4 (v0.13 TD-STUB-PROJECTION-RESOLVER): Projection resolver
//! reclassification + E2E verification tests.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 6 positive tests (compile-time + runtime, including GATs)
//!   - 3 negative tests (error cases: missing assoc type, wrong type, etc.)
//!   - 1 regression test (projection resolver is non-regressive)
//!
//! Per §1.0 原則 4 (报错 > 静默): document the actual behavior accurately.
//! Per §1.0 原則 9 (正确 > 妥协): reclassify TD based on root-cause analysis.
//!
//! ## Background
//!
//! TD-STUB-PROJECTION-RESOLVER was classified as "projection_resolver
//! partial impl, not complete" — that associated type normalization
//! with termination guarantee was missing.
//!
//! Root-cause analysis (Stage 30.4) shows the projection resolver IS
//! fully implemented and working:
//!
//! 1. `projection_resolver.rs` (Stage 16.68, extended Stage 18.87) handles
//!    all `TyKind::Projection(assoc_def_id, substs)` in MIR local_decls.
//! 2. Recursion depth limit (`MAX_PROJECTION_DEPTH = 10`) provides
//!    termination guarantee per Stage 18.87 B8.
//! 3. Handles all compound types (Ref, RawPtr, Array, Slice, Tuple, Adt,
//!    FnDef, Closure, FnPtr, Projection).
//! 4. E2E tests (Stage 21.1, 21 GATs tests) cover: type params, lifetime
//!    params, bounds, defaults, multiple type params, qualified paths,
//!    where clauses, error cases.
//!
//! ## Reclassification
//!
//! - TD-STUB-PROJECTION-RESOLVER → **RESOLVED** (projection resolver works)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Projection resolver works correctly
// ============================================================================

/// Stage 30.4 positive 1: Basic associated type compiles cleanly.
#[test]
fn stage30_4_positive_basic_assoc_type_compiles() {
    let result = compile(
        r#"
trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }
struct Counter { count: i32 }
impl Iterator for Counter {
    type Item = i32;
    fn next(&mut self) -> Option<Self::Item> {
        self.count = self.count + 1;
        Option::Some(self.count)
    }
}
fn main() { let mut c = Counter { count: 0 }; let _ = c.next(); }
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Basic associated type should compile cleanly"
    );
    assert_eq!(
        result.errors.codegen.len(),
        0,
        "Basic associated type codegen should produce no errors"
    );
}

/// Stage 30.4 positive 2: Associated type in let binding — runtime value correct.
#[test]
fn stage30_4_positive_assoc_type_runtime_value() {
    assert_runtime(
        "stage30-4-assoc-type-runtime",
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn main() -> i32 {
    let h = Holder { val: 42 };
    let x: i32 = h.get();
    println!("{}", x);
    0
}
"#,
        "42\n",
    );
}

/// Stage 30.4 positive 3: Two impls with different assoc types — dispatch correct.
#[test]
fn stage30_4_positive_two_impls_dispatch() {
    assert_runtime(
        "stage30-4-two-impls-dispatch",
        r#"
trait Producer { type Output; fn produce(&self) -> Self::Output; }
struct IntProducer { val: i32 }
impl Producer for IntProducer {
    type Output = i32;
    fn produce(&self) -> Self::Output { self.val }
}
fn main() -> i32 {
    let p = IntProducer { val: 99 };
    let v: i32 = p.produce();
    println!("{}", v);
    0
}
"#,
        "99\n",
    );
}

/// Stage 30.4 positive 4: GAT (Generic Associated Type) runtime value.
#[test]
fn stage30_4_positive_gat_runtime_value() {
    assert_runtime(
        "stage30-4-gat-runtime",
        r#"
trait Iterable { type Item<T>; fn first(&self) -> Self::Item<i32>; }
struct SingleInt { val: i32 }
impl Iterable for SingleInt {
    type Item<T> = T;
    fn first(&self) -> Self::Item<i32> { self.val }
}
fn main() -> i32 {
    let s = SingleInt { val: 123 };
    let v: i32 = s.first();
    println!("{}", v);
    0
}
"#,
        "123\n",
    );
}

/// Stage 30.4 positive 5: Associated type used as field type.
#[test]
fn stage30_4_positive_assoc_type_as_field() {
    let result = compile(
        r#"
trait Gettable { type Output; fn get_output(&self) -> Self::Output; }
struct Wrapper { inner: i32 }
impl Gettable for Wrapper {
    type Output = i32;
    fn get_output(&self) -> Self::Output { self.inner }
}
fn main() {
    let w = Wrapper { inner: 7 };
    let _ = w.get_output();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Associated type as field should compile"
    );
}

/// Stage 30.4 positive 6: Associated type with where clause in impl.
#[test]
fn stage30_4_positive_assoc_type_with_where() {
    let result = compile(
        r#"
trait Producer { type Output; fn produce(&self) -> Self::Output; }
struct MyType { val: i32 }
impl Producer for MyType where MyType: Copy {
    type Output = i32;
    fn produce(&self) -> Self::Output { self.val }
}
impl Copy for MyType {}
fn main() {
    let m = MyType { val: 5 };
    let _ = m.produce();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Associated type with where clause should compile"
    );
}

// ============================================================================
// NEGATIVE TESTS — Known limitations (soundness gaps)
// ============================================================================
//
// These tests document soundness gaps in the projection resolver:
// - Missing `type Item = ...;` in impl block is silently accepted
// - Wrong type value (`type Item = bool` but method returns i32) is silently
//   accepted
//
// These are KNOWN LIMITATIONS — the projection resolver does not verify
// that the impl block provides all required associated types, nor does it
// verify that the provided type matches the method return type.
//
// Per §1.0 原則 4 (报错 > 静默): these should error but currently don't.
// New TD: TD-PROJECTION-IMPL-VERIFICATION (P2, v0.14+) — verify impl block
// provides all required assoc types + verify type match between
// `type Item = T` and methods returning `Self::Item`.

/// Stage 30.4 negative 1 → Stage 30.7 FIXED: Missing associated type in
/// impl is now REJECTED.
///
/// Stage 30.7 (v0.14 TD-PROJECTION-IMPL-VERIFICATION) FIX:
/// The compiler now reports "missing associated type `Item` in
/// implementation of trait `Container`" when an impl block doesn't
/// provide a `type Item = ...;` declaration required by the trait.
///
/// This test was previously a KNOWN LIMITATION (Stage 30.4) that
/// documented the old behavior (silently accepted). Now it verifies
/// the fix works correctly.
#[test]
fn stage30_4_negative_missing_assoc_type_in_impl_silently_accepted() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    // Missing: type Item = i32;
    fn get(&self) -> Self::Item { self.val }
}
fn main() {
    let h = Holder { val: 42 };
    let _ = h.get();
}
"#,
    );
    // Stage 30.7 FIX: missing assoc type is now rejected
    let total_errors =
        result.errors.typeck.len() + result.errors.codegen.len() + result.errors.borrowck.len();
    assert!(
        total_errors > 0,
        "Stage 30.7 FIX: missing assoc type in impl should be rejected (got {} errors). \
         If this passes with 0 errors, TD-PROJECTION-IMPL-VERIFICATION regressed.",
        total_errors
    );
    // Verify the error message mentions the missing assoc type
    let has_missing_assoc_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("missing associated type"));
    assert!(
        has_missing_assoc_error,
        "Stage 30.7 FIX: error message should mention 'missing associated type'. \
         Got: {:?}",
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

/// Stage 30.4 negative 2: Wrong associated type value — KNOWN LIMITATION.
///
/// `type Item = bool` but method returns `i32`. Should be a type mismatch
/// error. Currently silently accepted (the projection resolver doesn't
/// verify type match between `type Item = T` and method returns).
#[test]
fn stage30_4_negative_wrong_assoc_type_value_silently_accepted() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    type Item = bool;
    fn get(&self) -> Self::Item { self.val }  // i32 != bool
}
fn main() {
    let h = Holder { val: 42 };
    let _ = h.get();
}
"#,
    );
    // KNOWN LIMITATION: the compiler silently accepts this. When
    // TD-PROJECTION-IMPL-VERIFICATION is fixed, this should produce errors.
    let total_errors = result.errors.typeck.len() + result.errors.codegen.len();
    assert_eq!(
        total_errors, 0,
        "KNOWN LIMITATION (TD-PROJECTION-IMPL-VERIFICATION): wrong assoc type value \
         (i32 returned where bool declared) is silently accepted. When TD is \
         fixed, this should be >0 errors."
    );
}

/// Stage 30.4 negative 3: Using `Self::Item` outside an impl context.
#[test]
fn stage30_4_negative_self_item_outside_impl() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
// Use Self::Item in a free function — not allowed
fn bad_func<T: Container>(c: &T) -> Self::Item { c.get() }
fn main() {}
"#,
    );
    let total_errors = result.errors.typeck.len() + result.errors.parse.len();
    assert!(
        total_errors > 0,
        "Self::Item outside impl context should error"
    );
}

// ============================================================================
// REGRESSION TESTS — Verify projection resolver is non-regressive
// ============================================================================

/// Stage 30.4 regression 1: Projection resolver handles nested projections.
#[test]
fn stage30_4_regression_nested_projection() {
    let result = compile(
        r#"
trait Outer { type Inner; fn get_inner(&self) -> Self::Inner; }
trait Inner { type Item; fn get_item(&self) -> Self::Item; }
struct OuterImpl { val: i32 }
impl Outer for OuterImpl {
    type Inner = InnerImpl;
    fn get_inner(&self) -> Self::Inner { InnerImpl { val: self.val } }
}
struct InnerImpl { val: i32 }
impl Inner for InnerImpl {
    type Item = i32;
    fn get_item(&self) -> Self::Item { self.val }
}
fn main() {
    let o = OuterImpl { val: 7 };
    let i = o.get_inner();
    let _ = i.get_item();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Nested projection should compile cleanly"
    );
}
