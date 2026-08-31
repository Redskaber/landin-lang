//! Stage 30.13 (v0.15 TD-HRTB-FULL-ENFORCEMENT): HRTB bound partial enforcement.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 4 positive tests (HRTB bounds with valid implementations)
//!   - 2 regression tests (non-HRTB impls still work)
//!
//! Per §1.0 原則 4 (报错 > 静默): HRTB bounds are now partially enforced.
//! Per §1.0 原則 9 (正确 > 妥协): honest scope — implementation check done,
//! universal quantification (placeholder universes) deferred to
//! TD-HRTB-PLACEHOLDER-CHECK (P2, v0.16+).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — HRTB bounds with valid implementations compile
// ============================================================================

/// Stage 30.13 positive 1: HRTB bound in where clause with valid impl.
#[test]
fn stage30_13_positive_hrtb_valid_impl() {
    let result = compile(
        r#"
trait Foo<'a> { fn foo(&self, x: &'a i32); }
struct MyType;
impl<'a> Foo<'a> for MyType {
    fn foo(&self, _x: &'a i32) {}
}
struct Wrapper<T> { inner: T }
impl<T> Wrapper<T> where T: for<'a> Foo<'a> {
    fn get(&self) -> &T { &self.inner }
}
fn main() {
    let w = Wrapper { inner: MyType };
    let _ = w.get();
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "HRTB bound with valid impl should compile cleanly"
    );
}

/// Stage 30.13 positive 2: HRTB bound in trait bound with valid impl.
#[test]
fn stage30_13_positive_hrtb_trait_bound_valid() {
    let result = compile(
        r#"
trait Foo<'a> { fn foo(&self, x: &'a i32); }
struct MyType;
impl<'a> Foo<'a> for MyType {
    fn foo(&self, _x: &'a i32) {}
}
fn use_it<T: for<'a> Foo<'a>>(x: &T) {}
fn main() {
    use_it(&MyType);
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "HRTB trait bound with valid impl should compile"
    );
}

/// Stage 30.13 positive 3: Multiple HRTB bounds with valid impls.
#[test]
fn stage30_13_positive_multiple_hrtb_valid() {
    let result = compile(
        r#"
trait Foo<'a> { fn foo(&self, x: &'a i32); }
trait Bar<'a> { fn bar(&self, x: &'a i32); }
struct MyType;
impl<'a> Foo<'a> for MyType { fn foo(&self, _x: &'a i32) {} }
impl<'a> Bar<'a> for MyType { fn bar(&self, _x: &'a i32) {} }
fn use_it<T: for<'a> Foo<'a> + for<'a> Bar<'a>>(x: &T) {}
fn main() {
    use_it(&MyType);
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Multiple HRTB bounds with valid impls should compile"
    );
}

/// Stage 30.13 positive 4: HRTB with multiple lifetime params.
#[test]
fn stage30_13_positive_hrtb_multiple_lifetimes() {
    let result = compile(
        r#"
trait Foo<'a, 'b> { fn foo(&self, x: &'a i32, y: &'b i32); }
struct MyType;
impl<'a, 'b> Foo<'a, 'b> for MyType {
    fn foo(&self, _x: &'a i32, _y: &'b i32) {}
}
fn use_it<T: for<'a, 'b> Foo<'a, 'b>>(x: &T) {}
fn main() {
    use_it(&MyType);
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "HRTB with multiple lifetimes + valid impl should compile"
    );
}

// ============================================================================
// REGRESSION TESTS — Non-HRTB code still works
// ============================================================================

/// Stage 30.13 regression 1: Non-HRTB bound still works.
#[test]
fn stage30_13_regression_non_hrtb_bound() {
    let result = compile(
        r#"
trait Foo { fn foo(&self); }
struct MyType;
impl Foo for MyType { fn foo(&self) {} }
fn use_it<T: Foo>(x: &T) {}
fn main() {
    use_it(&MyType);
}
"#,
    );
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "Non-HRTB bound should still compile"
    );
}

/// Stage 30.13 regression 2: HRTB with generic param (skipped — can't check).
#[test]
fn stage30_13_regression_hrtb_generic_param_skipped() {
    let result = compile(
        r#"
trait Foo<'a> { fn foo(&self, x: &'a i32); }
fn use_it<T: for<'a> Foo<'a>>(x: &T) {}
fn main() {}
"#,
    );
    // Generic param T — can't verify at this stage (no concrete type).
    // Should compile cleanly (no false positive).
    assert_eq!(
        result.errors.typeck.len(),
        0,
        "HRTB with generic param should compile (skipped — can't check)"
    );
}
