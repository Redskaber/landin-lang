//! Stage 5.82: TD-016 — dyn Trait return type refinement tests
//!
//! Tests the new `return_kind: StdlibTypeKind` field on `DynTraitMethodCall`
//! and the `stdlib_type_kind_to_emit_type()` converter. Verifies that
//! `codegen_dyn_trait_call` now emits precise return types instead of the
//! I32 placeholder.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{stdlib_type_kind_to_emit_type, EmitType, TextEmitter};
use landin_compiler::mir::body::MirBody;
use landin_compiler::mir::dyn_trait::DynTraitMethodCall;
use landin_compiler::mir::place::{LocalId, Operand, Place};
use landin_compiler::mir::ty::{Ty, TyKind};
use landin_compiler::mir::DynTraitFatPtr;
use landin_compiler::session::Span;
use landin_compiler::stdlib::StdlibTypeKind;
use landin_compiler::{codegen_dyn_trait_call, stdlib_trait_methods};
use lasso::Rodeo;

// ============================================================
// stdlib_type_kind_to_emit_type tests
// ============================================================

/// I32 → EmitType::I32
#[test]
fn test_stdlib_type_kind_to_emit_type_i32() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::I32),
        EmitType::I32
    );
}

/// U32 → EmitType::I32 (same width)
#[test]
fn test_stdlib_type_kind_to_emit_type_u32() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::U32),
        EmitType::I32
    );
}

/// Bool → EmitType::I8 (bools are i1 in LLVM but stored as i8)
#[test]
fn test_stdlib_type_kind_to_emit_type_bool() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::Bool),
        EmitType::I8
    );
}

/// Char → EmitType::I8 (chars are stored as i8 in this codebase)
#[test]
fn test_stdlib_type_kind_to_emit_type_char() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::Char),
        EmitType::I8
    );
}

/// Unit → EmitType::Void
#[test]
fn test_stdlib_type_kind_to_emit_type_unit() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::Unit),
        EmitType::Void
    );
}

/// Never → EmitType::Void
#[test]
fn test_stdlib_type_kind_to_emit_type_never() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::Never),
        EmitType::Void
    );
}

/// F64 → EmitType::F64
#[test]
fn test_stdlib_type_kind_to_emit_type_f64() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::F64),
        EmitType::F64
    );
}

/// F32 → EmitType::F32
#[test]
fn test_stdlib_type_kind_to_emit_type_f32() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::F32),
        EmitType::F32
    );
}

/// I64 → EmitType::I64
#[test]
fn test_stdlib_type_kind_to_emit_type_i64() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::I64),
        EmitType::I64
    );
}

/// I128 → EmitType::I128
#[test]
fn test_stdlib_type_kind_to_emit_type_i128() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::I128),
        EmitType::I128
    );
}

/// AllocType → EmitType::OpaquePtr (dyn Trait receivers are fat pointers)
#[test]
fn test_stdlib_type_kind_to_emit_type_alloc_type() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::AllocType),
        EmitType::OpaquePtr
    );
}

/// StdType → EmitType::OpaquePtr
#[test]
fn test_stdlib_type_kind_to_emit_type_std_type() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::StdType),
        EmitType::OpaquePtr
    );
}

/// Unknown → EmitType::OpaquePtr (safe fallback)
#[test]
fn test_stdlib_type_kind_to_emit_type_unknown() {
    assert_eq!(
        stdlib_type_kind_to_emit_type(StdlibTypeKind::Unknown),
        EmitType::OpaquePtr
    );
}

// ============================================================
// DynTraitMethodCall return_kind field tests
// ============================================================

/// DynTraitMethodCall::new includes return_kind field.
#[test]
fn test_dyn_trait_method_call_new_with_return_kind() {
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    assert_eq!(call.return_kind, StdlibTypeKind::Unit);
}

/// from_fat_ptr includes return_kind field.
#[test]
fn test_dyn_trait_method_call_from_fat_ptr_with_return_kind() {
    let fp = DynTraitFatPtr::new("Clone", "S");
    let call =
        DynTraitMethodCall::from_fat_ptr(&fp, "clone", 0, 0, StdlibTypeKind::AllocType, vec![]);
    assert_eq!(call.return_kind, StdlibTypeKind::AllocType);
    assert_eq!(call.trait_name, "Clone");
}

/// return_kind preserves the value passed in.
#[test]
fn test_return_kind_preserved() {
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 0, 1, StdlibTypeKind::Bool, vec![]);
    assert_eq!(call.return_kind, StdlibTypeKind::Bool);
}

// ============================================================
// codegen_dyn_trait_call uses return_kind tests
// ============================================================

/// Helper: build a MirBody with one dyn_trait_calls entry with given return_kind.
fn make_mir_with_return_kind(return_kind: StdlibTypeKind) -> MirBody {
    let mut mir = MirBody::new(Span::DUMMY);
    mir.new_local(Ty::new(TyKind::Error, Span::DUMMY), None, Span::DUMMY);
    mir.new_local(Ty::new(TyKind::Error, Span::DUMMY), None, Span::DUMMY);
    mir.dyn_trait_calls.push(DynTraitMethodCall::new(
        "Foo",
        "S",
        "bar",
        0,
        0,
        return_kind,
        vec![],
    ));
    mir
}

/// Drop::drop (return Unit) → call void %v
#[test]
fn test_codegen_dyn_trait_call_void_return() {
    let mir = make_mir_with_return_kind(StdlibTypeKind::Unit);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call(
        &mut emitter,
        &mir,
        0,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("call void %v"),
        "expected 'call void %v', got: {}",
        output
    );
}

/// Method returning I32 → call i32 %v
#[test]
fn test_codegen_dyn_trait_call_i32_return() {
    let mir = make_mir_with_return_kind(StdlibTypeKind::I32);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call(
        &mut emitter,
        &mir,
        0,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("call i32 %v"),
        "expected 'call i32 %v', got: {}",
        output
    );
}

/// Method returning F64 → call double %v
#[test]
fn test_codegen_dyn_trait_call_f64_return() {
    let mir = make_mir_with_return_kind(StdlibTypeKind::F64);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call(
        &mut emitter,
        &mir,
        0,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("call double %v"),
        "expected 'call double %v', got: {}",
        output
    );
}

/// Method returning Bool → call i8 %v
#[test]
fn test_codegen_dyn_trait_call_bool_return() {
    let mir = make_mir_with_return_kind(StdlibTypeKind::Bool);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call(
        &mut emitter,
        &mir,
        0,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    assert!(
        output.contains("call i8 %v"),
        "expected 'call i8 %v', got: {}",
        output
    );
}

/// Method returning AllocType (Self) → call ptr %v (OpaquePtr maps to ptr in LLVM 19)
#[test]
fn test_codegen_dyn_trait_call_alloc_type_return() {
    let mir = make_mir_with_return_kind(StdlibTypeKind::AllocType);
    let mut emitter = TextEmitter::new();
    let interner = Rodeo::new();
    let layouts = std::collections::HashMap::new();
    let args = vec![Operand::Copy(Place::local(LocalId(0), Span::DUMMY))];

    codegen_dyn_trait_call(
        &mut emitter,
        &mir,
        0,
        &args,
        &interner,
        &layouts,
        &std::collections::HashMap::new(),
    );

    let output = emitter.output_with_globals();
    // OpaquePtr → "i32*" in LLVM IR (legacy default for opaque pointers)
    assert!(
        output.contains("call ptr %v"),
        "expected 'call ptr %v', got: {}",
        output
    );
}

// ============================================================
// build_dyn_trait_method_calls_from_fat_ptrs integration
// ============================================================

/// build_dyn_trait_method_calls_from_fat_ptrs populates return_kind from stdlib.
#[test]
fn test_build_dyn_trait_method_calls_populates_return_kind() {
    use landin_compiler::mir::build_dyn_trait_method_calls_from_fat_ptrs;
    use landin_compiler::traits::{TraitResolver, Vtable, VtableEntry};
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::new();
    let trait_spur = interner.get_or_intern("Drop");
    let type_spur = interner.get_or_intern("S");
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id: landin_compiler::hir::DefId::new(0),
            entries: vec![VtableEntry {
                method_name: interner.get_or_intern("drop"),
                fn_name: "landin_S_drop".to_string(),
            }],
        },
    );
    let fps = landin_compiler::mir::build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fps);
    assert!(!calls.is_empty());
    // Drop::drop returns Unit in stdlib registry
    assert_eq!(calls[0].return_kind, StdlibTypeKind::Unit);
}

/// stdlib_trait_methods return_kind is populated for known traits.
#[test]
fn test_stdlib_trait_methods_have_return_kind() {
    let methods = stdlib_trait_methods("Drop");
    assert!(methods.is_some());
    let methods = methods.unwrap();
    assert!(!methods.is_empty());
    // Drop::drop should return Unit
    let drop_method = methods
        .iter()
        .find(|m| m.name == "drop")
        .expect("drop method");
    assert_eq!(drop_method.return_kind, StdlibTypeKind::Unit);
}
