//! Stage 16.77 MUV-2: `impl ModuleEmitter for TextEmitter`.
//!
//! Extracted from `text/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::*;

use super::TextEmitter;

impl ModuleEmitter for TextEmitter {
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
        // Stage 5.57: delegate to emit_vtable_global_text() (Stage 5.44 free function).
        // This also fixes the latent null-handling bug — the old inline code (Stage
        // 5.6) would emit `ptr @null` for "null" strings, while the free function
        // correctly emits `ptr null`.
        //
        // Layout (e.g. trait Foo with method `bar` impl'd for type S):
        //   @.vtable.Foo.S = private unnamed_addr constant [1 x ptr] [ptr @landin_S_bar]
        //
        // We do NOT dedupe: each (trait, type) pair is distinct by name,
        // and the caller (codegen `emit_vtables`) already guarantees a
        // unique global_name per vtable. If `method_symbols` is empty we
        // still emit the global as a zero-size array so downstream stages
        // can reference it unconditionally.
        let global_def = crate::codegen::emit_vtable_global_text(global_name, method_symbols);
        self.globals.push(global_def);
        // Return the global's name (without leading `@`).
        global_name.to_string()
    }

    fn emit_dyn_trait_const(
        &mut self,
        global_name: &str,
        data_symbol: &str,
        vtable_symbol: &str,
    ) -> EmitValue {
        // Stage 5.58: delegate to emit_dynptr_global_text() (Stage 5.48 free function).
        //
        // Layout (e.g. dyn Foo for type S, with data global @.data.S and
        // vtable global @.vtable.Foo.S):
        //   @.dynptr.Foo.S = private unnamed_addr constant
        //       { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }
        //
        // The fat pointer is { ptr (data), ptr (vtable) } — both opaque
        // because the concrete type is erased at the `dyn` boundary.
        let global_def =
            crate::codegen::emit_dynptr_global_text(global_name, data_symbol, vtable_symbol);
        self.globals.push(global_def);
        // Return the global's name (without leading `@`).
        global_name.to_string()
    }
}
