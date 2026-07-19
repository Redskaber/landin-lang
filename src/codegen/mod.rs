//! LLVM IR codegen: MIR → LLVM IR text (.ll).
//!
//! Stage 3.1 (MVP): supports function definition, return, i32 constants,
//! and basic binary operations (add/sub/mul/div).

use crate::hir::HirCrate;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use lasso::Rodeo;

/// Generate LLVM IR for a crate and return the .ll text.
pub fn codegen_crate(hir: &HirCrate, interner: &Rodeo) -> String {
    let mut cg = Codegen::new();
    cg.emit("; Landin compiler v0.5.0 — LLVM IR output");
    cg.emit("; Stage 3.1 codegen (MVP)");
    cg.newline();

    for (idx, (_, body)) in hir.bodies.iter().enumerate() {
        let fn_name = format!("fn_{}", idx);
        let (mut mir, unify) = crate::mir::lower::lower_hir_body_to_mir_full(body, interner, None);
        let mut tc = crate::typeck::TypeChecker::with_unify(unify);
        tc.populate_fn_sigs(hir);
        tc.check_mir_body(&mut mir);
        cg.codegen_function(&fn_name, &mir);
    }

    cg.output
}

struct Codegen {
    output: String,
    next_val: u32,
    ret_val: Option<String>,
    local_vals: std::collections::HashMap<u32, String>,
}

impl Codegen {
    fn new() -> Self {
        Self {
            output: String::new(),
            next_val: 1,
            ret_val: None,
            local_vals: std::collections::HashMap::new(),
        }
    }

    fn codegen_function(&mut self, name: &str, mir: &MirBody) {
        self.emit(&format!("define i32 @{}() {{", name));
        self.next_val = 1;
        self.ret_val = None;
        self.local_vals.clear();

        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                self.codegen_statement(stmt);
            }
            self.codegen_terminator(&bb.terminator);
        }

        self.emit("}");
        self.newline();
    }

    fn codegen_statement(&mut self, stmt: &Statement) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, rvalue) = &**boxed;
            let val = self.codegen_rvalue(rvalue);
            if let LvalueKind::Local(id) = &place.kind {
                if id.0 == 0 {
                    self.ret_val = Some(val.clone());
                }
                self.local_vals.insert(id.0, val);
            }
        }
    }

    fn codegen_rvalue(&mut self, rv: &Rvalue) -> String {
        match rv {
            Rvalue::Use(op) => self.codegen_operand(op),
            Rvalue::BinaryOp(op, a, b) => {
                let a = self.codegen_operand(a);
                let b = self.codegen_operand(b);
                let r = self.fresh();
                let opc = match op {
                    BinOp::Add => "add nsw i32",
                    BinOp::Sub => "sub nsw i32",
                    BinOp::Mul => "mul nsw i32",
                    BinOp::Div => "sdiv i32",
                    BinOp::Rem => "srem i32",
                    BinOp::BitAnd => "and i32",
                    BinOp::BitOr => "or i32",
                    BinOp::BitXor => "xor i32",
                    BinOp::Shl => "shl i32",
                    BinOp::Shr => "ashr i32",
                    _ => "add i32",
                };
                self.emit(&format!("  %v{} = {} {}, {}", r, opc, a, b));
                format!("%v{}", r)
            }
            Rvalue::UnaryOp(op, operand) => {
                let v = self.codegen_operand(operand);
                let r = self.fresh();
                match op {
                    UnOp::Neg => self.emit(&format!("  %v{} = sub i32 0, {}", r, v)),
                    UnOp::Not => self.emit(&format!("  %v{} = xor i32 {}, -1", r, v)),
                }
                format!("%v{}", r)
            }
            _ => "0".to_string(),
        }
    }

    fn codegen_operand(&mut self, op: &Operand) -> String {
        // Returns a value WITHOUT type prefix (e.g., "42", "%v1").
        // The caller adds the type when needed.
        match op {
            Operand::Constant(c) => match &c.val {
                ConstVal::Int(n) => format!("{}", n),
                ConstVal::Uint(n) => format!("{}", n),
                ConstVal::Bool(b) => format!("{}", if *b { 1 } else { 0 }),
                ConstVal::Float(f) => format!("{}", f),
                ConstVal::Char(c) => format!("{}", *c as u32),
                _ => "0".to_string(),
            },
            Operand::Copy(lv) | Operand::Move(lv) => {
                if let LvalueKind::Local(id) = &lv.kind {
                    self.local_vals
                        .get(&id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    "0".to_string()
                }
            }
        }
    }

    fn codegen_terminator(&mut self, term: &Terminator) {
        match term {
            Terminator::Return => {
                if let Some(ref v) = self.ret_val {
                    self.emit(&format!("  ret i32 {}", v));
                } else {
                    self.emit("  ret i32 0");
                }
            }
            Terminator::Unreachable => {
                self.emit("  unreachable");
            }
            _ => {
                self.emit("  ; unsupported terminator (Stage 3.1 MVP)");
            }
        }
    }

    fn fresh(&mut self) -> u32 {
        let v = self.next_val;
        self.next_val += 1;
        v
    }

    fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }
}
