//! Stage 16.77 MUV-2: `impl AggregateEmitter for TextEmitter`.
//!
//! Extracted from `text/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::*;

use super::emit_type_to_llvm_str;
use super::TextEmitter;

impl AggregateEmitter for TextEmitter {
    fn emit_phi(&mut self, ty: &EmitType, incoming: &[(EmitValue, String)]) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        let incoming_str = incoming
            .iter()
            .map(|(val, label)| format!("[ {}, %{} ]", val, label))
            .collect::<Vec<_>>()
            .join(", ");
        self.line(&format!("  %v{} = phi {} {}", r, ty_str, incoming_str));
        format!("%v{}", r)
    }

    fn emit_insertvalue(
        &mut self,
        agg_ty: &EmitType,
        agg: &EmitValue,
        val_ty: &EmitType,
        val: &EmitValue,
        index: u32,
    ) -> EmitValue {
        let r = self.fresh();
        let agg_str = emit_type_to_llvm_str(agg_ty);
        let val_str = emit_type_to_llvm_str(val_ty);
        self.line(&format!(
            "  %v{} = insertvalue {} {}, {} {}, {}",
            r, agg_str, agg, val_str, val, index
        ));
        format!("%v{}", r)
    }

    fn emit_extractvalue(&mut self, agg_ty: &EmitType, agg: &EmitValue, index: u32) -> EmitValue {
        let r = self.fresh();
        let agg_str = emit_type_to_llvm_str(agg_ty);
        self.line(&format!(
            "  %v{} = extractvalue {} {}, {}",
            r, agg_str, agg, index
        ));
        format!("%v{}", r)
    }

    fn emit_call(
        &mut self,
        fn_name: &str,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue {
        let r = self.fresh();
        let call_target = if fn_name.starts_with('%') || fn_name.starts_with('@') {
            fn_name.to_string()
        } else {
            format!("@{}", fn_name)
        };

        // Stage 18.330 (P1 soundness fix): For struct return > 16 bytes,
        // use sret calling convention: allocate result on stack, pass
        // sret pointer as first arg, call void, return the sret pointer.
        //
        // **Design boundary** (per System V ABI + rustc_codegen_llvm):
        // - ret_ty.needs_sret() → alloca result, pass ptr sret, call void
        // - Otherwise → normal `call <ret_ty> @fn(args)`
        //
        // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
        //
        // Stage 18.333 (P1 soundness fix): For struct/array args > 16 bytes,
        // use byval: alloca a slot, store the arg, pass the slot pointer with
        // `byval(<orig_ty>)` attribute (mirrors sret for params).
        // Per §20 (iterative audit): same root cause as sret; same fix pattern.
        // Per §1.0 原則 6 (通解 > 特解): one byval path for direct + indirect calls.
        let use_sret = ret_ty.needs_sret();

        // Build the sret slot if use_sret.
        let sret_name: Option<String> = if use_sret {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            let name = format!("%sret_{}", r);
            self.line(&format!("  {} = alloca {}", name, ret_str));
            Some(name)
        } else {
            None
        };

        // Build the byval slots and final args list.
        let mut all_args: Vec<String> = Vec::with_capacity(args.len() + 1);
        if let Some(ref sret) = sret_name {
            all_args.push(format!("ptr {}", sret));
        }
        for (i, (ty, a)) in args.iter().enumerate() {
            if ty.needs_byval() {
                let ty_str = emit_type_to_llvm_str(ty);
                let slot_name = format!("%byval_arg{}_{}", i, r);
                self.line(&format!("  {} = alloca {}", slot_name, ty_str));
                self.line(&format!("  store {} {}, ptr {}", ty_str, a, slot_name));
                all_args.push(format!("ptr byval({}) {}", ty_str, slot_name));
            } else {
                all_args.push(format!("{} {}", emit_type_to_llvm_str(ty), a));
            }
        }

        if use_sret {
            self.line(&format!(
                "  call void {}({})",
                call_target,
                all_args.join(", ")
            ));
            // Return the sret pointer — callers use it to access the result.
            sret_name.unwrap()
        } else if *ret_ty == EmitType::Void {
            self.line(&format!(
                "  call void {}({})",
                call_target,
                all_args.join(", ")
            ));
            "0".to_string()
        } else {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            self.line(&format!(
                "  %v{} = call {} {}({})",
                r,
                ret_str,
                call_target,
                all_args.join(", ")
            ));
            format!("%v{}", r)
        }
    }

    /// Stage 5.79: Emit a dyn Trait vtable indirect call.
    ///
    /// Three LLVM instructions:
    /// 1. `%vN = getelementptr { ptr, ptr }, ptr @<dynptr_symbol>, i32 0, i32 1`
    ///    — get vtable pointer slot (second field of the dynptr global)
    /// 2. `%vN+1 = load ptr, ptr %vN` — load the vtable pointer
    /// 3. `%vN+2 = load ptr, ptr %vN+1, i32 <slot_index>` — load the method fn ptr
    /// 4. `%vN+3 = call <ret_ty> %vN+2(<args>)` — indirect call
    ///
    /// Per §16 + Stage 5.78 marker convention: this method is invoked
    /// when codegen detects a `TerminatorKind::Call` whose `func` is
    /// `Operand::Constant(Const { ty: Error, val: Int(index) })` where
    /// `index < mir.dyn_trait_calls.len()`.
    fn emit_dyn_trait_method_call(
        &mut self,
        dynptr_symbol: &str,
        slot_index: u32,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue {
        // 1. Get the vtable pointer slot from the dynptr global.
        //    dynptr global is `{ ptr, ptr }` — first field is data ptr,
        //    second field (index 1) is vtable ptr.
        let gep_r = self.fresh();
        self.line(&format!(
            "  %v{gep_r} = getelementptr {{ ptr, ptr }}, ptr @{dynptr_symbol}, i32 0, i32 1"
        ));

        // 2. Load the vtable pointer.
        let vtable_r = self.fresh();
        self.line(&format!("  %v{vtable_r} = load ptr, ptr %v{gep_r}"));

        // 3. Load the method function pointer from the vtable at slot_index.
        //    The vtable is laid out as `[ptr; N]` — slot_index is the array index.
        let method_fn_r = self.fresh();
        self.line(&format!(
            "  %v{method_fn_r} = load ptr, ptr %v{vtable_r}, i32 {slot_index}"
        ));

        // Stage 18.332 (P1 soundness fix): For struct return > 16 bytes,
        // use sret at the indirect call site. Mirrors TextEmitter::emit_call
        // sret path (Stage 18.330) and LLVMSysEmitter's emit_dyn_trait_method_call.
        //
        // Stage 18.333 (P1 soundness fix): For struct/array args > 16 bytes,
        // use byval at the indirect call site (mirrors emit_call's byval path).
        //
        // Per §20 (iterative audit): same root cause as sret; same fix pattern.
        // Per §1.0 原則 6 (通解 > 特解): same sret+byval path for direct + indirect calls.
        let r = self.fresh();
        let use_sret = ret_ty.needs_sret();

        // Build sret slot if use_sret.
        let sret_name: Option<String> = if use_sret {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            let name = format!("%sret_{}", r);
            self.line(&format!("  {} = alloca {}", name, ret_str));
            Some(name)
        } else {
            None
        };

        // Build args list (sret pointer first, then user args with byval replacement).
        let mut all_args: Vec<String> = Vec::with_capacity(args.len() + 1);
        if let Some(ref sret) = sret_name {
            all_args.push(format!("ptr {}", sret));
        }
        for (i, (ty, a)) in args.iter().enumerate() {
            if ty.needs_byval() {
                let ty_str = emit_type_to_llvm_str(ty);
                let slot_name = format!("%dyncall_byval_arg{}_{}", i, r);
                self.line(&format!("  {} = alloca {}", slot_name, ty_str));
                self.line(&format!("  store {} {}, ptr {}", ty_str, a, slot_name));
                all_args.push(format!("ptr byval({}) {}", ty_str, slot_name));
            } else {
                all_args.push(format!("{} {}", emit_type_to_llvm_str(ty), a));
            }
        }

        if use_sret {
            self.line(&format!(
                "  call void %v{method_fn_r}({})",
                all_args.join(", ")
            ));
            sret_name.unwrap()
        } else if *ret_ty == EmitType::Void {
            self.line(&format!(
                "  call void %v{method_fn_r}({})",
                all_args.join(", ")
            ));
            "0".to_string()
        } else {
            let call_r = self.fresh();
            let ret_str = emit_type_to_llvm_str(ret_ty);
            self.line(&format!(
                "  %v{call_r} = call {ret_str} %v{method_fn_r}({})",
                all_args.join(", ")
            ));
            format!("%v{call_r}")
        }
    }
}
