//! Stage 16.77 MUV-2: `impl MemoryEmitter for TextEmitter`.
//!
//! Extracted from `text/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::*;

use super::emit_type_to_llvm_str;
use super::TextEmitter;

impl MemoryEmitter for TextEmitter {
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue {
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  {} = alloca {}", name, ty_str));
        name.to_string()
    }

    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue) {
        let ty_str = emit_type_to_llvm_str(ty);
        // Stage 18.327 (P1 soundness fix): LLVM 17+ opaque pointer requires
        // `store <ty> <val>, ptr <ptr_val>` format. Previously emitted
        // `store <ty> <val>, <ptr_val>` (missing `ptr` prefix) → invalid IR.
        // Per LLVM Language Reference: store instruction requires typed pointer.
        self.line(&format!("  store {} {}, ptr {}", ty_str, val, ptr));
    }

    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        // Stage 18.327 (P1 soundness fix): LLVM 17+ opaque pointer requires
        // `load <ty>, ptr <ptr_val>` format. Previously emitted `load <ty>, <ptr_val>`
        // (missing `ptr` prefix) → invalid IR → segfault.
        // Per LLVM Language Reference: load instruction requires typed pointer.
        // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
        self.line(&format!("  %v{} = load {}, ptr {}", r, ty_str, ptr));
        format!("%v{}", r)
    }

    fn emit_gep_field(
        &mut self,
        base_ptr: &EmitValue,
        struct_ty: &EmitType,
        field_index: u32,
    ) -> EmitValue {
        let r = self.fresh();
        let struct_str = emit_type_to_llvm_str(struct_ty);
        // Stage 18.327 (P1 soundness fix): use opaque pointer `ptr` instead of
        // typed pointer `{}*`. LLVM 17+ requires opaque pointers — typed
        // pointers like `{ ptr }*` produce invalid IR that causes segfaults.
        //
        // **Design boundary** (per LLVM Language Reference + rustc_codegen_llvm):
        // - GEP format: `getelementptr inbounds <elem_ty>, ptr <base>, i32 0, i32 <idx>`
        // - The base pointer type is ALWAYS `ptr` (opaque), not `T*` (typed).
        // - The element type is specified as the first GEP parameter.
        //
        // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
        // Per §1.0 原則 6 (通解>特解): one rule for ALL GEP instructions.
        self.line(&format!(
            "  %v{} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
            r, struct_str, base_ptr, field_index
        ));
        format!("%v{}", r)
    }

    fn emit_gep_index(
        &mut self,
        base_ptr: &EmitValue,
        array_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let array_str = emit_type_to_llvm_str(array_ty);
        // Stage 14.59: LLVM 19 opaque pointers — use "ptr" instead of "array*"
        let ptr_str = "ptr".to_string();
        self.line(&format!(
            "  %v{} = getelementptr inbounds {}, {} {}, i32 0, i32 {}",
            r, array_str, ptr_str, base_ptr, index
        ));
        format!("%v{}", r)
    }

    /// Stage 3.51: GEP into a raw element pointer (for slice indexing).
    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let elem_str = emit_type_to_llvm_str(elem_ty);
        // Stage 14.59: LLVM 19 opaque pointers — use "ptr" instead of "elem*"
        let ptr_str = "ptr".to_string();
        self.line(&format!(
            "  %v{} = getelementptr inbounds {}, {} {}, i32 {}",
            r, elem_str, ptr_str, base_ptr, index
        ));
        format!("%v{}", r)
    }
}
