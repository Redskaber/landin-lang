//! Stage 16.77 MUV-2: `impl FunctionEmitter for TextEmitter`.
//!
//! Extracted from `text/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::*;

use super::emit_type_to_llvm_str;
use super::TextEmitter;

impl FunctionEmitter for TextEmitter {
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType) {
        let ret_str = emit_type_to_llvm_str(ret);
        let param_strs: Vec<String> = params
            .iter()
            .map(|(ty, name)| format!("{} {}", emit_type_to_llvm_str(ty), name))
            .collect();
        self.line(&format!(
            "define {} @{}({}) {{",
            ret_str,
            name,
            param_strs.join(", ")
        ));
        self.next_val = params.len() as u32 + 1;
        self.locals.clear();
        self.local_ptrs.clear();
    }

    fn emit_function_end(&mut self) {
        self.line("}");
        self.line("");
    }

    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>) {
        let ty_str = emit_type_to_llvm_str(ty);
        match val {
            Some(v) => self.line(&format!("  ret {} {}", ty_str, v)),
            None => self.line("  ret void"),
        }
    }

    fn emit_unreachable(&mut self) {
        self.line("  unreachable");
    }

    fn emit_br(&mut self, label: &str) {
        self.line(&format!("  br label %{}", label));
    }

    fn emit_br_cond(&mut self, cond: &EmitValue, then_label: &str, else_label: &str) {
        self.line(&format!(
            "  br i1 {}, label %{}, label %{}",
            cond, then_label, else_label
        ));
    }

    fn emit_block(&mut self, label: &str) {
        self.line(&format!("{}:", label));
        // Stage 3.22: invalidate the local value cache at block boundaries.
        // Values assigned in a predecessor block must be reloaded from their
        // alloca slots — otherwise we'd leak the most-recent assignment into
        // successor blocks, which is unsound for if/match/while joins where
        // a local takes different values along different predecessors.
        // `local_ptrs` (the alloca handles) are NOT cleared — they persist
        // for the whole function.
        self.locals.clear();
    }

    fn emit_switch(
        &mut self,
        discr: &EmitValue,
        discr_ty: &EmitType,
        cases: &[(i128, String)],
        default_label: &str,
    ) {
        let ty_str = emit_type_to_llvm_str(discr_ty);
        self.line(&format!(
            "  switch {} {}, label %{} [",
            ty_str, discr, default_label
        ));
        for (val, label) in cases {
            self.line(&format!("    {} {}, label %{}", ty_str, val, label));
        }
        self.line("  ]");
    }
}
