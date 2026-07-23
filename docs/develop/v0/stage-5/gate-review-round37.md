# Stage 5 Gate Review Round 37 (5.37)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.37 (stdlib vtable slot layout)
> **基线版本**: v0.11.32 → v0.11.33
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (907.8 MiB removed)
cargo test: 1152 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 6 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibVtableSlot` | struct | `<Noun><Noun><Noun>` ✅ |
| `stdlib_trait_method_index` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_layout` | free fn | `<noun>_<noun>_<noun>` ✅ |
| `stdlib_vtable_slot_count` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `is_stdlib_marker_trait` | free fn | `is_<noun>_<adj>_<noun>` ✅ |
| `stdlib_traits_with_vtable` | free fn | `<noun>_<noun>_with_<noun>` ✅ |

字段命名：`slot_index` (`<noun>_<noun>`) + `method` (`<noun>`) — 全部合规。

## 3. 设计要点

1. **确定性 slot 编号**：vtable slot index 来自 `stdlib_trait_methods()` slice
   的 position（0-based），不依赖 HashMap 迭代顺序 — 同一 trait 在进程内任意
   时刻查询返回相同的 slot 顺序。
2. **markers vs unknown**：
   - `stdlib_vtable_slot_count("Copy") == Some(0)`（marker 注册但无 slot）
   - `stdlib_vtable_slot_count("BogusTrait") == None`（完全未注册）
   - `is_stdlib_marker_trait("Copy") == true`
   - `is_stdlib_marker_trait("BogusTrait") == false`（不算 marker）
3. **`stdlib_traits_with_vtable()`** 过滤掉 markers — codegen 只需为有方法的
   trait 发射 `@.vtable.<trait>.<type>` 全局，markers 无需 vtable。
4. **§16 自包含**：`StdlibVtableSlot` 包含 `&'static StdlibTraitMethod`（stdlib
   内部），不引用 `mir::ty` / `codegen::EmitType`，无循环依赖。

## 4. 新测试（22 个）

`tests/v0/stage5/plan/stdlib_vtable_layout_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_trait_method_index_clone` | Clone::clone@0, clone_from@1 |
| `test_stdlib_trait_method_index_drop` | Drop::drop@0 |
| `test_stdlib_trait_method_index_partial_eq` | PartialEq::eq@0, ne@1 |
| `test_stdlib_trait_method_index_add` | Add::add@0, Sub::sub@0 (各 trait 独立) |
| `test_stdlib_trait_method_index_unknown_trait` | 未注册 trait → None |
| `test_stdlib_trait_method_index_unknown_method` | 已知 trait 未知方法 → None |
| `test_stdlib_trait_method_index_marker` | Markers → None (无 slot) |
| `test_stdlib_vtable_layout_clone` | Clone 完整布局 |
| `test_stdlib_vtable_layout_drop` | Drop 完整布局 |
| `test_stdlib_vtable_layout_marker_empty` | Marker 布局为空 Vec |
| `test_stdlib_vtable_layout_unknown` | 未知 trait → None |
| `test_stdlib_vtable_layout_deterministic` | 重复查询返回相同顺序 |
| `test_stdlib_vtable_layout_arith` | Add/Sub 各自方法名正确 |
| `test_stdlib_vtable_slot_count` | 各 trait slot count |
| `test_is_stdlib_marker_trait_true` | 6 markers 全为 true |
| `test_is_stdlib_marker_trait_false` | Clone/Drop/Add 等为 false |
| `test_is_stdlib_marker_trait_unknown` | 未注册 trait → false |
| `test_stdlib_traits_with_vtable_includes_clone` | 含 Clone/Drop/Add/Iterator |
| `test_stdlib_traits_with_vtable_excludes_markers` | 不含 Copy/Send/... |
| `test_stdlib_traits_with_vtable_count` | ≥ 20 个 trait 有 vtable |
| `test_stdlib_vtable_slot_struct` | StdlibVtableSlot 字段访问 |
| `test_stdlib_vtable_slot_eq` | PartialEq/Eq 派生 |

## 5. 委员会投票

- Architect: GO — §16 隔离合规，stdlib 自包含，确定性 slot 编号正确
- Tech Lead: GO — 1152 tests, 0 clippy warnings
- QA: GO — 22 新测试覆盖正/负/边界/markers/determinism
- Doc: GO — plan + gate-review + test plan + dev-log + worklog + RELEASE_NOTES + README + api-naming-standard 同步
- API Naming: GO — 全部新 API 遵循 §23

**5/5 GO → PASS**

## 6. 后续依赖

- **Stage 5.38+ (dyn Trait MIR lowering)**: codegen 调用 `stdlib_vtable_layout()`
  发射 `@.vtable.<trait>.<type>` 全局时确定 element count；调用
  `stdlib_trait_method_index()` 计算 method 调用的字节偏移。
- **Stage 5.39+ (typeck trait bound solving)**: 验证 dyn Trait method 调用的
  slot_index 在 vtable 范围内。

---

**审查完成**: 2026-07-23
