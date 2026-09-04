# Stage 94 开发日志 — Default trait added to prelude

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.632.0 → v0.633.0 |
| 测试数 | 5564 → 5568 (+4) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC | +20 prelude.rs, +80 test |

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/stdlib/prelude.rs` | 添加 `trait Default { fn default() -> Self; }` + 4 impls (i32/i64/bool/usize) |
| `tests/v0/stage94/plan/default_trait_tests.rs` | 新建 — 4 tests (1 positive + 3 negative) |
| `Cargo.toml` | 版本 → 0.633.0 |

## 关键决策记录

### 决策 1: 只添加 Default，不添加 PartialEq/Eq

**情境**: TD-PRELUDE-TRAIT-COVERAGE 需要添加 Default, PartialEq, Eq, Hash, Ord, From, Into。

**选择**: 只添加 Default (4 impls)，PartialEq/Eq 延后。

**理由** (§12 最优>最小, §1.0 原则 9 正确>妥协):
- PartialEq/Eq 有 supertrait (`Eq: PartialEq<Self>`) 影响 object safety 分析。
- 实测添加 `Eq: PartialEq<Self>` 导致 2 个 lib test 失败 (`stage16_78_supertrait*`)。
- 先验证 Default 无回归，再设计 object safety 修复方案。

**未来路径** (Stage 95): Eq 不带 supertrait (Landin 无 automatic trait resolution, supertrait 仅影响 object safety 分析)。

### 决策 2: impl Default for usize 用 `0usize` 字面量

**情境**: `usize` 在 Landin 中默认推断为 i64，但 Default::default() 应返回 usize 类型。

**选择**: 显式 `0usize` 字面量。

**理由** (§1.0 原则 3 显式>隐式): 避免类型推断歧义。

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage94_default_trait_declared` | 正向 | trait + impls 编译通过 |
| `stage94_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage94_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage94_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## 下一步

- Stage 95: 添加 PartialEq + Eq (Eq 不带 supertrait)
- Stage 96: 添加 Ord (marker)
- Stage 97-98: 修复 TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH
