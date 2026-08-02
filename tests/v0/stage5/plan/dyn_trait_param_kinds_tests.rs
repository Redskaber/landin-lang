//! Stage 5.84: TD — dyn Trait param type refinement tests
//!
//! Tests the new `param_kinds` field on `StdlibTraitMethod` and
//! `DynTraitMethodCall`. Verifies that `codegen_dyn_trait_call` now emits
//! precise parameter types instead of the I32 placeholder.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{codegen_dyn_trait_call_direct, TextEmitter};
use landin_compiler::mir::dyn_trait::DynTraitMethodCall;
use landin_compiler::mir::place::{LocalId, Operand, Place};
use landin_compiler::mir::DynTraitFatPtr;
use landin_compiler::session::Span;
use landin_compiler::stdlib::StdlibTypeKind;
use landin_compiler::stdlib_trait_methods;
use lasso::Rodeo;

// ============================================================
// StdlibTraitMethod.param_kinds field tests
// ============================================================

/// StdlibTraitMethod has param_kinds field.
#[test]
fn test_stdlib_trait_method_has_param_kinds() {
    let methods = stdlib_trait_methods("Drop");
    assert!(methods.is_some());
    let methods = methods.unwrap();
    let drop_method = methods
        .iter()
        .find(|m| m.name == "drop")
        .expect("drop method");
    // Drop::drop has no params → empty param_kinds
    assert_eq!(drop_method.param_kinds.len(), 0);
}

/// Clone::clone has no params → empty param_kinds.
#[test]
fn test_clone_clone_param_kinds_empty() {
    let methods = stdlib_trait_methods("Clone");
    let clone_method = methods
        .unwrap()
        .iter()
        .find(|m| m.name == "clone")
        .expect("clone method");
    assert_eq!(clone_method.param_kinds.len(), 0);
}

/// Clone::clone_from has 1 param → param_kinds has 1 entry.
#[test]
fn test_clone_clone_from_param_kinds_one() {
    let methods = stdlib_trait_methods("Clone");
    let clone_from = methods
        .unwrap()
        .iter()
        .find(|m| m.name == "clone_from")
        .expect("clone_from method");
    assert_eq!(clone_from.param_kinds.len(), 1);
}

/// param_count matches param_kinds.len() for all stdlib methods.
#[test]
fn test_param_count_matches_param_kinds_length() {
    let all_traits = [
        "Drop",
        "Clone",
        "Default",
        "Display",
        "Debug",
        "PartialEq",
        "PartialOrd",
        "Ord",
        "Hash",
    ];
    for trait_name in &all_traits {
        if let Some(methods) = stdlib_trait_methods(trait_name) {
            for m in methods {
                assert_eq!(
                    m.param_count as usize,
                    m.param_kinds.len(),
                    "trait {} method {}: param_count {} != param_kinds.len() {}",
                    trait_name,
                    m.name,
                    m.param_count,
                    m.param_kinds.len()
                );
            }
        }
    }
}

// ============================================================
// DynTraitMethodCall.param_kinds field tests
// ============================================================

/// DynTraitMethodCall::new includes param_kinds field.
#[test]
fn test_dyn_trait_method_call_new_with_param_kinds() {
    let call = DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        2,
        StdlibTypeKind::Unit,
        vec![StdlibTypeKind::I32, StdlibTypeKind::Bool],
    );
    assert_eq!(call.param_kinds.len(), 2);
    assert_eq!(call.param_kinds[0], StdlibTypeKind::I32);
    assert_eq!(call.param_kinds[1], StdlibTypeKind::Bool);
}

/// from_fat_ptr includes param_kinds field.
#[test]
fn test_dyn_trait_method_call_from_fat_ptr_with_param_kinds() {
    let fp = DynTraitFatPtr::new("Foo", "S");
    let call = DynTraitMethodCall::from_fat_ptr(
        &fp,
        "bar",
        0,
        1,
        StdlibTypeKind::Unit,
        vec![StdlibTypeKind::F64],
    );
    assert_eq!(call.param_kinds.len(), 1);
    assert_eq!(call.param_kinds[0], StdlibTypeKind::F64);
}

/// param_kinds preserves the value passed in.
#[test]
fn test_param_kinds_preserved() {
    let call = DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        3,
        StdlibTypeKind::Unit,
        vec![StdlibTypeKind::I8, StdlibTypeKind::I16, StdlibTypeKind::I32],
    );
    assert_eq!(
        call.param_kinds,
        vec![StdlibTypeKind::I8, StdlibTypeKind::I16, StdlibTypeKind::I32]
    );
}

/// Empty param_kinds for zero-param methods.
#[test]
fn test_param_kinds_empty_for_zero_param() {
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    assert!(call.param_kinds.is_empty());
}

// ============================================================
// codegen_dyn_trait_call uses param_kinds tests
// ============================================================

/// Helper: build a DynTraitMethodCall with given param_kinds.
fn make_call_info_with_param_kinds(param_kinds: Vec<StdlibTypeKind>) -> DynTraitMethodCall {
    DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        param_kinds.len() as u32,
        StdlibTypeKind::Unit,
        param_kinds,
    )
}

/// Method with I32 param → IR contains i32 arg type.
#[test]
fn test_codegen_dyn_trait_call_i32_param() {
    let call_info = make_call_info_with_param_kinds(vec![StdlibTypeKind::I32]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    // self + 1 param
    let args = vec![
        Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
        Operand::Copy(Place::local(LocalId(2), Span::DUMMY)),
    ];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    // The call should contain "i32" for the param (not just self)
    assert!(
        output.contains("i32"),
        "expected i32 in IR, got: {}",
        output
    );
}

/// Method with F64 param → IR contains double arg type.
#[test]
fn test_codegen_dyn_trait_call_f64_param() {
    let call_info = make_call_info_with_param_kinds(vec![StdlibTypeKind::F64]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![
        Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
        Operand::Copy(Place::local(LocalId(2), Span::DUMMY)),
    ];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("double"),
        "expected double in IR, got: {}",
        output
    );
}

/// Method with Bool param → IR contains i8 arg type.
#[test]
fn test_codegen_dyn_trait_call_bool_param() {
    let call_info = make_call_info_with_param_kinds(vec![StdlibTypeKind::Bool]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![
        Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
        Operand::Copy(Place::local(LocalId(2), Span::DUMMY)),
    ];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(output.contains("i8"), "expected i8 in IR, got: {}", output);
}

/// Method with no params → IR only has self (OpaquePtr/i32*).
#[test]
fn test_codegen_dyn_trait_call_no_params() {
    let call_info = make_call_info_with_param_kinds(vec![]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    // Should have self as i32* (OpaquePtr) and void return (Unit)
    assert!(
        output.contains("call void"),
        "expected 'call void' for Unit return, got: {}",
        output
    );
}

/// Method with multiple params → IR contains all param types.
#[test]
fn test_codegen_dyn_trait_call_multiple_params() {
    let call_info = make_call_info_with_param_kinds(vec![StdlibTypeKind::I32, StdlibTypeKind::F64]);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![
        Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
        Operand::Copy(Place::local(LocalId(2), Span::DUMMY)),
        Operand::Copy(Place::local(LocalId(3), Span::DUMMY)),
    ];

    codegen_dyn_trait_call_direct(
        &mut emitter,
        &call_info,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("i32"),
        "expected i32 for first param, got: {}",
        output
    );
    assert!(
        output.contains("double"),
        "expected double for second param, got: {}",
        output
    );
}

// ============================================================
// build_dyn_trait_method_calls_from_fat_ptrs integration
// ============================================================

/// build_dyn_trait_method_calls_from_fat_ptrs populates param_kinds from stdlib.
#[test]
fn test_build_dyn_trait_method_calls_populates_param_kinds() {
    use landin_compiler::mir::build_dyn_trait_method_calls_from_fat_ptrs;
    use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern("Clone");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![
                VtableEntry {
                    method_name: interner.get_or_intern("clone"),
                    fn_name: interner.get_or_intern("landin_S_clone"),
                },
                VtableEntry {
                    method_name: interner.get_or_intern("clone_from"),
                    fn_name: interner.get_or_intern("landin_S_clone_from"),
                },
            ],
        },
    );
    let fps = landin_compiler::mir::build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert!(!calls.is_empty());

    // clone has 0 params → empty param_kinds
    let clone_call = calls
        .iter()
        .find(|c| c.method_name == "clone")
        .expect("clone call");
    assert_eq!(clone_call.param_kinds.len(), 0);

    // clone_from has 1 param → param_kinds has 1 entry
    let clone_from_call = calls
        .iter()
        .find(|c| c.method_name == "clone_from")
        .expect("clone_from call");
    assert_eq!(clone_from_call.param_kinds.len(), 1);
}
