# Test Plan: Stage 5.55 — Codegen Trait-Dispatch Emission Text Batch (Plan-Based)

> **Stage**: 5.55
> **Version**: v0.11.50 → v0.11.51
> **Test file**: `tests/v0/stage5/plan/codegen_text_batch_tests.rs`
> **Test count**: 12 new tests (1384 → 1396 total)
> **Process ref**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. 测试目标

验证 `emit_trait_dispatch_globals_text_batch()` 的正确性。

**关键不变量**：text batch 输出与 `emit_trait_dispatch_globals_from_plan()` (Stage 5.54)
通过 Emitter 生成的 IR **逐字节一致**——
`test_emit_trait_dispatch_globals_text_batch_match_orchestrator` 显式交叉验证。

## 2. 覆盖场景

### 2.1 边界

- 空 plan → 空 Vec
- 单 spec → 2 行（1 vtable + 1 dynptr）
- 多 spec → 多行

### 2.2 **行为等价交叉验证**

- `test_emit_trait_dispatch_globals_text_batch_match_orchestrator`: 调用 text batch
  vs orchestrator（通过 Emitter），断言每行 text 出现在 emitter 输出中

### 2.3 行正确性

- vtable IR 行正确（含 `@.vtable.` + `private unnamed_addr constant`）
- dynptr IR 行正确（含 `@.dynptr.` + `ptr @.data.` + `ptr @.vtable.`）
- 总行数 == 2 × specs.len()
- vtable 行在 dynptr 行前（顺序）

### 2.4 边界情况

- 纯函数——不修改输入 plan
- 无需 Emitter——不构造任何 Emitter trait 对象

### 2.5 真实场景 + 确定性

- 模拟真实场景：S impls Clone+Drop+Display → 6 行（3 vtable + 3 dynptr）
- 重复调用相同结果

## 3. 测试统计

- 新增: 12 tests
- 基线: 1384 tests
- 总计: 1396 tests
- 2 ignored (pre-existing, 未影响)

## 4. 依赖

- 上游:
  - Stage 5.53 (`build_trait_dispatch_emission_plan`)
  - Stage 5.54 (`emit_trait_dispatch_globals_from_plan`) — 用于交叉验证
  - Stage 5.44 (`emit_vtable_global_text`) + Stage 5.48 (`emit_dynptr_global_text`) — 内部调用
- 下游:
  - Stage 5.56 (codegen trait-dispatch emission refactor) — codegen 可直接 push text batch
  - Stage 5.57+ (dyn Trait MIR lowering) — 直接调用

## 5. CI/CD 验证

```
cargo clean: clean (974.9 MiB removed)
cargo test: 1396 passed, 0 failed, 2 ignored ✅
cargo fmt --check: clean ✅
cargo clippy --all-targets: 0 warnings ✅ (修复了 1 个 doc_lazy_continuation)
```

---

**创建日期**: 2026-07-23
