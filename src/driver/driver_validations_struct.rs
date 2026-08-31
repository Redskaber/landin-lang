//! Driver struct literal validations: field conformance, in-expr checks, single-literal validation.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 30.22):
//! Extracted from `driver_validations.rs` to satisfy J2 (单一职责) + J6 (科学合理粒度).
//! This file owns all struct-literal validations: field type/arity conformance,
//! struct literal in expression context, and single-literal validation.

use crate::hir::*;
use crate::typeck::TypeError;

/// Stage 18.72 P1-A: Validate struct literal field counts against struct
/// definitions.
///
/// For each `HirExprKind::Struct { path, fields }` expression in the HIR:
///   1. Resolve `path.res` to a struct DefId
///   2. Look up the struct's declared field names
///   3. Check for:
///      - Unknown fields (field name not in declaration)
///      - Duplicate fields (same name appears twice in literal)
///      - Missing fields (declared field not provided in literal)
///
/// Per §1.0 原则 4 "报错 > 静默": all three error types must be reported.
/// Per §1.0 原则 6 "通用 > 特例": one validator walks all bodies.
/// Per §10 naming: `validate_struct_literal_fields` follows
///   `validate_<noun>_<noun>_<noun>` pattern.
pub(super) fn validate_struct_literal_fields(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::{HirExprKind, HirStmt};

    // Build a lookup table: struct DefId → Vec<Spur> (field names).
    // Per §1.0 原則 6: one lookup table for all structs.
    let mut struct_fields_by_def_id: std::collections::HashMap<
        crate::hir::DefId,
        Vec<lasso::Spur>,
    > = std::collections::HashMap::new();
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(HirItem::Struct(s)) = owner {
            let field_names: Vec<lasso::Spur> = s
                .fields
                .iter()
                .filter_map(|f| f.ident.as_ref().map(|i| i.name))
                .collect();
            struct_fields_by_def_id.insert(s.hir_id.owner, field_names);
        }
    }

    // Walk all bodies and check struct literals.
    for (_, owner) in &hir.owners {
        // Extract BodyId from owner (Fn/Const/Static have bodies).
        // Per §2.2 原則 3 "显式 > 隐式" + §12 最优>最小 (Stage 18.127):
        // Use `if let Some(body)` pattern instead of `if f.body.is_some() => f.body.unwrap()`.
        let body_id = match owner {
            crate::hir::OwnerNode::Item(HirItem::Fn(f)) => match f.body {
                Some(b) => b,
                None => continue,
            },
            crate::hir::OwnerNode::Item(HirItem::Const(c)) => c.body,
            crate::hir::OwnerNode::Item(HirItem::Static(s)) => s.body,
            _ => continue,
        };
        let body = match hir.find_body(body_id) {
            Some(b) => b,
            None => continue,
        };
        // Walk all statements + trailing expr in the body.
        // body.value is HirExpr — if it's a Block, walk its stmts + expr.
        let mut exprs_to_check: Vec<&crate::hir::HirExpr> = Vec::new();
        if let HirExprKind::Block(block) = &body.value.kind {
            for stmt in &block.stmts {
                if let HirStmt::Expr(e, _) = stmt {
                    exprs_to_check.push(e);
                } else if let HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        exprs_to_check.push(init);
                    }
                }
            }
            if let Some(trailing) = &block.expr {
                exprs_to_check.push(trailing);
            }
        } else {
            exprs_to_check.push(&body.value);
        }

        for expr in exprs_to_check {
            check_struct_literal_in_expr(expr, &struct_fields_by_def_id, interner, errors);
        }
    }
}

/// Recursively walk an expression tree and validate all struct literals.
pub(super) fn check_struct_literal_in_expr(
    expr: &crate::hir::HirExpr,
    struct_fields: &std::collections::HashMap<crate::hir::DefId, Vec<lasso::Spur>>,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::HirExprKind;
    match &expr.kind {
        HirExprKind::Struct { path, fields } => {
            // Try to resolve path to a struct DefId.
            if let crate::hir::Res::Def(def_id, crate::resolve::DefKind::Struct) = path.res {
                if let Some(declared_fields) = struct_fields.get(&def_id) {
                    validate_one_struct_literal(
                        fields,
                        declared_fields,
                        interner,
                        expr.span,
                        errors,
                    );
                }
            }
            // Recurse into field expressions.
            for f in fields {
                if let Some(e) = &f.expr {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                }
            }
        }
        // Recurse into other expression kinds that may contain struct literals.
        HirExprKind::Call { func, args, .. } => {
            check_struct_literal_in_expr(func, struct_fields, interner, errors);
            for arg in args {
                check_struct_literal_in_expr(arg, struct_fields, interner, errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            check_struct_literal_in_expr(receiver, struct_fields, interner, errors);
            for arg in args {
                check_struct_literal_in_expr(arg, struct_fields, interner, errors);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            check_struct_literal_in_expr(receiver, struct_fields, interner, errors);
        }
        HirExprKind::Unary { expr: inner, .. } => {
            check_struct_literal_in_expr(inner, struct_fields, interner, errors);
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            check_struct_literal_in_expr(lhs, struct_fields, interner, errors);
            check_struct_literal_in_expr(rhs, struct_fields, interner, errors);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            check_struct_literal_in_expr(cond, struct_fields, interner, errors);
            for stmt in &then.stmts {
                if let crate::hir::HirStmt::Expr(e, _) = stmt {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                } else if let crate::hir::HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        check_struct_literal_in_expr(init, struct_fields, interner, errors);
                    }
                }
            }
            if let Some(trailing) = &then.expr {
                check_struct_literal_in_expr(trailing, struct_fields, interner, errors);
            }
            if let Some(e) = else_ {
                check_struct_literal_in_expr(e, struct_fields, interner, errors);
            }
        }
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => {
            check_struct_literal_in_expr(scrutinee, struct_fields, interner, errors);
            for arm in arms {
                if let Some(e) = &arm.guard {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                }
                // arm.body is Box<HirExpr>, not a Block — recurse directly.
                check_struct_literal_in_expr(&arm.body, struct_fields, interner, errors);
            }
        }
        HirExprKind::Block(block) => {
            for stmt in &block.stmts {
                if let crate::hir::HirStmt::Expr(e, _) = stmt {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                } else if let crate::hir::HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        check_struct_literal_in_expr(init, struct_fields, interner, errors);
                    }
                }
            }
            if let Some(trailing) = &block.expr {
                check_struct_literal_in_expr(trailing, struct_fields, interner, errors);
            }
        }
        HirExprKind::Return { expr: Some(e), .. } => {
            check_struct_literal_in_expr(e, struct_fields, interner, errors);
        }
        _ => {}
    }
}

/// Validate a single struct literal against its declared fields.
pub(super) fn validate_one_struct_literal(
    fields: &[crate::hir::HirExprField],
    declared_fields: &[lasso::Spur],
    interner: &lasso::Rodeo,
    span: crate::session::Span,
    errors: &mut Vec<TypeError>,
) {
    // Check for unknown + duplicate fields.
    let mut seen: std::collections::HashSet<lasso::Spur> = std::collections::HashSet::new();
    for f in fields {
        let name = f.ident.name;
        if !declared_fields.contains(&name) {
            let name_str = interner.try_resolve(&name).unwrap_or("?");
            errors.push(TypeError::new(
                format!("struct has no field `{}`", name_str),
                f.span,
            ));
        } else if !seen.insert(name) {
            let name_str = interner.try_resolve(&name).unwrap_or("?");
            errors.push(TypeError::new(
                format!("field `{}` specified more than once", name_str),
                f.span,
            ));
        }
    }

    // Check for missing fields (only if no unknown/duplicate errors).
    // Per §1.0 原則 4: report missing fields too.
    let provided: std::collections::HashSet<lasso::Spur> =
        fields.iter().map(|f| f.ident.name).collect();
    let missing: Vec<&lasso::Spur> = declared_fields
        .iter()
        .filter(|name| !provided.contains(name))
        .collect();
    if !missing.is_empty() {
        let missing_names: Vec<&str> = missing
            .iter()
            .map(|s| interner.try_resolve(s).unwrap_or("?"))
            .collect();
        errors.push(TypeError::new(
            format!("missing field(s): {}", missing_names.join(", ")),
            span,
        ));
    }
}
