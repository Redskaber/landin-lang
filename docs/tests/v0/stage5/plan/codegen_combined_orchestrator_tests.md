# Test Plan: Stage 5.51 — Codegen Vtable + Dynptr Combined Emission Orchestrator

> **Stage**: 5.51
> **Version**: v0.11.46 → v0.11.47
> **Test file**: `tests/v0/stage5/plan/codegen_combined_orchestrator_tests.rs`
> **Test count**: 12 new tests (1334 → 1346 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_vtables_and_dynptrs_from_resolver()` combined orchestrator 的正确性。

**关键不变量**：与分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()` **行为完全等价**——
`test_emit_vtables_and_dynptrs_match_separate_calls` 显式交叉验证。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 不调用 emitter
- 单个 vtable → vtable + dynptr global
- 多个 vtable → 多 vtable + 多 dynptr

### 2.2 **行为等价交叉验证**

- `test_emit_vtables_and_dynptrs_match_separate_calls`: 调用 combined orchestrator
  vs 分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()`，断言输出完全相同

### 2.3 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认名
- 不修改 resolver（pure w.r.t. TraitResolver）

### 2.4 调用正确性

- emitter 接收正确参数（vtable + dynptr 全局）
- vtable + dynptr 定义数 == vtables.len() × 2
- 组合两者验证（输出含 vtable + dynptr 全局）
- vtable 全局出现在 dynptr 全局前（顺序）

### 2.5 确定性 + 真实场景

- 重复调用产生相同计数
- 模拟真实场景：S impls Clone+Drop+Display → 3 vtable + 3 dynptr globals

## 3. 测试统计

- 新增: 12 tests
- 基线: 1334 tests
- 总计: 1346 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.47 (`emit_vtables_from_resolver`)
  - Stage 5.50 (`emit_dynptrs_from_resolver`)
  - 现有 `emit_vtables()` (Stage 5.6) + `emit_dyn_trait_ptrs()` (Stage 5.7) — 用于交叉验证
- 下游:
  - Stage 5.52 (codegen trait-dispatch emission refactor) — driver 调用
    combined orchestrator
  - Stage 5.53+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (1023.3 MiB removed)
cargo test: 1346 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
