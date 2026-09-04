# Stage 106 开发计划 — TD-TYPECK-WRITEBACK-INCOMPLETE: Constant type writeback

> **阶段**: v0.12 (TD-TYPECK-WRITEBACK-INCOMPLETE 修复)
> **TD**: TD-TYPECK-WRITEBACK-INCOMPLETE (P2, v0.12+)
> **复杂度**: L3 (跨模块: mir/lower/writeback + typeck/checker + codegen)
> **版本基线**: v0.642.0 (Stage 105 RCA, 5613 tests)
> **目标版本**: v0.643.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 在 typeck Phase 3 (writeback to local_decls) 之后, 添加新 Phase 3.6 — Constant type writeback: 对每个 `Operand::Constant(c)` 中的 `c.ty`, 如果是 Infer(IntVar), 用 unify.resolve() 解析为 concrete Int/Uint 类型 |
| **WHY** | 根因: `lit_to_const` 对无 suffix 的 int literal (如 `0`) 创建 Infer(IntVar). typeck Phase 2 `default_unresolved()` 将 unbound IntVar 默认为 I32. 但 Phase 3 只写 `local_decl.ty = unify.resolve(&local.ty)`, **不写 Constant 的 ty**. Constant 的 ty 仍是 Infer → param_check 报 warning → codegen fallback to i32 |
| **WHO** | ARCH-A 设计; DEV-A 实施; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 106 完成 → 验证 Infer warnings 减少 |
| **WHERE** | `src/typeck/checker.rs:check_mir_body_with_tables` Phase 3 后; 遍历 `mir.basic_blocks[*].statements[*]` 中的 `Rvalue::Use(Operand::Constant(c))` + `Rvalue::Cast(_, _, ty)` + `Rvalue::Aggregate(_, operands)` 等 |
| **HOW** | 遍历所有 statement + terminator 中的 Operand::Constant, 对 `c.ty` 调用 `unify.resolve(&c.ty)` 替换 |
| **HOW MUCH** | 1 src 文件 (~40 LOC) + 1 测试文件 (~80 LOC) |

## 二、根因分析

### Infer warnings 来源
1. `landin_Default_i32_default`: `fn default() -> i32 { 0 }` — `0` 无 suffix → Infer(IntVar)
2. `landin_Display_i32_fmt`: `let buf_size: i64 = 32;` — `32` 无 suffix → Infer(IntVar)
3. `landin_String_new`: `String { ptr: 0, len: 0usize, cap: 0usize }` — `0` 无 suffix → Infer(IntVar)
4. `landin_main`: `println!("{}", ...)` — format string literal → Infer
5. `landin___landin_format_v2`: format args → Infer

### 为什么 Phase 3 不修复
Phase 3 (line 189-191):
```rust
for local in mir.local_decls.iter_mut() {
    local.ty = self.unify.resolve(&local.ty);
}
```
这只写 `local_decl.ty` — 不写 `Operand::Constant` 的 `c.ty`。Constant 的 ty 在 MIR lower 时由 `lit_to_const` 创建, 之后不更新。

### 为什么 writeback_type_propagation 不修复
`writeback_type_propagation` 中的 `compute_writeback_ty` 只处理 `Rvalue::Use(Operand::Copy/Move(place))` — 不处理 `Rvalue::Use(Operand::Constant(c))`。`operand_ty` 对 Constant 直接返回 `c.ty.clone()` (line 486), 不调用 resolve。

### 修复方案
在 typeck Phase 3 之后添加 Phase 3.6: 遍历所有 statement + terminator, 对 `Operand::Constant(c)` 的 `c.ty` 调用 `unify.resolve()` 替换。

## 三、决策点

### 决策 1: 在 typeck Phase 3 后添加 Constant type writeback

**选择**: 在 `check_mir_body_with_tables` Phase 3 后添加新 Phase 3.6。

**理由** (§12 最优>最小, §1.0 原则 6 通解>特解):
- typeck Phase 3 已 resolve local_decls — 正确的时机也 resolve Constants
- 一条规则适用于所有 Constant (int/uint/float/str)
- 与 Phase 3 同层, 不需要新 driver 调用
