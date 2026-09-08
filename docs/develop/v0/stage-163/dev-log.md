# Stage 163 开发日志 — TD-INFERRED-TYPE-METHOD-MANGLING 部分修复

## 阶段统计
| 指标 | 值 |
|------|-----|
| 版本 | v0.686.0 → v0.687.0 |
| 测试数 | 6087 → 6091 (+4 new stage163 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~120 (method_call_lower.rs + method_resolution.rs + codegen/function.rs) + ~100 (4 tests) |

## 5W2H
### WHAT
部分修复 TD-INFERRED-TYPE-METHOD-MANGLING — 当 receiver 类型是 Infer/Error 时, method dispatch 产生 Error mangled name → linker error.

### 根因
1. `let r = none.or(some)` 无类型注解 → `r` 的 MIR 类型是 `Adt(Option, [Error])`
2. `r.unwrap()` → Strategy 1 找到 method (type name "Option"), 但 FnDef substs 含 Error
3. mono name 生成: `Option_unwrap_error` (Error → "error")
4. linker: undefined reference to `Option_unwrap_error`

### 部分修复
1. method_call_lower.rs: Stage 163 fallback — 当 recv_ty 是 Infer/Error 时, 尝试 find_local_init_type + resolve_inherent_method
2. method_resolution.rs: 添加 type_contains_param_pub helper
3. codegen/function.rs re_resolve: 添加 Error-type 分支 + substs fixup

### 未完全修复的原因
- method_def_id 是 Some (Strategy 1 通过 Adt type name 查找成功) — else 分支不被触发
- re_resolve 无法找到 inherent method (trait_method_map 只映射 trait methods)
- FnDef substs 仍含 Error — fixed_substs 逻辑仅在 re_resolve 成功后才执行

### 完全修复需要
- writeback 解析 Error substs in Adt types (从 Call dest 的返回类型推断)
- 或 codegen 使用 receiver 的 resolved type 生成 mono name

### 决策点 (§12 最优 > 最小, §1.0 原則 4/9/15)
1. 部分修复 + 注册已知限制 (§1.0 原則 15) — 正确 > 妥协, 但完全修复需要 writeback 架构改造
2. 添加 fallback 路径 (§1.0 原則 6) — 通解, 不针对单个方法
3. 记录 workaround (§1.0 原則 4) — 显式说明类型注解 workaround

## §3.2 全套验收
| 命令 | 结果 |
|------|------|
| cargo build --release --features llvm-backend | success (35s) |
| cargo check --features llvm-backend | 0 errors, 0 warnings |
| cargo fmt --check | exit 0 |
| cargo clippy --all-targets --features llvm-backend -- -D warnings | 0 warnings |
| cargo test --release --features llvm-backend | 898 lib + 5193 integration = **6091 tests, 0 failures, 12 ignored** |
