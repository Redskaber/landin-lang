//! AggregateEmitter sub-trait — aggregate construction & function calls.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). AggregateEmitter owns the 5
//! methods that build aggregate values or invoke functions: PHI nodes,
//! insertvalue/extractvalue for tuple/struct construction, direct calls,
//! and dyn-trait vtable indirect calls.

use super::{EmitType, EmitValue};

/// Aggregate-construction & call emission.
///
/// Per §13.4 J2 single responsibility: this trait covers everything that
/// constructs an aggregate value or invokes a function — PHI nodes,
/// insertvalue/extractvalue for tuple/struct field manipulation, direct
/// function calls, and dyn-trait vtable indirect calls.
pub trait AggregateEmitter {
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
}
