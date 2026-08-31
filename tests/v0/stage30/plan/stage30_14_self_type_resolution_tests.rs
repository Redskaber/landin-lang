//! Stage 30.14 (v0.16 TD-SELF-TYPE-RESOLUTION): Self::Item multi-segment
//! path resolution in the resolver + Projection lowering in ty_lower.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 4 positive tests (Self::Item compiles with valid impl)
//!   - 2 regression tests (non-Self::Item code still works)
//!
//! Per §1.0 原則 4 (报错 > 静默): Self::Item should not silently become Error.
//! Per §1.0 原則 9 (正确 > 妥协): honest scope — Self::Item now lowers to
//! Projection (was Error), but full resolution (substs with Self type)
//! requires impl-block context awareness (deferred).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Self::Item compiles with valid impl
// ============================================================================

/// Stage 30.14 positive 1: Self::Item in method return — compiles.
#[test]
fn stage30_14_positive_self_item_compiles() {
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
        "Self::Item with valid impl should compile cleanly"
    );
}

/// Stage 30.14 positive 2: Self::Item with bool assoc type.
#[test]
fn stage30_14_positive_self_item_bool() {
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
        "Self::Item with bool assoc type should compile"
    );
}

/// Stage 30.14 positive 3: Multiple assoc types with Self::Item.
#[test]
fn stage30_14_positive_multiple_self_item() {
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
        "Multiple Self::Item with valid impls should compile"
    );
}

/// Stage 30.14 positive 4: Self::Item with method not using it.
#[test]
fn stage30_14_positive_self_item_not_in_return() {
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
        "Self::Item with method not using it should compile"
    );
}

// ============================================================================
// REGRESSION TESTS — Non-Self::Item code still works
// ============================================================================

/// Stage 30.14 regression 1: Qualified path <T as Trait>::Item still works.
#[test]
fn stage30_14_regression_qualified_path() {
    let result = compile(
        r#"
trait Container { type Item; fn get(&self) -> Self::Item; }
struct Holder { val: i32 }
impl Container for Holder {
    type Item = i32;
    fn get(&self) -> <Holder as Container>::Item { self.val }
}
fn main() {}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Qualified path <T as Trait>::Item should still work"
    );
}

/// Stage 30.14 regression 2: Direct return type (no Self::Item) — typeck catches mismatch.
#[test]
fn stage30_14_regression_direct_mismatch_caught() {
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
