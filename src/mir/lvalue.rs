//! MIR lvalues, rvalues, and operands.
//!
//! Per 06-mir.md, MIR represents computation as assignments
//! (`Lvalue = Rvalue`) within basic blocks. Lvalues are "places"
//! (addressable locations), Rvalues are computed values.

use crate::mir::ty::*;
use crate::session::Span;

// Re-export Const and ConstVal from ty.rs for convenience.
pub use crate::mir::ty::ConstVal;

/// An lvalue (place expression): a memory location that can be read
/// from or written to.
///
/// Examples: `x`, `*p`, `a.field`, `arr[i]`
#[derive(Debug, Clone)]
pub struct Lvalue {
    pub kind: LvalueKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LvalueKind {
    /// A local variable (fn param or let binding).
    Local(LocalId),
    /// A static item.
    Static(crate::hir::DefId),
    /// A projection: `base.field`, `*base`, `base[idx]`, etc.
    Projection(Box<Lvalue>, ProjectionElem),
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

impl Lvalue {
    pub fn local(id: LocalId, span: Span) -> Self {
        Self {
            kind: LvalueKind::Local(id),
            span,
        }
    }
}

/// An rvalue: a computed value that is assigned to an lvalue.
///
/// `Lvalue = Rvalue` is the fundamental MIR statement.
#[derive(Debug, Clone)]
pub enum Rvalue {
    /// `x = use(operand)` — just copy/move the operand
    Use(Operand),
    /// `x = a + b`
    BinaryOp(BinOp, Operand, Operand),
    /// `x = -a` / `x = !a`
    UnaryOp(UnOp, Operand),
    /// `x = &mut? place`
    Ref(Region, BorrowKind, Lvalue),
    /// `x = operand as Ty`
    Cast(CastKind, Operand, Ty),
    /// `x = (a, b, c)` / `x = [a, b, c]` / `x = Foo { .. }`
    Aggregate(AggregateKind, Vec<Operand>),
    /// `x = a .. b`
    BinaryOp2(RangeOp, Operand, Operand),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Adt(crate::hir::DefId, u32 /* variant index */, SubstsRef),
    /// `Foo(a, b)` — closure
    Closure(crate::hir::DefId, SubstsRef),
}

/// An operand: the right-hand side of a binary op or the argument
/// to a function call.
#[derive(Debug, Clone)]
pub enum Operand {
    /// `Copy(place)` — copy the value at `place`
    Copy(Lvalue),
    /// `Move(place)` — move the value at `place`
    Move(Lvalue),
    /// A compile-time constant
    Constant(Const),
}

// Note: Const and ConstVal are defined in ty.rs and re-exported from mod.rs.
// The Operand::Constant variant uses ty::Const.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    #[test]
    fn lvalue_local() {
        let lv = Lvalue::local(LocalId(0), Span::DUMMY);
        assert!(matches!(lv.kind, LvalueKind::Local(LocalId(0))));
    }

    #[test]
    fn lvalue_projection() {
        let base = Lvalue::local(LocalId(0), Span::DUMMY);
        let lv = Lvalue {
            kind: LvalueKind::Projection(
                Box::new(base),
                ProjectionElem::Field(FieldId(0), Ty::new(TyKind::Bool, Span::DUMMY)),
            ),
            span: Span::DUMMY,
        };
        match lv.kind {
            LvalueKind::Projection(base, ProjectionElem::Field(FieldId(0), _)) => {
                assert!(matches!(base.kind, LvalueKind::Local(LocalId(0))));
            }
            _ => panic!("expected Projection"),
        }
    }

    #[test]
    fn rvalue_use() {
        let op = Operand::Copy(Lvalue::local(LocalId(0), Span::DUMMY));
        let rv = Rvalue::Use(op);
        assert!(matches!(rv, Rvalue::Use(Operand::Copy(_))));
    }

    #[test]
    fn rvalue_binary_op() {
        let lhs = Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
            val: ConstVal::Int(1),
        });
        let rhs = Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
            val: ConstVal::Int(2),
        });
        let rv = Rvalue::BinaryOp(BinOp::Add, lhs, rhs);
        assert!(matches!(rv, Rvalue::BinaryOp(BinOp::Add, _, _)));
    }

    #[test]
    fn rvalue_ref() {
        let lv = Lvalue::local(LocalId(0), Span::DUMMY);
        let rv = Rvalue::Ref(Region::Static, BorrowKind::Shared, lv);
        assert!(matches!(
            rv,
            Rvalue::Ref(Region::Static, BorrowKind::Shared, _)
        ));
    }
}
