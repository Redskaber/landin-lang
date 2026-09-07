# Stage 149 开发日志 — TD-TRAIT-METHOD-GENERIC-RET-SKIP 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.672.0 → v0.673.0 |
| 测试数 | 5978 → 5993 (+15 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
修复 trait impl 方法返回泛型枚举 (Option<T>) 时被 codegen 错误跳过 (linker error).

### WHY
`mir_body_contains_param_type` 检查 `Aggregate` 的 substs 和 field_tys 是否含 Param. 对 trait impl 方法返回 `Option<T>`, Aggregate substs 含 `Param(0)` (来自 Option<T> 泛型参数 T), 导致方法被误判为 "generic" 并被跳过 emit.

### HOW
`statement_contains_param` 的 `Aggregate` 分支不再检查 substs/field_tys — 仅检查 operands (实际值). Param 在 type metadata 中是 typeck 限制, 不是方法 generic 的标志.
- §1.0 原則 6 (通解 > 特解): 只检查 operands, 不检查 type metadata
- §1.0 原則 9 (正确 > 妥协): 用户函数总是 emit, 即使有 Param 泄漏
- §1.0 原則 4 (报错 > 静默): Param 警告通过 mir_type_to_emit_type 发出

### 测试
15 tests: trait 方法返回 Option with if/else (5) + without if/else (3) + Iterator trait (4) + regression (3)

### 下一步
Stage 150: TD-GENERIC-ENUM-MATCH-ARMS (match arm pattern binding 不解析泛型枚举 payload type T → concrete)
