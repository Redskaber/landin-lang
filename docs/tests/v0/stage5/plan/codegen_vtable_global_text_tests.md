# Test Plan: Stage 5.44 — Codegen Vtable Global Text Bridge

> **Stage**: 5.44
> **Version**: v0.11.39 → v0.11.40
> **Test file**: `tests/v0/stage5/plan/codegen_vtable_global_text_tests.rs`
> **Test count**: 12 new tests (1249 → 1261 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_vtable_global_text()` free function 的正确性。

**关键不变量**：输出与 `TextEmitter::emit_vtable_global()` 在非 null 路径上
**逐字节一致**——`test_emit_vtable_global_text_match_text_emitter` +
`_empty` 显式交叉验证。

## 2. 覆盖场景

### 2.1 基本 emission

- 2-symbol vtable IR (Clone)
- 空 symbols → zeroinitializer (Copy marker)
- 1 symbol (Drop)
- 3+ symbols

### 2.2 "null" 处理

- 单 "null" → `ptr null`
- 混合：真实符号 + null

### 2.3 格式组件

- 全局名格式 `@<global_name> = ...`
- 数组类型 `[N x ptr]`
- 输入 global_name 无 `@` 前缀（函数添加）→ 输出恰好一个 `@`

### 2.4 codegen 一致性交叉验证

- `test_emit_vtable_global_text_match_text_emitter`: 2-symbol 非空路径
- `test_emit_vtable_global_text_match_text_emitter_empty`: 空路径 (zeroinitializer)

### 2.5 null 路径分歧文档化

- `test_emit_vtable_global_text_null_path_diverges_from_text_emitter`:
  记录 free fn（正确处理 null → `ptr null`）与 TextEmitter（当前路径不处理
  null → 会产生 `ptr @null`）的分歧。Stage 5.45 委托重构后消除此分歧。

## 3. 测试统计

- 新增: 12 tests
- 基线: 1249 tests
- 总计: 1261 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.43 (`emit_vtable_global_from_emission` 高层 API)
  - 现有 `TextEmitter::emit_vtable_global()` (Stage 5.6)
- 下游:
  - Stage 5.45 (codegen vtable emission refactor) —
    `emit_vtable_global_from_emission()` 内部调用 `emit_vtable_global_text()`；
    `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
  - Stage 5.46+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (936.4 MiB removed)
cargo test: 1261 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
