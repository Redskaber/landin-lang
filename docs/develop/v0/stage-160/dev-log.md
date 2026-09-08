# Stage 160 开发日志 — TD-TYPECK-GENERIC-ARG-VALIDATION 完整修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.683.0 → v0.684.0 |
| 测试数 | 6066 → 6074 (+8 new stage160 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~30 (check.rs + unify.rs) + ~150 (8 tests) |

## 5W2H
### WHAT
修复 `identity::<i64>(42i32)` 等泛型调用的实参类型不匹配 turbofish 期望的类型时, typeck 静默接受而非报错.

### WHY
`typeck/check.rs` check_terminator 的 Call arg validation 用 `sig.inputs` 直接 unify (含 `Param(N)`), `unify(Param(0), i32)` 静默成功 (Param unifies with anything).

### HOW
1. `typeck/check.rs`: 当 func_ty 是 `FnDef(def_id, substs)` 且 substs 非空时, 用 `substitute(sig.inputs, substs)` 特化 inputs 后再 unify.
2. `typeck/unify.rs`: `bind_ty_var` 添加 bounds-safe access (Param index 超出 ty_vars.len 时跳过绑定而非 panic).

### 决策点 (§12 最优 > 最小, §1.0 原則 4/9/10)
1. 特化 inputs 后 unify (§1.0 原則 9) — 正确验证, 不依赖 Param 万能匹配
2. bounds-safe bind_ty_var (§1.0 原則 4) — 不 panic, defer to param_check

## §3.2 全套验收
| 命令 | 结果 |
|------|------|
| cargo build --release --features llvm-backend | success (32s) |
| cargo check --features llvm-backend | 0 errors, 0 warnings |
| cargo fmt --check | exit 0 |
| cargo clippy --all-targets --features llvm-backend -- -D warnings | 0 warnings |
| cargo test --release --features llvm-backend | 898 lib + 5176 integration = **6074 tests, 0 failures, 12 ignored** |
