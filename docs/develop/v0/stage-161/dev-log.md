# Stage 161 开发日志 — Stdlib 方法覆盖审计

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.684.0 → v0.685.0 |
| 测试数 | 6074 → 6087 (+13 new stage161 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H
### WHAT
审计 prelude 中所有 Option/Result/String/Vec 方法, 验证它们在类型注解下工作正确, 并记录已知限制.

### 发现
1. **所有 Option/Result/String/Vec 方法在有类型注解时工作正确** (13 tests 通过)
2. **新 TD: TD-INFERRED-TYPE-METHOD-MANGLING** — 当 receiver 类型是 Infer/Error 时 (无类型注解), method dispatch 使用 Error mangled name → linker error. 根因: `mangle_ty_with_interner` 将 Error 类型 mangle 为 "error". Workaround: 添加类型注解.

## §3.2 全套验收
| 命令 | 结果 |
|------|------|
| cargo build --release --features llvm-backend | success |
| cargo check --features llvm-backend | 0 errors, 0 warnings |
| cargo fmt --check | exit 0 |
| cargo clippy --all-targets --features llvm-backend -- -D warnings | 0 warnings |
| cargo test --release --features llvm-backend | 898 lib + 5189 integration = **6087 tests, 0 failures, 12 ignored** |
