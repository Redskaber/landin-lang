//! Stage 18.424-18.425 — §20 iterative audit: Index typeck + assignment path.
//!
//! Stage 18.424: typeck `infer_projection` for `ProjectionElem::Index` now
//! pushes errors for non-indexable concrete types (Int, Bool, Float, Adt,
//! Tuple). Was: silently returned None → `n[0]` on integer compiled.
//! Also removed `TyKind::Str => Some(u8)` in typeck (consistency with
//! Stage 18.422 MIR lower fix).
//!
//! Stage 18.425: `check_index_access_syntax` helper added to
//! `lower_expr_to_place` (assignment path). Was: no check → `s[0] = 65`
//! on &str silently compiled.
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §20: iterative audit — same class as Stage 18.412/18.416/18.420/18.422.
//! Per §1.0 原則 4 (报错 > 静默): non-indexable types must error.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_424_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("run landin-stage0");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

// ============================================================================
// Positive tests: valid index assignment (array)
// ============================================================================

#[test]
fn stage18_424_pos_array_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let mut a = [1, 2, 3]; a[0] = 5; 0 }"#);
    assert_eq!(exit, 0, "array[0] = val must compile");
}

#[test]
fn stage18_424_pos_array_assign_var_idx() {
    let exit =
        compile_only(r#"fn main() -> i32 { let mut a = [1, 2, 3]; let i = 1; a[i] = 5; 0 }"#);
    assert_eq!(exit, 0, "array[i] = val with variable index must compile");
}

#[test]
fn stage18_424_pos_array_read_ok() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a[0]; 0 }"#);
    assert_eq!(exit, 0, "array read must compile");
}

// ============================================================================
// Negative tests: indexing non-indexable types (read path, Stage 18.424)
// ============================================================================

#[test]
fn stage18_424_neg_int_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 42; let y = n[0]; 0 }"#);
    assert_ne!(exit, 0, "int[0] read must be rejected");
}

#[test]
fn stage18_424_neg_float_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let f: f64 = 1.5; let y = f[0]; 0 }"#);
    assert_ne!(exit, 0, "f64[0] read must be rejected");
}

#[test]
fn stage18_424_neg_bool_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = true; let y = b[0]; 0 }"#);
    assert_ne!(exit, 0, "bool[0] read must be rejected");
}

#[test]
fn stage18_424_neg_struct_read_index() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let f = Foo { x: 1 }; let y = f[0]; 0 }"#,
    );
    assert_ne!(exit, 0, "struct[0] read must be rejected");
}

#[test]
fn stage18_424_neg_tuple_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let t = (1, 2); let y = t[0]; 0 }"#);
    assert_ne!(exit, 0, "tuple[0] read must be rejected");
}

#[test]
fn stage18_424_neg_str_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let y = s[0]; 0 }"#);
    assert_ne!(exit, 0, "&str[0] read must be rejected");
}

#[test]
fn stage18_424_neg_char_read_index() {
    let exit = compile_only(r#"fn main() -> i32 { let c: char = 'a'; let y = c[0]; 0 }"#);
    assert_ne!(exit, 0, "char[0] read must be rejected");
}

// ============================================================================
// Negative tests: indexing non-indexable types (assignment path, Stage 18.425)
// ============================================================================

#[test]
fn stage18_425_neg_int_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let n = 42; n[0] = 5; 0 }"#);
    assert_ne!(exit, 0, "int[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_float_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let f = 1.5; f[0] = 5; 0 }"#);
    assert_ne!(exit, 0, "f64[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_bool_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; b[0] = 5; 0 }"#);
    assert_ne!(exit, 0, "bool[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_struct_assign_index() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let mut f = Foo { x: 1 }; f[0] = 5; 0 }"#,
    );
    assert_ne!(exit, 0, "struct[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_tuple_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let mut t = (1, 2); t[0] = 5; 0 }"#);
    assert_ne!(exit, 0, "tuple[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_str_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; s[0] = 65; 0 }"#);
    assert_ne!(exit, 0, "&str[0] = val assignment must be rejected");
}

#[test]
fn stage18_425_neg_char_assign_index() {
    let exit = compile_only(r#"fn main() -> i32 { let c: char = 'a'; c[0] = 65; 0 }"#);
    assert_ne!(exit, 0, "char[0] = val assignment must be rejected");
}

// ============================================================================
// Negative tests: invalid index type on assignment path
// ============================================================================

#[test]
fn stage18_425_neg_array_str_index_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let mut a = [1, 2, 3]; a["x"] = 5; 0 }"#);
    assert_ne!(exit, 0, "array[\"x\"] = val must be rejected");
}

#[test]
fn stage18_425_neg_array_bool_index_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let mut a = [1, 2, 3]; a[true] = 5; 0 }"#);
    assert_ne!(exit, 0, "array[true] = val must be rejected");
}

#[test]
fn stage18_425_neg_array_float_index_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let mut a = [1, 2, 3]; a[1.5] = 5; 0 }"#);
    assert_ne!(exit, 0, "array[1.5] = val must be rejected");
}
