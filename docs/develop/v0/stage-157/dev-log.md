# Stage 157 开发日志 — TD-DEFAULT-BODY-SELF-TYPE 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.680.0 → v0.681.0 |
| 测试数 | 6038 → 6046 (+8 new stage157 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~80 (mod.rs + compile_inner.rs) + ~200 (8 tests) |

## 5W2H

### WHAT
修复 trait default body 方法的 `&self` 参数类型解析为 `Error` 而非 impl 的 self_ty. 导致 codegen 将 `&self` 当作 `i32` → Call parameter type mismatch (`i32` vs `ptr`).

### WHY
`compile_inner.rs:194-218` 构建 `fn_sig_table` 时, 对 `&self` 参数调用 `resolve_self_param_type_for_sig`. 该函数从 `method_to_impl_index` 查找方法的 owner impl. 但 trait default body 方法在 trait 声明中 (不在 impl 中), 所以不在 `method_to_impl_index` 中 → 返回 `None` → fallback 到 `Error` 类型.

### HOW (修复方案)
在 `driver/mod.rs` 添加 `resolve_default_body_self_type` 函数. 当 `resolve_self_param_type_for_sig` 返回 `None` 时:
1. 遍历 HIR owners 查找声明该方法的 trait
2. 找到该 trait 的第一个 impl
3. 用 impl 的 self_ty 作为 `&self` 参数类型 (with Ref wrapping)

在 `compile_inner.rs:194-218` 的 fallback 链中添加 `.or_else(|| resolve_default_body_self_type(...))`.

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **查找 trait 的第一个 impl** (§1.0 原則 9 正确 > 妥协) — 不返回 Error, 而是从 impl 推断正确类型. v0.1 限制: 多 impl 时只用第一个 (已在 `populate_trait_default_fn_sigs` 中有 warning).
2. **不修改 `resolve_self_param_type_for_sig`** (§1.0 原則 6 通解 > 特解) — 保持现有逻辑, 添加 fallback 层. 这样不影响 impl 方法的解析.
3. **使用 `lower_hir_ty_to_mir_ty_with_hir`** (§1.0 原則 10 唯一可信数据源) — HIR 是类型定义的权威来源.

### 裁剪点 (§1.2.1)
L2 任务 (~80 LOC + 8 tests), 单轮收敛. 跳过 §14.6 跨阶段验证.

### 验证
- **修复前**: `e.greet()` (default body) 导致 LLVM verification error (Call parameter type mismatch: i32 vs ptr)
- **修复后**: `e.greet()` 返回 42 (正确); `e.greet() + e.name()` 返回 49 (42+7)

### 发现的新 TD
- **TD-VTABLE-DEFAULT-BODY-MISSING-ENTRY** (P3, v0.16+): dyn dispatch 调用 default body 方法时, vtable 缺少 entry. 静态调用已修复, 动态调用仍失败.
- **TD-DEFAULT-BODY-FIELD-ACCESS** (P3, v0.16+): default body 方法内部访问 `self.x` 等字段时返回错误值 (1 而非实际字段值). `&self` 参数类型已正确, 但字段访问 GEP 仍有问题.

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success (33s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings |
| `cargo test --release --features llvm-backend` | 898 lib + 5148 integration = **6046 tests, 0 failures, 12 ignored** |
