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
    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue;
}
