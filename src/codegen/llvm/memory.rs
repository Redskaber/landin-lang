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
            let val_ty = LLVMTypeOf(v);
            let target_llvm_ty = self.llvm_type(ty);
            let val_kind = LLVMGetTypeKind(val_ty);
            let target_kind = LLVMGetTypeKind(target_llvm_ty);

            // Stage 18.205 (TD-FUNCTION-REDEFINE-PARAMS fix): When storing a
            // POINTER-typed value (target_kind == PointerTypeKind), force an
            // 8-byte store by casting through i64. This works around a LLVM
            // backend optimization that collapses `store ptr null` to a 4-byte
            // `store i32 0` (which leaves upper 4 bytes uninitialized, causing
            // ABI mismatches when the value is later loaded as an 8-byte
            // pointer and passed to C functions).
            //
            // The optimization is incorrect for our use case because Landin
            // stores pointer constants (like null) to stack slots and later
            // loads them as full 8-byte pointers. LLVM's `-O2` pass sees the
            // null constant and uses a 32-bit store (since on x86-64, writing
            // to the lower 32 bits of a register zero-extends, but this does
            // NOT apply to memory writes).
            //
            // Fix: bitcast the pointer to `i64*` and store the pointer as an
            // `i64` value (via PtrToInt). This forces an 8-byte store.
            //
            // Per §1.0 原則 9 (正确>妥协): fix root cause (force 8-byte store),
            // not symptom (zero-initialize upper bytes separately).
            // Per §1.0 原則 6 (通解>特例): one rule for all pointer stores.
            if target_kind == llvm_sys::LLVMTypeKind::LLVMPointerTypeKind {
                let i64_ty = LLVMInt64TypeInContext(self.ctx);
                let i64_ptr_ty = LLVMPointerType(i64_ty, 0);
                // Cast the pointer to i64* (bitcast is valid for ptr→ptr).
                let name_c = cstr_owned("ptrstore_cast");
                let i64_ptr = LLVMBuildBitCast(self.builder, p, i64_ptr_ty, name_c.as_ptr());
                // Cast the value to i64 (PtrToInt handles ptr→i64).
                let val_i64 = if val_kind == llvm_sys::LLVMTypeKind::LLVMPointerTypeKind {
                    let name_c = cstr_owned("p2i");
                    LLVMBuildPtrToInt(self.builder, v, i64_ty, name_c.as_ptr())
                } else if val_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind {
                    // Already integer — cast to i64 if needed.
                    let val_width = LLVMGetIntTypeWidth(val_ty);
                    if val_width == 64 {
                        v
                    } else {
                        let name_c = cstr_owned("i2i64");
                        LLVMBuildIntCast2(self.builder, v, i64_ty, 0, name_c.as_ptr())
                    }
                } else {
                    v
                };
                LLVMBuildStore(self.builder, val_i64, i64_ptr);
                return;
            }

            let stored = if val_ty == target_llvm_ty {
                v
            } else if val_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                && target_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
            {
                let name_c = cstr_owned("cast");
                LLVMBuildIntCast2(self.builder, v, target_llvm_ty, 1, name_c.as_ptr())
            } else {
                v
            };

            // Stage 18.190 (TD-BOX-NEW-TYPE-COERCE fix): Cast the pointer to
            // the correct element type before storing. Previously, if the
            // pointer was `*mut u8` (i8*) but the value was i64, LLVM would
            // store only 1 byte (truncating the i64). Now we bitcast the
            // pointer to `target_llvm_ty*` before storing.
            //
            // Root cause: Box::new(x) allocates via __landin_alloc which
            // returns *mut u8. Storing an i64 through *mut u8 truncated it.
            //
            // Per §1.0 原則 9 (正确>妥协): fix root cause (cast pointer type),
            // not symptom (skip store).
            // Per §1.0 原則 6 (通解>特例): one bitcast for all type mismatches.
            let ptr_ty = LLVMTypeOf(p);
            let expected_ptr_ty = LLVMPointerType(target_llvm_ty, 0);
            let final_ptr = if ptr_ty == expected_ptr_ty {
                p
            } else {
                let name_c = cstr_owned("pcast");
                LLVMBuildBitCast(self.builder, p, expected_ptr_ty, name_c.as_ptr())
            };

            LLVMBuildStore(self.builder, stored, final_ptr);
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
