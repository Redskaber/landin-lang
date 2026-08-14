//! Stage 18.95: Trait error types.
//!
//! Extracted from `driver.rs` per §6 (single-source-of-truth) — the `TraitError`
//! enum and its impls were defined in the driver, violating the principle that
//! error types should live in their owning module.
//!
//! Per §1.0 原則 6 "通用 > 特例": follows the same pattern as
//! `TypeError` (in `typeck/error.rs`), `BorrowError` (in `borrowck/error.rs`).
//! Per §23: `TraitError` follows the `<Noun>Error` pattern.

use crate::traits::{CoherenceError, IncompleteImpl};
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
        }
    }
}
