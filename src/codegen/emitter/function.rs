//! FunctionEmitter sub-trait — function scope & control flow.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). FunctionEmitter owns the 8 methods
//! that manage function lifecycle and control flow: function begin/end,
//! basic blocks, returns, unreachable, branches, and switch.

use super::{EmitType, EmitValue};

/// Function-scope emission: function lifecycle + control flow.
///
/// Per §13.4 J2 single responsibility: this trait covers everything that
/// structures the control flow inside a function body — function prologue/
/// epilogue, basic blocks, returns, branches, and switch dispatch.
pub trait FunctionEmitter {
    /// Begin a new function definition.
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType);

    /// End the current function definition.
    fn emit_function_end(&mut self);

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
}
