//! Stage 5.63: emit_dyn_trait_fat_ptr_text tests
//!
//! Tests `emit_dyn_trait_fat_ptr_text()` — converts DynTraitFatPtr to
//! LLVM IR text. Bridges MIR representation with codegen output.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::dyn_trait_emit::emit_dyn_trait_fat_ptr_text;
use landin_compiler::mir::DynTraitFatPtr;

/// Basic conversion produces correct IR.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_basic() {
    let fp = DynTraitFatPtr::new("Foo", "S");
    let ir = emit_dyn_trait_fat_ptr_text(&fp);
    assert!(ir.starts_with("@.dynptr.Foo.S = private unnamed_addr constant"));
    assert!(ir.contains("ptr @.data.S"));
    assert!(ir.contains("ptr @.vtable.Foo.S"));
}

/// Full IR line verification.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_full_line() {
    let fp = DynTraitFatPtr::new("Display", "Vec");
    let ir = emit_dyn_trait_fat_ptr_text(&fp);
    assert_eq!(
        ir,
        "@.dynptr.Display.Vec = private unnamed_addr constant \
         { ptr, ptr } { ptr @.data.Vec, ptr @.vtable.Display.Vec }"
    );
}

/// Clone trait fat ptr.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_clone() {
    let fp = DynTraitFatPtr::new("Clone", "MyType");
    let ir = emit_dyn_trait_fat_ptr_text(&fp);
    assert!(ir.contains("@.dynptr.Clone.MyType"));
    assert!(ir.contains("{ ptr, ptr }"));
}

/// Drop trait fat ptr.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_drop() {
    let fp = DynTraitFatPtr::new("Drop", "S");
    let ir = emit_dyn_trait_fat_ptr_text(&fp);
    assert!(ir.contains("@.dynptr.Drop.S"));
}

/// Matches codegen emit_dynptr_global_text output.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_match_codegen() {
    use landin_compiler::codegen::emit_dynptr_global_text;
    let fp = DynTraitFatPtr::new("Foo", "S");
    let mir_ir = emit_dyn_trait_fat_ptr_text(&fp);
    let codegen_ir = emit_dynptr_global_text(".dynptr.Foo.S", ".data.S", ".vtable.Foo.S");
    assert_eq!(mir_ir, codegen_ir);
}

/// No side effects — pure function.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_no_side_effects() {
    let fp = DynTraitFatPtr::new("Foo", "S");
    let ir1 = emit_dyn_trait_fat_ptr_text(&fp);
    let ir2 = emit_dyn_trait_fat_ptr_text(&fp);
    assert_eq!(ir1, ir2);
}

/// Multiple fat ptrs → independent IR lines.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_multiple() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    for fp in &fps {
        let ir = emit_dyn_trait_fat_ptr_text(fp);
        assert!(ir.starts_with("@.dynptr."));
        assert!(ir.contains("ptr @.data.S"));
    }
}

/// Real scenario: S impls Clone + Drop + Display.
#[test]
fn test_emit_dyn_trait_fat_ptr_text_real_scenario() {
    let fps = [
        DynTraitFatPtr::new("Clone", "S"),
        DynTraitFatPtr::new("Drop", "S"),
        DynTraitFatPtr::new("Display", "S"),
    ];
    let irs: Vec<String> = fps.iter().map(emit_dyn_trait_fat_ptr_text).collect();
    assert_eq!(irs.len(), 3);
    for ir in &irs {
        assert!(ir.starts_with("@.dynptr."));
        assert!(ir.contains("private unnamed_addr constant"));
        assert!(ir.contains("{ ptr, ptr }"));
    }
    assert!(irs[0].contains("@.dynptr.Clone.S"));
    assert!(irs[1].contains("@.dynptr.Drop.S"));
    assert!(irs[2].contains("@.dynptr.Display.S"));
}
