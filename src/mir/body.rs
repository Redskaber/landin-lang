//! MIR body: basic blocks, statements, terminators.
//!
//! Per 06-mir.md, a MIR body is a control flow graph (CFG) of basic
//! blocks. Each block contains a sequence of statements followed by
//! a terminator.
//!
//! Stage 3.47 (per §16 — 阶段间接口隔离): `MirBody` now carries an
//! `adt_layouts` side-table mapping `DefId → AdtLayout`. This lets codegen
//! resolve `TyKind::Adt(def_id, _)` to its storage layout **without reading
//! HIR** — closing the L-PIPE-1 pipeline-coupling debt carried since
//! Stage 3.30.

use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use std::collections::HashMap;
use std::sync::Arc;

/// Stage 3.47 (L-PIPE-1 closure): Storage layout of an ADT
/// (struct or enum), computed once by MIR lower and consumed by codegen.
///
/// Per §16 (阶段间接口隔离): MIR lower reads HIR (data flows downstream —
/// allowed), and **sinks** the resulting layout into `MirBody::adt_layouts`.
/// Codegen then reads the layout from MIR — no HIR lookup needed.
///
/// `Enum` carries *all* variants' payload types (not just the first non-unit),
/// so the future L-ENUM-UNION fix in Stage 4 can switch codegen from
/// "first non-unit payload" to "union of all payloads" with **zero MIR data
/// structure change** (forward-compatible design per §15.2.1).
#[derive(Debug, Clone)]
pub enum AdtLayout {
    /// Struct layout: ordered field types.
    Struct { field_tys: Vec<Ty> },
    /// Enum layout: discriminant type + per-variant payload types.
    /// `variant_payloads[i]` is the payload of variant `i` (empty for unit
    /// variants). Codegen currently uses the first non-empty payload (Stage
    /// 3.38 behavior); Stage 4's L-ENUM-UNION will use the union.
    Enum {
        discriminant_ty: Ty,
        variant_payloads: Vec<Vec<Ty>>,
    },
}

/// Side-table mapping ADT `DefId` → `AdtLayout`, stored on `MirBody`.
pub type AdtLayouts = HashMap<crate::hir::DefId, AdtLayout>;

/// Stage 15.8 (v0.2): Shared crate-level ADT layouts.
///
/// `Arc<AdtLayouts>` is cheap to clone (refcount bump) and shares the
/// same underlying HashMap across all MirBodies in a compilation. This
/// eliminates the per-body duplication from Stages 14.30-14.84 where each
/// body had its own `AdtLayouts` HashMap (Phase 2 audit: ~500KB waste
/// for a typical 100-fn, 50-type crate).
///
/// Per `docs/develop/v0/stage-15/v0.2-preparation.md` Phase 1 quick wins:
/// "Share AdtLayouts crate-level instead of per-body (1 day)".
/// Per §1.0 原则 6 "通用 > 特例": one shared map for all bodies.
/// Per §16: codegen reads from MIR (via Arc deref), not HIR.
pub type SharedAdtLayouts = Arc<AdtLayouts>;

/// A MIR body: the CFG representation of a function body.
#[derive(Debug, Clone)]
pub struct MirBody {
    /// All basic blocks, indexed by BasicBlockId.
    pub basic_blocks: Vec<BasicBlock>,
    /// Local variable declarations (params + locals).
    pub local_decls: Vec<LocalDecl>,
    /// Span of the function body (for error reporting).
    pub span: Span,
    /// Stage 3.47 (L-PIPE-1): ADT layouts sunk from HIR by MIR lower.
    /// Consumed by codegen to avoid HIR lookup (per §16).
    /// Empty in test contexts where MIR bodies are constructed without HIR.
    ///
    /// Stage 15.8 (v0.2): Changed from `AdtLayouts` (owned HashMap) to
    /// `SharedAdtLayouts` (Arc<AdtLayouts>). The driver builds the
    /// crate-level map once from HIR and shares the Arc across all bodies.
    /// This eliminates per-body HashMap duplication (~500KB for typical crate).
    pub adt_layouts: SharedAdtLayouts,
    // Stage 15.65 (HP-22 cleanup): `dyn_trait_calls` field REMOVED.
    // The dyn Trait method call info is now carried directly on the
    // `TerminatorKind::Call` terminator's `dyn_trait_call: Option<DynTraitMethodCall>`
    // field (Stage 15.30). The legacy side-table (indexed by a magic
    // `ConstVal::Int(index)` marker on the func operand) has been removed.
    //
    // Per §1.0 原则 3 "显式 > 隐式": the dyn Trait info is now explicit on
    // the terminator, not implicit in a side-table indexed by a magic constant.
    // Per §15 "最优 > 最小": dead code (side-table + legacy codegen path) removed.
    // Per §16: MIR carries the info as data on the terminator (still no HIR
    // lookup in codegen).
    //
    // Stage 15.12 (v0.2): `lower_type_errors` field REMOVED.
    // Type errors collected during MIR lowering are now returned from
    // `lower_hir_body_to_mir_full*` as a separate `Vec<TypeError>` in
    // the return tuple. This separates IR data from error collection
    // (was an architectural smell — IR carrying error collection).
    // Per §1.0 原则 3 "显式 > 隐式": errors are now explicit in the
    // function signature, not implicit on the IR struct.
    /// Stage 16.17 (Task 10 Step 3+4 fix): The DefId of the function
    /// this MirBody belongs to. For regular functions, this is the fn's
    /// DefId. For synthesized closure `call` functions, this is the
    /// closure's DefId (allocated via `allocate_closure_def_id`).
    ///
    /// Used by codegen to resolve the function name via `fn_name_by_def_id`.
    /// `None` for test contexts where MIR bodies are constructed without
    /// a DefId.
    ///
    /// Per §16: data carried on the IR, not looked up from HIR.
    pub def_id: Option<crate::hir::DefId>,

    /// Stage 18.234 (TD-METHOD-RESOLVE-STRICT fix): Deferred method calls
    /// that couldn't be resolved at MIR lower time because the receiver
    /// type was Infer. typeck re-checks these after defaulting (Phase 5.5)
    /// when the receiver type is resolved to a concrete type.
    ///
    /// Per §1.0 原則 4 (报错>静默): unresolved methods must be reported.
    /// Per §1.0 原則 6 (通解>特例): one side-table for all deferred calls.
    /// Per §16: data carried on the IR, re-checked by typeck.
    pub deferred_method_calls: Vec<DeferredMethodCall>,
}

/// Stage 18.234: A method call that was deferred at MIR lower time
/// because the receiver type was Infer. typeck re-checks these after
/// type defaulting to report "no method found" errors.
#[derive(Debug, Clone)]
pub struct DeferredMethodCall {
    /// The receiver local (whose type was Infer at lower time).
    pub recv_local: crate::mir::place::LocalId,
    /// The method name that was called.
    pub method_name: crate::lexer::Symbol,
    /// Source span for error reporting.
    pub span: Span,
}

impl MirBody {
    pub fn new(span: Span) -> Self {
        Self {
            basic_blocks: Vec::new(),
            local_decls: Vec::new(),
            span,
            // clippy::arc_with_non_send_sync: AdtLayouts is not Send+Sync
            // (contains Ty with Box/Vec). Compiler is single-threaded, so
            // Arc is fine — keeps door open for future multi-threaded LSP.
            #[allow(clippy::arc_with_non_send_sync)]
            adt_layouts: Arc::new(AdtLayouts::new()),
            def_id: None,
            deferred_method_calls: Vec::new(),
        }
    }

    /// Allocate a new basic block and return its ID.
    pub fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.basic_blocks.len() as u32);
        self.basic_blocks.push(BasicBlock {
            statements: Vec::new(),
            terminator: Terminator::unreachable(Span::DUMMY),
            span: Span::DUMMY,
            terminator_span: Span::DUMMY,
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

    /// Stage 3.47 (L-PIPE-1): Record an ADT's storage layout.
    /// Called by MIR lower when it constructs a `TyKind::Adt(def_id, _)`.
    /// Idempotent: if the same `def_id` is registered twice with the same
    /// layout, the second call is a no-op.
    ///
    /// Stage 15.8 (v0.2): Uses `Arc::make_mut` to get mutable access to the
    /// inner HashMap. This clones the HashMap if the Arc is shared (refcount > 1),
    /// but in practice the Arc is only shared after the driver finishes building
    /// all bodies — so during lowering (when this method is called), the Arc
    /// has refcount 1 and `make_mut` is a no-op.
    pub fn register_adt_layout(&mut self, def_id: crate::hir::DefId, layout: AdtLayout) {
        let layouts = std::sync::Arc::make_mut(&mut self.adt_layouts);
        layouts.entry(def_id).or_insert(layout);
    }
}

/// A basic block: a straight-line sequence of statements ending with
/// a terminator (control flow instruction).
///
/// Stage 14.107 (HP-19/21 fix): BasicBlock now carries `span` and
/// `terminator_span` fields for debug info attribution.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Statements executed in order.
    pub statements: Vec<Statement>,
    /// The terminator: how control leaves this block.
    pub terminator: Terminator,
    /// Stage 14.107 (HP-19): Source span of this basic block.
    /// Set to the span of the first statement, or DUMMY if empty.
    pub span: crate::session::Span,
    /// Stage 14.107 (HP-21): Source span of the terminator.
    /// Set during MIR lowering to the span of the source construct
    /// that generated this terminator (e.g., `return` keyword span,
    /// `if` condition span, `match` scrutinee span).
    pub terminator_span: crate::session::Span,
}

/// A MIR statement: `Place = Rvalue`.
#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    /// `place = rvalue`
    Assign(Box<(Place, Rvalue)>),
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
    /// Distinct from TerminatorKind::Drop (which is for control-flow drops).
    Deinit(Place),
    /// Stage 18.226 (v0.2 Phase 2): Store value to raw pointer.
    /// `*ptr = val` → store value at pointer address.
    /// Per §1.0 原則 6 (通解>特例): one Store for all pointer types.
    /// Per §16.3 (06-mir.md): MIR intrinsic ops design.
    Store {
        /// Pointer (place) to store to.
        ptr: Place,
        /// Value to store.
        val: Operand,
        /// Value type (for codegen).
        val_ty: Ty,
    },
    // Stage 18.48: Println variant removed — println! now goes through
    // the Call path via __landin_println macro expansion.
    // Per §1.0 原則 6 "通用 > 特解": the 通解 (Call) has replaced the 特解.
}

/// Stage 14.112 (HP-21 proper fix): Terminator is now a struct carrying
/// both `kind` (the control-flow variant) and `span` (source location).
/// Previously the Terminator was a bare enum — no source location info.
/// v0.2 debug info needs source spans on terminators for accurate
/// line attribution in stack traces and debugger stepping.
///
/// The `terminator_span` field on BasicBlock (added in Stage 14.107 as
/// a shortcut) is now redundant — the span lives on the Terminator itself.
/// It's kept for backward compatibility but should be removed in v0.2.
#[derive(Debug, Clone)]
pub struct Terminator {
    /// The actual terminator kind (Goto, SwitchInt, Return, etc.)
    pub kind: TerminatorKind,
    /// Source span for debug info.
    pub span: crate::session::Span,
}

/// The kind of terminator: how control leaves a basic block.
#[derive(Debug, Clone)]
pub enum TerminatorKind {
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
        place: Place,
        target: BasicBlockId,
        unwind: Option<BasicBlockId>,
    },
    /// Function call: `destination = func(args)`.
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockId>,
        /// Stage 15.30 (HP-22): dyn Trait method call info.
        ///
        /// When `Some`, this call is a dyn Trait vtable indirect call.
        /// The `func` operand is a placeholder (ConstVal::Int with the
        /// old side-table index — kept for backward compat during migration).
        /// Codegen checks this field FIRST; if `Some`, it uses the
        /// DynTraitMethodCall info directly instead of decoding the
        /// magic `Error + Int(index)` marker.
        ///
        /// Per §1.0 原則 3 "显式 > 隐式": the dyn Trait info is now explicit
        /// on the terminator, not implicit in a side-table.
        /// Per §16: MIR carries the info as data on the terminator.
        dyn_trait_call: Option<crate::mir::dyn_trait::DynTraitMethodCall>,
    },
    /// Assert a boolean condition (for overflow checks, Stage 3+).
    Assert {
        cond: Operand,
        expected: bool,
        target: BasicBlockId,
        msg: AssertMessage,
    },
}

// Stage 14.112: Compatibility re-exports so existing code that writes
// `TerminatorKind::Goto(x)` still compiles. These delegate to TerminatorKind.
// New code should use `Terminator { kind: TerminatorKind::Goto(x), span }`.
impl Terminator {
    pub fn new(kind: TerminatorKind, span: crate::session::Span) -> Self {
        Terminator { kind, span }
    }

    pub fn goto(target: BasicBlockId, span: crate::session::Span) -> Self {
        Terminator {
            kind: TerminatorKind::Goto(target),
            span,
        }
    }

    pub fn ret(span: crate::session::Span) -> Self {
        Terminator {
            kind: TerminatorKind::Return,
            span,
        }
    }

    pub fn unreachable(span: crate::session::Span) -> Self {
        Terminator {
            kind: TerminatorKind::Unreachable,
            span,
        }
    }
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
///
/// Stage 18.67: `NegOverflow` carries the original operand for unary negation
/// overflow check (e.g., `-i32::MIN` overflows). Codegen emits
/// `0 - x` with `SubOverflow` semantics.
#[derive(Debug, Clone)]
pub enum AssertMessage {
    Overflow(BinOp, Operand, Operand),
    DivisionByZero(Operand),
    /// Stage 18.67: Unary negation overflow (e.g., `-i32::MIN`).
    /// Carries the operand being negated.
    NegOverflow(Operand),
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
                Place::local(local, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        // Set terminator: return
        body.block_mut(bb).terminator = Terminator::new(TerminatorKind::Return, Span::DUMMY);

        assert_eq!(body.block(bb).statements.len(), 1);
        assert!(matches!(
            body.block(bb).terminator.kind,
            TerminatorKind::Return
        ));
    }

    #[test]
    fn terminator_goto() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        body.block_mut(bb0).terminator = Terminator::new(TerminatorKind::Goto(bb1), Span::DUMMY);
        assert!(matches!(
            &body.block(bb0).terminator.kind,
            TerminatorKind::Goto(BasicBlockId(1))
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
        body.block_mut(bb0).terminator = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(discr_local, Span::DUMMY)),
                targets: vec![(ConstVal::Int(1), bb1)],
                otherwise: bb2,
            },
            Span::DUMMY,
        );
        match &body.block(bb0).terminator.kind {
            TerminatorKind::SwitchInt {
                targets, otherwise, ..
            } => {
                assert_eq!(targets.len(), 1);
                assert_eq!(*otherwise, bb2);
            }
            _ => panic!("expected SwitchInt"),
        }
    }

    // Stage 3.47 (L-PIPE-1) tests — verify the new adt_layouts side-table
    // is correctly initialized and populated.

    #[test]
    fn mir_body_adt_layouts_starts_empty() {
        // New MirBody should have an empty adt_layouts (no HIR sunk yet).
        let body = MirBody::new(Span::DUMMY);
        assert!(body.adt_layouts.is_empty());
    }

    #[test]
    fn mir_body_register_adt_layout_struct() {
        // Register a struct layout; should be retrievable.
        let mut body = MirBody::new(Span::DUMMY);
        let def_id = crate::hir::DefId::new(42);
        let layout = AdtLayout::Struct {
            field_tys: vec![
                Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Int(ast::IntTy::I64), Span::DUMMY),
            ],
        };
        body.register_adt_layout(def_id, layout);
        assert_eq!(body.adt_layouts.len(), 1);
        match &body.adt_layouts[&def_id] {
            AdtLayout::Struct { field_tys } => {
                assert_eq!(field_tys.len(), 2);
                assert!(matches!(field_tys[0].kind, TyKind::Int(ast::IntTy::I32)));
                assert!(matches!(field_tys[1].kind, TyKind::Int(ast::IntTy::I64)));
            }
            AdtLayout::Enum { .. } => panic!("expected Struct layout"),
        }
    }

    #[test]
    fn mir_body_register_adt_layout_enum() {
        // Register an enum layout with multiple variants.
        let mut body = MirBody::new(Span::DUMMY);
        let def_id = crate::hir::DefId::new(7);
        let layout = AdtLayout::Enum {
            discriminant_ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            variant_payloads: vec![
                vec![],                                                   // unit variant
                vec![Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)], // Some(i32)
            ],
        };
        body.register_adt_layout(def_id, layout);
        match &body.adt_layouts[&def_id] {
            AdtLayout::Enum {
                discriminant_ty,
                variant_payloads,
            } => {
                assert_eq!(variant_payloads.len(), 2);
                assert!(variant_payloads[0].is_empty());
                assert_eq!(variant_payloads[1].len(), 1);
                assert!(matches!(discriminant_ty.kind, TyKind::Int(ast::IntTy::I32)));
            }
            AdtLayout::Struct { .. } => panic!("expected Enum layout"),
        }
    }

    #[test]
    fn mir_body_register_adt_layout_idempotent() {
        // Registering the same def_id twice should not overwrite.
        let mut body = MirBody::new(Span::DUMMY);
        let def_id = crate::hir::DefId::new(1);
        let layout1 = AdtLayout::Struct {
            field_tys: vec![Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)],
        };
        let layout2 = AdtLayout::Struct {
            field_tys: vec![
                Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Int(ast::IntTy::I64), Span::DUMMY),
            ],
        };
        body.register_adt_layout(def_id, layout1);
        body.register_adt_layout(def_id, layout2); // should be ignored
        match &body.adt_layouts[&def_id] {
            AdtLayout::Struct { field_tys } => assert_eq!(field_tys.len(), 1), // layout1 won
            _ => panic!(),
        }
    }
}
