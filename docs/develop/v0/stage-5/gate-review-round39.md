# Stage 5 Gate Review Round 39 (5.39)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.39 (stdlib vtable construction planner)
> **基线版本**: v0.11.34 → v0.11.35
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (916.7 MiB removed)
cargo test: 1190 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 6 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibVtablePlanEntry` | struct | `<Noun><Noun><Noun><Noun>` ✅ |
| `StdlibVtablePlan` | struct | `<Noun><Noun><Noun>` ✅ |
| `StdlibVtablePlan::is_complete` | method | `<noun>_<adj>` ✅ |
| `StdlibVtablePlan::missing_methods` | method | `<adj>_<noun>` ✅ |
| `stdlib_vtable_plan` | free fn | `<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_plan_entry_count` | free fn | `<noun>_<noun>_<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_plan_is_complete` | free fn | `<noun>_<noun>_<noun>_<adj>` ✅ |
| `stdlib_vtable_plan_missing_methods` | free fn | `<noun>_<noun>_<noun>_<adj>_<noun>` ✅ |

字段命名：`slot_index` / `method_name` / `provided` / `trait_name` / `entries` — 全部合规。

## 3. 设计要点

1. **plan = trait 声明 ∩ impl 覆盖**：`stdlib_vtable_plan(trait, provided_methods)`
   一次合并 trait 方法签名（Stage 5.36）+ slot 索引（Stage 5.37）+ impl 覆盖
   情况，生成可直接消费的有序 entry 列表。
2. **`provided` 标志**：codegen 看到 `provided=true` 填入 `@landin_<Type>_<method>`
   symbol；`provided=false` 填入 `null` 或 panic stub。无需在 codegen 内重复
   推导 slot 顺序或 provided-checking。
3. **markers 空计划**：`stdlib_vtable_plan("Copy", &[])` 返回 `Some(plan)` with
   `entries: vec![]`，且 `is_complete() == true`（vacuously complete）。
4. **extra names 静默忽略**：`provided_method_names` 中不在 trait 声明里的名字
   不影响 plan —— 不报错、不添加 entry。这是宽容设计：impl 可能实现了多个
   trait 的方法，调用方传入完整方法集是合理的。
5. **`StdlibVtablePlan` 派生 `PartialEq`/`Eq`**：可用于测试断言 + plan 缓存
   去重（未来 codegen 可能缓存 plan）。
6. **§16 自包含**：`StdlibVtablePlan` / `StdlibVtablePlanEntry` 仅依赖
   `&'static str` + `Vec<>` + 标量字段，不引用 `mir::ty` / `codegen::EmitType` /
   `traits::TraitResolver`，无循环依赖。

## 4. 新测试（18 个）

`tests/v0/stage5/plan/stdlib_vtable_plan_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_vtable_plan_clone_complete` | Clone + 2 方法 → 全 provided |
| `test_stdlib_vtable_plan_clone_partial` | Clone + 1 方法 → 1 missing |
| `test_stdlib_vtable_plan_drop` | Drop + drop → 1 entry complete |
| `test_stdlib_vtable_plan_partial_eq` | PartialEq + eq → ne missing |
| `test_stdlib_vtable_plan_add` | Add + add → complete |
| `test_stdlib_vtable_plan_marker` | Copy + [] → 空 plan, vacuously complete |
| `test_stdlib_vtable_plan_unknown_trait` | BogusTrait/From/"" → None |
| `test_stdlib_vtable_plan_extra_provided_ignored` | Clone + [clone, bogus] → bogus 不影响 |
| `test_stdlib_vtable_plan_entry_count` | Clone=2, Drop=1, Copy=0 |
| `test_stdlib_vtable_plan_is_complete_true` | 完整 plan → true |
| `test_stdlib_vtable_plan_is_complete_false` | 部分 plan → false |
| `test_stdlib_vtable_plan_missing_methods_empty` | 完整 plan → 空 Vec |
| `test_stdlib_vtable_plan_missing_methods_partial` | Clone + [clone] → ["clone_from"] |
| `test_stdlib_vtable_plan_missing_methods_all` | PartialEq + [] → ["eq", "ne"] |
| `test_stdlib_vtable_plan_deterministic_order` | 重复调用顺序一致 |
| `test_stdlib_vtable_plan_eq` | PartialEq/Eq 派生 |
| `test_stdlib_vtable_plan_entry_struct` | StdlibVtablePlanEntry 字段访问 |
| `test_stdlib_vtable_plan_entries_ordered_by_slot` | entries 按 slot_index 升序 |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，纯函数 + 派生 PartialEq/Eq，markers 三态一致
- Tech Lead: GO — 1190 tests, 0 clippy warnings
- QA: GO — 18 新测试覆盖正/负/边界/markers/extra-names/determinism/struct semantics
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — 全部新 API 遵循 §23（含 5-noun 函数名 `stdlib_vtable_plan_entry_count`）

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.40+ (dyn Trait codegen)**: codegen 调用 `stdlib_vtable_plan()` 一次，
  遍历 plan entries 直接生成 LLVM IR：
  - `provided=true` → `@landin_<Type>_<method>` symbol
  - `provided=false` → `null` 或 panic stub
- **Stage 5.41+ (typeck impl completeness)**: 调用
  `stdlib_vtable_plan_is_complete()` / `stdlib_vtable_plan_missing_methods()`
  报告"impl 未实现 trait 的 X / Y 方法"——比 Stage 5.19 的
  `missing_impl_methods()` 更具体（带 slot index + 静态 trait 信号）。

---

**审查完成**: 2026-07-23
