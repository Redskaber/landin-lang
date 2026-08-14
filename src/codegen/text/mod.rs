//! Text emitter: implements Emitter trait by generating LLVM IR text (.ll).
//!
//! Stage 16.76 MUV-1: `Emitter` was split into 6 sub-traits
//! (`ModuleEmitter`, `FunctionEmitter`, `ArithmeticEmitter`,
//! `MemoryEmitter`, `AggregateEmitter`, `LocalStateEmitter`).
//! `TextEmitter` now implements each sub-trait in a separate `impl`
//! block below — the `Emitter` super-trait is auto-implemented via the
//! blanket `impl<T: ...> Emitter for T` in `emitter/mod.rs`.
//!
//! Stage 16.35: This module now owns the text-backend-specific string
//! rendering functions (`emit_type_to_llvm_str`, `binop_to_llvm_str`).
//! These were previously in the shared `emitter.rs` but are only used
//! by `TextEmitter` — the LLVM C-API backend has its own `llvm_type()`
//! method. Moving them here follows §23 rule 5 (DRY) and §1.0 原則 6
//! (通用 > 特例 — each backend owns its own rendering logic).

use crate::codegen::emitter::*;
use crate::mir::place::BinOp;
use std::collections::HashMap;

// ================================================================
// Stage 16.35: Text-backend-specific string rendering functions.
// Moved from emitter.rs — these are only used by TextEmitter.
// ================================================================

/// Map an EmitType to its LLVM type string (text backend only).
///
/// Stage 3.21: returns `String` (was `&'static str`) because struct and
/// array layouts must be rendered dynamically from their element types.
///
/// Stage 16.35: Moved from `emitter.rs` to `text/mod.rs`. The LLVM
/// C-API backend uses `LLVMSysEmitter::llvm_type()` (returns
/// `LLVMTypeRef`) instead of this string-based function.
pub(crate) fn emit_type_to_llvm_str(ty: &EmitType) -> String {
    match ty {
        EmitType::I1 => "i1".into(),
        EmitType::I8 => "i8".into(),
        EmitType::I16 => "i16".into(),
        EmitType::I32 => "i32".into(),
        EmitType::I64 => "i64".into(),
        EmitType::I128 => "i128".into(),
        EmitType::F32 => "float".into(),
        EmitType::F64 => "double".into(),
        // Stage 14.59: LLVM 19+ uses opaque pointers — all pointer types
        // emit as "ptr" regardless of pointee type. Was: "{}*" with pointee.
        EmitType::Ptr(_) | EmitType::OpaquePtr => "ptr".into(),
        EmitType::Void => "void".into(),
        EmitType::Struct(fields) => {
            if fields.is_empty() {
                // Stage 16.22: Empty struct ({}) has size 0 in LLVM, which
                // causes undefined behavior when used with alloca (the
                // pointer is invalid). Use i8 (size 1) instead to ensure
                // the pointer is valid. This is safe because empty structs
                // carry no data — the i8 byte is never read.
                // Per §1.0 原則 9 "正确 > 妥协": correct runtime behavior
                // over matching the conceptual type exactly.
                "i8".into()
            } else {
                let parts: Vec<String> = fields.iter().map(emit_type_to_llvm_str).collect();
                format!("{{ {} }}", parts.join(", "))
            }
        }
        EmitType::Array(elem, n) => format!("[{} x {}]", n, emit_type_to_llvm_str(elem)),
    }
}

/// Render a BinOp as its LLVM instruction string (text backend only).
///
/// Stage 3.46: generic integer type support — generates the instruction
/// with the correct type suffix for all integer widths (i8/i16/i32/i64/i128).
///
/// Stage 16.35: Moved from `emitter.rs` to `text/mod.rs`. The LLVM
/// C-API backend uses `LLVMBuildAdd` etc. directly.
pub(crate) fn binop_to_llvm_str(op: BinOp, ty: &EmitType) -> String {
    let ty_str = emit_type_to_llvm_str(ty);
    let is_int = matches!(
        ty,
        EmitType::I1
            | EmitType::I8
            | EmitType::I16
            | EmitType::I32
            | EmitType::I64
            | EmitType::I128
    );
    match (op, ty) {
        // Integer arithmetic
        (BinOp::Add, _) if is_int => format!("add nsw {}", ty_str),
        (BinOp::Sub, _) if is_int => format!("sub nsw {}", ty_str),
        (BinOp::Mul, _) if is_int => format!("mul nsw {}", ty_str),
        (BinOp::Div, _) if is_int => format!("sdiv {}", ty_str),
        (BinOp::Rem, _) if is_int => format!("srem {}", ty_str),
        // Float arithmetic
        (BinOp::Add, EmitType::F64) => "fadd double".into(),
        (BinOp::Add, EmitType::F32) => "fadd float".into(),
        (BinOp::Sub, EmitType::F64) => "fsub double".into(),
        (BinOp::Sub, EmitType::F32) => "fsub float".into(),
        (BinOp::Mul, EmitType::F64) => "fmul double".into(),
        (BinOp::Mul, EmitType::F32) => "fmul float".into(),
        (BinOp::Div, EmitType::F64) => "fdiv double".into(),
        (BinOp::Div, EmitType::F32) => "fdiv float".into(),
        (BinOp::Rem, EmitType::F64) => "frem double".into(),
        (BinOp::Rem, EmitType::F32) => "frem float".into(),
        // Bitwise (all integer types)
        (BinOp::BitAnd, _) if is_int => format!("and {}", ty_str),
        (BinOp::BitOr, _) if is_int => format!("or {}", ty_str),
        (BinOp::BitXor, _) if is_int => format!("xor {}", ty_str),
        (BinOp::Shl, _) if is_int => format!("shl {}", ty_str),
        (BinOp::Shr, _) if is_int => format!("ashr {}", ty_str),
        _ => "add i32".into(),
    }
}

// Stage 16.77 MUV-2: Backend file organization — 6 sub-trait impls split into separate files.
pub(crate) mod aggregate;
pub(crate) mod arithmetic;
pub(crate) mod function;
pub(crate) mod local_state;
pub(crate) mod memory;
pub(crate) mod module;

pub struct TextEmitter {
    /// Stage 18.88: Target triple for cross-compilation.
    target: crate::codegen::TargetTriple,
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
        Self::with_target(crate::codegen::TargetTriple::default())
    }

    /// Stage 18.88: Create with a specific target triple.
    pub fn with_target(target: crate::codegen::TargetTriple) -> Self {
        Self {
            target,
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

// ================================================================
// Stage 16.76 MUV-1: TextEmitter implements 6 sub-traits.
// `Emitter` (super-trait) is auto-implemented via the blanket impl
// in `emitter/mod.rs`. Method bodies are unchanged from the previous
// single `impl Emitter for TextEmitter` block — only the grouping
// into impl blocks differs.
// ================================================================
