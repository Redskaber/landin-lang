# Test Plan: Stage 5.50 — Codegen Dynptr Emission Orchestrator

> **Stage**: 5.50
> **Version**: v0.11.45 → v0.11.46
> **Test file**: `tests/v0/stage5/plan/codegen_dynptr_orchestrator_tests.rs`
> **Test count**: 12 new tests (1322 → 1334 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_dynptrs_from_resolver()` orchestrator 的正确性。

**关键不变量**：与 `emit_dyn_trait_ptrs()` (Stage 5.7) 当前内联循环**行为完全等价**——
`test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs` + `_multi` 显式交叉验证
（调用两者于相同 TraitResolver + interner + TextEmitter，断言输出完全相同）。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 不调用 emitter，输出不含 `.dynptr.`
- 单个 vtable → 1 次 emitter 调用
- 多个 vtable → 多次 emitter 调用

### 2.2 **行为等价交叉验证**

- `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs`: 单 vtable (Clone+S)
- `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs_multi`: 多 vtable (Foo+S, Drop+S)
- 两者都断言 `emit_dyn_trait_ptrs()` 与 `emit_dynptrs_from_resolver()` 输出完全相同

### 2.3 边界情况

- interner 未找到 Spur → "Trait"/"Type" 默认名
- 不修改 resolver（pure w.r.t. TraitResolver）

### 2.4 调用正确性

- emitter 接收正确参数（global_name + data_symbol + vtable_symbol）
- 调用次数 == vtables.len()
- 组合 build + emit 验证（输出含完整 IR 行）

### 2.5 确定性 + 真实场景

- 重复调用产生相同 dynptr 计数
- 模拟真实场景：S impls Clone+Drop+Display → 3 dynptr globals

## 3. 测试统计

- 新增: 12 tests
- 基线: 1322 tests
- 总计: 1334 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.49 (`build_dynptr_global_specs`)
  - 现有 `emit_dyn_trait_ptrs()` (Stage 5.7) — 用于交叉验证
  - `Emitter::emit_dyn_trait_const()` trait method
- 下游:
  - Stage 5.51 (codegen vtable + dynptr emission refactor) —
    `emit_dyn_trait_ptrs()` 委托给 orchestrator（一行方法体）
  - Stage 5.52+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (831.6 MiB removed)
cargo test: 1334 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅
```

---

**创建日期**: 2026-07-23
