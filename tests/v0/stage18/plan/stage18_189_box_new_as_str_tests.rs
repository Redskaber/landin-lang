//! Stage 18.189 — Box::new + String::as_str tests.
//!
//! Verifies:
//! 1. `Box::new(x)` allocates and stores x, accessible via `*b.0`.
//! 2. `Box::new` works with different types (i32, u8, i64).
//! 3. `String::as_str()` returns a &str with correct length.
//! 4. `String::as_str()` + str methods (len, is_empty) work together.
//! 5. Multiple Box::new calls create independent allocations.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

// === Box::new tests ===

#[test]
fn stage18_189_box_new_i32() {
    assert_runtime(
        "box-new-i32",
        r#"
fn main() -> i32 {
    let b: Box<i32> = Box::new(42);
    let v: i32 = *b.0;
    println!("{}", v);
    0
}
"#,
        "42\n",
    );
}

#[test]
fn stage18_189_box_new_u8() {
    assert_runtime(
        "box-new-u8",
        r#"
fn main() -> i32 {
    let b: Box<u8> = Box::new(255);
    let v: u8 = *b.0;
    println!("{}", v);
    0
}
"#,
        "255\n",
    );
}

/// Stage 18.190 (TD-BOX-NEW-TYPE-COERCE fix): Box<i64> now works correctly
/// for values that fit in i32 (the pointer bitcast fix in emit_store handles
/// the *mut u8 → *mut i64 cast). Large i64 values (> i32 max) are a separate
/// pre-existing issue (TD-INT-UINT-VAR: literals default to i32).
#[test]
fn stage18_189_box_new_i64() {
    assert_runtime(
        "box-new-i64",
        r#"
fn main() -> i32 {
    let b: Box<i64> = Box::new(42);
    let v: i64 = *b.0;
    println!("{}", v);
    0
}
"#,
        "42\n",
    );
}

#[test]
fn stage18_189_box_new_multiple() {
    assert_runtime(
        "box-new-multiple",
        r#"
fn main() -> i32 {
    let b1: Box<i32> = Box::new(10);
    let b2: Box<i32> = Box::new(20);
    println!("{} {}", *b1.0, *b2.0);
    0
}
"#,
        "10 20\n",
    );
}

// === String::as_str tests ===

#[test]
fn stage18_189_string_as_str_len() {
    assert_runtime(
        "string-as-str-len",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{}", sref.len());
    0
}
"#,
        "5\n",
    );
}

#[test]
fn stage18_189_string_as_str_is_empty() {
    assert_runtime(
        "string-as-str-is-empty",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("Hello, World!");
    let sref: &str = s.as_str();
    println!("{} {}", sref.len(), sref.is_empty());
    0
}
"#,
        "13 false\n",
    );
}

#[test]
fn stage18_189_string_as_str_empty() {
    assert_runtime(
        "string-as-str-empty",
        r#"
fn main() -> i32 {
    let s: String = String::new();
    let sref: &str = s.as_str();
    println!("{} {}", sref.len(), sref.is_empty());
    0
}
"#,
        "0 true\n",
    );
}

#[test]
fn stage18_189_string_as_str_byte_index() {
    assert_runtime(
        "string-as-str-byte-index",
        r#"
fn main() -> i32 {
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{}", sref[0]);
    println!("{}", sref[4]);
    0
}
"#,
        "104\n111\n", // 'h' 'o'
    );
}

// === Box::new + String combined ===

#[test]
fn stage18_189_box_and_string_combined() {
    assert_runtime(
        "box-and-string-combined",
        r#"
fn main() -> i32 {
    let b: Box<i32> = Box::new(42);
    let s: String = String::from_str("hello");
    let sref: &str = s.as_str();
    println!("{} {}", *b.0, sref.len());
    0
}
"#,
        "42 5\n",
    );
}
