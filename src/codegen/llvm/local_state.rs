//! Stage 16.77 MUV-1: `impl LocalStateEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::LocalStateEmitter;
use crate::codegen::emitter::*;

use super::LLVMSysEmitter;

impl LocalStateEmitter for LLVMSysEmitter {
    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue) {
        self.local_ptrs.insert(local_id, ptr);
    }

    fn get_local_ptr(&self, local_id: u32) -> Option<&EmitValue> {
        self.local_ptrs.get(&local_id)
    }

    fn set_local(&mut self, local_id: u32, val: EmitValue) {
        self.locals.insert(local_id, val);
    }

    fn get_local(&self, local_id: u32) -> Option<&EmitValue> {
        self.locals.get(&local_id)
    }
}
