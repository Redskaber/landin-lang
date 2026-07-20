//! Round 5 deep inspection tests for cross-stage integration.
//!
//! These tests verify that the compiler produces *correct output structure*,
//! not just "no errors". They check:
//!   - typeck_results contains expected resolved types
//!   - MIR contains StorageLive/StorageDead/Assert in expected places
//!   - Path resolution correctly maps to locals
//!   - Fn sig is unified with body value type
//!
//! Run with: cargo run --example round5_deep

use landin_compiler::driver::compile;
use landin_compiler::mir::body::{AssertMessage, StatementKind, Terminator};
use landin_compiler::mir::lvalue::{BinOp, LocalId};
use landin_compiler::mir::ty::TyKind;

fn main() {
    let mut pass = 0;
    let mut fail = 0;

    // Test 1: typeck writes back resolved Int type
    {
        let result = compile("fn f() { let x = 42; }");
        let mir = &result.mirs[0];
        let has_i32 = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
        if has_i32 {
            println!("T01_typeck_writeback_i32: PASS");
            pass += 1;
        } else {
            println!("T01_typeck_writeback_i32: FAIL — no i32 local found");
            fail += 1;
        }
    }

    // Test 2: typeck writes back Bool type
    {
        let result = compile("fn f() { let x = true; }");
        let mir = &result.mirs[0];
        let has_bool = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Bool));
        if has_bool {
            println!("T02_typeck_writeback_bool: PASS");
            pass += 1;
        } else {
            println!("T02_typeck_writeback_bool: FAIL — no bool local found");
            fail += 1;
        }
    }

    // Test 3: StorageLive emitted for return local (LocalId(0))
    {
        let result = compile("fn f() {}");
        let mir = &result.mirs[0];
        let has_live_return = mir.basic_blocks[0]
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(0))));
        if has_live_return {
            println!("T03_storage_live_return: PASS");
            pass += 1;
        } else {
            println!("T03_storage_live_return: FAIL");
            fail += 1;
        }
    }

    // Test 4: StorageLive emitted for params
    {
        let result = compile("fn f(a: i32, b: bool) {}");
        let mir = &result.mirs[0];
        let has_live_a = mir.basic_blocks[0]
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(1))));
        let has_live_b = mir.basic_blocks[0]
            .statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(2))));
        if has_live_a && has_live_b {
            println!("T04_storage_live_params: PASS");
            pass += 1;
        } else {
            println!(
                "T04_storage_live_params: FAIL (a={}, b={})",
                has_live_a, has_live_b
            );
            fail += 1;
        }
    }

    // Test 5: StorageDead emitted before Return
    {
        let result = compile("fn f(x: i32) { let y = x + 1; }");
        let mir = &result.mirs[0];
        let return_bb = mir
            .basic_blocks
            .iter()
            .find(|bb| matches!(bb.terminator, Terminator::Return))
            .expect("no Return terminator");
        let dead_count = return_bb
            .statements
            .iter()
            .filter(|s| matches!(s.kind, StatementKind::StorageDead(_)))
            .count();
        if dead_count >= 2 {
            println!("T05_storage_dead_before_return: PASS ({} dead)", dead_count);
            pass += 1;
        } else {
            println!("T05_storage_dead_before_return: FAIL ({} dead)", dead_count);
            fail += 1;
        }
    }

    // Test 6: Assert(Overflow) emitted for addition
    {
        let result = compile("fn f(a: i32, b: i32) -> i32 { a + b }");
        let mir = &result.mirs[0];
        let has_overflow_assert = mir.basic_blocks.iter().any(|bb| {
            matches!(
                &bb.terminator,
                Terminator::Assert {
                    msg: AssertMessage::Overflow(BinOp::Add, _, _),
                    ..
                }
            )
        });
        if has_overflow_assert {
            println!("T06_assert_overflow_add: PASS");
            pass += 1;
        } else {
            println!("T06_assert_overflow_add: FAIL");
            fail += 1;
        }
    }

    // Test 7: No Assert for comparison
    {
        let result = compile("fn f(a: i32, b: i32) -> bool { a == b }");
        let mir = &result.mirs[0];
        let has_assert = mir
            .basic_blocks
            .iter()
            .any(|bb| matches!(&bb.terminator, Terminator::Assert { .. }));
        if !has_assert {
            println!("T07_no_assert_comparison: PASS");
            pass += 1;
        } else {
            println!("T07_no_assert_comparison: FAIL — Assert found for comparison");
            fail += 1;
        }
    }

    // Test 8: typeck_results populated
    {
        let result = compile("fn f(x: i32) -> i32 { x }");
        if !result.typeck_results.is_empty() && !result.typeck_results[0].local_types.is_empty() {
            println!("T08_typeck_results_populated: PASS");
            pass += 1;
        } else {
            println!("T08_typeck_results_populated: FAIL");
            fail += 1;
        }
    }

    // Test 9: fn sig return type unified with body (return local has declared type)
    {
        let result = compile("fn f() -> i32 { 42 }");
        let mir = &result.mirs[0];
        let return_ty = &mir.local_decls[0].ty;
        if matches!(
            return_ty.kind,
            TyKind::Int(landin_compiler::ast::IntTy::I32)
        ) {
            println!("T09_fn_sig_unified: PASS");
            pass += 1;
        } else {
            println!(
                "T09_fn_sig_unified: FAIL — return ty = {:?}",
                return_ty.kind
            );
            fail += 1;
        }
    }

    // Test 10: Path resolves to local (G1 regression — variable use after let)
    {
        let result = compile("fn f() { let x = 1; let y = x; }");
        // If G1 regressed, x would resolve to Error and y's type would be Error.
        let mir = &result.mirs[0];
        let has_i32 = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
        let no_error = mir
            .local_decls
            .iter()
            .all(|ld| !matches!(&ld.ty.kind, TyKind::Error));
        if has_i32 && no_error {
            println!("T10_path_resolves_to_local: PASS");
            pass += 1;
        } else {
            println!(
                "T10_path_resolves_to_local: FAIL (has_i32={}, no_error={})",
                has_i32, no_error
            );
            fail += 1;
        }
    }

    // Test 11: String literal has Str type (P1-2 regression)
    {
        let result = compile("fn f() { let s = \"hello\"; }");
        let mir = &result.mirs[0];
        let has_str = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Str));
        if has_str {
            println!("T11_string_literal_str_type: PASS");
            pass += 1;
        } else {
            println!("T11_string_literal_str_type: FAIL");
            fail += 1;
        }
    }

    // Test 12: Unsuffixed int literal defaults to i32
    {
        let result = compile("fn f() { let x = 42; }");
        let mir = &result.mirs[0];
        let has_i32 = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
        if has_i32 {
            println!("T12_unsuffixed_int_defaults_i32: PASS");
            pass += 1;
        } else {
            println!("T12_unsuffixed_int_defaults_i32: FAIL");
            fail += 1;
        }
    }

    // Test 13: Unsuffixed float defaults to f64
    {
        let result = compile("fn f() { let x = 3.14; }");
        let mir = &result.mirs[0];
        let has_f64 = mir.local_decls.iter().any(|ld| {
            matches!(
                &ld.ty.kind,
                TyKind::Float(landin_compiler::ast::FloatTy::F64)
            )
        });
        if has_f64 {
            println!("T13_unsuffixed_float_defaults_f64: PASS");
            pass += 1;
        } else {
            println!("T13_unsuffixed_float_defaults_f64: FAIL");
            fail += 1;
        }
    }

    // Test 14: let with u64 annotation unifies literal
    {
        let result = compile("fn f() { let z: u64 = 100; }");
        let mir = &result.mirs[0];
        let has_u64 = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, TyKind::Uint(landin_compiler::ast::UintTy::U64)));
        if has_u64 {
            println!("T14_let_u64_annotation: PASS");
            pass += 1;
        } else {
            println!("T14_let_u64_annotation: FAIL");
            fail += 1;
        }
    }

    // Test 15: Short-circuit && produces control flow (not BitAnd)
    {
        let result = compile("fn f(a: bool, b: bool) -> bool { a && b }");
        let mir = &result.mirs[0];
        // && should produce multiple basic blocks (short-circuit)
        let has_multiple_blocks = mir.basic_blocks.len() >= 3;
        if has_multiple_blocks {
            println!(
                "T15_short_circuit_and_control_flow: PASS ({} blocks)",
                mir.basic_blocks.len()
            );
            pass += 1;
        } else {
            println!(
                "T15_short_circuit_and_control_flow: FAIL ({} blocks)",
                mir.basic_blocks.len()
            );
            fail += 1;
        }
    }

    println!("\n=== Deep inspection: {} PASS, {} FAIL ===", pass, fail);
}
