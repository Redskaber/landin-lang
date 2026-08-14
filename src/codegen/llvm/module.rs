//! Stage 16.77 MUV-1: `impl ModuleEmitter for LLVMSysEmitter`.
//!
//! Extracted from `llvm/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::ModuleEmitter;
use crate::codegen::emitter::*;
use llvm_sys::core::*;
use llvm_sys::prelude::*;
use std::ffi::CString;

use super::helpers::*;
use super::LLVMSysEmitter;

impl ModuleEmitter for LLVMSysEmitter {
    fn emit_header(&mut self) {
        unsafe {
            let triple = cstr_owned("x86_64-unknown-linux-gnu");
            LLVMSetTarget(self.module, triple.as_ptr());
            let dl = CString::new(
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128",
            )
            .unwrap();
            LLVMSetDataLayout(self.module, dl.as_ptr());
        }
    }

    fn emit_declare(&mut self, signature: &str) {
        // `signature` looks like `void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)`.
        // We don't fully parse the LLVM IR text here — instead we parse the
        // name + arg-count conservatively and emit an extern declaration.
        // MUV-2: pragmatic — many declarations repeat across crates.
        if let Some(name) = parse_declare_name(signature) {
            // Heuristic: count commas in the parens for arg count.
            let arg_count = count_args_in_signature(signature);
            let arg_tys: Vec<EmitType> = (0..arg_count).map(|_| EmitType::I32).collect();
            // Determine return type from the leading token (void or i32).
            let ret_ty = if signature.trim_start().starts_with("void") {
                EmitType::Void
            } else {
                EmitType::I32
            };
            self.get_or_declare_function(&name, &ret_ty, &arg_tys);
        }
    }

    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue {
        // Emit a module-level global string constant, return its name.
        // Matches TextEmitter semantics: name is ".str.N" (no leading '@').
        let name = format!(".str.{}", self.next_str);
        self.next_str += 1;
        unsafe {
            let array_ty = LLVMArrayType2(LLVMInt8TypeInContext(self.ctx), bytes.len() as u64);
            let name_c = cstr_owned(name.as_str());
            let global = LLVMAddGlobal(self.module, array_ty, name_c.as_ptr());
            // Initialiser: LLVMConstString adds a null terminator by default;
            // we use the in-context variant with DontNullTerminate=1 to match
            // the byte count exactly.
            let init = LLVMConstStringInContext2(
                self.ctx,
                bytes.as_ptr() as *const std::os::raw::c_char,
                bytes.len(),
                1,
            );
            LLVMSetInitializer(global, init);
            LLVMSetLinkage(global, llvm_sys::LLVMLinkage::LLVMPrivateLinkage);
            LLVMSetUnnamedAddress(global, llvm_sys::LLVMUnnamedAddr::LLVMGlobalUnnamedAddr);
            LLVMSetGlobalConstant(global, 1);
            // Register the global's *pointer* under its name so callers
            // can reference it directly.
            self.values.insert(name.clone(), global);
        }
        name
    }

    fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue {
        // Stage 14.13 (GAP-30): Emit `[N x ptr]` global with each method
        // symbol resolved to a real function pointer. Previously (MUV-2)
        // these were null pointers, causing dyn Trait method calls to
        // segfault at runtime. Now we resolve each symbol name (e.g.
        // `landin_S_hello`) via LLVMGetNamedFunction — the function must
        // already be defined in the module (codegen_from_mir emits all
        // user functions first, then vtables are emitted).
        //
        // Symbols that are the literal string "null" (missing slots in
        // stdlib traits) remain null pointers.
        unsafe {
            let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
            let array_ty = LLVMArrayType2(ptr_ty, method_symbols.len() as u64);
            let name_c = cstr_owned(global_name);
            let global = LLVMAddGlobal(self.module, array_ty, name_c.as_ptr());
            // Build a constant array — resolve each symbol to a function
            // pointer, or use null for "null" / unresolvable symbols.
            let entries: Vec<LLVMValueRef> = method_symbols
                .iter()
                .map(|sym| {
                    if sym == "null" {
                        LLVMConstNull(ptr_ty)
                    } else {
                        // Try to look up the function in the module.
                        let sym_c = cstr_owned(sym.as_str());
                        let func = LLVMGetNamedFunction(self.module, sym_c.as_ptr());
                        if func.is_null() {
                            // Stage 14.92 (Bug X3 complete fix): Function not
                            // yet defined — declare it using the correct
                            // signature from fn_sigs if available, or fall back
                            // to a generic ptr-taking i32-returning function.
                            //
                            // Previously (Stage 14.13), this created a
                            // declaration with `i32(void)` — 0 args. This
                            // caused emit_function_begin to find a mismatch
                            // (0 args vs N args) and create a duplicate (.1).
                            //
                            // Fix: use fn_sigs to get the correct signature.
                            // If fn_sigs doesn't have it, use a generic
                            // `i32(ptr)` — most trait methods take &self (ptr).
                            let (ret_ty, param_tys) = self
                                .fn_sigs
                                .get(sym)
                                .cloned()
                                .unwrap_or((EmitType::I32, vec![EmitType::OpaquePtr]));
                            let ret_llvm_ty = self.llvm_type(&ret_ty);
                            let param_llvm_tys: Vec<LLVMTypeRef> =
                                param_tys.iter().map(|t| self.llvm_type(t)).collect();
                            let fty = LLVMFunctionType(
                                ret_llvm_ty,
                                param_llvm_tys.as_ptr() as *mut LLVMTypeRef,
                                param_llvm_tys.len() as u32,
                                0,
                            );
                            let fwd = LLVMAddFunction(self.module, sym_c.as_ptr(), fty);
                            self.declared.insert(sym.clone(), fwd);
                            fwd
                        } else {
                            func
                        }
                    }
                })
                .collect();
            let init = LLVMConstArray2(
                ptr_ty,
                entries.as_ptr() as *mut LLVMValueRef,
                method_symbols.len() as u64,
            );
            LLVMSetInitializer(global, init);
            LLVMSetLinkage(global, llvm_sys::LLVMLinkage::LLVMPrivateLinkage);
            LLVMSetUnnamedAddress(global, llvm_sys::LLVMUnnamedAddr::LLVMGlobalUnnamedAddr);
            LLVMSetGlobalConstant(global, 1);
            self.values.insert(global_name.to_string(), global);
        }
        global_name.to_string()
    }

    fn emit_dyn_trait_const(
        &mut self,
        global_name: &str,
        data_symbol: &str,
        vtable_symbol: &str,
    ) -> EmitValue {
        // Stage 14.13 (GAP-30): Emit `{ ptr, ptr }` global with real data
        // and vtable pointers. Previously (MUV-2) both were null, causing
        // dyn Trait method calls to segfault. Now we resolve the symbols:
        //   - data_symbol (e.g. `.data.S`) — references a per-type data
        //     global. We emit it as a global zero-initialized struct if it
        //     doesn't exist yet (placeholder for the actual instance data).
        //   - vtable_symbol (e.g. `.vtable.Greet.S`) — references the
        //     vtable global emitted by emit_vtable_global above.
        unsafe {
            let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
            let fields = [ptr_ty, ptr_ty];
            let struct_ty =
                LLVMStructTypeInContext(self.ctx, fields.as_ptr() as *mut LLVMTypeRef, 2, 0);
            let name_c = cstr_owned(global_name);
            let global = LLVMAddGlobal(self.module, struct_ty, name_c.as_ptr());

            // Resolve vtable symbol — look up the existing vtable global.
            let vtable_ptr = {
                let vtable_c = cstr_owned(vtable_symbol);
                let vtable_global = LLVMGetNamedGlobal(self.module, vtable_c.as_ptr());
                if vtable_global.is_null() {
                    // Vtable not yet emitted — declare as external global.
                    let extern_global = LLVMAddGlobal(self.module, struct_ty, vtable_c.as_ptr());
                    LLVMSetLinkage(extern_global, llvm_sys::LLVMLinkage::LLVMExternalLinkage);
                    extern_global
                } else {
                    vtable_global
                }
            };

            // Resolve data symbol — emit a zero-initialized data global if
            // it doesn't exist. This is a placeholder; real instance data
            // would come from the actual struct value (future work).
            let data_ptr = {
                let data_c = cstr_owned(data_symbol);
                let existing = LLVMGetNamedGlobal(self.module, data_c.as_ptr());
                if existing.is_null() {
                    // Create a zero-initialized i8 global as placeholder.
                    let i8_ty = LLVMInt8TypeInContext(self.ctx);
                    let data_global = LLVMAddGlobal(self.module, i8_ty, data_c.as_ptr());
                    let zero = LLVMConstInt(i8_ty, 0, 0);
                    LLVMSetInitializer(data_global, zero);
                    LLVMSetLinkage(data_global, llvm_sys::LLVMLinkage::LLVMPrivateLinkage);
                    data_global
                } else {
                    existing
                }
            };

            // Cast both to opaque ptr for the struct initializer.
            let data_val = LLVMConstBitCast(data_ptr, ptr_ty);
            let vtable_val = LLVMConstBitCast(vtable_ptr, ptr_ty);
            let inits = [data_val, vtable_val];
            let init =
                LLVMConstStructInContext(self.ctx, inits.as_ptr() as *mut LLVMValueRef, 2, 0);
            LLVMSetInitializer(global, init);
            LLVMSetLinkage(global, llvm_sys::LLVMLinkage::LLVMPrivateLinkage);
            LLVMSetUnnamedAddress(global, llvm_sys::LLVMUnnamedAddr::LLVMGlobalUnnamedAddr);
            LLVMSetGlobalConstant(global, 1);
            self.values.insert(global_name.to_string(), global);
        }
        global_name.to_string()
    }
}
