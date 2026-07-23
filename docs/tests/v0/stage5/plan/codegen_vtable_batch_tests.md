# Test Plan: Stage 5.45 — Codegen Vtable Emission Batch Helper

> **Stage**: 5.45
> **Version**: v0.11.40 → v0.11.41
> **Test file**: `tests/v0/stage5/plan/codegen_vtable_batch_tests.rs`
> **Test count**: 12 new tests (1261 → 1273 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `StdlibVtableGlobalSpec` struct + `emit_vtable_globals_batch()` free function
的正确性。

**关键不变量**：batch 输出 == 逐个调用 `emit_vtable_global_text()` 收集的结果
（`test_emit_vtable_globals_batch_matches_individual` 验证）。

## 2. 覆盖场景

### 2.1 边界

- 空 input → 空 Vec
- 单个 spec → 1-element Vec

### 2.2 基本 batch

- 多个 spec → multi-element Vec，顺序保留
- 顺序保留（非字母序）—— 不排序

### 2.3 边界情况

- 含 marker spec (empty method_symbols → zeroinitializer)
- 含 null symbol → `ptr null`
- 混合：marker + null + real

### 2.4 一致性

- **batch == 逐个调用**：3 个 spec (含 marker + null + real) 的 batch 输出
  等于逐个调用 `emit_vtable_global_text()` 收集的结果
- 不去重：两个相同 spec → 两个相同 IR 行（去重责任在调用方）

### 2.5 结构体语义

- `StdlibVtableGlobalSpec` 字段访问
- `StdlibVtableGlobalSpec` 派生 PartialEq/Eq

### 2.6 真实场景模拟

- 模拟 `emit_vtables()` 场景：struct S impls Clone + Drop + Add →
  3 个 spec → 3 行有效 LLVM IR

## 3. 测试统计

- 新增: 12 tests
- 基线: 1261 tests
- 总计: 1273 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游: Stage 5.44 (`emit_vtable_global_text`)
- 下游:
  - Stage 5.46 (codegen vtable emission refactor) — `emit_vtables()`
    构造 spec list，调用 batch helper
  - Stage 5.47+ (dyn Trait MIR lowering) — 直接调用 batch helper

## 5. CI/CD 验证

```
cargo clean: clean (938.7 MiB removed)
cargo test: 1273 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
