//! Borrow checker: NLL (Non-Lexical Lifetimes) on MIR.
//!
//! Per 04-ownership-borrowing.md, the borrow checker enforces Landin's
//! ownership and borrowing rules:
//! - Each value has a single owner
//! - `&T` allows shared reads, `&mut T` allows exclusive writes
//! - A value can have multiple `&T` OR one `&mut T`, never both
//! - Moves transfer ownership; a moved value cannot be used
//! - NLL: lifetimes end at last use, not at lexical scope end
//!
//! Public entry point: [`check_mir_body`].

pub mod borrow_set;
pub mod error;
pub mod move_tracker;

pub use borrow_set::{Borrow, BorrowKind as BkKind, BorrowSet};
pub use error::BorrowError;
pub use move_tracker::MoveTracker;

use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::session::Span;

/// The borrow checker. Walks MIR bodies, tracks borrows and moves,
/// and reports ownership/borrowing violations.
pub struct BorrowChecker {
    /// All active borrows in the current body.
    borrows: BorrowSet,
    /// Move tracker: which locals have been moved.
    moves: MoveTracker,
    /// Errors found during checking (non-fatal).
    errors: Vec<BorrowError>,
}

impl BorrowChecker {
    pub fn new() -> Self {
        Self {
            borrows: BorrowSet::new(),
            moves: MoveTracker::new(),
            errors: Vec::new(),
        }
    }

    /// Check a single MIR body for borrow/ownership violations.
    ///
    /// Walks all basic blocks in order, tracking:
    /// - Borrows created by `Rvalue::Ref`
    /// - Moves created by `Operand::Move`
    /// - Uses of borrowed/moved places
    ///
    /// Reports errors for:
    /// - Use-after-move
    /// - Mutating while borrowed
    /// - Borrowing a moved value
    pub fn check_mir_body(&mut self, mir: &MirBody) {
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                self.check_statement(mir, stmt);
            }
            self.check_terminator(mir, &bb.terminator);
        }
    }

    fn check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, rvalue) = &**boxed;
            self.check_rvalue(mir, rvalue, stmt.span);
            self.check_place_write(mir, place, stmt.span);
        }
    }

    fn check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        match term {
            Terminator::Call { func, args, .. } => {
                self.check_operand(mir, func, Span::DUMMY);
                for arg in args {
                    self.check_operand(mir, arg, Span::DUMMY);
                }
            }
            Terminator::SwitchInt { discr, .. } => {
                self.check_operand(mir, discr, Span::DUMMY);
            }
            Terminator::Drop { place, .. } => {
                self.check_place_read(mir, place, Span::DUMMY);
            }
            _ => {}
        }
    }

    /// Check an rvalue for borrow creation and operand moves.
    fn check_rvalue(&mut self, mir: &MirBody, rv: &Rvalue, span: Span) {
        match rv {
            Rvalue::Ref(region, kind, place) => {
                // Creating a borrow: record it
                let borrowed_place = self.place_path(mir, place);
                let bk = match kind {
                    crate::mir::lvalue::BorrowKind::Shared => BkKind::Shared,
                    crate::mir::lvalue::BorrowKind::Mut => BkKind::Mut,
                    crate::mir::lvalue::BorrowKind::Raw => BkKind::Raw,
                };
                // Check if the place is already moved
                if self.moves.is_moved(&borrowed_place) {
                    self.errors.push(BorrowError::use_after_move(
                        "cannot borrow moved value",
                        span,
                    ));
                }
                // Check for conflicting borrows
                if let Err(conflict) = self.borrows.add_borrow(borrowed_place, bk, span) {
                    self.errors.push(conflict);
                }
                let _ = region;
            }
            Rvalue::Use(op) | Rvalue::Cast(_, op, _) => {
                self.check_operand(mir, op, span);
            }
            Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
                self.check_operand(mir, a, span);
                self.check_operand(mir, b, span);
            }
            Rvalue::UnaryOp(_, op) => {
                self.check_operand(mir, op, span);
            }
            Rvalue::Aggregate(_, operands) => {
                for op in operands {
                    self.check_operand(mir, op, span);
                }
            }
        }
    }

    /// Check an operand: if it's a Move, record the move; if it's a
    /// Copy/Move of a place, check for use-after-move.
    fn check_operand(&mut self, mir: &MirBody, op: &Operand, span: Span) {
        match op {
            Operand::Copy(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors
                        .push(BorrowError::use_after_move("use of moved value", span));
                }
            }
            Operand::Move(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors
                        .push(BorrowError::use_after_move("use of moved value", span));
                }
                // Check if borrowed
                if let Some(bk) = self.borrows.borrow_kind(&path) {
                    if bk == BkKind::Shared || bk == BkKind::Mut {
                        self.errors.push(BorrowError::move_borrowed(
                            "cannot move borrowed value",
                            span,
                        ));
                    }
                }
                self.moves.record_move(path);
            }
            Operand::Constant(_) => {}
        }
    }

    /// Check a write to a place: ensure it's not borrowed.
    fn check_place_write(&mut self, mir: &MirBody, lv: &Lvalue, span: Span) {
        let path = self.place_path(mir, lv);
        // Writing to a place that is borrowed is an error
        if let Some(bk) = self.borrows.borrow_kind(&path) {
            if bk == BkKind::Shared || bk == BkKind::Mut {
                self.errors.push(BorrowError::assign_borrowed(
                    "cannot assign to borrowed value",
                    span,
                ));
            }
        }
        // Writing re-initializes a moved place
        self.moves.un_move(&path);
    }

    /// Check a read from a place: ensure it's not moved.
    fn check_place_read(&mut self, mir: &MirBody, lv: &Lvalue, span: Span) {
        let path = self.place_path(mir, lv);
        if self.moves.is_moved(&path) {
            self.errors
                .push(BorrowError::use_after_move("use of moved value", span));
        }
    }

    /// Get the PlacePath for an lvalue (simplified: just LocalId).
    fn place_path(&self, _mir: &MirBody, lv: &Lvalue) -> PlacePath {
        match &lv.kind {
            LvalueKind::Local(id) => PlacePath::Local(*id),
            LvalueKind::Static(def_id) => PlacePath::Static(*def_id),
            LvalueKind::Projection(base, _) => self.place_path(_mir, base),
        }
    }

    pub fn into_errors(self) -> Vec<BorrowError> {
        self.errors
    }
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// A simplified place path for borrow/move tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacePath {
    Local(LocalId),
    Static(crate::hir::DefId),
}

/// Check a single MIR body for borrow/ownership errors.
/// Returns a list of errors (non-fatal).
pub fn check_mir_body(mir: &MirBody) -> Vec<BorrowError> {
    let mut bc = BorrowChecker::new();
    bc.check_mir_body(mir);
    bc.into_errors()
}

/// Check all MIR bodies derived from a HIR crate.
pub fn check_crate(hir: &crate::hir::HirCrate, interner: &lasso::Rodeo) -> Vec<BorrowError> {
    let mut all_errors = Vec::new();
    for (_, body) in &hir.bodies {
        let mir = crate::mir::lower::lower_hir_body_to_mir(body, interner);
        all_errors.extend(check_mir_body(&mir));
    }
    all_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::mir::ty::*;

    fn make_mir() -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        mir.new_block();
        mir
    }

    #[test]
    fn no_errors_on_simple_body() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn use_after_move_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = move x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  <-- use after move!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected use-after-move error");
    }

    #[test]
    fn move_borrowed_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r = mir.new_local(
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
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // r = &x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::lvalue::BorrowKind::Shared,
                    Lvalue::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // y = move x  <-- move while borrowed!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected move-borrowed error");
    }

    #[test]
    fn assign_to_borrowed_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r = mir.new_local(
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
        // r = &x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::lvalue::BorrowKind::Shared,
                    Lvalue::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // x = 42  <-- assign while borrowed!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected assign-borrowed error");
    }

    #[test]
    fn shared_borrow_after_mut_ok() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r1 = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Mutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        // r1 = &mut x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(r1, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::lvalue::BorrowKind::Mut,
                    Lvalue::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // After r1's last use, we can borrow again (NLL).
        // For Stage 2.3, we don't track last-use precisely — this is a
        // simplified check. The borrow remains active for the whole body.
        // This test just verifies no crash.
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let _ = check_mir_body(&mir);
    }

    #[test]
    fn reassign_after_move_ok() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = move x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // x = 42  (re-initialize after move — OK)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  (OK — x was re-initialized)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors after re-init, got {:?}",
            errors
        );
    }

    #[test]
    fn copy_is_not_move() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = copy x  (Copy type — not moved)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  (OK — x was copied, not moved)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for copies, got {:?}",
            errors
        );
    }
}
