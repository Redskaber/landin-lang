//! Driver scan functions: unresolved path scanning + HIR walking helpers.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.134):
//! Extracted from `driver.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).

use crate::hir::*;

use super::CompileErrors;

/// G4 fix: Scan HIR for unresolved paths after name resolution.
///
/// Any `HirPath` with `Res::Unknown` or `Res::Err` indicates an undefined
/// name (e.g., calling `undefined_fn()` or referring to an undefined
/// variable). Emit a resolve error for each.
///
/// Without this scan, undefined names silently fall through to
/// `Ty::Error` in MIR lower, which typeck treats as "always succeeds"
/// (intentional error recovery). The result: typos in function names
/// go undetected.
pub(super) fn scan_for_unresolved_paths(hir: &HirCrate, errors: &mut CompileErrors) {
    for (_, body) in &hir.bodies {
        scan_expr_for_unresolved(&body.value, errors);
        for param in &body.params {
            if let Some(ty) = &param.ty {
                scan_ty_for_unresolved(ty, errors);
            }
            scan_pat_for_unresolved(&param.pat, errors);
        }
    }
}

pub(super) fn scan_expr_for_unresolved(expr: &crate::hir::HirExpr, errors: &mut CompileErrors) {
    use crate::hir::{HirExprKind, Res};
    match &expr.kind {
        HirExprKind::Path(p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindValue,
                    "cannot find value in this scope".to_string(),
                    p.span,
                ));
            }
        }
        HirExprKind::Block(b) => {
            for stmt in &b.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &b.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            scan_expr_for_unresolved(lhs, errors);
            scan_expr_for_unresolved(rhs, errors);
        }
        HirExprKind::Unary { expr: inner, .. } => {
            scan_expr_for_unresolved(inner, errors);
        }
        HirExprKind::Call { func, args, .. } => {
            scan_expr_for_unresolved(func, errors);
            for a in args {
                scan_expr_for_unresolved(a, errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_for_unresolved(receiver, errors);
            for a in args {
                scan_expr_for_unresolved(a, errors);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            scan_expr_for_unresolved(receiver, errors);
        }
        HirExprKind::Index {
            receiver, index, ..
        } => {
            scan_expr_for_unresolved(receiver, errors);
            scan_expr_for_unresolved(index, errors);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            scan_expr_for_unresolved(cond, errors);
            for stmt in &then.stmts {
                use crate::hir::HirStmt;
                if let HirStmt::Expr(e, _) = stmt {
                    scan_expr_for_unresolved(e, errors);
                }
            }
            if let Some(e) = &then.expr {
                scan_expr_for_unresolved(e, errors);
            }
            if let Some(e) = else_ {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => {
            scan_expr_for_unresolved(scrutinee, errors);
            for arm in arms {
                scan_pat_for_unresolved(&arm.pat, errors);
                scan_expr_for_unresolved(&arm.body, errors);
            }
        }
        HirExprKind::Return { expr } | HirExprKind::Break { expr } => {
            if let Some(e) = expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            scan_expr_for_unresolved(lhs, errors);
            scan_expr_for_unresolved(rhs, errors);
        }
        HirExprKind::Tuple { elems, .. } => {
            for e in elems {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Array { elems, .. } => {
            for e in elems {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Struct { fields, .. } => {
            for f in fields {
                if let Some(e) = &f.expr {
                    scan_expr_for_unresolved(e, errors);
                }
            }
        }
        HirExprKind::Cast {
            expr: inner, ty, ..
        } => {
            scan_expr_for_unresolved(inner, errors);
            scan_ty_for_unresolved(ty, errors);
        }
        HirExprKind::AddrOf { expr: inner, .. } => {
            scan_expr_for_unresolved(inner, errors);
        }
        HirExprKind::Loop { body, .. } | HirExprKind::While { body, .. } => {
            for stmt in &body.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &body.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA2 fix): For-loop body must be scanned for
        // unresolved paths. Previously the `_ => {}` catch-all skipped For,
        // so `for i in 0..5 { let _ = nonexistent_xyz; }` silently compiled.
        HirExprKind::For { iter, body, .. } => {
            scan_expr_for_unresolved(iter, errors);
            for stmt in &body.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &body.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA3 fix): Range start/end must be scanned.
        // Previously the catch-all skipped Range, so `for i in foo..5 {}`
        // silently used foo=0.
        HirExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_for_unresolved(s, errors);
            }
            if let Some(e) = end {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA4 fix): Repeat elem/count must be scanned.
        // Previously the catch-all skipped Repeat, so `let arr = [foo; 3];`
        // silently used foo=0.
        HirExprKind::Repeat { elem, count } => {
            scan_expr_for_unresolved(elem, errors);
            scan_expr_for_unresolved(count, errors);
        }
        // Stage 18.48: HirExprKind::Println variant removed.
        HirExprKind::Closure { body, .. } => scan_expr_for_unresolved(body, errors),
        // Stage 14.101 (Phase 1 audit fix): Scan Try expr, Unsafe block,
        // MacroCall path, Await expr, Async block.
        // Previously the catch-all silently skipped these, so unresolved
        // paths inside them went unreported.
        HirExprKind::Try { expr, .. } => scan_expr_for_unresolved(expr, errors),
        HirExprKind::Unsafe(block) => {
            for stmt in &block.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &block.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::MacroCall { path, .. } => {
            // Stage 14.101: MacroCall path resolution. Built-in macros
            // (vec!, println!, assert!, etc.) are single-segment paths that
            // the resolver doesn't resolve to Res::Def — they're handled
            // specially during HIR lowering. Only report multi-segment paths
            // (e.g., `std::println!`) as errors if unresolved.
            //
            // Per §1.0 原则 5 "报错 > 静默": unresolved macro paths should be
            // reported, but we must not false-positive on built-in macros.
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindMacro,
                    "cannot find macro in this scope".to_string(),
                    path.span,
                ));
            }
        }
        HirExprKind::Await { expr, .. } => scan_expr_for_unresolved(expr, errors),
        HirExprKind::Async { block, .. } => {
            for stmt in &block.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &block.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Lit, Unit, Continue — genuinely no sub-expressions
        HirExprKind::Lit(_) | HirExprKind::Unit | HirExprKind::Continue => {}
    }
}

pub(super) fn scan_pat_for_unresolved(pat: &crate::hir::HirPat, errors: &mut CompileErrors) {
    // Stage 14.101 (Phase 1 audit fix): Re-enabled pattern scanning.
    //
    // Previously this was a no-op stub (G4 fix) because enum variant patterns
    // like `Circle(r)` appeared as Res::Unknown. However, this meant unresolved
    // IDENTIFIER patterns (e.g., `match x { nonexistent => ... }`) were also
    // silently accepted.
    //
    // Now we scan patterns but ONLY report paths that resolve to Res::Unknown
    // AND are not enum variant patterns. We detect enum variant patterns by
    // checking if the path has multiple segments (e.g., `Color::Red`) — single-
    // segment paths in TupleStruct/Struct/Path patterns might be enum variants
    // (resolved lazily during typeck) so we skip them.
    //
    // Per §1.0 原则 5 "报错 > 静默": unresolved identifiers in patterns should
    // be reported. Per §1.0 原则 6 "通用 > 特例": one rule handles all pattern
    // kinds by recursing into sub-patterns.
    use crate::hir::{HirPatKind, Res};
    match &pat.kind {
        HirPatKind::Wild | HirPatKind::Rest | HirPatKind::Lit(_) => {}
        HirPatKind::Ident(_mode, ident, sub) => {
            // Ident patterns bind a new variable — they don't reference an
            // existing path. No resolution check needed.
            let _ = ident;
            if let Some(s) = sub {
                scan_pat_for_unresolved(s, errors);
            }
        }
        HirPatKind::Struct(path, fields, _has_rest) => {
            // Multi-segment paths (e.g., `Color::Red { ... }`) should be resolved.
            // Single-segment paths might be enum variants (lazily resolved).
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
            for f in fields {
                scan_pat_for_unresolved(&f.pat, errors);
            }
        }
        HirPatKind::TupleStruct(path, sub_pats) => {
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Tuple(sub_pats) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Slice(sub_pats, rest) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
            if let Some(r) = rest {
                scan_pat_for_unresolved(r, errors);
            }
        }
        HirPatKind::Or(sub_pats) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Path(path) => {
            // Multi-segment paths (e.g., `Color::Red`) should be resolved.
            // Single-segment paths might be enum variants (lazily resolved).
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
        }
        HirPatKind::Range(start, end, _) => {
            if let Some(s) = start {
                scan_expr_for_unresolved(s, errors);
            }
            if let Some(e) = end {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirPatKind::Ref(sub, _) => {
            scan_pat_for_unresolved(sub, errors);
        }
    }
}

/// Stage 16.65 (Task 14 Phase 2): Check object safety for all `dyn Trait` usages.
///
/// Scans all HIR types for `HirTyKind::TraitObject`. For each, resolves the
/// trait DefId from the bound's path, looks up the `HirTrait` definition,
/// and calls `check_trait_object_safety`. If any violations are found, emits
/// typeck errors.
///
/// Per §23: `check_object_safety_for_dyn_trait_usage` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR + TraitResolver (allowed during driver pre-computation).
pub(super) fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    use crate::hir::{HirTyKind, Res};
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    // Stage 15.87: use the type path's span (was:
                    // Span::DUMMY, producing "1:1" for type resolution
                    // errors like `let x: Undefined = 42;`).
                    //
                    // Per §1.0 原則 3 "显式 > 隐式": error spans are
                    // explicitly sourced from the type path.
                    // Per §1.0 原則 4 "报错 > 静默": error locations
                    // are accurate, not cryptic.
                    p.span,
                ));
            }
        }
        HirTyKind::Ref(_, _, inner)
        | HirTyKind::Ptr(_, inner)
        | HirTyKind::Slice(inner)
        | HirTyKind::Array(inner, _) => scan_ty_for_unresolved(inner, errors),
        HirTyKind::Tuple(tys) => {
            for t in tys {
                scan_ty_for_unresolved(t, errors);
            }
        }
        // Stage 14.101 (Phase 1 audit fix): FnPtr inputs/output must be scanned.
        // Previously the catch-all silently skipped FnPtr, so
        // `fn(unresolved) -> i32` went unreported.
        HirTyKind::FnPtr { inputs, output, .. } => {
            for t in inputs {
                scan_ty_for_unresolved(t, errors);
            }
            scan_ty_for_unresolved(output, errors);
        }
        // Stage 14.101 (Phase 1 audit fix): TraitObject bounds must be scanned.
        HirTyKind::TraitObject { bounds, .. } => {
            for bound in bounds {
                scan_type_bound_for_unresolved(bound, errors);
            }
        }
        // Stage 14.101 (Phase 1 audit fix): ImplTrait bounds must be scanned.
        HirTyKind::ImplTrait(bounds) => {
            for bound in bounds {
                scan_type_bound_for_unresolved(bound, errors);
            }
        }
        // Bool, Char, Int, Uint, Float, Never, Infer — no sub-types
        HirTyKind::Bool
        | HirTyKind::Char
        | HirTyKind::Int(_)
        | HirTyKind::Uint(_)
        | HirTyKind::Float(_)
        | HirTyKind::Never
        | HirTyKind::Infer => {}
    }
}

/// Stage 14.101 (Phase 1 audit fix): Scan a type bound for unresolved paths.
/// Used by TraitObject and ImplTrait scanning.
pub(super) fn scan_type_bound_for_unresolved(
    bound: &crate::hir::HirTypeBound,
    errors: &mut CompileErrors,
) {
    use crate::hir::Res;
    if let crate::hir::HirTypeBound::Trait(trait_bound) = bound {
        let path = &trait_bound.path;
        if matches!(path.res, Res::Unknown | Res::Err) {
            errors.resolve.push(crate::resolve::ResolveError::with_kind(
                crate::resolve::ResolveErrorKind::CannotFindTrait,
                "cannot find trait in this scope".to_string(),
                path.span,
            ));
        }
    }
}

/// Compile a source string and assert that there are zero errors.
///
/// Intended for use in integration tests where any error is a bug.
/// Returns the CompileResult on success; panics with a detailed
/// breakdown on failure.
pub(super) fn walk_hir_ty<F>(ty: &crate::hir::HirTy, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    use crate::hir::HirTyKind;
    f(ty);
    match &ty.kind {
        HirTyKind::Ref(_, _, inner) | HirTyKind::Ptr(_, inner) | HirTyKind::Slice(inner) => {
            walk_hir_ty(inner, f);
        }
        HirTyKind::Array(inner, _) => walk_hir_ty(inner, f),
        HirTyKind::Tuple(tys) => {
            for t in tys {
                walk_hir_ty(t, f);
            }
        }
        // Stage 16.71: FnPtr — recurse into inputs and output
        HirTyKind::FnPtr { inputs, output, .. } => {
            for t in inputs {
                walk_hir_ty(t, f);
            }
            walk_hir_ty(output, f);
        }
        // Stage 18.61: TraitObject / ImplTrait — recurse into bounds
        // (bounds contain HirTypeBound::Trait(path) which has paths to scan).
        // Per §1.0 原則 2 "整体 > 局部": walker must cover all type variants.
        HirTyKind::TraitObject { bounds, .. } | HirTyKind::ImplTrait(bounds) => {
            for bound in bounds {
                if let crate::hir::HirTypeBound::Trait(tb) = bound {
                    // The trait bound's path may have generic args with types.
                    // Walk the path segments' args.
                    for seg in &tb.path.segments {
                        if let Some(crate::ast::GenericArgs::AngleBracketed(_)) = &seg.args {
                            // AST Ty args — can't walk via walk_hir_ty (needs HirTy).
                            // The resolver will catch unresolved paths here
                            // via resolve_ty_paths during resolution.
                        }
                    }
                }
            }
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}

/// Walk a HirExpr for HirTy occurrences (in cast expressions, let bindings, etc.).
pub(super) fn walk_hir_ty_in_body<F>(expr: &crate::hir::HirExpr, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    use crate::hir::HirExprKind;
    match &expr.kind {
        HirExprKind::Cast { expr, ty } => {
            walk_hir_ty_in_body(expr, f);
            walk_hir_ty(ty, f);
        }
        HirExprKind::Call { func, args } => {
            walk_hir_ty_in_body(func, f);
            for arg in args {
                walk_hir_ty_in_body(arg, f);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            walk_hir_ty_in_body(receiver, f);
            for arg in args {
                walk_hir_ty_in_body(arg, f);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            walk_hir_ty_in_body(receiver, f);
        }
        HirExprKind::Index { receiver, index } => {
            walk_hir_ty_in_body(receiver, f);
            walk_hir_ty_in_body(index, f);
        }
        HirExprKind::AddrOf { expr, .. } => {
            walk_hir_ty_in_body(expr, f);
        }
        HirExprKind::Unary { expr, .. } => {
            walk_hir_ty_in_body(expr, f);
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            walk_hir_ty_in_body(lhs, f);
            walk_hir_ty_in_body(rhs, f);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            walk_hir_ty_in_body(cond, f);
            walk_hir_block(then, f);
            if let Some(e) = else_ {
                walk_hir_ty_in_body(e, f);
            }
        }
        HirExprKind::Match { expr, arms } => {
            walk_hir_ty_in_body(expr, f);
            for arm in arms {
                walk_hir_ty_in_body(&arm.body, f);
            }
        }
        HirExprKind::Block(block) => {
            walk_hir_block(block, f);
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}

/// Walk a HirBlock for HirTy occurrences.
pub(super) fn walk_hir_block<F>(block: &crate::hir::HirBlock, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    for stmt in &block.stmts {
        walk_hir_ty_in_stmt(stmt, f);
    }
    if let Some(expr) = &block.expr {
        walk_hir_ty_in_body(expr, f);
    }
}

/// Walk a HirStmt for HirTy occurrences.
pub(super) fn walk_hir_ty_in_stmt<F>(stmt: &crate::hir::HirStmt, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    match stmt {
        crate::hir::HirStmt::Local(local) => {
            if let Some(ty) = &local.ty {
                walk_hir_ty(ty, f);
            }
            if let Some(init) = &local.init {
                walk_hir_ty_in_body(init, f);
            }
        }
        crate::hir::HirStmt::Expr(expr, _) => {
            walk_hir_ty_in_body(expr, f);
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}
