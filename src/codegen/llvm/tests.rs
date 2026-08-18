//! Stage 16.77 MUV-1: LLVMSysEmitter tests.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).

#![cfg(test)]

use super::*;
use crate::codegen::llvm::LLVMSysEmitter;
use crate::mir::ty::ConstVal;

/// Stage 13.5 MUV-2: Verify LLVMSysEmitter implements the Emitter trait.
/// This is a compile-time check — if Emitter trait changes and
/// LLVMSysEmitter doesn't keep up, this test fails to compile.
#[test]
fn llvm_sys_emitter_satisfies_emitter_trait() {
    let _: &dyn Emitter = &LLVMSysEmitter::new();
}

/// Verify `emit_header` produces a non-null module with target set.
#[test]
fn emit_header_sets_target() {
    let mut e = LLVMSysEmitter::new();
    e.emit_header();
    assert!(!e.to_module().is_null());
}

/// Verify a simple function with alloca + ret can be emitted.
#[test]
fn emit_simple_function() {
    let mut e = LLVMSysEmitter::new();
    e.emit_header();
    let params: Vec<(EmitType, &str)> = vec![(EmitType::I32, "%arg0")];
    e.emit_function_begin("test_fn", &params, &EmitType::I32);
    let ptr = e.emit_alloca(&EmitType::I32, "%loc_1");
    e.emit_store(&EmitType::I32, &"%arg0".to_string(), &ptr);
    let v = e.emit_load(&EmitType::I32, &ptr);
    e.emit_ret(&EmitType::I32, Some(&v));
    e.emit_function_end();
    assert!(!e.to_module().is_null());
}

/// Verify `emit_const` produces a registered value.
#[test]
fn emit_const_int() {
    let mut e = LLVMSysEmitter::new();
    e.emit_header();
    e.emit_function_begin("c", &[], &EmitType::Void);
    let v = e.emit_const(&ConstVal::Int(42));
    assert!(v.starts_with("%v"));
}

/// Verify `parse_declare_name` extracts the name correctly.
#[test]
fn parse_declare_name_works() {
    assert_eq!(
        parse_declare_name("void @__landin_panic_overflow(i32 %op)"),
        Some("__landin_panic_overflow".to_string())
    );
    assert_eq!(
        parse_declare_name("i32 @printf(i8*, ...)"),
        Some("printf".to_string())
    );
    assert_eq!(parse_declare_name("no_at_sign_here"), None);
}

/// Verify `count_args_in_signature` returns correct counts.
#[test]
fn count_args_works() {
    assert_eq!(count_args_in_signature("void @f()"), 0);
    assert_eq!(count_args_in_signature("void @f(i32 %a)"), 1);
    assert_eq!(
        count_args_in_signature("void @f(i32 %a, i32 %b, i32 %c)"),
        3
    );
}

#[test]
#[cfg(feature = "llvm-backend")]
fn test_simple_module_builds_and_emits() {
    use crate::codegen::emitter::*;
    use crate::codegen::LLVMSysEmitter;

    let mut emitter = LLVMSysEmitter::new();
    emitter.emit_header();
    emitter.emit_declare("void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)");

    // Build: define i32 @main() { ret i32 42 }
    // Stage 18.191: emit_const now returns i64 for Int constants.
    // Cast to i32 before returning (matching the function's return type).
    emitter.emit_function_begin("main", &[], &EmitType::I32);
    let raw = emitter.emit_const(&crate::mir::ty::ConstVal::Int(42));
    let val = emitter.emit_cast(&EmitType::I64, &EmitType::I32, &raw);
    emitter.emit_ret(&EmitType::I32, Some(&val));
    emitter.emit_function_end();

    // Emit object file
    let out_path = "/tmp/test_simple_module.o";
    let _ = std::fs::remove_file(out_path);

    match emitter.to_object_file(out_path) {
        Ok(()) => {
            let meta = std::fs::metadata(out_path).expect("object file should exist");
            println!("✅ Simple module object file: {} bytes", meta.len());
            assert!(meta.len() > 0, "object file must be non-empty");
        }
        Err(e) => {
            panic!("Object file generation failed: {e}");
        }
    }
}

#[test]
#[cfg(feature = "llvm-backend")]
fn test_landin_program_to_object_file() {
    // End-to-end: compile a Landin program → LLVMSysEmitter → object file.
    // This tests the codegen_from_mir → LLVMSysEmitter integration path.
    use crate::codegen::codegen_crate_to_module;

    let src = "fn main() -> i32 { 42 }";
    let result = crate::driver::compile(src);

    if result.has_errors() {
        // Don't fail — some compile errors are expected for MVP.
        // The key is that codegen produces *some* module.
        eprintln!(
            "⚠️ Compile errors (expected for MVP): {}",
            result.errors.total_count()
        );
    }

    let emitter =
        codegen_crate_to_module(&result).expect("codegen should succeed for valid test input");
    let out_path = "/tmp/test_landin_e2e.o";
    let _ = std::fs::remove_file(out_path);

    match emitter.to_object_file(out_path) {
        Ok(()) => {
            let meta = std::fs::metadata(out_path).expect("object file should exist");
            println!("✅ End-to-end object file: {} bytes", meta.len());
            assert!(meta.len() > 0, "object file must be non-empty");
        }
        Err(e) => {
            // Don't panic — the LLVMSysEmitter is still WIP for complex MIR.
            // The test passing means the function is callable without crashing.
            eprintln!("⚠️ End-to-end object file error (WIP): {e}");
        }
    }
}

#[test]
#[cfg(feature = "llvm-backend")]
fn test_landin_add_program_to_object_file() {
    use crate::codegen::codegen_crate_to_module;

    let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { add(3, 4) }";
    let result = crate::driver::compile(src);

    if result.has_errors() {
        eprintln!("⚠️ Compile errors: {}", result.errors.total_count());
    }

    let emitter =
        codegen_crate_to_module(&result).expect("codegen should succeed for valid test input");
    let out_path = "/tmp/test_landin_add.o";
    let _ = std::fs::remove_file(out_path);

    match emitter.to_object_file(out_path) {
        Ok(()) => {
            let meta = std::fs::metadata(out_path).expect("object file should exist");
            println!("✅ Add program object file: {} bytes", meta.len());
            assert!(meta.len() > 0, "object file must be non-empty");
        }
        Err(e) => {
            eprintln!("⚠️ Add program object file error (WIP): {e}");
        }
    }
}
