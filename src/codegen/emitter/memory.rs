//! MemoryEmitter sub-trait — stack allocation & pointer arithmetic.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). MemoryEmitter owns the 6 methods
//! that work with memory: stack allocation, load/store, and the three
//! GEP variants (struct field, array index, raw element pointer).

use super::{EmitType, EmitValue};

/// Memory / pointer-arithmetic emission.
///
/// Per §13.4 J2 single responsibility: this trait covers everything that
/// accesses or addresses memory — stack allocation (`alloca`), load/store,
/// and the three getelementptr variants (struct field, array index, raw
/// element pointer).
pub trait MemoryEmitter {
    /// Allocate stack space for a local variable.
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue;

    /// Store a value to a pointer.
    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue);

    /// Load a value from a pointer.
    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue;

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
    ///
    /// # Parameters
    ///
    /// - `base_ptr`: the data pointer (already unwrapped from any fat pointer
    ///   storage) — points to the first element of the buffer.
    /// - `elem_ty`: the element type (e.g. `i8` for `*mut u8`, `i32` for
    ///   `*mut i32`, `i32` for `&[i32]` slice element).
    /// - `idx_ty`: the **index value's** type (e.g. `i32` if the index is a
    ///   `let i: i32` local, `i64` if it's a `usize`/`i64` local). This is the
    ///   source-of-truth for the GEP index type in TextEmitter output. The
    ///   LLVMSysEmitter ignores this parameter — LLVM's `LLVMBuildInBoundsGEP2`
    ///   accepts any integer type as the index.
    /// - `index`: the index value (SSA name or literal).
    ///
    /// # Why `idx_ty` is a separate parameter (Stage 143, TD-PTR-INDEX-GEP-TYPE)
    ///
    /// `EmitValue` is a `String` (SSA name or literal) and does not carry type
    /// information. Previously, `TextEmitter::emit_gep_index_ptr` hardcoded `i64`
    /// for the index type — which failed when the actual MIR local was `i32`
    /// (text IR tests expected `i32 <idx>`, got `i64 <idx>`).
    ///
    /// Per §1.0 原則 6 (通解 > 特解) + §1.0 原則 10 (唯一可信数据源): the caller
    /// (which has access to MIR `local_decls`) is the source of truth for the
    /// index type, and must pass it explicitly to this method.
    ///
    /// Per §1.0 原則 5 (去除兼容思维): the old signature is replaced, not kept
    /// alongside a new one.
    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        idx_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;
}
