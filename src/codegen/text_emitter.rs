//! Text emitter: implements Emitter trait by generating LLVM IR text (.ll).

use crate::codegen::emitter::*;
use crate::mir::lvalue::{BinOp, UnOp};
use crate::mir::ty::ConstVal;
use std::collections::HashMap;

pub struct TextEmitter {
    output: String,
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
}

impl Emitter for TextEmitter {
    fn begin_function(&mut self, name: &str, params: &[(EmitType, &str)], ret: EmitType) {
        let ret_str = emit_type_to_llvm_str(ret);
        let param_strs: Vec<String> = params
            .iter()
            .map(|(ty, name)| format!("{} {}", emit_type_to_llvm_str(*ty), name))
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

    fn end_function(&mut self) {
        self.line("}");
        self.line("");
    }

    fn emit_constant(&mut self, val: &ConstVal) -> EmitValue {
        match val {
            ConstVal::Int(n) => format!("{}", n),
            ConstVal::Uint(n) => format!("{}", n),
            ConstVal::Bool(b) => format!("{}", if *b { 1 } else { 0 }),
            ConstVal::Float(f) => format!("{}", f),
            ConstVal::Char(c) => format!("{}", *c as u32),
            ConstVal::Str(_) => "0".to_string(),
            ConstVal::Unevaluated => "0".to_string(),
        }
    }

    fn emit_binary_op(
        &mut self,
        op: BinOp,
        ty: EmitType,
        lhs: &EmitValue,
        rhs: &EmitValue,
    ) -> EmitValue {
        let r = self.fresh();
        let opc = binop_to_llvm_str(op, ty);
        self.line(&format!("  %v{} = {} {}, {}", r, opc, lhs, rhs));
        format!("%v{}", r)
    }

    fn emit_unary_op(&mut self, op: UnOp, ty: EmitType, operand: &EmitValue) -> EmitValue {
        let r = self.fresh();
        match op {
            UnOp::Neg => {
                let ty_str = emit_type_to_llvm_str(ty);
                self.line(&format!("  %v{} = sub {} 0, {}", r, ty_str, operand));
            }
            UnOp::Not => {
                let ty_str = emit_type_to_llvm_str(ty);
                self.line(&format!("  %v{} = xor {} {}, -1", r, ty_str, operand));
            }
        }
        format!("%v{}", r)
    }

    fn emit_return(&mut self, ty: EmitType, val: Option<&EmitValue>) {
        let ty_str = emit_type_to_llvm_str(ty);
        match val {
            Some(v) => self.line(&format!("  ret {} {}", ty_str, v)),
            None => self.line("  ret void"),
        }
    }

    fn emit_unreachable(&mut self) {
        self.line("  unreachable");
    }

    fn emit_alloca(&mut self, ty: EmitType, name: &str) -> EmitValue {
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  {} = alloca {}", name, ty_str));
        name.to_string()
    }

    fn emit_store(&mut self, ty: EmitType, val: &EmitValue, ptr: &EmitValue) {
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  store {} {}, {}", ty_str, val, ptr));
    }

    fn emit_load(&mut self, ty: EmitType, ptr: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!("  %v{} = load {}, {}", r, ty_str, ptr));
        format!("%v{}", r)
    }

    fn emit_branch(&mut self, label: &str) {
        self.line(&format!("  br label %{}", label));
    }

    fn emit_cond_branch(&mut self, cond: &EmitValue, then_label: &str, else_label: &str) {
        self.line(&format!(
            "  br i1 {}, label %{}, label %{}",
            cond, then_label, else_label
        ));
    }

    fn begin_block(&mut self, label: &str) {
        self.line(&format!("{}:", label));
    }

    fn emit_call(&mut self, fn_name: &str, args: &[EmitValue], ret_ty: EmitType) -> EmitValue {
        let r = self.fresh();
        let ret_str = emit_type_to_llvm_str(ret_ty);
        let args_str = args
            .iter()
            .map(|a| format!("i32 {}", a))
            .collect::<Vec<_>>()
            .join(", ");
        self.line(&format!(
            "  %v{} = call {} @{}({})",
            r, ret_str, fn_name, args_str
        ));
        format!("%v{}", r)
    }

    fn local_ptr(&self, local_id: u32) -> Option<&EmitValue> {
        self.local_ptrs.get(&local_id)
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

    fn output(&self) -> &str {
        &self.output
    }

    fn emit_icmp(&mut self, op: &str, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue {
        let r = self.fresh();
        let ty_str = emit_type_to_llvm_str(ty);
        self.line(&format!(
            "  %v{} = icmp {} {} {}, {}",
            r, op, ty_str, lhs, rhs
        ));
        format!("%v{}", r)
    }

    fn emit_zext_i1_to_i32(&mut self, val: &EmitValue) -> EmitValue {
        let r = self.fresh();
        self.line(&format!("  %v{} = zext i1 {} to i32", r, val));
        format!("%v{}", r)
    }
}
