//! Text emitter: implements Emitter trait by generating LLVM IR text (.ll).

use crate::codegen::emitter::*;
use crate::mir::place::{BinOp, UnOp};
use crate::mir::ty::ConstVal;
use std::collections::HashMap;

pub struct TextEmitter {
    output: String,
    /// Accumulated module-level global definitions (emitted at end of module).
    /// Stage 3.27: holds string-literal globals.
    globals: Vec<String>,
    /// Deduplication map: byte content → global name. Stage 3.27.
    string_globals: HashMap<Vec<u8>, String>,
    /// Counter for generating unique global names. Stage 3.27.
    next_str: u32,
    next_val: u32,
    locals: HashMap<u32, EmitValue>,
    local_ptrs: HashMap<u32, EmitValue>,
}

impl Default for TextEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEmitter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            globals: Vec::new(),
            string_globals: HashMap::new(),
            next_str: 0,
            next_val: 1,
            locals: HashMap::new(),
            local_ptrs: HashMap::new(),
        }
    }

    fn fresh(&mut self) -> u32 {
        let v = self.next_val;
        self.next_val += 1;
        v
    }

    fn line(&mut self, text: &str) {
        self.output.push_str(text);
        self.output.push('\n');
    }

    /// Return the full module text: function bodies followed by accumulated
    /// module-level globals (string constants). Stage 3.27.
    pub fn output_with_globals(&self) -> String {
        let mut out = String::with_capacity(self.output.len() + 1024);
        out.push_str(&self.output);
        if !self.globals.is_empty() {
            out.push_str("; --- Module-level string constants ---\n");
            for g in &self.globals {
                out.push_str(g);
                out.push('\n');
            }
        }
        out
    }
}

impl Emitter for TextEmitter {
    fn emit_header(&mut self) {
        self.line("; Landin compiler v0.8.6 — LLVM IR output");
        self.line("; Stage 3.21 codegen (typed aggregates + typed call args)");
        self.line("target triple = \"x86_64-unknown-linux-gnu\"");
        self.line("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"");
        self.line("");
    }

    fn emit_declare(&mut self, signature: &str) {
        self.line(&format!("declare {}", signature));
    }

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

    fn emit_const(&mut self, val: &ConstVal) -> EmitValue {
        match val {
            ConstVal::Int(n) => format!("{}", n),
            ConstVal::Uint(n) => format!("{}", n),
            ConstVal::Bool(b) => format!("{}", if *b { 1 } else { 0 }),
            ConstVal::Float(f) => {
                if *f == 0.0 {
                    "0.000000e+00".to_string()
                } else if *f == 1.0 {
                    "1.000000e+00".to_string()
                } else {
                    format!("{:e}", f)
                }
            }
            ConstVal::Char(c) => format!("{}", *c as u32),
            // Stage 3.27: emit a module-level global and return its name.
            // The caller will treat the value as a `i8*` (pointer to the
            // first byte of the global). Full &str fat-pointer (ptr+len)
            // representation is deferred — Stage 3.27 just gives the ptr.
            ConstVal::Str(sym) => {
                // The symbol's bytes are looked up by the codegen translation
                // layer (which has the interner) and passed to
                // `emit_string_global`. By the time we get here, the value
                // has already been turned into a global name and stored in
                // `string_globals` under the symbol's key. We can't call
                // `emit_string_global` from here because we don't have the
                // bytes — the codegen layer intercepts Str before calling
                // emit_const.
                //
                // Fallback: if emit_const is called directly with Str,
                // emit a placeholder zero pointer.
                let _ = sym;
                "0".to_string()
            }
            ConstVal::Unevaluated => "0".to_string(),
        }
    }

    fn emit_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let opc = binop_to_llvm_str(op, ty);
        self.line(&format!("  %v{} = {} {}, {}", r, opc, lhs, rhs));
        format!("%v{}", r)
    }

    fn emit_unop(&mut self, op: UnOp, ty: &EmitType, operand: &EmitValue) -> EmitValue {
        let r = self.fresh();
        match op {
            UnOp::Neg => {
                let ty_str = emit_type_to_llvm_str(ty);
                if *ty == EmitType::F64 || *ty == EmitType::F32 {
                    self.line(&format!("  %v{} = fneg {} {}", r, ty_str, operand));
                } else {
                    self.line(&format!("  %v{} = sub {} 0, {}", r, ty_str, operand));
                }
            }
            UnOp::Not => {
                let ty_str = emit_type_to_llvm_str(ty);
                self.line(&format!("  %v{} = xor {} {}, -1", r, ty_str, operand));
            }
        }
        format!("%v{}", r)
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

    fn emit_alloca(&mut self, ty: &EmitType, name: &str) -> EmitValue {
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  {} = alloca {}", name, ty_str));
        name.to_string()
    }

    fn emit_store(&mut self, ty: &EmitType, val: &EmitValue, ptr: &EmitValue) {
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  store {} {}, {}", ty_str, val, ptr));
    }

    fn emit_load(&mut self, ty: &EmitType, ptr: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = load {}, {}", r, ty_str, ptr));
        format!("%v{}", r)
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

    fn emit_call(
        &mut self,
        fn_name: &str,
        args: &[(EmitType, &EmitValue)],
        ret_ty: &EmitType,
    ) -> EmitValue {
        let r = self.fresh();
        let ret_str = emit_type_to_llvm_str(ret_ty);
        let args_str = args
            .iter()
            .map(|(ty, a)| format!("{} {}", emit_type_to_llvm_str(ty), a))
            .collect::<Vec<_>>()
            .join(", ");
        // For void calls we don't assign a result register.
        if *ret_ty == EmitType::Void {
            self.line(&format!("  call void @{}({})", fn_name, args_str));
            "0".to_string()
        } else {
            self.line(&format!(
                "  %v{} = call {} @{}({})",
                r, ret_str, fn_name, args_str
            ));
            format!("%v{}", r)
        }
    }

    fn emit_icmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = icmp {} {} {}, {}",
            r, op, ty_str, lhs, rhs
        ));
        format!("%v{}", r)
    }

    fn emit_fcmp(
        &mut self,
        op: &str,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = fcmp {} {} {}, {}",
            r, op, ty_str, lhs, rhs
        ));
        format!("%v{}", r)
    }

    /// Stage 3.49 (L13 closure): bitwise AND for fat-pointer eq comparison.
    fn emit_and(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = and {} {}, {}", r, ty_str, lhs, rhs));
        format!("%v{}", r)
    }

    /// Stage 3.49 (L13 closure): bitwise OR for fat-pointer ne comparison.
    fn emit_or(&mut self, ty: &EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = or {} {}, {}", r, ty_str, lhs, rhs));
        format!("%v{}", r)
    }

    fn emit_zext(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let src_str = emit_type_to_llvm_str(src);
        let dst_str = emit_type_to_llvm_str(dst);
        self.line(&format!(
            "  %v{} = zext {} {} to {}",
            r, src_str, val, dst_str
        ));
        format!("%v{}", r)
    }

    fn emit_cast(&mut self, src: &EmitType, dst: &EmitType, val: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let src_str = emit_type_to_llvm_str(src);
        let dst_str = emit_type_to_llvm_str(dst);
        let op = match (src, dst) {
            (a, b) if a == b => return val.clone(),
            (EmitType::I32, EmitType::I64) => "sext",
            (EmitType::I1, EmitType::I32) => "zext",
            (EmitType::I64, EmitType::I32) => "trunc",
            (EmitType::I32, EmitType::I1) => "trunc",
            (EmitType::I32, EmitType::F64) | (EmitType::I64, EmitType::F64) => "sitofp",
            (EmitType::I32, EmitType::F32) | (EmitType::I64, EmitType::F32) => "sitofp",
            (EmitType::F64, EmitType::I32) | (EmitType::F64, EmitType::I64) => "fptosi",
            (EmitType::F32, EmitType::I32) | (EmitType::F32, EmitType::I64) => "fptosi",
            (EmitType::F64, EmitType::F32) => "fptrunc",
            (EmitType::F32, EmitType::F64) => "fpext",
            _ => "bitcast",
        };
        self.line(&format!(
            "  %v{} = {} {} {} to {}",
            r, op, src_str, val, dst_str
        ));
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
        let ptr_str = format!("{}*", struct_str);
        self.line(&format!(
            "  %v{} = getelementptr inbounds {}, {} {}, i32 0, i32 {}",
            r, struct_str, ptr_str, base_ptr, field_index
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
        let ptr_str = format!("{}*", array_str);
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
        let ptr_str = format!("{}*", elem_str);
        self.line(&format!(
            "  %v{} = getelementptr inbounds {}, {} {}, i32 {}",
            r, elem_str, ptr_str, base_ptr, index
        ));
        format!("%v{}", r)
    }

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

    fn emit_checked_binop(
        &mut self,
        op: BinOp,
        ty: &EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        // Stage 3.24: emit `llvm.{sadd,ssub,smul}.with.overflow.{i32,i64}`.
        // Returns `{ T, i1 }` — caller extracts index 1 for the overflow flag.
        let elem_str = emit_type_to_llvm_str(ty);
        let intrinsic = match (op, ty) {
            (BinOp::Add, EmitType::I8) => "llvm.sadd.with.overflow.i8",
            (BinOp::Add, EmitType::I16) => "llvm.sadd.with.overflow.i16",
            (BinOp::Add, EmitType::I32) => "llvm.sadd.with.overflow.i32",
            (BinOp::Add, EmitType::I64) => "llvm.sadd.with.overflow.i64",
            (BinOp::Add, EmitType::I128) => "llvm.sadd.with.overflow.i128",
            (BinOp::Sub, EmitType::I8) => "llvm.ssub.with.overflow.i8",
            (BinOp::Sub, EmitType::I16) => "llvm.ssub.with.overflow.i16",
            (BinOp::Sub, EmitType::I32) => "llvm.ssub.with.overflow.i32",
            (BinOp::Sub, EmitType::I64) => "llvm.ssub.with.overflow.i64",
            (BinOp::Sub, EmitType::I128) => "llvm.ssub.with.overflow.i128",
            (BinOp::Mul, EmitType::I8) => "llvm.smul.with.overflow.i8",
            (BinOp::Mul, EmitType::I16) => "llvm.smul.with.overflow.i16",
            (BinOp::Mul, EmitType::I32) => "llvm.smul.with.overflow.i32",
            (BinOp::Mul, EmitType::I64) => "llvm.smul.with.overflow.i64",
            (BinOp::Mul, EmitType::I128) => "llvm.smul.with.overflow.i128",
            // Unsupported op or type — fall back to "no overflow".
            // Synthesize `{ T, i1 } undef` with the overflow flag zeroed.
            _ => {
                let r = self.fresh();
                let agg_str = format!("{{ {}, i1 }}", elem_str);
                self.line(&format!(
                    "  %v{} = insertvalue {} undef, {} 0, 1",
                    r, agg_str, elem_str
                ));
                return format!("%v{}", r);
            }
        };
        let r = self.fresh();
        let agg_str = format!("{{ {}, i1 }}", elem_str);
        self.line(&format!(
            "  %v{} = call {} @{}({} {}, {} {})",
            r, agg_str, intrinsic, elem_str, lhs, elem_str, rhs
        ));
        format!("%v{}", r)
    }

    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue {
        // Stage 3.27: dedupe by content. Same bytes → same global.
        if let Some(name) = self.string_globals.get(bytes) {
            return name.clone();
        }
        let name = format!(".str.{}", self.next_str);
        self.next_str += 1;
        // LLVM c"..." literal: bytes are printed as `c` + quoted string with
        // `\NN` hex escapes for non-printable bytes. We always emit the bytes
        // verbatim (using `\NN` for non-ASCII / control chars to be safe).
        let mut literal = String::from("c\"");
        for &b in bytes {
            match b {
                // Printable ASCII except `"` and `\` (those need escaping).
                b' '..=b'~' if b != b'"' && b != b'\\' => literal.push(b as char),
                _ => literal.push_str(&format!("\\{:02X}", b)),
            }
        }
        literal.push('"');
        let global_def = format!(
            "@{} = private unnamed_addr constant [{} x i8] {}",
            name,
            bytes.len(),
            literal
        );
        self.globals.push(global_def);
        self.string_globals.insert(bytes.to_vec(), name.clone());
        // Return the global's *name*; callers reference it as `@.str.N`.
        // The pointer-typed value is `getelementptr inbounds ([N x i8], [N x i8]* @.str.N, i32 0, i32 0)`.
        // To keep the API simple we return the name and let the codegen
        // translation layer emit the GEP if it needs an `i8*`.
        name
    }

    fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue {
        // Stage 5.6: emit a vtable as a constant global array of opaque
        // function pointers. Uses LLVM 15+ opaque pointer type (`ptr`).
        //
        // Layout (e.g. trait Foo with method `bar` impl'd for type S):
        //   @.vtable.Foo.S = private unnamed_addr constant [1 x ptr] [ptr @landin_S_bar]
        //
        // We do NOT dedupe: each (trait, type) pair is distinct by name,
        // and the caller (codegen `emit_vtables`) already guarantees a
        // unique global_name per vtable. If `method_symbols` is empty we
        // still emit the global as a zero-size array so downstream stages
        // can reference it unconditionally.

        // Build the LLVM initializer expression.
        let init = if method_symbols.is_empty() {
            "zeroinitializer".to_string()
        } else {
            let entries: Vec<String> = method_symbols
                .iter()
                .map(|sym| format!("ptr @{}", sym))
                .collect();
            format!("[{} x ptr] [{}]", method_symbols.len(), entries.join(", "))
        };

        let global_def = format!("@{} = private unnamed_addr constant {}", global_name, init);
        self.globals.push(global_def);
        // Return the global's name (without leading `@`).
        global_name.to_string()
    }

    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue) {
        self.local_ptrs.insert(local_id, ptr);
    }

    fn get_local_ptr(&self, local_id: u32) -> Option<&EmitValue> {
        self.local_ptrs.get(&local_id)
    }

    fn set_local(&mut self, local_id: u32, val: EmitValue) {
        self.locals.insert(local_id, val);
    }

    fn get_local(&self, local_id: u32) -> Option<&EmitValue> {
        self.locals.get(&local_id)
    }

    fn emit_output(&self) -> &str {
        // Note: globals are emitted at the END of the module (after all
        // functions). The codegen_crate entry point calls `emit_output()` to
        // get the function bodies, then separately calls `globals_output()`
        // to get the trailing globals. To keep backward compatibility, we
        // return just the function bodies here. See `output_with_globals()`
        // for the combined output.
        &self.output
    }
}
