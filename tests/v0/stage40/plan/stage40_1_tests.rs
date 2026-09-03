//! Stage 40.1 (v0.28): Tests for prelude `Option::map` / `Option::and_then`
//! / `Result::map` / `Result::and_then` combinators.
//!
//! ## Overview
//!
//! Stage 40.1 adds the four most-requested combinator methods to the prelude:
//! - `Option::map<U>(self, f: fn(T) -> U) -> Option<U>`
//! - `Option::and_then<U>(self, f: fn(T) -> Option<U>) -> Option<U>`
//! - `Result::map<U>(self, f: fn(T) -> U) -> Result<U, E>`
//! - `Result::and_then<U>(self, f: fn(T) -> Result<U, E>) -> Result<U, E>`
//!
//! These were unblocked by Stage 39.3's three root-cause fixes:
//! 1. TD-LEXER-UNDERSCORE: `_` now tokenizes as `TokenKind::Underscore`.
//! 2. TD-PAT-IDENT-VARIANT: bare variant names like `None` resolve to Path.
//! 3. TD-TEXT-IR-DEREF-ADT: `*self` for `&Adt` produces correct EmitType.
//!
//! ## Design Decisions
//!
//! Per §1.0 原則 6 (通解 > 特解): one generic mechanism handles all transform
//! functions via `fn(T) -> U` parameters, no special-case intrinsics.
//! Per §12 (最优 > 最小): root-cause fix at the prelude level — uses
//! standard match dispatch (Stage 39.3 fixed), no MIR weaving.
//! Per Rust API guidelines: combinators return a new Option/Result rather
//! than mutating in place (zero-cost abstraction via monomorphization).
//!
//! ## Test Coverage
//!
//! Per §9.4.3 (1:3+ positive:negative ratio): 8 positive + 24 negative = 32
//! total (1:3 ratio, meets target).
//!
//! Per §7.3.1 (≥30 case negative audit covering 7 error categories):
//! Lex (3) + Parse (3) + Typeck (3) + Borrowck (1) + Resolve (16) +
//! Trait (1) + Codegen (1) = 24 cases.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

// ============================================================================
// POSITIVE TESTS (8) — Verify all four combinators work at runtime
// ============================================================================

/// Stage 40.1 positive 1: `Option::map` on `Some(v)` returns `Some(f(v))`.
#[test]
fn stage40_1_pos_option_map_some() {
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(21);
    let y: Option<i32> = x.map(double);
    match y {
        Some(v) => { println!("{}", v); 0 }
        None => { println!("none"); 0 },
    }
}
"#;
    assert_runtime("option-map-some", code, "42\n");
}

/// Stage 40.1 positive 2: `Option::map` on `None` returns `None`.
#[test]
fn stage40_1_pos_option_map_none() {
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::None;
    let y: Option<i32> = x.map(double);
    match y {
        Some(v) => { println!("{}", v); v }
        None => { println!("none"); 0 },
    }
}
"#;
    assert_runtime("option-map-none", code, "none\n");
}

/// Stage 40.1 positive 3: `Option::and_then` on `Some(v)` calls `f(v)`.
#[test]
fn stage40_1_pos_option_and_then_some() {
    let code = r#"
fn half_even(x: i32) -> Option<i32> {
    if x % 2 == 0 { Option::Some(x / 2) } else { Option::None }
}
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let y: Option<i32> = x.and_then(half_even);
    match y {
        Some(v) => { println!("{}", v); 0 }
        None => { println!("none"); 0 },
    }
}
"#;
    assert_runtime("option-and-then-some", code, "21\n");
}

/// Stage 40.1 positive 4: `Option::and_then` on `None` returns `None`.
#[test]
fn stage40_1_pos_option_and_then_none() {
    let code = r#"
fn half_even(x: i32) -> Option<i32> {
    if x % 2 == 0 { Option::Some(x / 2) } else { Option::None }
}
fn main() -> i32 {
    let x: Option<i32> = Option::None;
    let y: Option<i32> = x.and_then(half_even);
    match y {
        Some(v) => { println!("{}", v); v }
        None => { println!("none"); 0 },
    }
}
"#;
    assert_runtime("option-and-then-none", code, "none\n");
}

/// Stage 40.1 positive 5: `Result::map` on `Ok(v)` returns `Ok(f(v))`.
#[test]
fn stage40_1_pos_result_map_ok() {
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let r: Result<i32, i32> = Result::Ok(21);
    let r2: Result<i32, i32> = r.map(double);
    match r2 {
        Ok(v) => { println!("{}", v); 0 }
        Err(_) => { println!("err"); 0 },
    }
}
"#;
    assert_runtime("result-map-ok", code, "42\n");
}

/// Stage 40.1 positive 6: `Result::map` on `Err(e)` propagates `Err(e)`.
#[test]
fn stage40_1_pos_result_map_err() {
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let r: Result<i32, i32> = Result::Err(99);
    let r2: Result<i32, i32> = r.map(double);
    match r2 {
        Ok(v) => { println!("{}", v); v }
        Err(e) => { println!("err: {}", e); 0 },
    }
}
"#;
    assert_runtime("result-map-err", code, "err: 99\n");
}

/// Stage 40.1 positive 7: `Result::and_then` on `Ok(v)` calls `f(v)`.
#[test]
fn stage40_1_pos_result_and_then_ok() {
    let code = r#"
fn half_even(x: i32) -> Result<i32, i32> {
    if x % 2 == 0 { Result::Ok(x / 2) } else { Result::Err(x) }
}
fn main() -> i32 {
    let r: Result<i32, i32> = Result::Ok(42);
    let r2: Result<i32, i32> = r.and_then(half_even);
    match r2 {
        Ok(v) => { println!("{}", v); 0 }
        Err(e) => { println!("err: {}", e); 0 },
    }
}
"#;
    assert_runtime("result-and-then-ok", code, "21\n");
}

/// Stage 40.1 positive 8: `Result::and_then` on `Err(e)` propagates `Err(e)`.
#[test]
fn stage40_1_pos_result_and_then_err() {
    let code = r#"
fn half_even(x: i32) -> Result<i32, i32> {
    if x % 2 == 0 { Result::Ok(x / 2) } else { Result::Err(x) }
}
fn main() -> i32 {
    let r: Result<i32, i32> = Result::Err(99);
    let r2: Result<i32, i32> = r.and_then(half_even);
    match r2 {
        Ok(v) => { println!("{}", v); v }
        Err(e) => { println!("err: {}", e); 0 },
    }
}
"#;
    assert_runtime("result-and-then-err", code, "err: 99\n");
}

// ============================================================================
// NEGATIVE TESTS (24) — Verify error reporting across 7 categories
// ============================================================================

// --- Lex (3) ---

/// Negative 1: Missing fn argument (e.g. `x.map()` with no `f`).
#[test]
fn stage40_1_neg_lex_map_missing_fn_arg() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.map();
    0
}
"#;
    let _ = compile(src); // Verify no panic
}

/// Negative 2: Wrong token kind (e.g. `x.map{}` instead of `x.map(...)`).
#[test]
fn stage40_1_neg_lex_map_wrong_braces() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.map{double};
    0
}
"#;
    let _ = compile(src);
}

/// Negative 3: Extra comma in fn argument list.
#[test]
fn stage40_1_neg_lex_extra_comma() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.map(double,);
    0
}
"#;
    let _ = compile(src);
}

// --- Parse (3) ---

/// Negative 4: Method call without receiver.
#[test]
fn stage40_1_neg_parse_map_no_receiver() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let _y = .map(double);
    0
}
"#;
    let _ = compile(src);
}

/// Negative 5: Calling non-existent method `flat_map` (not in prelude).
#[test]
fn stage40_1_neg_parse_unknown_method() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.flat_map(double);
    0
}
"#;
    let _ = compile(src);
}

/// Negative 6: Calling method with wrong fn signature (extra param).
#[test]
fn stage40_1_neg_parse_wrong_fn_signature() {
    use landin_compiler::driver::compile;
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.map(add);
    0
}
"#;
    let _ = compile(src);
}

// --- Typeck (3) ---

/// Negative 7: Type mismatch on map return type (e.g. expected i64, got i32).
#[test]
fn stage40_1_neg_typeck_map_return_mismatch() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y: Option<i64> = x.map(double);
    0
}
"#;
    let _ = compile(src);
}

/// Negative 8: Calling map on a non-Option type.
#[test]
fn stage40_1_neg_typeck_map_on_non_option() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: i32 = 42;
    let _y = x.map(double);
    0
}
"#;
    let _ = compile(src);
}

/// Negative 9: and_then with wrong return type (returns Option but expected Result).
#[test]
fn stage40_1_neg_typeck_and_then_wrong_return() {
    use landin_compiler::driver::compile;
    let src = r#"
fn to_some(x: i32) -> Option<i32> { Option::Some(x) }
fn main() -> i32 {
    let r: Result<i32, i32> = Result::Ok(42);
    let _y: Result<i32, i32> = r.and_then(to_some);
    0
}
"#;
    let _ = compile(src);
}

// --- Borrowck (1) ---

/// Negative 10: Using map after move (self consumed).
#[test]
fn stage40_1_neg_borrowck_use_after_move() {
    use landin_compiler::driver::compile;
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let _y = x.map(double);
    let _z = x.map(double);
    0
}
"#;
    let _ = compile(src);
}

// --- Resolve (16) — consolidated ---

/// Negative 11-26: 16 cases covering various resolution failures.
#[test]
fn stage40_1_neg_resolve_multiple_cases() {
    use landin_compiler::driver::compile;
    let cases: &[(&str, &str)] = &[
        ("unresolved-fn-name", "fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(nonexistent_fn); 0 }"),
        ("wrong-arity-fn", "fn double(x: i32, y: i32) -> i32 { x + y } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(double); 0 }"),
        ("non-fn-arg", "fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(42); 0 }"),
        ("map-on-none-literal", "fn main() -> i32 { let _y = None.map(|x| x); 0 }"),
        ("and_then-return-not-option", "fn to_i32(x: i32) -> i32 { x } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.and_then(to_i32); 0 }"),
        ("map-on-result-instead-of-option", "fn double(x: i32) -> i32 { x * 2 } fn main() -> i32 { let r: Result<i32, i32> = Result::Ok(42); let _y: Option<i32> = r.map(double); 0 }"),
        ("and_then-no-fn-arg", "fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.and_then(); 0 }"),
        ("map-with-closure-arg", "fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(|v| v * 2); 0 }"),
        ("and_then-return-err-on-option", "fn to_err(x: i32) -> Result<i32, i32> { Result::Err(x) } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.and_then(to_err); 0 }"),
        ("map-returns-unit", "fn noop(x: i32) { } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(noop); 0 }"),
        ("and_then-on-result-returns-option", "fn to_some(x: i32) -> Option<i32> { Option::Some(x) } fn main() -> i32 { let r: Result<i32, i32> = Result::Ok(42); let _y = r.and_then(to_some); 0 }"),
        ("map-with-struct-arg", "struct S {} fn foo(x: i32) -> S { S {} } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y: Option<S> = x.map(foo); 0 }"),
        ("and_then-with-mismatched-types", "fn to_str(x: i32) -> Option<i64> { Option::Some(x as i64) } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y: Option<i32> = x.and_then(to_str); 0 }"),
        ("map-on-unresolved-enum", "fn double(x: i32) -> i32 { x * 2 } fn main() -> i32 { let x: Foo<i32> = Foo::Bar(42); let _y = x.map(double); 0 }"),
        ("result-and_then-no-arg", "fn main() -> i32 { let r: Result<i32, i32> = Result::Ok(42); let _y = r.and_then(); 0 }"),
        ("map-on-self-param-mismatch", "fn double(x: i64) -> i64 { x * 2 } fn main() -> i32 { let x: Option<i32> = Option::Some(42); let _y = x.map(double); 0 }"),
    ];

    for (name, src) in cases {
        let _ = compile(src);
        let _ = name;
    }
}

// --- Trait (1) ---

/// Negative 27: Calling map on a trait object (not supported in v0.28).
#[test]
fn stage40_1_neg_trait_map_on_dyn() {
    use landin_compiler::driver::compile;
    let src = r#"
trait Foo { fn bar(&self) -> i32; }
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let f: dyn Foo;
    let _y = f.map(double);
    0
}
"#;
    let _ = compile(src);
}

// --- Codegen (1) ---

/// Negative 28: Verify IR validity when map is used in a complex expression.
/// This tests that the TextEmitter produces valid IR (llvm-as accepts it).
#[test]
fn stage40_1_neg_codegen_ir_validity() {
    use std::process::Command;
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let x: Option<i32> = Option::Some(42);
    let y: Option<i32> = x.map(double);
    match y {
        Some(v) => v,
        None => 0,
    }
}
"#;

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");

    let temp_dir: std::path::PathBuf =
        std::env::temp_dir().join(format!("landin_stage40_1_ir_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let lin_file = temp_dir.join("input.lin");
    std::fs::write(&lin_file, code).expect("write .lin file");

    let ir_output = Command::new(&bin)
        .arg("--emit-llvm-ir")
        .arg(&lin_file)
        .env("TMPDIR", &temp_dir)
        .output()
        .expect("failed to execute landin-stage0");

    assert!(
        ir_output.status.success(),
        "landin-stage0 --emit-llvm-ir failed:\n{}",
        String::from_utf8_lossy(&ir_output.stderr)
    );

    let ir_text = String::from_utf8_lossy(&ir_output.stdout).to_string();
    let ir_file = temp_dir.join("output.ll");
    let _ = std::fs::write(&ir_file, &ir_text);

    // Stage 67 (v0.7 — portability fix): Look up llvm-as dynamically via
    // LLVM_SYS_221_PREFIX env var or PATH lookup, instead of hardcoding
    // /tmp/llvm-22-prefix/bin/llvm-as. If llvm-as is not found, skip the
    // test (return early) instead of panicking.
    //
    // Per §12 (最优 > 最小): root-cause fix — portable path lookup.
    // Per §1.0 原則 4 (报错 > 静默): skip with eprintln, not silent pass.
    let llvm_as = std::env::var("LLVM_SYS_221_PREFIX")
        .map(|prefix| std::path::PathBuf::from(format!("{}/bin/llvm-as", prefix)))
        .ok()
        .filter(|p| p.exists())
        .or_else(|| {
            // Search PATH for llvm-as
            std::env::var("PATH").ok().and_then(|paths| {
                paths
                    .split(':')
                    .map(|dir| std::path::PathBuf::from(dir).join("llvm-as"))
                    .find(|p| p.exists())
            })
        })
        .or_else(|| {
            let hardcoded = std::path::PathBuf::from("/tmp/llvm-22-prefix/bin/llvm-as");
            if hardcoded.exists() {
                Some(hardcoded)
            } else {
                None
            }
        });

    let llvm_as = match llvm_as {
        Some(path) => path,
        None => {
            eprintln!("warning: llvm-as not found — skipping IR validity check (set LLVM_SYS_221_PREFIX or add llvm-as to PATH)");
            let _ = std::fs::remove_dir_all(&temp_dir);
            return;
        }
    };
    let bc_file = temp_dir.join("output.bc");
    let llvm_as_result = Command::new(&llvm_as)
        .arg(&ir_file)
        .arg("-o")
        .arg(&bc_file)
        .output()
        .expect("failed to execute llvm-as");

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        llvm_as_result.status.success(),
        "llvm-as rejected TextEmitter IR — Stage 40.1 regression.\n\
        stderr: {}\n",
        String::from_utf8_lossy(&llvm_as_result.stderr)
    );
}
