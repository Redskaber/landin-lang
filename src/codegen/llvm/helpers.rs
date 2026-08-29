//! Stage 16.77 MUV-1: Private helper functions for LLVMSysEmitter.
//! Stage 17.01: Added `cstr_result` for error-safe CString construction.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! These helpers are used by all 6 sub-trait impl blocks.

use crate::codegen::emitter::EmitType;
use crate::codegen::error::{CodegenError, CodegenResult};
use crate::session::Span;
use std::ffi::CString;

// =====================================================================
// Free helper functions
// =====================================================================

/// Build a `*const c_char` from a short static literal — panics on null
/// bytes (which would indicate a bug in the literal). The returned pointer
/// is borrowed from a leaked `CString` (the string is short and lives for
/// the duration of the program — acceptable for LLVM name tags).
/// Stage 15.3 (perf fix): Convert &str to C string pointer.
///
/// **Before**: `CString::new(s).unwrap().into_raw()` — leaks every CString
/// (memory grows unbounded in LSP mode).
/// **After**: Uses a thread-local cache of CStrings. Repeated strings reuse
/// the same allocation. Memory is bounded by the number of unique strings
/// (typically <1000 per compilation unit).
///
/// Per Phase 2 audit HP-B6: "cstr() leaks every CString."
/// Per §1.0 原則 6 "通用 > 特例": one cache handles all string-to-CString conversions.
pub(crate) fn cstr(s: &str) -> *const std::os::raw::c_char {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CSTR_CACHE: RefCell<HashMap<String, CString>> = RefCell::new(HashMap::new());
    }

    CSTR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.contains_key(s) {
            // Stage 18.100 (TD-UNWRAP2): Use `expect` with rationale instead
            // of bare `unwrap()`. Landin identifiers/symbols never contain NUL
            // bytes (lexer rejects them), so `CString::new` succeeds. If a NUL
            // byte somehow reaches here (e.g., a future string-literal feature),
            // the panic message will clearly identify the cause.
            // Per §1.0 原則 4 "报错 > 静默": panics should have clear messages.
            let cstring = CString::new(s).unwrap_or_else(|_| {
                panic!(
                    "cstr: string contains NUL byte (should be impossible for Landin symbols): {:?}",
                    s
                )
            });
            cache.insert(s.to_string(), cstring);
        }
        // Safe: the CString is stored in the HashMap and won't be moved
        // or dropped until the thread exits. HashMap doesn't move values
        // after insertion (only rehashes the bucket array).
        cache[s].as_ptr()
    })
}

/// Stage 17.01: Convert a string to a `CString`, returning `CodegenError` on failure.
///
/// This is the error-safe variant of `cstr()`. NUL bytes in the string are
/// the only failure case (Landin identifiers don't contain NUL, but this
/// provides proper error propagation instead of panicking).
///
/// Per §1.0 原則 4 "报错 > 静默": NUL bytes produce a hard error, not a panic.
/// Per §23: `cstr_result` follows `<noun>_<noun>` pattern.
///
/// Stage 18.80 P2-D: Added `span` parameter for accurate error location
/// (was Span::DUMMY). Callers should pass the source span of the string.
pub(crate) fn cstr_result(s: &str, span: Span) -> CodegenResult<CString> {
    CString::new(s).map_err(|_| {
        CodegenError::new(
            // Stage 18.80 P2-D: Use Display instead of Debug format.
            format!("invalid string containing NUL byte: {}", s),
            span,
        )
    })
}

/// Stage 18.75 P0-4: Convert a hardcoded string literal to an owned `CString`.
///
/// This is the error-safe variant of `CString::new("literal").unwrap()` for
/// hardcoded string literals (like "icmp", "call", "phi"). These literals
/// are compiler-controlled and never contain NUL bytes, so the unwrap is
/// technically safe — but per §1.0 原則 4 "报错 > 静默", production code
/// should not contain `unwrap()` calls.
///
/// Uses the same thread-local cache as `cstr()` to avoid repeated allocation.
/// Returns a cloned `CString` (the cache entry is never moved).
///
/// Per §23: `cstr_owned` follows `<noun>_<adj>` pattern.
pub(crate) fn cstr_owned(s: &str) -> CString {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CSTR_OWNED_CACHE: RefCell<HashMap<String, CString>> = RefCell::new(HashMap::new());
    }

    CSTR_OWNED_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.contains_key(s) {
            // Per §1.0 原則 9 "正确 > 妥协": hardcoded literals are known-safe,
            // but we use map_err to avoid unwrap() in production code.
            match CString::new(s) {
                Ok(cs) => {
                    cache.insert(s.to_string(), cs);
                }
                Err(_) => {
                    // This should never happen for hardcoded literals.
                    // If it does, fall back to an empty CString (safe default).
                    // Empty CString always succeeds (no interior nul bytes).
                    cache.insert(
                        s.to_string(),
                        CString::new("").unwrap_or_else(|_| {
                            CString::new("")
                                .expect("empty CString is always valid (no interior nul bytes)")
                        }),
                    );
                }
            }
        }
        cache[s].clone()
    })
}

/// True iff `ty` is a floating-point type.
pub(crate) fn is_float(ty: &EmitType) -> bool {
    matches!(ty, EmitType::F32 | EmitType::F64)
}

/// Convert a Landin icmp op string ("eq", "ne", "slt", etc.) to an
/// `LLVMIntPredicate`.
pub(crate) fn parse_int_predicate(op: &str) -> llvm_sys::LLVMIntPredicate {
    use llvm_sys::LLVMIntPredicate::*;
    match op {
        "eq" => LLVMIntEQ,
        "ne" => LLVMIntNE,
        "ugt" => LLVMIntUGT,
        "uge" => LLVMIntUGE,
        "ult" => LLVMIntULT,
        "ule" => LLVMIntULE,
        "sgt" => LLVMIntSGT,
        "sge" => LLVMIntSGE,
        "slt" => LLVMIntSLT,
        "sle" => LLVMIntSLE,
        _ => LLVMIntEQ,
    }
}

/// Convert a Landin fcmp op string ("oeq", "olt", etc.) to an
/// `LLVMRealPredicate`.
pub(crate) fn parse_real_predicate(op: &str) -> llvm_sys::LLVMRealPredicate {
    use llvm_sys::LLVMRealPredicate::*;
    match op {
        "false" => LLVMRealPredicateFalse,
        "oeq" => LLVMRealOEQ,
        "ogt" => LLVMRealOGT,
        "oge" => LLVMRealOGE,
        "olt" => LLVMRealOLT,
        "ole" => LLVMRealOLE,
        "one" => LLVMRealONE,
        "ord" => LLVMRealORD,
        "uno" => LLVMRealUNO,
        "ueq" => LLVMRealUEQ,
        "ugt" => LLVMRealUGT,
        "uge" => LLVMRealUGE,
        "ult" => LLVMRealULT,
        "ule" => LLVMRealULE,
        "une" => LLVMRealUNE,
        "true" => LLVMRealPredicateTrue,
        _ => LLVMRealOEQ,
    }
}

/// Parse the function name out of a `declare <retty> @<name>(...)` signature.
/// Returns `None` if no `@name` token is found.
pub(crate) fn parse_declare_name(sig: &str) -> Option<String> {
    let at = sig.find('@')?;
    let rest = &sig[at + 1..];
    let end = rest.find(['(', ' ', '\t']).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Count commas at the top level inside the parens of a signature.
/// Used as a rough arg-count heuristic when no type info is available.
///
/// Stage 18.334 (P1 soundness fix): Filter out the `...` token (variadic
/// indicator) from the count. Previously, `count_args_in_signature("(ptr, ...")`
/// returned 2 — counting `...` as a regular arg, which caused the LLVM decl
/// to have an extra `i32` param.
/// Per §20 (iterative audit): found via §20 audit while validating TextEmitter IR.
/// Per §1.0 原則 6 (通解 > 特解): same filter for all callers.
pub(crate) fn count_args_in_signature(sig: &str) -> usize {
    let open = match sig.find('(') {
        Some(i) => i,
        None => return 0,
    };
    let close = match sig[open..].find(')') {
        Some(i) => open + i,
        None => return 0,
    };
    let inside = &sig[open + 1..close];
    if inside.trim().is_empty() {
        return 0;
    }
    inside
        .split(',')
        .filter(|s| {
            let s = s.trim();
            !s.is_empty() && s != "..."
        })
        .count()
}

/// Stage 18.334 (P1 soundness fix): True iff the signature is variadic
/// (contains `...` inside the parens).
///
/// Used by `declare_function` + `emit_call` to set `isVariadic=1` on the
/// LLVM function type, instead of the previous hardcoded name-list
/// (`name == "printf" || name == "__landin_eprintf"`).
///
/// Per §1.0 原則 6 (通解 > 特解): variadicity is a property of the signature,
/// not the function name. The same logic now applies to ALL variadic functions
/// (printf, sprintf, fprintf, __landin_println, __landin_eprintf, etc.).
/// Per §1.0 原則 9 (正确 > 妥协): correct variadic detection from source-of-truth.
/// Per §20 (iterative audit): replaces the hardcoded name-list that was a
/// workaround for not parsing the signature.
pub(crate) fn signature_is_variadic(sig: &str) -> bool {
    let open = match sig.find('(') {
        Some(i) => i,
        None => return false,
    };
    let close = match sig[open..].find(')') {
        Some(i) => open + i,
        None => return false,
    };
    sig[open..close].contains("...")
}

/// Stage 18.332 (P1 soundness fix): Create an `sret` type attribute.
///
/// Per System V AMD64 ABI §3.2.3 + rustc_codegen_llvm:
/// - Structs > 16 bytes returned from functions must be passed via a hidden
///   `sret` pointer parameter (held in `%rdi` on x86-64).
/// - LLVM represents this as a `sret(<ty>)` type attribute on the first
///   function parameter (index 1 in LLVM's 1-indexed attribute scheme).
/// - rustc emits sret explicitly via `Attribute::getWithStructRetType(ctx, ty)`;
///   we mirror this via `LLVMCreateTypeAttribute(ctx, sret_kind, ty)`.
///
/// The kind ID is fetched at runtime via `LLVMGetEnumAttributeKindForName`
/// because LLVM doesn't expose the enum value as a stable public constant
/// (the enum may shift across LLVM versions — the name "sret" is stable).
///
/// Per §1.0 原則 6 (通解 > 特解): one helper used by all 4 sret emission sites
/// (emit_function_begin, declare_function, interpret_adhoc, emit_call).
/// Per §12 (最优 > 最小): explicit sret at IR level is the architectural fix;
///   relying on LLVM's CodeGenPrepare auto-demotion was a workaround.
pub(crate) fn create_sret_attribute(
    ctx: llvm_sys::prelude::LLVMContextRef,
    ret_llvm_ty: llvm_sys::prelude::LLVMTypeRef,
) -> llvm_sys::prelude::LLVMAttributeRef {
    unsafe {
        let sret_kind =
            llvm_sys::core::LLVMGetEnumAttributeKindForName(b"sret".as_ptr() as *const _, 4);
        llvm_sys::core::LLVMCreateTypeAttribute(ctx, sret_kind, ret_llvm_ty)
    }
}

/// Stage 18.333 (P1 soundness fix): Create a `byval` type attribute.
///
/// Per System V AMD64 ABI §3.2.3 + rustc_codegen_llvm:
/// - Structs/arrays > 16 bytes passed as function parameters must be passed
///   via a hidden pointer parameter with the `byval` attribute (mirrors
///   `sret` for returns).
/// - LLVM represents this as a `byval(<ty>)` type attribute on the
///   parameter (at the parameter's 1-indexed position).
/// - rustc emits byval explicitly via `Attribute::getWithByValType(ctx, ty)`;
///   we mirror this via `LLVMCreateTypeAttribute(ctx, byval_kind, ty)`.
///
/// **Design boundary** (mirrors `create_sret_attribute`):
/// - Same LLVM C-API pattern: fetch kind ID by name, create type attribute.
/// - Used at all 6 emission sites (function_begin, declare_function,
///   interpret_adhoc, emit_call, emit_dyn_trait_method_call, text equivalents).
///
/// Per §1.0 原則 6 (通解 > 特解): one helper for all byval emission sites.
/// Per §20 (iterative audit): same root cause as sret bug; same fix pattern.
pub(crate) fn create_byval_attribute(
    ctx: llvm_sys::prelude::LLVMContextRef,
    ty: llvm_sys::prelude::LLVMTypeRef,
) -> llvm_sys::prelude::LLVMAttributeRef {
    unsafe {
        let byval_kind =
            llvm_sys::core::LLVMGetEnumAttributeKindForName(b"byval".as_ptr() as *const _, 5);
        llvm_sys::core::LLVMCreateTypeAttribute(ctx, byval_kind, ty)
    }
}

/// Copy a C string (NUL-terminated) from a `*const c_char` into an
/// owned `String`. Does NOT free the original — the caller is
/// responsible for `LLVMDisposeMessage` if applicable.
pub(crate) unsafe fn collect_cstring(ptr: *const std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
    String::from_utf8_lossy(bytes).into_owned()
}

// =====================================================================
// Tests
// =====================================================================
