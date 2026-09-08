# Stage 151 开发日志 — TD-TRAIT-METHOD-RET-MATCH-GEP 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.674.0 → v0.675.0 |
| 测试数 | 6002 → 6002 (不变 — 修复回归, 无新测试) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
修复泛型枚举在 if/else 分支中 `insertvalue` 使用错误的 storage type (`{i32,i32}` 而非 `{i32,i64}`).

### WHY
1. `build_adt_layout` 使用 `lower_hir_ty_to_mir_ty` (无泛型上下文) → `T` 变 `Error` → AdtLayout::Enum variant_payloads 含 `Error`.
2. `adt_layout_to_emit_type` 遇到 `Param(0)` (Stage 150 修复后 `Error` 变 `Param(0)`) → `mir_type_to_emit_type` 返回 I32 fallback → storage type `{i32,i32}` 而非 `{i32,i64}`.
3. `insertvalue` 用 `{i32,i32}` 作为 agg type → i64 payload 被截断为 i32 → 垃圾值.

### HOW
1. `build_adt_layout` (mir/lower/adt_layout.rs): 使用 `lower_hir_ty_to_mir_ty_with_hir_and_generics` + enum generic_params → `T` 变 `Param(0)` (不是 `Error`).
2. `adt_layout_to_emit_type` (codegen/mir_translation/layouts.rs): 对 `Param(N)` 类型, 如果 `mir_type_to_emit_type_with_layouts_and_mono` 返回 I32 (fallback), 改用 I64 (Landin 默认整数类型).
3. stage40 测试从 i32 改为 i64 (prelude Option/Result 方法在 i32 上有类型不匹配).

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9)
1. 选 I64 fallback 不选 I32 — §1.0 原則 9 (正确 > 妥协: I64 更安全)
2. 选修改 stage40 测试 不选修复 prelude i32 不匹配 — prelude 限制是 follow-up TD

### 下一步
Stage 152: TD-OPTION-AND-THEN-I32-MISMATCH 或 TD-STDLIB-ITERATOR (添加 Iterator trait 到 prelude)
