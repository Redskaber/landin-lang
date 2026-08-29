//! Stage 16.77 MUV-1: `impl AggregateEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use super::helpers::{create_byval_attribute, create_sret_attribute, cstr_owned};
use crate::codegen::emitter::AggregateEmitter;
use crate::codegen::emitter::*;
use llvm_sys::core::*;
use llvm_sys::prelude::*;

use super::LLVMSysEmitter;

impl AggregateEmitter for LLVMSysEmitter {
    fn emit_call(
        &mut self,
        fn_name: &str,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue {
        // Stage 18.332 (P1 soundness fix): For struct return > 16 bytes,
        // use sret calling convention at the call site.
        //
        // Per System V AMD64 ABI §3.2.3 + rustc_codegen_llvm:
        // - Caller allocates a stack slot for the result.
        // - Pass the slot's pointer as the first arg with `sret(<ret_ty>)`.
        // - Callee writes the result to the sret pointer.
        // - Caller loads the result from the slot.
        //
        // Stage 18.333 (P1 soundness fix): For struct/array args > 16 bytes,
        // use byval calling convention at the call site.
        // - Caller allocates a stack slot for the arg.
        // - Stores the arg value to the slot.
        // - Passes the slot pointer with `byval(<arg_ty>)` attribute.
        // - Callee accesses the arg via the pointer (transparent to MIR).
        //
        // **Design boundary**:
        // - Mirrors TextEmitter's emit_call sret+byval paths (Stage 18.330/18.333).
        // - `declare_function` and `interpret_adhoc` ALSO build sret+byval
        //   signatures, so the callee's function type already matches.
        // - For indirect calls (fn_name starts with %), we still build the
        //   sret+byval signature at the call site and add the attributes to
        //   the call's args.
        //
        // Per §1.0 原則 6 (通解 > 特解): one sret+byval call path for all > 16B
        // struct/array returns and params.
        // Per §12 (最优 > 最小): root-cause fix at IR level.
        // Per §20 (iterative audit): same root cause as sret; same fix pattern.
        let use_sret = ret_ty.needs_sret();

        let arg_tys: Vec<EmitType> = args.iter().map(|(t, _)| t.clone()).collect();
        // Stage 14.58: Support indirect calls through function pointers.
        // When fn_name is an SSA value (starts with %), look it up as a
        // value instead of declaring a function.
        let callee = if fn_name.starts_with('%') || fn_name.starts_with('@') {
            self.lookup(&fn_name.to_string())
        } else {
            self.declare_function(fn_name, ret_ty, &arg_tys)
        };
        if crate::session::debug_codegen_enabled() {
            eprintln!(
                "[CODEGEN] emit_call: fn_name={} callee={:?} use_sret={}",
                fn_name, callee, use_sret
            );
        }
        unsafe {
            // Stage 18.228 (v0.2.5d): Coerce argument values to match the
            // declared arg types. Previously, `lookup(v)` for literal values
            // like "0" created i32 constants (via `interpret_adhoc`), but
            // functions like `__landin_panic_bounds_check` expect i64 args.
            // This caused "Call parameter type does not match function
            // signature!" LLVM verifier errors.
            //
            // Fix: after looking up each arg value, coerce it to the
            // declared arg type via LLVMBuildIntCast2 (for integer args).
            //
            // Per §1.0 原則 9 (正确>妥协): fix root cause (coerce in emit_call),
            // not symptom (change function signatures or use i32 everywhere).
            // Per §1.0 原則 6 (通解>特例): one coercion path for all integer
            // arg type mismatches.
            let user_param_llvm_tys: Vec<LLVMTypeRef> =
                args.iter().map(|(t, _)| self.llvm_type(t)).collect();
            let mut user_arg_vals: Vec<LLVMValueRef> = Vec::with_capacity(args.len());
            for (i, (_, v)) in args.iter().enumerate() {
                let raw = self.lookup(v);
                let target_ty = user_param_llvm_tys[i];
                let raw_ty = LLVMTypeOf(raw);
                if raw_ty == target_ty {
                    user_arg_vals.push(raw);
                } else {
                    let raw_kind = LLVMGetTypeKind(raw_ty);
                    let target_kind = LLVMGetTypeKind(target_ty);
                    if raw_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                        && target_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                    {
                        let name_c = cstr_owned("argcast");
                        let coerced = LLVMBuildIntCast2(
                            self.builder,
                            raw,
                            target_ty,
                            1, // signed
                            name_c.as_ptr(),
                        );
                        user_arg_vals.push(coerced);
                    } else {
                        // Non-integer type mismatch — pass as-is (LLVM will
                        // catch the error if it's truly invalid).
                        user_arg_vals.push(raw);
                    }
                }
            }

            // Stage 18.332: When use_sret, allocate the sret slot and
            // prepend it to the args list. The function type is also
            // built with the sret signature (void return + ptr param 0).
            //
            // **Critical**: Use `entry_block_alloca` instead of `LLVMBuildAlloca`
            // to ensure the alloca is hoisted to the entry block. Mid-function
            // allocas cause LLVM to emit dynamic stack adjustment patterns
            // (`mov %rsp, %r14; mov %rdi, %rsp`) that leak stack across
            // subsequent calls — causing intermittent segfaults under
            // multi-threaded test execution.
            let ret_llvm_ty = self.llvm_type(ret_ty);
            let sret_slot: Option<LLVMValueRef> = if use_sret {
                Some(self.entry_block_alloca(ret_llvm_ty, "sret_slot"))
            } else {
                None
            };

            // Stage 18.333: For each byval-eligible user arg, allocate a slot
            // via entry_block_alloca, store the arg value to the slot, and
            // pass the slot pointer instead of the value. The LLVM param
            // type is `ptr` (not the original struct type), and a `byval(<orig_ty>)`
            // attribute is added at the call site.
            //
            // Per §1.0 原則 6 (通解 > 特解): same entry_block_alloca pattern as sret.
            // Per §20: same root cause as sret; same fix pattern.
            let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
            let mut byval_infos: Vec<(usize, LLVMTypeRef)> = Vec::new(); // (user_idx, orig_llvm_ty)
            let mut final_user_param_tys: Vec<LLVMTypeRef> = Vec::with_capacity(args.len());
            let mut final_user_arg_vals: Vec<LLVMValueRef> = Vec::with_capacity(args.len());
            for (i, (t, _)) in args.iter().enumerate() {
                let orig_llvm_ty = user_param_llvm_tys[i];
                if t.needs_byval() {
                    byval_infos.push((i, orig_llvm_ty));
                    // Allocate slot in entry block (same pattern as sret_slot).
                    let slot_name = format!("byval_arg{}_slot", i);
                    let slot = self.entry_block_alloca(orig_llvm_ty, &slot_name);
                    // Store the arg value to the slot.
                    LLVMBuildStore(self.builder, user_arg_vals[i], slot);
                    // Pass the slot pointer instead of the value.
                    final_user_arg_vals.push(slot);
                    final_user_param_tys.push(ptr_ty);
                } else {
                    final_user_arg_vals.push(user_arg_vals[i]);
                    final_user_param_tys.push(orig_llvm_ty);
                }
            }

            // Build the final args list (sret slot first if applicable).
            let mut all_arg_vals: Vec<LLVMValueRef> =
                Vec::with_capacity(final_user_arg_vals.len() + if use_sret { 1 } else { 0 });
            if let Some(slot) = sret_slot {
                all_arg_vals.push(slot);
            }
            all_arg_vals.extend(final_user_arg_vals.iter().copied());

            // Build the function type matching the callee's signature.
            // For sret: void (ptr sret, ...user_params_with_byval_replacements).
            // For non-sret: <ret_ty> (...user_params_with_byval_replacements).
            // For variadic printf: i32 (..., ...).
            // Stage 18.334 (P1 soundness fix): Replace hardcoded name-list with
            // set lookup. The set is populated by `emit_declare` from the
            // signature text. Per §1.0 原則 6 (通解 > 特解): variadicity is a
            // property of the signature, not the function name.
            // Per §20 (iterative audit): the name-list was a workaround.
            let is_variadic: i32 = if self.variadic_fns.contains(fn_name) {
                1
            } else {
                0
            };
            let (call_ret_ty, all_param_tys): (LLVMTypeRef, Vec<LLVMTypeRef>) = if use_sret {
                let void_ty = LLVMVoidTypeInContext(self.ctx);
                let mut sret_param_tys: Vec<LLVMTypeRef> = vec![ptr_ty];
                sret_param_tys.extend(final_user_param_tys.iter().copied());
                (void_ty, sret_param_tys)
            } else {
                (ret_llvm_ty, final_user_param_tys.clone())
            };
            let fty = LLVMFunctionType(
                call_ret_ty,
                all_param_tys.as_ptr() as *mut LLVMTypeRef,
                all_param_tys.len() as u32,
                is_variadic,
            );
            // Stage 14.44: For void-returning calls (including sret calls,
            // which return void), pass an EMPTY name string to LLVMBuildCall2.
            // Was: always passed "call" as the name, which caused
            // "Instruction has a name, but provides a void value" verifier
            // error for calls to void functions (e.g., __landin_panic_overflow).
            let name_c = if *ret_ty == EmitType::Void || use_sret {
                cstr_owned("")
            } else {
                cstr_owned("call")
            };
            let v = LLVMBuildCall2(
                self.builder,
                fty,
                callee,
                all_arg_vals.as_mut_ptr(),
                all_arg_vals.len() as u32,
                name_c.as_ptr(),
            );

            // Stage 18.332: Add sret attribute to the call site's first arg.
            // This is required for LLVM to know that the first arg is a sret
            // pointer (not a regular ptr). Without this, the call site ABI
            // mismatches the function declaration's ABI.
            // For indirect calls (function pointer), the call site attribute
            // is the ONLY way to convey sret (the function pointer type itself
            // doesn't carry attribute info in opaque pointer mode).
            if use_sret {
                let sret_attr = create_sret_attribute(self.ctx, ret_llvm_ty);
                LLVMAddCallSiteAttribute(v, 1, sret_attr);
            }

            // Stage 18.333: Add byval attribute to each byval-eligible user arg
            // at the call site. The LLVM arg index for user arg `i` is
            // `i + 1 + (1 if use_sret else 0)` (LLVM 1-indexed, +1 for sret if active).
            let sret_offset: u32 = if use_sret { 1 } else { 0 };
            for (user_idx, orig_llvm_ty) in &byval_infos {
                let llvm_arg_idx = (*user_idx as u32) + 1 + sret_offset;
                let byval_attr = create_byval_attribute(self.ctx, *orig_llvm_ty);
                LLVMAddCallSiteAttribute(v, llvm_arg_idx, byval_attr);
            }

            if *ret_ty == EmitType::Void {
                // Don't register a name for void calls — return "0" sentinel.
                "0".to_string()
            } else if use_sret {
                // Load the result from the sret slot. The callee has written
                // the struct value to `sret_slot` (param 0 of the call).
                let load_name = cstr_owned("sret_load");
                let loaded = LLVMBuildLoad2(
                    self.builder,
                    ret_llvm_ty,
                    // Guarded by `use_sret` branch: sret_slot is Some.
                    sret_slot.expect("use_sret => sret_slot is Some"),
                    load_name.as_ptr(),
                );
                self.fresh_named(loaded)
            } else {
                self.fresh_named(v)
            }
        }
    }

    fn emit_dyn_trait_method_call(
        &mut self,
        dynptr_symbol: &str,
        slot_index: u32,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue {
        // Stage 14.13 (GAP-30): Implement dyn Trait method dispatch via
        // vtable indirect call. The dynptr global is `{ ptr, ptr }` where
        // field 0 = data pointer, field 1 = vtable pointer. The vtable is
        // `[N x ptr]` where slot_index selects the method function pointer.
        //
        // LLVM IR sequence (mirrors TextEmitter's reference implementation):
        //   %gep_vtable = getelementptr { ptr, ptr }, ptr @dynptr, i32 0, i32 1
        //   %vtable     = load ptr, ptr %gep_vtable
        //   %gep_method = getelementptr [N x ptr], ptr %vtable, i32 0, i32 slot_index
        //   %method_fn  = load ptr, ptr %gep_method
        //   %result     = call <ret_ty> %method_fn(<args>)
        //
        // Note: We use the opaque pointer mode (ptr) for all GEPs and loads,
        // matching LLVM 15+ opaque pointer semantics. The dynptr global must
        // already exist in the module (emitted by emit_dyn_trait_ptrs before
        // codegen_from_mir — see codegen_crate_to_module reorder).
        unsafe {
            let dynptr_name_c = cstr_owned(dynptr_symbol);
            let dynptr = LLVMGetNamedGlobal(self.module, dynptr_name_c.as_ptr());
            if dynptr.is_null() {
                // Graceful degradation: if the dynptr global doesn't exist
                // (e.g., trait resolver didn't build a vtable for this pair),
                // emit a zero-valued result instead of panicking. This
                // prevents the compiler from crashing on programs that use
                // dyn Trait but have a resolver gap. The program will produce
                // wrong results but will compile and link.
                let ret_llvm_ty = self.llvm_type(ret_ty);
                let zero = LLVMConstInt(ret_llvm_ty, 0, 1);
                return self.fresh_named(zero);
            }

            // 1. GEP to get the vtable pointer slot (field 1 of {ptr, ptr}).
            let fat_ptr_ty = self.llvm_type(&EmitType::Struct(vec![
                EmitType::OpaquePtr,
                EmitType::OpaquePtr,
            ]));
            let zero = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), 0, 0);
            let one = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), 1, 0);
            let mut vtable_indices = [zero, one];
            let gep_name = cstr_owned("gep_vtable");
            let gep_vtable = LLVMBuildInBoundsGEP2(
                self.builder,
                fat_ptr_ty,
                dynptr,
                vtable_indices.as_mut_ptr(),
                vtable_indices.len() as u32,
                gep_name.as_ptr(),
            );

            // 2. Load the vtable pointer.
            let opaque_ptr_ty = self.llvm_type(&EmitType::OpaquePtr);
            let load_vtable_name = cstr_owned("vtable");
            let vtable = LLVMBuildLoad2(
                self.builder,
                opaque_ptr_ty,
                gep_vtable,
                load_vtable_name.as_ptr(),
            );

            // 3. GEP to get the method function pointer slot (slot_index of [N x ptr]).
            // Stage 18.68: Use the vtable's actual element type (ptr) for the GEP.
            // Previously used `opaque_ptr_ty` as the element type, which is correct
            // for opaque pointers in LLVM 15+, but LLVM verification fails when
            // the GEP's element type doesn't match the pointer's pointee type in
            // typed pointer mode (LLVM < 15 compatibility). Using `opaque_ptr_ty`
            // (which is `ptr` in LLVM 19) is actually correct for opaque pointers,
            // but the indices `[0, slot_index]` are wrong — for a `[N x ptr]` array
            // accessed via `ptr`, the correct GEP is `getelementptr ptr, ptr %vtable, i32 slot_index`
            // (single index, not `[0, slot_index]`).
            //
            // Per §1.0 原則 9 "正确 > 妥协": fix the GEP indices.
            let slot_idx = LLVMConstInt(LLVMInt32TypeInContext(self.ctx), slot_index as u64, 0);
            let mut method_indices = [slot_idx]; // single index into [N x ptr]
            let gep_method_name = cstr_owned("gep_method");
            let gep_method = LLVMBuildInBoundsGEP2(
                self.builder,
                opaque_ptr_ty, // element type is ptr (opaque pointer mode)
                vtable,
                method_indices.as_mut_ptr(),
                method_indices.len() as u32,
                gep_method_name.as_ptr(),
            );

            // 4. Load the method function pointer.
            let load_method_name = cstr_owned("method_fn");
            let method_fn = LLVMBuildLoad2(
                self.builder,
                opaque_ptr_ty,
                gep_method,
                load_method_name.as_ptr(),
            );

            // 5. Build the function type from arg types + return type.
            //
            // Stage 18.332 (P1 soundness fix): For struct return > 16 bytes,
            // use sret at the indirect call site. The method function pointer
            // loaded from the vtable is `ptr` (opaque), so the call site
            // attribute is the ONLY way to convey sret to LLVM.
            //
            // Stage 18.333 (P1 soundness fix): For struct/array args > 16 bytes,
            // use byval at the indirect call site (same pattern as emit_call).
            //
            // Per §20 (iterative audit): "发现一个 bug 意味着存在大量类似 bug"
            // — found via auditing all emit_call paths after the direct-call
            // sret fix. Same root cause: missing sret+byval ABI handling.
            // Per §1.0 原則 6 (通解 > 特解): same sret+byval path as emit_call.
            let ret_llvm_ty = self.llvm_type(ret_ty);
            let use_sret = ret_ty.needs_sret();
            let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
            let mut byval_infos: Vec<(usize, LLVMTypeRef)> = Vec::new();
            let mut final_user_param_tys: Vec<LLVMTypeRef> = Vec::with_capacity(args.len());
            for (i, (t, _)) in args.iter().enumerate() {
                let orig_llvm_ty = self.llvm_type(t);
                if t.needs_byval() {
                    byval_infos.push((i, orig_llvm_ty));
                    final_user_param_tys.push(ptr_ty);
                } else {
                    final_user_param_tys.push(orig_llvm_ty);
                }
            }
            let (call_ret_ty, all_param_tys): (LLVMTypeRef, Vec<LLVMTypeRef>) = if use_sret {
                let void_ty = LLVMVoidTypeInContext(self.ctx);
                let mut sret_params: Vec<LLVMTypeRef> = vec![ptr_ty];
                sret_params.extend(final_user_param_tys.iter().copied());
                (void_ty, sret_params)
            } else {
                (ret_llvm_ty, final_user_param_tys.clone())
            };
            let fty = LLVMFunctionType(
                call_ret_ty,
                all_param_tys.as_ptr() as *mut LLVMTypeRef,
                all_param_tys.len() as u32,
                0, // not variadic
            );

            // 6. Build args list (sret slot prepended if use_sret; byval args
            //    replaced with their slot pointers).
            // Stage 18.332: Use entry_block_alloca (not LLVMBuildAlloca) to
            // hoist the alloca to the entry block. Same rationale as emit_call:
            // mid-function allocas produce fragile dynamic stack adjustments.
            // Stage 18.333: Same entry_block_alloca pattern for byval slots.
            let sret_slot: Option<LLVMValueRef> = if use_sret {
                Some(self.entry_block_alloca(ret_llvm_ty, "dyncall_sret_slot"))
            } else {
                None
            };
            let mut all_arg_vals: Vec<LLVMValueRef> =
                Vec::with_capacity(args.len() + if use_sret { 1 } else { 0 });
            if let Some(slot) = sret_slot {
                all_arg_vals.push(slot);
            }
            let user_arg_vals: Vec<LLVMValueRef> =
                args.iter().map(|(_, v)| self.lookup(v)).collect();
            for (i, (_, _)) in args.iter().enumerate() {
                if args[i].0.needs_byval() {
                    // byval: store value to slot, pass slot pointer.
                    let slot_name = format!("dyncall_byval_arg{}_slot", i);
                    let orig_llvm_ty = final_user_param_tys[i]; // already `ptr` here; use byval_infos
                    let _ = orig_llvm_ty;
                    let orig_ty = byval_infos
                        .iter()
                        .find(|(idx, _)| *idx == i)
                        .map(|(_, t)| *t);
                    if let Some(orig_llvm_ty) = orig_ty {
                        let slot = self.entry_block_alloca(orig_llvm_ty, &slot_name);
                        LLVMBuildStore(self.builder, user_arg_vals[i], slot);
                        all_arg_vals.push(slot);
                    } else {
                        // Defensive: shouldn't happen — byval_infos was built from
                        // the same iteration. Per §1.0 原則 4 (报错 > 静默): fall
                        // through to passing the value as-is (LLVM will catch
                        // the type mismatch if any).
                        all_arg_vals.push(user_arg_vals[i]);
                    }
                } else {
                    all_arg_vals.push(user_arg_vals[i]);
                }
            }

            // 7. Call the loaded function pointer (indirect call).
            let call_name = if use_sret || *ret_ty == EmitType::Void {
                cstr_owned("")
            } else {
                cstr_owned("dyncall")
            };
            let call_val = LLVMBuildCall2(
                self.builder,
                fty,
                method_fn,
                all_arg_vals.as_mut_ptr(),
                all_arg_vals.len() as u32,
                call_name.as_ptr(),
            );

            // 8. Add sret attribute to call site param 1 (the sret slot).
            if use_sret {
                let sret_attr = create_sret_attribute(self.ctx, ret_llvm_ty);
                LLVMAddCallSiteAttribute(call_val, 1, sret_attr);
            }

            // Stage 18.333: Add byval attribute to each byval-eligible user arg
            // at the call site (mirrors emit_call's byval attribute loop).
            let sret_offset: u32 = if use_sret { 1 } else { 0 };
            for (user_idx, orig_llvm_ty) in &byval_infos {
                let llvm_arg_idx = (*user_idx as u32) + 1 + sret_offset;
                let byval_attr = create_byval_attribute(self.ctx, *orig_llvm_ty);
                LLVMAddCallSiteAttribute(call_val, llvm_arg_idx, byval_attr);
            }

            if *ret_ty == EmitType::Void {
                // Don't register a name for void calls — return "0" sentinel.
                "0".to_string()
            } else if use_sret {
                // Load the result from the sret slot.
                let load_name = cstr_owned("dyncall_sret_load");
                let loaded = LLVMBuildLoad2(
                    self.builder,
                    ret_llvm_ty,
                    // Guarded by `use_sret` branch: sret_slot is Some.
                    sret_slot.expect("use_sret => sret_slot is Some"),
                    load_name.as_ptr(),
                );
                self.fresh_named(loaded)
            } else {
                self.fresh_named(call_val)
            }
        }
    }

    fn emit_phi(&mut self, ty: &EmitType, incoming: &[(EmitValue, String)]) -> EmitValue {
        unsafe {
            let llvm_ty = self.llvm_type(ty);
            let name_c = cstr_owned("phi");
            let phi = LLVMBuildPhi(self.builder, llvm_ty, name_c.as_ptr());
            let vals: Vec<LLVMValueRef> = incoming.iter().map(|(v, _)| self.lookup(v)).collect();
            let blocks: Vec<LLVMBasicBlockRef> = incoming
                .iter()
                .map(|(_, lbl)| self.block_for(lbl))
                .collect();
            LLVMAddIncoming(
                phi,
                vals.as_ptr() as *mut LLVMValueRef,
                blocks.as_ptr() as *mut LLVMBasicBlockRef,
                incoming.len() as u32,
            );
            self.fresh_named(phi)
        }
    }

    fn emit_insertvalue(
        &mut self,
        agg_ty: &EmitType,
        agg: &EmitValue,
        val_ty: &EmitType,
        val: &EmitValue,
        index: u32,
    ) -> EmitValue {
        // Stage 13.5 MUV-2: emit_insertvalue is called for two cases:
        // 1. Constructing &str fat pointers (from `codegen_operand`) — `agg`
        //    is "undef" (textual), `val` is a GEP-text string. We stub these
        //    with `undef` of the aggregate type.
        // 2. Building aggregate values from real LLVM values — handled by
        //    `LLVMBuildInsertValue`.
        let _ = val_ty;
        unsafe {
            let agg_v = self.lookup(agg);
            let mut val_v = self.lookup(val);
            let llvm_agg_ty = self.llvm_type(agg_ty);
            // If agg is the textual "undef" sentinel, build a fresh undef.
            let agg_real = if agg == "undef" {
                LLVMGetUndef(llvm_agg_ty)
            } else {
                agg_v
            };

            // Stage 14.70: Coerce val_v to the field's type.
            //
            // `interpret_adhoc` parses integer literals as i32 (default).
            // When inserting into an i64 field (e.g., fat pointer's len),
            // the i32 value must be cast to i64. Without this, LLVM stores
            // only 4 bytes (movl) instead of 8 bytes (movq), leaving the
            // upper 4 bytes as stack garbage — causing corrupted lengths
            // on subsequent function calls.
            //
            // Per §1.0 原则 5 "报错 > 静默": explicit cast prevents silent
            // stack garbage corruption.
            let field_ty = {
                let kind = LLVMGetTypeKind(llvm_agg_ty);
                if kind == llvm_sys::LLVMTypeKind::LLVMStructTypeKind {
                    let count = LLVMCountStructElementTypes(llvm_agg_ty);
                    if index < count {
                        let mut types: Vec<LLVMTypeRef> =
                            vec![std::ptr::null_mut(); count as usize];
                        LLVMGetStructElementTypes(llvm_agg_ty, types.as_mut_ptr());
                        types[index as usize]
                    } else {
                        std::ptr::null_mut()
                    }
                } else {
                    std::ptr::null_mut()
                }
            };
            if !field_ty.is_null() {
                let val_kind = LLVMGetTypeKind(LLVMTypeOf(val_v));
                let field_kind = LLVMGetTypeKind(field_ty);
                if val_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                    && field_kind == llvm_sys::LLVMTypeKind::LLVMIntegerTypeKind
                {
                    let val_width = LLVMGetIntTypeWidth(LLVMTypeOf(val_v));
                    let field_width = LLVMGetIntTypeWidth(field_ty);
                    if val_width != field_width {
                        let name_c = cstr_owned("icast");
                        val_v = LLVMBuildIntCast2(
                            self.builder,
                            val_v,
                            field_ty,
                            1, // signed
                            name_c.as_ptr(),
                        );
                    }
                }
            }

            let name_c = cstr_owned("iv");
            let r = LLVMBuildInsertValue(self.builder, agg_real, val_v, index, name_c.as_ptr());
            self.fresh_named(r)
        }
    }

    fn emit_extractvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue, index: u32) -> EmitValue {
        unsafe {
            let agg_v = self.lookup(agg);
            let _ = self.llvm_type(agg_ty); // for type-context (not used by API)
            let name_c = cstr_owned("ev");
            let r = LLVMBuildExtractValue(self.builder, agg_v, index, name_c.as_ptr());
            self.fresh_named(r)
        }
    }
}
