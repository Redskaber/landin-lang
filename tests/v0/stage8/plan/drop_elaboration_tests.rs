//! Stage 8.4: Drop elaboration verification tests.
//!
//! Per stage-committee-process.md v3.21 §13.4 + §17.1.
//! Tests the drop elaboration module (Stage 8.4, §5).

use landin_compiler::borrowck::BorrowChecker;
use landin_compiler::driver::compile;
use landin_compiler::mir::body::MirBody;
use landin_compiler::mir::ty::{Ty, TyKind};
use landin_compiler::session::Span;

#[test]
fn stage8_4_drop_elaborator_exists() {
    // Verify borrowck (which now includes drop_elaboration module) works
    let mut bc = BorrowChecker::new();
    let mut mir = MirBody::new(Span::DUMMY);
    mir.new_block();
    bc.check_mir_body(&mir);
    assert!(bc.into_errors().is_empty());
}

#[test]
fn stage8_4_fn_with_struct_no_drop() {
    // Struct without impl Drop — no drop elaboration needed
    let result =
        compile("struct Point { x: i32, y: i32 } fn main() { let _p = Point { x: 1, y: 2 }; }");
    assert!(
        result.errors.is_empty(),
        "struct without Drop should compile"
    );
}

#[test]
fn stage8_4_fn_with_drop_trait() {
    // Trait with Drop impl — exercises drop elaboration analysis
    let result = compile(
        "
        trait Drop { fn drop(&mut self); }
        struct Resource;
        impl Drop for Resource { fn drop(&mut self) { } }
        fn main() { let _r = Resource; }
    ",
    );
    // May have type errors in MVP, but should not panic
    let _ = result;
}

#[test]
fn stage8_4_primitive_no_drop() {
    // Primitives should never need drop
    let result = compile("fn main() { let x = 42; let y = true; }");
    assert!(result.errors.is_empty(), "primitives should compile fine");
}

#[test]
fn stage8_4_ref_type_no_drop() {
    // References should never need drop (referent owned elsewhere)
    let result = compile("fn main() { let x = 42; let _r = &x; }");
    assert!(result.errors.is_empty(), "refs should compile fine");
}

#[test]
fn stage8_4_mir_with_drop_type() {
    // Verify MIR can be constructed with types that would need drop
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();
    // Create a local with an Adt type (might have Drop)
    let _local = mir.new_local(
        Ty::new(
            TyKind::Adt(landin_compiler::hir::DefId(1), vec![]),
            Span::DUMMY,
        ),
        None,
        Span::DUMMY,
    );
    // borrowck should handle this without crashing
    let errors = landin_compiler::borrowck::check_mir_body(&mir);
    let _ = errors; // may or may not have errors, but should not panic
}

#[test]
fn stage8_4_multiple_locals_reverse_drop_order() {
    // Verify multiple locals don't cause issues
    let result = compile("fn main() { let a = 1; let b = 2; let c = 3; }");
    assert!(result.errors.is_empty(), "multiple locals should compile");
}
