//! Stage 16.77 MUV-1: `impl FunctionEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use super::helpers::cstr_owned;
use crate::codegen::emitter::FunctionEmitter;
use crate::codegen::emitter::*;
use llvm_sys::core::*;
use llvm_sys::prelude::*;

use super::LLVMSysEmitter;

impl FunctionEmitter for LLVMSysEmitter {
    fn emit_function_begin(&mut self, name: &str, params: &[(EmitType, &str)], ret: &EmitType) {
        unsafe {
            // Build function type.
            let ret_ty = self.llvm_type(ret);
            let param_tys: Vec<LLVMTypeRef> =
                params.iter().map(|(t, _)| self.llvm_type(t)).collect();
            let fty = LLVMFunctionType(
                ret_ty,
                param_tys.as_ptr() as *mut LLVMTypeRef,
                param_tys.len() as u32,
                0,
            );
            let name_c = cstr_owned(name);
            // Stage 14.63: Reuse existing forward declaration if present.
            //
            // When functions are mutually recursive, a forward declaration
            // is created via `get_or_declare_function` (called by emit_call)
            // before we reach `emit_function_begin` for the actual definition.
            // If we call `LLVMAddFunction` again with the same name, LLVM
            // silently renames the new function (e.g. `foo` → `foo.1`),
            // producing an "undefined reference" link error.
            //
            // Fix: first check `self.declared` cache and the module's named-
            // function table. If a declaration already exists, reuse it
            // (LLVM allows redefining a function's body in-place by adding
            // basic blocks to the existing function value).
            let existing = if let Some(v) = self.declared.get(name) {
                Some(*v)
            } else {
                let v = LLVMGetNamedFunction(self.module, name_c.as_ptr());
                if !v.is_null() {
                    Some(v)
                } else {
                    None
                }
            };
            let fn_val = if let Some(existing) = existing {
                // Stage 14.92 (Bug X3 complete fix): Always reuse the existing
                // function declaration, regardless of type mismatch.
                //
                // Stage 18.188 (TD-FUNCTION-REDEFINE bug fix): If the existing
                // declaration was auto-created by `get_or_declare_function` (a
                // forward declaration with WRONG return type, e.g., i32 variadic),
                // AND the actual function definition has a struct return type,
                // LLVM will REUSE the wrong-typed declaration — producing
                // "Function return type does not match operand type of return inst"
                // verification errors.
                //
                // Fix: Delete the existing declaration's body (if any) and let
                // LLVMAddFunction create a new one. But LLVM doesn't allow
                // re-adding a function with the same name — it silently renames
                // (.1 suffix). So instead, we delete the existing function and
                // re-add with the correct type.
                //
                // Per §1.0 原則 4 (报错>静默): the old code silently reused
                // wrong-typed declarations, producing invalid IR.
                // Per §1.0 原則 9 (正确>妥协): fix the root cause (delete + re-add)
                // rather than the symptom (skip verification).
                let existing_ret_ty = LLVMGetReturnType(LLVMGlobalGetValueType(existing));
                if existing_ret_ty != ret_ty {
                    // Type mismatch — delete the old declaration and re-add.
                    // This is safe because the old declaration has no body (just
                    // a forward decl from get_or_declare_function).
                    LLVMDeleteFunction(existing);
                    self.declared.remove(name);
                    LLVMAddFunction(self.module, name_c.as_ptr(), fty)
                } else {
                    existing
                }
            } else {
                LLVMAddFunction(self.module, name_c.as_ptr(), fty)
            };
            // Register the function in the declared cache so subsequent
            // emit_call sites resolve to this same function value.
            self.declared.insert(name.to_string(), fn_val);
            self.cur_fn = Some(fn_val);

            // Reset per-function state.
            self.locals.clear();
            self.local_ptrs.clear();
            self.blocks.clear();
            self.next_val = params.len() as u32 + 1;

            // Create entry block and position builder there.
            let entry_name = cstr_owned("entry");
            let entry_bb = LLVMAppendBasicBlockInContext(self.ctx, fn_val, entry_name.as_ptr());
            LLVMPositionBuilderAtEnd(self.builder, entry_bb);

            // Register each parameter under its name (e.g. "%arg0").
            for (i, (_, pname)) in params.iter().enumerate() {
                let pval = LLVMGetParam(fn_val, i as u32);
                self.set_value_name(pval, pname);
                self.values.insert(pname.to_string(), pval);
            }

            // Register the entry block under "entry" and "%entry".
            self.blocks.insert("%entry".to_string(), entry_bb);
        }
    }

    fn emit_function_end(&mut self) {
        // LLVM handles this — the module already contains the function.
        // We just clear the current-function pointer.
        self.cur_fn = None;
    }

    fn emit_ret(&mut self, ty: &EmitType, val: Option<&EmitValue>) {
        unsafe {
            match val {
                Some(v) => {
                    let _ = ty;
                    let v_ref = self.lookup(v);
                    LLVMBuildRet(self.builder, v_ref);
                }
                None => {
                    LLVMBuildRetVoid(self.builder);
                }
            }
        }
    }

    fn emit_unreachable(&mut self) {
        unsafe {
            LLVMBuildUnreachable(self.builder);
        }
    }

    fn emit_br(&mut self, label: &str) {
        unsafe {
            let bb = self.block_for(label);
            LLVMBuildBr(self.builder, bb);
        }
    }

    fn emit_br_cond(&mut self, cond: &EmitValue, then_label: &str, else_label: &str) {
        unsafe {
            let cond_v = self.lookup(cond);
            let then_bb = self.block_for(then_label);
            let else_bb = self.block_for(else_label);
            // Stage 14.44: Ensure the condition is i1 (boolean).
            // Comparison operators (Eq/Lt/etc.) produce i1, but the result may
            // be stored in an i32 alloca (when the local's type is Infer→i32)
            // and loaded back as i32. LLVM requires br conditions to be i1.
            // Was: passed i32 directly → "Branch condition is not 'i1' type"
            // verifier error (caught now that we added LLVMVerifyModule).
            let cond_ty = LLVMTypeOf(cond_v);
            let i1_ty = LLVMInt1TypeInContext(self.ctx);
            let cond_i1 = if LLVMGetTypeKind(cond_ty) == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                && LLVMGetIntTypeWidth(cond_ty) != 1
            {
                // Truncate i32 → i1 (non-zero is true)
                let name_c = cstr_owned("tobool");
                LLVMBuildTrunc(self.builder, cond_v, i1_ty, name_c.as_ptr())
            } else if LLVMGetTypeKind(cond_ty) == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                && LLVMGetIntTypeWidth(cond_ty) == 1
            {
                cond_v
            } else {
                // Other types — try ICMP ne 0 to convert to i1
                let zero = LLVMConstInt(cond_ty, 0, 0);
                let name_c = cstr_owned("tobool");
                LLVMBuildICmp(
                    self.builder,
                    llvm_sys::LLVMIntPredicate::LLVMIntNE,
                    cond_v,
                    zero,
                    name_c.as_ptr(),
                )
            };
            LLVMBuildCondBr(self.builder, cond_i1, then_bb, else_bb);
        }
    }

    fn emit_block(&mut self, label: &str) {
        // Stage 13.6 fix: For the first block after emit_function_begin,
        // reuse the entry block instead of creating a new one.
        // emit_function_begin creates an entry BB and registers it as "%entry".
        // codegen_from_mir then calls emit_block("bb0") — this should reuse
        // the entry BB (rename it) rather than creating a second orphan BB.
        let key = if label.starts_with('%') {
            label.to_string()
        } else {
            format!("%{}", label)
        };

        // Check if this is the first emit_block call (entry BB exists, no other
        // blocks registered yet besides %entry)
        if self.blocks.len() == 1 && self.blocks.contains_key("%entry") {
            // Reuse the entry block — just register it under the new label.
            let entry_bb = self.blocks["%entry"];
            self.blocks.insert(key.clone(), entry_bb);
            self.blocks.remove("%entry");
            unsafe {
                LLVMPositionBuilderAtEnd(self.builder, entry_bb);
            }
        } else {
            // Normal case: create or look up the BB.
            unsafe {
                let bb = self.block_for(label);
                LLVMPositionBuilderAtEnd(self.builder, bb);
            }
        }

        // Invalidate the local value cache at block boundaries.
        self.locals.clear();
    }

    fn emit_switch(
        &mut self,
        discr: &EmitValue,
        discr_ty: &EmitType,
        cases: &[(i128, String)],
        default_label: &str,
    ) {
        unsafe {
            let discr_v = self.lookup(discr);
            let default_bb = self.block_for(default_label);
            let sw = LLVMBuildSwitch(self.builder, discr_v, default_bb, cases.len() as u32);
            let case_ty = self.llvm_type(discr_ty);
            for (val, label) in cases {
                let case_bb = self.block_for(label);
                let case_v = LLVMConstInt(case_ty, *val as u64, 1);
                LLVMAddCase(sw, case_v, case_bb);
            }
        }
    }
}
