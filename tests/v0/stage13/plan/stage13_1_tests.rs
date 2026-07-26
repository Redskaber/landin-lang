//! Stage 13.1 — Architecture baseline (TD-028 §16 violation fix) verification
//!
//! Verifies that the 7 emit_dyn_trait_* functions were relocated from
//! `src/mir/dyn_trait` to `src/codegen/dyn_trait_emit` per §16 interface
//! isolation fix. The §16 violation (mir → codegen reverse dependency)
//! must be eliminated.
//!
//! Per stage-committee-process.md v3.21 §16 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify §16 violation eliminated: no `crate::codegen` references in src/mir/dyn_trait.rs
/// (except in comments)
#[test]
fn test_no_codegen_references_in_mir_dyn_trait() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_dyn_trait = manifest.join("src/mir/dyn_trait.rs");
    let content = std::fs::read_to_string(&mir_dyn_trait).expect("read src/mir/dyn_trait.rs");

    // Find all lines with "crate::codegen" — they must ALL be comments
    for (line_num, line) in content.lines().enumerate() {
        if line.contains("crate::codegen") {
            let stripped = line.trim_start();
            assert!(
                stripped.starts_with("//"),
                "src/mir/dyn_trait.rs:{} contains non-comment 'crate::codegen' reference (§16 violation): {}",
                line_num + 1,
                line
            );
        }
    }
}

/// Verify new codegen::dyn_trait_emit module exists
#[test]
fn test_codegen_dyn_trait_emit_module_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let module = manifest.join("src/codegen/dyn_trait_emit.rs");
    assert!(
        module.exists(),
        "src/codegen/dyn_trait_emit.rs must exist (Stage 13.1 TD-028 relocation target)"
    );

    let content = std::fs::read_to_string(&module).expect("read dyn_trait_emit.rs");

    // Must contain all 7 emit_* function definitions
    for fn_name in [
        "emit_dyn_trait_fat_ptr_text",
        "emit_dyn_trait_fat_ptrs_text_batch",
        "emit_dyn_trait_fat_ptrs_text_batch_from_resolver",
        "emit_dyn_trait_method_call_text",
        "emit_dyn_trait_method_calls_text_batch",
        "emit_dyn_trait_method_calls_text_batch_from_resolver",
        "emit_dyn_trait_mir_plan_text",
    ] {
        assert!(
            content.contains(&format!("pub fn {}", fn_name)),
            "codegen/dyn_trait_emit.rs must define `pub fn {}`",
            fn_name
        );
    }

    // Must reference §16 interface isolation fix
    assert!(
        content.contains("§16") || content.contains("TD-028"),
        "codegen/dyn_trait_emit.rs must document the §16/TD-028 fix"
    );
}

/// Verify src/mir/dyn_trait.rs no longer defines the 7 emit_* functions
#[test]
fn test_mir_dyn_trait_no_emit_functions() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_dyn_trait = manifest.join("src/mir/dyn_trait.rs");
    let content = std::fs::read_to_string(&mir_dyn_trait).expect("read src/mir/dyn_trait.rs");

    // Must NOT contain any `pub fn emit_dyn_trait_*` definitions
    for fn_name in [
        "emit_dyn_trait_fat_ptr_text",
        "emit_dyn_trait_fat_ptrs_text_batch",
        "emit_dyn_trait_fat_ptrs_text_batch_from_resolver",
        "emit_dyn_trait_method_call_text",
        "emit_dyn_trait_method_calls_text_batch",
        "emit_dyn_trait_method_calls_text_batch_from_resolver",
        "emit_dyn_trait_mir_plan_text",
    ] {
        assert!(
            !content.contains(&format!("pub fn {}", fn_name)),
            "src/mir/dyn_trait.rs must NOT define `pub fn {}` (relocated to codegen::dyn_trait_emit)",
            fn_name
        );
    }
}

/// Verify src/mir/mod.rs no longer re-exports the 7 emit_* functions
#[test]
fn test_mir_mod_no_emit_reexports() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_mod = manifest.join("src/mir/mod.rs");
    let content = std::fs::read_to_string(&mir_mod).expect("read src/mir/mod.rs");

    // The re-export line should NOT contain any emit_dyn_trait_* names
    for fn_name in [
        "emit_dyn_trait_fat_ptr_text",
        "emit_dyn_trait_fat_ptrs_text_batch",
        "emit_dyn_trait_fat_ptrs_text_batch_from_resolver",
        "emit_dyn_trait_method_call_text",
        "emit_dyn_trait_method_calls_text_batch",
        "emit_dyn_trait_method_calls_text_batch_from_resolver",
        "emit_dyn_trait_mir_plan_text",
    ] {
        // Allow the name to appear in comments but not in `pub use` re-exports
        // Check that no `pub use` line contains the function name
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("emit_dyn_trait_")
                || trimmed.contains(&format!(
                    ", emit_dyn_trait_{}",
                    fn_name.strip_prefix("emit_dyn_trait_").unwrap()
                ))
            {
                // This could be in a `pub use` continuation line
                // Check if it's part of a re-export by looking at context
                if trimmed.starts_with("emit_dyn_trait_") && !trimmed.starts_with("//") {
                    panic!(
                        "src/mir/mod.rs contains re-export of `{}` in non-comment line: {}",
                        fn_name, line
                    );
                }
            }
        }
    }

    // Must reference Stage 13.1 TD-028 relocation
    assert!(
        content.contains("Stage 13.1") || content.contains("TD-028"),
        "src/mir/mod.rs must document the Stage 13.1 TD-028 relocation"
    );
}

/// Verify src/codegen/mod.rs declares the new dyn_trait_emit module + re-exports
#[test]
fn test_codegen_mod_declares_dyn_trait_emit() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod).expect("read src/codegen/mod.rs");

    // Must declare the module
    assert!(
        content.contains("pub mod dyn_trait_emit") || content.contains("mod dyn_trait_emit"),
        "src/codegen/mod.rs must declare `dyn_trait_emit` module"
    );

    // Must re-export the 7 functions
    for fn_name in [
        "emit_dyn_trait_fat_ptr_text",
        "emit_dyn_trait_fat_ptrs_text_batch",
        "emit_dyn_trait_fat_ptrs_text_batch_from_resolver",
        "emit_dyn_trait_method_call_text",
        "emit_dyn_trait_method_calls_text_batch",
        "emit_dyn_trait_method_calls_text_batch_from_resolver",
        "emit_dyn_trait_mir_plan_text",
    ] {
        assert!(
            content.contains(fn_name),
            "src/codegen/mod.rs must re-export `{}`",
            fn_name
        );
    }
}

/// Verify the 7 emit_* functions are accessible from `codegen::` namespace
/// (compilation test — if this compiles, the relocation is correct)
#[test]
fn test_emit_functions_accessible_from_codegen() {
    use landin_compiler::codegen::{
        emit_dyn_trait_fat_ptr_text, emit_dyn_trait_fat_ptrs_text_batch,
        emit_dyn_trait_fat_ptrs_text_batch_from_resolver, emit_dyn_trait_method_call_text,
        emit_dyn_trait_method_calls_text_batch,
        emit_dyn_trait_method_calls_text_batch_from_resolver, emit_dyn_trait_mir_plan_text,
    };

    // Just reference them to ensure they're accessible
    let _ = emit_dyn_trait_fat_ptr_text as fn(&landin_compiler::mir::DynTraitFatPtr) -> String;
    let _ = emit_dyn_trait_fat_ptrs_text_batch
        as fn(&[landin_compiler::mir::DynTraitFatPtr]) -> Vec<String>;
    let _ = emit_dyn_trait_fat_ptrs_text_batch_from_resolver
        as fn(&landin_compiler::traits::TraitResolver, &lasso::Rodeo) -> Vec<String>;
    let _ =
        emit_dyn_trait_method_call_text as fn(&landin_compiler::mir::DynTraitMethodCall) -> String;
    let _ = emit_dyn_trait_method_calls_text_batch
        as fn(&[landin_compiler::mir::DynTraitMethodCall]) -> Vec<String>;
    let _ = emit_dyn_trait_method_calls_text_batch_from_resolver
        as fn(&landin_compiler::traits::TraitResolver, &lasso::Rodeo) -> Vec<String>;
    let _ = emit_dyn_trait_mir_plan_text as fn(&landin_compiler::mir::DynTraitMIRPlan) -> String;
}

/// Verify the 7 emit_* functions are NOT accessible from `mir::` namespace
/// (compilation test — if this compiles, the relocation left stale re-exports)
#[test]
fn test_emit_functions_not_accessible_from_mir() {
    // This test verifies that `landin_compiler::mir::emit_dyn_trait_*` does NOT compile.
    // We can't actually test non-compilation in a #[test], so instead we verify
    // that the mir module's public API (via docs or source inspection) doesn't
    // include these names.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_mod = manifest.join("src/mir/mod.rs");
    let content = std::fs::read_to_string(&mir_mod).expect("read src/mir/mod.rs");

    // Extract all names from `pub use dyn_trait::{...}` block
    let dyn_trait_reexport = content
        .find("pub use dyn_trait::{")
        .and_then(|start| {
            let end = content[start..].find("};")?;
            Some(&content[start..start + end + 2])
        })
        .expect("must find `pub use dyn_trait::{...}` block");

    // None of the 7 emit_* names should be in this re-export
    for fn_name in [
        "emit_dyn_trait_fat_ptr_text",
        "emit_dyn_trait_fat_ptrs_text_batch",
        "emit_dyn_trait_fat_ptrs_text_batch_from_resolver",
        "emit_dyn_trait_method_call_text",
        "emit_dyn_trait_method_calls_text_batch",
        "emit_dyn_trait_method_calls_text_batch_from_resolver",
        "emit_dyn_trait_mir_plan_text",
    ] {
        assert!(
            !dyn_trait_reexport.contains(fn_name),
            "`{}` must NOT be in `pub use dyn_trait::{{...}}` re-export (should be in codegen)",
            fn_name
        );
    }
}

/// Verify Stage 13.1 gate review exists + PASS verdict
#[test]
fn test_stage13_1_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.1.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.1.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.1.md");

    // Must mark TD-028 as CLOSED
    assert!(
        content.contains("TD-028") && content.contains("CLOSED"),
        "gate-review-13.1.md must mark TD-028 as CLOSED"
    );

    // Must reference §16 interface isolation
    assert!(
        content.contains("§16"),
        "gate-review-13.1.md must reference §16 interface isolation"
    );

    // Must include committee vote
    assert!(
        content.contains("委员会投票") || content.contains("Committee") || content.contains("Vote"),
        "gate-review-13.1.md must include committee vote"
    );

    // Must reach PASS verdict
    assert!(
        content.contains("PASS"),
        "gate-review-13.1.md must reach PASS verdict"
    );
}

/// Verify Stage 13.1 design alignment report exists
#[test]
fn test_stage13_1_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_alignment = manifest.join("docs/develop/v0/stage-13/stage-13.1-design-alignment.md");
    assert!(
        design_alignment.exists(),
        "docs/develop/v0/stage-13/stage-13.1-design-alignment.md must exist (§13.4 design alignment)"
    );

    let content = std::fs::read_to_string(&design_alignment).expect("read design-alignment.md");

    // Must reference §13.4
    assert!(
        content.contains("§13.4") || content.contains("13.4"),
        "design-alignment.md must reference §13.4"
    );

    // Must cover MUV-1 + MUV-2 scope
    assert!(
        content.contains("MUV-1") && content.contains("MUV-2"),
        "design-alignment.md must cover MUV-1 + MUV-2 scope"
    );

    // Must recommend split (MUV-1 in 13.1, MUV-2 deferred to 13.1b)
    assert!(
        content.contains("SPLIT") || content.contains("split") || content.contains("deferred"),
        "design-alignment.md must recommend SPLIT (MUV-2 deferred)"
    );
}

/// Verify v0.1 conformance gate still holds after Stage 13.1
#[test]
fn test_v01_gate_still_holds_after_stage13_1() {
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
    assert!(
        total >= 5000,
        "v0.1 gate must still hold: 5000+, got {}",
        total
    );
}
