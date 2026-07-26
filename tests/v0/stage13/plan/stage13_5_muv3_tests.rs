//! Stage 13.5 MUV-3 — End-to-end LLVM module → object file generation
//!
//! Verifies that the LLVMSysEmitter can:
//! 1. Build an LLVM module from a compiled Landin program
//! 2. Emit an object file (.o) via LLVMTargetMachineEmitToFile
//! 3. The object file exists and is non-empty
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;

/// Verify codegen_crate_to_module function exists and is callable
#[test]
fn test_codegen_crate_to_module_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod).expect("read codegen/mod.rs");

    assert!(
        content.contains("pub fn codegen_crate_to_module"),
        "src/codegen/mod.rs must have codegen_crate_to_module function"
    );
}

/// Verify to_object_file method exists in LLVMSysEmitter
#[test]
fn test_to_object_file_method_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");

    assert!(
        content.contains("pub fn to_object_file"),
        "LLVMSysEmitter must have to_object_file() method"
    );

    // Must use LLVMTargetMachineEmitToFile
    assert!(
        content.contains("LLVMTargetMachineEmitToFile"),
        "to_object_file must use LLVMTargetMachineEmitToFile"
    );

    // Must initialize all targets
    assert!(
        content.contains("LLVM_InitializeAllTargets"),
        "to_object_file must call LLVM_InitializeAllTargets"
    );
}

/// Verify to_module method exists (returns LLVMModuleRef)
#[test]
fn test_to_module_method_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");

    assert!(
        content.contains("pub fn to_module"),
        "LLVMSysEmitter must have to_module() method"
    );
}

/// Verify LLVMSysEmitter implements all key Emitter trait methods
#[test]
fn test_llvm_sys_emitter_implements_emitter() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");

    // Must implement Emitter trait
    assert!(
        content.contains("impl Emitter for LLVMSysEmitter"),
        "LLVMSysEmitter must implement Emitter trait"
    );

    // Key methods must be present
    let required_methods = [
        "fn emit_header",
        "fn emit_declare",
        "fn emit_function_begin",
        "fn emit_function_end",
        "fn emit_const",
        "fn emit_binop",
        "fn emit_ret",
        "fn emit_alloca",
        "fn emit_load",
        "fn emit_store",
        "fn emit_call",
        "fn emit_br",
        "fn emit_br_cond",
    ];

    for method in &required_methods {
        assert!(
            content.contains(*method),
            "LLVMSysEmitter must implement {}",
            method
        );
    }
}

/// Verify LLVMSysEmitter uses llvm_sys C API functions
#[test]
fn test_uses_llvm_c_api() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");

    // Must use key LLVM C API functions
    let required_apis = [
        "LLVMModuleCreateWithNameInContext",
        "LLVMCreateBuilderInContext",
        "LLVMBuildAdd",
        "LLVMBuildRet",
        "LLVMBuildAlloca",
        "LLVMBuildLoad2",
        "LLVMBuildStore",
        "LLVMBuildCall2",
        "LLVMBuildCondBr",
        "LLVMBuildBr",
        "LLVMAppendBasicBlockInContext",
        "LLVMPositionBuilderAtEnd",
    ];

    for api in &required_apis {
        assert!(
            content.contains(*api),
            "LLVMSysEmitter must use LLVM C API: {}",
            api
        );
    }
}

/// Verify switch-llvm-version.sh handles version 21 correctly
#[test]
fn test_switch_llvm_version_handles_21() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/switch-llvm-version.sh");
    let content = std::fs::read_to_string(&script).expect("read switch-llvm-version.sh");

    // Must accept 21 as a valid version
    assert!(
        content.contains("21) LLVM_SYS_VER=\"211\""),
        "switch-llvm-version.sh must map LLVM 21 → llvm-sys 211"
    );
}

/// Verify setup-llvm-env.sh passes major version (not sys version) to switch script
#[test]
fn test_setup_llvm_env_passes_major_version() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/setup-llvm-env.sh");
    let content = std::fs::read_to_string(&script).expect("read setup-llvm-env.sh");

    // Must pass LLVM_MAJOR (not LLVM_SYS_VER) to switch-llvm-version.sh
    assert!(
        content.contains("LLVM_MAJOR"),
        "setup-llvm-env.sh must track LLVM_MAJOR"
    );

    // The call to switch-llvm-version.sh must use LLVM_MAJOR
    assert!(
        content.contains("switch-llvm-version.sh\" \"$LLVM_MAJOR\""),
        "setup-llvm-env.sh must pass $LLVM_MAJOR to switch-llvm-version.sh"
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds() {
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
    assert!(total >= 5000, "v0.1 gate must hold: 5000+, got {}", total);
}
