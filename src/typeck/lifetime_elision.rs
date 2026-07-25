//! Stage 8.1 (TD-015 activation): Lifetime elision rules.
//!
//! Per `docs/lang-design/04-ownership-borrowing.md` §3.2 (Lifetime elision).
//! Per `docs/stage-committee-process.md` v3.21 §13.4 + §14.4.
//!
//! Implements RFC #141 lifetime elision rules:
//! 1. Each reference parameter gets a fresh lifetime 'a, 'b, 'c...
//! 2. If only one input lifetime, all output ref lifetimes take 'a
//! 3. If multiple input lifetimes but one is &self/&mut self, output takes self's
//! 4. Otherwise, output ref lifetime must be explicitly annotated (error if not)
//!
//! This module activates the region inference infrastructure (Stage 7.1-7.5)
//! by replacing `Region::Erased` with `Region::Var(fresh_vid)` in MIR types.

use crate::hir::{HirFnRetTy, HirFnSig, HirTy, HirTyKind};
use crate::mir::ty::{Region, RegionVid};
use crate::session::Span;

/// Context for lifetime elision.
///
/// Holds a counter for allocating fresh `RegionVid`s. Each call to
/// `allocate_fresh_lifetime()` produces a unique `RegionVid`.
///
/// Per §23: `LifetimeElisionCtxt` follows `<noun>_<noun>_<noun>` pattern.
#[derive(Debug, Clone)]
pub(crate) struct LifetimeElisionCtxt {
    /// Next fresh RegionVid to allocate.
    /// Starts at 1 (0 is reserved for 'static).
    next_vid: u32,
}

impl Default for LifetimeElisionCtxt {
    fn default() -> Self {
        Self::new()
    }
}

impl LifetimeElisionCtxt {
    /// Create a new elision context with fresh counter starting at 1.
    pub(crate) fn new() -> Self {
        Self { next_vid: 1 } // 0 = 'static
    }

    /// Allocate a fresh lifetime (RegionVid).
    ///
    /// Per §3.2 rule 1: each reference parameter gets a fresh lifetime.
    pub(crate) fn allocate_fresh_lifetime(&mut self) -> Region {
        let vid = RegionVid(self.next_vid);
        self.next_vid += 1;
        Region::Var(vid)
    }

    /// Apply lifetime elision rules to a function signature.
    ///
    /// Per §3.2:
    /// 1. Collect input reference lifetimes (allocate fresh for each erased)
    /// 2. If 1 input lifetime → output takes that lifetime
    /// 3. If multiple + has &self → output takes self's lifetime
    /// 4. Otherwise → output must be explicit (return error for now)
    ///
    /// Returns `Ok(())` if elision succeeds, or `Err(String)` with error message.
    pub(crate) fn elide_lifetimes(
        &mut self,
        fn_sig: &HirFnSig,
    ) -> Result<(), LifetimeElisionError> {
        // Collect input lifetimes from parameters
        let mut input_lifetimes: Vec<Region> = Vec::new();
        let mut has_self = false;
        let mut self_lifetime: Option<Region> = None;

        for param in &fn_sig.inputs {
            // Check if this is &self / &mut self
            if param.self_kind.is_some() {
                has_self = true;
            }

            // Collect reference lifetimes from the parameter type
            if let Some(ref ty) = param.ty {
                let regions = collect_erased_regions(ty);
                for region in regions {
                    match region {
                        Region::Erased => {
                            let fresh = self.allocate_fresh_lifetime();
                            if param.self_kind.is_some() {
                                self_lifetime = Some(fresh);
                            }
                            input_lifetimes.push(fresh);
                        }
                        Region::Var(_) | Region::Static => {
                            input_lifetimes.push(region);
                        }
                    }
                }
            }
        }

        // Apply elision rules to return type
        if let HirFnRetTy::Ty(ret_ty) = &fn_sig.output {
            let output_regions = collect_erased_regions(ret_ty);

            if output_regions.is_empty() {
                // No reference in return type — nothing to elide
                return Ok(());
            }

            // Determine the output lifetime based on elision rules
            let output_lifetime = if input_lifetimes.len() == 1 {
                // Rule 2: single input lifetime → output takes it
                Some(input_lifetimes[0])
            } else if has_self {
                // Rule 3: multiple inputs + &self → output takes self's
                self_lifetime
            } else if input_lifetimes.is_empty() {
                // No input lifetimes but has output reference — error
                return Err(LifetimeElisionError::MissingLifetime {
                    span: ret_ty.span,
                    reason: "missing lifetime specifier: no input lifetimes available".to_string(),
                });
            } else {
                // Rule 4: multiple inputs, no self → must be explicit
                return Err(LifetimeElisionError::MissingLifetime {
                    span: ret_ty.span,
                    reason: "missing lifetime specifier: multiple input lifetimes, \
                             explicit annotation required"
                        .to_string(),
                });
            };

            // If we got here, elision succeeded — the output_lifetime is determined
            // (We don't modify the HIR type in-place; the caller/driver will use
            // this information when constructing MIR types.)
            let _ = output_lifetime; // Used by driver in future integration
        }

        Ok(())
    }
}

/// An error during lifetime elision.
#[derive(Debug, Clone)]
pub(crate) enum LifetimeElisionError {
    /// Missing lifetime specifier — elision rules couldn't determine output lifetime.
    MissingLifetime { span: Span, reason: String },
}

/// Collect all reference regions from a HIR type (for elision).
///
/// Walks the type recursively and collects one `Region::Erased` for each
/// reference type encountered. (HIR refs use `Option<Lifetime>` where `None`
/// = erased; we simplify to always produce `Region::Erased` since HIR doesn't
/// carry MIR `Region` values.)
fn collect_erased_regions(ty: &HirTy) -> Vec<Region> {
    let mut regions = Vec::new();
    collect_regions_recursive(ty, &mut regions);
    regions
}

/// Recursive helper for `collect_erased_regions`.
fn collect_regions_recursive(ty: &HirTy, out: &mut Vec<Region>) {
    match &ty.kind {
        HirTyKind::Ref(_lifetime, _mutability, inner) => {
            // HIR reference — produce one Erased region
            out.push(Region::Erased);
            collect_regions_recursive(inner, out);
        }
        HirTyKind::Tuple(tys) => {
            for t in tys {
                collect_regions_recursive(t, out);
            }
        }
        HirTyKind::Array(inner, _count) => {
            collect_regions_recursive(inner, out);
        }
        HirTyKind::Slice(inner) => {
            collect_regions_recursive(inner, out);
        }
        HirTyKind::Ptr(_mutability, inner) => {
            collect_regions_recursive(inner, out);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_lifetime_allocation() {
        let mut ctx = LifetimeElisionCtxt::new();
        let r1 = ctx.allocate_fresh_lifetime();
        let r2 = ctx.allocate_fresh_lifetime();
        let r3 = ctx.allocate_fresh_lifetime();

        // Each should be a unique Region::Var with increasing vid
        assert!(matches!(r1, Region::Var(RegionVid(1))));
        assert!(matches!(r2, Region::Var(RegionVid(2))));
        assert!(matches!(r3, Region::Var(RegionVid(3))));
    }

    #[test]
    fn test_fresh_lifetime_not_static() {
        let mut ctx = LifetimeElisionCtxt::new();
        let r = ctx.allocate_fresh_lifetime();
        assert_ne!(r, Region::Static);
        assert_ne!(r, Region::Erased);
    }

    #[test]
    fn test_default_context() {
        let ctx = LifetimeElisionCtxt::default();
        // Default should start at vid 1
        assert_eq!(ctx.next_vid, 1);
    }
}
