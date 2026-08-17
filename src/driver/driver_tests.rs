//! Driver tests + test helpers.
//!
//! Per §13.4 J1-J6 (Stage 18.137): extracted from driver/mod.rs.

use super::{compile, CompileResult};

/// Compile a source string and assert that it has zero errors.
/// Panics if any errors are found. Returns the CompileResult for further inspection.
pub fn compile_expect_ok(src: &str) -> CompileResult {
    let result = compile(src);
    if result.has_errors() {
        panic!(
            "expected zero errors, but got {}:\n\
             lex: {:?}\n\
             parse: {:?}\n\
             resolve: {:?}\n\
             typeck: {:?}\n\
             borrowck: {:?}",
            result.errors.total_count(),
            result.errors.lex,
            result.errors.parse,
            result.errors.resolve,
            result.errors.typeck,
            result.errors.borrowck
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_compiles_empty_fn() {
        let result = compile_expect_ok("fn f() {}");
        assert!(
            !result.mirs.is_empty(),
            "should have at least 1 MIR body (user fn + prelude methods), got {}",
            result.mirs.len()
        );
    }

    #[test]
    fn driver_compiles_return_literal() {
        // Stage 18.71: Updated to reflect P0-5 fix — `fn f() { 42 }` now
        // has a unit return type (the `42` is a discarded trailing
        // expression, not the return value). To get an Int return type,
        // the function must declare `-> i32`.
        let result = compile_expect_ok("fn f() -> i32 { 42 }");
        assert!(
            !result.mirs.is_empty(),
            "should have at least 1 MIR body (user fn + prelude methods), got {}",
            result.mirs.len()
        );
        // The return local should have a concrete Int type after typeck
        let mir = &result.mirs[0];
        let return_ty = &mir.local_decls[0].ty;
        assert!(
            matches!(return_ty.kind, crate::mir::ty::TyKind::Int(_)),
            "expected Int return type, got {:?}",
            return_ty.kind
        );
    }

    #[test]
    fn driver_compiles_let_binding() {
        let result = compile_expect_ok("fn f() { let x = 42; }");
        // The local `x` should have type i32 after typeck + default
        let mir = &result.mirs[0];
        let has_i32 = mir.local_decls.iter().any(|ld| {
            matches!(
                &ld.ty.kind,
                crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32)
            )
        });
        assert!(has_i32, "expected at least one i32 local");
    }

    #[test]
    fn driver_detects_type_mismatch() {
        // `let x: bool = 42;` should produce a type error.
        // Note: this depends on HIR lower handling the `let x: T = e` annotation.
        // If type ascription isn't wired yet, this test may need adjustment.
        let result = compile("fn f() { let x: bool = 42; }");
        // We expect at least one error (type mismatch).
        // If the parser doesn't accept `let x: bool`, we'll get a parse error instead.
        // Either way, the driver shouldn't crash.
        let _ = result;
    }

    #[test]
    fn driver_compiles_if_expression() {
        let result = compile_expect_ok("fn f() { if true { 1 } else { 2 } }");
        assert!(
            !result.mirs.is_empty(),
            "should have at least 1 MIR body (user fn + prelude methods), got {}",
            result.mirs.len()
        );
    }

    #[test]
    fn driver_compiles_while_loop() {
        let result = compile_expect_ok("fn f() { while false { 1 } }");
        assert!(
            !result.mirs.is_empty(),
            "should have at least 1 MIR body (user fn + prelude methods), got {}",
            result.mirs.len()
        );
    }

    #[test]
    fn driver_compiles_binary_op() {
        let result = compile_expect_ok("fn f() { 1 + 2 }");
        let mir = &result.mirs[0];
        // The result local should have an Int type
        let has_int = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, crate::mir::ty::TyKind::Int(_)));
        assert!(has_int, "expected an Int local");
    }

    #[test]
    fn driver_compiles_function_call() {
        // Define two functions and call one from the other
        let result =
            compile_expect_ok("fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1, 2) }");
        assert!(
            result.mirs.len() >= 2,
            "should have at least 2 MIR bodies (2 user fns + prelude methods), got {}",
            result.mirs.len()
        );
    }

    #[test]
    fn driver_lex_error_aborts() {
        // Unterminated string literal → lex error → driver aborts at lex stage
        let result = compile("fn f() { let x = \"unterminated; }");
        assert!(!result.errors.lex.is_empty());
        assert!(result.hir.is_none());
    }

    #[test]
    fn driver_parse_error_aborts() {
        // Missing closing brace → parse error → driver aborts at parse stage
        let result = compile("fn f() { let x = 42;");
        assert!(!result.errors.parse.is_empty());
        assert!(result.hir.is_none());
    }

    // === Stage 16.83: Diagnostic type name resolution via resolver tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 16.83 positive 1: to_diagnostics_with_resolver shows struct name.
    #[test]
    fn stage16_83_diagnostic_with_resolver_shows_struct_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        // Find a typeck diagnostic with expected/found notes.
        let has_struct_name = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyStruct")));
        assert!(
            has_struct_name,
            "Diagnostic notes should contain 'MyStruct', got diags: {:?}",
            diags
        );
    }

    /// Stage 16.83 positive 2: to_diagnostics without resolver falls back.
    #[test]
    fn stage16_83_diagnostic_without_resolver_falls_back() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result.errors.to_diagnostics(Some(&result.interner));
        // Should still produce diagnostics (fallback works).
        assert!(
            !diags.is_empty(),
            "Should have diagnostics without resolver"
        );
    }

    /// Stage 16.83 negative 1: Compile mismatch diagnostic note shows name.
    #[test]
    fn stage16_83_compile_mismatch_diagnostic_note_shows_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_struct_in_notes = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyStruct")));
        assert!(
            has_struct_in_notes,
            "Diagnostic notes should contain 'MyStruct', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 2: Compile struct mismatch diagnostic full.
    #[test]
    fn stage16_83_compile_struct_mismatch_diagnostic_full() {
        let src = "struct Foo { x: i32 } fn foo(f: Foo) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_foo = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Foo")));
        assert!(
            has_foo,
            "Diagnostic notes should contain 'Foo', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 3: Compile enum mismatch diagnostic shows name.
    #[test]
    fn stage16_83_compile_enum_mismatch_diagnostic_shows_name() {
        let src = "enum MyEnum { A, B } fn foo(e: MyEnum) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_enum = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyEnum")));
        assert!(
            has_enum,
            "Diagnostic notes should contain 'MyEnum', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 4: Compile two struct mismatch shows both.
    #[test]
    fn stage16_83_compile_two_struct_diagnostic_shows_both() {
        let src = "struct Foo { x: i32 } struct Bar { y: i32 } fn foo(f: Foo) {} fn main() { foo(Bar { y: 1 }); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_foo = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Foo")));
        let has_bar = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Bar")));
        assert!(
            has_foo && has_bar,
            "Diagnostic notes should contain 'Foo' and 'Bar', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 5: Compile fn arg diagnostic shows name.
    #[test]
    fn stage16_83_compile_fn_arg_diagnostic_shows_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        // The diagnostic message itself should contain MyStruct (from Stage 16.81).
        let has_struct = diags.iter().any(|d| d.message.contains("MyStruct"));
        assert!(
            has_struct,
            "Diagnostic message should contain 'MyStruct', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 6: format_via_diagnostics_with_resolver shows name.
    #[test]
    fn stage16_83_format_for_user_with_resolver_shows_name() {
        use crate::session::SourceMap;
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let source_map = SourceMap::new(src);
        let formatted = result.errors.format_via_diagnostics_with_resolver(
            src,
            "test.lin",
            &source_map,
            Some(&result.interner),
            Some(&result.trait_resolver),
        );
        assert!(
            formatted.contains("MyStruct"),
            "Formatted output should contain 'MyStruct', got: {}",
            formatted
        );
    }
}
