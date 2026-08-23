//! Stage 18.188 — String::new + function redefine bug fix tests.
//!
//! Verifies:
//! 1. `String::new()` returns an empty String (len = 0).
//! 2. The LLVM function redefine bug is fixed (struct-returning functions
//!    whose forward declaration was auto-created with wrong type).
//! 3. Multiple struct-returning functions coexist correctly.
//! 4. User struct ::new() + prelude String::new() work together.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file = std::env::temp_dir().join(format!(
        "landin_s188_test_{}_{}.lin",
        std::process::id(),
        id
    ));
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

#[test]
fn stage18_188_string_new_empty() {
    assert_runtime(
        "string-new-empty",
        r#"
fn main() -> i32 {
    let s: String = String::new();
    println!("{}", s.len);
    0
}
"#,
        "0\n",
    );
}

#[test]
fn stage18_188_string_new_no_println() {
    // This was the original failing case — no println, just String::new()
    assert_runtime(
        "string-new-no-println",
        r#"
fn main() -> i32 {
    let s: String = String::new();
    0
}
"#,
        "",
    );
}

#[test]
fn stage18_188_user_struct_new() {
    assert_runtime(
        "user-struct-new",
        r#"
struct Foo { x: i32 }
impl Foo {
    fn new() -> Foo { Foo { x: 42 } }
}
fn main() -> i32 {
    let f: Foo = Foo::new();
    println!("{}", f.x);
    0
}
"#,
        "42\n",
    );
}

#[test]
fn stage18_188_both_news_together() {
    // The canonical regression test: Foo::new + String::new together
    assert_runtime(
        "both-news-together",
        r#"
struct Foo { x: i32 }
impl Foo {
    fn new() -> Foo { Foo { x: 42 } }
}
fn main() -> i32 {
    let f: Foo = Foo::new();
    let s: String = String::new();
    println!("{} {}", f.x, s.len);
    0
}
"#,
        "42 0\n",
    );
}

#[test]
fn stage18_188_string_new_then_from_str() {
    // Mix String::new() with String::from_str()
    assert_runtime(
        "string-new-then-from-str",
        r#"
fn main() -> i32 {
    let empty: String = String::new();
    let hello: String = String::from_str("hello");
    println!("{} {}", empty.len, hello.len);
    0
}
"#,
        "0 5\n",
    );
}
