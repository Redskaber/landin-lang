# Stage 5 Gate Review Round 41 (5.41)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.41 (stdlib vtable emission plan — aggregate)
> **基线版本**: v0.11.36 → v0.11.37
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (801.5 MiB removed)
cargo test: 1223 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 3 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibVtableEmission` | struct | `<Noun><Noun><Noun>` ✅ |
| `stdlib_vtable_emission` | free fn | `<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_emissions_for_traits` | free fn | `<noun>_<noun>_<noun>_<prep>_<noun>` ✅ |

字段命名（9 个）：
- `trait_name` / `type_name` / `global_name` (`<noun>_<noun>`) ✅
- `method_symbols` (`<noun>_<noun>`) ✅
- `slot_count` (`<noun>_<noun>`) ✅
- `byte_size_32` / `byte_size_64` (`<noun>_<noun>_<digits>`) ✅
- `is_marker` / `is_complete` (`is_<adj>`) ✅

## 3. 设计要点

1. **单次调用聚合**：`stdlib_vtable_emission(trait, type, provided)` 一次
   返回 codegen emit `@.vtable.<trait>.<type>` 全局所需的**全部信息**——
   全局名 + 方法符号列表 + slot count + 32/64 位字节大小 + marker/complete
   标志。Stage 5.42+ 修改 codegen 时只需一次调用，不再分别调用 5 个不同的
   stdlib 函数。
2. **组合既有 API**：内部调用 Stage 5.40 的 `stdlib_vtable_global_name()` +
   `stdlib_vtable_method_symbols()` + Stage 5.37/5.38 的 slot_count/byte_size
   逻辑，单一真相源，无重复计算。
3. **批量查询**：`stdlib_vtable_emissions_for_traits(traits, type, provided)`
   为一个类型上的多个 trait 一次性生成 emission 列表。未知 trait 静默跳过
   （调用方可能传入用户定义的 trait 名）。
4. **markers 包含在批量结果中**：`stdlib_vtable_emissions_for_traits(["Clone", "Copy"], ...)`
   返回 2 个 emission，Copy 的 `is_marker=true`。这让 codegen 可以决定是否
   跳过 marker 的 vtable emit（marker vtable 是空的，但仍可消费结构体字段）。
5. **`StdlibVtableEmission` 派生 `PartialEq`/`Eq`**：可用于测试断言 + 未来
   emission 缓存去重。
6. **§16 自包含**：结构体仅依赖 `&'static str` + `String` + `Vec<String>` +
   标量字段，不引用 `codegen::EmitType` / `mir::ty` / `traits::TraitResolver`，
   无循环依赖。

## 4. 新测试（17 个）

`tests/v0/stage5/plan/stdlib_vtable_emission_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_emission_clone_complete` | Clone + S + 2 方法 → 2 slots, complete |
| `test_stdlib_vtable_emission_clone_partial` | Clone + S + [clone] → not complete |
| `test_stdlib_vtable_emission_drop` | Drop + S + [drop] → 1 slot |
| `test_stdlib_vtable_emission_marker` | Copy + S + [] → 0 slots, is_marker |
| `test_stdlib_vtable_emission_unknown_trait` | BogusTrait/From/"" → None |
| `test_stdlib_vtable_emission_global_name` | global_name 字段正确 |
| `test_stdlib_vtable_emission_byte_sizes` | byte_size_32/64 各 slot count |
| `test_stdlib_vtable_emission_is_complete_true` | 完整 → true |
| `test_stdlib_vtable_emission_is_complete_false` | 部分 → false |
| `test_stdlib_vtable_emission_is_marker` | 6 markers + 非 markers |
| `test_stdlib_vtable_emission_arith` | Add + Vec + [add] |
| `test_stdlib_vtable_emissions_for_traits` | 批量 Clone + Drop |
| `test_stdlib_vtable_emissions_for_traits_filters_unknown` | 未知 trait 静默跳过 |
| `test_stdlib_vtable_emissions_for_traits_empty` | 空 trait 列表 |
| `test_stdlib_vtable_emissions_for_traits_includes_markers` | markers 包含在结果中 |
| `test_stdlib_vtable_emission_struct_eq` | PartialEq/Eq 派生 |
| `test_stdlib_vtable_emission_struct_field_access` | 字段访问 |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，组合既有 API 单一真相源，批量查询实用
- Tech Lead: GO — 1223 tests, 0 clippy warnings
- QA: GO — 17 新测试覆盖正/负/边界/markers/batch/struct semantics
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — 全部新 API 遵循 §23（含 9 个字段命名）

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.42+ (codegen vtable emission refactor)**: codegen 调用
  `stdlib_vtable_emission()` 一次，直接消费 `StdlibVtableEmission` 字段
  生成 LLVM IR——无需分别调用 5 个 stdlib 函数，代码更简洁。
- **Stage 5.43+ (dyn Trait MIR lowering)**: MIR lowering 调用
  `stdlib_vtable_emissions_for_traits()` 批量获取所有需要 emit 的 vtable。

---

**审查完成**: 2026-07-23
