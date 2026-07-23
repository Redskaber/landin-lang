# Stage 5 Gate Review Round 46 (5.46)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.46 (codegen vtable spec builder)
> **基线版本**: v0.11.41 → v0.11.42
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (759.5 MiB removed)
cargo test: 1285 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 1 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `build_vtable_global_specs` | free fn | `<verb>_<noun>_<adj>_<noun>` ✅ |

`build_` 前缀表明构造函数（输入数据 → 输出数据，无副作用）。`_specs`（复数）
表明返回多个 spec。

## 3. 设计要点

1. **纯函数提取**：`build_vtable_global_specs(&TraitResolver, &Rodeo) -> Vec<StdlibVtableGlobalSpec>`
   是 `emit_vtables()` 当前内联构造 spec 列表逻辑的纯函数版本。输入参数
   与 `emit_vtables()` 完全相同（除 emitter）。
2. **逐字节一致**：与 `emit_vtables()` 当前内联构造逻辑**逐字节一致**
   （`test_build_vtable_global_specs_match_emit_vtables_inline` 验证）。
3. **不修改现有路径**：`emit_vtables()` 保持不变。Stage 5.47 才让
   `emit_vtables()` 调用 `build_vtable_global_specs()` +
   `emit_vtable_globals_batch()` + 批量 push 到 emitter。
4. **§16 接口隔离**：输入 `&TraitResolver` + `&Rodeo`，输出
   `Vec<StdlibVtableGlobalSpec>`。不引用 `mir::ty` / `Emitter`，无循环依赖。

## 4. 新测试（12 个）

`tests/v0/stage5/plan/codegen_vtable_spec_builder_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_build_vtable_global_specs_empty` | 空 TraitResolver → 空 Vec |
| `test_build_vtable_global_specs_single` | 单个 vtable |
| `test_build_vtable_global_specs_multi` | 多个 vtable |
| `test_build_vtable_global_specs_global_name_format` | `.vtable.<trait>.<type>` 格式 |
| `test_build_vtable_global_specs_method_symbols` | method_symbols 从 VtableEntry.fn_name 提取 |
| `test_build_vtable_global_specs_unresolved_interner` | interner 未找到 → "Trait"/"Type" 默认 |
| `test_build_vtable_global_specs_no_side_effects` | 纯函数，不修改输入 |
| `test_build_vtable_global_specs_deterministic` | 重复调用返回相同结果 |
| `test_build_vtable_global_specs_match_emit_vtables_inline` | **与 emit_vtables 内联构造一致** |
| `test_build_vtable_global_specs_then_batch_emit` | build + batch → 完整 IR 文本 |
| `test_build_vtable_global_specs_empty_vtable_entries` | vtable.entries 空 → 空 method_symbols |
| `test_build_vtable_global_specs_real_scenario` | 模拟真实 TraitResolver 场景（S impls Clone+Drop+Display） |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，纯函数提取策略清晰，为 5.47 重构铺路
- Tech Lead: GO — 1285 tests, 0 clippy warnings, 交叉验证测试保证一致性
- QA: GO — 12 新测试覆盖正/负/边界/unresolved/side-effects/determinism/real-scenario
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — `build_vtable_global_specs` 遵循 §23 `<verb>_<noun>_<adj>_<noun>`

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.47 (codegen vtable emission refactor)**:
  - `emit_vtables()` 内部调用 `build_vtable_global_specs()` +
    `emit_vtable_globals_batch()` + 批量 push 到 emitter
  - `TextEmitter::emit_vtable_global()` 委托给 `emit_vtable_global_text()`
- **Stage 5.48+ (dyn Trait MIR lowering)**: 直接调用 spec builder + batch

---

**审查完成**: 2026-07-23
