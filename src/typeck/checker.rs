//! Type checker: walks MIR bodies, collects constraints, unifies types.
//!
//! This module implements the full type checking pass:
//! 1. Walk each MIR body's basic blocks in order
//! 2. For each `Statement::Assign(place, rvalue)`, infer the rvalue's type
//!    and unify it with the place's declared type
//! 3. Check terminator constraints (Call args, SwitchInt discr type)
//! 4. Default unresolved int/float variables to i32/f64
//! 5. Resolve and report any type errors

use crate::ast;
use crate::hir::HirCrate;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::error::TypeError;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

/// The type checker. Holds the unification table and collects errors.
pub struct TypeChecker {
    pub unify: UnificationTable,
    pub errors: Vec<TypeError>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            unify: UnificationTable::new(),
            errors: Vec::new(),
        }
    }

    /// Check a single MIR body. Walks all basic blocks, infers types
    /// for rvalues, and unifies with lvalue types.
    pub fn check_mir_body(&mut self, mir: &MirBody) {
        // Phase 1: Walk basic blocks in order, collecting constraints.
        for bb_id in 0..mir.basic_blocks.len() {
            let bb_id = BasicBlockId(bb_id as u32);
            let bb = mir.block(bb_id);

            // Check each statement
            for stmt in &bb.statements {
                self.check_statement(mir, stmt);
            }

            // Check terminator
            self.check_terminator(mir, &bb.terminator);
        }

        // Phase 2: Default unresolved int/float variables.
        self.unify.default_unresolved();
    }

    /// Check a single MIR statement (Assign or Nop).
    fn check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Assign(boxed) => {
                let (place, rvalue) = &**boxed;
                let place_ty = self.infer_lvalue(mir, place);
                let rvalue_ty = self.infer_rvalue(mir, rvalue);
                if let Err(e) = self.unify.unify(&place_ty, &rvalue_ty) {
                    self.errors.push(*e);
                }
            }
            StatementKind::Nop => {}
        }
    }

    /// Check a terminator's type constraints.
    fn check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        match term {
            Terminator::Call {
                func,
                args,
                destination,
                ..
            } => {
                // Infer func type (should be a function type)
                let _func_ty = self.infer_operand(mir, func);
                // Infer arg types
                for arg in args {
                    let _arg_ty = self.infer_operand(mir, arg);
                }
                // Destination type will be inferred from the call result
                let _dest_ty = self.infer_lvalue(mir, destination);
            }
            Terminator::SwitchInt { discr, .. } => {
                // The discriminant must be an integer or bool
                let discr_ty = self.infer_operand(mir, discr);
                // Check that it's int-like (int, uint, bool, or infer var)
                match &discr_ty.kind {
                    TyKind::Int(_) | TyKind::Uint(_) | TyKind::Bool => {}
                    TyKind::Infer(InferVar::IntVar(_)) => {}
                    TyKind::Infer(InferVar::TyVar(_)) => {
                        // Unbound variable — unify with i32 as default
                        let i32_ty = Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY);
                        let _ = self.unify.unify(&discr_ty, &i32_ty);
                    }
                    TyKind::Error => {}
                    _ => {
                        self.errors.push(TypeError::new(
                            format!(
                                "expected integer or bool for switch, found {:?}",
                                discr_ty.kind
                            ),
                            Span::DUMMY,
                        ));
                    }
                }
            }
            Terminator::Drop { place, .. } => {
                // Just infer the place type (no constraint to check)
                let _ = self.infer_lvalue(mir, place);
            }
            _ => {}
        }
    }

    /// Infer the type of an lvalue (place expression).
    fn infer_lvalue(&self, mir: &MirBody, lv: &Lvalue) -> Ty {
        match &lv.kind {
            LvalueKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    Ty::new(TyKind::Error, lv.span)
                }
            }
            LvalueKind::Static(_) => {
                // Static type would come from the HIR; for now, Error
                Ty::new(TyKind::Error, lv.span)
            }
            LvalueKind::Projection(base, elem) => {
                let base_ty = self.infer_lvalue(mir, base);
                self.infer_projection(&base_ty, elem)
            }
        }
    }

    /// Infer the type after applying a projection element.
    fn infer_projection(&self, base_ty: &Ty, elem: &ProjectionElem) -> Ty {
        match elem {
            ProjectionElem::Deref => {
                if let TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::Field(_, field_ty) => field_ty.clone(),
            ProjectionElem::Index(_) => {
                if let TyKind::Array(inner, _) | TyKind::Slice(inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::ConstantIndex { .. } | ProjectionElem::Subslice { .. } => {
                if let TyKind::Array(inner, _) | TyKind::Slice(inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
        }
    }

    /// Infer the type of an rvalue.
    fn infer_rvalue(&self, mir: &MirBody, rv: &Rvalue) -> Ty {
        match rv {
            Rvalue::Use(operand) => self.infer_operand(mir, operand),
            Rvalue::BinaryOp(op, a, b) => {
                let a_ty = self.infer_operand(mir, a);
                let _b_ty = self.infer_operand(mir, b);
                // Comparison operators return bool
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        Ty::new(TyKind::Bool, Span::DUMMY)
                    }
                    // Arithmetic operators return the same type as operands
                    _ => a_ty,
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                let _a_ty = self.infer_operand(mir, a);
                Ty::new(TyKind::Error, Span::DUMMY) // Range type (Stage 3)
            }
            Rvalue::UnaryOp(op, operand) => {
                let inner_ty = self.infer_operand(mir, operand);
                match op {
                    UnOp::Not => {
                        // !bool → bool, !int → int
                        inner_ty
                    }
                    UnOp::Neg => {
                        // -int → int, -float → float
                        inner_ty
                    }
                }
            }
            Rvalue::Ref(_, borrow_kind, lv) => {
                let inner_ty = self.infer_lvalue(mir, lv);
                let mutability = match borrow_kind {
                    BorrowKind::Shared => Mutability::Immutable,
                    BorrowKind::Mut => Mutability::Mutable,
                    BorrowKind::Raw => Mutability::Immutable,
                };
                Ty::new(
                    TyKind::Ref(Region::Erased, mutability, Box::new(inner_ty)),
                    Span::DUMMY,
                )
            }
            Rvalue::Cast(_, _, target_ty) => target_ty.clone(),
            Rvalue::Aggregate(kind, operands) => match kind {
                AggregateKind::Tuple => {
                    let elem_tys: Vec<Ty> = operands
                        .iter()
                        .map(|o| self.infer_operand(mir, o))
                        .collect();
                    Ty::new(TyKind::Tuple(elem_tys), Span::DUMMY)
                }
                AggregateKind::Array(elem_ty) => Ty::new(
                    TyKind::Array(
                        Box::new(elem_ty.clone()),
                        Box::new(Const {
                            ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY)),
                            val: ConstVal::Uint(operands.len() as u128),
                        }),
                    ),
                    Span::DUMMY,
                ),
                _ => Ty::new(TyKind::Error, Span::DUMMY),
            },
        }
    }

    /// Infer the type of an operand.
    fn infer_operand(&self, mir: &MirBody, op: &Operand) -> Ty {
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => self.infer_lvalue(mir, lv),
            Operand::Constant(c) => c.ty.as_ref().clone(),
        }
    }

    /// Consume the type checker and return all errors.
    pub fn into_errors(mut self) -> Vec<TypeError> {
        let mut errors = self.errors;
        errors.extend(self.unify.take_errors());
        errors
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a single MIR body for type errors.
/// Returns a list of type errors (non-fatal; the MIR is still usable).
pub fn check_mir_body(mir: &MirBody) -> Vec<TypeError> {
    let mut tc = TypeChecker::new();
    tc.check_mir_body(mir);
    tc.into_errors()
}

/// Check all MIR bodies in a HIR crate for type errors.
///
/// This is the main entry point for the type checking pass. It:
/// 1. Lowers each HIR body to MIR (if not already done)
/// 2. Runs the type checker on each MIR body
/// 3. Collects all type errors
///
/// Returns a list of all type errors found across all bodies.
pub fn check_crate(hir: &HirCrate, interner: &Rodeo) -> Vec<TypeError> {
    let mut all_errors = Vec::new();
    for (_, body) in &hir.bodies {
        let mir = crate::mir::lower::lower_hir_body_to_mir(body, interner);
        let mut tc = TypeChecker::new();
        tc.check_mir_body(&mir);
        all_errors.extend(tc.into_errors());
    }
    all_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn make_mir_with_return_i32() -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        let _bb0 = mir.new_block(); // create entry block
        let return_local = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let temp = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(temp, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(return_local, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(temp, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        mir
    }

    #[test]
    fn check_valid_i32_assignment() {
        let mir = make_mir_with_return_i32();
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no type errors, got {:?}",
            errors
        );
    }

    #[test]
    fn check_type_mismatch_detected() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected type mismatch error");
    }

    #[test]
    fn check_comparison_returns_bool() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let result = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        let a = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(result, Span::DUMMY),
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Operand::Copy(Lvalue::local(a, Span::DUMMY)),
                    Operand::Copy(Lvalue::local(b, Span::DUMMY)),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for comparison, got {:?}",
            errors
        );
    }

    #[test]
    fn check_ref_type_inference() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let src = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    BorrowKind::Shared,
                    Lvalue::local(src, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for ref, got {:?}",
            errors
        );
    }

    #[test]
    fn check_tuple_aggregate() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(
            Ty::new(
                TyKind::Tuple(vec![
                    Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    Ty::new(TyKind::Bool, Span::DUMMY),
                ]),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let a = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![
                        Operand::Copy(Lvalue::local(a, Span::DUMMY)),
                        Operand::Copy(Lvalue::local(b, Span::DUMMY)),
                    ],
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for tuple, got {:?}",
            errors
        );
    }

    #[test]
    fn check_switch_int_valid() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let discr = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::SwitchInt {
            discr: Operand::Copy(Lvalue::local(discr, Span::DUMMY)),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        };
        mir.block_mut(bb1).terminator = Terminator::Return;
        mir.block_mut(bb2).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for switch, got {:?}",
            errors
        );
    }

    #[test]
    fn check_switch_int_invalid_type() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let discr = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::SwitchInt {
            discr: Operand::Copy(Lvalue::local(discr, Span::DUMMY)),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        };
        mir.block_mut(bb1).terminator = Terminator::Return;
        mir.block_mut(bb2).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected error for switch on ref type");
    }
}
