# Stage 96 开发日志 — Ord trait (marker) added to prelude

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.634.0 → v0.635.0 |
| 测试数 | 5572 → 5576 (+4) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC | +10 prelude.rs, +56 test |

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/stdlib/prelude.rs` | 添加 `trait Ord {}` + 4 marker impls |
| `tests/v0/stage96/plan/ord_trait_tests.rs` | 新建 — 4 tests |
| `Cargo.toml` | 版本 → 0.635.0 |

## 关键发现 — 新 TD

### TD-PRELUDE-TRAIT-IMPL-CODEGEN-CRASH (P3 → 后续升级为 P2)

**现象**: 任何 prelude impl method 含复杂控制流 (if/match) 返回 String/Option/i32 触发 codegen SIGSEGV。

**根因初判**: codegen 对 prelude impl method body 的处理不完整。

**复现**: `impl Debug for i32 { fn fmt(&self) -> String { if *self == 0 { String::from_str("zero") } else { String::from_str("nonzero") } } }` → SIGSEGV in lib tests.

**临时方案**: 只添加 marker traits (无 body), Stage 97 转根因调查。

**升级路径**: TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH (P2, v0.10+) — Stage 97 调查。

## 决策记录

### 决策 1: 只添加 Ord (marker, no method body)

**理由** (§12 最优>最小):
- 先验证 marker trait 无回归。
- Debug + PartialOrd impl bodies (returning String/Option) → SIGSEGV。
- 不阉割 — Stage 97 转根因分析。

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage96_ord_trait_declared` | 正向 | trait + marker impls 编译通过 |
| `stage96_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage96_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage96_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## 下一步

- Stage 97: TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 根因调查
- Stage 98: 修复根因
- Stage 99+: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (新发现)
