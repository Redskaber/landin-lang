# Stage 158 开发日志 — TD-VTABLE-DEFAULT-BODY-MISSING-ENTRY 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.681.0 → v0.682.0 |
| 测试数 | 6046 → 6056 (+10 new stage158 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~50 (resolver.rs) + ~200 (10 tests) |

## 5W2H

### WHAT
修复 dyn dispatch 调用 trait default body 方法时, vtable 缺少 default body 方法的 entry, 导致 vtable 为空 → matched_call=None → 静态调用 → fat pointer 传入 thin pointer 参数 → Call parameter type mismatch.

### WHY
`traits/resolver.rs` vtable 构建只遍历 impl items, 不遍历 trait items 的 default body. 当 impl 没有覆盖 trait 的 default body 方法时 (如 `impl Trait for S {}`), vtable 为空.

### HOW (修复方案)
在 `traits/resolver.rs` vtable_entries 构建完成后:
1. 获取 trait 的 DefId (通过 `trait_by_name` 查找)
2. 遍历 trait items, 对有 default body (body.is_some()) 的方法:
   - 如果 impl 已覆盖 (method_names 包含), 跳过
   - 否则, 添加 vtable entry (fn_name = `landin_{trait}_default_{method}`)

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **遍历 trait items 添加 default body entry** (§1.0 原則 9 正确 > 妥协) — 不留空 vtable, 而是填充 default body 方法.
2. **使用 `landin_{trait}_default_{method}` 命名** (§1.0 原則 6 通解 > 特解) — 与 `populate_trait_default_fn_sigs` 的命名一致.
3. **clone trait_str 避免 borrow conflict** (§1.0 原則 10) — `interner.try_resolve` 返回 `&str` (immutable borrow), `interner.get_or_intern` 需要 `&mut`.

### 裁剪点 (§1.2.1)
L2 任务 (~50 LOC + 10 tests), 单轮收敛. 跳过 §14.6 跨阶段验证.

### 验证
- **修复前**: `use_getter(&p)` 调用 `g.get_x()` (default body) 导致 LLVM verification error (Call parameter type mismatch: fat pointer vs thin pointer)
- **修复后**: 返回 42 (正确)
- **同时验证**: TD-DEFAULT-BODY-FIELD-ACCESS 已被 Stage 157 间接修复 (字段访问返回正确值)

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success (35s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings |
| `cargo test --release --features llvm-backend` | 898 lib + 5158 integration = **6056 tests, 0 failures, 12 ignored** |
