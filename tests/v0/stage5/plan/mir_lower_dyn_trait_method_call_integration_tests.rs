//! Stage 5.78: HirExprKind::MethodCall dyn Trait integration tests
//!
//! Tests the FIRST real `mir/lower` integration of dyn Trait data:
//! - `build_dyn_trait_call_terminator()` helper constructs a `TerminatorKind::Call`
//!   with a marker Const whose Int value is the index into `cx.mir.dyn_trait_calls`
//! - `HirExprKind::MethodCall` branch queries `cx.dyn_trait_plan()` and uses
//!   the helper when a match is found; falls through to legacy path otherwise.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::hir::lower::lower_crate;
use landin_compiler::lexer::tokenize;
use landin_compiler::mir::body::TerminatorKind;
use landin_compiler::mir::dyn_trait::DynTraitMethodCall;
use landin_compiler::mir::lower::{lower_hir_body_to_mir_full, MirLowerCtxt};
use landin_compiler::mir::place::{LocalId, Operand, PlaceKind};
use landin_compiler::mir::ty::{ConstVal, TyKind};
use landin_compiler::mir::{
    build_dyn_trait_call_terminator, build_dyn_trait_mir_plan, DynTraitFatPtr,
};
use landin_compiler::parser::Parser;
use landin_compiler::resolve::resolve_crate;
use landin_compiler::session::Span;
use landin_compiler::stdlib::StdlibTypeKind;
use lasso::Rodeo;

/// Helper: extract LocalId from an Operand::Copy(Place).
fn local_of(op: &Operand) -> LocalId {
    match op {
        Operand::Copy(p) | Operand::Move(p) => match &p.kind {
            PlaceKind::Local(id) => *id,
            _ => panic!("expected PlaceKind::Local"),
        },
        _ => panic!("expected Operand::Copy/Move"),
    }
}

// ============================================================
// Helper-function tests: build_dyn_trait_call_terminator
// ============================================================

/// Helper: build a MirLowerCtxt for direct testing of the helper.
fn make_cx() -> MirLowerCtxt<'static> {
    // Leak a Rodeo so we can get a 'static reference for the cx.
    // Test-only — production code uses scoped references.
    let interner = Box::leak(Box::new(Rodeo::new()));
    MirLowerCtxt::new(interner, Span::DUMMY)
}

/// Basic construction: returns TerminatorKind::Call.
#[test]
fn test_build_dyn_trait_call_terminator_returns_call() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let recv = LocalId(0);
    let dest = LocalId(1);
    let terminator = build_dyn_trait_call_terminator(&mut cx, &call, recv, &[], dest, Span::DUMMY);
    assert!(matches!(&terminator.kind, TerminatorKind::Call { .. }));
}

/// Function operand is Operand::Constant.
#[test]
fn test_build_dyn_trait_call_terminator_func_is_constant() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call { func, .. } = &terminator.kind {
        assert!(matches!(func, Operand::Constant(_)));
    } else {
        panic!("expected TerminatorKind::Call");
    }
}

/// Function operand's ConstVal::Int is the side-table index (0 for first call).
#[test]
fn test_build_dyn_trait_call_terminator_index_zero_for_first() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call {
        func: Operand::Constant(c),
        ..
    } = &terminator.kind
    {
        assert!(matches!(c.val, ConstVal::Int(0)));
    } else {
        panic!("expected Call with Constant func");
    }
}

/// Stage 15.65: Each call carries its own dyn_trait_call info on the terminator.
/// (Previously: side-table index increments. Now: each terminator is self-contained.)
#[test]
fn test_build_dyn_trait_call_terminator_index_increments() {
    let mut cx = make_cx();
    let call1 = DynTraitMethodCall::new("Drop", "A", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let call2 = DynTraitMethodCall::new("Drop", "B", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let call3 = DynTraitMethodCall::new("Clone", "C", "clone", 0, 0, StdlibTypeKind::Unit, vec![]);

    let t1 =
        build_dyn_trait_call_terminator(&mut cx, &call1, LocalId(0), &[], LocalId(1), Span::DUMMY);
    let t2 =
        build_dyn_trait_call_terminator(&mut cx, &call2, LocalId(0), &[], LocalId(1), Span::DUMMY);
    let t3 =
        build_dyn_trait_call_terminator(&mut cx, &call3, LocalId(0), &[], LocalId(1), Span::DUMMY);

    // Stage 15.65: side-table removed — verify each terminator carries its own call info.
    let check = |t: &TerminatorKind| {
        matches!(
            t,
            TerminatorKind::Call {
                dyn_trait_call: Some(_),
                ..
            }
        )
    };
    assert!(check(&t1.kind));
    assert!(check(&t2.kind));
    assert!(check(&t3.kind));

    // Verify the call info is distinct (different trait names).
    let info1 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t1.kind
    {
        c.trait_name.clone()
    } else {
        panic!()
    };
    let info2 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t2.kind
    {
        c.trait_name.clone()
    } else {
        panic!()
    };
    let info3 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t3.kind
    {
        c.trait_name.clone()
    } else {
        panic!()
    };
    assert_eq!(info1, "Drop");
    assert_eq!(info2, "Drop");
    assert_eq!(info3, "Clone");
    // call1 and call2 have different type names.
    let type1 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t1.kind
    {
        c.type_name.clone()
    } else {
        panic!()
    };
    let type2 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t2.kind
    {
        c.type_name.clone()
    } else {
        panic!()
    };
    assert_ne!(type1, type2);
}

/// Terminator's `dyn_trait_call` field preserves the call info.
#[test]
fn test_build_dyn_trait_call_terminator_preserves_call_info() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Display", "Vec", "fmt", 2, 1, StdlibTypeKind::Unit, vec![]);
    let term =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call { dyn_trait_call, .. } = &term.kind {
        let recorded = dyn_trait_call
            .as_ref()
            .expect("dyn_trait_call should be Some");
        assert_eq!(recorded.trait_name, "Display");
        assert_eq!(recorded.type_name, "Vec");
        assert_eq!(recorded.method_name, "fmt");
        assert_eq!(recorded.slot_index, 2);
        assert_eq!(recorded.param_count, 1);
    } else {
        panic!("expected Call terminator");
    }
}

/// Args list: self first, then explicit args.
#[test]
fn test_build_dyn_trait_call_terminator_args_self_first() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Foo", "S", "bar", 0, 2, StdlibTypeKind::Unit, vec![]);
    let terminator = build_dyn_trait_call_terminator(
        &mut cx,
        &call,
        LocalId(5),
        &[LocalId(6), LocalId(7)],
        LocalId(8),
        Span::DUMMY,
    );
    if let TerminatorKind::Call { args, .. } = &terminator.kind {
        assert_eq!(args.len(), 3); // self + 2 args
                                   // self is at index 0
        assert_eq!(local_of(&args[0]), LocalId(5));
        // arg0 at index 1
        assert_eq!(local_of(&args[1]), LocalId(6));
        // arg1 at index 2
        assert_eq!(local_of(&args[2]), LocalId(7));
    } else {
        panic!("expected Call");
    }
}

/// Destination is the given local.
#[test]
fn test_build_dyn_trait_call_terminator_destination() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(42), Span::DUMMY);
    if let TerminatorKind::Call { destination, .. } = &terminator.kind {
        assert_eq!(local_of(&Operand::Copy(destination.clone())), LocalId(42));
    } else {
        panic!("expected Call");
    }
}

/// Target is None (caller sets it).
#[test]
fn test_build_dyn_trait_call_terminator_target_none() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call { target, .. } = &terminator.kind {
        assert!(target.is_none());
    } else {
        panic!("expected Call");
    }
}

/// Func's Ty is Error (placeholder for codegen to recognize).
#[test]
fn test_build_dyn_trait_call_terminator_func_ty_is_error() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call {
        func: Operand::Constant(c),
        ..
    } = &terminator.kind
    {
        assert!(matches!(c.ty.kind, TyKind::Error));
    } else {
        panic!("expected Call with Constant func");
    }
}

// ============================================================
// Integration tests: HirExprKind::MethodCall branch
// ============================================================

/// Without plan attached: MethodCall falls through to legacy path
/// (TerminatorKind::Call with placeholder Const Int(0) and no side-table entry).
#[test]
fn test_method_call_without_plan_uses_legacy_path() {
    let src = "fn f() { let x = 1; x.foo(); }";
    let mut interner = Rodeo::new();
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    let mut hir = lower_crate(&krate, &interner);
    let _ = resolve_crate(&mut hir, &mut interner);

    let (mir, _unify, _) = lower_hir_body_to_mir_full(&hir.bodies[0].1, &interner, &hir, None);

    // Stage 15.65: side-table removed — verify no terminator has dyn_trait_call set.
    let has_dyn_call = mir.basic_blocks.iter().any(|bb| {
        matches!(
            &bb.terminator.kind,
            TerminatorKind::Call {
                dyn_trait_call: Some(_),
                ..
            }
        )
    });
    assert!(
        !has_dyn_call,
        "expected no dyn_trait_call terminators without plan"
    );
}

/// With plan attached AND method_name matches: side-table records the call.
#[test]
fn test_method_call_with_matching_plan_records_dyn_call() {
    // We can't easily attach a plan via the public `lower_hir_body_to_mir_full`
    // entry point (driver doesn't wire it yet — Stage 5.80+). So this test
    // directly constructs a MirLowerCtxt, attaches a plan, and lowers a
    // small MethodCall via the internal `lower_expr_to_operand` (which is
    // private — but we can verify via the public `build_dyn_trait_call_terminator`
    // helper that the side-table mechanism works end-to-end).
    //
    // The end-to-end integration with the driver is verified in Stage 5.80+.

    // Build a plan with one method call: Drop::S::drop
    let fps = [DynTraitFatPtr::new("Drop", "S")];
    let calls = [DynTraitMethodCall::new(
        "Drop",
        "S",
        "drop",
        0,
        0,
        StdlibTypeKind::Unit,
        vec![],
    )];
    let plan = build_dyn_trait_mir_plan(&fps, &calls);

    // Set the plan on a fresh cx, then call the helper directly.
    let interner = Box::leak(Box::new(Rodeo::new()));
    let mut cx = MirLowerCtxt::new(interner, Span::DUMMY);
    cx.set_dyn_trait_plan(plan);

    // Use the helper — simulates what the MethodCall branch does when
    // find_dyn_trait_method_call_in_plan_by_method returns Some.
    let call = DynTraitMethodCall::new("Drop", "S", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let terminator =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);

    // Stage 15.65: side-table removed — verify via terminator's dyn_trait_call field.
    if let TerminatorKind::Call { dyn_trait_call, .. } = &terminator.kind {
        let recorded = dyn_trait_call
            .as_ref()
            .expect("dyn_trait_call should be Some");
        assert_eq!(recorded.method_name, "drop");
        assert_eq!(recorded.trait_name, "Drop");
        assert_eq!(recorded.type_name, "S");
    } else {
        panic!("expected Call terminator");
    }
}

/// Multiple dyn Trait calls each carry distinct call info on their terminators.
#[test]
fn test_multiple_dyn_trait_calls_get_distinct_call_info() {
    let mut cx = make_cx();
    let call1 = DynTraitMethodCall::new("Drop", "A", "drop", 0, 0, StdlibTypeKind::Unit, vec![]);
    let call2 = DynTraitMethodCall::new("Clone", "B", "clone", 0, 0, StdlibTypeKind::Unit, vec![]);

    let t1 =
        build_dyn_trait_call_terminator(&mut cx, &call1, LocalId(0), &[], LocalId(1), Span::DUMMY);
    let t2 =
        build_dyn_trait_call_terminator(&mut cx, &call2, LocalId(0), &[], LocalId(1), Span::DUMMY);

    // Stage 15.65: side-table removed — verify each terminator carries its own call info.
    let info1 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t1.kind
    {
        c.clone()
    } else {
        panic!("t1 should have dyn_trait_call Some");
    };
    let info2 = if let TerminatorKind::Call {
        dyn_trait_call: Some(c),
        ..
    } = &t2.kind
    {
        c.clone()
    } else {
        panic!("t2 should have dyn_trait_call Some");
    };

    // The two calls have distinct trait/type/method names.
    assert_ne!(info1.trait_name, info2.trait_name);
    assert_ne!(info1.type_name, info2.type_name);
    assert_ne!(info1.method_name, info2.method_name);
}

/// DynTraitMethodCall records the original method_name string faithfully.
#[test]
fn test_terminator_records_method_name_verbatim() {
    let mut cx = make_cx();
    let call = DynTraitMethodCall::new(
        "Iterator",
        "Range",
        "size_hint",
        1,
        0,
        StdlibTypeKind::Unit,
        vec![],
    );
    let term =
        build_dyn_trait_call_terminator(&mut cx, &call, LocalId(0), &[], LocalId(1), Span::DUMMY);
    if let TerminatorKind::Call {
        dyn_trait_call: Some(recorded),
        ..
    } = &term.kind
    {
        assert_eq!(recorded.method_name, "size_hint");
    } else {
        panic!("expected Call terminator with dyn_trait_call Some");
    }
}
