//! Stage 16.77 MUV-1: `impl FunctionEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use super::helpers::{create_byval_attribute, create_sret_attribute, cstr_owned};
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
            // Stage 18.333 (P1 soundness fix): For struct/array parameters > 16 bytes,
            // build the byval signature: replace the param type with `ptr` and add
            // `byval(<orig_ty>)` attribute at the param's 1-indexed position.
            //
            // Per System V AMD64 ABI §3.2.3: structs/arrays > 16 bytes passed as
            // parameters must be passed via a hidden pointer parameter with the
            // `byval` attribute (mirrors `sret` for returns).
            //
            // rustc_codegen_llvm emits byval explicitly via `Attribute::ByVal`;
            // we mirror this via `LLVMCreateTypeAttribute(ctx, byval_kind, ty)`.
            //
            // **Design boundary** (mirrors Stage 18.332 sret):
            // - `needs_byval()` is the SINGLE source of truth for the threshold.
            // - Both TextEmitter and LLVMSysEmitter agree on byval emission.
            // - Param index shifts when sret is active: user param `i` is at
            //   LLVM index `i + 1 + (1 if use_sret else 0)` (LLVM 1-indexed).
            //
            // Per §1.0 原則 6 (通解 > 特解): one byval path for all > 16B struct/array params.
            // Per §20 (iterative audit): same root cause as sret bug; same fix pattern.
            let use_sret = ret.needs_sret();
            let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);

            // Compute the LLVM param types, replacing byval-eligible types with `ptr`.
            // Track which user params are byval so we can add attributes after fn creation.
            let mut byval_infos: Vec<(usize, LLVMTypeRef)> = Vec::new(); // (user_idx, orig_llvm_ty)
            let mut user_param_llvm_tys: Vec<LLVMTypeRef> = Vec::with_capacity(params.len());
            for (i, (t, _)) in params.iter().enumerate() {
                let orig_llvm_ty = self.llvm_type(t);
                if t.needs_byval() {
                    byval_infos.push((i, orig_llvm_ty));
                    user_param_llvm_tys.push(ptr_ty);
                } else {
                    user_param_llvm_tys.push(orig_llvm_ty);
                }
            }

            // Stage 18.332 (P1 soundness fix): For struct return > 16 bytes,
            // build the sret signature: `void (ptr sret(<ret_ty>), ...params)`.
            //
            // Per System V AMD64 ABI §3.2.3: structs > 16 bytes must be returned
            // via a hidden sret pointer parameter. Without this, the generated
            // machine code corrupts the stack when the caller's return-value
            // slot is smaller than the actual struct size.
            //
            // rustc_codegen_llvm emits sret explicitly via `Attribute::StructRet`;
            // we mirror this via `LLVMCreateTypeAttribute(ctx, sret_kind, ret_ty)`
            // added at parameter index 1 (LLVM uses 1-indexed param attributes).
            //
            // **Design boundary** (per Stage 18.330 TextEmitter + rustc reference):
            // - `needs_sret()` is the SINGLE source of truth for the sret threshold.
            // - Both TextEmitter and LLVMSysEmitter agree on sret emission.
            // - The sret pointer is registered under "%_sret" — same name as TextEmitter.
            //
            // Per §1.0 原則 6 (通解 > 特解): one sret path for all > 16B struct returns.
            // Per §12 (最优 > 最小): root-cause fix at IR level, not auto-demotion.
            // Per §2.2 原則 9 (正确 > 妥协): correct ABI > no optimization.
            let (fn_ret_ty, fn_param_tys): (LLVMTypeRef, Vec<LLVMTypeRef>) = if use_sret {
                let void_ty = LLVMVoidTypeInContext(self.ctx);
                let mut sret_params: Vec<LLVMTypeRef> = vec![ptr_ty];
                sret_params.extend(user_param_llvm_tys.iter().copied());
                (void_ty, sret_params)
            } else {
                (ret_ty, user_param_llvm_tys.clone())
            };

            let fty = LLVMFunctionType(
                fn_ret_ty,
                fn_param_tys.as_ptr() as *mut LLVMTypeRef,
                fn_param_tys.len() as u32,
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
                // Stage 18.332: Now that declare_function + interpret_adhoc
                // BOTH build sret signatures when needs_sret(), forward decls
                // already have the correct signature. The delete + re-add
                // fallback below remains as a safety net for any legacy callers
                // that still produce mismatched decls.
                let existing_ret_ty = LLVMGetReturnType(LLVMGlobalGetValueType(existing));
                if existing_ret_ty != fn_ret_ty {
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

            // Stage 18.332: Add sret attribute to parameter 1 (the hidden
            // sret pointer) when use_sret is true. This must be applied
            // regardless of whether the function was newly created or
            // reused from a forward declaration (forward decls also add
            // sret via declare_function / interpret_adhoc, but applying
            // it again is idempotent in LLVM — same attribute, no effect).
            if use_sret {
                let sret_attr = create_sret_attribute(self.ctx, ret_ty);
                LLVMAddAttributeAtIndex(fn_val, 1, sret_attr);
            }

            // Stage 18.333: Add byval attribute to each byval-eligible user param.
            // The LLVM param index for user param `i` is `i + 1 + (1 if use_sret else 0)`
            // because:
            // - LLVM uses 1-indexed params (1..N).
            // - When use_sret, param 1 is the sret slot, so user params start at 2.
            // - When not use_sret, user params start at 1.
            let sret_offset: u32 = if use_sret { 1 } else { 0 };
            for (user_idx, orig_llvm_ty) in &byval_infos {
                let llvm_param_idx = (*user_idx as u32) + 1 + sret_offset;
                let byval_attr = create_byval_attribute(self.ctx, *orig_llvm_ty);
                LLVMAddAttributeAtIndex(fn_val, llvm_param_idx, byval_attr);
            }

            // Register the function in the declared cache so subsequent
            // emit_call sites resolve to this same function value.
            self.declared.insert(name.to_string(), fn_val);
            self.cur_fn = Some(fn_val);

            // Reset per-function state.
            self.locals.clear();
            self.local_ptrs.clear();
            self.blocks.clear();
            self.next_val = if use_sret {
                params.len() as u32 + 2 // +1 for sret, +1 for 1-indexed
            } else {
                params.len() as u32 + 1
            };

            // Create entry block and position builder there.
            let entry_name = cstr_owned("entry");
            let entry_bb = LLVMAppendBasicBlockInContext(self.ctx, fn_val, entry_name.as_ptr());
            LLVMPositionBuilderAtEnd(self.builder, entry_bb);

            // Stage 18.332: When use_sret, register the sret pointer under
            // "%_sret" (same name as TextEmitter). Param 0 is the sret slot;
            // user-visible params start at index 1.
            if use_sret {
                let sret_param = LLVMGetParam(fn_val, 0);
                self.set_value_name(sret_param, "_sret");
                self.values.insert("%_sret".to_string(), sret_param);
            }

            // Register each parameter under its name (e.g. "%arg0").
            // When use_sret, skip param 0 (sret pointer).
            // For byval params, the LLVM param value is a `ptr` (the caller's
            // stack slot pointer). We register it under the user-visible name
            // so the function body can use it transparently (load from the ptr
            // to access the struct value).
            let param_offset = if use_sret { 1u32 } else { 0u32 };
            for (i, (_, pname)) in params.iter().enumerate() {
                let pval = LLVMGetParam(fn_val, (i as u32) + param_offset);
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
            // Stage 18.332 (P1 soundness fix): For sret functions, store the
            // return value to the sret pointer (registered as "%_sret" in
            // emit_function_begin), then build `ret void`.
            //
            // Per System V AMD64 ABI §3.2.3 + rustc_codegen_llvm:
            // - sret functions return void; the result is written to the
            //   caller-provided sret pointer (passed as the first parameter).
            // - The store instruction writes the struct value to memory at
            //   the sret pointer's location.
            //
            // Per §1.0 原則 6 (通解 > 特解): one sret return path for all
            // > 16B struct returns. Matches TextEmitter's emit_ret.
            // Per §1.0 原則 4 (报错 > 静默): if val is None for an sret fn,
            // emit only `ret void` (the sret slot is uninitialized — UB but
            // better than crashing). This case should never happen in
            // practice (MIR Return terminator always has a value for non-void
            // return types — see codegen/terminator.rs).
            if ty.needs_sret() {
                if let Some(v) = val {
                    let v_ref = self.lookup(v);
                    if let Some(&sret_ptr) = self.values.get("%_sret") {
                        LLVMBuildStore(self.builder, v_ref, sret_ptr);
                    } else {
                        // Defensive: %_sret should always be present for sret
                        // functions. If not, the function signature was built
                        // incorrectly. Emit a no-op (the sret slot remains
                        // uninitialized) and continue with ret void.
                        // Per §1.0 原則 4 (报错 > 静默): eprintln for debugging.
                        if crate::session::debug_codegen_enabled() {
                            eprintln!("[CODEGEN] emit_ret: sret fn but %_sret not registered");
                        }
                    }
                }
                LLVMBuildRetVoid(self.builder);
            } else {
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
