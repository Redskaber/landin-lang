//! Stage 13.18 — Runtime verification tests (--run based, not just --compile)
//!
//! These tests verify that Landin programs produce correct RUNTIME output
//! when executed via `--run`, not just that they compile. This closes the
//! gap identified in the user feedback: conformance tests only run `--compile`,
//! which cannot catch runtime bugs.
//!
//! **IMPORTANT**: These tests require the `llvm-backend` feature to be enabled
//! (the `--run` flag needs LLVM to compile to object code + link). They are
//! gated behind `#[cfg(feature = "llvm-backend")]` and will be skipped if
//! the feature is not enabled.
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8 +
//! `stage-13.18-design-alignment.md` + `gate-review-13.18.md`.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;

/// Helper: compile + run a Landin program and return (stdout, exit_code).
fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };
    // Use a unique file name per test invocation to avoid parallel conflicts.
    // Include a counter via atomic to ensure uniqueness.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_rt_test_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");

    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .output()
        .expect("failed to execute landin-stage0");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let _ = std::fs::remove_file(&lin_file);
    (stdout, output.status.code().unwrap_or(-1))
}

/// Helper: assert a program produces expected stdout and exit code 0.
fn assert_runtime(name: &str, code: &str, expected_stdout: &str) {
    let (stdout, exit) = run_program(code);
    assert_eq!(
        stdout, expected_stdout,
        "Test '{}': stdout mismatch.\nExpected: {:?}\nGot:      {:?}",
        name, expected_stdout, stdout
    );
    assert_eq!(
        exit, 0,
        "Test '{}': exit code mismatch (expected 0, got {})",
        name, exit
    );
}

// === Arithmetic ===

#[test]
fn rt_add() {
    assert_runtime(
        "add",
        "fn main() -> i32 { let a = 3; let b = 4; println!(\"{}\", a + b); 0 }",
        "7\n",
    );
}

#[test]
fn rt_sub() {
    assert_runtime(
        "sub",
        "fn main() -> i32 { println!(\"{}\", 10 - 3); 0 }",
        "7\n",
    );
}

#[test]
fn rt_mul() {
    assert_runtime(
        "mul",
        "fn main() -> i32 { println!(\"{}\", 6 * 7); 0 }",
        "42\n",
    );
}

#[test]
fn rt_div() {
    assert_runtime(
        "div",
        "fn main() -> i32 { println!(\"{}\", 20 / 4); 0 }",
        "5\n",
    );
}

#[test]
fn rt_mod() {
    assert_runtime(
        "mod",
        "fn main() -> i32 { println!(\"{}\", 17 % 5); 0 }",
        "2\n",
    );
}

// === Control Flow ===

#[test]
fn rt_if_true() {
    assert_runtime(
        "if-true",
        "fn main() -> i32 { if 1 < 2 { println!(\"yes\"); } else { println!(\"no\"); } 0 }",
        "yes\n",
    );
}

#[test]
fn rt_if_false() {
    assert_runtime(
        "if-false",
        "fn main() -> i32 { if 1 > 2 { println!(\"yes\"); } else { println!(\"no\"); } 0 }",
        "no\n",
    );
}

#[test]
fn rt_while() {
    assert_runtime(
        "while",
        "fn main() -> i32 { let mut i = 0; while i < 3 { println!(\"{}\", i); i = i + 1; } 0 }",
        "0\n1\n2\n",
    );
}

#[test]
fn rt_match() {
    assert_runtime("match", "fn main() -> i32 { let x = 2; match x { 1 => { println!(\"one\"); } 2 => { println!(\"two\"); } _ => { println!(\"other\"); } } 0 }", "two\n");
}

// === Functions ===

#[test]
fn rt_fn_call() {
    assert_runtime("fn-call", "fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { println!(\"{}\", add(3, 4)); 0 }", "7\n");
}

#[test]
fn rt_recursion() {
    assert_runtime("recursion", "fn fib(n: i32) -> i32 { if n < 2 { n } else { fib(n-1) + fib(n-2) } } fn main() -> i32 { println!(\"{}\", fib(10)); 0 }", "55\n");
}

// === Structs ===

#[test]
fn rt_struct_field() {
    assert_runtime("struct-field", "struct P { x: i32, y: i32 } fn main() -> i32 { let p = P { x: 10, y: 20 }; println!(\"{}\", p.x + p.y); 0 }", "30\n");
}

// === Method Calls (Stage 13.17 + 13.18) ===

#[test]
fn rt_method_no_self() {
    assert_runtime(
        "method-no-self",
        "struct P { x: i32 } impl P { fn get(self) -> i32 { 42 } } fn main() -> i32 { let p = P { x: 1 }; println!(\"{}\", p.get()); 0 }",
        "42\n",
    );
}

#[test]
fn rt_method_self_x() {
    assert_runtime(
        "method-self-x",
        "struct P { x: i32 } impl P { fn get(self) -> i32 { self.x } } fn main() -> i32 { let p = P { x: 42 }; println!(\"{}\", p.get()); 0 }",
        "42\n",
    );
}

#[test]
fn rt_method_ref_self() {
    assert_runtime(
        "method-ref-self",
        "struct P { x: i32 } impl P { fn get(&self) -> i32 { self.x } } fn main() -> i32 { let p = P { x: 42 }; println!(\"{}\", p.get()); 0 }",
        "42\n",
    );
}

#[test]
fn rt_method_two_fields() {
    assert_runtime(
        "method-two-fields",
        "struct P { x: i32, y: i32 } impl P { fn sum(self) -> i32 { self.x + self.y } } fn main() -> i32 { let p = P { x: 10, y: 20 }; println!(\"{}\", p.sum()); 0 }",
        "30\n",
    );
}

// === Tuples ===

#[test]
fn rt_tuple_access() {
    assert_runtime(
        "tuple-access",
        "fn main() -> i32 { let t = (1, 2, 3); println!(\"{}\", t.0 + t.1 + t.2); 0 }",
        "6\n",
    );
}

// === Enums ===

#[test]
fn rt_enum_match() {
    assert_runtime(
        "enum-match",
        "enum Opt { None, Some(i32) } fn main() -> i32 { let x = Opt::Some(42); match x { Opt::Some(v) => { println!(\"{}\", v); } Opt::None => { println!(\"none\"); } } 0 }",
        "42\n",
    );
}

// === References ===

#[test]
fn rt_ref_param() {
    assert_runtime(
        "ref-param",
        "fn inc(x: &i32) -> i32 { *x + 1 } fn main() -> i32 { let a = 10; println!(\"{}\", inc(&a)); 0 }",
        "11\n",
    );
}

// === Closures ===

#[test]
fn rt_closure() {
    assert_runtime(
        "closure",
        "fn main() -> i32 { let f = |x: i32| x + 1; println!(\"{}\", f(10)); 0 }",
        "11\n",
    );
}

// === Return Values ===

#[test]
fn rt_return_value() {
    let (stdout, exit) = run_program("fn main() -> i32 { 42 }");
    assert_eq!(stdout, "", "return value test: stdout should be empty");
    assert_eq!(exit, 42, "return value test: exit code should be 42");
}

#[test]
fn rt_return_value_7() {
    let (stdout, exit) = run_program("fn main() -> i32 { 7 }");
    assert_eq!(stdout, "");
    assert_eq!(exit, 7);
}

// === String Output ===

#[test]
fn rt_string_literal() {
    assert_runtime(
        "string-literal",
        "fn main() -> i32 { println!(\"hello world\"); 0 }",
        "hello world\n",
    );
}

// === Multiple println ===

#[test]
fn rt_multiple_println() {
    assert_runtime(
        "multiple-println",
        "fn main() -> i32 { println!(\"line 1\"); println!(\"line 2\"); println!(\"line 3\"); 0 }",
        "line 1\nline 2\nline 3\n",
    );
}

// === eprintln (stderr) ===

#[test]
fn rt_eprintln_stderr() {
    let (stdout, _exit) =
        run_program("fn main() -> i32 { eprintln!(\"to stderr\"); println!(\"to stdout\"); 0 }");
    // stdout should only have "to stdout\n" (eprintln goes to stderr)
    assert_eq!(stdout, "to stdout\n");
}

// === Break/Continue (Stage 13.19) ===

#[test]
fn rt_break() {
    assert_runtime(
        "break",
        "fn main() -> i32 { let mut i = 0; while i < 10 { if i == 3 { break; } println!(\"{}\", i); i = i + 1; } 0 }",
        "0\n1\n2\n",
    );
}

#[test]
fn rt_continue() {
    assert_runtime(
        "continue",
        "fn main() -> i32 { let mut i = 0; while i < 5 { i = i + 1; if i == 3 { continue; } println!(\"{}\", i); } 0 }",
        "1\n2\n4\n5\n",
    );
}

#[test]
fn rt_loop_break() {
    assert_runtime(
        "loop-break",
        "fn main() -> i32 { let mut i = 0; loop { if i >= 3 { break; } println!(\"{}\", i); i = i + 1; } 0 }",
        "0\n1\n2\n",
    );
}

// === String Variables (Stage 13.20) ===

#[test]
fn rt_string_var() {
    assert_runtime(
        "string-var",
        "fn main() -> i32 { let s = \"hello\"; println!(\"{}\", s); 0 }",
        "hello\n",
    );
}

#[test]
fn rt_string_direct() {
    assert_runtime(
        "string-direct",
        "fn main() -> i32 { println!(\"{}\", \"world\"); 0 }",
        "world\n",
    );
}

#[test]
fn rt_string_multi() {
    assert_runtime(
        "string-multi",
        "fn main() -> i32 { let a = \"foo\"; let b = \"bar\"; println!(\"{} {}\", a, b); 0 }",
        "foo bar\n",
    );
}

// === Negative Numbers (Stage 13.21) ===

#[test]
fn rt_negative_number() {
    assert_runtime(
        "negative-number",
        "fn main() -> i32 { let x = -5; println!(\"{}\", x); 0 }",
        "-5\n",
    );
}

#[test]
fn rt_negative_arithmetic() {
    assert_runtime(
        "negative-arithmetic",
        "fn main() -> i32 { let a = 10; let b = 20; println!(\"{}\", a - b); 0 }",
        "-10\n",
    );
}

// === Early Return (Stage 13.21) ===

#[test]
fn rt_early_return() {
    assert_runtime(
        "early-return",
        "fn f(x: i32) -> i32 { if x > 0 { return x; } 0 } fn main() -> i32 { println!(\"{}\", f(5)); 0 }",
        "5\n",
    );
}

#[test]
fn rt_early_return_negative() {
    assert_runtime(
        "early-return-negative",
        "fn f(x: i32) -> i32 { if x < 0 { return -1; } 1 } fn main() -> i32 { println!(\"{}\", f(-5)); 0 }",
        "-1\n",
    );
}
