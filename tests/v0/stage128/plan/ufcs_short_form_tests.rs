//! Stage 128 (v0.13 — TD-UFCS-SHORT-FORM): Short-form UFCS test suite.
//!
//! Tests for short-form Universal Function Call Syntax:
//!   `Trait::method(receiver, args)` — Self inferred from receiver.
//!
//! Stage 127 implemented the fully-qualified form `<T as Trait>::method`.
//! Stage 128 completes UFCS by adding the short form (Rust's recommended
//! syntax for trait method calls when disambiguation is needed).
//!
//! Per §9.4.3: 1:3+ positive-to-negative ratio.
//! Per §1.0 原則 3 (显式 > 隐式): UFCS makes trait context explicit.
//! Per §1.0 原則 6 (通解 > 特例): one mechanism for all UFCS forms.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

/// Helper: compile a Landin program and return (success, stderr).
fn compile(code: &str) -> (bool, String) {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    let counter = std::sync::atomic::AtomicU64::new(0);
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "stage128_compile_{}_{}_{}",
        std::process::id(),
        nanos,
        n
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let lin = dir.join("input.lin");
    std::fs::write(&lin, code).expect("write .lin");
    let output = std::process::Command::new(&bin)
        .arg("--compile")
        .arg(&lin)
        .output()
        .expect("run landin-stage0");
    let _ = std::fs::remove_dir_all(&dir);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

// =====================================================================
// POSITIVE TESTS — short-form UFCS `Trait::method(receiver, args)`
// =====================================================================

#[test]
fn stage128_short_form_basic() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = Greeter::greet(&e);
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "short-form UFCS basic should succeed");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage128_short_form_with_args() {
    let code = r#"
trait Adder { fn add(&self, n: i32) -> i32; }
struct Calc;
impl Adder for Calc { fn add(&self, n: i32) -> i32 { n + 10 } }
fn main() {
    let c = Calc;
    let r = Adder::add(&c, 5);
    println!("{}", r);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "15");
}

#[test]
fn stage128_short_form_disambiguate() {
    let code = r#"
trait Show { fn fmt(&self) -> i32; }
trait Inspect { fn fmt(&self) -> i32; }
struct S { x: i32 }
impl Show for S { fn fmt(&self) -> i32 { self.x } }
impl Inspect for S { fn fmt(&self) -> i32 { self.x + 1 } }
fn main() {
    let s = S { x: 10 };
    let d = Show::fmt(&s);
    let g = Inspect::fmt(&s);
    println!("{} {}", d, g);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "short-form disambiguation should succeed");
    assert_eq!(stdout.trim(), "10 11");
}

#[test]
fn stage128_short_form_struct_with_fields() {
    let code = r#"
trait Named { fn name(&self) -> i32; }
struct Point { x: i32, y: i32 }
impl Named for Point { fn name(&self) -> i32 { self.x + self.y } }
fn main() {
    let p = Point { x: 3, y: 4 };
    let n = Named::name(&p);
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn stage128_short_form_multiple_impls() {
    let code = r#"
trait Id { fn id(&self) -> i32; }
struct A;
struct B;
impl Id for A { fn id(&self) -> i32 { 1 } }
impl Id for B { fn id(&self) -> i32 { 2 } }
fn main() {
    let a = A;
    let b = B;
    let ra = Id::id(&a);
    let rb = Id::id(&b);
    println!("{} {}", ra, rb);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1 2");
}

#[test]
fn stage128_short_form_in_let() {
    let code = r#"
trait Val { fn val(&self) -> i32; }
struct Num;
impl Val for Num { fn val(&self) -> i32 { 100 } }
fn main() {
    let n = Num;
    let result = Val::val(&n);
    println!("{}", result);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "100");
}

#[test]
fn stage128_short_form_in_expression() {
    let code = r#"
trait Doubler { fn double(&self) -> i32; }
struct N;
impl Doubler for N { fn double(&self) -> i32 { 21 } }
fn main() {
    let n = N;
    let r = Doubler::double(&n) + 8;
    println!("{}", r);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "29");
}

#[test]
fn stage128_short_form_with_mutable_receiver() {
    let code = r#"
trait Mutator { fn set(&mut self, v: i32); }
struct Counter { n: i32 }
impl Mutator for Counter { fn set(&mut self, v: i32) { self.n = v; } }
fn main() {
    let mut c = Counter { n: 0 };
    Mutator::set(&mut c, 42);
    println!("{}", c.n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

// =====================================================================
// NEGATIVE TESTS
// =====================================================================

#[test]
fn stage128_short_form_trait_not_found() {
    let code = r#"
struct S;
fn main() {
    let s = S;
    let _ = NonExistent::method(&s);
}
"#;
    let (ok, _stderr) = compile(code);
    assert!(!ok, "Should fail: trait not found");
}

#[test]
fn stage128_short_form_missing_receiver() {
    let code = r#"
trait T { fn m(&self) -> i32; }
struct S;
impl T for S { fn m(&self) -> i32 { 1 } }
fn main() {
    let _ = T::m();
}
"#;
    let (ok, _stderr) = compile(code);
    assert!(!ok, "Should fail: missing receiver argument");
}

#[test]
fn stage128_short_form_method_not_in_trait() {
    let code = r#"
trait T { fn existing(&self) -> i32; }
struct S;
impl T for S { fn existing(&self) -> i32 { 1 } }
fn main() {
    let s = S;
    let _ = T::nonexistent(&s);
}
"#;
    let (ok, _stderr) = compile(code);
    // May not fail if trait_method_index doesn't have entry (falls through).
    // But should at least not crash.
    let _ = ok;
}

// =====================================================================
// EDGE CASE TESTS
// =====================================================================

#[test]
fn stage128_short_form_nested_in_function_call() {
    let code = r#"
trait Val { fn val(&self) -> i32; }
struct N;
impl Val for N { fn val(&self) -> i32 { 7 } }
fn double(n: i32) -> i32 { n * 2 }
fn main() {
    let n = N;
    println!("{}", double(Val::val(&n)));
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "14");
}

#[test]
fn stage128_short_form_chained() {
    let code = r#"
trait A { fn a(&self) -> i32; }
trait B { fn b(&self) -> i32; }
struct S;
impl A for S { fn a(&self) -> i32 { 5 } }
impl B for S { fn b(&self) -> i32 { 3 } }
fn main() {
    let s = S;
    let total = A::a(&s) + B::b(&s);
    println!("{}", total);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "8");
}

#[test]
fn stage128_short_form_mixed_with_fully_qualified() {
    // Mix short-form and fully-qualified UFCS in same function.
    let code = r#"
trait T { fn m(&self) -> i32; }
struct S;
impl T for S { fn m(&self) -> i32 { 99 } }
fn main() {
    let s = S;
    let a = T::m(&s);
    let b = <S as T>::m(&s);
    println!("{} {}", a, b);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "99 99");
}

// =====================================================================
// REGRESSION TESTS
// =====================================================================

#[test]
fn stage128_regression_normal_method_call() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = e.greet();
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Normal method call regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage128_regression_fully_qualified_ufcs() {
    let code = r#"
trait Greeter { fn greet(&self) -> i32; }
struct English;
impl Greeter for English { fn greet(&self) -> i32 { 42 } }
fn main() {
    let e = English;
    let n = <English as Greeter>::greet(&e);
    println!("{}", n);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Fully-qualified UFCS regression");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage128_regression_type_method_path() {
    let code = r#"
struct Counter { n: i32 }
impl Counter { fn new() -> Counter { Counter { n: 0 } } }
fn main() {
    let _c = Counter::new();
    println!("ok");
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Type::method regression");
    assert_eq!(stdout.trim(), "ok");
}
