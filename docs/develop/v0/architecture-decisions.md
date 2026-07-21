# Architecture Decision Records (ADR)

> **创建日期**: 2026-07-22 (Stage 4.11)
> **目的**: 记录 Landin 编译器的关键架构决策，使新 Agent 能仅凭文档理解设计理由。
> **来源**: 深度审查 R37 D7（文档与知识传承）条件项

---

## ADR-001: HirParam 重复设计

**日期**: Stage 1.1 (v0.2.0)
**状态**: ✅ Accepted
**决策者**: Stage Committee (5/5 APPROVED)

### 背景

`HirFnSig.inputs: Vec<HirParam>` 和 `Body.params: Vec<HirParam>` 都携带相同的
参数数据（通过 clone 复制）。这看起来是冗余的。

### 决策

**接受重复**，不改为引用共享。

### 理由

1. 与 rustc 设计一致——rustc 也维护声明参数和实现参数的分离
2. `HirFnSig` 是签名声明（可能在没有 body 的情况下存在，如 trait fn 声明）
3. `Body.params` 是实现参数（携带运行时信息）
4. clone 成本低（HirParam 是小结构体）
5. 改为引用共享会增加生命周期复杂度，影响 MIR lower 和 typeck

### 深度审查结论 (R37)

Stage 3.65 深度审查接受此为设计选择，非缺陷。

---

## ADR-002: Emitter Trait 36 方法

**日期**: Stage 3.4 (v0.6.0) → Stage 3.59 (v0.8.6) 记录
**状态**: ✅ Accepted (decompose when 2nd backend added)
**决策者**: ARCH-A

### 背景

`Emitter` trait 有 36 个方法，只有 1 个实现（`TextEmitter`）。这看起来是过度设计。

### 决策

**保留当前设计**，在添加第二个后端时分解为子 trait。

### 理由

1. 单实现的 trait 不影响性能或正确性
2. 分解需要修改 36 个方法的签名，风险高
3. 当添加 MLIR/LLVM-C 后端时，分解为 `EmitterArith`/`EmitterMemory`/`EmitterCf` 等
4. 当前 `TextEmitter` 实现完整且经过 30+ 轮 gate review 验证

### 偿还计划

Stage 4+ 添加第二后端时分解。

---

## ADR-003: L1 PHI 优化 — 依赖 LLVM mem2reg

**日期**: Stage 4.2 (v0.9.0)
**状态**: ✅ CLOSED (design decision)
**决策者**: ARCH-A + deep review R37

### 背景

Codegen 为所有 local 生成 `alloca` + `load` + `store`，依赖 LLVM `mem2reg`
优化 pass 产生 SSA form（PHI 节点）。这看起来是 IR 质量限制。

### 决策

**不是限制——是标准设计**。L1 标记为 CLOSED。

### 理由

1. Clang、rustc 和大多数 LLVM 前端都使用此方法
2. `mem2reg` 是经过充分测试的 LLVM pass，产生最优 SSA form
3. 手动实现 PHI 发射会重复 `mem2reg` 逻辑（高工作量、高风险、低收益）
4. `alloca`-based IR 是正确的——任何 LLVM 工具链都能优化
5. `opt -mem2reg` 或 `lli`（运行默认 pass）产生最优代码

### 结论

L1 是设计决策，不是待修复限制。`alloca`-based IR 是预期设计。

---

## ADR-004: 可见性强制检查 — Same-Crate 访问

**日期**: Stage 4.3 (v0.9.1)
**状态**: ✅ Accepted (full enforcement deferred)
**决策者**: ARCH-A

### 背景

`check_visibility` 实现了可见性检查基础设施，但当前所有 same-crate 访问都允许。
完整 `pub(crate)`/`pub(super)`/private 强制需要 `current_module` 跟踪。

### 决策

**分阶段实现**：Stage 4.3 实现基础设施，完整强制推迟到有嵌套模块上下文跟踪后。

### 理由

1. Stage 4.1 刚实现嵌套模块支持——`current_module` 跟踪需要更多工作
2. 当前所有 item 在 crate root 解析——same-crate 访问是安全的
3. 基础设施（`def_visibility` map + `check_visibility` hook）已就位
4. 一旦 `current_module` 跟踪添加，完整强制自动激活

### 偿还计划

Stage 4+ 添加 `current_module` 跟踪后激活完整强制。

---

## ADR-005: 闭包捕获 — Copy 模式

**日期**: Stage 4.7 (v0.9.4)
**状态**: ✅ Accepted (move/borrow deferred)
**决策者**: ALG-C

### 背景

闭包捕获分析（Stage 4.7）使用 `Operand::Copy` 捕获所有外部变量。
Rust 区分 `Copy`/`Move`/`Borrow` 捕获模式。

### 决策

**Stage 4.7 使用 Copy 模式**，move/borrow 区分推迟。

### 理由

1. Copy 是最简单的捕获模式——不需要借用检查器参与
2. 对于 `i32`/`bool`/`f64` 等 Copy 类型，Copy 捕获是正确的
3. Move/Borrow 捕获需要 borrowck 集成（更复杂）
4. 分阶段实现：先 Copy（Stage 4.7），后 Move/Borrow（Stage 4+）

### 偿还计划

Stage 4+ 实现 Move/Borrow 捕获模式区分。

---

## ADR-006: 闭包调用 — 简化 Placeholder

**日期**: Stage 4.9 (v0.9.6)
**状态**: ✅ Accepted (full call lowering deferred)
**决策者**: ARCH-A

### 背景

闭包调用（Stage 4.9）检测 `TyKind::Closure` 但返回 unit placeholder，
不实际提取捕获环境 + 调用闭包体。

### 决策

**Stage 4.9 使用简化 placeholder**，完整调用 lowering 推迟。

### 理由

1. 完整闭包调用需要：提取捕获字段 + 生成闭包函数 + 传递捕获 + 调用
2. 这是一个独立函数生成 + 闭包环境传递的复杂工作
3. 简化 placeholder 避免了错误的 `Terminator::Call`（把闭包当函数指针）
4. 分阶段实现：先检测（Stage 4.9），后完整 lowering（Stage 4+）

### 偿还计划

Stage 4+ 实现完整闭包调用 lowering。

---

## ADR-007: 内置宏展开 — MIR Lowering 阶段

**日期**: Stage 4.10 (v0.9.7)
**状态**: ✅ Accepted (user-defined macros deferred)
**决策者**: ARCH-A

### 背景

宏展开（Stage 4.10）在 MIR lowering 阶段处理，而非在 AST→HIR lowering 之前。
仅支持内置宏（`println!`/`stringify!`/`assert!`）。

### 决策

**Stage 4.10 在 MIR lowering 展开内置宏**，用户自定义 `macro_rules!` 推迟。

### 理由

1. 在 MIR lowering 展开避免了修改 driver 流水线（lex→parse→lower→resolve→mir）
2. 内置宏不需要 token tree 匹配——直接检查宏名称即可
3. 用户自定义 `macro_rules!` 需要 token tree 匹配 + 重写引擎（非常复杂）
4. 分阶段实现：先内置（Stage 4.10），后用户自定义（Stage 5+）

### 偿还计划

Stage 5+ 实现用户自定义 `macro_rules!` 宏系统。

---

**最后更新**: 2026-07-22 (Stage 4.11)
