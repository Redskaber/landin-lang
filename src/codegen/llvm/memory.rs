//! Stage 16.77 MUV-1: `impl MemoryEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use super::helpers::cstr_owned;
use crate::codegen::emitter::MemoryEmitter;
use crate::codegen::emitter::*;
use llvm_sys::core::*;

use super::LLVMSysEmitter;

impl MemoryEmitter for LLVMSysEmitter {
    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue {
        unsafe {
            let llvm_ty = self.llvm_type(ty);
            let name_c = cstr_owned(name);
            let ptr = LLVMBuildAlloca(self.builder, llvm_ty, name_c.as_ptr());
            self.named(ptr, name)
        }
    }

    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue) {
        unsafe {
            let v = self.lookup(val);
            let p = self.lookup(ptr);
            // Stage 14.64: Coerce INTEGER values to the target type before storing.
            //
            // Previously, this function ignored the `ty` parameter and just
            // called `LLVMBuildStore(builder, v, p)`, which uses the value's
            // actual LLVM type. This caused silent miscompilation when the
            // value's type didn't match the alloca's type:
            //
            //   - i32 constant stored to i64 alloca: only 4 bytes written,
            //     upper 4 bytes are garbage. Loading as i64 produces wrong
            //     values (e.g., `180228417674752` instead of `3000000000`).
            //   - i32 comparison result stored to i1 alloca: type mismatch.
            //
            // Fix: for INTEGER types only, check the value's actual LLVM type
            // (via LLVMTypeOf). If it doesn't match `ty`, cast the value first
            // (zext/sext/trunc). For non-integer types (struct, array, etc.),
            // we assume the types match and store directly — a mismatch there
            // is a codegen bug that should surface as an LLVM verification error.
            //
            // Per §1.0 原则 5 "报错 > 静默": integer mismatches are fixed by
            // explicit casts; non-integer mismatches surface as errors.
            let val_ty = LLVMTypeOf(v);
            let target_llvm_ty = self.llvm_type(ty);
            let val_kind = LLVMGetTypeKind(val_ty);
            let target_kind = LLVMGetTypeKind(target_llvm_ty);
            let stored = if val_ty == target_llvm_ty {
                v
            } else if val_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                && target_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
            {
                // Integer-to-integer cast (zext/sext/trunc).
                // Use signed extension (1) since Landin's integer literals
                // default to i32 (signed). This matches the `emit_cast`
                // behavior for (I32, I64) → SExt.
                let name_c = cstr_owned("cast");
                LLVMBuildIntCast2(self.builder, v, target_llvm_ty, 1, name_c.as_ptr())
            } else {
                // Non-integer types with mismatch — store directly and let
                // LLVM module verification catch it (surfaces the bug).
                v
            };
            LLVMBuildStore(self.builder, stored, p);
        }
    }

    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue {
        unsafe {
            let llvm_ty = self.llvm_type(ty);
            let p = self.lookup(ptr);
            let name_c = cstr_owned("ld");
            let v = LLVMBuildLoad2(self.builder, llvm_ty, p, name_c.as_ptr());
            self.fresh_named(v)
        }
    }

    fn emit_gep_field(
        &mut self,
        base_ptr: &EmitValue,
        struct_ty: &EmitType,
        field_index: u32,
    ) -> EmitValue {
        unsafe {
            let base = self.lookup(base_ptr);
            let llvm_struct_ty = self.llvm_type(struct_ty);
            // Indices: [0, field_index] — first 0 indexes through the pointer.
            let zero = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), 0, 0);
            let idx = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), field_index as u64, 0);
            let mut indices = [zero, idx];
            let name_c = cstr_owned("gep");
            let v = LLVMBuildInBoundsGEP2(
                self.builder,
                llvm_struct_ty,
                base,
                indices.as_mut_ptr(),
                indices.len() as u32,
                name_c.as_ptr(),
            );
            self.fresh_named(v)
        }
    }

    fn emit_gep_index(
        &mut self,
        base_ptr: &EmitValue,
        array_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue {
        unsafe {
            let base = self.lookup(base_ptr);
            let llvm_array_ty = self.llvm_type(array_ty);
            let zero = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), 0, 0);
            let idx_v = self.lookup(index);
            let mut indices = [zero, idx_v];
            let name_c = cstr_owned("gep");
            let v = LLVMBuildInBoundsGEP2(
                self.builder,
                llvm_array_ty,
                base,
                indices.as_mut_ptr(),
                indices.len() as u32,
                name_c.as_ptr(),
            );
            self.fresh_named(v)
        }
    }

    fn emit_gep_index_ptr(
        &mut self,
        base_ptr: &EmitValue,
        elem_ty: &EmitType,
        index: &EmitValue,
    ) -> EmitValue {
        unsafe {
            let base = self.lookup(base_ptr);
            let llvm_elem_ty = self.llvm_type(elem_ty);
            let idx_v = self.lookup(index);
            let mut indices = [idx_v];
            let name_c = cstr_owned("gep");
            let v = LLVMBuildInBoundsGEP2(
                self.builder,
                llvm_elem_ty,
                base,
                indices.as_mut_ptr(),
                indices.len() as u32,
                name_c.as_ptr(),
            );
            self.fresh_named(v)
        }
    }
}
