# Test Plan: Stage 5.46 — Codegen Vtable Spec Builder

> **Stage**: 5.46
> **Version**: v0.11.41 → v0.11.42
> **Test file**: `tests/v0/stage5/plan/codegen_vtable_spec_builder_tests.rs`
> **Test count**: 12 new tests (1273 → 1285 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `build_vtable_global_specs()` free function 的正确性。

**关键不变量**：输出与 `emit_vtables()` 当前内联构造逻辑**逐字节一致**——
`test_build_vtable_global_specs_match_emit_vtables_inline` 显式交叉验证
（手动内联相同逻辑并断言集合相等）。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 空 Vec
- 单个 vtable → 1 spec
- 多个 vtable → multi specs（HashMap 顺序非确定，用集合比较）

### 2.2 格式组件

- global_name 格式 `.vtable.<trait>.<type>`
- method_symbols 从 VtableEntry.fn_name 提取

### 2.3 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认值（用 fresh Rodeo 模拟）
- 纯函数——不修改输入 TraitResolver
- 确定性——重复调用返回相同结果（spec 内容，HashMap 顺序可能不同）
- vtable.entries 空 → 空 method_symbols（后续 batch emit 会产生 zeroinitializer）

### 2.4 一致性

- **与 emit_vtables 内联构造一致**：手动内联 `emit_vtables()` 的构造逻辑，
  断言 build_vtable_global_specs 输出的 spec 集合与之相等

### 2.5 集成

- build + batch → 完整 LLVM IR 文本（模拟 Stage 5.47 重构后的 emit_vtables 流程）

### 2.6 真实场景

- 模拟真实 TraitResolver：struct S impls Clone + Drop + Display → 3 specs

## 3. 测试统计

- 新增: 12 tests
- 基线: 1273 tests
- 总计: 1285 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.45 (`StdlibVtableGlobalSpec` + `emit_vtable_globals_batch`)
  - 现有 `emit_vtables()` (Stage 5.6) — 用于交叉验证
  - `TraitResolver` + `Vtable` + `VtableEntry` (Stage 5.5)
- 下游:
  - Stage 5.47 (codegen vtable emission refactor) — `emit_vtables()`
    调用 `build_vtable_global_specs()` + `emit_vtable_globals_batch()`
  - Stage 5.48+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (759.5 MiB removed)
cargo test: 1285 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
