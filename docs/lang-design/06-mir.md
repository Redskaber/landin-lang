# 06 — MIR 设计

> MIR（Mid-level IR）是 Landin 编译器的**灵魂**（R1、R2 报告一致结论）。所有静态分析（borrow check、liveness、初始化、drop 顺序）在 MIR 上做。本文定义 MIR 数据结构、构建算法、优化 pass。

---

## 1. 设计目标

参考 Rust RFC #1211（MIR 引入）与 R2 报告现代 rustc MIR 实现：

1. **简单**：无 match、无闭包表达式、无 ref 绑定，全部 lowering 掉
2. **CFG-based**：基本块 + 跳转，便于做数据流分析
3. **三地址码**：每条指令至多 3 个操作数
4. **类型保留**：MIR 保留 Rust 类型信息（与 LLVM IR 不同），便于 borrow check
5. **SSA-like**：Local 可多次赋值，但分析时按需 SSA 化

---

## 2. 顶层结构

```rust
struct Body {
    basic_blocks: IndexVec<BasicBlock, BasicBlockData>,
    locals: IndexVec<Local, LocalDecl>,
    source_scopes: IndexVec<SourceScope, SourceScopeData>,
    arg_count: usize,
    spread_arg: Option<Local>,    // v0.2: 外部 ABI
    span: Span,
    /// Stage 3.47 (L-PIPE-1 closure per §16): ADT layouts sunk from HIR.
    /// Maps DefId → AdtLayout for every `TyKind::Adt(def_id, _)` referenced
    /// by this body's locals or `AggregateKind::Adt` field_tys. Populated
    /// by MIR lower at the end of `lower_hir_body_to_mir_full`. Consumed
    /// by codegen to resolve ADT storage layouts **without reading HIR**.
    adt_layouts: HashMap<DefId, AdtLayout>,
}

type BasicBlock = u32;
type Local = u32;
type SourceScope = u32;

struct BasicBlockData {
    statements: Vec<Statement>,
    terminator: Option<Terminator>,
    is_cleanup: bool,            // v0.2: unwind
}

struct LocalDecl {
    ty: Ty,
    mutability: Mutability,
    source_info: SourceInfo,
    // 推导出的属性
    is_temp: bool,                // 临时变量
    is_arg: bool,                 // 函数参数
}

struct SourceInfo {
    span: Span,
    scope: SourceScope,
}

struct SourceScopeData {
    span: Span,
    parent_scope: Option<SourceScope>,
}

/// Stage 3.47 (L-PIPE-1 closure per §16): Storage layout of an ADT
/// (struct or enum), computed once by MIR lower (reading HIR — allowed
/// per §16.2.1, data flows downstream) and consumed by codegen (reading
/// MIR — no HIR lookup, closing L-PIPE-1).
///
/// `Enum` carries *all* variants' payload types (not just the first
/// non-unit), so the future L-ENUM-UNION fix in Stage 4 can switch
/// codegen from "first non-empty payload" to "union of all payloads"
/// with **zero MIR data-structure change** (forward-compatible design
/// per §15.2.1).
enum AdtLayout {
    Struct { field_tys: Vec<Ty> },
    Enum {
        discriminant_ty: Ty,
        variant_payloads: Vec<Vec<Ty>>,
    },
}
```

---

## 3. Statement

```rust
enum StatementKind {
    /// `let p = v;` — assign rvalue to place
    Assign(Box<Place>, Box<Rvalue>),
    
    /// `FakeRead(cause, place)` — 读取 place 但不产生值
    /// 用于 match scrutinee、closure capture、let binding 等隐式读
    FakeRead(ReadCause, Box<Place>),
    
    /// `SetDiscriminant(place, variant_idx)` — 直接设 enum tag（优化）
    SetDiscriminant(Box<Place>, VariantIdx),
    
    /// `Deinit(place)` — 标记 place 为未初始化（v1.2 删除：rustc 已移除，使用 SetDiscriminant + 显式处理）
    /// （此 variant 在 v1.2 中删除，不再出现在 StatementKind 中）
    /// Deinit(Box<Place>),  // 已删除
    
    /// `StorageLive(_1)` — 局部变量开始生命周期
    StorageLive(Local),
    
    /// `StorageDead(_1)` — 局部变量结束生命周期（触发 drop）
    StorageDead(Local),
    
    /// `Intrinsic(non_diverging)` — 不产生值的 intrinsic，如 copy_nonoverlapping
    Intrinsic(NonDivergingIntrinsic),
    
    /// `let _ = const_eval(...)` — 编译期求值限制计数
    ConstEvalCounter,
    
    /// `AscribeUserType(_1, ty, variance)` — 用户类型标注（typeck 用）
    AscribeUserType(Box<Place>, Box<Ty>, Variance),
    
    /// `PlaceMention(place)` — 类似 FakeRead，但不创建借用（用于 `let _ = expr` 的 side effect）
    PlaceMention(Box<Place>),
    
    /// Nop（用于优化 pass 占位）
    Nop,
}

enum ReadCause {
    MatchScrutinee,
    Use,
    LetBinding,
    ClosureCapture,
    CopyForDeref,
}

enum NonDivergingIntrinsic {
    CopyNonOverlapping { src: Operand, dst: Operand, count: Operand },
    // v0.2: more
}
```

struct Statement {
    kind: StatementKind,
    source_info: SourceInfo,
}

```

---

## 4. Place（左值）

```rust
enum Place {
    /// 局部变量
    Local(Local),
    
    /// Place 投影
    Projection {
        base: Box<Place>,
        elem: ProjectionElem,
    },
}

enum ProjectionElem {
    /// `place.field`
    Field(Field, Ty),
    
    /// `*place` (deref)
    Deref,
    
    /// `place[index]`
    Index(Local),
    
    /// `place.constant_index { offset, from_end }`
    /// 优化：编译期已知的切片索引
    ConstantIndex {
        offset: u32,
        from_end: bool,
        min_length: u32,
    },
    
    /// `place.subslice[from..to]`
    Subslice {
        from: u32,
        to: u32,
        from_end: bool,
    },
    
    /// `place.downcast(Variant)` — enum variant 模式匹配
    Downcast(Option<Symbol>, VariantIdx),
}

type Field = u32;
type VariantIdx = u32;
```

Place 是"内存位置"的抽象，可被取地址、可被赋值、可被 borrow。

---

## 5. Rvalue（右值）

```rust
enum Rvalue {
    /// `place`（place 作为右值）
    Use(Box<Place>),
    
    /// `&place` / `&mut place`
    Ref(Region, BorrowKind, Box<Place>),
    
    /// `&raw const place` / `&raw mut place` (v0.2)
    /// v1.2 修正：与 rustc 一致，使用 RawPtr(RawPtrKind, Place) 而非 AddressOf
    RawPtr(RawPtrKind, Box<Place>),

    /// `len(place)` — 数组/切片长度
    Len(Box<Place>),
    
    /// `place as ty`
    Cast(CastKind, Box<Operand>, Ty),
    
    /// `binop lhs, rhs`
    BinaryOp(BinOp, Box<Operand>, Box<Operand>),
    
    /// `unop operand`
    UnaryOp(UnOp, Box<Operand>),
    
    /// `[elem; count]` — repeat array 构造
    Repeat(Box<Operand>, ty::Const<'tcx>),
    
    /// 构造 aggregate
    Aggregate(AggregateKind, Vec<Operand>),
    
    /// ` discriminant(place)` — 取 enum discriminant
    Discriminant(Box<Place>),
    
    /// `_1 = Y` where Y is a thread local static (v0.2)
    ThreadLocalRef(DefId),
    
    /// v1.2 删除：NullaryOp(SizeOf/AlignOf) 已被 rustc 移除，改为 intrinsic
    /// （SizeOf/AlignOf 在 v0.2 中通过 core::mem::size_of/align_of intrinsic 实现）
    // NullaryOp(NullOp, Ty),  // v1.2 删除
}

enum BorrowKind {
    // v1.2.2 修正：与 rustc master 一致，使用 Fake(FakeBorrowKind) 而非 Shallow
    Shared,                        // &T
    Fake(FakeBorrowKind),          // match scrutinee / closure capture 的所谂借用
    Mut { kind: MutBorrowKind },   // &mut T
    // 注意：v1.2 修正——rustc 已废弃 BorrowKind::Unique，raw ptr 走 Rvalue::RawPtr
}

enum FakeBorrowKind {
    Shallow,   // 仅借用表面（match scrutinee）
    Deep,      // 深借用（closure capture）
}

enum MutBorrowKind {
    Default,                // 普通可变借用
    TwoPhaseBorrow,         // two-phase borrow（method call auto-ref）
    ClosureCapture,         // 闭包捕获的可变借用
}

enum CastKind {
    Numeric,                       // i32 as u64
    Pointer,                       // *const T as *const U
    PointerExposeProvenance,      // *const T as usize（指针→整数）
    PointerWithExposedProvenance, // usize as *const T（整数→指针）
    Unsize,                        // &[T; N] as &[T]
    FnPointer,                     // fn() as fn()
    Transmute,                     // mem::transmute
    // v0.2: FloatToInt, IntToFloat, PtrToPtr 等
}

enum AggregateKind {
    Tuple,
    Array(Ty),
    Adt(DefId, VariantIdx, Vec<GenericArg>),
    Closure(DefId, Vec<GenericArg>),
    // v1.2.2 修正：与 rustc master 一致，Coroutine 不含 Movability 字段（Movability 在 CoroutineKind 内）
    Coroutine(DefId, Vec<GenericArg>),
    CoroutineClosure(DefId, Vec<GenericArg>),
    // v0.2: 实际实现 Coroutine/CoroutineClosure
}

/// v1.2 新增：RawPtrKind 与 rustc 一致
/// v1.2.2 修正：Fake → FakeForPtrMetadata（与 rustc master 一致）
enum RawPtrKind {
    Mut,
    Const,
    FakeForPtrMetadata,
}

/// v1.2 删除：NullOp 已被 rustc 移除，SizeOf/AlignOf 改为 intrinsic
// enum NullOp { SizeOf, AlignOf }  // v1.2 删除
```

---

## 6. Operand（操作数）

```rust
enum Operand {
    /// Place 作为操作数（隐式读）
    Copy(Box<Place>),       // 要求 Place 类型: Copy
    
    /// Place 作为操作数（move）
    Move(Box<Place>),       // 把 Place 的值 move 出来
    
    /// 常量
    Constant(Box<Constant>),
}

struct Constant {
    ty: Ty,
    span: Span,
    user_ty: Option<Ty>,    // 用户标注
    kind: ConstantKind,
}

enum ConstantKind {
    Ty(ConstValue),         // 已求值的常量
    Unevaluated(UnevaluatedConst, Ty),   // v0.2: 待求值
}

enum ConstValue {
    Scalar(Scalar),
    Slice { data: Vec<u8>, start: usize, end: usize },
    ZeroSized,
}

enum Scalar {
    Int(i128),
    Uint(u128),
    Ptr(AllocId, Size),
}
```

---

## 7. Terminator

```rust
enum TerminatorKind {
    /// `goto target`
    Goto { target: BasicBlock },
    
    /// `if cond then target1 else target2`
    SwitchInt {
        discr: Operand,
        targets: SwitchTargets,
    },
    
    /// `f(a, b)` 调用函数
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlock>,    // None = diverges
        // v0.2: unwind: UnwindAction,
    },
    
    /// `return`
    Return,
    
    /// `unreachable`
    Unreachable,
    
    /// `drop _1` — 析构 place
    Drop {
        place: Place,
        target: BasicBlock,
        // v0.2: unwind: UnwindAction,
        replace: bool,    // v0.2: drop & replace
    },
    
    /// `FalseEdge { real_target, imaginary_target }` (v1.2 恢复：match guard CFG 精确建模)
    /// Rust RFC 1211 明确说明 FalseEdge 是 match guard lowering 的关键
    FalseEdge {
        real_target: BasicBlock,
        imaginary_target: BasicBlock,
    },
    
    /// `FalseUnwind { real_target, unwind }` (v0.2: unwind 时使用)
    FalseUnwind {
        real_target: BasicBlock,
        unwind: UnwindAction,
    },
    
    /// `UnwindResume` — 恢复 unwind（v1.2 占位，MVP 不实现 unwind，仅类型存在）
    UnwindResume,
    
    /// `UnwindTerminate` — 终止 unwind（v0.2）
    UnwindTerminate(UnwindTerminateReason),
    
    /// `Assert { cond, expected, msg, target }` — debug assertion
    Assert {
        cond: Operand,
        expected: bool,
        msg: AssertMessage,
        target: BasicBlock,
    },
}

enum UnwindTerminateReason {
    Abi,
    InCleanup,
}

struct SwitchTargets {
    values: Vec<u128>,        // discriminant values
    targets: Vec<BasicBlock>,
    otherwise: BasicBlock,    // 默认分支
}

enum AssertMessage {
    BoundsCheck { len: Operand, index: Operand },
    Overflow(BinOp, Operand, Operand),
    OverflowNeg(Operand),
    DivisionByZero(Operand),
    RemainderByZero(Operand),
    // v1.2.2 修正：与 rustc master 一致，ResumedAfter* 都需 CoroutineKind 参数
    ResumedAfterReturn(CoroutineKind),
    ResumedAfterPanic(CoroutineKind),
    ResumedAfterDeinit(CoroutineKind),
    ResumedAfterDrop(CoroutineKind),
    NullPointerDereference,                     // v1.2 新增（rustc PR #119620）
    NullReferenceConstructed,                   // v1.2 新增
    InvalidEnumConstruction(Operand),           // v1.2.2 修正：缺 Operand 参数
    MisalignedPointerDereference { required: Operand, found: Operand },
}

// v1.2.3 修正：CoroutineKind 与 rustc master 一致
// rustc master 实际为 Coroutine(Movability) / Desugared(CoroutineDesugaring, CoroutineSource) / CoroutineClosure
enum CoroutineKind {
    Coroutine(Movability),
    Desugared(CoroutineDesugaring, CoroutineSource),    // v1.2.3 修正：双参数与 rustc 一致
    CoroutineClosure,
}

enum CoroutineDesugaring {
    Async,
    AsyncGen,
    Gen,
}

enum CoroutineSource {
    Block,
    Closure,
    Fn,
}

// v1.2.2 修正：Movability::Dynamic → Movable（与 rustc master 一致）
enum Movability { Static, Movable }

enum UnwindAction {
    Continue,
    Unreachable,
    Terminate,
    Cleanup(BasicBlock),
}
```

MVP 不支持 unwind，所有 UnwindAction = `Unreachable`。v0.2 加 unwind。

---

## 8. MIR 构建算法

从 HIR 构建 MIR 的过程称为 **MIR building**。算法分两步：

### 8.1 第一步：CFG 框架

为每个 HIR 表达式分配 Local，并构建基本块框架：

```rust
struct Builder {
    cfg: CFG,
    local_decls: IndexVec<Local, LocalDecl>,
    source_scopes: IndexVec<SourceScope, SourceScopeData>,
    breakable_scopes: Vec<BreakableScope>,
    current_block: BasicBlock,
}

struct BreakableScope {
    region_scope: SourceScope,
    continue_target: Option<BasicBlock>,
    break_target: BasicBlock,
    break_value: Option<Place>,
}

impl Builder {
    fn expr(&mut self, expr: &hir::Expr) -> Place {
        match expr {
            hir::Expr::Lit(lit) => {
                let temp = self.new_temp(lit.ty);
                self.cfg.assign(self.current_block, temp, Rvalue::Use(...));
                temp
            }
            hir::Expr::BinOp(op, lhs, rhs) => {
                let lhs_place = self.expr(lhs);
                let rhs_place = self.expr(rhs);
                let temp = self.new_temp(result_ty);
                self.cfg.assign(
                    self.current_block,
                    temp,
                    Rvalue::BinaryOp(op, lhs_place, rhs_place),
                );
                temp
            }
            hir::Expr::If(cond, then, else_) => {
                let cond_place = self.expr(cond);
                let then_block = self.cfg.new_block();
                let else_block = self.cfg.new_block();
                let end_block = self.cfg.new_block();
                
                self.cfg.terminate(
                    self.current_block,
                    TerminatorKind::SwitchInt {
                        discr: Operand::Read(cond_place),
                        targets: SwitchTargets::new(
                            vec![1], vec![then_block], else_block,
                        ),
                    },
                );
                
                self.current_block = then_block;
                let then_value = self.expr(then);
                self.cfg.terminate_goto(self.current_block, end_block);
                
                self.current_block = else_block;
                let else_value = self.expr(else_);
                self.cfg.terminate_goto(self.current_block, end_block);
                
                self.current_block = end_block;
                // phi: end_block 的值来自 then_value 或 else_value
                let result = self.new_temp(result_ty);
                // ... 用 phi 模拟（或重写为两次赋值）
                result
            }
            // ... 其他表达式
        }
    }
}
```

### 8.2 第二步：Drop elaboration

为每个 Local 在 StorageDead 之前插入 `Drop` terminator（如果类型 impl Drop）：

```
原代码：
    StorageDead(_1)

drop elaboration 后：
    if _1 needs drop:
        Drop(_1, target)
    else:
        goto target
```

drop elaboration 还要处理：

- 函数提前 return 时的 drop（每个 return 路径插入 drop chain）
- panic 时的 drop（MVP abort，不做）
- partial move 后的 drop（moved 字段跳过 drop）

---

## 9. MIR 优化 pass

MVP 实现以下 pass（按顺序）：

### 9.1 必需 pass

1. **MIR building**：HIR → MIR
2. **Drop elaboration**：插入 Drop terminator
3. **Borrow check**：NLL + liveness + init analysis（详见 04 文档）
4. **Dead store elimination**：删除对永不读取的 Local 的赋值
5. **Const propagation**：编译期已知的常量传播
6. **Jump threading**：合并 goto 链

### 9.2 可选 pass（v0.2）

1. **Inline**：内联小函数
2. **Loop unrolling**：循环展开
3. **CSE**：公共子表达式消除
4. **SROA**：Scalar replacement of aggregates

MVP 不做 7-10，把优化交给 LLVM。

### 9.3 pass 顺序

```
HIR
 ↓
MIR build
 ↓
Drop elaboration
 ↓
Borrow check
 ↓
Dead store elimination
 ↓
Const propagation
 ↓
Jump threading
 ↓
LLVM IR codegen
```

每个 pass 输入输出都是 `Body`，纯函数式转换。

### 9.4 实现状态（Stage 18.96 接线）

**Stage 18.96**: MIR 优化正式接入 driver 流水线。

| Pass | 实现阶段 | 接线状态 |
|------|---------|---------|
| Dead store elimination (DCE) | Stage 17.10 | ✅ Stage 18.96 wired |
| Constant propagation + folding | Stage 17.13 | ✅ Stage 18.96 wired |
| Jump threading | — | ❌ 推迟到 v0.3 |

**接线位置**: `src/driver.rs::compile_inner()` 在 `writeback_closures` 之后、`mirs.push` 之前调用 `crate::mir::optimization::run_mir_optimizations(&mut mir)`。

**Orchestrator API**: `src/mir/optimization.rs::run_mir_optimizations(&mut MirBody)` 按 §9.3 顺序运行 DCE → const_prop → DCE（第二次 DCE 保证幂等性，清理 const_prop 暴露的新死代码）。

**测试入口**: `src/driver.rs::compile_no_opt(src)` 跳过 MIR opt，用于 IR/MIR 结构验证测试（per §11 接口隔离）。

**Stage 18.99 DCE 修复**: `collect_terminator_read_locals` 现在标记 `TerminatorKind::Return` 读取 `LocalId(0)`（返回值局部），防止 DCE 错误移除返回值赋值。

---

## 10. MIR 文本表示（debug 用）

为调试方便，MIR 可序列化为文本：

```
fn main() -> () {
    let mut _0: ();
    let mut _1: i32;
    let mut _2: bool;

    bb0: {
        StorageLive(_1);
        _1 = const 1_i32;
        StorageLive(_2);
        _2 = Lt(_1, const 10_i32);
        switchInt(_2) -> [true: bb1, false: bb2];
    }

    bb1: {
        _0 = _1;
        StorageDead(_2);
        StorageDead(_1);
        return;
    }

    bb2: {
        _0 = const -1_i32;
        StorageDead(_2);
        StorageDead(_1);
        return;
    }
}
```

环境变量 `LANDIN_MIR_DUMP=1` 触发 dump。

---

## 11. MIR 数据流分析框架

所有数据流分析共用一个 engine：

```rust
trait Analysis {
    type Domain: BitSetLike;
    
    const DIRECTION: Direction;        // Forward / Backward
    
    fn init(&self, body: &Body) -> Self::Domain;
    fn transfer(&self, state: &mut Self::Domain, stmt: &Statement);
    fn transfer_term(&self, state: &mut Self::Domain, term: &Terminator);
    fn meet(&self, lhs: &mut Self::Domain, rhs: &Self::Domain);
}

enum Direction {
    Forward,
    Backward,
}

fn run_analysis<A: Analysis>(body: &Body, analysis: A) -> Results<A> {
    let mut results = Results::new(body, analysis);
    let mut worklist = VecDeque::new();
    
    // 初始化所有基本块
    for bb in body.basic_blocks.indices() {
        worklist.push_back(bb);
    }
    
    // RPO 排序加速收敛
    let rpo = reverse_postorder(&body);
    
    while let Some(bb) = worklist.pop_front() {
        let mut state = analysis.init(body);
        
        // 根据 direction 决定 in/out
        match A::DIRECTION {
            Direction::Forward => {
                for pred in body.predecessors(bb) {
                    analysis.meet(&mut state, &results.out_state(pred));
                }
                for stmt in &body[bb].statements {
                    analysis.transfer(&mut state, stmt);
                }
                if let Some(term) = &body[bb].terminator {
                    analysis.transfer_term(&mut state, term);
                }
                if state != results.in_state(bb) {
                    results.set_in_state(bb, state.clone());
                    for succ in body.successors(bb) {
                        worklist.push_back(succ);
                    }
                }
            }
            Direction::Backward => {
                // 类似但反向
            }
        }
    }
    
    results
}
```

### 11.1 Liveness analysis

```rust
struct LivenessAnalysis {
    body: Body,
}

impl Analysis for LivenessAnalysis {
    type Domain = BitSet<Local>;
    const DIRECTION: Direction = Direction::Backward;
    
    fn transfer(&self, state: &mut Self::Domain, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                // kill: place 的 base local
                if let Place::Local(l) = place.base() {
                    state.remove(l);
                }
                // gen: rvalue 中读取的 local
                for l in rvalue.locals_read() {
                    state.insert(l);
                }
            }
            _ => {}
        }
    }
    
    fn meet(&self, lhs: &mut Self::Domain, rhs: &Self::Domain) {
        lhs.union_with(rhs);
    }
}
```

### 11.2 Maybe-initialized places

```rust
struct MaybeInitializedPlaces {
    body: Body,
}

// Domain: Place → InitState
// Forward analysis, May (union at meet)
```

### 11.3 Borrow analysis

```rust
struct BorrowAnalysis {
    body: Body,
    borrows: Vec<BorrowRecord>,
}

// Domain: BitSet<BorrowIndex>  — 活跃的借用集合
// Forward analysis, May (union)
```

---

## 12. MIR 与 rustc MIR 的差异（v1.2 修正）

| 维度 | rustc MIR | Landin MIR | 理由 |
| --- | --- | --- | --- |
| FalseEdge | 有 | **有**（v1.2 恢复） | match guard CFG 精确建模（RFC 1211 要求） |
| FalseUnwind | 有 | **有占位**（v0.2 启用） | unwind 时使用 |
| UnwindResume / UnwindTerminate | 有 | **有占位**（MVP 不实现 unwind） | 类型存在，运行时直接 abort |
| UnwindAction | 完整 | **Unreachable only** | MVP 不支持 unwind |
| Generator/Coroutine transform | 有 | **无** | MVP 无 async/generator（Coroutine variant 仅占位） |
| ConstantKind::Unevaluated | 有 | **无**（全部立即求值） | 简化 |
| CastKind | 11 种 | **7 种**（v1.2 修正） | 简化，v0.2 补 FloatToInt 等 |
| ProjectionElem | 8 种 | **6 种**（无 UnwrapUnsafeBinder, Opaque） | 简化 |
| StatementKind | 12 种 | **10 种**（v1.2.2 修正：删除 Deinit 后实际 10 个 active variant） | MVP 必需 |
| TerminatorKind | 15 种 | **11 种**（v1.2 补 UnwindResume/UnwindTerminate） | MVP 必需 |
| BorrowKind | 3 顶层 + 子 enum | **3 顶层 + MutBorrowKind**（v1.2 修正，废弃 Unique） | 与 rustc 当前实现一致 |
| CastKind 命名 | PointerExposeProvenance 等 | **一致**（v1.2 修正） | rustc PR #2944 命名 |
| AssertMessage | 13 种 | **13 种**（v1.2.3 修正：含 ResumedAfterReturn/Panic/Deinit/Drop（4）+ NullPointerDereference + NullReferenceConstructed + InvalidEnumConstruction(Operand)（3）+ MisalignedPointerDereference + BoundsCheck + Overflow + OverflowNeg + DivisionByZero + RemainderByZero（6）= 13） | 与 rustc master 一致 |
| AggregateKind | 含 Coroutine/CoroutineClosure | **含 Coroutine/CoroutineClosure**（v1.2 修正：Generator → Coroutine） | rustc PR #105832 |
| Rvalue::AddressOf | 已删除（改 RawPtr） | **RawPtr(RawPtrKind, Place)**（v1.2 修正） | 与 rustc 一致 |
| Rvalue::NullaryOp | 已删除（改 intrinsic） | **删除**（v1.2 修正） | rustc 已移除，SizeOf/AlignOf 改 intrinsic |
| Operand | 4 种 | **3 种**（无 RuntimeChecks） | RuntimeChecks 推 v0.2 |
| `AdtLayout` side-table | 无（rustc 用 `Ty::Adt(AdtDef, Substs)` 内嵌） | **有**（`Body::adt_layouts: HashMap<DefId, AdtLayout>`，Stage 3.47 新增） | Landin 的 `TyKind::Adt(DefId, Vec<Ty>)` 不携带 layout，故 side-table 下沉；与 rustc `AdtDef` 等价但解耦 |

---

## 12.1 AdtLayout 设计说明（Stage 3.47 新增）

Landin 的 `TyKind::Adt(DefId, Substs)` 在 Stage 3.30 引入时不携带
storage layout 信息，导致 codegen 必须回查 HIR 才能解析 struct/enum
的 LLVM 存储类型——形成 L-PIPE-1 管道耦合债（持续 14 轮审查至
Stage 3.47 闭合）。

Stage 3.47 引入 `AdtLayout` side-table（`Body::adt_layouts`），由
MIR lower 在 `lower_hir_body_to_mir_full` 末尾通过 `populate_adt_layouts`
后处理一次性下沉。Codegen 通过 `mir_type_to_emit_type_with_layouts(ty,
&mir.adt_layouts)` 解析 `TyKind::Adt(def_id, _)`，**不再读 HIR**。

设计选择（Option B 而非 Option A）的理由见
`docs/develop/v0/stage-3/gate-review-round14.md` §3 — 简言之，Option A
（修改 `TyKind::Adt` 加 `Rc<AdtLayout>` 字段）会触及 typeck/borrowck
≥10 个 pattern-match 点，超出 §16.5.1 ≤3 文件 in-stage-fix 阈值；
Option B（side-table）只触 3 文件且 `Ty` 类型签名不变，零 typeck/
borrowck 影响。

`AdtLayout::Enum { variant_payloads: Vec<Vec<Ty>> }` 故意存储**全部**
variant 的 payload（而非仅第一个非空），为 Stage 4 的 L-ENUM-UNION
修复做前向兼容——届时 codegen 只需把 `first non-empty payload` 改为
`union of all payloads`，MIR 数据结构无需变动（per §15.2.1 消除根因）。

---

## 12.2 Enum 存储布局：扁平化设计（Stage 3.48 新增）

Stage 3.48 闭合了 L-ENUM-UNION（enum 存储布局 soundness bug）和
L-ENUM-BINDING（pattern 绑定不提取 payload 的隐藏 P0 bug）。

### 扁平化布局规则

`mir_type_to_emit_type_with_layouts` 对 `AdtLayout::Enum` 的处理改为
**扁平化所有非空 variant 的 payload 字段**到存储结构：

```
Storage = { discriminant, variant_0_field_0, ..., variant_0_field_N-1,
                         variant_1_field_0, ..., variant_1_field_M-1,
                         ... }
```

其中 unit variant 不贡献任何字段。具体分三种情况：

| Case | 描述 | 存储布局 | 示例 |
|------|------|---------|------|
| A | 所有 variant 都是 unit | `{ discr }` | `enum Color { Red, Green, Blue }` → `{ i32 }` |
| B | 恰好一个非空 variant | `{ discr, payload_fields... }` | `enum Opt { None, Some(i32) }` → `{ i32, i32 }` |
| C | ≥2 个非空 variant | `{ discr, v0_fields..., v1_fields..., ... }` | `enum E { A, B(i32), C(i64) }` → `{ i32, i32, i64 }` |

Case A/B 的布局与 Stage 3.38-3.47 完全一致（无回归）。Case C 是
Stage 3.48 的 soundness 修复——之前 `{ i32, i32 }` 只为 B 的 i32
分配空间，写入 C 的 i64 会越界破坏相邻栈内存。

### variant_idx → field_idx 映射

扁平化布局下，`variant_idx` 不直接对应 `field_idx`。映射公式：

```
field_idx(variant_V, field_F) = 1 + sum(field_counts of variants 0..V-1) + F
```

其中 `1` 是 discriminant 占用的 field 0。MIR lower 的
`compute_enum_payload_starting_idx` 函数从 HIR 计算这个偏移（per §16.2.1
数据下行允许），生成 `ProjectionElem::Field(field_idx, field_ty)` 投影。
Codegen 直接读 MIR 投影，不查 HIR。

### Pattern 绑定提取（L-ENUM-BINDING 修复）

Stage 3.40 引入 enum match 时，`collect_pat_bindings_for_mir` 只为
`Ident` 子模式分配 local，但**未生成任何投影**把 enum 的 payload
赋给 binding local——导致 `Opt::Some(x) => x` 的 `x` 读取未初始化内存
（隐藏 P0 soundness bug，持续 5 轮审查未被发现，因为现有测试只断言
`switch i32` 存在而未验证 binding 实际接收 payload）。

Stage 3.48 新增 `lower_enum_variant_pattern_bindings` 函数，对
`TupleStruct`/`Struct` 模式：

1. 通过 `resolve_enum_variant` 从 HIR 解析 variant_idx 和 field_tys
2. 通过 `compute_enum_payload_starting_idx` 计算扁平 field_idx
3. 为每个 `Ident` 子模式生成 `binding = Copy(scrut.Field(field_idx, field_ty))`

该函数在 match arm lowering 的两个路径（arm block 和 otherwise block）
都调用，与 `collect_pat_bindings_for_mir` 并列。

### 设计选择：扁平布局 vs rustc-style union struct

| 方案 | 优点 | 缺点 | 选择 |
|------|------|------|------|
| **扁平布局**（已选） | 所有投影单层级 `Field(N, ty) on Local`，与 Case A/B 一致，codegen 无需重构 | 多 variant 时存储略浪费（每个 variant 的 slot 按自身大小，非 max） | ✅ Stage 3.48 |
| 最大宽度整数 | 简单 | 仅适用同类型整数 payload，混合类型（i32 + f64）失效 | ❌ |
| 字节数组 `[N x i8]` | 通用 | 丢失类型信息，所有访问需 bitcast | ❌ |
| rustc-style union struct（per-variant slot） | 类型精确 | 需要嵌套 `Field(Field(Local))` 投影，codegen 当前不支持嵌套 Field（会把中间值当指针 GEP） | ❌（需 codegen 重构） |

扁平布局的取舍：用少量存储浪费换取 codegen 的简洁性和与 Case A/B 的
一致性。未来若需精确布局（如对齐优化），可在 Stage 4+ 引入 rustc-style
union struct 并重构 codegen 的嵌套 Field 支持。

---

## 12.3 嵌套 Adt 递归（Stage 3.48 修复）

Stage 3.48 还修复了 `mir_type_to_emit_type_with_layouts` 的一个
**遗留 bug**：对 `Tuple/Array/Ref/RawPtr/Slice` 类型，原本 fall through
到 `mir_type_to_emit_type`（不带 layouts），导致嵌套 Adt（如 `(E, i32)`
或 `&MyStruct`）会折叠为 I32。

修复后，所有容器类型都递归调用 `_with_layouts`，确保嵌套 Adt 的
storage layout 正确解析。该 bug 由 R15 audit 的 `e07_enum_in_tuple`
case 暴露。

---

## 13. MIR 大小估算

对一个典型函数（~50 行 HIR）：

- BasicBlocks: 10-30
- Statements: 30-100
- Locals: 10-30
- 平均每个 statement 约 60 字节

→ 整个 MIR 约 5-15 KB/函数。

一个 30k 行的 Landin 程序约 1000 函数 → 5-15 MB MIR 内存占用，可接受。

---

## 14. 实现状态（v0.13.0，§25.8 回写）

> 本节由 Stage 6.11 依据流程 v3.21 §25.8 阶段末尾设计回写协议生成。
> 仅记录"设计 + 理由"，实现细节归 `docs/develop/v0/stage-N/dev-log.md`。

### 14.1 §2 顶层结构 — 偏差清单

| 设计字段 | 实现状态 | 偏差类型 | 说明 |
|---------|---------|---------|------|
| `basic_blocks: IndexVec<BasicBlock, BasicBlockData>` | ✅ 实现 | — | 实现用 `Vec<BasicBlock>`，等价 |
| `locals: IndexVec<Local, LocalDecl>` | ✅ 实现 | — | 实现用 `Vec<LocalDecl>` + `LocalId(u32)`，等价 |
| `source_scopes: IndexVec<SourceScope, SourceScopeData>` | ❌ 未实现 | B1 | 实现用 `LocalDecl.source_info: Span` 简化，无独立 scope 树。v0.2 + drop elaboration 阶段补 |
| `arg_count: usize` | ❌ 未实现 | B1 | 实现通过 `LocalDecl` 的隐式约定（params 是 local 1..N）表达，无显式字段。可接受简化 |
| `spread_arg: Option<Local>` | ❌ 未实现 | B1 | v0.2 外部 ABI 阶段补，MVP 不需要 |
| `span: Span` | ✅ 实现 | — | — |
| `adt_layouts: HashMap<DefId, AdtLayout>` | ✅ 实现 | — | Stage 3.47 L-PIPE-1 闭合 |
| `BasicBlockData.is_cleanup: bool` | ❌ 未实现 | B1 | v0.2 unwind 阶段补，MVP 直接 abort |
| `BasicBlockData.terminator: Option<Terminator>` | ✅ 实现（非 Option） | B3（实现更优） | 实现用 `Terminator::Unreachable` 作为默认值，避免 Option unwrap。等价且更易用 |
| `LocalDecl.is_temp: bool` | ❌ 未实现 | B1 | 实现通过命名约定（无名 local = temp）表达，可接受简化 |
| `LocalDecl.is_arg: bool` | ❌ 未实现 | B1 | 实现通过 `LocalId` 范围约定（1..arg_count 是 args）表达，可接受简化 |

### 14.2 §8 MIR 构建算法 — 实现扩展（B4 补写）

设计文档 §8 描述了"CFG 框架 + Drop elaboration"两步构建算法，但未描述
Stage 5.78-5.80 新增的 **dyn Trait lowering** 子算法。本节补写：

#### 14.2.1 dyn Trait lowering 算法（Stage 5.78-5.80 新增）

**触发条件**：HIR 中存在 `receiver.method(args)` 表达式，且 receiver 类型
为 `dyn Trait`。

**算法步骤**：

1. **driver 阶段**：driver 调用 `build_dyn_trait_mir_plan_from_resolver`
   构造 `DynTraitMIRPlan`，包含每个 dyn Trait 方法的 `(trait, type, method,
   slot_index, param_count, return_kind, param_kinds)` 七元组。
2. **MIR lower 入口**：`lower_hir_body_to_mir_full_with_dyn_trait_plan`
   接收 plan，存入 `MirLowerCtxt.dyn_trait_plan`。
3. **MethodCall 降低**：`lower_expr_to_operand` 在 `HirExprKind::MethodCall`
   分支查询 plan，匹配到则调用 `build_dyn_trait_call_terminator`：
   - 构造 `Terminator::Call`，其 `func` 是 `Operand::Constant(Const { ty: Error,
     val: Int(index) })` —— `index` 是 side-table 索引（marker）。
   - 将 `(trait, type, method, slot_index, param_count)` 推入
     `MirBody.dyn_trait_calls` side-table。
4. **codegen 消费**：codegen 在 `Terminator::Call` 分支检测 marker
   （func 是 `Const { ty: Error, val: Int(_) }`），读取 side-table 对应条目，
   发出 vtable indirect call（`load vtable slot → indirect call`）。

**§16 合规性**：MIR 携带 dyn Trait 调用信息作为数据（side-table），
codegen 不查 HIR / TraitResolver。数据流单向：driver → MIR lower →
MirBody side-table → codegen。

**设计理由**：参考 rustc 的 `TerminatorKind::Call` 的 `fn_span` +
`call_source` 设计——把调用元信息作为数据 sunk 到 MIR，避免下游回查。
Landin 用 side-table + marker const 的方式实现等价语义，更适合当前
MIR 结构（无 Call 上的扩展字段）。

### 14.3 偏差处理计划

| 偏差 | 处理时机 | 理由 |
|------|---------|------|
| B1（`source_scopes` / `is_cleanup` / `is_temp` / `is_arg` / `spread_arg`） | v0.2 unwind 阶段 | 与 unwind ABI 强相关，提前实现无意义 |
| B3（`Option<Terminator>` → `Terminator::Unreachable`） | 接受为永久偏差 | 实现更优，无需重构 |
| B4（dyn Trait lowering 算法补写） | 已在 §14.2 补写 | — |

---

## 15. Stage 12.4 §25.8 追溯回写（v0.21.2，r217 二次审查）

> 本节由 Stage 12.4（r217 二次审查）依据流程 v3.21 §25.8 追溯回写协议生成。
> Stage 5 (99 子阶段) 在 v3.20 流程下执行，未做 §25.8 设计回写；本节补做。
> 审计来源: `docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md` §2.4

### 15.1 `DynTraitMIRSummary` — dyn Trait MIR 4 层架构补写（B4 设计灰区）

**实现来源**: Stage 5.71 (TD-018 准备阶段)
**代码位置**: `src/mir/dyn_trait.rs::DynTraitMIRSummary`

**4 层架构完整定义**:

| 层级 | 类型 | 引入阶段 | 职责 |
|------|------|---------|------|
| 1 | `DynTraitFatPtr` | Stage 5.61 | 胖指针表示（data ptr + vtable ptr） |
| 2 | `DynTraitMethodCall` | Stage 5.65 | 单个 dyn Trait 方法调用点 |
| 3 | **`DynTraitMIRSummary`** | Stage 5.71 | **汇总一个 MIR body 内所有 dyn Trait 调用** |
| 4 | `DynTraitMIRPlan` | Stage 5.75 | 完整发射计划（包含 fat ptrs + method calls + summary） |

**设计意图**: 4 层架构遵循"summary → plan"两阶段模式（与 rustc `MonoItem` → `MonoItemData` 类似）：
- Layer 1-2 是数据点（per-call-site）
- Layer 3 是汇总（per-body）
- Layer 4 是发射计划（per-crate，driver 持有）

**当前文档状态**: §14 实现状态章节列出 `DynTraitFatPtr` 和 `DynTraitMethodCall` 但未列 `DynTraitMIRSummary`。本回写补全。

**回写动作**: §14 实现状态表新增一行：
| DynTraitMIRSummary | ✅ 实现 (5.71) | `mir/dyn_trait.rs` per-body 汇总 |

### 15.2 设计偏差状态（截至 v0.21.2）

| 偏差 | 类型 | 状态 | 计划 |
|------|------|------|------|
| `source_scopes` / `is_cleanup` 等字段 | B1 | ❌ 未实现 | v0.2 unwind 阶段 |
| `Option<Terminator>` → `Terminator::Unreachable` | B3（已接受） | — | 永久偏差 |
| dyn Trait lowering 算法 | B4 | ✅ 已回写 | §14.2 |
| `DynTraitMIRSummary` 4 层架构补写 | B4 | ✅ 本节回写 | §15.1 |

---

**下一文档**: [`07-codegen.md`](./07-codegen.md) — LLVM IR 生成

---

## 16. MIR Intrinsic Ops 设计 (v0.2 Phase 2, Stage 18.225 新增)

> 本节由 Stage 18.225 依据流程 v6.4 §13.1 (设计对齐) + §17.6 (缺陷纳入) 生成。
> 设计目标: 替换 4 个复合 C runtime helpers 为 MIR-level intrinsic ops,
> 为 v0.3 自举做准备 (TD-C-WRAPPER-OVERUSE)。

### 16.1 设计动机

当前 4 个复合操作通过 C runtime helpers 实现:
- `__landin_vec_push(vec_ptr, val_ptr, elem_size)` — Vec 增长 + 存储 + len++
- `__landin_vec_get(vec_ptr, index, out_ptr, elem_size)` — 边界检查 + 元素加载
- `__landin_string_push_str(str_ptr, src_ptr, src_len)` — String 增长 + 字节拷贝
- `__landin_format_variadic(...)` — 格式化字符串构造 (va_list)

这些 C helpers 违反:
- §11 接口隔离: codegen 直接依赖 C runtime 内部细节
- §1.3 拒绝特判: Vec::push 变成编译器特判类型
- §12 最优 > 最小: C helper 是 MVP 简化, 最优方案是 MIR-level intrinsic

### 16.2 新增 MIR Rvalue 变体

```rust
pub enum Rvalue {
    // ... existing variants ...

    /// Stage 18.225: Load value from raw pointer.
    /// `*ptr` → load the value pointed to by `ptr`.
    /// Codegen: `LLVMBuildLoad2`.
    /// Per §1.0 原則 6 (通解>特例): one Load for all pointer types.
    Load(Operand /* ptr */, Ty /* pointee type */),

    /// Stage 18.225: Get element pointer (GEP).
    /// `&base[offset]` → compute address of element at offset.
    /// Codegen: `LLVMBuildGEP2` / `LLVMBuildStructGEP2`.
    /// Per §1.0 原則 6 (通解>特例): one GEP for all indexing.
    GetElementPtr {
        base: Operand,       // pointer to struct/array
        indices: Vec<Operand>, // field index or array index
        result_ty: Ty,      // pointer type
    },
}
```

### 16.3 新增 Statement 变体

```rust
pub enum StatementKind {
    // ... existing variants ...

    /// Stage 18.225: Store value to raw pointer.
    /// `*ptr = val` → store value at pointer address.
    /// Per §1.0 原則 6 (通解>特例): one Store for all pointer types.
    Store {
        ptr: Place,     // pointer to store to
        val: Operand,   // value to store
        val_ty: Ty,     // value type (for codegen)
    },
}
```

### 16.4 迁移计划

| C Helper | MIR Intrinsic 替换 | 复杂度 |
|----------|-------------------|--------|
| `__landin_vec_get` | Load + GetElementPtr + BinaryOp(icmp) + SwitchInt(bounds) | Low |
| `__landin_vec_push` | Load(len) + BinaryOp(icmp len>=cap) + Alloc(realloc) + Store(val) + Store(len+1) | Medium |
| `__landin_string_push_str` | Load(len/cap) + BinaryOp(icmp) + Alloc(realloc) + memcpy loop + Store(new_len) | Medium |
| `__landin_format_variadic` | MIR-level format string walker + per-arg type dispatch | High |

### 16.5 保留的原语 C Helpers

以下 C helpers 是设计文档明确允许的, **不在迁移范围内**:
- `__landin_alloc(size)` — libc malloc wrapper (07-codegen.md §5.2)
- `__landin_dealloc(ptr)` — libc free wrapper (07-codegen.md §5.2)
- `__landin_realloc(ptr, old, new)` — libc realloc wrapper (07-codegen.md §5)
- `__landin_memcpy(dst, src, n)` — libc memcpy wrapper
- `__landin_panic_*()` — abort runtime (07-codegen.md §4)
- `__landin_oom_abort()` — OOM abort (07-codegen.md §5.2)

Per §13.2: 这些原语在 v0.3 自举后将变成 Landin stdlib 的 `extern "C"` 声明。

### 16.6 实现优先级

```
v0.2.5a: 设计文档 (本节) ← Stage 18.225 done
v0.2.5b: 添加 Rvalue::Load, Rvalue::GetElementPtr, StatementKind::Store ← Stage 18.226 done
v0.2.5c: codegen 支持 (LLVMBuildLoad2, LLVMBuildGEP2, LLVMBuildStore) ← Stage 18.227 done
v0.2.5d: 迁移 __landin_vec_get → MIR intrinsic (最简单, 验证设计) ← Stage 18.228 done
v0.2.5e: 迁移 __landin_vec_push → MIR intrinsic ← Stage 18.229 done
v0.2.5f: 迁移 __landin_string_push_str → MIR intrinsic ← Stage 18.230 done
v0.2.5g: 迁移 __landin_format_variadic → MIR intrinsic (最复杂) ← Stage 18.231 (next)
```

#### 16.6.4 Stage 18.230 实现详情 (v0.2.5f `__landin_string_push_str` migration)

**Migration**: `lower_string_push_str_intrinsic` rewritten from C helper Call → MIR intrinsic sequence
with **while loop** for growth calculation (first MIR loop in an intrinsic).

**MIR Sequence** (10 basic blocks):
1. bb0: Extract str fields + src fields; compute new_len; need_grow = (new_len > cap); SwitchInt
2. grow_init_bb: is_zero = (cap == 0); SwitchInt
3. zero_cap_bb: new_cap = 4; goto grow_loop_bb
4. nonzero_cap_bb: new_cap = cap; goto grow_loop_bb
5. grow_loop_bb: cond = (new_cap < new_len); SwitchInt ← **BACK-EDGE TARGET**
6. grow_body_bb: new_cap = new_cap + new_cap (2x); goto grow_loop_bb ← **BACK-EDGE**
7. alloc_bb: Call `__landin_realloc`; Store to str.ptr + str.cap; goto copy_bb
8. copy_bb: reload str.ptr; GEP(dest, len); Call `__landin_memcpy`; Store str.len

**Key difference from vec_push (Stage 18.229)**:
- **While loop for growth**: vec_push uses `new_cap = cap * 2` (single doubling).
  string_push_str uses `while (new_cap < new_len) new_cap *= 2` — grows new_cap until
  it exceeds new_len. This handles cases where src_len >> cap (e.g., appending a 43-byte
  string to an empty String → new_cap goes 4 → 8 → 16 → 32 → 64).
- **MIR back-edge**: First intrinsic to generate a loop (grow_loop_bb ↔ grow_body_bb).
  All previous intrinsics used straight-line code or simple if/else branching.
- **`__landin_memcpy` for byte copy**: Reuses the primitive C helper (per §16.5, not
  in migration scope). The C helper's byte-by-byte loop is replaced by a single Call.

**No new bugs discovered**: All infrastructure fixes from Stages 18.228-18.229 (DCE,
borrowck Store, Store Deref codegen, push_statement API, Mutable PHI-like locals)
applied directly without modification.

**MVP scope (§17.6 record)**:
- **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
- **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
- **PHI avoidance**: Reload `str.ptr` in copy_bb via `Projection(recv, Field(0))`.
- **memcpy via C helper**: `__landin_memcpy` is a primitive C helper (per §16.5).

**Test verification**:
- 6 regression tests pass: `stage18_198_push_str_append/from_empty/multiple/growth/empty_src/long`
- Growth test verifies cap=16 (correct while-loop growth: 4→8→16)
- Full suite: 3783 tests, 0 failures

#### 16.6.3 Stage 18.229 实现详情 (v0.2.5e `__landin_vec_push` migration)

**Migration**: `lower_vec_push_intrinsic` rewritten from C helper Call → MIR intrinsic sequence
with conditional growth logic.

**MIR Sequence** (8 basic blocks):
1. bb0: Extract `vec.ptr` (Field 0), `vec.len` (Field 1), `vec.cap` (Field 2); compute `need_grow = len >= cap`; SwitchInt
2. grow_bb: Compute `is_zero = (cap == 0)`; SwitchInt
3. zero_cap_bb: `new_cap = 4` (initial capacity); goto alloc_bb
4. nonzero_cap_bb: `new_cap = cap + cap` (2x growth); goto alloc_bb
5. alloc_bb: `new_bytes = new_cap * elem_size`; Call `__landin_realloc`; Store to `vec.ptr` + `vec.cap`; goto store_bb
6. store_bb: Reload `vec.ptr`; `elem_ptr = GetElementPtr(current_ptr, [len])`; Store val through `*elem_ptr`; `new_len = len + 1`; Store to `vec.len`

**Critical Fixes Discovered During Migration** (per §17.6 同类型整体修复):
- **Borrowck StatementKind::Store**: `check_statement` didn't handle `StatementKind::Store` —
  Store writes to `ptr` (a Place) bypassed borrowck's mutability/borrow checks. Fixed: Store
  now calls `check_place_write` + `check_operand` (same as Assign).
- **Borrowck mutability for PHI-like locals**: `new_cap_local` is assigned in both
  `zero_cap_bb` (= 4) and `nonzero_cap_bb` (= cap * 2). The borrowck's `initialized` set
  is cumulative across blocks — without `Mutability::Mutable`, it flags the second assignment
  as "assign twice to immutable". Fixed: use `new_local_with_mut(..., Mutable)` for PHI-like
  locals (same pattern as if/else result locals in control_flow.rs:31).
- **StatementKind::Store Deref codegen**: `compute_place_address` doesn't have a `Deref` arm —
  it falls through to `codegen_place_load_typed` which loads the VALUE (not the address).
  This caused "Invalid bitcast i32 to ptr" errors when storing through `*elem_ptr = val`.
  Fixed: StatementKind::Store codegen now handles `Projection(base, Deref)` specially —
  loads the POINTER from base, then stores through it (mirrors Assign's Deref handling,
  Stage 14.27).
- **MirLowerCtxt.push_statement**: Added a new API to push arbitrary `StatementKind` onto
  the current block (used by `lower_vec_push_intrinsic` to emit `StatementKind::Store`).

**MVP scope (§17.6 record)**:
- **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
  When `cap == 0`, `vec.ptr` is NULL, so `__landin_realloc(NULL, 0, new_bytes)` is
  equivalent to `malloc(new_bytes)`. One Call path instead of two.
- **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
- **PHI avoidance**: Reload `vec.ptr` in store_bb via `Projection(recv, Field(0))`.
  Handles both growth (field updated) and no-growth (field unchanged) cases.

**Test verification**:
- 6 regression tests pass: `stage18_197_vec_push_single/multiple/growth/i64/u8/large_growth`
- 4 roundtrip tests pass: `stage18_203_vec_i32/i64/i8/u32_roundtrip`
- Full suite: 3783 tests, 0 failures

#### 16.6.2 Stage 18.228 实现详情 (v0.2.5d `__landin_vec_get` migration)

**Migration**: `lower_vec_get_intrinsic` rewritten from C helper Call → MIR intrinsic sequence.

**MIR Sequence**:
1. `data_ptr = Use(Copy(Projection(recv, Field(0, *mut T))))` — extract `vec.ptr`
2. `len = Use(Copy(Projection(recv, Field(1, i64))))` — extract `vec.len`
3. `idx_i64 = Cast(Numeric, idx, i64)` — cast index
4. `cond = BinaryOp(Lt, idx_i64, len)` — bounds check
5. `Assert(cond, expected=true, target=ok_bb, msg=BoundsCheck)` — panic on OOB
6. ok_bb: `elem_ptr = GetElementPtr(data_ptr, [idx_i64], *mut T)` — compute element address
7. `dest = Load(elem_ptr, T)` — typed load (no memcpy)

**Critical Fixes Discovered During Migration** (per §17.6 同类型整体修复):
- **DCE bug**: `collect_rvalue_locals` and `collect_terminator_read_locals` didn't handle
  `Rvalue::Load`, `Rvalue::GetElementPtr`, `StatementKind::Store`, and `TerminatorKind::Assert`.
  This caused DCE to remove assignments that ARE used, producing uninitialized memory reads.
  Fixed: all 4 variants now correctly collect their operand reads.
- **Borrowck bug**: `rvalue_reads` and `check_rvalue` in borrowck didn't handle Load/GEP.
  Fixed: both now correctly check operands.
- **LLVM emit_call type coercion**: `interpret_adhoc("0")` creates i32 constants, but
  `__landin_panic_bounds_check` expects i64 args. Fixed: `emit_call` now coerces integer
  arg values to match declared types via `LLVMBuildIntCast2`.
- **GEP element type**: codegen passed `EmitType::I32` for all GEP operations, causing
  `Vec<i64>::get(1)` to compute 4-byte offsets instead of 8-byte. Fixed: derive element
  type from `result_ty`'s pointee.

**MVP scope (§17.6 record)**: Only checks `idx < len` (upper bound). The `idx < 0` check
is deferred — Landin's `Vec::get` index is `usize`-like in idiomatic usage. Safe because
the existing test `stage18_200_vec_get_oob_panics` only tests upper-bound OOB.

**Test verification**:
- 4 regression tests pass: `stage18_200_vec_get_first/all/after_growth/oob_panics`
- 2 type tests pass: `stage18_203_vec_i64_roundtrip`, `stage18_208_vec_i64_get/struct_multiple`
- 2 DCE tests updated (plain assignments instead of arithmetic to avoid Assert interference)
- Full suite: 3783 tests, 0 failures

#### 16.6.1 Stage 18.227 实现详情 (v0.2.5c codegen)

| 变体 | codegen 路径 | 验证 |
|------|-------------|------|
| `Rvalue::Load(ptr_op, pointee_ty)` | `codegen_operand(ptr_op)` → `mir_type_to_emit_type_with_layouts_and_mono(pointee_ty)` → `MemoryEmitter::emit_load(pointee_emit_ty, ptr_val)` | `load i32,` / `load i64,` text-IR verification |
| `Rvalue::GetElementPtr { base, indices, result_ty }` | `codegen_operand(base)` → for each `idx_op`: `codegen_operand(idx_op)` + `MemoryEmitter::emit_gep_index_ptr(cur_ptr, I32, idx_val)` | `getelementptr inbounds` + chained indices verification |
| `StatementKind::Store { ptr, val, val_ty }` | `compute_place_address(ptr)` → `codegen_operand(val)` + `mir_type_to_emit_type_with_layouts_and_mono(val_ty)` → `MemoryEmitter::emit_store(val_emit_ty, val, ptr_addr)` | `store i32` / `store i64` text-IR verification |

**测试覆盖 (per §9.4.3 1:3+ 正负比例)**:
- 11 lib unit tests in `src/codegen/rvalue.rs::intrinsic_ops_tests`
- 正向: i32/i64 Load/GEP/Store text-IR verification (5 tests)
- 负向: void Load returns `CodegenError` (1 test)
- 集成: GEP→Load chain (mirrors `__landin_vec_get` migration target shape)
- 回归: Stage 18.226 data structures still construct (1 test)
- 锚定: `mir_type_to_emit_type_with_layouts_and_mono` resolves i32 pointee (1 test)

**MVP scope (§17.6 record)**:
- `result_ty` field is currently unused at codegen time because LLVM 19 opaque
  pointers (`ptr`) carry no element type. The element type passed to
  `emit_gep_index_ptr` is `EmitType::I32` — a placeholder that works for
  opaque-ptr LLVM 19 because the GEP instruction's source type is encoded
  separately. If the v0.2.5d migration reveals a need for typed GEP, this
  stub will be extended with proper element-type derivation — recorded as a
  tracked MVP, not a silent defect.

### 16.7 设计原则

- §1.0 原則 6 (通解>特例): Load/Store/GEP 是通用操作, 不针对特定类型
- §11 接口隔离: MIR intrinsic 在 MIR 层定义, codegen 只翻译 MIR
- §12 最优 > 最小: 用 MIR ops 替换 C helpers, 不是 hack
- §10 DRY: Load/Store/GEP 复用现有 codegen 基础设施 (emit_load/emit_store/emit_gep_field)
- §1.0 原則 4 (报错>静默): 边界检查在 MIR 层通过 SwitchInt 实现, 不在 C 层
