# Stage 5 Gate Review Round 49 (5.49)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.49 (codegen dynptr spec builder)
> **基线版本**: v0.11.44 → v0.11.45
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (828.0 MiB removed)
cargo test: 1322 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 2 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibDynptrGlobalSpec` | struct | `<Noun><Noun><Noun><Noun>` ✅ |
| `build_dynptr_global_specs` | free fn | `<verb>_<noun>_<adj>_<noun>` ✅ |

字段命名：`global_name` / `data_symbol` / `vtable_symbol` (`<noun>_<noun>`) ✅

命名与 Stage 5.46 `build_vtable_global_specs` / `StdlibVtableGlobalSpec`
对称（vtable → dynptr）。

## 3. 设计要点

1. **dynptr 对应版本**：Stage 5.46 添加了 `build_vtable_global_specs()`（vtable
   spec 构造），本轮添加 `build_dynptr_global_specs()`（dynptr spec 构造）。
   两者命名对称，设计模式一致。
2. **纯函数提取**：`build_dynptr_global_specs()` 是 `emit_dyn_trait_ptrs()`
   当前内联构造 spec 列表逻辑的纯函数版本。输入参数与
   `emit_dyn_trait_ptrs()` 完全相同（除 emitter）。
3. **逐字节一致**：与 `emit_dyn_trait_ptrs()` 当前内联构造逻辑**逐字节一致**
   （`test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs` 验证）。
4. **不修改现有路径**：`emit_dyn_trait_ptrs()` 保持不变。Stage 5.50 才让
   `emit_dyn_trait_ptrs()` 调用 `build_dynptr_global_specs()` + 批量 push。
5. **§16 接口隔离**：输入 `&TraitResolver` + `&Rodeo`，输出
   `Vec<StdlibDynptrGlobalSpec>`。不引用 `mir::ty` / `Emitter`，无循环依赖。

## 4. 新测试（12 个）

`tests/v0/stage5/plan/codegen_dynptr_spec_builder_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_build_dynptr_global_specs_empty` | 空 TraitResolver → 空 Vec |
| `test_build_dynptr_global_specs_single` | 单个 vtable |
| `test_build_dynptr_global_specs_multi` | 多个 vtable |
| `test_build_dynptr_global_specs_global_name_format` | `.dynptr.<trait>.<type>` 格式 |
| `test_build_dynptr_global_specs_data_symbol` | `.data.<type>` 格式 |
| `test_build_dynptr_global_specs_vtable_symbol` | `.vtable.<trait>.<type>` 格式 |
| `test_build_dynptr_global_specs_unresolved_interner` | interner 未找到 → "Trait"/"Type" 默认 |
| `test_build_dynptr_global_specs_no_side_effects` | 纯函数，不修改输入 |
| `test_build_dynptr_global_specs_deterministic` | 重复调用返回相同结果 |
| `test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs` | **与 emit_dyn_trait_ptrs 内联构造一致** |
| `test_build_dynptr_global_specs_then_emit` | build + emit_dynptr_global_text 验证 |
| `test_build_dynptr_global_specs_real_scenario` | 模拟真实场景（S impls Clone+Drop+Display） |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，dynptr 对称设计，为 5.50 重构铺路
- Tech Lead: GO — 1322 tests, 0 clippy warnings, 交叉验证测试保证一致性
- QA: GO — 12 新测试覆盖正/负/边界/unresolved/side-effects/determinism/real-scenario
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — `build_dynptr_global_specs` 遵循 §23 `<verb>_<noun>_<adj>_<noun>`，与 `build_vtable_global_specs` 对称

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.50 (codegen dynptr emission refactor)**:
  - `emit_dyn_trait_ptrs()` 内部调用 `build_dynptr_global_specs()` + 批量 push
  - `TextEmitter::emit_dyn_trait_const()` 委托给 `emit_dynptr_global_text()` (Stage 5.48)
- **Stage 5.51+ (dyn Trait MIR lowering)**: 直接调用 spec builder

---

**审查完成**: 2026-07-23
