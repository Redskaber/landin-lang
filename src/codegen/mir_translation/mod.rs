//! Stage 16.76 MUV-3: MIR type/place/operand translation helpers.
//!
//! Split from the original `mir_translation.rs` (1144 LOC) into 4 sub-modules
//! per §13.4 J1-J6 (alignment with `docs/lang-design/07-codegen.md`):
//!
//! - `types`   (§2.1-§2.3): MIR Ty → EmitType translation (with layouts/mono)
//! - `layouts` (§2.3-§2.4): AdtLayout → EmitType translation
//! - `places`  (§4.4):      Place projection address computation + load
//! - `stdlib`  (cross-§):   StdlibTypeKind → EmitType bridge
//!
//! Per §16 (interface isolation): all functions read MIR data only (Ty,
//! MirBody.adt_layouts, Place) — no HIR access.
//!
//! Per §13.4 J2 (single responsibility): each sub-module owns one concept.
//! Per §13.4 J3 (unidirectional flow): mod.rs depends on all sub-modules;
//! sub-modules do not depend on each other.

pub(crate) mod layouts;
pub(crate) mod places;
pub(crate) mod stdlib;
pub(crate) mod types;

// Re-export public API (backward compatibility with old `mir_translation::*`).
pub use stdlib::stdlib_type_kind_to_emit_type;
pub use types::{mir_type_to_emit_type_with_layouts, mir_type_to_emit_type_with_layouts_and_mono};

// pub(crate) re-exports for crate-internal helpers (used by codegen sub-modules).
pub(crate) use places::{
    codegen_place_load, codegen_place_load_typed, compute_place_address, detect_operand_type,
    detect_place_storage_type, detect_place_type, unwrap_fat_ptr_for_index,
};
