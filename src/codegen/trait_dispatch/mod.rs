//! Trait dispatch emission — vtable/dynptr global generation.
//!
//! Stage 14.3 §14.4 architectural split: the historical 962-LOC
//! `codegen/trait_dispatch.rs` was split into three focused sub-modules
//! along the vtable/dynptr/orchestrator boundary, per the six refactoring
//! criteria (J1-J6):
//!
//! - **`vtable`** — vtable global emission (pure helpers + resolver-driven
//!   orchestrator). Produces `@.vtable.<trait>.<type>` globals.
//! - **`dynptr`** — `dyn Trait` fat-pointer global emission (pure helpers +
//!   resolver-driven orchestrator). Produces `@.dynptr.<trait>.<type>` globals.
//! - **`orchestrator`** — high-level orchestration combining vtable + dynptr
//!   emission, plus project-level `EmissionPlan` / `EmissionSummary`
//!   aggregates for batch text generation and diagnostic output.
//!
//! ### Why the split (§14.4 J1-J6 compliance)
//!
//! | # | Criterion | How this split satisfies it |
//! |---|-----------|---------------------------|
//! | J1 | Architecture design alignment | Mirrors the vtable/dynptr dichotomy in `docs/lang-design/07-codegen.md` |
//! | J2 | Single responsibility | Each sub-module produces exactly one kind of LLVM global |
//! | J3 | Single-direction flow | `vtable` and `dynptr` are leaves; `orchestrator` depends on both — DAG, no cycles |
//! | J4 | Complete compilation expression | Each sub-module owns its full concern (spec builder + text helper + emitter orchestrator) |
//! | J5 | Stage partition clear | All within codegen stage; no cross-stage calls (§16 compliant) |
//! | J6 | Scientific granularity | Each sub-module is 200-400 LOC, well within the 100-1500 range |
//!
//! Per §16: all functions take pre-built data (TraitResolver / StdlibVtableEmission)
//! — no HIR access. Data flows downstream: traits → codegen/trait_dispatch → LLVM IR.

pub mod dynptr;
pub mod orchestrator;
pub mod vtable;

// Stage 14.3 §23 compliance: explicit re-export list (no glob `pub use X::*;`).
// Each name below is a public symbol from a sub-module that callers may use
// via `crate::codegen::trait_dispatch::<Name>` or
// `landin_compiler::codegen::trait_dispatch::<Name>`.

// From `vtable` — vtable global emission.
pub use vtable::{
    build_vtable_global_specs, emit_vtable_global_from_emission, emit_vtable_global_text,
    emit_vtable_globals_batch, emit_vtables, emit_vtables_from_resolver, StdlibVtableGlobalSpec,
};

// From `dynptr` — dyn Trait fat-pointer global emission.
pub use dynptr::{
    build_dynptr_global_specs, emit_dyn_trait_ptrs, emit_dynptr_global_text,
    emit_dynptrs_from_resolver, StdlibDynptrGlobalSpec,
};

// From `orchestrator` — combined vtable+dynptr emission + plan/summary aggregates.
pub use orchestrator::{
    build_trait_dispatch_emission_plan, build_trait_dispatch_emission_summary,
    emit_trait_dispatch_globals_from_plan, emit_trait_dispatch_globals_text_batch,
    emit_trait_dispatch_globals_text_batch_from_resolver, emit_vtables_and_dynptrs_from_resolver,
    CodegenTraitDispatchEmissionPlan, CodegenTraitDispatchEmissionSummary,
};
