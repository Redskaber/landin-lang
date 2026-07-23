# Test Plan: Stage 5.47 — Codegen Vtable Emission Orchestrator

> **Stage**: 5.47
> **Version**: v0.11.42 → v0.11.43
> **Test file**: `tests/v0/stage5/plan/codegen_vtable_orchestrator_tests.rs`
> **Test count**: 13 new tests (1285 → 1298 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_vtables_from_resolver()` orchestrator 的正确性。

**关键不变量**：与 `emit_vtables()` (Stage 5.6) 当前内联循环**行为完全等价**——
`test_emit_vtables_from_resolver_match_emit_vtables` + `_multi` 显式交叉验证
（调用两者于相同 TraitResolver + interner + TextEmitter，断言输出完全相同）。

## 2. 覆盖场景

### 2.1 边界

- 空 TraitResolver → 不调用 emitter，输出不含 `.vtable.`
- 单个 vtable → 1 次 emitter 调用
- 多个 vtable → 多次 emitter 调用

### 2.2 **行为等价交叉验证**

- `test_emit_vtables_from_resolver_match_emit_vtables`: 单 vtable (Clone+S)
- `test_emit_vtables_from_resolver_match_emit_vtables_multi`: 多 vtable (Foo+S, Drop+S)
- 两者都断言 `emit_vtables()` 与 `emit_vtables_from_resolver()` 输出完全相同

### 2.3 边界情况

- vtable.entries 空 → 仍调用 emitter（emits zeroinitializer）
- interner 未找到 Spur → "Trait"/"Type" 默认名
- 不修改 resolver（pure w.r.t. TraitResolver）

### 2.4 调用正确性

- emitter 接收正确参数（global_name + method_symbols）
- 调用次数 == vtables.len()
- 组合 build + emit 验证（输出含完整 IR 行）

### 2.5 确定性 + 真实场景

- 重复调用产生相同 vtable 计数
- 模拟真实场景：S impls Clone+Drop+Display → 3 vtables

## 3. 测试统计

- 新增: 13 tests
- 基线: 1285 tests
- 总计: 1298 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.46 (`build_vtable_global_specs`)
  - 现有 `emit_vtables()` (Stage 5.6) — 用于交叉验证
  - `Emitter::emit_vtable_global()` trait method
- 下游:
  - Stage 5.48 (codegen vtable emission refactor) — `emit_vtables()` 委托
    给 orchestrator（一行方法体）
  - Stage 5.49+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (822.9 MiB removed)
cargo test: 1298 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅ (修复了 1 个 unused import)
```

---

**创建日期**: 2026-07-23
