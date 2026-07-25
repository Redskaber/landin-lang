# Stage 7 Gate Review Round 6 (7.6) — User-defined trait dyn support (TD-018)

> **审查日期**: 2026-07-25 | **版本**: v0.14.5 → v0.14.6
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 126 unit + 1897 integration = 2023 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `03-type-system.md` §2.3 (Trait object) + `09-stdlib.md` (vtable 布局)。
TD-018: 当前 dyn Trait 仅支持 stdlib traits，用户自定义 trait 被静默跳过。

## 新增内容

### 1. `build_dyn_trait_method_calls_from_resolver` (新函数)

Stage 7.6 新增函数，扩展 dyn Trait 方法调用构建以支持**用户自定义 trait**：

- 对于 stdlib trait：使用 `stdlib_trait_methods` + `stdlib_trait_method_index`（Stage 5.36-5.37）
- 对于用户自定义 trait：使用 `TraitResolver.vtables` 查找方法 + vtable slot 索引

### 2. `build_dyn_trait_mir_plan_from_resolver` 更新

更新为使用 `build_dyn_trait_method_calls_from_resolver`（替代旧的
`build_dyn_trait_method_calls_from_fat_ptrs`），使 DynTraitMIRPlan 自动
支持用户自定义 trait。

### 3. 测试文件（§17.1）

`tests/v0/stage7/plan/user_defined_trait_dyn_tests.rs` — 8 个测试：

| 测试 | 内容 |
|------|------|
| `stage7_user_defined_trait_fat_ptr_generation` | 用户 trait fat ptr 生成 |
| `stage7_user_defined_trait_method_calls_from_resolver` | 方法调用 + slot 索引 |
| `stage7_user_defined_trait_slot_index_ordering` | slot 索引顺序 (0,1,2) |
| `stage7_user_defined_trait_empty_methods` | 空方法 trait |
| `stage7_user_defined_trait_multiple_traits` | 同类型多 trait |
| `stage7_regression_stdlib_traits_still_work` | stdlib trait 回归 |
| `stage7_user_defined_trait_method_call_fields` | 方法调用字段验证 |
| `stage7_user_defined_trait_multiple_types_same_trait` | 同 trait 多类型 |

## §23 + §16 合规

- `build_dyn_trait_method_calls_from_resolver` 遵循 `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` 模式
- 读 TraitResolver 数据（§16 允许 — 数据流下游）
- 1881 原有 tests 零回归

## TD-018 状态

**✅ TD-018 (用户自定义 trait dyn 支持) 完成**

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
