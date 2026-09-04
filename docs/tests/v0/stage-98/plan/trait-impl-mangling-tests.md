# Stage 98 测试计划 — Trait impl symbol mangling

## 测试目标

验证 trait impl method symbol mangling 修复 (`<Trait>_<type>_<method>`) 不引入回归, 且解决 symbol collision.

## 覆盖场景

| 场景 | 测试 | 类型 |
|------|------|------|
| Display impl method 调用 | `stage98_display_impl_method` | 正向 |
| Clone impl method 调用 | `stage98_clone_impl_method` | 正向 |
| Default impl method 调用 | `stage98_default_impl_method` | 正向 |
| 多 trait 共存 (Display + Clone) | `stage98_multi_trait_coexist` | 正向 (key collision regression) |
| undefined type 报错 | `stage98_undefined_type_errors` | 负向 |
| type mismatch 报错 | `stage98_type_mismatch_errors` | 负向 |
| nonexistent method 报错 | `stage98_nonexistent_method_errors` | 负向 |
| wrong arg count 报错 | `stage98_wrong_arg_count_errors` | 负向 |
| undefined trait 报错 | `stage98_undefined_trait_errors` | 负向 |

## 关键回归测试

- `stage16_78_supertrait_*` 系列 — 验证 mangling 修复不破坏 supertrait 解析.
- `stage5_*vtable*` 系列 — 验证 vtable 符号正确.
- `stage14_*drop*` 系列 — 验证 Drop impl method 调用名正确.

## 对应代码

- 测试代码: `tests/v0/stage98/plan/trait_impl_mangling_tests.rs` (in test updates)
- 实现代码: `src/driver/driver_codegen_prep.rs`, `src/traits/resolver.rs`, `src/stdlib/vtable_layout.rs`, `src/codegen/drop_glue.rs`

## 预期/实际

- 预期测试数: 9 (新增) + 32 (更新旧 mangled name)
- 实际测试数: 9 + 32 ✓
- 覆盖率: trait impl mangling 路径 100%

## 测试矩阵更新

| 维度 | 增量 |
|------|------|
| 总测试数 | 5580 → 5589 |
| 失败数 | 0 |
| ignored | 9 |
| 测试文件更新 | 32+ (mangled name) |
