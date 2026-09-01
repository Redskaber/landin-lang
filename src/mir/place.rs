//! MIR places, rvalues, and operands.
//!
//! Per 06-mir.md, MIR represents computation as assignments
//! (`Place = Rvalue`) within basic blocks. Places are addressable
//! locations (formerly called "lvalues"), Rvalues are computed values.
//!
//! Stage 3.66: this module was renamed from `lvalue` to `place` to align
//! with the design doc (06-mir.md §4) and the borrowck internal vocabulary
//! (`PlacePath`, `PlaceRoot`). The type `Lvalue` → `Place`, `LvalueKind` →
//! `PlaceKind`.

use crate::mir::ty::*;
use crate::session::Span;

// Re-export Const and ConstVal from ty.rs for convenience.
pub use crate::mir::ty::ConstVal;

/// A place (memory location that can be read from or written to).
///
/// Formerly called `Lvalue` (legacy rustc name from pre-RFC-1211 era).
/// Renamed to `Place` in Stage 3.66 to align with the design doc
/// (06-mir.md §4) and the borrowck internal vocabulary.
///
/// Examples: `x`, `*p`, `a.field`, `arr[i]`
#[derive(Debug, Clone)]
pub struct Place {
    pub kind: PlaceKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PlaceKind {
    /// A local variable (fn param or let binding).
    Local(LocalId),
    /// A static item.
    Static(crate::hir::DefId),
    /// A projection: `base.field`, `*base`, `base[idx]`, etc.
    Projection(Box<Place>, ProjectionElem),
}

/// Element of a projection.
#[derive(Debug, Clone)]
pub enum ProjectionElem {
    /// `*base` — dereference
    Deref,
    /// `base.0`, `base.1` — tuple field access by index
    Field(FieldId, Ty),
    /// `base[idx]` — index by a local variable
    Index(LocalId),
    /// `base[N]` — constant index
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    /// `base[..]` — subslice
    Subslice { from: u64, to: u64, from_end: bool },
}

/// Identifier for a local variable within a MIR body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

/// Identifier for a field within a struct/enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId(pub u32);

impl Place {
    pub fn local(id: LocalId, span: Span) -> Self {
        Self {
            kind: PlaceKind::Local(id),
            span,
        }
    }
}

/// An rvalue: a computed value that is assigned to a place.
///
/// `Place = Rvalue` is the fundamental MIR statement.
#[derive(Debug, Clone)]
pub enum Rvalue {
    /// `x = use(operand)` — just copy/move the operand
    Use(Operand),
    /// `x = a + b`
    BinaryOp(BinOp, Operand, Operand),
    /// `x = -a` / `x = !a`
    UnaryOp(UnOp, Operand),
    /// `x = &mut? place`
    Ref(Region, BorrowKind, Place),
    /// `x = operand as Ty`
    Cast(CastKind, Operand, Ty),
    /// `x = (a, b, c)` / `x = [a, b, c]` / `x = Foo { .. }`
    Aggregate(AggregateKind, Vec<Operand>),
    /// `x = a .. b`
    BinaryOp2(RangeOp, Operand, Operand),
    /// Stage 18.226 (v0.2 Phase 2): Load value from raw pointer.
    /// `x = *ptr` → load the value pointed to by `ptr`.
    /// Per §1.0 原則 6 (通解>特例): one Load for all pointer types.
    /// Per §16.2 (06-mir.md): MIR intrinsic ops design.
    Load(Operand /* ptr */, Ty /* pointee type */),
    /// Stage 18.226 (v0.2 Phase 2): Get element pointer (GEP).
    /// `x = &base[offset]` → compute address of element at offset.
    /// Per §1.0 原則 6 (通解>特例): one GEP for all indexing.
    /// Per §16.2 (06-mir.md): MIR intrinsic ops design.
    GetElementPtr {
        /// Pointer to the base (struct, array, or slice).
        base: Operand,
        /// Indices for the GEP (field index, array index, etc.).
        indices: Vec<Operand>,
        /// Result pointer type (for codegen).
        result_ty: Ty,
    },
    /// Stage 33.1 (TD-VEC-PUSH-GET-MIGRATION): SizeOf — compute byte size
    /// of a type at codegen time (AFTER monomorphization substitution).
    /// `x = sizeof T` → evaluates to usize constant of T's byte size.
    ///
    /// Was: `sizeof T` was folded to a constant at MIR lower time, which
    /// returned the fallback value (8) for Param types. After monomorphization
    /// substituted Param(0) → i64, the constant was still 8 (not recomputed).
    /// This caused wrong elem_size for non-i32 types in Vec::push (i64, i8, struct).
    ///
    /// Fix: keep `sizeof T` as a deferred Rvalue::SizeOf(Ty) that codegen
    /// evaluates AFTER substitute_mir_body has replaced Param(0) with the
    /// concrete type. Codegen calls compute_type_size_with_fallback on the
    /// substituted type.
    ///
    /// Per §1.0 原則 6 (通解 > 特解): one SizeOf for all types.
    /// Per §1.0 原則 3 (显式 > 隐式): size is explicitly deferred.
    /// Per §12 (最优 > 最小): root-cause fix — defer evaluation to codegen.
    SizeOf(Ty /* type to compute size of */),
}

/// Binary operator in MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
}

/// Unary operator in MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

/// Range operator (for `..` / `..=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeOp {
    Range,          // ..
    RangeInclusive, // ..=
}

/// Kind of borrow.
///
/// Stage 3.63 (cross-stage naming standardization): This is the single
/// source of truth for `BorrowKind` across the codebase. The former
/// duplicate in `borrowck::borrow_set` has been removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowKind {
    /// `&T`
    Shared,
    /// `&mut T`
    Mut,
    /// `&raw const T` (raw pointer, Stage 3+)
    Raw,
}

/// Kind of cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    /// Numeric cast: `i32 as u64`
    Numeric,
    /// Pointer cast: `*const T as *mut T`
    Pointer,
    /// Unsize: `&[T; N] as &[T]`
    Unsize,
}

/// Kind of aggregate (collection construction).
#[derive(Debug, Clone)]
pub enum AggregateKind {
    /// `(a, b, c)` — tuple
    Tuple,
    /// `[a, b, c]` — array
    Array(Ty),
    /// `Foo { x: 1, y: 2 }` — struct/enum variant
    ///
    /// Stage 3.30 (per §16 阶段间接口隔离): the 4th field `field_tys`
    /// carries the LLVM-relevant field types of the variant. This is a
    /// "data sink" — MIR lower (Stage 2.1) computes the field types from
    /// HIR and stores them here so codegen (Stage 3) doesn't have to
    /// re-query HIR or call `lower_hir_ty_to_mir_ty` (which would be a
    /// cross-stage internal-API call, violating §16).
    Adt(
        crate::hir::DefId,
        u32, /* variant index */
        SubstsRef,
        Vec<Ty>, /* field types of this variant */
    ),
    /// `Foo(a, b)` — closure
    Closure(crate::hir::DefId, SubstsRef),
}

/// An operand: the right-hand side of a binary op or the argument
/// to a function call.
#[derive(Debug, Clone)]
pub enum Operand {
    /// `Copy(place)` — copy the value at `place`
    Copy(Place),
    /// `Move(place)` — move the value at `place`
    Move(Place),
    /// A compile-time constant
    Constant(Const),
}

// Note: Const and ConstVal are defined in ty.rs and re-exported from mod.rs.
// The Operand::Constant variant uses ty::Const.

/// Stage 15.86: Extract the source span from an `Operand`.
///
/// Shared helper used by both `typeck::checker::TypeChecker` and
/// `borrowck::mod::BorrowChecker` to attach accurate spans to errors
/// that originate from operand checks (e.g., type mismatches,
/// use-after-move, not-Copy).
///
/// - `Operand::Copy(place)` / `Operand::Move(place)` → `place.span`
/// - `Operand::Constant(const)` → `Span::DUMMY` (Const has no span field)
///
/// Per §1.0 原則 3 "显式 > 隐式": error spans are explicitly sourced
/// from the operand's Place, not defaulted to Span::DUMMY.
/// Per §23 rule 5 (DRY): single source of truth for operand span
/// extraction. Previously duplicated as private methods on
/// `TypeChecker` (Stage 15.81) and `BorrowChecker` (Stage 15.85) —
/// now unified here.
pub fn operand_span(op: &Operand) -> Span {
    match op {
        Operand::Copy(lv) | Operand::Move(lv) => lv.span,
        Operand::Constant(_) => Span::DUMMY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    #[test]
    fn place_local() {
        let lv = Place::local(LocalId(0), Span::DUMMY);
        assert!(matches!(lv.kind, PlaceKind::Local(LocalId(0))));
    }

    #[test]
    fn place_projection() {
        let base = Place::local(LocalId(0), Span::DUMMY);
        let lv = Place {
            kind: PlaceKind::Projection(
                Box::new(base),
                ProjectionElem::Field(FieldId(0), Ty::new(TyKind::Bool, Span::DUMMY)),
            ),
            span: Span::DUMMY,
        };
        match lv.kind {
            PlaceKind::Projection(base, ProjectionElem::Field(FieldId(0), _)) => {
                assert!(matches!(base.kind, PlaceKind::Local(LocalId(0))));
            }
            _ => panic!("expected Projection"),
        }
    }

    #[test]
    fn rvalue_use() {
        let op = Operand::Copy(Place::local(LocalId(0), Span::DUMMY));
        let rv = Rvalue::Use(op);
        assert!(matches!(rv, Rvalue::Use(Operand::Copy(_))));
    }

    #[test]
    fn rvalue_binary_op() {
        let lhs = Operand::Constant(Const {
            ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            val: ConstVal::Int(1),
        });
        let rhs = Operand::Constant(Const {
            ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            val: ConstVal::Int(2),
        });
        let rv = Rvalue::BinaryOp(BinOp::Add, lhs, rhs);
        assert!(matches!(rv, Rvalue::BinaryOp(BinOp::Add, _, _)));
    }

    #[test]
    fn rvalue_ref() {
        let lv = Place::local(LocalId(0), Span::DUMMY);
        let rv = Rvalue::Ref(Region::Static, BorrowKind::Shared, lv);
        assert!(matches!(
            rv,
            Rvalue::Ref(Region::Static, BorrowKind::Shared, _)
        ));
    }

    /// Stage 15.86: Verify the shared `operand_span` helper extracts the
    /// Place span from Copy/Move operands and returns DUMMY for Constant.
    #[test]
    fn stage15_86_operand_span_extracts_place_span() {
        let span = Span::new(42, 45);
        let place = Place::local(LocalId(0), span);
        // Copy operand → returns the place's span.
        let copy_op = Operand::Copy(place.clone());
        assert_eq!(operand_span(&copy_op), span);
        // Move operand → returns the place's span.
        let move_op = Operand::Move(place);
        assert_eq!(operand_span(&move_op), span);
        // Constant operand → returns Span::DUMMY (Const has no span field).
        let const_op = Operand::Constant(Const {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            val: ConstVal::Int(42),
        });
        assert_eq!(operand_span(&const_op), Span::DUMMY);
    }
}
