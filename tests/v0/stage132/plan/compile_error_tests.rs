//! Stage 132 (v0.14 — TD-COMPILE-ERROR-MACRO): compile_error! tests.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

fn compile(code: &str) -> (bool, String) {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    let counter = std::sync::atomic::AtomicU64::new(0);
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let thread_id = std::thread::current().id();
    let dir = std::env::temp_dir().join(format!(
        "stage132_{:?}_{}_{}_{}",
        thread_id,
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
        .expect("run landin-stage0");
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stderr)
}

// =====================================================================
// POSITIVE TESTS — compile_error! produces error
// =====================================================================

#[test]
fn stage132_compile_error_basic() {
    let code = r#"
fn main() {
    compile_error!("custom error message");
}
"#;
    let (ok, stderr) = compile(code);
    assert!(!ok, "Should fail: compile_error! produces error");
    assert!(
        stderr.contains("custom error message"),
        "stderr should contain the error message"
    );
}

#[test]
fn stage132_compile_error_empty_string() {
    let code = r#"
fn main() {
    compile_error!("");
}
"#;
    let (ok, _stderr) = compile(code);
    assert!(
        !ok,
        "Should fail: compile_error! with empty string still errors"
    );
}

// =====================================================================
// REGRESSION TESTS
// =====================================================================

#[test]
fn stage132_regression_env_macro() {
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
fn stage132_regression_stringify_macro() {
    let code = r#"
fn main() {
    let s = stringify!(hello);
    println!("{}", s);
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0, "stringify! regression");
    assert_eq!(stdout.trim(), "hello");
}
