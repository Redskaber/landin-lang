# Stage 95 开发日志 — PartialEq + Eq traits added to prelude

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.633.0 → v0.634.0 |
| 测试数 | 5568 → 5572 (+4) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC | +15 prelude.rs, +73 test |

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/stdlib/prelude.rs` | 添加 `PartialEq<Rhs>` trait + 4 impls; `Eq` trait (无 supertrait) + 4 marker impls |
| `tests/v0/stage95/plan/partial_eq_eq_trait_tests.rs` | 新建 — 4 tests |
| `Cargo.toml` | 版本 → 0.634.0 |

## 关键决策记录

### 决策 1: Eq 不带 supertrait (Eq: PartialEq<Self> ❌ vs Eq {} ✅)

**情境**: Rust 标准 `trait Eq: PartialEq<Self> {}`, 但 Landin 加 supertrait 后 2 个 stage16_78 supertrait 测试失败。

**选择**: `trait Eq {}` (无 supertrait)。

**根因**: Landin 没有 automatic trait resolution, supertrait 仅在 object safety 分析中起作用, 在 Eq 上加 PartialEq<Self> supertrait 导致 resolver 在 trait 声明查找时找到 prelude Eq 而非 user-defined trait。

**理由** (§12 最优>最小):
- 根因修复: Landin trait resolution 模型与 Rust 不同, supertrait 在 Landin 中无语义意义, 不应加。
- 通解: 适用于所有 v0.9 trait, 不带 supertrait 是一致性方案。
- 正确性: 用户独立 impl PartialEq + Eq — 与 Landin v0.9 MVP 一致, == operator 重载推迟到 v0.10+。

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage95_partial_eq_eq_traits_declared` | 正向 | trait + impls 编译通过 |
| `stage95_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage95_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage95_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## 下一步

- Stage 96: 添加 Ord (marker trait)
- Stage 97: TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 调查
