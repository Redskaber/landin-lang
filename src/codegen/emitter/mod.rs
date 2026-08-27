//! Emitter trait: abstracts the codegen backend.
//!
//! Stage 16.76 MUV-1: The original 39-method `Emitter` trait has been
//! split into 6 sub-traits per §13.4 J2 single responsibility:
//!
//! - [`ModuleEmitter`] (5 methods) — module-level globals & declarations
//! - [`FunctionEmitter`] (8 methods) — function scope & control flow
//! - [`ArithmeticEmitter`] (11 methods) — value computation from operands
//! - [`MemoryEmitter`] (6 methods) — stack allocation & pointer arithmetic
//! - [`AggregateEmitter`] (5 methods) — aggregate construction & calls
//! - [`LocalStateEmitter`] (4 methods) — local value/pointer mapping
//!
//! [`Emitter`] is now a super-trait that requires all 6 sub-traits. A
//! blanket impl preserves backward compatibility for callers that use
//! `&mut dyn Emitter` (the 20 call sites in `codegen/` keep working
//! unchanged).
//!
//! **Breaking change for implementers**: external backends that previously
//! wrote a single `impl Emitter for MyBackend` must now implement the 6
//! sub-traits separately. See `RELEASE_NOTES.md` for the migration note.
//!
//! Naming conventions (per §14 review):
//! - All IR-producing methods use `emit_` prefix
//! - State query methods use `get_` / `set_` prefix
//! - Function/block scope methods use `emit_` prefix for consistency
//! - No redundant suffixes (e.g. `_typed` when there's only one variant)
//! - `local_ptr` removed — `get_local_ptr` is the only accessor
//!
//! Stage 3.21 (v0.8.6): EmitType now carries full structure:
//! - `Struct(Vec<EmitType>)` for tuples / structs
//! - `Array(Box<EmitType>, u64)` for `[T; N]`
//! - `Ptr(Box<EmitType>)` for typed pointers (pointee tracked)
//!
//! `emit_type_to_llvm_str` (text-backend-specific) lives in
//! `text/mod.rs` since Stage 16.35 — moved out of this shared module
//! because the LLVM C-API backend uses `LLVMSysEmitter::llvm_type()`
//! (returns `LLVMTypeRef`) instead of string-based rendering.

use crate::mir::ty::ConstVal;

// ================================================================
// Sub-trait modules
// ================================================================

pub mod aggregate;
pub mod arithmetic;
pub mod function;
pub mod local_state;
pub mod memory;
pub mod module;

// Re-export all sub-traits + their methods for backward compatibility.
// Callers that previously wrote `use crate::codegen::emitter::*;`
// (text/mod.rs, llvm/mod.rs) continue to work — they pick up the 6
// sub-traits + the super-trait + the helper types/functions.
pub use aggregate::AggregateEmitter;
pub use arithmetic::ArithmeticEmitter;
pub use function::FunctionEmitter;
pub use local_state::LocalStateEmitter;
pub use memory::MemoryEmitter;
pub use module::ModuleEmitter;

// ================================================================
// EmitType + EmitValue (shared types)
// ================================================================

/// A value produced by the emitter — opaque to the translation layer.
pub type EmitValue = String;

/// The type of a value — used to select the right instruction.
///
/// Stage 3.21: made non-Copy (now carries `Vec`/`Box`) so struct and array
/// layouts can be fully represented. Callers pass `&EmitType` to emitters.
#[derive(Debug, Clone, PartialEq)]
pub enum EmitType {
    I1,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    /// Typed pointer to a pointee.
    Ptr(Box<EmitType>),
    /// Opaque pointer — no pointee info available.
    OpaquePtr,
    Void,
    /// Struct / tuple: ordered field types.
    Struct(Vec<EmitType>),
    /// Array `[T; N]`: element type + length.
    Array(Box<EmitType>, u64),
}

// Convenience constructors (keep call-sites readable).
impl EmitType {
    pub fn ptr_to(pointee: EmitType) -> Self {
        EmitType::Ptr(Box::new(pointee))
    }

    pub fn struct_of(fields: Vec<EmitType>) -> Self {
        EmitType::Struct(fields)
    }

    pub fn array_of(elem: EmitType, len: u64) -> Self {
        EmitType::Array(Box::new(elem), len)
    }

    /// Return the LLVM type used to dereference this pointer.
    /// For `Ptr(t)` returns `&t`; for `OpaquePtr` returns `i32` (legacy
    /// default — most loads in old code paths assumed i32).
    pub fn pointee(&self) -> EmitType {
        match self {
            EmitType::Ptr(t) => (**t).clone(),
            EmitType::OpaquePtr => EmitType::OpaquePtr, // Stage 14.58: ptr's pointee is still ptr
            other => other.clone(),
        }
    }

    /// True if this is a pointer (typed or opaque).
    pub fn is_ptr(&self) -> bool {
        matches!(self, EmitType::Ptr(_) | EmitType::OpaquePtr)
    }

    /// Stage 18.330 (P1 soundness fix): Compute the byte size of this type
    /// on x86-64 (System V ABI). Used to determine if a struct return value
    /// needs `sret` (structs > 16 bytes must use sret per System V ABI).
    ///
    /// **Design boundary** (per System V ABI + rustc_codegen_llvm):
    /// - Structs > 16 bytes → must use sret (hidden pointer parameter)
    /// - Structs ≤ 16 bytes → returned via registers (RAX:RDX or XMM0:XMM1)
    /// - Rust's rustc_codegen_llvm explicitly uses sret for large structs
    ///   in the IR, rather than relying on LLVM backend's auto-conversion.
    ///
    /// This is a simplified size calculation (ignores padding/alignment).
    /// For sret threshold checking, we only need to know if the struct
    /// exceeds 16 bytes — exact size with padding doesn't change the decision.
    ///
    /// Per §2.2 (根因思维): root cause — LLVM's CodeGenPrepare pass
    /// should auto-convert `ret T` to sret, but LLVM 22 has intermittent
    /// issues. Per Rust: explicit sret in IR is more reliable.
    pub fn size_bytes_x86_64(&self) -> u64 {
        match self {
            EmitType::I1 | EmitType::I8 => 1,
            EmitType::I16 => 2,
            EmitType::I32 | EmitType::F32 => 4,
            EmitType::I64 | EmitType::F64 => 8,
            EmitType::I128 => 16,
            EmitType::Ptr(_) | EmitType::OpaquePtr => 8, // 64-bit pointer
            EmitType::Void => 0,
            EmitType::Struct(fields) => {
                // Simplified: sum of field sizes (ignores padding).
                // For sret threshold, this is sufficient — a struct with
                // 3+ pointer-sized fields (e.g., { ptr, i64, i64 } = 24)
                // will exceed 16 bytes regardless of padding.
                fields.iter().map(|f| f.size_bytes_x86_64()).sum()
            }
            EmitType::Array(elem, n) => elem.size_bytes_x86_64() * n,
        }
    }

    /// Stage 18.330: Returns true if this type needs sret (struct > 16 bytes).
    /// Per System V ABI: structs > 16 bytes must be returned via sret pointer.
    pub fn needs_sret(&self) -> bool {
        self.size_bytes_x86_64() > 16
    }

    /// Stage 18.333 (P1 soundness fix): Returns true if this type needs byval
    /// when passed as a function parameter.
    ///
    /// Per System V AMD64 ABI §3.2.3: structs/arrays > 16 bytes passed as
    /// parameters must be passed via a hidden pointer parameter with the
    /// `byval` attribute (mirrors `sret` for returns).
    ///
    /// **Design boundary**:
    /// - Same threshold as `needs_sret()` (size > 16) — both are driven by
    ///   System V ABI's "size > 16 bytes → pass via pointer" rule.
    /// - The distinction between sret and byval is **semantic** (return vs
    ///   parameter), not threshold-based.
    ///
    /// Per §1.0 原則 6 (通解 > 特解): one threshold function for both sret
    /// and byval — both are "size > 16" per System V ABI.
    /// Per §1.0 原則 4 (显式 > 隐式): the threshold is explicit at IR level,
    /// not relying on LLVM's CodeGenPrepare auto-demotion (which Stage 18.332
    /// found unreliable across LLVM versions).
    /// Per §20 (iterative audit): same root cause as sret bug (Stage 18.332);
    /// byval applies the same fix pattern to the parameter side.
    pub fn needs_byval(&self) -> bool {
        self.size_bytes_x86_64() > 16
    }
}

// ================================================================
// Emitter super-trait + blanket impl
// ================================================================

/// Super-trait that combines all 6 emission sub-traits.
///
/// Stage 16.76 MUV-1: previously a single 39-method trait; now a thin
/// super-trait that requires the 6 sub-traits. The blanket impl below
/// means any type implementing all 6 sub-traits automatically implements
/// `Emitter`, so `&mut dyn Emitter` continues to work for the 20 call
/// sites in `codegen/`.
///
/// External backends: implement the 6 sub-traits individually — do NOT
/// implement `Emitter` directly (the blanket impl handles that).
pub trait Emitter:
    ModuleEmitter
    + FunctionEmitter
    + ArithmeticEmitter
    + MemoryEmitter
    + AggregateEmitter
    + LocalStateEmitter
{
}

impl<T> Emitter for T where
    T: ModuleEmitter
        + FunctionEmitter
        + ArithmeticEmitter
        + MemoryEmitter
        + AggregateEmitter
        + LocalStateEmitter
{
}

// ================================================================
// Type mapping helpers (shared between all backends)
// ================================================================

/// Map a MIR Ty to an EmitType.
///
/// Stage 3.21: now produces proper `Struct` / `Array` / `Ptr` variants
/// (was hardcoded to `Tuple` / `Array` / `Ptr` opaque).
/// Stage 3.49 (L13 closure): Construct the EmitType for a fat pointer
/// (`{ ptr_to_elem, i64 }`). Used for `&str` and `&[T]` references,
/// which carry both a data pointer and a length.
///
/// Per §15 (最优 > 最小): fat pointers are the architecturally correct
/// representation for references to unsized types (`str`, `[T]`). The
/// previous thin-pointer model (Stage 3.27/3.28) lost the length
/// component, making it impossible to recover the length of a `&str`
/// after passing it to a function — a soundness/completeness gap
/// carried as L13 debt since Stage 3.27 (18 rounds).
pub fn emit_fat_ptr_type(elem: EmitType) -> EmitType {
    EmitType::Struct(vec![EmitType::ptr_to(elem), EmitType::I64])
}

// Stage 16.35: Removed `emit_dyn_trait_ptr_type` — dead code.
// Was: `EmitType::Struct(vec![EmitType::OpaquePtr, EmitType::OpaquePtr])`.
// Never called by any codegen path. The dyn Trait fat pointer is
// constructed inline where needed via `EmitType::struct_of(...)`.
// Per §1.0 原則 5 "去除兼容思维": dead code removed.

/// Translate a MIR `Ty` to an `EmitType` (legacy fallback, no ADT layouts).
///
/// **Stage 3.65 (P2 fix)**: This is the legacy variant that does NOT have
/// access to `AdtLayouts`. For `TyKind::Adt` it falls back to `I32` (wrong
/// for any struct/enum with real payload). The canonical entry point is
/// `codegen::mir_type_to_emit_type_with_layouts` which takes `&AdtLayouts`
/// and resolves ADT layouts correctly per §16 (reads `MirBody::adt_layouts`
/// side-table — no HIR access).
///
/// **When to use which**:
/// - Inside `codegen_function` (where `MirBody` is available): always use
///   `mir_type_to_emit_type_with_layouts`.
/// - In tests / standalone helpers where no `MirBody` is available and
///   the type is known to be primitive: `mir_type_to_emit_type` is OK.
///
/// The two functions are kept separate (rather than unified with
/// `Option<&AdtLayouts>`) because the `_with_layouts` variant recurses
/// into nested ADTs/arrays/refs, while this one doesn't — unifying them
/// would require threading layouts through every recursion, which is
/// already done correctly in the `_with_layouts` variant.
pub fn mir_type_to_emit_type(ty: &crate::mir::ty::Ty) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(crate::ast::IntTy::I8) | TyKind::Uint(crate::ast::UintTy::U8) => EmitType::I8,
        TyKind::Int(crate::ast::IntTy::I16) | TyKind::Uint(crate::ast::UintTy::U16) => {
            EmitType::I16
        }
        TyKind::Int(crate::ast::IntTy::I32) | TyKind::Uint(crate::ast::UintTy::U32) => {
            EmitType::I32
        }
        TyKind::Int(crate::ast::IntTy::I64) | TyKind::Uint(crate::ast::UintTy::U64) => {
            EmitType::I64
        }
        TyKind::Int(crate::ast::IntTy::I128) | TyKind::Uint(crate::ast::UintTy::U128) => {
            EmitType::I128
        }
        TyKind::Int(crate::ast::IntTy::Isize) | TyKind::Uint(crate::ast::UintTy::Usize) => {
            EmitType::I64
        }
        // All explicit IntTy/UintTy variants are covered above.
        // This catch-all is unreachable but kept for safety.
        #[allow(unreachable_patterns)]
        TyKind::Int(_) | TyKind::Uint(_) => EmitType::I32,
        TyKind::Bool => EmitType::I1,
        TyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        TyKind::Float(_) => EmitType::F64,
        TyKind::Char => EmitType::I8,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 3.49 (L13 closure): `&str` and `&[T]` are fat pointers
            // `{ ptr, len }`. Other references remain thin pointers.
            match &inner.kind {
                TyKind::Str => emit_fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => emit_fat_ptr_type(mir_type_to_emit_type(elem)),
                _ => EmitType::ptr_to(mir_type_to_emit_type(inner)),
            }
        }
        // Stage 3.49: `Str` and `Slice(T)` are unsized types — they cannot
        // appear as values, only behind a reference (`&str`, `&[T]`). We
        // keep their direct EmitType as thin pointers for internal use
        // (e.g., global string types), but the reference type is now a
        // fat pointer (see the Ref case above).
        TyKind::Str => EmitType::ptr_to(EmitType::I8),
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type(elem)),
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Struct(tys.iter().map(mir_type_to_emit_type).collect())
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            EmitType::array_of(mir_type_to_emit_type(elem), n)
        }
        // Stage 4.7 (L3): Closure type — emit as a struct with captured fields.
        // The substs vector carries the capture field types.
        TyKind::Closure(_, substs) => {
            let fields: Vec<EmitType> = substs.iter().map(mir_type_to_emit_type).collect();
            EmitType::Struct(fields)
        }
        // Stage 14.57: Function pointer and function definition types — emit as
        // opaque pointer (function reference). Was: fell through to I32, causing
        // fn pointer params to be treated as i32 — function refs passed as `0`.
        TyKind::FnPtr(_) | TyKind::FnDef(_, _) => EmitType::OpaquePtr,
        // ADTs and other complex types — Stage 3 treats as opaque i32 placeholder.
        _ => EmitType::I32,
    }
}

// Stage 16.35: Removed `binop_to_llvm_str`, `emit_type_to_llvm_str`,
// `llvm_ptr_str` — text-backend-specific functions moved to
// `text/mod.rs`. The LLVM C-API backend uses `LLVMSysEmitter::llvm_type()`
// (returns `LLVMTypeRef`) and `LLVMBuildAdd` etc. directly, not strings.
//
// Per §1.0 原則 5 "去除兼容思维": text utilities removed from shared module.
// Per §1.0 原則 6 "通用 > 特例": each backend owns its own rendering logic.
// Per §23 rule 5 (DRY): no duplicate type-rendering logic in shared module.
//
// `llvm_ptr_str` was dead code (never called) — deleted entirely.

// ================================================================
// Tests
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::TextEmitter;

    /// Stage 3.57 (P1-5): Verify TextEmitter implements the Emitter trait.
    /// This is a compile-time check — if Emitter trait changes and
    /// TextEmitter doesn't keep up, this test fails to compile.
    #[test]
    fn text_emitter_satisfies_emitter_trait() {
        let _: &dyn Emitter = &TextEmitter::new();
    }

    // Stage 16.35: `emit_type_to_llvm_str_roundtrips` test moved to
    // `text/mod.rs` (the function now lives there).

    /// Stage 3.57: Verify emit_fat_ptr_type produces the correct { Ptr, I64 } shape.
    #[test]
    fn fat_ptr_type_correct_shape() {
        let fp = emit_fat_ptr_type(EmitType::I8);
        match fp {
            EmitType::Struct(fields) => {
                assert_eq!(fields.len(), 2);
                assert!(fields[0].is_ptr());
                assert_eq!(fields[1], EmitType::I64);
            }
            _ => panic!("expected Struct, got {:?}", fp),
        }
    }

    /// Stage 3.57: Verify mir_type_to_emit_type for basic types.
    #[test]
    fn mir_type_to_emit_type_correct() {
        let i32_ty = crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
            crate::session::Span::DUMMY,
        );
        assert_eq!(mir_type_to_emit_type(&i32_ty), EmitType::I32);

        let f64_ty = crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Float(crate::ast::FloatTy::F64),
            crate::session::Span::DUMMY,
        );
        assert_eq!(mir_type_to_emit_type(&f64_ty), EmitType::F64);
    }

    /// Stage 3.57: Verify EmitType helper methods (ptr_to, pointee, is_ptr).
    #[test]
    fn emit_type_helpers() {
        let ptr = EmitType::ptr_to(EmitType::I32);
        assert!(ptr.is_ptr());
        assert_eq!(ptr.pointee(), EmitType::I32);

        let struct_ty = EmitType::struct_of(vec![EmitType::I32, EmitType::I64]);
        assert!(!struct_ty.is_ptr());

        let arr = EmitType::array_of(EmitType::I8, 10);
        match arr {
            EmitType::Array(elem, len) => {
                assert_eq!(*elem, EmitType::I8);
                assert_eq!(len, 10);
            }
            _ => panic!("expected Array"),
        }
    }

    /// Stage 3.57: Verify TextEmitter produces non-empty output.
    #[test]
    fn text_emitter_produces_output() {
        let mut emitter = TextEmitter::new();
        emitter.emit_header();
        emitter.emit_declare("void @test()");
        let output = emitter.output_with_globals();
        assert!(!output.is_empty());
        assert!(output.contains("target triple"));
    }
}

// ================================================================
// Stage 16.76 MUV-1: Sub-trait satisfaction tests (compile-time)
// ================================================================
//
// These tests verify that each backend implements all 6 sub-traits (and
// thus `Emitter` via the blanket impl). If a sub-trait method is removed
// or renamed, the corresponding test fails to compile.
//
// 6 sub-traits × 2 backends = 12 type assertions + 2 super-trait
// assertions = 14 compile-time checks total.

#[cfg(test)]
mod trait_satisfaction_tests {
    use super::*;
    use crate::codegen::TextEmitter;

    #[cfg(feature = "llvm-backend")]
    use crate::codegen::llvm::LLVMSysEmitter;

    #[test]
    fn text_emitter_satisfies_all_sub_traits() {
        let _: &dyn ModuleEmitter = &TextEmitter::new();
        let _: &dyn FunctionEmitter = &TextEmitter::new();
        let _: &dyn ArithmeticEmitter = &TextEmitter::new();
        let _: &dyn MemoryEmitter = &TextEmitter::new();
        let _: &dyn AggregateEmitter = &TextEmitter::new();
        let _: &dyn LocalStateEmitter = &TextEmitter::new();
        let _: &dyn Emitter = &TextEmitter::new();
    }

    #[cfg(feature = "llvm-backend")]
    #[test]
    fn llvm_emitter_satisfies_all_sub_traits() {
        let _: &dyn ModuleEmitter = &LLVMSysEmitter::new();
        let _: &dyn FunctionEmitter = &LLVMSysEmitter::new();
        let _: &dyn ArithmeticEmitter = &LLVMSysEmitter::new();
        let _: &dyn MemoryEmitter = &LLVMSysEmitter::new();
        let _: &dyn AggregateEmitter = &LLVMSysEmitter::new();
        let _: &dyn LocalStateEmitter = &LLVMSysEmitter::new();
        let _: &dyn Emitter = &LLVMSysEmitter::new();
    }
}
