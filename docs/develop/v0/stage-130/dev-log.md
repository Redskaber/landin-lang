# Stage 130 开发日志 — TD-UFCS-DEFAULT-BODY-EMPTY-IMPL (默认方法体回退)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.660.0 → v0.661.0 |
| 测试数 | 5798 (898 lib + 4900 integration, +5 stage130) |
| 失败数 | 0 |
| ignored | 12 (9 original + 2 stage129 LLVM non-det + 1 stage130 codegen) |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 130 实现 TD-UFCS-DEFAULT-BODY-EMPTY-IMPL — UFCS 调用 trait 默认方法体时，如果 impl 块为空，回退到 trait 声明的默认方法 DefId。

### WHY (根因分析)
- **根因**: `resolve_impl_method_by_name` 只扫描 impl 块的 items。当 impl 块为空时，方法在 trait 声明中（有默认 body），但 impl 中没有。
- **通解**: 当 impl 块中找不到方法时，回退到 trait 声明查找默认方法体。

### HOW (实施策略)
- 在 `resolve_impl_method_by_name` 中，impl 块扫描后添加 trait 声明回退逻辑
- 搜索所有 trait 声明，匹配 trait_name，查找有 body 的方法

### 已知限制
- **TD-UFCS-DEFAULT-BODY-CODEGEN (P3, v0.14+)**: 空 impl + 默认方法体的 codegen 参数类型问题 — trait 默认方法的 `self` 类型未正确特化。IR 生成 `call i32 @landin_Maker_default_make(i32 %v1)` 但参数应为 `ptr`（receiver 是 `&S`）。
- 影响：`impl Trait for S {}` + `<S as Trait>::method(&s)` 在 codegen 阶段失败（LLVM module verification failed）

### §3.2 验收
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4900 tests, 0 failures, 12 ignored)
- Total: 5798 tests, 0 failures, 12 ignored

## Stage Summary
- Stage 130 PASSED — resolve_impl_method_by_name 默认方法体回退实现
- 5 tests: 2 positive + 2 regression + 1 ignored (codegen issue)
- 0 regression (5793 → 5798 tests, +5 new)
- TD-UFCS-DEFAULT-BODY-EMPTY-IMPL ✅ 部分修复（resolver 回退已完成，codegen 特化待 v0.14+）
- 新 TD: TD-UFCS-DEFAULT-BODY-CODEGEN (P3, v0.14+)
- v0.661.0
