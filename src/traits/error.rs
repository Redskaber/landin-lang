//! Stage 18.95: Trait error types.
//!
//! Extracted from `driver.rs` per §6 (single-source-of-truth) — the `TraitError`
//! enum and its impls were defined in the driver, violating the principle that
//! error types should live in their owning module.
//!
//! Per §1.0 原則 6 "通用 > 特例": follows the same pattern as
//! `TypeError` (in `typeck/error.rs`), `BorrowError` (in `borrowck/error.rs`).
//! Per §23: `TraitError` follows the `<Noun>Error` pattern.

use crate::traits::{
    CoherenceError, IncompleteImpl, InherentImplConflict, OrphanRuleError,
    PrimitiveInherentImplError,
};
use lasso::Rodeo;

/// A trait-related error encountered during trait validation.
///
/// Stage 5.20: Replaces the previous `Vec<String>` for `CompileErrors.trait_errors`.
/// Carries the structured `CoherenceError`/`IncompleteImpl` data so downstream
/// consumers (LSP, error reporters) can access the DefIds and Spur names
/// without re-parsing strings.
///
/// Per §1.0 原则 3 "显式 > 隐式": the error kind is explicit (enum variant),
/// not implicit (string prefix).
/// Per §23 (API Naming): `TraitError` follows the `<Noun>Error` pattern
/// consistent with `TypeError`, `BorrowError`, etc.
#[derive(Debug, Clone)]
pub enum TraitError {
    /// Stage 5.18: Multiple `impl Trait for Type` blocks exist for the same
    /// (trait, type) pair — coherence violation.
    Coherence(CoherenceError),
    /// Stage 5.19: An `impl Trait for Type` block is missing one or more
    /// methods declared by the trait.
    Incomplete(IncompleteImpl),
    /// Stage 18.292 (类 Rust 架构修正): Multiple `impl Type` blocks define
    /// the same method name — duplicate inherent impl. 类 Rust 设计:
    /// 用户不能覆盖 prelude 定义的原始类型方法。
    InherentConflict(InherentImplConflict),
    /// Stage 18.293 (类 Rust 架构修正): User inherent impl on primitive type.
    /// 类 Rust: only prelude ("core") can define `impl i32 { fn method {} }`.
    /// Users must extend primitive types via traits: `impl MyTrait for i32`.
    PrimitiveInherentImpl(PrimitiveInherentImplError),
    /// Stage 22.1 (v0.5 Trait Coherence P2): Orphan rule violation —
    /// `impl Trait for Type` where neither is local.
    /// Per §5.6: impl must have at least one local component.
    OrphanRule(OrphanRuleError),
}

impl TraitError {
    /// Format the error as a human-readable string, using the interner
    /// to resolve Spur symbols to &str.
    ///
    /// Per §23 (API Naming): `format_with_interner` follows
    /// `<verb>_<noun>_<noun>` pattern.
    pub fn format_with_interner(&self, interner: &Rodeo) -> String {
        match self {
            TraitError::Coherence(ce) => {
                let trait_str = interner.try_resolve(&ce.trait_name).unwrap_or("?");
                let type_str = interner.try_resolve(&ce.self_ty_name).unwrap_or("?");
                format!(
                    "conflicting implementations of trait `{}` for type `{}` ({} impl blocks)",
                    trait_str,
                    type_str,
                    ce.impl_def_ids.len()
                )
            }
            TraitError::Incomplete(inc) => {
                let trait_str = interner.try_resolve(&inc.trait_name).unwrap_or("?");
                let type_str = interner.try_resolve(&inc.self_ty_name).unwrap_or("?");
                let missing: Vec<&str> = inc
                    .missing_methods
                    .iter()
                    .map(|s| interner.try_resolve(s).unwrap_or("?"))
                    .collect();
                let missing_consts: Vec<&str> = inc
                    .missing_associated_consts
                    .iter()
                    .map(|s| interner.try_resolve(s).unwrap_or("?"))
                    .collect();
                let mut parts: Vec<String> = Vec::new();
                if !missing.is_empty() {
                    parts.push(format!("method(s): {}", missing.join(", ")));
                }
                if !missing_consts.is_empty() {
                    parts.push(format!(
                        "associated const(s): {}",
                        missing_consts.join(", ")
                    ));
                }
                format!(
                    "impl `{}` for `{}` is missing {}",
                    trait_str,
                    type_str,
                    parts.join("; ")
                )
            }
            TraitError::InherentConflict(ic) => {
                let type_str = interner.try_resolve(&ic.self_ty_name).unwrap_or("?");
                let method_str = interner.try_resolve(&ic.method_name).unwrap_or("?");
                format!(
                    "duplicate definitions with name `{}` for type `{}` ({} impl blocks)",
                    method_str,
                    type_str,
                    ic.impl_def_ids.len()
                )
            }
            TraitError::PrimitiveInherentImpl(_pie) => {
                "cannot define inherent `impl` for primitive type — only prelude is allowed; use `impl Trait for Type` instead".to_string()
            }
            TraitError::OrphanRule(ore) => {
                let trait_str = interner.try_resolve(&ore.trait_name).unwrap_or("?");
                let type_str = interner.try_resolve(&ore.self_ty_name).unwrap_or("?");
                format!(
                    "orphan rule violation: `impl {} for {}` — neither trait nor type is local to this crate",
                    trait_str, type_str
                )
            }
        }
    }

    /// Stage 15.96: Format the error as a human-readable string without
    /// an interner. Uses "?" for unresolved symbols instead of Debug format.
    ///
    /// Per §1.0 原則 4 "报错 > 静默": errors are always human-readable,
    /// even without an interner.
    /// Per §23: `format_without_interner` follows `<verb>_<noun>_<noun>`
    /// pattern.
    pub fn format_without_interner(&self) -> String {
        match self {
            TraitError::Coherence(ce) => {
                format!(
                    "conflicting implementations of trait `<unknown>` for type `<unknown>` ({} impl blocks)",
                    ce.impl_def_ids.len()
                )
            }
            TraitError::Incomplete(inc) => {
                format!(
                    "impl `<unknown>` for `<unknown>` is missing <{} method(s), {} associated const(s)>",
                    inc.missing_methods.len(),
                    inc.missing_associated_consts.len()
                )
            }
            TraitError::InherentConflict(ic) => {
                format!(
                    "duplicate definitions with name `<unknown>` for type `<unknown>` ({} impl blocks)",
                    ic.impl_def_ids.len()
                )
            }
            TraitError::PrimitiveInherentImpl(_pie) => {
                "cannot define inherent `impl` for primitive type — only prelude is allowed; use `impl Trait for Type` instead".to_string()
            }
            TraitError::OrphanRule(_ore) => {
                "orphan rule violation: `impl <unknown> for <unknown>` — neither trait nor type is local to this crate".to_string()
            }
        }
    }
}
