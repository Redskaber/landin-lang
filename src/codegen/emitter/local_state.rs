//! LocalStateEmitter sub-trait — local value/pointer mapping.
//!
//! Stage 16.76 MUV-1: Split from the original 39-method `Emitter` trait
//! (single responsibility per §13.4 J2). LocalStateEmitter owns the 4
//! methods that maintain per-function local state: the `local_ptr` map
//! (alloca handles) and the `local` value cache (most-recent SSA value).

use super::EmitValue;

/// Per-function local state: alloca handles + value cache.
///
/// Per §13.4 J2 single responsibility: this trait owns the two side-tables
/// that the codegen translation layer uses to look up locals by id — the
/// alloca pointer map (set once at function entry, persists for the whole
/// function) and the value cache (most-recent SSA value, cleared at block
/// boundaries).
pub trait LocalStateEmitter {
    /// Store a local's pointer handle (alloca result).
    fn set_local_ptr(&mut self, local_id: u32, ptr: EmitValue);

    /// Get a local's pointer handle.
    fn local_ptr(&self, local_id: u32) -> Option<&EmitValue>;

    /// Store a local's value handle.
    fn set_local(&mut self, local_id: u32, val: EmitValue);

    /// Get a local's stored value handle.
    fn local(&self, local_id: u32) -> Option<&EmitValue>;
}
