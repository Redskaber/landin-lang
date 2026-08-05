//! ModuleEmitter sub-trait — module-level globals & declarations.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). ModuleEmitter owns the 5 methods
//! that emit module-scope constructs: header, declarations, string globals,
//! vtable globals, and dyn-trait fat-pointer constants.

use super::EmitValue;

/// Module-level emission: globals, declarations, module header.
///
/// Per §13.4 J2 single responsibility: this trait covers everything that
/// lives at module scope (outside any function body). Backends implement
/// this in addition to the other 5 sub-traits to gain `Emitter` (via the
/// blanket impl in `super`).
pub trait ModuleEmitter {
    /// Emit module header (target triple, datalayout).
    fn emit_header(&mut self);

    /// Emit an external function declaration.
    fn emit_declare(&mut self, signature: &str);

    /// Emit (or look up) a module-level string constant global.
    fn emit_string_global(&mut self, bytes: &[u8]) -> EmitValue;

    /// Emit a vtable as a module-level constant global.
    fn emit_vtable_global(&mut self, global_name: &str, method_symbols: &[String]) -> EmitValue;

    /// Emit a `dyn Trait` fat-pointer constant global.
    fn emit_dyn_trait_const(
        &mut self,
        global_name: &str,
        data_symbol: &str,
        vtable_symbol: &str,
    ) -> EmitValue;
}
