//! Stage 18.241 — str method resolution tests.
//!
//! Verifies that unknown methods on str/&str are correctly reported,
//! while known str methods (len, is_empty, as_bytes) continue to work.

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
        std::env::temp_dir().join(format!("landin_str_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER2: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER2.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_strrun_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn stage18_241_str_len_works() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let s = "hello"; println!("{}", s.len()); 0 }"#);
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_241_str_is_empty_works() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 { println!("{}", "".is_empty()); println!("{}", "x".is_empty()); 0 }"#,
    );
    assert_eq!(stdout, "true\nfalse\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_241_str_unknown_method_fails() {
    let code = r#"fn main() -> i32 { let s = "hello"; s.nonexistent(); 0 }"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "unknown str method should fail");
}

#[test]
fn stage18_241_str_another_unknown_fails() {
    let code = r#"fn main() -> i32 { let s = "hello"; s.foobar(42); 0 }"#;
    let exit = compile_only(code);
    assert_ne!(exit, 0, "unknown str method should fail");
}
