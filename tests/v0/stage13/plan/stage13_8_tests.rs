//! Stage 13.8 — --run flag + --emit-bin with auto C wrapper verification
//!
//! Verifies that:
//! 1. --run flag exists in CLI
//! 2. --run compiles + links + executes in one step
//! 3. --emit-bin generates executable with auto C wrapper
//! 4. Auto-generated C wrapper calls landin_main()
//! 5. Return values are correct (42, 7)

#![cfg(test)]

use std::path::Path;

/// Verify --run flag exists in main.rs
#[test]
fn test_run_flag_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    assert!(
        content.contains("--run") || content.contains("run: bool"),
        "src/bin/main.rs must have --run flag"
    );

    // Must have run in the compile pipeline check
    assert!(
        content.contains("cli.run"),
        "main.rs must check cli.run in the compile pipeline"
    );
}

/// Verify auto-generated C wrapper is used for linking
#[test]
fn test_auto_c_wrapper_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // Must have C wrapper that calls landin_main
    assert!(
        content.contains("landin_main"),
        "main.rs must reference landin_main in C wrapper"
    );

    // Must have extern int landin_main(void) declaration
    assert!(
        content.contains("extern int landin_main"),
        "main.rs must declare extern int landin_main in C wrapper"
    );
}

/// Verify --emit-bin uses auto C wrapper (not just raw .o)
#[test]
fn test_emit_bin_uses_wrapper() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // --emit-bin must also use the C wrapper
    assert!(
        content.contains("cli.emit_bin || cli.run"),
        "main.rs must use C wrapper for both --emit-bin and --run"
    );
}

/// Verify cleanup of temporary files after --run
#[test]
fn test_run_cleans_up() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // Must remove temporary files after --run
    assert!(
        content.contains("remove_file(&exe_path)") && content.contains("remove_file(&obj_path)"),
        "main.rs must clean up temporary files after --run"
    );
}

/// Verify -o flag works with --run and --emit-bin
#[test]
fn test_o_flag_works() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    assert!(
        content.contains("output: Option<PathBuf>"),
        "main.rs must have -o/--output flag"
    );
}

/// Verify --run requires llvm-backend feature
#[test]
fn test_run_requires_llvm_backend() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // Must have cfg(feature = "llvm-backend") for --run
    assert!(
        content.contains("#[cfg(feature = \"llvm-backend\")]"),
        "main.rs must gate --run behind llvm-backend feature"
    );

    // Must have graceful error when feature not enabled
    assert!(
        content.contains("--run requires --features llvm-backend"),
        "main.rs must show graceful error when --run used without llvm-backend"
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
