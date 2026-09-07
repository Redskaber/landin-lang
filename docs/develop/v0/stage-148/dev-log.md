# Stage 148 开发日志 — TD-GENERIC-ENUM-PAYLOAD-SUBST 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.671.0 → v0.672.0 |
| 测试数 | 5960 → 5978 (+18 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
修复泛型 enum variant payload 类型替换 — Option<T> 和 Wrapper<T> 的 pattern matching 现在正确提取值。

### WHY
`codegen/rvalue.rs` 的 enum variant 构建路径使用空 substs 计算 storage_ty 和未替换的 field_tys (含 Param(N))。`mir_type_to_emit_type` 对 Param 返回 I32 (fallback), 导致 `insertvalue` 使用错误 payload 类型 (I32 而非 I64) → 值截断。

### HOW
1. `storage_ty` 使用 `adt_substs` (而非空 substs) — Stage 148
2. `field_tys` 通过 `substitute(field_ty, adt_substs)` 替换 Param — Stage 148
3. 18 tests: generic enum value extraction (5) + Option pattern matching (6) + Iterator trait (4) + regression (3)

### 发现的新 TD
- TD-GENERIC-ENUM-MATCH-ARMS: match arm pattern binding for generic enums 不解析 payload type T → i64 (P3, v0.16+)
- TD-TRAIT-METHOD-REMONO-LINK: Stage 147 bodyless trait 方法获得新 DefId 后, vtable 引用 `landin_Iterator_Counter_next` 但函数未 emit (P3, v0.16+)

### 下一步
Stage 149: TD-TRAIT-METHOD-REMONO-LINK (Stage 147 回归) 或 TD-GENERIC-ENUM-MATCH-ARMS
