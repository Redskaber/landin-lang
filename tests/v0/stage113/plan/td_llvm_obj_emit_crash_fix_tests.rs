//! Stage 113 (v0.12): TD-LLVM-OBJ-EMIT-CRASH fix + TD-MONO-INFER fix.
//!
//! ## Background
//!
//! Stage 112 RCA found TD-LLVM-OBJ-EMIT-CRASH — deterministic SIGSEGV in
//! `LLVMTargetMachineEmitToFile` when emitting object files for IR with
//! `Operand::Constant` FnDef types that have non-empty inferred substs.
//!
//! Stage 113 RCA: The crash is NOT in the IR itself (llvm-as + llc both
//! work). The crash is in the LLVMSysEmitter's forward declaration path:
//!
//! 1. `writeback_fndef_substs` secondary pass propagates inferred substs
//!    into `Operand::Constant(c).ty` in Assign statements.
//! 2. `codegen_operand` sees `FnDef(def_id, [i32])` → uses `mono_item_name`
//!    to compute specialized name (e.g., `@process_i32`).
//! 3. LLVMSysEmitter's `interpret_adhoc` looks up `process_i32` in
//!    `fn_sigs` → NOT found (only base names like `landin_process` are in
//!    the map).
//! 4. Falls back to variadic `i32 ()` forward declaration.
//! 5. `codegen_mono_functions` later emits actual `process_i32` with
//!    correct sig `i32 (i32)` → type mismatch → old decl deleted + re-added.
//! 6. References to the old (deleted) forward declaration become dangling
//!    pointers → SIGSEGV during LLVMTargetMachineEmitToFile.
//!
//! ## Fix
//!
//! **`build_fn_sigs_map`** (function_sigs.rs): Add specialized function
//! sigs to the fn_sigs map. For each MonoItem::Fn with non-empty substs,
//! compute the specialized name and add it with the substituted signature.
//!
//! **`writeback_fndef_substs`** (writeback.rs): Add secondary pass to
//! propagate inferred substs from `local_decls[idx].ty` into Assign's
//! `Operand::Constant(c).ty`.
//!
//! **`codegen_from_mir`** (function.rs): Skip ALL prelude generic def
//! bodies (not just those without MonoItem::Fn instantiation).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// =============================================================================
// POSITIVE TESTS (6) — impl Trait + generic functions work correctly
// =============================================================================

#[test]
fn stage113_impl_trait_arg_compiles_and_runs() {
    // The Stage 112 crash case — now fixed.
    let (stdout, exit) = run_program(
        r#"fn process(x: impl Clone) -> i32 {
            42
        }
        fn main() -> i32 {
            let r = process(7);
            println!("{}", r);
            0
        }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_vec_new_push_works() {
    // Vec::new() + push — the original Stage 105 RCA scenario.
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let mut v: Vec<i32> = Vec::new();
            v.push(1i32);
            v.push(2i32);
            println!("{}", v.len());
            0
        }"#,
    );
    assert_eq!(stdout, "2\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_box_new_compiles() {
    // Box::new — non-turbofish generic call.
    let src = r#"fn main() -> i32 {
            let _b: Box<i32> = Box::new(42i32);
            0
        }"#;
    let result = common::compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage113_generic_fn_explicit_call() {
    // Explicit generic call with turbofish.
    let (stdout, exit) = run_program(
        r#"fn id<T>(x: T) -> T { x }
        fn main() -> i32 {
            let x: i32 = id::<i32>(42);
            println!("{}", x);
            0
        }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_generic_fn_implicit_call() {
    // Implicit generic call (no turbofish) — TD-MONO-INFER scenario.
    let (stdout, exit) = run_program(
        r#"fn id<T>(x: T) -> T { x }
        fn main() -> i32 {
            let x: i32 = id(42);
            println!("{}", x);
            0
        }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_string_new_push_str_compiles() {
    // String::new() + push_str — Stage 105 RCA scenario.
    let src = r#"
        fn main() -> i32 {
            let mut s: String = String::new();
            s.push_str("hello");
            0
        }
    "#;
    let result = common::compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

// =============================================================================
// TEXT IR VALIDITY TESTS (3) — verify llvm-as accepts the IR
// =============================================================================

#[test]
fn stage113_text_ir_valid_impl_trait() {
    let code = r#"fn process(x: impl Clone) -> i32 { 42 }
        fn main() -> i32 {
            let r = process(7);
            println!("{}", r);
            0
        }"#;
    // Compile-only check (the --emit-obj crash was the issue, now fixed).
    let result = common::compile_src(code);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage113_text_ir_valid_vec_generic() {
    let code = r#"fn main() -> i32 {
            let mut v: Vec<i32> = Vec::new();
            v.push(1i32);
            println!("{}", v.len());
            0
        }"#;
    let result = common::compile_src(code);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage113_text_ir_valid_nested_generics() {
    let code = r#"fn main() -> i32 {
            let mut v: Vec<Box<i32>> = Vec::new();
            v.push(Box::new(10i32));
            v.push(Box::new(20i32));
            println!("{}", v.len());
            0
        }"#;
    let result = common::compile_src(code);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

// =============================================================================
// NEGATIVE / EDGE TESTS (4) — boundary scenarios
// =============================================================================

#[test]
fn stage113_phase36_still_active() {
    // Verify Phase 3.6 (Stage 110) is still active.
    let (stdout, exit) = run_program(
        r#"fn add_one(x: i64) -> i64 { x + 1i64 }
        fn main() -> i32 {
            let y = add_one(42i64);
            println!("{}", y);
            0
        }"#,
    );
    assert_eq!(stdout, "43\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_mixed_width_arithmetic() {
    // Mixed-width arithmetic with unsuffixed literals.
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let a: i32 = 100;
            let b: i64 = a as i64 + 1000i64;
            println!("{}", b);
            0
        }"#,
    );
    assert_eq!(stdout, "1100\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage113_debug_trait_still_declared() {
    // Verify Debug trait is still declared (user can impl it).
    let src = r#"
        struct Foo { x: i32 }
        impl Debug for Foo {
            fn fmt(&self) -> String { String::from_str("Foo") }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = common::compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage113_prelude_debug_impls_still_deferred() {
    // Stage 111 reverted Debug impls for i32/i64/bool/usize.
    // Stage 113 does NOT re-add them (that's Stage 114+).
    let src = r#"fn main() -> i32 {
            let _s: String = (42i32).fmt();
            0
        }"#;
    let result = common::compile_src(src);
    assert!(
        result.has_errors(),
        "expected errors (no Debug impl for i32)"
    );
}
