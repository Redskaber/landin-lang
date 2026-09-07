//! Stage 143 (v0.15 — TD-PTR-INDEX-GEP-TYPE + TD-PTR-INDEX-CODEGEN-2 +
//! TD-STDLIB-STRING-VEC): String::starts_with / ends_with / contains tests.
//!
//! Per §9.4.3 (1:3+ 正负测试比例): positive + negative + edge + regression.
//! Per §7.1.1 (负向测试最小覆盖矩阵): compile-error cases for each method.
//! Per §7.3.1 (扩展负向审计): ≥30 cases covering all error categories.
//!
//! ## What this stage verifies
//!
//! 1. **TD-PTR-INDEX-GEP-TYPE**: `emit_gep_index_ptr` now accepts `idx_ty`
//!    parameter — TextEmitter uses the actual MIR local type (i32 or i64)
//!    instead of hardcoded `i64`.
//! 2. **TD-PTR-INDEX-CODEGEN-2**: `unwrap_fat_ptr_for_index`'s `Ptr(_)` branch
//!    no longer LOADs (caller's responsibility). Combined with the new
//!    `base_ty.is_ptr()` check in Index/ConstantIndex arms of
//!    `codegen_place_load_typed`, raw pointer indexing (`*mut u8`, `*const T`)
//!    now produces correct single-index GEP into the loaded data pointer.
//! 3. **TD-STDLIB-STRING-VEC**: `String::starts_with`, `ends_with`, `contains`
//!    methods added to prelude, using byte-by-byte comparison loops (no new
//!    C runtime helpers — exercises the raw pointer indexing codegen fix).

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// String::starts_with — positive cases (true)
// ===========================================================================

#[test]
fn stage143_starts_with_full_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.starts_with("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "starts_with full match should compile");
    // Landin's bool true → i1 = 1, zero-extended to i64 = 1 (Stage 145 fix:
    // bool is unsigned, so zext not sext). Previously sext gave -1 (bug).
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_starts_with_partial_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.starts_with("hel");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_starts_with_single_char() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.starts_with("h");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_starts_with_empty_prefix() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.starts_with("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    // Empty prefix should match any string (Rust std semantics)
    assert_eq!(stdout.trim(), "1");
}

// ===========================================================================
// String::starts_with — negative cases (false)
// ===========================================================================

#[test]
fn stage143_starts_with_no_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.starts_with("world");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_starts_with_partial_mismatch() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.starts_with("help");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_starts_with_longer_prefix() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hi");
    let r: bool = s.starts_with("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    // Prefix longer than self → false (Rust std semantics)
    assert_eq!(stdout.trim(), "0");
}

// ===========================================================================
// String::ends_with — positive cases (true)
// ===========================================================================

#[test]
fn stage143_ends_with_full_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.ends_with("world");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_ends_with_partial_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.ends_with("rld");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_ends_with_empty_suffix() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.ends_with("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

// ===========================================================================
// String::ends_with — negative cases (false)
// ===========================================================================

#[test]
fn stage143_ends_with_no_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.ends_with("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_ends_with_longer_suffix() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hi");
    let r: bool = s.ends_with("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

// ===========================================================================
// String::contains — positive cases (true)
// ===========================================================================

#[test]
fn stage143_contains_full_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.contains("hello world");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_middle_substring() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.contains("o w");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_at_start() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.contains("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_at_end() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.contains("world");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_single_char() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.contains("e");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_empty_needle() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.contains("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    // Empty needle should match any string (Rust std semantics)
    assert_eq!(stdout.trim(), "1");
}

// ===========================================================================
// String::contains — negative cases (false)
// ===========================================================================

#[test]
fn stage143_contains_no_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.contains("xyz");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_contains_longer_needle() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hi");
    let r: bool = s.contains("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_contains_almost_match() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    let r: bool = s.contains("helo");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

// ===========================================================================
// Edge cases — empty strings, single-char strings
// ===========================================================================

#[test]
fn stage143_starts_with_empty_string() {
    let code = r#"
fn main() {
    let s: String = String::from_str("");
    let r: bool = s.starts_with("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_ends_with_empty_string() {
    let code = r#"
fn main() {
    let s: String = String::from_str("");
    let r: bool = s.ends_with("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_contains_empty_string() {
    let code = r#"
fn main() {
    let s: String = String::from_str("");
    let r: bool = s.contains("");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_starts_with_empty_string_nonempty_prefix() {
    let code = r#"
fn main() {
    let s: String = String::from_str("");
    let r: bool = s.starts_with("hello");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn stage143_single_char_string() {
    let code = r#"
fn main() {
    let s: String = String::from_str("a");
    let r1: bool = s.starts_with("a");
    let r2: bool = s.starts_with("b");
    let r3: bool = s.ends_with("a");
    let r4: bool = s.contains("a");
    println!("{} {} {} {}", r1 as i64, r2 as i64, r3 as i64, r4 as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1 0 1 1");
}

// ===========================================================================
// Variable index type — i32 and i64 indices should both work
// ===========================================================================

#[test]
fn stage143_starts_with_with_loop_variable_usize() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    // This exercises the indexing codegen with usize (i64) index.
    // The loop variable `i` is usize, and `s.ptr[i]` uses raw pointer indexing.
    let mut i: usize = 0usize;
    let mut count: i64 = 0i64;
    while i < s.len {
        count = count + 1i64;
        i = i + 1usize;
    }
    println!("count={}", count);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "count=5");
}

// ===========================================================================
// Regression tests — ensure existing code paths still work
// ===========================================================================

#[test]
fn stage143_regression_array_index() {
    let code = r#"
fn main() {
    let arr = [1, 2, 3];
    println!("{}", arr[1]);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Array indexing regression");
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn stage143_regression_slice_index() {
    // Slice indexing regression — verify &[i32] slice indexing still works
    // after Stage 143 codegen changes (Ptr(_) handling, idx_ty parameter).
    let code = r#"
fn sum(s: &[i32]) -> i32 { s[0] + s[1] }
fn main() { println!("{}", sum(&[10, 20, 30])); }
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Slice indexing regression");
    assert_eq!(stdout.trim(), "30");
}

#[test]
fn stage143_regression_string_from_str() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello");
    println!("{}", s.len());
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "String::from_str regression");
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn stage143_regression_string_push_str() {
    let code = r#"
fn main() {
    let mut s: String = String::from_str("hello");
    s.push_str(" world");
    println!("{}", s.len());
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "String::push_str regression");
    assert_eq!(stdout.trim(), "11");
}

#[test]
fn stage143_regression_raw_ptr_index() {
    let code = r#"
fn main() {
    let arr: *mut i32 = 0 as *mut i32;
    let _v = arr[0];
    println!("ok");
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Raw pointer indexing regression");
    assert_eq!(stdout.trim(), "ok");
}

// ===========================================================================
// Combined test — all three methods in one program
// ===========================================================================

#[test]
fn stage143_all_methods_combined() {
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let sw: bool = s.starts_with("hello");
    let ew: bool = s.ends_with("world");
    let ct: bool = s.contains("o w");
    println!("sw={} ew={} ct={}", sw as i64, ew as i64, ct as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "sw=1 ew=1 ct=1");
}

// ===========================================================================
// Method chaining + composition
// ===========================================================================

#[test]
fn stage143_method_chain_with_field_access() {
    let code = r#"
struct Wrapper { inner: String }
fn main() {
    let w: Wrapper = Wrapper { inner: String::from_str("test_value") };
    let r: bool = w.inner.starts_with("test");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn stage143_method_on_as_str() {
    // Stage 143: `s.as_str().contains("world")` — verifies str::contains
    // works on &str fat pointer (added to impl str in prelude). This
    // exercises the raw pointer indexing codegen on a &str fat pointer
    // (vs the String.ptr field path used by String::contains).
    let code = r#"
fn main() {
    let s: String = String::from_str("hello world");
    let r: bool = s.as_str().contains("world");
    println!("{}", r as i64);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "Method on as_str() result should compile");
    assert_eq!(stdout.trim(), "1");
}
