//! Stage 133 (v0.14 — TD-MATCHES-MACRO): matches! tests.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// =====================================================================
// POSITIVE TESTS
// =====================================================================

#[test]
fn stage133_matches_true() {
    let code = r#"
fn main() {
    let x = 5;
    let is_five = matches!(x, 5);
    println!("{}", is_five);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "matches! should work");
    assert_eq!(stdout.trim(), "true");
}

#[test]
fn stage133_matches_false() {
    let code = r#"
fn main() {
    let x = 10;
    let is_five = matches!(x, 5);
    println!("{}", is_five);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "matches! should work for non-matching");
    assert_eq!(stdout.trim(), "false");
}

#[test]
fn stage133_matches_with_variable() {
    let code = r#"
fn main() {
    let x = 42;
    let is_42 = matches!(x, 42);
    println!("{}", is_42);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "true");
}

// =====================================================================
// REGRESSION TESTS
// =====================================================================

#[test]
fn stage133_regression_env_macro() {
    let code = r#"
fn main() {
    let v = env!("HOME");
    println!("{}", v);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "env! regression");
    assert!(!stdout.trim().is_empty());
}

#[test]
fn stage133_regression_compile_error() {
    let code = r#"
fn main() {
    compile_error!("test error");
}
"#;
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    let counter = std::sync::atomic::AtomicU64::new(0);
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "stage133_{:?}_{}_{}_{}",
        std::thread::current().id(),
        std::process::id(),
        nanos,
        n
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let lin = dir.join("input.lin");
    std::fs::write(&lin, code).expect("write .lin");
    let output = std::process::Command::new(&bin)
        .arg("--run")
        .arg(&lin)
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!output.status.success(), "compile_error! should fail");
}
