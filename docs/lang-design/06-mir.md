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

## 13. MIR 大小估算

对一个典型函数（~50 行 HIR）：

- BasicBlocks: 10-30
- Statements: 30-100
- Locals: 10-30
- 平均每个 statement 约 60 字节

→ 整个 MIR 约 5-15 KB/函数。

一个 30k 行的 Landin 程序约 1000 函数 → 5-15 MB MIR 内存占用，可接受。

---

**下一文档**: [`07-codegen.md`](./07-codegen.md) — LLVM IR 生成
