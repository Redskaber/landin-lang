//! MIR body: basic blocks, statements, terminators.
//!
//! Per 06-mir.md, a MIR body is a control flow graph (CFG) of basic
//! blocks. Each block contains a sequence of statements followed by
//! a terminator.

use crate::mir::lvalue::*;
use crate::mir::ty::*;
use crate::session::Span;
use std::collections::HashMap;

/// A MIR body: the CFG representation of a function body.
#[derive(Debug, Clone)]
pub struct MirBody {
    /// All basic blocks, indexed by BasicBlockId.
    pub basic_blocks: Vec<BasicBlock>,
    /// Local variable declarations (params + locals).
    pub local_decls: Vec<LocalDecl>,
    /// Span of the function body (for error reporting).
    pub span: Span,
}

impl MirBody {
    pub fn new(span: Span) -> Self {
        Self {
            basic_blocks: Vec::new(),
            local_decls: Vec::new(),
            span,
        }
    }

    /// Allocate a new basic block and return its ID.
    pub fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.basic_blocks.len() as u32);
        self.basic_blocks.push(BasicBlock {
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    /// Allocate a new local variable and return its ID.
    pub fn new_local(&mut self, ty: Ty, name: Option<crate::lexer::Symbol>, span: Span) -> LocalId {
        self.new_local_with_mut(ty, name, span, Mutability::Immutable)
    }

    /// Allocate a new local variable with explicit mutability.
    ///
    /// G5 fix (Stage 2.4e): Used by `let mut x = ...` lowering to mark
    /// the local as mutable. The borrow checker checks this field in
    /// `check_place_write` to reject writes to immutable locals.
    pub fn new_local_with_mut(
        &mut self,
        ty: Ty,
        name: Option<crate::lexer::Symbol>,
        span: Span,
        mutability: Mutability,
    ) -> LocalId {
        let id = LocalId(self.local_decls.len() as u32);
        self.local_decls.push(LocalDecl {
            ty,
            name,
            mutability,
            source_info: span,
        });
        id
    }

    /// Get a basic block by ID.
    pub fn block(&self, id: BasicBlockId) -> &BasicBlock {
        &self.basic_blocks[id.0 as usize]
    }

    /// Get a mutable basic block by ID.
    pub fn block_mut(&mut self, id: BasicBlockId) -> &mut BasicBlock {
        &mut self.basic_blocks[id.0 as usize]
    }

    /// Get a local declaration by ID.
    pub fn local(&self, id: LocalId) -> &LocalDecl {
        &self.local_decls[id.0 as usize]
    }
}

/// A basic block: a straight-line sequence of statements ending with
/// a terminator (control flow instruction).
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Statements executed in order.
    pub statements: Vec<Statement>,
    /// The terminator: how control leaves this block.
    pub terminator: Terminator,
}

/// A MIR statement: `Lvalue = Rvalue`.
#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    /// `place = rvalue`
    Assign(Box<(Lvalue, Rvalue)>),
    /// No-op (placeholder, for debugging).
    Nop,
    /// Mark a local as live — it's now safe to use.
    /// Emitted at the start of a local's scope (e.g., at the `let`).
    /// Codegen uses this to know when to allocate stack space.
    StorageLive(LocalId),
    /// Mark a local as dead — its storage can be reclaimed.
    /// Emitted at the end of a local's scope (or its last use, under NLL).
    /// Codegen uses this to know when to run destructors / free stack space.
    StorageDead(LocalId),
    /// Run the destructor for the value at `place`. Used for explicit
    /// `drop(x)` calls (not for scope-end cleanup, which uses StorageDead).
    /// Distinct from Terminator::Drop (which is for control-flow drops).
    Deinit(Lvalue),
}

/// A MIR terminator: the last instruction in a basic block.
/// Determines how control flows to the next block(s).
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Unconditional jump to `target`.
    Goto(BasicBlockId),
    /// `switchInt(discr) { val1 => bb1, val2 => bb2, _ => otherwise }`
    SwitchInt {
        discr: Operand,
        targets: Vec<(ConstVal, BasicBlockId)>,
        otherwise: BasicBlockId,
    },
    /// `return` from the function.
    Return,
    /// Unreachable code (e.g., after `return`, `break` in infinite loop).
    Unreachable,
    /// Drop a value (run its destructor).
    Drop {
        place: Lvalue,
        target: BasicBlockId,
        unwind: Option<BasicBlockId>,
    },
    /// Function call: `destination = func(args)`.
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Lvalue,
        target: Option<BasicBlockId>,
    },
    /// Assert a boolean condition (for overflow checks, Stage 3+).
    Assert {
        cond: Operand,
        expected: bool,
        target: BasicBlockId,
        msg: AssertMessage,
    },
}

/// Assert message for runtime checks.
///
/// Stage 3.24: `Overflow` now carries the original `lhs` and `rhs` operands
/// (per design doc `06-mir.md` §"AssertMessage"). Codegen uses these to emit
/// `llvm.{sadd,ssub,smul}.with.overflow.*` intrinsics and branch on the
/// extracted overflow flag.
///
/// Stage 3.25: `DivisionByZero` now carries the divisor operand so codegen
/// can emit `icmp eq divisor, 0` and branch to a panic block.
#[derive(Debug, Clone)]
pub enum AssertMessage {
    Overflow(BinOp, Operand, Operand),
    DivisionByZero(Operand),
    BoundsCheck,
}

/// ID of a basic block within a MIR body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BasicBlockId(pub u32);

/// A local variable declaration in MIR.
#[derive(Debug, Clone)]
pub struct LocalDecl {
    /// The type of this local.
    pub ty: Ty,
    /// The name of this local (if it came from a named binding).
    pub name: Option<crate::lexer::Symbol>,
    /// Whether this local is `mut`.
    pub mutability: Mutability,
    /// Source span for error reporting.
    pub source_info: Span,
}

/// Type for the `visible_names` map: name → local ID.
pub type VisibleNames = HashMap<crate::lexer::Symbol, LocalId>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    #[test]
    fn mir_body_new_block() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        assert_eq!(bb0, BasicBlockId(0));
        let bb1 = body.new_block();
        assert_eq!(bb1, BasicBlockId(1));
        assert_eq!(body.basic_blocks.len(), 2);
    }

    #[test]
    fn mir_body_new_local() {
        let mut body = MirBody::new(Span::DUMMY);
        let l0 = body.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        assert_eq!(l0, LocalId(0));
        let l1 = body.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        assert_eq!(l1, LocalId(1));
        assert_eq!(body.local_decls.len(), 2);
    }

    #[test]
    fn basic_block_assign_statement() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb = body.new_block();
        let local = body.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // Add: local = 42
        body.block_mut(bb).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(local, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        // Set terminator: return
        body.block_mut(bb).terminator = Terminator::Return;

        assert_eq!(body.block(bb).statements.len(), 1);
        assert!(matches!(body.block(bb).terminator, Terminator::Return));
    }

    #[test]
    fn terminator_goto() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        body.block_mut(bb0).terminator = Terminator::Goto(bb1);
        assert!(matches!(
            body.block(bb0).terminator,
            Terminator::Goto(BasicBlockId(1))
        ));
    }

    #[test]
    fn terminator_switch_int() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        let bb2 = body.new_block();
        let discr_local = body.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        body.block_mut(bb0).terminator = Terminator::SwitchInt {
            discr: Operand::Copy(Lvalue::local(discr_local, Span::DUMMY)),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        };
        match &body.block(bb0).terminator {
            Terminator::SwitchInt {
                targets, otherwise, ..
            } => {
                assert_eq!(targets.len(), 1);
                assert_eq!(*otherwise, bb2);
            }
            _ => panic!("expected SwitchInt"),
        }
    }
}
