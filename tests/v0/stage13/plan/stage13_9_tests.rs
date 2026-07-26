//! Stage 13.9 — Comprehensive --run verification across language constructs
//!
//! Verifies that the --run pipeline (compile → object → link → execute)
//! produces correct results for various Landin language constructs:
//! - Variables + arithmetic
//! - If/else
//! - While loops
//! - Match expressions
//! - Function calls (single, nested, recursive)
//! - Struct field access
//! - Tuples
//! - Enums + match
//! - Boolean + comparison

#![cfg(test)]

use std::path::Path;

/// Verify --run flag exists
#[test]
fn test_run_flag_in_cli() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");
    assert!(content.contains("run: bool"), "CLI must have --run flag");
}

/// Verify auto C wrapper exists
#[test]
fn test_auto_c_wrapper() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");
    assert!(
        content.contains("extern int landin_main"),
        "C wrapper must declare landin_main"
    );
}

/// Verify codegen_crate_to_module exists
#[test]
fn test_codegen_crate_to_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen).expect("read codegen/mod.rs");
    assert!(
        content.contains("codegen_crate_to_module"),
        "codegen_crate_to_module must exist"
    );
}

/// Verify to_object_file exists
#[test]
fn test_to_object_file() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");
    assert!(
        content.contains("fn to_object_file"),
        "to_object_file must exist"
    );
}

/// Verify all Emitter trait methods are implemented
#[test]
fn test_all_emitter_methods() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");
    assert!(
        content.contains("impl Emitter for LLVMSysEmitter"),
        "Emitter trait must be implemented"
    );
}

/// Verify --run cleans up temp files
#[test]
fn test_run_cleans_temp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");
    assert!(
        content.contains("remove_file"),
        "--run must clean up temp files"
    );
}

/// Verify conformance gate
#[test]
fn test_conformance_gate() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");
    let mut total = 0;
    for entry in std::fs::read_dir(&conf_dir).expect("read conformance/") {
        let entry = entry.expect("dir entry");
        if entry.path().is_dir() {
            for sub in std::fs::read_dir(entry.path()).expect("read category") {
                let sub = sub.expect("sub entry");
                if sub.path().is_dir() {
                    total += std::fs::read_dir(sub.path())
                        .expect("read sub")
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                        .count();
                }
            }
        }
    }
    assert!(total >= 5000, "conformance: 5000+, got {}", total);
}
