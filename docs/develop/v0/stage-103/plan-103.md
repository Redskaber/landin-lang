# Stage 103 开发计划 — Adt Aggregate writeback (Layer 3 根因修复)

> **阶段**: v0.11 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 修复 - Layer 3 真正根因)
> **TD**: TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (P2, v0.11+) — Stage 102 发现, 本阶段修复真正根因
> **复杂度**: L3 (跨模块: mir/lower/writeback + typeck + 影响 codegen)
> **版本基线**: v0.641.0 (Stage 102 Layer 4, 5606 tests)
> **目标版本**: v0.642.0

## 一、5W2H 设计分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 在 `compute_writeback_ty` 添加 `AggregateKind::Adt` 处理 — 从 Adt struct definition 解析 field types, 替换 struct literal 中 Infer 类型 |
| **WHY** | **真正根因**: Stage 102 误判为 "LLVM module 全局累积", 实际是 typeck writeback 对 `String::new()` body 中的 struct literal `String { ptr: 0, len: 0usize, cap: 0usize }` 不解析 field types. 0 字面量推断为 Infer(IntVar), codegen fallback 到 i32, String struct layout 错误, 运行时 SIGSEGV (signal 11, exit 139). 100 次跑 1 次失败 (非确定, 因 LLVM 内存布局). |
| **WHO** | ARCH-A 设计; DEV-A 实施; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 103 完成 → 进入 Stage 104 (重新添加 Debug + PartialOrd impls) |
| **WHERE** | `src/mir/lower/writeback.rs:233-289 compute_writeback_ty`; 参考 Rule 1 (Tuple Aggregate) + Rule 2 (Array Aggregate) 模式 |
| **HOW** | 1) 添加 `AggregateKind::Adt(def_id, variant, substs, field_tys)` arm; 2) 对每个 field_ty 是 Infer 的, 从 HIR struct definition 解析 field type; 3) 替换 field_tys 中的 Infer |
| **HOW MUCH** | 1 src 文件 (writeback.rs ~30 LOC), 1 测试文件 (~80 LOC) |

## 二、对齐设计文档 (§13.1 / §8.4.5)

### docs/lang-design/06-mir.md 对齐
Rust 设计: struct literal field types 由 typeck 解析. Landin 当前 typeck writeback 对 Adt Aggregate 缺失, 违反设计.

### docs/graph/mir/data-flow.md 对齐
data flow: MIR lower (产生 Infer) → writeback (应解析 Infer) → codegen. Adt Aggregate 的 field types 在 writeback 阶段未被解析, 违反 data flow.

### Rust 设计对齐
rustc typeck: struct literal field types 通过 `TypeckResults::field_ty` 解析. Landin 应一致.

## 三、决策点 (§12 最优>最小, §1.0 原则 4 报错>静默)

### 决策 1: 添加 Adt Aggregate writeback

**选择**: 在 `compute_writeback_ty` 添加 `AggregateKind::Adt` arm, 从 HIR struct definition 解析 field types.

**替代方案 (拒绝)**:
- ❌ 在 codegen 中 fallback 到 i32 (现状) — 治症不治根, 产生错误 LLVM IR
- ❌ 在 MIR lower 中强制 field types (违反单一职责)

**理由** (§1.0 原则 4 报错>静默, §12 最优>最小):
- writeback 是解析 Infer 的正确位置
- 与 Rule 1 (Tuple) + Rule 2 (Array) 一致 — Adt 也应解析
- 根因修复, 非最小补丁

### 决策 2: 从 HIR struct definition 解析 field types

**选择**: 通过 Adt def_id 查 HIR struct definition, 获取 field types.

**理由** (§1.0 原则 10 唯一可信数据源):
- HIR struct definition 是 field types 的唯一可信源
- writeback 已有 HIR 访问 (writeback_fndef_substs 用 HIR)

## 四、MUV 拆分

| MUV | 任务 | 验收 |
|-----|------|------|
| 103.1 | 5W2H 根因分析 (Stage 102 误判修正) | 根因明确 |
| 103.2 | 添加 `AggregateKind::Adt` writeback arm | 编译通过 |
| 103.3 | 从 HIR struct definition 解析 field types | field_tys Infer 被替换 |
| 103.4 | 添加 stage103 测试 (1:3+ 正负比例) | cargo test 全绿 |
| 103.5 | 加 Debug impl 验证修复效果 | cargo test 全绿 (无 SIGSEGV) |
| 103.6 | §3.2 验收 + 文档同步 + 打包 | 完整交付 |

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- [ ] `cargo test --release --features llvm-backend --lib` ✓ (898+ tests, 0 failures)
- [ ] `cargo test --release --features llvm-backend --test all_tests` ✓ (5613+ tests, 0 failures, 9 ignored)
- [ ] 加 Debug impl 后 cargo test 100 次跑 0 SIGSEGV
