//! Emitter trait: abstracts the codegen backend.
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
//! `emit_type_to_llvm_str` returns `String` (was `&'static str`) so that
//! dynamic struct/array layouts can be rendered.

use crate::mir::place::{BinOp, UnOp};
use crate::mir::ty::ConstVal;

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
}

/// Abstract emitter trait.
///
/// Naming conventions:
/// - `emit_*`: produces IR instructions, may return EmitValue
/// - `get_*` / `set_*`: queries or updates emitter state
///
/// Stage 16.36: Removed `emit_output` (dead code). The trait methods are
/// organized into clear documentation groups:
/// - Module-level: header, declares, globals (survive across functions)
/// - Function scope: instructions, control flow (between begin/end)
/// - Local state: set/get local pointers and values
///
/// Per §1.0 原則 5 "去除兼容思维": dead `emit_output` removed.
/// Per §23: clear documentation grouping.
pub trait Emitter {
    // === Module-level ===

    /// Emit module header (target triple, datalayout).
    fn emit_header(&mut self);

    /// Emit an external function declaration.
    fn emit_declare(&mut self, signature: &str);

    /// Emit (or look up) a module-level string constant global.
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;

    /// Emit a vtable as a module-level constant global.
    fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue;

    /// Emit a `dyn Trait` fat-pointer constant global.
    fn emit_dyn_trait_const(
        &mut self,
        global_name: &str,
        data_symbol: &str,
        vtable_symbol: &str,
    ) -> EmitValue;

    // === Function scope ===

    /// Begin a new function definition.
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType);

    /// End the current function definition.
    fn emit_function_end(&mut self);

    /// Emit a constant value and return its handle.
    fn emit_const(&mut self, val: &ConstVal) -> EmitValue;

    /// Emit a binary operation and return the result value.
    fn emit_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    /// Emit a unary operation and return the result value.
    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue;

    /// Emit a return instruction.
    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>);

    /// Emit an unreachable instruction.
    fn emit_unreachable(&mut self);

    /// Emit an unconditional branch to a label.
    fn emit_br(&mut self, label: &str);

    /// Emit a conditional branch.
    fn emit_br_cond(&mut self, cond: &EmitValue, then_label: &str, else_label: &str);

    /// Begin a new basic block with the given label.
    fn emit_block(&mut self, label: &str);

    /// Emit a switch instruction.
    fn emit_switch(
        &mut self,
        discr: &EmitValue,
        discr_ty: &EmitType,
        cases: &[(i128, String)],
        default_label: &str,
    );

    /// Allocate stack space for a local variable.
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue;

    /// Store a value to a pointer.
    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue);

    /// Load a value from a pointer.
    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue;

    /// Emit a function call with typed arguments.
    fn emit_call(
        &mut self,
        fn_name: &str,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue;

    /// Emit a dyn Trait vtable indirect call.
    fn emit_dyn_trait_method_call(
        &mut self,
        dynptr_symbol: &str,
        slot_index: u32,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue;

    /// Emit an integer comparison (icmp).
    fn emit_icmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Emit a float comparison (fcmp).
    fn emit_fcmp(&mut self, op: &str, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue)
        -> EmitValue;

    /// Emit a bitwise AND.
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a bitwise OR.
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;

    /// Emit a zero-extend (zext).
    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a type cast.
    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue;

    /// Emit a `select` instruction.
    fn emit_select(
        &mut self,
        ty: &EmitType,
        cond: &EmitValue,
        true_val: &EmitValue,
        false_val: &EmitValue,
    ) -> EmitValue;

    /// Emit a getelementptr for struct field access.
    fn emit_gep_field(
        &mut self,
        base_ptr: &EmitValue,
        struct_ty: &EmitType,
        field_index: u32,
    ) -> EmitValue;

    /// Emit a getelementptr for array index access.
    fn emit_gep_index(
        &mut self,
        base_ptr: &EmitValue,
        array_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;

    /// Emit a getelementptr for element access via a raw element pointer.
    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;

    /// Emit a PHI node.
    fn emit_phi(&mut self, ty: &EmitType, incoming: &[(EmitValue, String)]) -> EmitValue;

    /// Emit insertvalue for tuple/struct construction.
    fn emit_insertvalue(
        &mut self,
        agg_ty: &EmitType,
        agg: &EmitValue,
        val_ty: &EmitType,
        val: &EmitValue,
        index: u32,
    ) -> EmitValue;

    /// Emit extractvalue for tuple/struct field extraction.
    fn emit_extractvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue, index: u32) -> EmitValue;

    /// Emit a checked-binary-op intrinsic call.
    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue;

    // === Local state ===

    /// Store a local's pointer handle (alloca result).
    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue);

    /// Get a local's pointer handle.
    fn get_local_ptr(&self, local_id: u32) -> Option<&EmitValue>;

    /// Store a local's value handle.
    fn set_local(&mut self, local_id: u32, val: EmitValue);

    /// Get a local's stored value handle.
    fn get_local(&self, local_id: u32) -> Option<&EmitValue>;
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
