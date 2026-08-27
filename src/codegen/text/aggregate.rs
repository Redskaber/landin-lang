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
        if ret_ty.needs_sret() {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            // Allocate space for the return value.
            let sret_name = format!("%sret_{}", r);
            self.line(&format!("  {} = alloca {}", sret_name, ret_str));
            // Build args: sret pointer first, then original args.
            let mut all_args = vec![format!("ptr {}", sret_name)];
            for (ty, a) in args {
                all_args.push(format!("{} {}", emit_type_to_llvm_str(ty), a));
            }
            self.line(&format!(
                "  call void {}({})",
                call_target,
                all_args.join(", ")
            ));
            // Return the sret pointer — callers use it to access the result.
            sret_name
        } else if *ret_ty == EmitType::Void {
            let args_str = args
                .iter()
                .map(|(ty, a)| format!("{} {}", emit_type_to_llvm_str(ty), a))
                .collect::<Vec<_>>()
                .join(", ");
            self.line(&format!("  call void {}({})", call_target, args_str));
            "0".to_string()
        } else {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            let args_str = args
                .iter()
                .map(|(ty, a)| format!("{} {}", emit_type_to_llvm_str(ty), a))
                .collect::<Vec<_>>()
                .join(", ");
            self.line(&format!(
                "  %v{} = call {} {}({})",
                r, ret_str, call_target, args_str
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

        // 4. Call the loaded function pointer (indirect call).
        let args_str = args
            .iter()
            .map(|(ty, a)| format!("{} {}", emit_type_to_llvm_str(ty), a))
            .collect::<Vec<_>>()
            .join(", ");

        // Stage 18.332 (P1 soundness fix): For struct return > 16 bytes,
        // use sret at the indirect call site. Mirrors TextEmitter::emit_call
        // sret path (Stage 18.330) and LLVMSysEmitter's emit_dyn_trait_method_call.
        //
        // Per §20 (iterative audit): found via auditing all emit_call paths.
        // Per §1.0 原則 6 (通解 > 特解): same sret path for direct + indirect calls.
        if ret_ty.needs_sret() {
            let ret_str = emit_type_to_llvm_str(ret_ty);
            // Allocate space for the return value.
            let sret_name = format!("%sret_{}", self.fresh());
            self.line(&format!("  {} = alloca {}", sret_name, ret_str));
            // Build args: sret pointer first, then original args.
            let mut all_args = vec![format!("ptr {}", sret_name)];
            for (ty, a) in args {
                all_args.push(format!("{} {}", emit_type_to_llvm_str(ty), a));
            }
            self.line(&format!(
                "  call void %v{method_fn_r}({})",
                all_args.join(", ")
            ));
            // Return the sret pointer — callers use it to access the result.
            sret_name
        } else if *ret_ty == EmitType::Void {
            self.line(&format!("  call void %v{method_fn_r}({args_str})"));
            "0".to_string()
        } else {
            let call_r = self.fresh();
            let ret_str = emit_type_to_llvm_str(ret_ty);
            self.line(&format!(
                "  %v{call_r} = call {ret_str} %v{method_fn_r}({args_str})"
            ));
            format!("%v{call_r}")
        }
    }
}
