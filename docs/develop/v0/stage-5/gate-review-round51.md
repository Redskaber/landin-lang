# Stage 5 Gate Review Round 51 (5.51)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.51 (codegen vtable + dynptr combined emission orchestrator)
> **基线版本**: v0.11.46 → v0.11.47
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (1023.3 MiB removed)
cargo test: 1346 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 1 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `emit_vtables_and_dynptrs_from_resolver` | free fn | `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` ✅ |

`emit_` 前缀一致。`_and_` 连接两个名词（vtables + dynptrs），`_from_resolver`
表明输入来自 TraitResolver。

## 3. 设计要点

1. **单一入口点**：`emit_vtables_and_dynptrs_from_resolver()` 一次调用发射所有
   trait-dispatch globals（vtable + dynptr）。Stage 5.52 driver/codegen 可调用
   这一个函数替代分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()`。
2. **组合既有 orchestrator**：内部调用 Stage 5.47 `emit_vtables_from_resolver()`
   + Stage 5.50 `emit_dynptrs_from_resolver()`。单一真相源，无重复逻辑。
3. **行为等价**：与分别调用 `emit_vtables()` + `emit_dyn_trait_ptrs()`
   **行为完全等价**——`test_emit_vtables_and_dynptrs_match_separate_calls`
   交叉验证。
4. **不修改现有路径**：`emit_vtables()` / `emit_dyn_trait_ptrs()` /
   `emit_vtables_from_resolver()` / `emit_dynptrs_from_resolver()` 全部保持不变。
   Stage 5.52 才让 driver 调用 combined orchestrator。
5. **§16 接口隔离**：输入 `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`，
   无循环依赖。

## 4. 新测试（12 个）

`tests/v0/stage5/plan/codegen_combined_orchestrator_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_emit_vtables_and_dynptrs_empty` | 空 TraitResolver → 不调用 emitter |
| `test_emit_vtables_and_dynptrs_single` | 单个 vtable → vtable + dynptr global |
| `test_emit_vtables_and_dynptrs_multi` | 多个 vtable → 多 vtable + 多 dynptr |
| `test_emit_vtables_and_dynptrs_match_separate_calls` | **== emit_vtables + emit_dyn_trait_ptrs** |
| `test_emit_vtables_and_dynptrs_no_side_effects_on_resolver` | 不修改 resolver |
| `test_emit_vtables_and_dynptrs_real_scenario` | 模拟真实场景（S impls Clone+Drop+Display） |
| `test_emit_vtables_and_dynptrs_unresolved_interner` | interner 未找到 → 默认名 |
| `test_emit_vtables_and_dynptrs_emitter_called_correctly` | emitter 接收正确参数 |
| `test_emit_vtables_and_dynptrs_count_matches_vtables` | vtable + dynptr 数 == vtables.len() × 2 |
| `test_emit_vtables_and_dynptrs_composes_both` | 组合两者验证 |
| `test_emit_vtables_and_dynptrs_deterministic_count` | 重复调用相同次数 |
| `test_emit_vtables_and_dynptrs_order` | vtable 在 dynptr 前 |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，单一入口点设计清晰，为 5.52 driver 重构铺路
- Tech Lead: GO — 1346 tests, 0 clippy warnings, 行为等价交叉验证
- QA: GO — 12 新测试覆盖正/负/边界/real-scenario/behavior-equivalence/order
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — `emit_vtables_and_dynptrs_from_resolver` 遵循 §23 `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>`

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.52 (codegen trait-dispatch emission refactor)**:
  - driver/codegen 调用 `emit_vtables_and_dynptrs_from_resolver()` 替代分别调用
  - `TextEmitter::emit_vtable_global()` / `emit_dyn_trait_const()` 委托给 free fn
- **Stage 5.53+ (dyn Trait MIR lowering)**: 直接调用 combined orchestrator

---

**审查完成**: 2026-07-23
