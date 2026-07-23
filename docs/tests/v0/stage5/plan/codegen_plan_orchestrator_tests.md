# Test Plan: Stage 5.54 — Codegen Trait-Dispatch Emission Orchestrator (Plan-Based)

> **Stage**: 5.54
> **Version**: v0.11.49 → v0.11.50
> **Test file**: `tests/v0/stage5/plan/codegen_plan_orchestrator_tests.rs`
> **Test count**: 12 new tests (1372 → 1384 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_trait_dispatch_globals_from_plan()` orchestrator 的正确性。

**关键不变量**：与 `emit_vtables_and_dynptrs_from_resolver()` (Stage 5.51)
**行为完全等价**——`test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator`
显式交叉验证。

## 2. 覆盖场景

### 2.1 边界

- 空 plan → 不调用 emitter
- 单 spec → vtable + dynptr global
- 多 spec → 多 vtable + 多 dynptr

### 2.2 **行为等价交叉验证**

- `test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator`:
  调用 plan-based orchestrator vs resolver-based orchestrator (Stage 5.51)，
  断言输出完全相同

### 2.3 发射正确性

- vtable globals 发射
- dynptr globals 发射
- vtable + dynptr 数 == 2 × specs.len()
- vtable 在 dynptr 前（顺序）

### 2.4 边界情况

- 不修改 plan（pure w.r.t. plan）

### 2.5 真实场景 + 组合 + 确定性

- 模拟真实场景：S impls Clone+Drop+Display → 3 vtable + 3 dynptr
- 组合 plan + emit 验证
- 重复调用相同次数

## 3. 测试统计

- 新增: 12 tests
- 基线: 1372 tests
- 总计: 1384 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.53 (`build_trait_dispatch_emission_plan`)
  - Stage 5.51 (`emit_vtables_and_dynptrs_from_resolver`) — 用于交叉验证
- 下游:
  - Stage 5.55 (codegen trait-dispatch emission refactor) — driver 调用
    plan + orchestrator
  - Stage 5.56+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (970.5 MiB removed)
cargo test: 1384 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
