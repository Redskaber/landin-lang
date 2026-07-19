# Stage 2.0 — Type Check + Borrow Check (NLL on MIR)

> **Sub-stage**: 2.0 (Month 5-7, split into 2.1/2.2/2.3)
> **Complexity**: L3 (core architecture, cross-module changes)
> **Baseline rounds**: 8~15 per sub-stage
> **Goal**: Implement type inference, trait resolution, and NLL borrow
> checking on MIR, producing a sound type system that prevents
> use-after-move, double-free, and data races at compile time.

---

## Sub-stage Breakdown

Stage 2.0 is too large for a single pass. Per the "小步快跑" principle,
it is split into 3 sub-stages:

### Stage 2.1 — MIR Data Structures + HIR→MIR Lowering
- Define MIR node types (BasicBlock, Statement, Terminator, Operand, Rvalue, Lvalue)
- Implement HIR→MIR lowering (control flow graph construction)
- MIR type representation (Ty with inference variables)
- 30+ MIR unit tests
- **Complexity**: L2 (mechanical lowering, similar to Stage 1.2)
- **Rounds**: 4~9

### Stage 2.2 — Type Inference + Trait Resolution
- Type inference engine (unification, Hindley-Milner + constraints)
- Trait resolution (method dispatch, associated types, where clause obligations)
- Generic monomorphization
- Type error reporting
- 30+ typeck tests
- **Complexity**: L3 (core algorithm, complex unification logic)
- **Rounds**: 8~15

### Stage 2.3 — NLL Borrow Check
- Borrow checker on MIR (non-lexical lifetimes)
- Move tracking (use-after-move detection)
- Lifetime inference (region inference)
- Borrow error reporting
- 30+ borrowck tests
- **Complexity**: L3 (core safety algorithm, region inference)
- **Rounds**: 8~15

---

## Stage 2.1 Tasks (MIR Data Structures + HIR→MIR Lowering)

### Phase A — MIR Data Structures

#### A1. MIR types: `Ty` + `TyKind` + inference variables

```rust
// src/mir/ty.rs
pub struct Ty { pub kind: TyKind, pub inferred: Option<InferVar> }
pub enum TyKind {
    Bool, Char, Int(IntTy), Uint(UintTy), Float(FloatTy),
    Str, Never,
    Ref(Region, Mutability, Box<Ty>),
    RawPtr(Mutability, Box<Ty>),
    Array(Box<Ty>, Const),
    Slice(Box<Ty>),
    Tuple(Vec<Ty>),
    FnDef(DefId, Vec<Ty>),
    FnPtr(Vec<Ty>, Box<Ty>),
    Closure(DefId, Vec<Ty>),
    Adt(DefId, Vec<Ty>),
    Foreign,
    Param(ParamTy),
    Error,
}
pub enum InferVar { TyVar(TyVid), IntVar(IntVid), FloatVar(FloatVid) }
```

#### A2. MIR Body: `BasicBlock` + `Statement` + `Terminator`

```rust
// src/mir/body.rs
pub struct MirBody {
    pub basic_blocks: Vec<BasicBlock>,
    pub local_decls: Vec<LocalDecl>,
    pub source_info: ..., // span tracking
}
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
pub enum Statement { Assign(Lvalue, Rvalue), Nop, }
pub enum Terminator {
    Goto(BasicBlock),
    SwitchInt { discr: Operand, targets: Vec<(Const, BasicBlock)>, otherwise: BasicBlock },
    Return,
    Unreachable,
    Drop { place: Lvalue, target: BasicBlock, unwind: Option<BasicBlock> },
    Call { func: Operand, args: Vec<Operand>, destination: Lvalue, target: Option<BasicBlock> },
}
```

#### A3. `Lvalue` + `Rvalue` + `Operand`

```rust
pub enum Lvalue {
    Local(LocalId),
    Static(DefId),
    Projection(Box<Lvalue>, ProjectionElem),
}
pub enum ProjectionElem { Deref, Field(FieldId), Index(LocalId), ConstantIndex {..} }
pub enum Rvalue {
    Use(Operand),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Ref(Region, BorrowKind, Lvalue),
    Cast(CastKind, Operand, Ty),
    Aggregate(AggregateKind, Vec<Operand>),
}
pub enum Operand { Copy(Lvalue), Move(Lvalue), Constant(Const), }
```

### Phase B — HIR→MIR Lowering

#### B1. `lower_hir_to_mir` — entry point

Walk each HIR body and construct a MIR body:
1. Assign LocalIds to fn params + local variables
2. Build control flow graph (basic blocks + terminators)
3. Lower expressions to Rvalues/Operands
4. Lower control flow (if/match/loop) to Goto/SwitchInt

#### B2. Expression lowering

- `HirExprKind::Lit` → `Rvalue::Use(Operand::Constant(..))`
- `HirExprKind::Binary { op, lhs, rhs }` → `Rvalue::BinaryOp(..)`
- `HirExprKind::Path` → `Operand::Copy/Move(Lvalue::Local(..))`
- `HirExprKind::Call` → `Terminator::Call { .. }`
- `HirExprKind::If` → `SwitchInt` + 2 basic blocks
- `HirExprKind::Match` → `SwitchInt` with multiple targets
- `HirExprKind::Block` → sequential statements + optional terminator

#### B3. Pattern → SwitchInt lowering

Match expressions are lowered to `SwitchInt` terminators:
- Literal patterns → constant targets
- Wildcard → `otherwise` target
- Struct/tuple patterns → temporary + field projections

### Phase C — Tests

#### C1. MIR structural tests (20+)

- 5 tests: MIR type construction (primitive/ref/tuple/adt/fn-ptr)
- 5 tests: BasicBlock + Statement + Terminator construction
- 5 tests: HIR→MIR lowering for simple expressions
- 5 tests: Control flow lowering (if/match/loop)

#### C2. Integration: all existing programs lower to MIR

- Verify all 451 existing test programs can be lowered to MIR without panic
- Verify MIR has at least 1 basic block per body

---

## Acceptance Criteria (Stage 2.1)

1. ✅ All tasks implemented
2. ✅ `cargo build` 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥480 tests (451 existing + 30 new MIR)
6. ✅ All HIR bodies can be lowered to MIR without panic
7. ✅ Committee vote ≥ 95% weighted approval (strict mode)
