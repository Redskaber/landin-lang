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
    ///
    /// Stage 154 (TD-DYN-LOCAL-FAT-PTR-COERCION): `receiver_value` is the
    /// SSA value of the fat pointer `{ptr, ptr}` to dispatch through. This
    /// can be either:
    /// - A global symbol like `@.dynptr.Greeter.English` (call-site coercion,
    ///   Stage 89 path — the global dynptr constant)
    /// - A local SSA value like `%loc_5` (let-binding coercion, Stage 154
    ///   path — the local fat pointer constructed by `codegen_statement`)
    ///
    /// The GEP + load + indirect call pattern is the same for both:
    /// ```text
    /// %gep_vtable = getelementptr { ptr, ptr }, ptr <receiver>, i32 0, i32 1
    /// %vtable     = load ptr, ptr %gep_vtable
    /// %gep_method = getelementptr [N x ptr], ptr %vtable, i32 0, i32 slot_index
    /// %method_fn  = load ptr, ptr %gep_method
    /// %gep_data   = getelementptr { ptr, ptr }, ptr <receiver>, i32 0, i32 0
    /// %data_ptr   = load ptr, ptr %gep_data
    /// %result     = call <ret_ty> %method_fn(<args>)
    /// ```
    ///
    /// Per §1.0 原則 6 (通解 > 特解): one dispatch path for both global and
    /// local fat pointers — the GEP pattern is identical.
    /// Per §1.0 原則 9 (正确 > 妥协): use the actual receiver value, not a
    /// hardcoded global symbol.
    fn emit_dyn_trait_method_call(
        &mut self,
        receiver_value: &str,
        slot_index: u32,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue;
}
