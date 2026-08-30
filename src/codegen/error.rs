//! Stage 17.01: CodegenError error type.
//!
//! Provides `CodegenError` and `CodegenResult<T>` for codegen error propagation.
//! Currently, codegen uses `unwrap()` for CString construction and LLVM C-API
//! calls. This module introduces a proper error type that can be propagated
//! via `?` operator instead of panicking.
//!
//! Per §10.1.8: error types use `Error` suffix with `{ message: String, span: Span }` minimal form.
//! Per §1.0 原則 4 "报错 > 静默": codegen errors are reported, not silently ignored.
//! Per §1.0 原則 5 "去除兼容思维": replaces panic-prone unwrap() paths.

use crate::session::Span;

/// A codegen error encountered during LLVM IR generation or object file emission.
///
/// Per §10.1.8: `{ message: String, span: Span }` minimal form.
/// Per §23: `CodegenError` follows `<Noun>_<Noun>` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenErrorKind {
    Generic,
    LlvmVerification,
    LlvmTargetMachine,
    LlvmEmission,
    InvalidString,
    /// Stage 18.438 (v0.5+ Phase 5 Step 1): Unresolved MIR type kind
    /// (Param, Infer, Error, Projection) reached codegen.
    ///
    /// Per §1.0 原則 4 (报错 > 静默): codegen should report unresolved
    /// types, not silently fall back to EmitType::I32.
    /// Per §1.0 原則 6 (通解 > 特解): one error kind for all unresolved
    /// type categories.
    UnresolvedType,
}

#[derive(Debug, Clone)]
pub struct CodegenError {
    pub message: String,
    pub span: Span,
    pub kind: CodegenErrorKind,
}

impl CodegenError {
    /// Create a new CodegenError with a message and span.
    ///
    /// Per §23: `new` follows `<verb>` pattern.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind: CodegenErrorKind::Generic,
        }
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {} at {}", self.message, self.span)
    }
}

impl std::error::Error for CodegenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// A `Result` alias for codegen operations that may fail.
///
/// Per §23: `CodegenResult` follows `<Noun>_<Noun>` pattern.
pub type CodegenResult<T> = Result<T, CodegenError>;

// Stage 18.83: Tests use cstr_result from llvm::helpers, which requires
// the llvm-backend feature. Gate the entire test module.
#[cfg(all(test, feature = "llvm-backend"))]
mod tests {
    use super::*;

    /// Stage 17.01 positive 1: CodegenError::new creates a valid error.
    #[test]
    fn stage17_01_codegen_error_new_creates_error() {
        let error = CodegenError::new("test error", Span::DUMMY);
        assert_eq!(error.message, "test error");
        assert_eq!(error.span, Span::DUMMY);
    }

    /// Stage 17.01 positive 2: cstr_result helper for valid string returns Ok.
    #[test]
    fn stage17_01_cstr_valid_string_returns_ok() {
        use crate::codegen::llvm::helpers::cstr_result;
        let result = cstr_result("hello", Span::DUMMY);
        assert!(result.is_ok());
        let c_string = result.expect("should be Ok");
        assert_eq!(c_string.to_str().expect("valid UTF-8"), "hello");
    }

    /// Stage 17.01 negative 1: cstr_result helper for NUL byte returns Err.
    #[test]
    fn stage17_01_cstr_nul_byte_returns_error() {
        use crate::codegen::llvm::helpers::cstr_result;
        let result = cstr_result("hello\0world", Span::DUMMY);
        let error = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err for NUL byte"),
        };
        assert!(
            error.message.contains("NUL"),
            "Error should mention NUL, got: {}",
            error.message
        );
    }

    /// Stage 17.01 negative 2: CodegenError message is correct.
    #[test]
    fn stage17_01_codegen_error_message_correct() {
        let error = CodegenError::new("specific error message", Span::DUMMY);
        assert_eq!(error.message, "specific error message");
    }

    /// Stage 17.01 negative 3: CodegenError span is correct.
    #[test]
    fn stage17_01_codegen_error_span_correct() {
        let span = Span::new(10, 20);
        let error = CodegenError::new("test", span);
        assert_eq!(error.span, span);
    }

    /// Stage 17.01 negative 4: CodegenResult Ok variant.
    #[test]
    fn stage17_01_codegen_result_ok_variant() {
        let result: CodegenResult<i32> = Ok(42);
        match result {
            Ok(v) => assert_eq!(v, 42),
            Err(_) => panic!("expected Ok"),
        }
    }

    /// Stage 17.01 negative 5: CodegenResult Err variant.
    #[test]
    fn stage17_01_codegen_result_err_variant() {
        let result: CodegenResult<i32> = Err(CodegenError::new("failure", Span::DUMMY));
        match result {
            Ok(_) => panic!("expected Err"),
            Err(e) => assert_eq!(e.message, "failure"),
        }
    }

    /// Stage 17.01 negative 6: cstr_result empty string returns Ok.
    #[test]
    fn stage17_01_cstr_empty_string_returns_ok() {
        use crate::codegen::llvm::helpers::cstr_result;
        let result = cstr_result("", Span::DUMMY);
        let c_string = result.expect("empty string should be Ok");
        assert_eq!(c_string.to_str().expect("valid UTF-8"), "");
    }
}
