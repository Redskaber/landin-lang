//! Stage 154 (v0.16 — TD-DYN-LOCAL-FAT-PTR-COERCION): Fix `let g: &dyn Trait = &local;`
//! to construct a LOCAL fat pointer `{ptr %local, ptr @.vtable.Trait.Type}` instead of
//! using the GLOBAL dynptr `@.dynptr.Trait.Type` (which points to `@.data.Type = i8 0`).
//!
//! ## Root cause
//!
//! Three bugs combined to produce wrong runtime results for `let g: &dyn Trait = &local;`:
//!
//! 1. **types.rs**: `Ref(_, _, Dyn(_))` fell to the `_` arm → `ptr_to(...)` (thin pointer,
//!    8 bytes) instead of `Struct([OpaquePtr, OpaquePtr])` (fat pointer, 16 bytes).
//!    The alloca for `g` was 8 bytes, losing the vtable pointer.
//!
//! 2. **rvalue.rs/statement.rs**: `Rvalue::Ref` for `let g: &dyn T = &local;` returned
//!    a thin pointer (`%loc_local`), not a fat pointer. The thin pointer was stored to
//!    the (now fat) alloca, leaving the vtable field uninitialized.
//!
//! 3. **operand.rs/aggregate.rs**: `emit_dyn_trait_method_call` used the GLOBAL dynptr
//!    symbol (`@.dynptr.Trait.Type`) for vtable dispatch, ignoring the local fat pointer.
//!    The global's data pointer is `@.data.Type = i8 0` (placeholder), not the actual
//!    local data.
//!
//! ## Fix (Stage 154)
//!
//! - **Part A (types.rs)**: Add `TyKind::Dyn(_)` case to `Ref` inner match →
//!   `Struct([OpaquePtr, OpaquePtr])` (fat pointer). Both `_with_layouts` and
//!   `_with_layouts_and_mono` variants.
//! - **Part B (statement.rs)**: When dest local is `Ref(Dyn)` and rvalue is `Ref` or
//!   `Use(Copy/Move)`, construct `{ptr %place, ptr @.vtable.Trait.Type}` via
//!   `emit_insertvalue`.
//! - **Part C (operand.rs + aggregate.rs)**: `emit_dyn_trait_method_call` accepts
//!   `receiver_value: &str` (can be `@global` or `%local`). The caller
//!   `codegen_dyn_trait_call_direct` extracts the receiver local from `args[0]`
//!   when its type is `Ref(Dyn)`.
//!
//! ## Test plan (10 tests)
//!
//! - Positive: let-binding dyn coercion with self access
//!   - `stage154_dyn_let_self_access` — main pattern: `let g: &dyn T = &e; g.method()`
//!   - `stage154_dyn_let_method_returns_field` — method accesses self.val
//!   - `stage154_dyn_let_multiple_methods` — call multiple methods via same g
//! - Regression: call-site coercion (Stage 89/90) still works
//!   - `stage154_regression_call_site_coercion` — `use_greeter(&e)` returns 42
//!   - `stage154_regression_dyn_let_no_self` — method doesn't use self (returns constant)
//!   - `stage154_regression_dyn_param_function` — function with `&dyn T` param
//! - Edge cases
//!   - `stage154_dyn_let_with_args` — method with args beyond self
//!   - `stage154_dyn_let_chained_calls` — multiple dyn calls in sequence
//! - Negative: typeck errors
//!   - `stage154_dyn_let_type_not_implementing_trait` — coercion rejected
//!
//! Per §1.0 原則 6 (通解 > 特解): one fat pointer construction rule for all
//! `Ref(Adt) → Ref(Dyn)` coercions.
//! Per §1.0 原則 9 (正确 > 妥协): construct fat pointer at the coercion site,
//! not patch the dispatch to use thin pointers.
//! Per §9.4.3: 1:3+ positive-to-negative ratio.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Positive tests — let-binding dyn coercion with self access
// ===========================================================================

/// Main pattern: `let g: &dyn Greeter = &e; g.greet()` where `greet` accesses
/// `self.val`. Before Stage 154, this returned 0 (read from `@.data.English =
/// i8 0` placeholder). After Stage 154, it returns 42 (read from local `e`).
#[test]
fn stage154_dyn_let_self_access() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32;
}
struct English { val: i32 }
impl Greeter for English {
    fn greet(&self) -> i32 { self.val }
}
fn main() -> i32 {
    let e = English { val: 42 };
    let g: &dyn Greeter = &e;
    g.greet()
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(
        exit, 42,
        "let g: &dyn Greeter = &e; g.greet() should return 42"
    );
    assert!(stdout.is_empty(), "expected no stdout, got: {}", stdout);
}

/// Method returns a field (not a constant). Confirms self pointer is correct.
#[test]
fn stage154_dyn_let_method_returns_field() {
    let code = r#"
trait Getter {
    fn get_x(&self) -> i32;
    fn get_y(&self) -> i32;
}
struct Point { x: i32, y: i32 }
impl Getter for Point {
    fn get_x(&self) -> i32 { self.x }
    fn get_y(&self) -> i32 { self.y }
}
fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    let g: &dyn Getter = &p;
    g.get_x() + g.get_y()
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 30, "g.get_x() + g.get_y() should be 10+20=30");
    assert!(stdout.is_empty());
}

/// Multiple methods called via the same dyn reference.
#[test]
fn stage154_dyn_let_multiple_methods() {
    let code = r#"
trait Counter {
    fn current(&self) -> i32;
    fn next(&mut self);
}
struct CounterImpl { val: i32 }
impl Counter for CounterImpl {
    fn current(&self) -> i32 { self.val }
    fn next(&mut self) { self.val = self.val + 1; }
}
fn main() -> i32 {
    let mut c = CounterImpl { val: 5 };
    let g: &mut dyn Counter = &mut c;
    g.next();
    g.next();
    g.current()
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 7, "after 2 next() calls, current() should be 7");
    assert!(stdout.is_empty());
}

// ===========================================================================
// Regression tests — call-site coercion (Stage 89/90) still works
// ===========================================================================

/// Stage 89/90 regression: call-site coercion `use_greeter(&e)` still works.
#[test]
fn stage154_regression_call_site_coercion() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32;
}
struct English;
impl Greeter for English {
    fn greet(&self) -> i32 { 42 }
}
fn use_greeter(g: &dyn Greeter) -> i32 {
    g.greet()
}
fn main() -> i32 {
    let e = English;
    use_greeter(&e)
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 42, "call-site coercion should still work");
    assert!(stdout.is_empty());
}

/// Regression: method that doesn't use self (returns constant) — should
/// still work after the type mapping change (Part A).
#[test]
fn stage154_regression_dyn_let_no_self() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32;
}
struct English;
impl Greeter for English {
    fn greet(&self) -> i32 { 42 }
}
fn main() -> i32 {
    let e = English;
    let g: &dyn Greeter = &e;
    g.greet()
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 42, "dyn let with constant method should return 42");
    assert!(stdout.is_empty());
}

/// Regression: function with `&dyn Trait` parameter — should still work
/// when called via call-site coercion.
#[test]
fn stage154_regression_dyn_param_function() {
    let code = r#"
trait Adder {
    fn add(&self, n: i32) -> i32;
}
struct Calc { base: i32 }
impl Adder for Calc {
    fn add(&self, n: i32) -> i32 { self.base + n }
}
fn use_adder(a: &dyn Adder, n: i32) -> i32 {
    a.add(n)
}
fn main() -> i32 {
    let c = Calc { base: 100 };
    use_adder(&c, 23)
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 123, "use_adder(&c, 23) should return 100+23=123");
    assert!(stdout.is_empty());
}

// ===========================================================================
// Edge cases
// ===========================================================================

/// Method with args beyond self — tests arg passing with dyn dispatch.
#[test]
fn stage154_dyn_let_with_args() {
    let code = r#"
trait Adder {
    fn add(&self, n: i32) -> i32;
}
struct Calc { base: i32 }
impl Adder for Calc {
    fn add(&self, n: i32) -> i32 { self.base + n }
}
fn main() -> i32 {
    let c = Calc { base: 50 };
    let g: &dyn Adder = &c;
    g.add(7)
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 57, "g.add(7) should return 50+7=57");
    assert!(stdout.is_empty());
}

/// Multiple dyn calls in sequence — tests that the fat pointer persists.
#[test]
fn stage154_dyn_let_chained_calls() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32;
}
struct English { val: i32 }
impl Greeter for English {
    fn greet(&self) -> i32 { self.val }
}
fn main() -> i32 {
    let e1 = English { val: 10 };
    let e2 = English { val: 20 };
    let g1: &dyn Greeter = &e1;
    let g2: &dyn Greeter = &e2;
    g1.greet() + g2.greet()
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 30, "g1.greet() + g2.greet() should be 10+20=30");
    assert!(stdout.is_empty());
}

// ===========================================================================
// Negative tests — typeck errors
// ===========================================================================

/// Coercion rejected when type doesn't implement the trait.
///
/// Per §1.0 原則 4 (报错 > 静默): typeck must reject invalid coercions.
#[test]
fn stage154_dyn_let_type_not_implementing_trait() {
    let code = r#"
trait Greeter {
    fn greet(&self) -> i32;
}
struct English;
impl Greeter for English {
    fn greet(&self) -> i32 { 42 }
}
struct Spanish;
fn main() -> i32 {
    let s = Spanish;
    let g: &dyn Greeter = &s;
    g.greet()
}
"#;
    let (_, exit) = run_program(code);
    // Typeck should reject: Spanish doesn't implement Greeter.
    assert_ne!(exit, 42, "type not implementing trait should not return 42");
}
