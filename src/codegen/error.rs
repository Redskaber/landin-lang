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

    /// Stage 20.1 (v0.5 CodegenError P1): Create a new CodegenError with
    /// a specific kind.
    ///
    /// Per §1.0 原則 3 (显式 > 隐式): explicit kind for diagnostic categorization.
    /// Per §23: `with_kind` follows `<verb>_<noun>` pattern.
    pub fn with_kind(message: impl Into<String>, span: Span, kind: CodegenErrorKind) -> Self {
        Self {
            message: message.into(),
            span,
            kind,
        }
    }

    /// Stage 20.1 (v0.5 CodegenError P1): Create an UnresolvedType error.
    ///
    /// Per §1.0 原則 4 (报错 > 静默): unresolved types must be reported,
    /// not silently mapped to I32.
    /// Per §1.0 原則 6 (通解 > 特解): one constructor for all unresolved type
    /// categories (Param, Infer, Error, Projection, Never, Foreign, Adt).
    pub fn unresolved_type(ty_kind_debug: impl std::fmt::Debug, span: Span) -> Self {
        Self::with_kind(
            format!(
                "unresolved type kind `{:?}` reached codegen — \
                caller should migrate to mir_type_to_emit_type_checked or layouts variant",
                ty_kind_debug
            ),
            span,
            CodegenErrorKind::UnresolvedType,
        )
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

    // =====================================================================
    // Stage 20.1 (v0.5 CodegenError P1) tests
    // =====================================================================

    /// Stage 20.1 positive 1: with_kind creates error with specific kind.
    #[test]
    fn stage20_01_with_kind_creates_error_with_kind() {
        let err = CodegenError::with_kind(
            "test message",
            Span::DUMMY,
            CodegenErrorKind::UnresolvedType,
        );
        assert_eq!(err.message, "test message");
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
    }

    /// Stage 20.1 positive 2: unresolved_type creates UnresolvedType error.
    #[test]
    fn stage20_01_unresolved_type_creates_correct_kind() {
        use crate::mir::ty::{InferVar, Ty, TyKind, TyVid};
        let ty = Ty::from_kind(TyKind::Infer(InferVar::TyVar(TyVid(0))));
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("unresolved type kind"));
        assert!(err.message.contains("Infer"));
    }

    /// Stage 20.1 positive 3: unresolved_type for Param kind.
    #[test]
    fn stage20_01_unresolved_type_for_param() {
        use crate::mir::ty::{ParamTy, Ty, TyKind};
        use lasso::Rodeo;
        thread_local! {
            static TEST_RODEO: std::cell::RefCell<Rodeo> = std::cell::RefCell::new(Rodeo::new());
        }
        let spur = TEST_RODEO.with(|r| r.borrow_mut().get_or_intern("T"));
        let ty = Ty::from_kind(TyKind::Param(ParamTy {
            index: 0,
            name: spur,
        }));
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Param"));
    }

    /// Stage 20.1 positive 4: unresolved_type for Error kind.
    #[test]
    fn stage20_01_unresolved_type_for_error() {
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Error);
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Error"));
    }

    /// Stage 20.1 positive 5: new creates Generic kind (default).
    #[test]
    fn stage20_01_new_creates_generic_kind() {
        let err = CodegenError::new("test", Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::Generic);
    }

    /// Stage 20.1 positive 6: with_kind for LlvmVerification kind.
    #[test]
    fn stage20_01_with_kind_llvm_verification() {
        let err = CodegenError::with_kind(
            "verification failed",
            Span::DUMMY,
            CodegenErrorKind::LlvmVerification,
        );
        assert_eq!(err.kind, CodegenErrorKind::LlvmVerification);
        assert_eq!(err.message, "verification failed");
    }

    /// Stage 20.1 positive 7: with_kind for LlvmTargetMachine kind.
    #[test]
    fn stage20_01_with_kind_llvm_target_machine() {
        let err = CodegenError::with_kind(
            "target machine creation failed",
            Span::DUMMY,
            CodegenErrorKind::LlvmTargetMachine,
        );
        assert_eq!(err.kind, CodegenErrorKind::LlvmTargetMachine);
    }

    /// Stage 20.1 positive 8: with_kind for LlvmEmission kind.
    #[test]
    fn stage20_01_with_kind_llvm_emission() {
        let err = CodegenError::with_kind(
            "emission failed",
            Span::DUMMY,
            CodegenErrorKind::LlvmEmission,
        );
        assert_eq!(err.kind, CodegenErrorKind::LlvmEmission);
    }

    /// Stage 20.1 positive 9: with_kind for InvalidString kind.
    #[test]
    fn stage20_01_with_kind_invalid_string() {
        let err = CodegenError::with_kind(
            "invalid string",
            Span::DUMMY,
            CodegenErrorKind::InvalidString,
        );
        assert_eq!(err.kind, CodegenErrorKind::InvalidString);
    }

    /// Stage 20.1 negative 1: unresolved_type message contains diagnostic info.
    #[test]
    fn stage20_01_unresolved_type_message_has_diagnostic() {
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Error);
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        // Message should contain "unresolved type kind" + "Error" + migration hint.
        assert!(err.message.contains("unresolved type kind"));
        assert!(err.message.contains("Error"));
        assert!(err.message.contains("mir_type_to_emit_type_checked"));
    }

    /// Stage 20.1 negative 2: with_kind with empty message.
    #[test]
    fn stage20_01_with_kind_empty_message() {
        let err = CodegenError::with_kind("", Span::DUMMY, CodegenErrorKind::Generic);
        assert_eq!(err.message, "");
        assert_eq!(err.kind, CodegenErrorKind::Generic);
    }

    /// Stage 20.1 negative 3: Display shows message + span.
    #[test]
    fn stage20_01_display_shows_message_and_span() {
        let err =
            CodegenError::with_kind("test error", Span::DUMMY, CodegenErrorKind::UnresolvedType);
        let display = format!("{}", err);
        assert!(display.contains("test error"));
    }

    /// Stage 20.1 negative 4: CodegenErrorKind equality.
    #[test]
    fn stage20_01_codegen_error_kind_equality() {
        assert_eq!(CodegenErrorKind::Generic, CodegenErrorKind::Generic);
        assert_ne!(CodegenErrorKind::Generic, CodegenErrorKind::UnresolvedType);
        assert_ne!(
            CodegenErrorKind::LlvmVerification,
            CodegenErrorKind::LlvmEmission
        );
    }

    /// Stage 20.1 negative 5: unresolved_type for Projection kind.
    #[test]
    fn stage20_01_unresolved_type_for_projection() {
        use crate::hir::DefId;
        use crate::mir::ty::{Ty, TyKind};
        use std::rc::Rc;
        let ty = Ty::from_kind(TyKind::Projection(DefId::new(0), Rc::from([])));
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Projection"));
    }

    /// Stage 20.1 negative 6: unresolved_type for Never kind.
    #[test]
    fn stage20_01_unresolved_type_for_never() {
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Never);
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Never"));
    }

    /// Stage 20.1 negative 7: unresolved_type for Foreign kind.
    #[test]
    fn stage20_01_unresolved_type_for_foreign() {
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Foreign);
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Foreign"));
    }

    /// Stage 20.1 negative 8: unresolved_type for Adt kind.
    #[test]
    fn stage20_01_unresolved_type_for_adt() {
        use crate::hir::DefId;
        use crate::mir::ty::{Ty, TyKind};
        use std::rc::Rc;
        let ty = Ty::from_kind(TyKind::Adt(DefId::new(0), Rc::from([])));
        let err = CodegenError::unresolved_type(&ty.kind, Span::DUMMY);
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
        assert!(err.message.contains("Adt"));
    }

    /// Stage 20.1 integration 1: mir_type_to_emit_type_checked returns Ok for i32.
    #[test]
    fn stage20_01_checked_returns_ok_for_i32() {
        use crate::ast::IntTy;
        use crate::codegen::emitter::mir_type_to_emit_type_checked;
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Int(IntTy::I32));
        let result = mir_type_to_emit_type_checked(&ty);
        assert!(result.is_ok());
    }

    /// Stage 20.1 integration 2: mir_type_to_emit_type_checked returns Err for Infer.
    #[test]
    fn stage20_01_checked_returns_err_for_infer() {
        use crate::codegen::emitter::mir_type_to_emit_type_checked;
        use crate::mir::ty::{InferVar, Ty, TyKind, TyVid};
        let ty = Ty::from_kind(TyKind::Infer(InferVar::TyVar(TyVid(0))));
        let result = mir_type_to_emit_type_checked(&ty);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, CodegenErrorKind::UnresolvedType);
    }

    /// Stage 20.1 integration 3: mir_type_to_emit_type_checked returns Err for Param.
    #[test]
    fn stage20_01_checked_returns_err_for_param() {
        use crate::codegen::emitter::mir_type_to_emit_type_checked;
        use crate::mir::ty::{ParamTy, Ty, TyKind};
        use lasso::Rodeo;
        thread_local! {
            static TEST_RODEO: std::cell::RefCell<Rodeo> = std::cell::RefCell::new(Rodeo::new());
        }
        let spur = TEST_RODEO.with(|r| r.borrow_mut().get_or_intern("T"));
        let ty = Ty::from_kind(TyKind::Param(ParamTy {
            index: 0,
            name: spur,
        }));
        let result = mir_type_to_emit_type_checked(&ty);
        assert!(result.is_err());
    }

    /// Stage 20.1 integration 4: mir_type_to_emit_type_checked returns Err for Error.
    #[test]
    fn stage20_01_checked_returns_err_for_error() {
        use crate::codegen::emitter::mir_type_to_emit_type_checked;
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Error);
        let result = mir_type_to_emit_type_checked(&ty);
        assert!(result.is_err());
    }

    /// Stage 20.1 integration 5: mir_type_to_emit_type_checked returns Ok for Bool.
    #[test]
    fn stage20_01_checked_returns_ok_for_bool() {
        use crate::codegen::emitter::mir_type_to_emit_type_checked;
        use crate::mir::ty::{Ty, TyKind};
        let ty = Ty::from_kind(TyKind::Bool);
        let result = mir_type_to_emit_type_checked(&ty);
        assert!(result.is_ok());
    }
}
