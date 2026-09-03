//! Stage 16.54-16.60 (Task 11): Monomorphization — collection, naming,
//! per-mono layouts, and codegen integration.
//!
//! This module is split into three sub-modules per the single-responsibility
//! principle (§16 high cohesion, low coupling):
//!
//! - `item` — `MonoItem` enum + `collect_mono_items` collection pass
//! - `mangle` — `mangle_ty`, `mono_item_name`, `build_mono_item_names`
//! - `layout` — `MonoLayoutKey`, `MonoLayoutMap`, `build_mono_layouts`,
//!   `lookup_mono_layout`
//!
//! Per §14.4 (architectural split): each sub-module owns one concern.
//! Per §23: re-exports use explicit lists (§5.1) with stage-tracking
//! comments (§5.2).
//! Per §1.0 原則 6 "通用 > 特例": one module for all monomorphization needs.

mod item;
mod layout;
mod mangle;
mod trait_method_map;

// Stage 16.54 (Phase 3): Monomorphization collection.
pub use item::{collect_mono_items, MonoItem};

// Stage 16.55 (Phase 4a): Specialized naming.
pub use mangle::{build_mono_item_names, mangle_ty, mangle_ty_with_interner, mono_item_name};

// Stage 16.57-16.58 (Phase 4b-4c): Per-mono layouts + codegen integration.
pub use layout::{build_mono_layouts, lookup_mono_layout, MonoLayoutKey, MonoLayoutMap};

// Stage 68 (v0.8 — TD-IMPL-TRAIT-MONO-RESOLUTION): Trait method resolution map.
// Pre-computed in driver, passed as data to codegen. Per §16 (codegen is HIR-free).
pub use trait_method_map::{build_trait_method_resolution_map, TraitMethodResolutionMap};
