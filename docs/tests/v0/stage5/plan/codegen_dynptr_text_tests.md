# Test Plan: Stage 5.48 — Codegen Dynptr Global Text Helper

> **Stage**: 5.48
> **Version**: v0.11.43 → v0.11.44
> **Test file**: `tests/v0/stage5/plan/codegen_dynptr_text_tests.rs`
> **Test count**: 12 new tests (1298 → 1310 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_dynptr_global_text()` free function 的正确性。

**关键不变量**：输出与 `TextEmitter::emit_dyn_trait_const()` **逐字节一致**——
`test_emit_dynptr_global_text_match_text_emitter` 显式交叉验证（构造
TextEmitter，调用 trait method，断言 free fn 输出出现在 TextEmitter 输出中）。

## 2. 覆盖场景

### 2.1 基本 emission

- 基本调用 (Foo + S)
- Foo + S 例子（匹配 doc comment）
- Display + Vec 例子

### 2.2 格式组件

- 全局名格式 `@<global_name> = ...`
- data symbol `ptr @<data_symbol>`
- vtable symbol `ptr @<vtable_symbol>`
- 输入无 `@` 前缀（函数添加）→ 输出恰好三个 `@`
- struct 类型 `{ ptr, ptr }`
- 完整格式验证

### 2.3 codegen 一致性交叉验证

- `test_emit_dynptr_global_text_match_text_emitter`: 构造 (global_name,
  data_symbol, vtable_symbol)，同时调用 free fn 和
  `TextEmitter::emit_dyn_trait_const()`，断言 free fn 输出出现在
  TextEmitter 的 `output_with_globals()` 中

### 2.4 真实场景 + 多常量

- 模拟真实场景：S impls Clone + Drop → 2 dynptr globals（共享 .data.S）
- 多个常量值（A.X / B.Y / C.Z）独立性验证

## 3. 测试统计

- 新增: 12 tests
- 基线: 1298 tests
- 总计: 1310 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.44 (`emit_vtable_global_text` — 设计对称参考)
  - 现有 `TextEmitter::emit_dyn_trait_const()` (Stage 5.7)
- 下游:
  - Stage 5.49 (codegen dynptr emission refactor) —
    `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()`
  - Stage 5.50+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (969.6 MiB removed)
cargo test: 1310 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
