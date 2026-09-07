# Stage 150 开发日志 — TD-GENERIC-ENUM-MATCH-ARMS 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.673.0 → v0.674.0 |
| 测试数 | 5993 → 6002 (+9 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
修复 match arm pattern binding 对泛型枚举的 payload 类型不解析 — `Option::Some(v)` 中 `v` 被当作 `T` (Param) 而非具体类型 (如 `i64`).

### WHY
1. `resolve_enum_variant` 使用 `lower_hir_ty_to_mir_ty` (无泛型上下文), 对 `Some(T)` 的 `T` 产生 `Error` 而非 `Param(0)`.
2. `pattern_bindings` 直接使用此 `Error` 类型作为 binding local 的类型.

### HOW
1. `resolve_enum_variant`: 使用 `lower_hir_ty_to_mir_ty_with_hir_and_generics` + enum 的 generic_params, 使 `T` 解析为 `Param(0)`.
2. `pattern_bindings`: 从 scrutinee 的 `local_decl.ty` 提取具体 substs, 用 `substitute` 替换 `Param(0)` → 具体类型 (如 `i64`).

### 测试
9 tests: Option::Some arithmetic (2) + None arm (1) + user generic enum (1) + unwrap arithmetic (1) + Iterator sum (1) + both arms (1) + regression (2)

### 发现的新 TD
- TD-TRAIT-METHOD-RET-MATCH-GEP: Iterator sum 的 match arm 提取值不正确 (count works, sum returns garbage). P3, v0.16+.
- TD-OPTION-UNWRAP-OR-MATCH: unwrap_or 在 Some+None 组合时 Some 返回 0. P3, v0.16+.

### 下一步
Stage 151: TD-TRAIT-METHOD-RET-MATCH-GEP 或 TD-STDLIB-ITERATOR (添加 Iterator trait 到 prelude)
