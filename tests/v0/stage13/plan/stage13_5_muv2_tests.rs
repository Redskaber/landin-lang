//! Stage 13.5 MUV-2 — LLVMSysEmitter + LLVM version switching verification
//!
//! Verifies:
//! 1. LLVMSysEmitter module exists and implements Emitter trait
//! 2. switch-llvm-version.sh script exists
//! 3. .cargo/config.toml has LLVM env vars
//! 4. Cargo.toml has llvm-sys dependency
//! 5. docs/llvm/ documentation exists
//! 6. codegen_crate_to_module function exists (behind feature gate)
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify LLVMSysEmitter source file exists
#[test]
fn test_llvm_sys_emitter_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emitter = manifest.join("src/codegen/llvm_sys_emitter.rs");
    assert!(
        emitter.exists(),
        "src/codegen/llvm_sys_emitter.rs must exist"
    );

    let content = std::fs::read_to_string(&emitter).expect("read llvm_sys_emitter.rs");

    // Must implement Emitter trait
    assert!(
        content.contains("impl Emitter for LLVMSysEmitter"),
        "llvm_sys_emitter.rs must implement Emitter trait"
    );

    // Must have LLVM module + builder + context
    assert!(
        content.contains("LLVMModuleRef")
            && content.contains("LLVMBuilderRef")
            && content.contains("LLVMContextRef"),
        "LLVMSysEmitter must use LLVMModuleRef, LLVMBuilderRef, LLVMContextRef"
    );

    // Must have to_object_file method
    assert!(
        content.contains("fn to_object_file"),
        "LLVMSysEmitter must have to_object_file() method"
    );

    // Must have to_module method
    assert!(
        content.contains("fn to_module"),
        "LLVMSysEmitter must have to_module() method"
    );

    // Must reference Stage 13.5
    assert!(
        content.contains("Stage 13.5") || content.contains("MUV-2"),
        "llvm_sys_emitter.rs must reference Stage 13.5 / MUV-2"
    );
}

/// Verify switch-llvm-version.sh script exists
#[test]
fn test_switch_llvm_version_script_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/switch-llvm-version.sh");
    assert!(script.exists(), "scripts/switch-llvm-version.sh must exist");

    let content = std::fs::read_to_string(&script).expect("read switch-llvm-version.sh");

    // Must auto-detect LLVM version
    assert!(
        content.contains("llvm-config") && content.contains("--version"),
        "switch-llvm-version.sh must auto-detect LLVM version via llvm-config"
    );

    // Must support version mapping (181, 191, 201, 211, 221)
    for ver in ["181", "191", "201", "211", "221"] {
        assert!(
            content.contains(ver),
            "switch-llvm-version.sh must support llvm-sys version {}",
            ver
        );
    }

    // Must update .cargo/config.toml
    assert!(
        content.contains("config.toml") || content.contains("CARGO_CONFIG"),
        "switch-llvm-version.sh must update .cargo/config.toml"
    );

    // Must update Cargo.toml
    assert!(
        content.contains("Cargo.toml") || content.contains("CARGO_TOML"),
        "switch-llvm-version.sh must update Cargo.toml"
    );
}

/// Verify .cargo/config.toml has LLVM env vars
#[test]
fn test_cargo_config_has_llvm_env() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = manifest.join(".cargo/config.toml");
    assert!(config.exists(), ".cargo/config.toml must exist");

    let content = std::fs::read_to_string(&config).expect("read .cargo/config.toml");

    // Must have LLVM_SYS_XXX_PREFIX
    assert!(
        content.contains("LLVM_SYS_") && content.contains("_PREFIX"),
        ".cargo/config.toml must have LLVM_SYS_XXX_PREFIX env var"
    );

    // Must have LLVM_LINK_SHARED
    assert!(
        content.contains("LLVM_LINK_SHARED"),
        ".cargo/config.toml must have LLVM_LINK_SHARED"
    );
}

/// Verify Cargo.toml has llvm-sys dependency
#[test]
fn test_cargo_toml_has_llvm_sys() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    // Must have llvm-sys dependency
    assert!(
        content.contains("llvm-sys"),
        "Cargo.toml must have llvm-sys dependency"
    );

    // Must have llvm-backend feature
    assert!(
        content.contains("llvm-backend"),
        "Cargo.toml must have llvm-backend feature"
    );

    // Must have prefer-dynamic
    assert!(
        content.contains("prefer-dynamic"),
        "Cargo.toml llvm-sys must have prefer-dynamic feature"
    );
}

/// Verify docs/llvm/ documentation exists
#[test]
fn test_llvm_docs_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let llvm_docs = manifest.join("docs/llvm");

    // README.md
    assert!(
        llvm_docs.join("README.md").exists(),
        "docs/llvm/README.md must exist"
    );

    // Version switching doc
    assert!(
        llvm_docs.join("version-switching.md").exists(),
        "docs/llvm/version-switching.md must exist"
    );

    // LLVM 19 setup doc
    assert!(
        llvm_docs.join("llvm-19-build-server-setup.md").exists(),
        "docs/llvm/llvm-19-build-server-setup.md must exist"
    );

    // LLVM 21 setup doc
    assert!(
        llvm_docs.join("llvm-21-user-environment-setup.md").exists(),
        "docs/llvm/llvm-21-user-environment-setup.md must exist"
    );
}

/// Verify codegen module declares llvm_sys_emitter (behind feature gate)
#[test]
fn test_codegen_mod_declares_emitter() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod).expect("read codegen/mod.rs");

    // Must declare llvm_sys_emitter module (behind feature gate)
    assert!(
        content.contains("llvm_sys_emitter"),
        "src/codegen/mod.rs must declare llvm_sys_emitter module"
    );

    // Must have cfg(feature = "llvm-backend")
    assert!(
        content.contains("llvm-backend"),
        "src/codegen/mod.rs must gate llvm_sys_emitter behind llvm-backend feature"
    );

    // Must have codegen_crate_to_module function
    assert!(
        content.contains("codegen_crate_to_module"),
        "src/codegen/mod.rs must have codegen_crate_to_module function"
    );
}

/// Verify setup-llvm-env.sh script exists
#[test]
fn test_setup_llvm_env_script_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/setup-llvm-env.sh");
    assert!(script.exists(), "scripts/setup-llvm-env.sh must exist");

    let content = std::fs::read_to_string(&script).expect("read setup-llvm-env.sh");

    // Must auto-detect system LLVM vs build server
    assert!(
        content.contains("llvm-config") && content.contains("command -v"),
        "setup-llvm-env.sh must auto-detect system LLVM via command -v llvm-config"
    );

    // Must support multiple LLVM versions
    assert!(
        content.contains("191") || content.contains("211"),
        "setup-llvm-env.sh must support multiple LLVM versions"
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
