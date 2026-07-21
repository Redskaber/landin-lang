//! Type lowering.

use crate::ast::{self};
use crate::hir::kinds::*;
use crate::hir::lower::cx::HirLowerCtxt;
use crate::hir::lower::path;
use crate::session::Span;

pub fn lower_ty(cx: &mut HirLowerCtxt, ty: &ast::Ty) -> HirTy {
    let hir_id = cx.fresh_hir_id();
    let span = ty_span(ty);
    let kind = match ty {
        ast::Ty::Bool(_) => HirTyKind::Bool,
        ast::Ty::Char(_) => HirTyKind::Char,
        ast::Ty::Int(int_ty, _) => HirTyKind::Int(*int_ty),
        ast::Ty::Uint(uint_ty, _) => HirTyKind::Uint(*uint_ty),
        ast::Ty::Float(float_ty, _) => HirTyKind::Float(*float_ty),
        ast::Ty::Never(_) => HirTyKind::Never,
        ast::Ty::Tuple(tys, _) => HirTyKind::Tuple(tys.iter().map(|t| lower_ty(cx, t)).collect()),
        ast::Ty::Array(ty, count, _) => HirTyKind::Array(
            Box::new(lower_ty(cx, ty)),
            Box::new(crate::hir::lower::body::lower_expr(cx, count)),
        ),
        ast::Ty::Slice(ty, _) => HirTyKind::Slice(Box::new(lower_ty(cx, ty))),
        ast::Ty::Ref(lt, mutability, ty, _) => {
            HirTyKind::Ref(lt.clone(), *mutability, Box::new(lower_ty(cx, ty)))
        }
        ast::Ty::Ptr(mutability, ty, _) => HirTyKind::Ptr(*mutability, Box::new(lower_ty(cx, ty))),
        ast::Ty::FnPtr {
            inputs,
            output,
            abi,
            is_unsafe,
            ..
        } => HirTyKind::FnPtr {
            inputs: inputs.iter().map(|t| lower_ty(cx, t)).collect(),
            output: Box::new(lower_ty(cx, output)),
            abi: *abi,
            is_unsafe: *is_unsafe,
        },
        ast::Ty::Path(qself, path, _) => HirTyKind::Path(
            HirQSelf {
                ty: qself.ty.as_ref().map(|t| Box::new(lower_ty(cx, t))),
                position: qself.position,
            },
            path::lower_path(cx, path),
        ),
        ast::Ty::TraitObject {
            bounds, lifetime, ..
        } => HirTyKind::TraitObject {
            bounds: crate::hir::lower::generics::lower_type_bounds(cx, bounds),
            lifetime: lifetime.clone(),
        },
        ast::Ty::ImplTrait(bounds, _) => {
            HirTyKind::ImplTrait(crate::hir::lower::generics::lower_type_bounds(cx, bounds))
        }
        ast::Ty::Infer(_) => HirTyKind::Infer,
    };
    HirTy {
        hir_id,
        kind,
        inferred: None, // set by Stage 2 typeck
        span,
    }
}

fn ty_span(ty: &ast::Ty) -> Span {
    use ast::Ty::*;
    match ty {
        Bool(s) => *s,
        Char(s) => *s,
        Int(_, s) => *s,
        Uint(_, s) => *s,
        Float(_, s) => *s,
        Never(s) => *s,
        Tuple(_, s) => *s,
        Array(_, _, s) => *s,
        Slice(_, s) => *s,
        Ref(_, _, _, s) => *s,
        Ptr(_, _, s) => *s,
        FnPtr { span, .. } => *span,
        Path(_, _, s) => *s,
        TraitObject { span, .. } => *span,
        ImplTrait(_, s) => *s,
        Infer(s) => *s,
    }
}
