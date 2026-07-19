//! HIR → MIR lowering.
//!
//! Converts each HIR body (expression tree) into a MIR body (control
//! flow graph of basic blocks + statements + terminators).
//!
//! Public entry point: [`lower_hir_body_to_mir`].

use crate::ast;
use crate::hir::*;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::*;
use crate::session::Span;
use lasso::Rodeo;

/// Lowering context for HIR→MIR conversion.
///
/// Holds the MIR body being built, a local-variable map (HIR HirId →
/// MIR LocalId), and the current basic block being filled.
pub struct MirLowerCtxt<'a> {
    pub interner: &'a Rodeo,
    pub mir: MirBody,
    /// Map from HirId → LocalId for variables that have been assigned
    /// a MIR local.
    pub local_map: std::collections::HashMap<HirId, LocalId>,
    /// The current basic block being filled with statements.
    pub current_block: BasicBlockId,
}

impl<'a> MirLowerCtxt<'a> {
    pub fn new(interner: &'a Rodeo, span: Span) -> Self {
        let mut mir = MirBody::new(span);
        let current_block = mir.new_block();
        Self {
            interner,
            mir,
            local_map: std::collections::HashMap::new(),
            current_block,
        }
    }

    /// Allocate a new local variable for a HirId.
    pub fn new_local(
        &mut self,
        hir_id: HirId,
        ty: Ty,
        name: Option<crate::lexer::Symbol>,
    ) -> LocalId {
        let local_id = self.mir.new_local(ty, name, Span::DUMMY);
        self.local_map.insert(hir_id, local_id);
        local_id
    }

    /// Look up the MIR LocalId for a HirId.
    pub fn local_of(&self, hir_id: HirId) -> Option<LocalId> {
        self.local_map.get(&hir_id).copied()
    }

    /// Allocate a fresh basic block and return its ID.
    pub fn new_block(&mut self) -> BasicBlockId {
        self.mir.new_block()
    }

    /// Set the terminator of the current block and switch to a new block.
    pub fn terminate(&mut self, terminator: Terminator) {
        self.mir.block_mut(self.current_block).terminator = terminator;
    }

    /// Set the terminator of the current block and switch to `next`.
    pub fn terminate_and_goto(&mut self, terminator: Terminator, next: BasicBlockId) {
        self.mir.block_mut(self.current_block).terminator = terminator;
        self.current_block = next;
    }

    /// Push a statement onto the current block.
    pub fn push_assign(&mut self, place: Lvalue, rvalue: Rvalue, span: Span) {
        self.mir
            .block_mut(self.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((place, rvalue))),
                span,
            });
    }

    /// Push a Nop statement (for debugging / placeholders).
    pub fn push_nop(&mut self, span: Span) {
        self.mir
            .block_mut(self.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::Nop,
                span,
            });
    }

    /// Allocate a temporary local and assign the given rvalue to it.
    pub fn eval_rvalue_to_temp(&mut self, rvalue: Rvalue, ty: Ty, span: Span) -> LocalId {
        let temp = self.mir.new_local(ty, None, span);
        self.push_assign(Lvalue::local(temp, span), rvalue, span);
        temp
    }

    /// Convert a HIR LitKind to a MIR Const.
    pub fn lit_to_const(&self, lit: &HirLitKind) -> (Const, Ty) {
        match lit {
            HirLitKind::Bool(b) => (
                Const {
                    ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
                    val: ConstVal::Bool(*b),
                },
                Ty::new(TyKind::Bool, Span::DUMMY),
            ),
            HirLitKind::Int(n, suffix) => {
                let ty_kind = match suffix {
                    Some(ast::IntTy::I32) => TyKind::Int(ast::IntTy::I32),
                    Some(ast::IntTy::I64) => TyKind::Int(ast::IntTy::I64),
                    _ => TyKind::Int(ast::IntTy::I32), // default
                };
                let ty = Ty::new(ty_kind.clone(), Span::DUMMY);
                (
                    Const {
                        ty: Box::new(ty.clone()),
                        val: ConstVal::Int(*n),
                    },
                    ty,
                )
            }
            HirLitKind::Uint(n, suffix) => {
                let ty_kind = match suffix {
                    Some(ast::UintTy::U32) => TyKind::Uint(ast::UintTy::U32),
                    Some(ast::UintTy::U64) => TyKind::Uint(ast::UintTy::U64),
                    _ => TyKind::Uint(ast::UintTy::U32),
                };
                let ty = Ty::new(ty_kind.clone(), Span::DUMMY);
                (
                    Const {
                        ty: Box::new(ty.clone()),
                        val: ConstVal::Uint(*n),
                    },
                    ty,
                )
            }
            HirLitKind::Float(f, suffix) => {
                let ty_kind = match suffix {
                    Some(ast::FloatTy::F64) => TyKind::Float(ast::FloatTy::F64),
                    _ => TyKind::Float(ast::FloatTy::F32),
                };
                let ty = Ty::new(ty_kind.clone(), Span::DUMMY);
                (
                    Const {
                        ty: Box::new(ty.clone()),
                        val: ConstVal::Float(*f),
                    },
                    ty,
                )
            }
            HirLitKind::Char(c) => (
                Const {
                    ty: Box::new(Ty::new(TyKind::Char, Span::DUMMY)),
                    val: ConstVal::Char(*c),
                },
                Ty::new(TyKind::Char, Span::DUMMY),
            ),
            _ => {
                // Default to i32 for unknown literals
                let ty = Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY);
                (
                    Const {
                        ty: Box::new(ty.clone()),
                        val: ConstVal::Int(0),
                    },
                    ty,
                )
            }
        }
    }

    /// Convert a HIR BinOp to a MIR BinOp.
    pub fn lower_bin_op(op: HirBinOp) -> BinOp {
        match op {
            HirBinOp::Add => BinOp::Add,
            HirBinOp::Sub => BinOp::Sub,
            HirBinOp::Mul => BinOp::Mul,
            HirBinOp::Div => BinOp::Div,
            HirBinOp::Rem => BinOp::Rem,
            HirBinOp::BitAnd => BinOp::BitAnd,
            HirBinOp::BitOr => BinOp::BitOr,
            HirBinOp::BitXor => BinOp::BitXor,
            HirBinOp::Shl => BinOp::Shl,
            HirBinOp::Shr => BinOp::Shr,
            HirBinOp::Eq => BinOp::Eq,
            HirBinOp::Ne => BinOp::Ne,
            HirBinOp::Lt => BinOp::Lt,
            HirBinOp::Le => BinOp::Le,
            HirBinOp::Gt => BinOp::Gt,
            HirBinOp::Ge => BinOp::Ge,
            // Logical and/or are lowered to control flow (short-circuit),
            // but for Stage 2.1b we treat them as bitwise for simplicity.
            HirBinOp::And => BinOp::BitAnd,
            HirBinOp::Or => BinOp::BitOr,
        }
    }

    /// Convert a HIR UnaryOp to a MIR UnOp.
    pub fn lower_un_op(op: HirUnaryOp) -> UnOp {
        match op {
            HirUnaryOp::Neg => UnOp::Neg,
            HirUnaryOp::Not => UnOp::Not,
            HirUnaryOp::Deref => UnOp::Not, // Deref is handled specially
        }
    }
}

/// Lower a HIR body to MIR.
///
/// This is the main entry point for HIR→MIR lowering. It takes a
/// HIR Body (expression tree) and produces a MirBody (CFG).
///
/// The returned MirBody has:
/// - LocalId(0) as the return value placeholder
/// - LocalId(1..N) for fn params
/// - LocalId(N+1..) for local variables and temporaries
/// - At least 1 basic block (the entry block)
pub fn lower_hir_body_to_mir(body: &Body, interner: &Rodeo) -> MirBody {
    let mut cx = MirLowerCtxt::new(interner, body.span);

    // Allocate LocalId(0) as the return value placeholder.
    let return_local = cx.mir.new_local(
        Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    debug_assert_eq!(return_local, LocalId(0));

    // Allocate locals for fn params.
    for param in &body.params {
        let ty = match &param.ty {
            Some(t) => lower_hir_ty_to_mir_ty(t),
            None => Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY),
        };
        cx.new_local(param.hir_id, ty, None);
    }

    // Lower the body's value expression into the return local.
    let value_local = lower_expr_to_operand(&mut cx, &body.value);

    // Assign the value to the return local.
    cx.push_assign(
        Lvalue::local(return_local, Span::DUMMY),
        Rvalue::Use(Operand::Copy(Lvalue::local(value_local, Span::DUMMY))),
        body.span,
    );

    // Terminate the current block with Return.
    cx.terminate(Terminator::Return);

    cx.mir
}

/// Lower a HIR type to a MIR type.
fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Bool => Ty::new(TyKind::Bool, span),
        HirTyKind::Char => Ty::new(TyKind::Char, span),
        HirTyKind::Int(int_ty) => Ty::new(TyKind::Int(*int_ty), span),
        HirTyKind::Uint(uint_ty) => Ty::new(TyKind::Uint(*uint_ty), span),
        HirTyKind::Float(float_ty) => Ty::new(TyKind::Float(*float_ty), span),
        HirTyKind::Never => Ty::new(TyKind::Never, span),
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(tys.iter().map(lower_hir_ty_to_mir_ty).collect()),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_lt) => Region::Var(RegionVid(0)), // placeholder
                None => Region::Erased,
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(mir_region, mir_mut, Box::new(lower_hir_ty_to_mir_ty(inner))),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(mir_mut, Box::new(lower_hir_ty_to_mir_ty(inner))),
                span,
            )
        }
        HirTyKind::Slice(inner) => {
            Ty::new(TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty(inner))), span)
        }
        HirTyKind::Array(inner, _) => Ty::new(
            TyKind::Array(
                Box::new(lower_hir_ty_to_mir_ty(inner)),
                Box::new(Const {
                    ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), span)),
                    val: ConstVal::Uint(0), // placeholder; real const eval in Stage 3
                }),
            ),
            span,
        ),
        HirTyKind::Infer => Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), span),
        _ => Ty::new(TyKind::Error, span), // complex types → Error for now
    }
}

/// Lower a HIR expression to a MIR Operand (a value that can be used
/// as an argument to a binary op, call, etc.).
///
/// Returns the LocalId of the temporary that holds the result.
fn lower_expr_to_operand(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId {
    match &expr.kind {
        HirExprKind::Lit(lit) => {
            let (const_val, ty) = cx.lit_to_const(lit);
            cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Constant(const_val)), ty, expr.span)
        }
        HirExprKind::Path(path) => {
            // If the path resolves to a local, return that local.
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return local_id;
                }
            }
            // Otherwise, create an error placeholder.
            cx.eval_rvalue_to_temp(
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                    val: ConstVal::Int(0),
                })),
                Ty::new(TyKind::Error, Span::DUMMY),
                expr.span,
            )
        }
        HirExprKind::Binary { op, lhs, rhs, .. } => {
            let lhs_local = lower_expr_to_operand(cx, lhs);
            let rhs_local = lower_expr_to_operand(cx, rhs);
            let mir_op = MirLowerCtxt::lower_bin_op(*op);
            cx.eval_rvalue_to_temp(
                Rvalue::BinaryOp(
                    mir_op,
                    Operand::Copy(Lvalue::local(lhs_local, lhs.span)),
                    Operand::Copy(Lvalue::local(rhs_local, rhs.span)),
                ),
                Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), expr.span),
                expr.span,
            )
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } => {
            let inner_local = lower_expr_to_operand(cx, inner);
            let mir_op = MirLowerCtxt::lower_un_op(*op);
            cx.eval_rvalue_to_temp(
                Rvalue::UnaryOp(
                    mir_op,
                    Operand::Copy(Lvalue::local(inner_local, inner.span)),
                ),
                Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), expr.span),
                expr.span,
            )
        }
        HirExprKind::Block(block) => lower_block(cx, block),
        HirExprKind::Call { func, args, .. } => {
            // Lower func and args to operands
            let func_local = lower_expr_to_operand(cx, func);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();
            let arg_operands: Vec<Operand> = arg_locals
                .iter()
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
                .collect();
            // Create a destination local
            let dest = cx.mir.new_local(
                Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY),
                None,
                expr.span,
            );
            // Create a continuation block
            let cont = cx.new_block();
            // Terminate current block with Call
            cx.terminate_and_goto(
                Terminator::Call {
                    func: Operand::Copy(Lvalue::local(func_local, func.span)),
                    args: arg_operands,
                    destination: Lvalue::local(dest, expr.span),
                    target: Some(cont),
                },
                cont,
            );
            dest
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => lower_if(cx, cond, then, else_.as_deref(), expr.span),
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => lower_match(cx, scrutinee, arms, expr.span),
        HirExprKind::Return { expr: ret_expr, .. } => {
            if let Some(ret) = ret_expr {
                let ret_local = lower_expr_to_operand(cx, ret);
                cx.push_assign(
                    Lvalue::local(LocalId(0), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Lvalue::local(ret_local, ret.span))),
                    expr.span,
                );
            }
            cx.terminate(Terminator::Return);
            // Return a dummy local (unreachable after Return)
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            let rhs_local = lower_expr_to_operand(cx, rhs);
            // If lhs is a Path to a local, assign directly
            if let HirExprKind::Path(path) = &lhs.kind {
                if let Res::Local(hir_id) = path.res {
                    if let Some(dest) = cx.local_of(hir_id) {
                        cx.push_assign(
                            Lvalue::local(dest, lhs.span),
                            Rvalue::Use(Operand::Copy(Lvalue::local(rhs_local, rhs.span))),
                            expr.span,
                        );
                        return rhs_local;
                    }
                }
            }
            // Fallback: just evaluate rhs
            rhs_local
        }
        HirExprKind::Tuple { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
                .collect();
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), expr.span),
                expr.span,
            )
        }
        HirExprKind::Unit => cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), expr.span),
            expr.span,
        ),
        // For unhandled expr kinds, create a placeholder
        _ => cx.eval_rvalue_to_temp(
            Rvalue::Use(Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                val: ConstVal::Int(0),
            })),
            Ty::new(TyKind::Error, Span::DUMMY),
            expr.span,
        ),
    }
}

/// Lower a HIR block to MIR. Processes statements in order and
/// evaluates the trailing expression (if any). Returns the LocalId
/// of the block's result value.
fn lower_block(cx: &mut MirLowerCtxt, block: &HirBlock) -> LocalId {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Local(local) => {
                // Lower the init expression first (if present)
                if let Some(init) = &local.init {
                    let init_local = lower_expr_to_operand(cx, init);
                    // Allocate a local for this binding
                    let ty = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), local.span);
                    let local_id = cx.new_local(local.hir_id, ty, None);
                    cx.push_assign(
                        Lvalue::local(local_id, local.span),
                        Rvalue::Use(Operand::Copy(Lvalue::local(init_local, init.span))),
                        local.span,
                    );
                } else {
                    // No init: just allocate the local
                    let ty = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), local.span);
                    cx.new_local(local.hir_id, ty, None);
                }
            }
            HirStmt::Expr(e, _) => {
                lower_expr_to_operand(cx, e);
            }
            _ => {}
        }
    }
    // Trailing expression
    if let Some(expr) = &block.expr {
        lower_expr_to_operand(cx, expr)
    } else {
        // No trailing expr → unit
        cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), block.span),
            block.span,
        )
    }
}

/// Lower an if expression to MIR control flow.
///
/// ```text
/// bb0: switchInt(cond) { true => bb1, _ => bb2 }
/// bb1: ... (then block) ... goto bb3
/// bb2: ... (else block) ... goto bb3
/// bb3: (continuation)
/// ```
fn lower_if(
    cx: &mut MirLowerCtxt,
    cond: &HirExpr,
    then: &HirBlock,
    else_: Option<&HirExpr>,
    span: Span,
) -> LocalId {
    let cond_local = lower_expr_to_operand(cx, cond);
    let then_block = cx.new_block();
    let else_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_local = cx.mir.new_local(
        Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), span),
        None,
        span,
    );

    // Terminate current block: switchInt(cond) { 1 => then, _ => else }
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Lvalue::local(cond_local, cond.span)),
        targets: vec![(ConstVal::Bool(true), then_block)],
        otherwise: else_block,
    });

    // Then block
    cx.current_block = then_block;
    let then_result = lower_block(cx, then);
    cx.push_assign(
        Lvalue::local(result_local, span),
        Rvalue::Use(Operand::Copy(Lvalue::local(then_result, then.span))),
        then.span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // Else block
    cx.current_block = else_block;
    if let Some(else_expr) = else_ {
        let else_result = lower_expr_to_operand(cx, else_expr);
        cx.push_assign(
            Lvalue::local(result_local, span),
            Rvalue::Use(Operand::Copy(Lvalue::local(else_result, else_expr.span))),
            else_expr.span,
        );
    } else {
        // No else → unit
        cx.push_assign(
            Lvalue::local(result_local, span),
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            span,
        );
    }
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation block
    cx.current_block = cont_block;
    result_local
}

/// Lower a match expression to MIR control flow.
///
/// For Stage 2.1b, match is lowered to a simplified SwitchInt:
/// - Integer literal patterns → constant targets
/// - Wildcard `_` → otherwise
/// - Other patterns → otherwise (simplified)
fn lower_match(cx: &mut MirLowerCtxt, scrutinee: &HirExpr, arms: &[HirArm], span: Span) -> LocalId {
    let scrut_local = lower_expr_to_operand(cx, scrutinee);
    let cont_block = cx.new_block();
    let result_local = cx.mir.new_local(
        Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), span),
        None,
        span,
    );

    // Collect targets: (constant, arm_block) pairs
    let mut targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arm_blocks: Vec<BasicBlockId> = Vec::new();
    let otherwise_block = cx.new_block();

    for arm in arms {
        let arm_block = cx.new_block();
        arm_blocks.push(arm_block);

        // Check if this arm's pattern is a literal
        if let HirPatKind::Lit(expr) = &arm.pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                targets.push((ConstVal::Int(*n), arm_block));
                continue;
            }
            if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                targets.push((ConstVal::Bool(*b), arm_block));
                continue;
            }
        }

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
        // For Stage 2.1b, we treat them all as "otherwise"
        // The first non-literal arm becomes the otherwise target
        if targets.is_empty() {
            // If no literal targets yet, this arm is the otherwise
            // We'll set it as the otherwise block
        }
    }

    // Terminate current block with SwitchInt
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Lvalue::local(scrut_local, scrutinee.span)),
        targets: targets.clone(),
        otherwise: otherwise_block,
    });

    // Lower each arm body
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        cx.current_block = arm_block;

        // Collect pattern bindings (for Ident patterns)
        collect_pat_bindings_for_mir(cx, &arm.pat);

        // Lower the arm body
        let arm_result = lower_expr_to_operand(cx, &arm.body);
        cx.push_assign(
            Lvalue::local(result_local, span),
            Rvalue::Use(Operand::Copy(Lvalue::local(arm_result, arm.body.span))),
            arm.span,
        );
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Lower the otherwise block (for non-literal patterns)
    cx.current_block = otherwise_block;
    // Find the first arm with a non-literal pattern
    for arm in arms {
        let is_literal = matches!(&arm.pat.kind, HirPatKind::Lit(_));
        if !is_literal {
            collect_pat_bindings_for_mir(cx, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body);
            cx.push_assign(
                Lvalue::local(result_local, span),
                Rvalue::Use(Operand::Copy(Lvalue::local(arm_result, arm.body.span))),
                arm.span,
            );
            break;
        }
    }
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation
    cx.current_block = cont_block;
    result_local
}

/// Collect pattern bindings into the MIR local map.
fn collect_pat_bindings_for_mir(cx: &mut MirLowerCtxt, pat: &HirPat) {
    match &pat.kind {
        HirPatKind::Ident(_mode, _ident, sub) => {
            // Allocate a local for this binding
            let ty = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), pat.span);
            cx.new_local(pat.hir_id, ty, None);
            if let Some(s) = sub {
                collect_pat_bindings_for_mir(cx, s);
            }
        }
        HirPatKind::TupleStruct(_, pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Tuple(pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Struct(_, fields, _) => {
            for f in fields {
                collect_pat_bindings_for_mir(cx, &f.pat);
            }
        }
        _ => {}
    }
}
