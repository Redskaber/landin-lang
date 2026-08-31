#!/usr/bin/env python3
"""
Stage 31.6e: Add SizeOf handling to all match-exhaustive locations.

Adds SizeOf arms to:
1. driver/driver_scan.rs — scan_expr_for_unresolved (no sub-exprs)
2. hir/kinds.rs — hir_expr_kind_to_string
3. hir/lower/body.rs — lower_expr + expr_span
4. mir/lower/expr_operand.rs — lower_expr_to_operand (evaluate size)
5. mir/lower/closure_capture.rs — collect_captured_locals (no captures)
6. resolve/path_resolve.rs — resolve_expr (no resolution needed)
7. parser/expr.rs — parse sizeof + ExprSpan trait
"""
import re
from pathlib import Path

BASE = Path("/home/z/my-project/landin-stage0/src")

# 1. driver/driver_scan.rs — after FatPtrLit arm
f = BASE / "driver/driver_scan.rs"
content = f.read_text()
old = """        // Stage 31.1 (v0.19): FatPtrLit — scan ptr + len sub-expressions.
        HirExprKind::FatPtrLit { ptr, len, .. } => {
            scan_expr_for_unresolved(ptr, errors);
            scan_expr_for_unresolved(len, errors);
        }
    }
}"""
new = """        // Stage 31.1 (v0.19): FatPtrLit — scan ptr + len sub-expressions.
        HirExprKind::FatPtrLit { ptr, len, .. } => {
            scan_expr_for_unresolved(ptr, errors);
            scan_expr_for_unresolved(len, errors);
        }
        // Stage 31.6e: SizeOf — no sub-expressions to scan.
        HirExprKind::SizeOf { .. } => {}
    }
}"""
content = content.replace(old, new)
f.write_text(content)
print(f"✓ {f.name}")

# 2. hir/kinds.rs — hir_expr_kind_to_string
f = BASE / "hir/kinds.rs"
content = f.read_text()
old = """        // Stage 31.1 (v0.19): Fat pointer literal.
        HirExprKind::FatPtrLit { .. } => "fat pointer literal",
    }
}"""
new = """        // Stage 31.1 (v0.19): Fat pointer literal.
        HirExprKind::FatPtrLit { .. } => "fat pointer literal",
        // Stage 31.6e: sizeof expression.
        HirExprKind::SizeOf { .. } => "sizeof expression",
    }
}"""
content = content.replace(old, new)
f.write_text(content)
print(f"✓ {f.name}")

# 3. hir/lower/body.rs — lower_expr + expr_span
f = BASE / "hir/lower/body.rs"
content = f.read_text()

# lower_expr
old_lower = """        // Stage 31.1 (v0.19): Fat pointer literal `&str { ptr: expr, len: expr }`
        Expr::FatPtrLit {
            target_ty,
            ptr,
            len,
            ..
        } => {
            let hir_target_ty = ty::lower_ty(cx, target_ty);
            let hir_ptr = lower_expr(cx, ptr);
            let hir_len = lower_expr(cx, len);
            HirExprKind::FatPtrLit {
                target_ty: hir_target_ty,
                ptr: Box::new(hir_ptr),
                len: Box::new(hir_len),
            }
        }
    };"""
new_lower = """        // Stage 31.1 (v0.19): Fat pointer literal `&str { ptr: expr, len: expr }`
        Expr::FatPtrLit {
            target_ty,
            ptr,
            len,
            ..
        } => {
            let hir_target_ty = ty::lower_ty(cx, target_ty);
            let hir_ptr = lower_expr(cx, ptr);
            let hir_len = lower_expr(cx, len);
            HirExprKind::FatPtrLit {
                target_ty: hir_target_ty,
                ptr: Box::new(hir_ptr),
                len: Box::new(hir_len),
            }
        }
        // Stage 31.6e (v0.19): `sizeof TYPE` — compile-time type size.
        Expr::SizeOf { ty, .. } => {
            let hir_ty = ty::lower_ty(cx, ty);
            HirExprKind::SizeOf { ty: hir_ty }
        }
    };"""
content = content.replace(old_lower, new_lower)

# expr_span
old_span = """        // Stage 31.1 (v0.19): Fat pointer literal.
        FatPtrLit { span, .. } => *span,
    }
}"""
new_span = """        // Stage 31.1 (v0.19): Fat pointer literal.
        FatPtrLit { span, .. } => *span,
        // Stage 31.6e: sizeof expression.
        SizeOf { span, .. } => *span,
    }
}"""
content = content.replace(old_span, new_span)
f.write_text(content)
print(f"✓ {f.name}")

# 4. mir/lower/expr_operand.rs — lower_expr_to_operand
f = BASE / "mir/lower/expr_operand.rs"
content = f.read_text()
old = """        // Stage 31.1 (v0.19): Fat pointer literal `&str { ptr: expr, len: expr }`
        HirExprKind::FatPtrLit { target_ty, ptr, len } => {
            lower_fat_ptr_lit(cx, expr, target_ty, ptr, len)
        }"""
new = """        // Stage 31.1 (v0.19): Fat pointer literal `&str { ptr: expr, len: expr }`
        HirExprKind::FatPtrLit { target_ty, ptr, len } => {
            lower_fat_ptr_lit(cx, expr, target_ty, ptr, len)
        }

        // Stage 31.6e (v0.19): `sizeof TYPE` — compile-time type size.
        // Evaluates to a usize constant at MIR lower time.
        // Per §1.0 原則 6 (通解 > 特解): one sizeof for all types.
        // Per §12 (最优 > 最小): root-cause fix via language feature.
        HirExprKind::SizeOf { ty } => {
            let mir_ty = crate::mir::lower::ty_lower::lower_hir_ty_to_mir_ty_with_hir(ty, cx.hir);
            let size = crate::mir::lower::adt_layout::compute_type_size_with_fallback(
                &mir_ty,
                cx.hir,
                8, // fallback for Param/Infer/Error
            );
            let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Use(Operand::Constant(crate::mir::ty::Const {
                    ty: usize_ty.clone(),
                    val: crate::mir::ty::ConstVal::Uint(size as u128),
                })),
                usize_ty,
                expr.span,
            )
        }"""
content = content.replace(old, new)
f.write_text(content)
print(f"✓ {f.name}")

# 5. mir/lower/closure_capture.rs
f = BASE / "mir/lower/closure_capture.rs"
content = f.read_text()
old = """        // Stage 31.1 (v0.19): FatPtrLit — collect from ptr + len sub-expressions.
        HirExprKind::FatPtrLit { ptr, len, .. } => {
            collect_captured_locals(cx, ptr, param_hir_ids, captured, seen);
            collect_captured_locals(cx, len, param_hir_ids, captured, seen);
        }
    }
}"""
new = """        // Stage 31.1 (v0.19): FatPtrLit — collect from ptr + len sub-expressions.
        HirExprKind::FatPtrLit { ptr, len, .. } => {
            collect_captured_locals(cx, ptr, param_hir_ids, captured, seen);
            collect_captured_locals(cx, len, param_hir_ids, captured, seen);
        }
        // Stage 31.6e: SizeOf — no sub-expressions to collect.
        HirExprKind::SizeOf { .. } => {}
    }
}"""
content = content.replace(old, new)
f.write_text(content)
print(f"✓ {f.name}")

# 6. resolve/path_resolve.rs
f = BASE / "resolve/path_resolve.rs"
content = f.read_text()
old = """            // Stage 31.1 (v0.19): Fat pointer literal — resolve ptr + len sub-exprs.
            HirExprKind::FatPtrLit { ptr, len, .. } => {
                self.resolve_expr(ptr, interner);
                self.resolve_expr(len, interner);
            }
        }
    }"""
new = """            // Stage 31.1 (v0.19): Fat pointer literal — resolve ptr + len sub-exprs.
            HirExprKind::FatPtrLit { ptr, len, .. } => {
                self.resolve_expr(ptr, interner);
                self.resolve_expr(len, interner);
            }
            // Stage 31.6e: SizeOf — no sub-expressions to resolve.
            HirExprKind::SizeOf { .. } => {}
        }
    }"""
content = content.replace(old, new)
f.write_text(content)
print(f"✓ {f.name}")

# 7. parser/expr.rs — parse sizeof + ExprSpan trait
f = BASE / "parser/expr.rs"
content = f.read_text()

# Parse sizeof in parse_unary_expr
old_unary = """            TokenKind::And => {"""
new_unary = """            // Stage 31.6e (v0.19): `sizeof TYPE` — compile-time type size.
            TokenKind::KwSizeof => {
                self.bump(); // consume `sizeof`
                let ty = self.parse_ty();
                Expr::SizeOf {
                    ty,
                    span: self.current_span(),
                }
            }
            TokenKind::And => {"""
content = content.replace(old_unary, new_unary, 1)  # only first occurrence

# ExprSpan trait impl
old_span = """            // Stage 31.1 (v0.19): Fat pointer literal.
            Expr::FatPtrLit { span, .. } => *span,
        }
    }
}"""
new_span = """            // Stage 31.1 (v0.19): Fat pointer literal.
            Expr::FatPtrLit { span, .. } => *span,
            // Stage 31.6e: sizeof expression.
            Expr::SizeOf { span, .. } => *span,
        }
    }
}"""
content = content.replace(old_span, new_span)

# is_block_like_expr — add SizeOf to the match (not block-like)
# Actually, let's check if is_block_like_expr needs updating
# The match in is_block_like_expr uses explicit patterns, so SizeOf is fine
# (it falls through to the default `false`).

f.write_text(content)
print(f"✓ {f.name}")

print("\nDone. Now build + test.")
