//! Stage 18.188 — String::new + function redefine bug fix tests.
//!
//! Verifies:
//! 1. `String::new()` returns an empty String (len = 0).
//! 2. The LLVM function redefine bug is fixed (struct-returning functions
//!    whose forward declaration was auto-created with wrong type).
//! 3. Multiple struct-returning functions coexist correctly.
//! 4. User struct ::new() + prelude String::new() work together.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

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
